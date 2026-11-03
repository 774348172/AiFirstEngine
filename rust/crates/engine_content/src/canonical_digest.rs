use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::fmt;

pub const CONSISTENCY_DIGEST_SCHEMA_VERSION: &str = "consistency-digest.v1";
pub const CANONICAL_DIGEST_ALGORITHM: &str = "sha256";
pub const CANONICAL_DIGEST_ENCODING: &str = "aife-canonical-framed.v1";
const PREIMAGE_MAGIC: &[u8] = b"AIFE-CONSISTENCY\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyDigest {
    pub schema_version: String,
    pub kind: String,
    pub algorithm: String,
    pub canonical_encoding: String,
    pub payload_schema_version: String,
    pub value: String,
}

impl ConsistencyDigest {
    pub fn sha256<T: Serialize>(
        kind: impl Into<String>,
        payload_schema_version: impl Into<String>,
        payload: &T,
    ) -> Result<Self, CanonicalDigestError> {
        let payload = serde_json::to_value(payload)
            .map_err(|error| CanonicalDigestError::Serialize(error.to_string()))?;
        Self::sha256_value(kind, payload_schema_version, &payload)
    }

    pub fn sha256_value(
        kind: impl Into<String>,
        payload_schema_version: impl Into<String>,
        payload: &Value,
    ) -> Result<Self, CanonicalDigestError> {
        let kind = kind.into();
        let payload_schema_version = payload_schema_version.into();
        let payload_bytes = canonical_json_bytes(payload)?;
        let mut hasher = Sha256::new();
        hasher.update(PREIMAGE_MAGIC);
        write_frame(&mut hasher, CONSISTENCY_DIGEST_SCHEMA_VERSION.as_bytes());
        write_frame(&mut hasher, kind.as_bytes());
        write_frame(&mut hasher, payload_schema_version.as_bytes());
        write_frame(&mut hasher, &payload_bytes);
        Ok(Self {
            schema_version: CONSISTENCY_DIGEST_SCHEMA_VERSION.to_string(),
            kind,
            algorithm: CANONICAL_DIGEST_ALGORITHM.to_string(),
            canonical_encoding: CANONICAL_DIGEST_ENCODING.to_string(),
            payload_schema_version,
            value: hex_lower(hasher.finalize().as_slice()),
        })
    }

    pub fn prefixed_value(&self) -> String {
        format!("{}:{}", self.algorithm, self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalDigestError {
    Serialize(String),
    NonFiniteNumber,
    ScopeViolation(String),
}

impl fmt::Display for CanonicalDigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(message) => {
                write!(formatter, "canonical serialization failed: {message}")
            }
            Self::NonFiniteNumber => {
                formatter.write_str("canonical encoding rejects non-finite numbers")
            }
            Self::ScopeViolation(message) => {
                write!(formatter, "canonical digest scope violation: {message}")
            }
        }
    }
}

impl std::error::Error for CanonicalDigestError {}

pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, CanonicalDigestError> {
    let mut output = Vec::new();
    write_canonical_value(&mut output, value)?;
    Ok(output)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(hasher.finalize().as_slice())
}

pub fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

pub fn payload_tree_digest<'a, I>(entries: I) -> Result<ConsistencyDigest, CanonicalDigestError>
where
    I: IntoIterator<Item = (&'a str, &'a [u8])>,
{
    let entries = entries
        .into_iter()
        .map(|(relative_path, bytes)| (relative_path, sha256_prefixed(bytes)))
        .collect::<Vec<_>>();
    Ok(file_hash_inventory_digest(
        "payload-tree",
        "payload-tree.v1",
        entries
            .iter()
            .map(|(relative_path, file_hash)| (*relative_path, file_hash.as_str())),
    ))
}

pub fn file_hash_inventory_digest<'a, I>(
    kind: &str,
    payload_schema_version: &str,
    entries: I,
) -> ConsistencyDigest
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    let mut hasher = Sha256::new();
    hasher.update(PREIMAGE_MAGIC);
    write_frame(&mut hasher, CONSISTENCY_DIGEST_SCHEMA_VERSION.as_bytes());
    write_frame(&mut hasher, kind.as_bytes());
    write_frame(&mut hasher, payload_schema_version.as_bytes());
    for (relative_path, file_hash) in entries {
        write_frame(&mut hasher, relative_path.as_bytes());
        write_frame(&mut hasher, file_hash.as_bytes());
    }
    ConsistencyDigest {
        schema_version: CONSISTENCY_DIGEST_SCHEMA_VERSION.to_string(),
        kind: kind.to_string(),
        algorithm: CANONICAL_DIGEST_ALGORITHM.to_string(),
        canonical_encoding: CANONICAL_DIGEST_ENCODING.to_string(),
        payload_schema_version: payload_schema_version.to_string(),
        value: hex_lower(hasher.finalize().as_slice()),
    }
}

pub fn sorted_object(entries: impl IntoIterator<Item = (String, Value)>) -> Value {
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Value::Object(entries.into_iter().collect::<Map<_, _>>())
}

fn write_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn write_canonical_value(output: &mut Vec<u8>, value: &Value) -> Result<(), CanonicalDigestError> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(boolean) => output.extend_from_slice(if *boolean { b"true" } else { b"false" }),
        Value::Number(number) => write_canonical_number(output, number)?,
        Value::String(string) => {
            let encoded = serde_json::to_string(string)
                .map_err(|error| CanonicalDigestError::Serialize(error.to_string()))?;
            output.extend_from_slice(encoded.as_bytes());
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_value(output, value)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                let encoded_key = serde_json::to_string(*key)
                    .map_err(|error| CanonicalDigestError::Serialize(error.to_string()))?;
                output.extend_from_slice(encoded_key.as_bytes());
                output.push(b':');
                write_canonical_value(output, &values[*key])?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn write_canonical_number(
    output: &mut Vec<u8>,
    number: &Number,
) -> Result<(), CanonicalDigestError> {
    if let Some(integer) = number.as_i64() {
        output.extend_from_slice(integer.to_string().as_bytes());
        return Ok(());
    }
    if let Some(integer) = number.as_u64() {
        output.extend_from_slice(integer.to_string().as_bytes());
        return Ok(());
    }
    let float = number
        .as_f64()
        .ok_or(CanonicalDigestError::NonFiniteNumber)?;
    if !float.is_finite() {
        return Err(CanonicalDigestError::NonFiniteNumber);
    }
    if float == 0.0 {
        output.push(b'0');
        return Ok(());
    }
    let encoded = serde_json::to_string(&float)
        .map_err(|error| CanonicalDigestError::Serialize(error.to_string()))?;
    output.extend_from_slice(encoded.as_bytes());
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_digest_sha256_known_vector_and_lowercase_hex() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let digest = ConsistencyDigest::sha256("test", "test.v1", &json!({ "value": 1 })).unwrap();
        assert_eq!(digest.value.len(), 64);
        assert!(digest
            .value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_eq!(digest.prefixed_value().len(), 71);
    }

    #[test]
    fn canonical_digest_ignores_json_whitespace_and_object_key_order() {
        let first: Value = serde_json::from_str("{\"b\": [2, 3], \"a\": 1}").unwrap();
        let second: Value = serde_json::from_str(" { \"a\" : 1, \"b\" : [2,3] }").unwrap();
        let first = ConsistencyDigest::sha256_value("test", "test.v1", &first).unwrap();
        let second = ConsistencyDigest::sha256_value("test", "test.v1", &second).unwrap();
        assert_eq!(first.value, second.value);
    }

    #[test]
    fn canonical_digest_separates_kind_schema_and_ordered_array_semantics() {
        let payload = json!({ "events": ["first", "second"] });
        let changed_order = json!({ "events": ["second", "first"] });
        let base =
            ConsistencyDigest::sha256_value("runtime-content", "payload.v1", &payload).unwrap();
        assert_ne!(
            base.value,
            ConsistencyDigest::sha256_value("other", "payload.v1", &payload)
                .unwrap()
                .value
        );
        assert_ne!(
            base.value,
            ConsistencyDigest::sha256_value("runtime-content", "payload.v2", &payload)
                .unwrap()
                .value
        );
        assert_ne!(
            base.value,
            ConsistencyDigest::sha256_value("runtime-content", "payload.v1", &changed_order)
                .unwrap()
                .value
        );
    }

    #[test]
    fn canonical_digest_normalizes_negative_zero() {
        let negative_zero = Value::Number(Number::from_f64(-0.0).unwrap());
        let zero = Value::Number(Number::from_f64(0.0).unwrap());
        assert_eq!(
            canonical_json_bytes(&negative_zero).unwrap(),
            canonical_json_bytes(&zero).unwrap()
        );
    }

    #[test]
    fn canonical_digest_payload_tree_is_path_order_independent() {
        let first =
            payload_tree_digest([("b.bin", b"b".as_slice()), ("a.bin", b"a".as_slice())]).unwrap();
        let second =
            payload_tree_digest([("a.bin", b"a".as_slice()), ("b.bin", b"b".as_slice())]).unwrap();
        assert_eq!(first.value, second.value);
    }
}
