#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath {
    value: String,
}

impl FieldPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, FieldPathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FieldPathError::new("empty_field_path"));
        }
        if value.contains('[')
            || value.contains(']')
            || value.contains('*')
            || value.contains('(')
            || value.contains(')')
        {
            return Err(FieldPathError::new("unsupported_field_path"));
        }
        if value
            .split('.')
            .any(|segment| segment.is_empty() || !is_valid_segment(segment))
        {
            return Err(FieldPathError::new("invalid_field_path"));
        }
        Ok(Self { value })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.value.split('.')
    }
}

impl TryFrom<&str> for FieldPath {
    type Error = FieldPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPathError {
    pub code: &'static str,
}

impl FieldPathError {
    fn new(code: &'static str) -> Self {
        Self { code }
    }
}

fn is_valid_segment(segment: &str) -> bool {
    segment
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_path_accepts_simple_dot_path() {
        let path = FieldPath::parse("stats.attack").expect("path should parse");
        assert_eq!(path.segments().collect::<Vec<_>>(), vec!["stats", "attack"]);
    }

    #[test]
    fn field_path_rejects_array_index_path() {
        let error = FieldPath::parse("inventory[3].count").expect_err("path should fail");
        assert_eq!(error.code, "unsupported_field_path");
    }
}
