use crate::components::ComponentTypeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableOrder {
    EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySpec {
    pub all: Vec<ComponentTypeId>,
    pub none: Vec<ComponentTypeId>,
    pub include_disabled: bool,
    pub limit: Option<usize>,
    pub stable_order: StableOrder,
}

impl QuerySpec {
    pub fn all(component_types: impl IntoIterator<Item = ComponentTypeId>) -> Self {
        Self {
            all: component_types.into_iter().collect(),
            none: Vec::new(),
            include_disabled: false,
            limit: None,
            stable_order: StableOrder::EntityId,
        }
    }

    pub fn excluding(mut self, component_types: impl IntoIterator<Item = ComponentTypeId>) -> Self {
        self.none = component_types.into_iter().collect();
        self
    }

    pub fn include_disabled(mut self) -> Self {
        self.include_disabled = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl Default for QuerySpec {
    fn default() -> Self {
        Self {
            all: Vec::new(),
            none: Vec::new(),
            include_disabled: false,
            limit: None,
            stable_order: StableOrder::EntityId,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_spec_defaults_to_entity_id_order() {
        let spec = QuerySpec::all([ComponentTypeId::transform()]);
        assert_eq!(spec.stable_order, StableOrder::EntityId);
        assert!(!spec.include_disabled);
    }
}
