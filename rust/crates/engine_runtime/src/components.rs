use crate::ids::EntityId;
use crate::math::Vec3;
use std::collections::BTreeSet;
use std::fmt;

pub use crate::animator2d::RuntimeAnimator2D as Animator2D;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentTypeId(String);

impl ComponentTypeId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn entity_meta() -> Self {
        Self::new("engine.entity_meta")
    }

    pub fn hierarchy() -> Self {
        Self::new("engine.hierarchy")
    }

    pub fn transform() -> Self {
        Self::new("engine.transform")
    }

    pub fn renderable() -> Self {
        Self::new("engine.renderable")
    }

    pub fn sprite_renderer2d() -> Self {
        Self::new("engine.sprite_renderer2d")
    }

    pub fn animator2d() -> Self {
        Self::new("engine.animator2d")
    }

    pub fn collider2d() -> Self {
        Self::new("engine.collider2d")
    }
}

impl fmt::Display for ComponentTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<&str> for ComponentTypeId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ComponentTypeId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ComponentRegistry {
    known_types: BTreeSet<ComponentTypeId>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register(ComponentTypeId::entity_meta());
        registry.register(ComponentTypeId::hierarchy());
        registry.register(ComponentTypeId::transform());
        registry.register(ComponentTypeId::renderable());
        registry.register(ComponentTypeId::sprite_renderer2d());
        registry.register(ComponentTypeId::animator2d());
        registry.register(ComponentTypeId::collider2d());
        registry
    }

    pub fn register(&mut self, component_type: ComponentTypeId) {
        self.known_types.insert(component_type);
    }

    pub fn contains(&self, component_type: &ComponentTypeId) -> bool {
        self.known_types.contains(component_type)
    }

    pub fn component_types(&self) -> Vec<&ComponentTypeId> {
        self.known_types.iter().collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    pub local_position: Vec3,
    pub local_rotation: Vec3,
    pub local_scale: Vec3,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            local_position: Vec3::ZERO,
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Renderable {
    pub mesh_ref: Option<String>,
    pub material_ref: Option<String>,
    pub visible: bool,
    pub layer: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpriteRenderer2D {
    pub sprite_ref: Option<String>,
    pub material_ref: Option<String>,
    pub color: [f32; 4],
    pub flip_x: bool,
    pub flip_y: bool,
    pub sorting_layer: i16,
    pub order_in_layer: i32,
    pub sort_z: f32,
    pub visible: bool,
}

impl Default for SpriteRenderer2D {
    fn default() -> Self {
        Self {
            sprite_ref: None,
            material_ref: None,
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
            visible: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hierarchy {
    pub parent_id: Option<EntityId>,
    pub sibling_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityMeta {
    pub id: EntityId,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub alive: bool,
    pub hierarchy: Hierarchy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_registry_registers_engine_component_types() {
        let registry = ComponentRegistry::new();
        assert!(registry.contains(&ComponentTypeId::entity_meta()));
        assert!(registry.contains(&ComponentTypeId::hierarchy()));
        assert!(registry.contains(&ComponentTypeId::transform()));
        assert!(registry.contains(&ComponentTypeId::renderable()));
        assert!(registry.contains(&ComponentTypeId::sprite_renderer2d()));
        assert!(registry.contains(&ComponentTypeId::animator2d()));
        assert!(registry.contains(&ComponentTypeId::collider2d()));
    }

    #[test]
    fn component_registry_orders_types_deterministically() {
        let mut registry = ComponentRegistry::default();
        registry.register(ComponentTypeId::from("z"));
        registry.register(ComponentTypeId::from("a"));
        let names = registry
            .component_types()
            .into_iter()
            .map(ComponentTypeId::as_str)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["a", "z"]);
    }
}
