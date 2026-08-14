use serde_json::{Map, Value};
fn is_supported_component_field_path(field_path: &str) -> bool {
    let trimmed = field_path.trim();
    !trimmed.is_empty()
        && !trimmed.contains("..")
        && trimmed.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        })
}

fn set_json_object_path(object: &mut Map<String, Value>, field_path: &str, value: Value) {
    let mut segments = field_path.split('.').peekable();
    let mut current = object;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            current.insert(segment.to_string(), value);
            return;
        }
        let entry = current
            .entry(segment.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        current = entry
            .as_object_mut()
            .expect("entry was converted to object");
    }
}


