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
// Source names are immutable content identities. Clones retain their backing;
// this is not interning, and independent equal names still compare by content.
pub struct SourceEntityId(std::sync::Arc<str>);

impl SourceEntityId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into().into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrowed immutable payload for the existing allocation census. Arc
    /// control blocks and allocator overhead are outside this data observation.
    pub(crate) fn storage(&self) -> &std::sync::Arc<str> {
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

#[cfg(test)]
mod shared_payload_regression {
    use super::*;
    #[test]
    fn identity_publication_clones_reuse_immutable_payload_at_scale() {
        for size in [1usize, 100, 1000, 10000] {
            let original = SourceEntityId::new("long-lived-source-identity-payload");
            let clones = (0..size).map(|_| original.clone()).collect::<Vec<_>>();
            let distinct = clones
                .iter()
                .map(|id| id.as_str().as_ptr() as usize)
                .chain(std::iter::once(original.as_str().as_ptr() as usize))
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            assert_eq!(
                distinct, 1,
                "{size} publication clones allocate {distinct} source payloads"
            );
            drop(original);
            assert!(clones
                .iter()
                .all(|id| id.as_str() == "long-lived-source-identity-payload"));
        }
    }
    #[test]
    fn identity_equality_hash_and_order_use_content_not_backing() {
        let a = SourceEntityId::new(String::from("same"));
        let b = SourceEntityId::new(String::from("same"));
        assert_ne!(a.as_str().as_ptr(), b.as_str().as_ptr());
        assert_eq!(a, b);
        assert_eq!(
            std::collections::HashSet::from([a.clone(), b.clone()]).len(),
            1
        );
        assert_eq!(std::collections::BTreeSet::from([a, b]).len(), 1);
    }
}
