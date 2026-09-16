use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedLint {
    pub fingerprint: String,
    pub lint_code: String,
    pub relative_path: String,
    pub anchor_hash: String,
    pub message: String,
}

pub fn parse_cargo_json(input: &[u8]) -> Result<Vec<ObservedLint>, String> {
    let text = std::str::from_utf8(input).map_err(|error| error.to_string())?;
    let mut observed = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|error| format!("line {}: {error}", index + 1))?;
        if value.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(message) = value.get("message") else {
            return Err(format!(
                "line {}: compiler-message missing message",
                index + 1
            ));
        };
        if message.get("level").and_then(Value::as_str) != Some("warning") {
            continue;
        }
        let lint_code = message
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("line {}: warning missing lint code", index + 1))?;
        let diagnostic_message = message
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("line {}: warning missing message text", index + 1))?;
        let spans = message
            .get("spans")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("line {}: warning missing spans", index + 1))?;
        let span = spans
            .iter()
            .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
            .ok_or_else(|| format!("line {}: warning missing primary span", index + 1))?;
        let relative_path = normalize_path(
            span.get("file_name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("line {}: primary span missing path", index + 1))?,
        );
        if is_absolute_path(&relative_path) {
            return Err(format!(
                "line {}: diagnostic path is not workspace-relative: {relative_path}",
                index + 1
            ));
        }
        let anchor = span
            .get("text")
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(|line| line.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let anchor = normalize_whitespace(&anchor);
        let message = normalize_whitespace(diagnostic_message);
        let tool = if lint_code.starts_with("clippy::") {
            "clippy"
        } else {
            "rustc"
        };
        let canonical = [tool, lint_code, &relative_path, &message, &anchor].join("\n");
        observed.push(ObservedLint {
            fingerprint: format!("sha256:{}", sha256_hex(canonical.as_bytes())),
            lint_code: lint_code.to_string(),
            relative_path,
            anchor_hash: format!("sha256:{}", sha256_hex(anchor.as_bytes())),
            message,
        });
    }
    Ok(observed)
}

pub fn occurrence_counts(items: &[ObservedLint]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item.fingerprint.clone()).or_default() += 1;
    }
    counts
}

pub fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_path(value: &str) -> String {
    let normalized = normalize_whitespace(&value.replace('\\', "/"));
    let mut parts = Vec::new();
    let mut leading_parents = 0_usize;
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    leading_parents += 1;
                }
            }
            other => parts.push(other),
        }
    }
    let mut result = "../".repeat(leading_parents);
    result.push_str(&parts.join("/"));
    result
}

pub fn is_absolute_path(value: &str) -> bool {
    value.starts_with('/')
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|separator| *separator == b':')
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warning_json(path: &str, line_start: u64, anchor: &str, message: &str) -> String {
        serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "warning",
                "message": message,
                "code": {"code": "clippy::example"},
                "spans": [{
                    "is_primary": true,
                    "file_name": path,
                    "line_start": line_start,
                    "text": [{"text": anchor}]
                }]
            }
        })
        .to_string()
    }

    #[test]
    fn fingerprint_ignores_line_numbers_and_slash_style() {
        let first =
            parse_cargo_json(warning_json("crates\\a.rs", 4, " let x = 1; ", "same").as_bytes())
                .unwrap();
        let second =
            parse_cargo_json(warning_json("crates/a.rs", 99, "let   x = 1;", "same").as_bytes())
                .unwrap();
        assert_eq!(first[0].fingerprint, second[0].fingerprint);
    }

    #[test]
    fn semantic_anchor_change_changes_fingerprint() {
        let first =
            parse_cargo_json(warning_json("crates/a.rs", 4, "let x = 1;", "same").as_bytes())
                .unwrap();
        let second =
            parse_cargo_json(warning_json("crates/a.rs", 4, "let x = 2;", "same").as_bytes())
                .unwrap();
        assert_ne!(first[0].fingerprint, second[0].fingerprint);
    }

    #[test]
    fn malformed_json_fails_closed() {
        assert!(parse_cargo_json(b"{not-json}").is_err());
    }

    #[test]
    fn absolute_diagnostic_path_fails_closed() {
        let input = warning_json("C:\\repo\\a.rs", 1, "x", "same");
        assert!(parse_cargo_json(input.as_bytes()).is_err());
    }
}
