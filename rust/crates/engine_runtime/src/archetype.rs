use crate::component_value::RuntimeValue;
use crate::components::{
    Animator2D, ComponentTypeId, EntityMeta, Hierarchy, Renderable, SpriteRenderer2D, Transform,
};
use crate::ids::RuntimeEntityId;
use crate::physics2d::Collider2D;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeSignature {
    component_types: Vec<ComponentTypeId>,
}

impl ArchetypeSignature {
    pub fn new(component_types: impl IntoIterator<Item = ComponentTypeId>) -> Self {
        let unique = component_types.into_iter().collect::<BTreeSet<_>>();
        Self {
            component_types: unique.into_iter().collect(),
        }
    }

    pub fn component_types(&self) -> &[ComponentTypeId] {
        &self.component_types
    }

    pub fn contains(&self, component_type: &ComponentTypeId) -> bool {
        self.component_types.binary_search(component_type).is_ok()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    EntityMeta(EntityMeta),
    Hierarchy(Hierarchy),
    Transform(Transform),
    Renderable(Renderable),
    SpriteRenderer2D(SpriteRenderer2D),
    Animator2D(Animator2D),
    Collider2D(Collider2D),
    Dynamic {
        component_type: ComponentTypeId,
        value: RuntimeValue,
    },
}

impl ComponentValue {
    pub fn component_type(&self) -> ComponentTypeId {
        match self {
            ComponentValue::EntityMeta(_) => ComponentTypeId::entity_meta(),
            ComponentValue::Hierarchy(_) => ComponentTypeId::hierarchy(),
            ComponentValue::Transform(_) => ComponentTypeId::transform(),
            ComponentValue::Renderable(_) => ComponentTypeId::renderable(),
            ComponentValue::SpriteRenderer2D(_) => ComponentTypeId::sprite_renderer2d(),
            ComponentValue::Animator2D(_) => ComponentTypeId::animator2d(),
            ComponentValue::Collider2D(_) => ComponentTypeId::collider2d(),
            ComponentValue::Dynamic { component_type, .. } => component_type.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentColumn {
    EntityMeta(Vec<EntityMeta>),
    Hierarchy(Vec<Hierarchy>),
    Transform(Vec<Transform>),
    Renderable(Vec<Renderable>),
    SpriteRenderer2D(Vec<SpriteRenderer2D>),
    Animator2D(Vec<Animator2D>),
    Collider2D(Vec<Collider2D>),
    Dynamic {
        component_type: ComponentTypeId,
        values: Vec<RuntimeValue>,
    },
}

impl ComponentColumn {
    pub fn component_type(&self) -> ComponentTypeId {
        match self {
            ComponentColumn::EntityMeta(_) => ComponentTypeId::entity_meta(),
            ComponentColumn::Hierarchy(_) => ComponentTypeId::hierarchy(),
            ComponentColumn::Transform(_) => ComponentTypeId::transform(),
            ComponentColumn::Renderable(_) => ComponentTypeId::renderable(),
            ComponentColumn::SpriteRenderer2D(_) => ComponentTypeId::sprite_renderer2d(),
            ComponentColumn::Animator2D(_) => ComponentTypeId::animator2d(),
            ComponentColumn::Collider2D(_) => ComponentTypeId::collider2d(),
            ComponentColumn::Dynamic { component_type, .. } => component_type.clone(),
        }
    }

    pub fn new_for(component_type: &ComponentTypeId) -> Self {
        if component_type == &ComponentTypeId::entity_meta() {
            Self::EntityMeta(Vec::new())
        } else if component_type == &ComponentTypeId::hierarchy() {
            Self::Hierarchy(Vec::new())
        } else if component_type == &ComponentTypeId::transform() {
            Self::Transform(Vec::new())
        } else if component_type == &ComponentTypeId::renderable() {
            Self::Renderable(Vec::new())
        } else if component_type == &ComponentTypeId::sprite_renderer2d() {
            Self::SpriteRenderer2D(Vec::new())
        } else if component_type == &ComponentTypeId::animator2d() {
            Self::Animator2D(Vec::new())
        } else if component_type == &ComponentTypeId::collider2d() {
            Self::Collider2D(Vec::new())
        } else {
            Self::Dynamic {
                component_type: component_type.clone(),
                values: Vec::new(),
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ComponentColumn::EntityMeta(values) => values.len(),
            ComponentColumn::Hierarchy(values) => values.len(),
            ComponentColumn::Transform(values) => values.len(),
            ComponentColumn::Renderable(values) => values.len(),
            ComponentColumn::SpriteRenderer2D(values) => values.len(),
            ComponentColumn::Animator2D(values) => values.len(),
            ComponentColumn::Collider2D(values) => values.len(),
            ComponentColumn::Dynamic { values, .. } => values.len(),
        }
    }

    fn push_value(&mut self, value: ComponentValue) {
        match (self, value) {
            (ComponentColumn::EntityMeta(values), ComponentValue::EntityMeta(value)) => {
                values.push(value)
            }
            (ComponentColumn::Hierarchy(values), ComponentValue::Hierarchy(value)) => {
                values.push(value)
            }
            (ComponentColumn::Transform(values), ComponentValue::Transform(value)) => {
                values.push(value)
            }
            (ComponentColumn::Renderable(values), ComponentValue::Renderable(value)) => {
                values.push(value)
            }
            (
                ComponentColumn::SpriteRenderer2D(values),
                ComponentValue::SpriteRenderer2D(value),
            ) => values.push(value),
            (ComponentColumn::Animator2D(values), ComponentValue::Animator2D(value)) => {
                values.push(value)
            }
            (ComponentColumn::Collider2D(values), ComponentValue::Collider2D(value)) => {
                values.push(value)
            }
            (ComponentColumn::Dynamic { values, .. }, ComponentValue::Dynamic { value, .. }) => {
                values.push(value)
            }
            (column, value) => panic!(
                "component value {} does not match column {}",
                value.component_type(),
                column.component_type()
            ),
        }
    }

    fn swap_remove_value(&mut self, row: usize) -> ComponentValue {
        match self {
            ComponentColumn::EntityMeta(values) => {
                ComponentValue::EntityMeta(values.swap_remove(row))
            }
            ComponentColumn::Hierarchy(values) => {
                ComponentValue::Hierarchy(values.swap_remove(row))
            }
            ComponentColumn::Transform(values) => {
                ComponentValue::Transform(values.swap_remove(row))
            }
            ComponentColumn::Renderable(values) => {
                ComponentValue::Renderable(values.swap_remove(row))
            }
            ComponentColumn::SpriteRenderer2D(values) => {
                ComponentValue::SpriteRenderer2D(values.swap_remove(row))
            }
            ComponentColumn::Animator2D(values) => {
                ComponentValue::Animator2D(values.swap_remove(row))
            }
            ComponentColumn::Collider2D(values) => {
                ComponentValue::Collider2D(values.swap_remove(row))
            }
            ComponentColumn::Dynamic {
                component_type,
                values,
            } => ComponentValue::Dynamic {
                component_type: component_type.clone(),
                value: values.swap_remove(row),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArchetypeTable {
    pub id: ArchetypeId,
    pub signature: ArchetypeSignature,
    entities: Vec<RuntimeEntityId>,
    columns: Vec<ComponentColumn>,
}

impl ArchetypeTable {
    pub fn new(id: ArchetypeId, signature: ArchetypeSignature) -> Self {
        let columns = signature
            .component_types()
            .iter()
            .map(ComponentColumn::new_for)
            .collect();
        Self {
            id,
            signature,
            entities: Vec::new(),
            columns,
        }
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn entities(&self) -> &[RuntimeEntityId] {
        &self.entities
    }

    pub fn columns(&self) -> &[ComponentColumn] {
        &self.columns
    }

    pub fn has_component(&self, component_type: &ComponentTypeId) -> bool {
        self.signature.contains(component_type)
    }

    pub fn push_row(&mut self, entity: RuntimeEntityId, values: Vec<ComponentValue>) -> usize {
        assert_eq!(
            values.len(),
            self.columns.len(),
            "row values must match archetype columns"
        );
        for (column, value) in self.columns.iter_mut().zip(values.into_iter()) {
            column.push_value(value);
        }
        self.entities.push(entity);
        self.entities.len() - 1
    }

    pub fn swap_remove_row(&mut self, row: usize) -> RemovedArchetypeRow {
        let entity = self.entities.swap_remove(row);
        let moved_entity = if row < self.entities.len() {
            Some(self.entities[row])
        } else {
            None
        };
        let values = self
            .columns
            .iter_mut()
            .map(|column| column.swap_remove_value(row))
            .collect();
        RemovedArchetypeRow {
            entity,
            values,
            moved_entity,
        }
    }

    pub fn entity_meta(&self, row: usize) -> Option<&EntityMeta> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::EntityMeta(values) => values.get(row),
            _ => None,
        })
    }

    pub fn transform(&self, row: usize) -> Option<&Transform> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::Transform(values) => values.get(row),
            _ => None,
        })
    }

    pub fn renderable(&self, row: usize) -> Option<&Renderable> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::Renderable(values) => values.get(row),
            _ => None,
        })
    }

    pub fn sprite_renderer2d(&self, row: usize) -> Option<&SpriteRenderer2D> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::SpriteRenderer2D(values) => values.get(row),
            _ => None,
        })
    }

    pub fn animator2d(&self, row: usize) -> Option<&Animator2D> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::Animator2D(values) => values.get(row),
            _ => None,
        })
    }

    pub fn collider2d(&self, row: usize) -> Option<&Collider2D> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::Collider2D(values) => values.get(row),
            _ => None,
        })
    }

    pub fn component_value(
        &self,
        row: usize,
        component_type: &ComponentTypeId,
    ) -> Option<ComponentValue> {
        self.columns.iter().find_map(|column| match column {
            ComponentColumn::EntityMeta(values)
                if component_type == &ComponentTypeId::entity_meta() =>
            {
                values.get(row).cloned().map(ComponentValue::EntityMeta)
            }
            ComponentColumn::Hierarchy(values)
                if component_type == &ComponentTypeId::hierarchy() =>
            {
                values.get(row).cloned().map(ComponentValue::Hierarchy)
            }
            ComponentColumn::Transform(values)
                if component_type == &ComponentTypeId::transform() =>
            {
                values.get(row).cloned().map(ComponentValue::Transform)
            }
            ComponentColumn::Renderable(values)
                if component_type == &ComponentTypeId::renderable() =>
            {
                values.get(row).cloned().map(ComponentValue::Renderable)
            }
            ComponentColumn::SpriteRenderer2D(values)
                if component_type == &ComponentTypeId::sprite_renderer2d() =>
            {
                values
                    .get(row)
                    .cloned()
                    .map(ComponentValue::SpriteRenderer2D)
            }
            ComponentColumn::Animator2D(values)
                if component_type == &ComponentTypeId::animator2d() =>
            {
                values.get(row).cloned().map(ComponentValue::Animator2D)
            }
            ComponentColumn::Collider2D(values)
                if component_type == &ComponentTypeId::collider2d() =>
            {
                values.get(row).cloned().map(ComponentValue::Collider2D)
            }
            ComponentColumn::Dynamic {
                component_type: column_type,
                values,
            } if component_type == column_type => {
                values
                    .get(row)
                    .cloned()
                    .map(|value| ComponentValue::Dynamic {
                        component_type: column_type.clone(),
                        value,
                    })
            }
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemovedArchetypeRow {
    pub entity: RuntimeEntityId,
    pub values: Vec<ComponentValue>,
    pub moved_entity: Option<RuntimeEntityId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Hierarchy;
    use crate::ids::EntityId;
    use crate::math::Vec3;

    fn meta(id: &str) -> EntityMeta {
        EntityMeta {
            id: EntityId::from(id),
            name: id.to_string(),
            kind: "test".to_string(),
            enabled: true,
            alive: true,
            hierarchy: Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        }
    }

    #[test]
    fn archetype_signature_sorts_and_deduplicates_component_types() {
        let signature = ArchetypeSignature::new([
            ComponentTypeId::renderable(),
            ComponentTypeId::transform(),
            ComponentTypeId::transform(),
        ]);
        let names = signature
            .component_types()
            .iter()
            .map(ComponentTypeId::as_str)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["engine.renderable", "engine.transform"]);
    }

    #[test]
    fn archetype_table_pushes_and_reads_rows() {
        let signature =
            ArchetypeSignature::new([ComponentTypeId::entity_meta(), ComponentTypeId::transform()]);
        let mut table = ArchetypeTable::new(ArchetypeId(1), signature);
        let row = table.push_row(
            RuntimeEntityId::new(0, 0),
            vec![
                ComponentValue::EntityMeta(meta("entity-a")),
                ComponentValue::Transform(Transform {
                    local_position: Vec3 {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    },
                    local_rotation: Vec3::ZERO,
                    local_scale: Vec3::ONE,
                }),
            ],
        );
        assert_eq!(row, 0);
        assert_eq!(table.entity_meta(0).unwrap().id.as_str(), "entity-a");
        assert_eq!(table.transform(0).unwrap().local_position.y, 2.0);
    }

    #[test]
    fn archetype_table_swap_remove_reports_moved_entity() {
        let signature = ArchetypeSignature::new([ComponentTypeId::entity_meta()]);
        let mut table = ArchetypeTable::new(ArchetypeId(1), signature);
        table.push_row(
            RuntimeEntityId::new(1, 0),
            vec![ComponentValue::EntityMeta(meta("a"))],
        );
        table.push_row(
            RuntimeEntityId::new(2, 0),
            vec![ComponentValue::EntityMeta(meta("b"))],
        );
        let removed = table.swap_remove_row(0);
        assert_eq!(removed.entity, RuntimeEntityId::new(1, 0));
        assert_eq!(removed.moved_entity, Some(RuntimeEntityId::new(2, 0)));
        assert_eq!(table.entity_meta(0).unwrap().id.as_str(), "b");
    }

    #[test]
    fn dynamic_component_stores_runtime_value() {
        let dynamic_type = ComponentTypeId::from("project.marker");
        let signature =
            ArchetypeSignature::new([ComponentTypeId::entity_meta(), dynamic_type.clone()]);
        let mut table = ArchetypeTable::new(ArchetypeId(1), signature);
        table.push_row(
            RuntimeEntityId::new(1, 0),
            vec![
                ComponentValue::EntityMeta(meta("a")),
                ComponentValue::Dynamic {
                    component_type: dynamic_type,
                    value: RuntimeValue::object([("count", RuntimeValue::I64(1))]),
                },
            ],
        );

        let removed = table.swap_remove_row(0);

        assert_eq!(
            removed.values[1],
            ComponentValue::Dynamic {
                component_type: ComponentTypeId::from("project.marker"),
                value: RuntimeValue::object([("count", RuntimeValue::I64(1))]),
            }
        );
    }
}
