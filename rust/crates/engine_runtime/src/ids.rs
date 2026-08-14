use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeEntityId {
    pub index: u32,
    pub generation: u32,
}

impl RuntimeEntityId {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
}

impl fmt::Display for RuntimeEntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.index, self.generation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceEntityId(String);

impl SourceEntityId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SourceEntityId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SourceEntityId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for SourceEntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

pub type EntityId = SourceEntityId;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_entity_id_generation_prevents_stale_reuse() {
        let first = RuntimeEntityId::new(7, 1);
        let reused = RuntimeEntityId::new(7, 2);
        assert_ne!(first, reused);
        assert_eq!(first.index, reused.index);
    }

    #[test]
    fn source_entity_id_roundtrip() {
        let id = SourceEntityId::from("entity-player");
        assert_eq!(id.as_str(), "entity-player");
        assert_eq!(id.to_string(), "entity-player");
    }

    #[test]
    fn runtime_entity_id_orders_deterministically() {
        let mut ids = vec![
            RuntimeEntityId::new(2, 0),
            RuntimeEntityId::new(1, 1),
            RuntimeEntityId::new(1, 0),
        ];
        ids.sort();
        assert_eq!(
            ids,
            vec![
                RuntimeEntityId::new(1, 0),
                RuntimeEntityId::new(1, 1),
                RuntimeEntityId::new(2, 0),
            ]
        );
    }
}
