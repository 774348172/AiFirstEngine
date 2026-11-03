use crate::component_value::RuntimeValue;
use crate::components::{
    Animator2D, AudioSource, ComponentTypeId, EntityMeta, Hierarchy, Renderable, SpriteRenderer2D,
    Transform,
};
use crate::ids::RuntimeEntityId;
use crate::physics2d::Collider2D;
use crate::runtime_particles::ParticleEffect;
use crate::spatial_contract::Light3D;
use crate::spatial_contract::{Camera3D, CanonicalTransform3D, MeshRenderer3D, Visibility3D};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeSignature {
    component_types: Vec<ComponentTypeId>,
}

impl ArchetypeSignature {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_vec(&self.component_types, "world", &["resident"]);
        for component in &self.component_types {
            component.observe_memory(sample);
        }
    }

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
    Transform3D(CanonicalTransform3D),
    Visibility3D(Visibility3D),
    MeshRenderer3D(MeshRenderer3D),
    Camera3D(Camera3D),
    Light3D(Light3D),
    Transform(Transform),
    Renderable(Renderable),
    SpriteRenderer2D(SpriteRenderer2D),
    Animator2D(Animator2D),
    AudioSource(AudioSource),
    ParticleEffect(ParticleEffect),
    Collider2D(Collider2D),
    Dynamic {
        component_type: ComponentTypeId,
        value: RuntimeValue,
    },
}

impl ComponentValue {
    pub fn component_type(&self) -> ComponentTypeId {
        ComponentTypeId::from(self.component_type_name())
    }

    pub(crate) fn component_type_name(&self) -> &str {
        match self {
            Self::EntityMeta(_) => "engine.entity_meta",
            Self::Hierarchy(_) => "engine.hierarchy",
            Self::Transform3D(_) => "engine.transform3d",
            Self::Visibility3D(_) => "engine.visibility3d",
            Self::MeshRenderer3D(_) => "engine.mesh_renderer3d",
            Self::Camera3D(_) => "engine.camera3d",
            Self::Light3D(_) => "engine.light3d",
            Self::Transform(_) => "engine.transform",
            Self::Renderable(_) => "engine.renderable",
            Self::SpriteRenderer2D(_) => "engine.sprite_renderer2d",
            Self::Animator2D(_) => "engine.animator2d",
            Self::AudioSource(_) => "engine.audio_source",
            Self::ParticleEffect(_) => "engine.particle_effect",
            Self::Collider2D(_) => "engine.collider2d",
            Self::Dynamic { component_type, .. } => component_type.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentColumn {
    EntityMeta(Vec<EntityMeta>),
    Hierarchy(Vec<Hierarchy>),
    Transform3D(Vec<CanonicalTransform3D>),
    Visibility3D(Vec<Visibility3D>),
    MeshRenderer3D(Vec<MeshRenderer3D>),
    Camera3D(Vec<Camera3D>),
    Light3D(Vec<Light3D>),
    Transform(Vec<Transform>),
    Renderable(Vec<Renderable>),
    SpriteRenderer2D(Vec<SpriteRenderer2D>),
    Animator2D(Vec<Animator2D>),
    AudioSource(Vec<AudioSource>),
    ParticleEffect(Vec<ParticleEffect>),
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
            ComponentColumn::Transform3D(_) => ComponentTypeId::transform3d(),
            ComponentColumn::Visibility3D(_) => ComponentTypeId::visibility3d(),
            ComponentColumn::MeshRenderer3D(_) => ComponentTypeId::mesh_renderer3d(),
            ComponentColumn::Camera3D(_) => ComponentTypeId::camera3d(),
            ComponentColumn::Light3D(_) => ComponentTypeId::light3d(),
            ComponentColumn::Transform(_) => ComponentTypeId::transform(),
            ComponentColumn::Renderable(_) => ComponentTypeId::renderable(),
            ComponentColumn::SpriteRenderer2D(_) => ComponentTypeId::sprite_renderer2d(),
            ComponentColumn::Animator2D(_) => ComponentTypeId::animator2d(),
            ComponentColumn::AudioSource(_) => ComponentTypeId::audio_source(),
            ComponentColumn::ParticleEffect(_) => ComponentTypeId::particle_effect(),
            ComponentColumn::Collider2D(_) => ComponentTypeId::collider2d(),
            ComponentColumn::Dynamic { component_type, .. } => component_type.clone(),
        }
    }

    pub fn new_for(component_type: &ComponentTypeId) -> Self {
        if component_type == &ComponentTypeId::entity_meta() {
            Self::EntityMeta(Vec::new())
        } else if component_type == &ComponentTypeId::hierarchy() {
            Self::Hierarchy(Vec::new())
        } else if component_type == &ComponentTypeId::transform3d() {
            Self::Transform3D(Vec::new())
        } else if component_type == &ComponentTypeId::visibility3d() {
            Self::Visibility3D(Vec::new())
        } else if component_type == &ComponentTypeId::mesh_renderer3d() {
            Self::MeshRenderer3D(Vec::new())
        } else if component_type == &ComponentTypeId::camera3d() {
            Self::Camera3D(Vec::new())
        } else if component_type == &ComponentTypeId::light3d() {
            Self::Light3D(Vec::new())
        } else if component_type == &ComponentTypeId::transform() {
            Self::Transform(Vec::new())
        } else if component_type == &ComponentTypeId::renderable() {
            Self::Renderable(Vec::new())
        } else if component_type == &ComponentTypeId::sprite_renderer2d() {
            Self::SpriteRenderer2D(Vec::new())
        } else if component_type == &ComponentTypeId::animator2d() {
            Self::Animator2D(Vec::new())
        } else if component_type == &ComponentTypeId::audio_source() {
            Self::AudioSource(Vec::new())
        } else if component_type == &ComponentTypeId::particle_effect() {
            Self::ParticleEffect(Vec::new())
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
            ComponentColumn::Transform3D(values) => values.len(),
            ComponentColumn::Visibility3D(values) => values.len(),
            ComponentColumn::MeshRenderer3D(values) => values.len(),
            ComponentColumn::Camera3D(values) => values.len(),
            ComponentColumn::Light3D(values) => values.len(),
            ComponentColumn::Transform(values) => values.len(),
            ComponentColumn::Renderable(values) => values.len(),
            ComponentColumn::SpriteRenderer2D(values) => values.len(),
            ComponentColumn::Animator2D(values) => values.len(),
            ComponentColumn::AudioSource(values) => values.len(),
            ComponentColumn::ParticleEffect(values) => values.len(),
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
            (ComponentColumn::Transform3D(values), ComponentValue::Transform3D(value)) => {
                values.push(value)
            }
            (ComponentColumn::Visibility3D(values), ComponentValue::Visibility3D(value)) => {
                values.push(value)
            }
            (ComponentColumn::MeshRenderer3D(values), ComponentValue::MeshRenderer3D(value)) => {
                values.push(value)
            }
            (ComponentColumn::Camera3D(values), ComponentValue::Camera3D(value)) => {
                values.push(value)
            }
            (ComponentColumn::Light3D(values), ComponentValue::Light3D(value)) => {
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
            (ComponentColumn::AudioSource(values), ComponentValue::AudioSource(value)) => {
                values.push(value)
            }
            (ComponentColumn::ParticleEffect(values), ComponentValue::ParticleEffect(value)) => {
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

    fn replace_value(&mut self, row: usize, value: ComponentValue) {
        match (self, value) {
            (Self::EntityMeta(values), ComponentValue::EntityMeta(value)) => values[row] = value,
            (Self::Hierarchy(values), ComponentValue::Hierarchy(value)) => values[row] = value,
            (Self::Transform3D(values), ComponentValue::Transform3D(value)) => values[row] = value,
            (Self::Visibility3D(values), ComponentValue::Visibility3D(value)) => {
                values[row] = value
            }
            (Self::MeshRenderer3D(values), ComponentValue::MeshRenderer3D(value)) => {
                values[row] = value
            }
            (Self::Camera3D(values), ComponentValue::Camera3D(value)) => values[row] = value,
            (Self::Light3D(values), ComponentValue::Light3D(value)) => values[row] = value,
            (Self::Transform(values), ComponentValue::Transform(value)) => values[row] = value,
            (Self::Renderable(values), ComponentValue::Renderable(value)) => values[row] = value,
            (Self::SpriteRenderer2D(values), ComponentValue::SpriteRenderer2D(value)) => {
                values[row] = value
            }
            (Self::Animator2D(values), ComponentValue::Animator2D(value)) => values[row] = value,
            (Self::AudioSource(values), ComponentValue::AudioSource(value)) => values[row] = value,
            (Self::ParticleEffect(values), ComponentValue::ParticleEffect(value)) => {
                values[row] = value
            }
            (Self::Collider2D(values), ComponentValue::Collider2D(value)) => values[row] = value,
            (Self::Dynamic { values, .. }, ComponentValue::Dynamic { value, .. }) => {
                values[row] = value
            }
            (column, value) => panic!(
                "component value {} does not match column {}",
                value.component_type(),
                column.component_type()
            ),
        }
    }

    fn value(&self, row: usize) -> Option<ComponentValue> {
        match self {
            Self::EntityMeta(values) => values.get(row).cloned().map(ComponentValue::EntityMeta),
            Self::Hierarchy(values) => values.get(row).cloned().map(ComponentValue::Hierarchy),
            Self::Transform3D(values) => values.get(row).cloned().map(ComponentValue::Transform3D),
            Self::Visibility3D(values) => {
                values.get(row).cloned().map(ComponentValue::Visibility3D)
            }
            Self::MeshRenderer3D(values) => {
                values.get(row).cloned().map(ComponentValue::MeshRenderer3D)
            }
            Self::Camera3D(values) => values.get(row).cloned().map(ComponentValue::Camera3D),
            Self::Light3D(values) => values.get(row).cloned().map(ComponentValue::Light3D),
            Self::Transform(values) => values.get(row).cloned().map(ComponentValue::Transform),
            Self::Renderable(values) => values.get(row).cloned().map(ComponentValue::Renderable),
            Self::SpriteRenderer2D(values) => values
                .get(row)
                .cloned()
                .map(ComponentValue::SpriteRenderer2D),
            Self::Animator2D(values) => values.get(row).cloned().map(ComponentValue::Animator2D),
            Self::AudioSource(values) => values.get(row).cloned().map(ComponentValue::AudioSource),
            Self::ParticleEffect(values) => {
                values.get(row).cloned().map(ComponentValue::ParticleEffect)
            }
            Self::Collider2D(values) => values.get(row).cloned().map(ComponentValue::Collider2D),
            Self::Dynamic {
                component_type,
                values,
            } => values
                .get(row)
                .cloned()
                .map(|value| ComponentValue::Dynamic {
                    component_type: component_type.clone(),
                    value,
                }),
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
            ComponentColumn::Transform3D(values) => {
                ComponentValue::Transform3D(values.swap_remove(row))
            }
            ComponentColumn::Visibility3D(values) => {
                ComponentValue::Visibility3D(values.swap_remove(row))
            }
            ComponentColumn::MeshRenderer3D(values) => {
                ComponentValue::MeshRenderer3D(values.swap_remove(row))
            }
            ComponentColumn::Camera3D(values) => ComponentValue::Camera3D(values.swap_remove(row)),
            ComponentColumn::Light3D(values) => ComponentValue::Light3D(values.swap_remove(row)),
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
            ComponentColumn::AudioSource(values) => {
                ComponentValue::AudioSource(values.swap_remove(row))
            }
            ComponentColumn::ParticleEffect(values) => {
                ComponentValue::ParticleEffect(values.swap_remove(row))
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

#[cfg(test)]
std::thread_local! { static TYPED_COLUMN_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[derive(Debug, Clone)]
pub struct ArchetypeTable {
    pub id: ArchetypeId,
    pub signature: ArchetypeSignature,
    entities: Vec<RuntimeEntityId>,
    columns: Vec<ComponentColumn>,
    typed_columns: [Option<usize>; 14],
}

impl ArchetypeTable {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        self.signature.observe_memory(sample);
        sample.cpu_vec(&self.entities, "world", &["resident"]);
        sample.cpu_vec(&self.columns, "world", &["resident"]);
        for column in &self.columns {
            match column {
                ComponentColumn::EntityMeta(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Hierarchy(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Transform3D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Visibility3D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::MeshRenderer3D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Camera3D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Light3D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Transform(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Renderable(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::SpriteRenderer2D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Animator2D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::AudioSource(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::ParticleEffect(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Collider2D(v) => sample.cpu_vec(v, "world", &["resident"]),
                ComponentColumn::Dynamic { values, .. } => {
                    sample.cpu_vec(values, "world", &["resident"]);
                    for v in values {
                        observe_value_memory(v, sample);
                    }
                }
            }
            if let ComponentColumn::EntityMeta(values) = column {
                for v in values {
                    sample.cpu_string(&v.name, "world", &["resident"]);
                    sample.cpu_string(&v.kind, "world", &["resident"]);
                    sample.cpu_arc(v.id.storage(), "world", &["resident"]);
                    if let Some(parent) = &v.hierarchy.parent_id {
                        sample.cpu_arc(parent.storage(), "world", &["resident"]);
                    }
                }
            }
            if let ComponentColumn::Hierarchy(values) = column {
                for v in values {
                    if let Some(parent) = &v.parent_id {
                        sample.cpu_arc(parent.storage(), "world", &["resident"]);
                    }
                }
            }
        }
        // The columns own these strings/vectors directly; their outer Vec
        // capacity includes only the headers, never the nested allocation.
        for column in &self.columns {
            match column {
                ComponentColumn::Visibility3D(values) => {
                    for v in values {
                        sample.cpu_string(&v.schema_version, "world", &["resident"]);
                    }
                }
                ComponentColumn::MeshRenderer3D(values) => {
                    for v in values {
                        sample.cpu_string(&v.schema_version, "world", &["resident"]);
                        observe_spatial_ref(&v.mesh, sample);
                        observe_world_strings(&v.layers, sample);
                        for (slot, binding) in &v.materials {
                            sample.cpu_string(slot, "world", &["resident"]);
                            observe_spatial_ref(binding, sample);
                        }
                    }
                }
                ComponentColumn::Camera3D(values) => {
                    for v in values {
                        sample.cpu_string(&v.schema_version, "world", &["resident"]);
                        observe_world_strings(&v.layers, sample);
                        if let Some(color) = &v.color_pipeline {
                            color.observe_memory(sample, "world", "resident");
                        }
                    }
                }
                ComponentColumn::Light3D(values) => {
                    for v in values {
                        sample.cpu_string(&v.schema_version, "world", &["resident"]);
                        observe_world_strings(&v.layers, sample);
                    }
                }
                ComponentColumn::Renderable(values) => {
                    for v in values {
                        for text in v
                            .mesh_ref
                            .iter()
                            .chain(v.material_ref.iter())
                            .chain([&v.layer])
                        {
                            sample.cpu_string(text, "world", &["resident"]);
                        }
                    }
                }
                ComponentColumn::SpriteRenderer2D(values) => {
                    for v in values {
                        for text in v.sprite_ref.iter().chain(v.material_ref.iter()) {
                            sample.cpu_string(text, "world", &["resident"]);
                        }
                    }
                }
                ComponentColumn::Animator2D(values) => {
                    for v in values {
                        for text in [&v.controller_id, &v.registry_digest]
                            .into_iter()
                            .chain(v.initial_bools.keys())
                        {
                            sample.cpu_string(text, "world", &["resident"]);
                        }
                    }
                }
                ComponentColumn::AudioSource(values) => {
                    for v in values {
                        observe_world_asset_ref(&v.clip_ref, sample);
                    }
                }
                ComponentColumn::ParticleEffect(values) => {
                    for v in values {
                        observe_world_asset_ref(&v.effect_ref, sample);
                        for name in v.parameters.keys() {
                            sample.cpu_string(name, "world", &["resident"]);
                        }
                    }
                }
                ComponentColumn::Dynamic { component_type, .. } => {
                    component_type.observe_memory(sample)
                }
                _ => (),
            }
        }
        sample.partial("cpu.owner.world", "world.archetypeEntities+columns+signature+dynamicPayload+typedNestedStringsVectors.capacity;stdTreeNodesOpaque;allocatorArcControlBlocksOpaque");
    }

    pub fn new(id: ArchetypeId, signature: ArchetypeSignature) -> Self {
        let columns: Vec<ComponentColumn> = signature
            .component_types()
            .iter()
            .map(ComponentColumn::new_for)
            .collect();
        let mut typed_columns = [None; 14];
        for (index, column) in columns.iter().enumerate() {
            let kind = match column {
                ComponentColumn::EntityMeta(_) => 0,
                ComponentColumn::Hierarchy(_) => 1,
                ComponentColumn::Transform3D(_) => 2,
                ComponentColumn::Visibility3D(_) => 3,
                ComponentColumn::MeshRenderer3D(_) => 4,
                ComponentColumn::Camera3D(_) => 5,
                ComponentColumn::Light3D(_) => 6,
                ComponentColumn::Transform(_) => 7,
                ComponentColumn::Renderable(_) => 8,
                ComponentColumn::SpriteRenderer2D(_) => 9,
                ComponentColumn::Animator2D(_) => 10,
                ComponentColumn::AudioSource(_) => 11,
                ComponentColumn::ParticleEffect(_) => 12,
                ComponentColumn::Collider2D(_) => 13,
                ComponentColumn::Dynamic { .. } => continue,
            };
            typed_columns[kind] = Some(index);
        }
        Self {
            id,
            signature,
            entities: Vec::new(),
            columns,
            typed_columns,
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

    fn typed_column(&self, kind: usize) -> Option<&ComponentColumn> {
        let index = self.typed_columns[kind]?;
        #[cfg(test)]
        TYPED_COLUMN_VISITS.with(|n| n.set(n.get() + 1));
        Some(&self.columns[index])
    }

    pub(crate) fn replace_transform3d(&mut self, row: usize, value: CanonicalTransform3D) -> bool {
        let Some(index) = self.typed_columns[2] else {
            return false;
        };
        let ComponentColumn::Transform3D(values) = &mut self.columns[index] else {
            unreachable!("private typed column layout");
        };
        let Some(slot) = values.get_mut(row) else {
            return false;
        };
        *slot = value;
        true
    }

    pub fn entity_meta(&self, row: usize) -> Option<&EntityMeta> {
        match self.typed_column(0)? {
            ComponentColumn::EntityMeta(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn transform3d(&self, row: usize) -> Option<&CanonicalTransform3D> {
        match self.typed_column(2)? {
            ComponentColumn::Transform3D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn visibility3d(&self, row: usize) -> Option<&Visibility3D> {
        match self.typed_column(3)? {
            ComponentColumn::Visibility3D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn mesh_renderer3d(&self, row: usize) -> Option<&MeshRenderer3D> {
        match self.typed_column(4)? {
            ComponentColumn::MeshRenderer3D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn camera3d(&self, row: usize) -> Option<&Camera3D> {
        match self.typed_column(5)? {
            ComponentColumn::Camera3D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }
    pub fn light3d(&self, row: usize) -> Option<&Light3D> {
        match self.typed_column(6)? {
            ComponentColumn::Light3D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn transform(&self, row: usize) -> Option<&Transform> {
        match self.typed_column(7)? {
            ComponentColumn::Transform(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn renderable(&self, row: usize) -> Option<&Renderable> {
        match self.typed_column(8)? {
            ComponentColumn::Renderable(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn sprite_renderer2d(&self, row: usize) -> Option<&SpriteRenderer2D> {
        match self.typed_column(9)? {
            ComponentColumn::SpriteRenderer2D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn animator2d(&self, row: usize) -> Option<&Animator2D> {
        match self.typed_column(10)? {
            ComponentColumn::Animator2D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn audio_source(&self, row: usize) -> Option<&AudioSource> {
        match self.typed_column(11)? {
            ComponentColumn::AudioSource(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }
    pub fn particle_effect(&self, row: usize) -> Option<&ParticleEffect> {
        match self.typed_column(12)? {
            ComponentColumn::ParticleEffect(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub fn collider2d(&self, row: usize) -> Option<&Collider2D> {
        match self.typed_column(13)? {
            ComponentColumn::Collider2D(values) => values.get(row),
            _ => unreachable!("private typed column layout"),
        }
    }

    pub(crate) fn has_component_named(&self, name: &str) -> bool {
        self.signature
            .component_types
            .binary_search_by(|kind| kind.as_str().cmp(name))
            .is_ok()
    }

    // A value write cannot change the signature or any entity's row location.
    // Return ownership only when a component must actually be added.
    pub(crate) fn replace_component_value(
        &mut self,
        row: usize,
        value: ComponentValue,
    ) -> Option<ComponentValue> {
        let Ok(index) = self
            .signature
            .component_types
            .binary_search_by(|kind| kind.as_str().cmp(value.component_type_name()))
        else {
            return Some(value);
        };
        self.columns[index].replace_value(row, value);
        None
    }

    pub fn component_value(
        &self,
        row: usize,
        component_type: &ComponentTypeId,
    ) -> Option<ComponentValue> {
        let index = self
            .signature
            .component_types
            .binary_search(component_type)
            .ok()?;
        self.columns[index].value(row)
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
    fn world_3d_census_names_live_nested_capacity_and_keeps_map_nodes_unknown() {
        let reference = || crate::runtime_package::RuntimeAssetRef {
            id: "mesh".into(),
            asset_type: "mesh".into(),
            guid: Some("guid".into()),
            sub_asset: Some("part".into()),
        };
        let mut layer = String::with_capacity(128);
        layer.push_str("world");
        let mut camera = Camera3D::perspective_template();
        camera.layers = vec![layer.clone()];
        camera.color_pipeline = Some(Default::default());
        let mesh = MeshRenderer3D {
            schema_version: "mesh-renderer3d.v1".into(),
            mesh: crate::spatial_contract::SpatialAssetRef(reference()),
            materials: [(
                "surface".into(),
                crate::spatial_contract::SpatialAssetRef(reference()),
            )]
            .into(),
            visible: true,
            layers: vec![layer],
        };
        let signature = ArchetypeSignature::new([
            ComponentTypeId::camera3d(),
            ComponentTypeId::mesh_renderer3d(),
            ComponentTypeId::visibility3d(),
        ]);
        let mut table = ArchetypeTable::new(ArchetypeId(1), signature);
        table.push_row(
            RuntimeEntityId::new(1, 0),
            vec![
                ComponentValue::Camera3D(camera),
                ComponentValue::MeshRenderer3D(mesh),
                ComponentValue::Visibility3D(Visibility3D {
                    schema_version: "visibility3d.v1".into(),
                    visible: true,
                }),
            ],
        );
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        table.observe_memory(&mut sample);
        let expected = |sample: &crate::resource_memory::ResourceMemorySample, value: &String| {
            let id = format!("cpu-storage:{:x}", value.as_ptr() as usize);
            assert!(
                sample.allocations.iter().any(|a| a.allocation_id == id
                    && a.bytes == value.capacity() as u64
                    && a.owner == "world"
                    && a.roles.contains("resident")),
                "unobserved real String capacity {id}"
            );
        };
        for column in &table.columns {
            if let ComponentColumn::Camera3D(v) = column {
                expected(&sample, &v[0].layers[0]);
                expected(&sample, &v[0].schema_version);
                expected(
                    &sample,
                    &v[0].color_pipeline.as_ref().unwrap().schema_version,
                );
            }
            if let ComponentColumn::MeshRenderer3D(v) = column {
                let m = &v[0];
                expected(&sample, &m.layers[0]);
                expected(&sample, &m.mesh.0.id);
                expected(&sample, m.mesh.0.sub_asset.as_ref().unwrap());
                for (slot, binding) in &m.materials {
                    expected(&sample, slot);
                    expected(&sample, &binding.0.id);
                }
            }
            if let ComponentColumn::Visibility3D(v) = column {
                expected(&sample, &v[0].schema_version);
            }
        }
        assert_eq!(
            sample.coverage["cpu.owner.world"],
            crate::resource_memory::MemoryCoverage::Partial
        );
        let before = crate::resource_memory::memory_totals(&sample.allocations).unwrap();
        table.observe_memory(&mut sample);
        assert_eq!(
            crate::resource_memory::memory_totals(&sample.allocations).unwrap(),
            before,
            "borrowed aliases must deduplicate"
        );
        drop(table);
        let empty = ArchetypeTable::new(ArchetypeId(2), ArchetypeSignature::new([]));
        let mut after = crate::resource_memory::ResourceMemorySample::new(2, 1, 0);
        empty.observe_memory(&mut after);
        assert!(
            after.allocations.is_empty(),
            "census must not retain dead nested allocations"
        );
    }

    #[test]
    fn typed_columns_visit_only_the_requested_column_at_multiple_scales() {
        for count in [1u32, 100, 1000, 10000] {
            let mut kinds = vec![
                ComponentTypeId::entity_meta(),
                ComponentTypeId::transform3d(),
            ];
            kinds.extend((0..64).map(|i| ComponentTypeId::from(format!("a.extra-{i:02}"))));
            let signature = ArchetypeSignature::new(kinds);
            let mut table = ArchetypeTable::new(ArchetypeId(5), signature.clone());
            for i in 0..count {
                let values = signature
                    .component_types()
                    .iter()
                    .map(|kind| match kind.as_str() {
                        "engine.entity_meta" => ComponentValue::EntityMeta(meta("node")),
                        "engine.transform3d" => ComponentValue::Transform3D(CanonicalTransform3D {
                            translation: [i as f32, 0., 0.],
                            rotation_xyzw: [0., 0., 0., 1.],
                            scale: [1.; 3],
                        }),
                        _ => ComponentValue::Dynamic {
                            component_type: kind.clone(),
                            value: RuntimeValue::I64(1),
                        },
                    })
                    .collect();
                table.push_row(RuntimeEntityId::new(i, 0), values);
            }
            TYPED_COLUMN_VISITS.with(|n| n.set(0));
            for i in 0..count as usize {
                assert!(table.entity_meta(i).is_some());
                assert_eq!(table.transform3d(i).unwrap().translation[0], i as f32);
                assert!(table.camera3d(i).is_none());
            }
            let visits = TYPED_COLUMN_VISITS.with(|n| n.get());
            eprintln!(
                "actual typed column visits: count={count}, columns={}, visits={visits}",
                table.columns.len()
            );
            assert_eq!(
                visits,
                2 * count as usize,
                "typed lookup rescans unrelated columns"
            );
            let clone = table.clone();
            if count > 1 {
                table.swap_remove_row(0);
                assert_eq!(
                    table.transform3d(0).unwrap().translation[0],
                    (count - 1) as f32
                );
            }
            assert_eq!(clone.transform3d(0).unwrap().translation[0], 0.);
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

fn observe_value_memory(
    v: &RuntimeValue,
    sample: &mut crate::resource_memory::ResourceMemorySample,
) {
    match v {
        RuntimeValue::String(s) | RuntimeValue::AssetRef(s) => {
            sample.cpu_string(s, "world", &["resident"])
        }
        RuntimeValue::Array(values) => {
            sample.cpu_vec(values, "world", &["resident"]);
            for v in values {
                observe_value_memory(v, sample);
            }
        }
        RuntimeValue::Object(fields) => {
            for (k, v) in fields {
                sample.cpu_string(k, "world", &["resident"]);
                observe_value_memory(v, sample);
            }
        }
        _ => {}
    }
}

fn observe_world_strings(
    values: &Vec<String>,
    sample: &mut crate::resource_memory::ResourceMemorySample,
) {
    sample.cpu_vec(values, "world", &["resident"]);
    for value in values {
        sample.cpu_string(value, "world", &["resident"]);
    }
}
fn observe_spatial_ref(
    value: &crate::spatial_contract::SpatialAssetRef,
    sample: &mut crate::resource_memory::ResourceMemorySample,
) {
    observe_world_asset_ref(&value.0, sample);
}
fn observe_world_asset_ref(
    value: &crate::runtime_package::RuntimeAssetRef,
    sample: &mut crate::resource_memory::ResourceMemorySample,
) {
    for value in [&value.id, &value.asset_type].into_iter().chain(
        [value.guid.as_ref(), value.sub_asset.as_ref()]
            .into_iter()
            .flatten(),
    ) {
        sample.cpu_string(value, "world", &["resident"]);
    }
}
