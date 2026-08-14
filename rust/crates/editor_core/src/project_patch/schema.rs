use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde_json::{Map, Value};

use super::ProjectPatchDocument;

pub fn project_patch_json_schema() -> Value {
    let root = schemars::schema_for!(ProjectPatchDocument);
    let mut schema =
        serde_json::to_value(root).expect("ProjectPatchDocument generated schema must serialize");
    if let Value::Object(object) = &mut schema {
        object.remove("$schema");
        object.remove("title");
    }
    normalize_strict_objects(&mut schema);
    canonicalize_json(&mut schema);
    schema
}

pub fn project_patch_json_schema_string() -> String {
    serde_json::to_string(&project_patch_json_schema())
        .expect("ProjectPatchDocument generated schema must serialize")
}

pub fn project_patch_json_schema_hash() -> String {
    let schema = project_patch_json_schema();
    let bytes = canonical_json_bytes(&schema)
        .expect("ProjectPatchDocument generated schema must be canonical JSON");
    sha256_prefixed(&bytes)
}

fn normalize_strict_objects(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get("properties") {
                let required = properties.keys().cloned().map(Value::String).collect();
                object.insert("required".to_string(), Value::Array(required));
                object.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            for child in object.values_mut() {
                normalize_strict_objects(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_strict_objects(item);
            }
        }
        _ => {}
    }
}

fn canonicalize_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, mut child) in entries {
                canonicalize_json(&mut child);
                sorted.insert(key, child);
            }
            *object = sorted;
        }
        Value::Array(items) => {
            for item in items {
                canonicalize_json(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_patch_schema_is_strict_complete_and_stable() {
        let schema = project_patch_json_schema();
        let encoded = serde_json::to_string(&schema).unwrap();

        for domain in ["scene", "input", "asset", "prefab", "aui", "rule", "build"] {
            assert!(encoded.contains(domain), "missing domain {domain}");
        }
        assert!(encoded.contains("additionalProperties"));
        assert!(encoded.contains("required"));
        assert!(
            encoded.contains("null"),
            "Option fields must remain nullable"
        );
        assert_eq!(
            project_patch_json_schema_hash(),
            project_patch_json_schema_hash()
        );
    }

    #[test]
    fn llm_patch_schema_marks_every_object_property_required() {
        fn inspect(value: &Value) {
            match value {
                Value::Object(object) => {
                    if let Some(Value::Object(properties)) = object.get("properties") {
                        assert_eq!(
                            object.get("additionalProperties"),
                            Some(&Value::Bool(false))
                        );
                        let required = object
                            .get("required")
                            .and_then(Value::as_array)
                            .expect("object properties must have required list");
                        for key in properties.keys() {
                            assert!(required.iter().any(|item| item.as_str() == Some(key)));
                        }
                    }
                    for child in object.values() {
                        inspect(child);
                    }
                }
                Value::Array(items) => items.iter().for_each(inspect),
                _ => {}
            }
        }

        inspect(&project_patch_json_schema());
    }
}
