use crate::archetype::{ArchetypeId, ArchetypeSignature, ArchetypeTable, ComponentValue};
use crate::component_value::RuntimeValue;
use crate::components::{
    ComponentTypeId, EntityMeta, Hierarchy, Renderable, SpriteRenderer2D, Transform,
};
use crate::ids::{EntityId, RuntimeEntityId, SourceEntityId};
use crate::query::{QuerySpec, StableOrder};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldMutationError {
    pub code: &'static str,
    pub operation: &'static str,
    pub source_entity_id: Option<EntityId>,
    pub runtime_entity_id: Option<RuntimeEntityId>,
    pub parent_entity_id: Option<EntityId>,
    pub component_type: Option<ComponentTypeId>,
    pub message: String,
    pub suggested_fix: Option<String>,
}

impl WorldMutationError {
    fn new(
        code: &'static str,
        operation: &'static str,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        Self {
            code,
            operation,
            source_entity_id: None,
            runtime_entity_id: None,
            parent_entity_id: None,
            component_type: None,
            message: message.into(),
            suggested_fix: Some(suggested_fix.into()),
        }
    }

    pub(crate) fn missing_entity(operation: &'static str, entity_id: EntityId) -> Self {
        Self::new(
            "world.entity.missing",
            operation,
            format!("entity does not exist: {entity_id}"),
            "Use an alive entity id returned by the current World.",
        )
        .with_source_entity_id(entity_id)
    }

    pub(crate) fn missing_component(
        operation: &'static str,
        entity_id: EntityId,
        component_type: ComponentTypeId,
    ) -> Self {
        Self::new(
            "world.component.missing",
            operation,
            format!("component '{component_type}' is missing on entity '{entity_id}'"),
            "Add the component before reading, writing, or removing it.",
        )
        .with_source_entity_id(entity_id)
        .with_component_type(component_type)
    }

    pub(crate) fn unsupported_field(entity_id: EntityId, component_type: ComponentTypeId) -> Self {
        Self::new(
            "world.component.unsupported_field",
            "write_component_field",
            format!("field path is not supported for component '{component_type}'"),
            "Use a field path declared by the component schema.",
        )
        .with_source_entity_id(entity_id)
        .with_component_type(component_type)
    }

    pub(crate) fn invalid_component_value(
        operation: &'static str,
        entity_id: EntityId,
        component_type: ComponentTypeId,
    ) -> Self {
        Self::new(
            "world.component.type_mismatch",
            operation,
            format!("component value does not match component type '{component_type}'"),
            "Provide a ComponentValue whose component type matches the requested type.",
        )
        .with_source_entity_id(entity_id)
        .with_component_type(component_type)
    }

    fn with_source_entity_id(mut self, entity_id: EntityId) -> Self {
        self.source_entity_id = Some(entity_id);
        self
    }

    fn with_runtime_entity_id(mut self, runtime_entity_id: RuntimeEntityId) -> Self {
        self.runtime_entity_id = Some(runtime_entity_id);
        self
    }

    fn with_parent_entity_id(mut self, parent_entity_id: EntityId) -> Self {
        self.parent_entity_id = Some(parent_entity_id);
        self
    }

    fn with_component_type(mut self, component_type: ComponentTypeId) -> Self {
        self.component_type = Some(component_type);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityLocation {
    pub archetype_id: ArchetypeId,
    pub row: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySlot {
    pub generation: u32,
    pub alive: bool,
    pub source_id: SourceEntityId,
    pub location: EntityLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirtyType {
    RenderState,
    Transform,
    DynamicData,
    InstanceData,
    Physics2D,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyRecord {
    pub entity_id: SourceEntityId,
    pub dirty_type: DirtyType,
}

#[derive(Debug, Clone, Default)]
pub struct World {
    entity_slots: Vec<EntitySlot>,
    source_to_runtime: BTreeMap<SourceEntityId, RuntimeEntityId>,
    archetypes: BTreeMap<ArchetypeId, ArchetypeTable>,
    signature_to_archetype: BTreeMap<ArchetypeSignature, ArchetypeId>,
    dirty_records: Vec<DirtyRecord>,
    next_archetype_id: u32,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_spawn_entity(
        &mut self,
        id: EntityId,
        name: impl Into<String>,
        kind: impl Into<String>,
        enabled: bool,
        hierarchy: Hierarchy,
    ) -> Result<RuntimeEntityId, WorldMutationError> {
        self.validate_new_entity(&id, &hierarchy, "spawn_entity")?;
        Ok(self.spawn_entity(id, name, kind, enabled, hierarchy))
    }

    pub fn try_spawn_with_components(
        &mut self,
        id: EntityId,
        name: impl Into<String>,
        kind: impl Into<String>,
        enabled: bool,
        hierarchy: Hierarchy,
        transform: Option<Transform>,
        renderable: Option<Renderable>,
    ) -> Result<RuntimeEntityId, WorldMutationError> {
        self.validate_new_entity(&id, &hierarchy, "spawn_with_components")?;
        Ok(self.spawn_with_components(id, name, kind, enabled, hierarchy, transform, renderable))
    }

    pub(crate) fn commit_prepared_entity(
        &mut self,
        id: EntityId,
        name: String,
        kind: String,
        enabled: bool,
        hierarchy: Hierarchy,
        components: Vec<ComponentValue>,
    ) -> RuntimeEntityId {
        debug_assert!(!self.source_to_runtime.contains_key(&id));
        let component_types = components
            .iter()
            .map(ComponentValue::component_type)
            .collect::<Vec<_>>();
        let meta = EntityMeta {
            id: id.clone(),
            name,
            kind,
            enabled,
            alive: true,
            hierarchy: hierarchy.clone(),
        };
        let runtime_id = self.allocate_runtime_id(id.clone());
        let mut values = vec![
            ComponentValue::EntityMeta(meta),
            ComponentValue::Hierarchy(hierarchy),
        ];
        values.extend(components);
        self.insert_entity_row(runtime_id, values);
        for component_type in component_types {
            self.mark_dirty(id.clone(), dirty_type_for_component(&component_type));
        }
        runtime_id
    }

    pub fn try_insert_transform(
        &mut self,
        id: EntityId,
        transform: Transform,
    ) -> Result<(), WorldMutationError> {
        self.ensure_entity_exists(&id, "insert_transform")?;
        self.insert_transform(id, transform);
        Ok(())
    }

    pub fn try_insert_renderable(
        &mut self,
        id: EntityId,
        renderable: Renderable,
    ) -> Result<(), WorldMutationError> {
        self.ensure_entity_exists(&id, "insert_renderable")?;
        self.insert_renderable(id, renderable);
        Ok(())
    }

    pub fn try_insert_sprite_renderer2d(
        &mut self,
        id: EntityId,
        sprite: SpriteRenderer2D,
    ) -> Result<(), WorldMutationError> {
        self.ensure_entity_exists(&id, "insert_sprite_renderer2d")?;
        self.insert_sprite_renderer2d(id, sprite);
        Ok(())
    }

    pub fn try_insert_dynamic_component(
        &mut self,
        id: EntityId,
        component_type: ComponentTypeId,
        value: impl Into<RuntimeValue>,
    ) -> Result<(), WorldMutationError> {
        self.ensure_entity_exists(&id, "insert_dynamic_component")?;
        self.insert_dynamic_component(id, component_type, value);
        Ok(())
    }

    pub fn try_insert_component_value(
        &mut self,
        id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    ) -> Result<(), WorldMutationError> {
        self.ensure_entity_exists(&id, "insert_component")?;
        if value.component_type() != component_type {
            return Err(WorldMutationError::invalid_component_value(
                "insert_component",
                id,
                component_type,
            ));
        }
        self.insert_component_value(id, value);
        Ok(())
    }

    pub fn try_remove_component_value(
        &mut self,
        id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Result<ComponentValue, WorldMutationError> {
        self.ensure_entity_exists(id, "remove_component")?;
        self.remove_component_value(id, component_type)
            .ok_or_else(|| {
                WorldMutationError::missing_component(
                    "remove_component",
                    id.clone(),
                    component_type.clone(),
                )
            })
    }

    pub fn try_set_parent(
        &mut self,
        id: EntityId,
        parent_id: Option<EntityId>,
    ) -> Result<(), WorldMutationError> {
        self.validate_parent_change(&id, parent_id.as_ref())?;
        self.set_parent(id, parent_id);
        Ok(())
    }

    pub fn try_despawn_entity(
        &mut self,
        id: &EntityId,
    ) -> Result<RuntimeEntityId, WorldMutationError> {
        self.ensure_entity_exists(id, "despawn_entity")?;
        Ok(self
            .despawn_entity(id)
            .expect("entity existence was validated before despawn"))
    }

    pub fn try_resolve_runtime_entity(
        &self,
        runtime_id: RuntimeEntityId,
    ) -> Result<&EntitySlot, WorldMutationError> {
        let Some(slot) = self.entity_slots.get(runtime_id.index as usize) else {
            return Err(WorldMutationError::new(
                "world.entity.stale_handle",
                "resolve_runtime_entity",
                format!("runtime entity handle is out of range: {runtime_id}"),
                "Discard the stale handle and resolve the entity again.",
            )
            .with_runtime_entity_id(runtime_id));
        };
        if slot.generation != runtime_id.generation || !slot.alive {
            return Err(WorldMutationError::new(
                "world.entity.stale_handle",
                "resolve_runtime_entity",
                format!("runtime entity handle is stale: {runtime_id}"),
                "Discard the stale handle and resolve the entity again.",
            )
            .with_runtime_entity_id(runtime_id)
            .with_source_entity_id(slot.source_id.clone()));
        }
        Ok(slot)
    }

    pub fn try_despawn_runtime_entity(
        &mut self,
        runtime_id: RuntimeEntityId,
    ) -> Result<SourceEntityId, WorldMutationError> {
        let source_id = self
            .try_resolve_runtime_entity(runtime_id)?
            .source_id
            .clone();
        self.try_despawn_entity(&source_id)?;
        Ok(source_id)
    }

    pub(crate) fn spawn_entity(
        &mut self,
        id: EntityId,
        name: impl Into<String>,
        kind: impl Into<String>,
        enabled: bool,
        hierarchy: Hierarchy,
    ) -> RuntimeEntityId {
        let meta = EntityMeta {
            id: id.clone(),
            name: name.into(),
            kind: kind.into(),
            enabled,
            alive: true,
            hierarchy: hierarchy.clone(),
        };
        let runtime_id = self.allocate_runtime_id(id);
        self.insert_entity_row(
            runtime_id,
            vec![
                ComponentValue::EntityMeta(meta),
                ComponentValue::Hierarchy(hierarchy),
            ],
        );
        runtime_id
    }

    pub(crate) fn spawn_with_components(
        &mut self,
        id: EntityId,
        name: impl Into<String>,
        kind: impl Into<String>,
        enabled: bool,
        hierarchy: Hierarchy,
        transform: Option<Transform>,
        renderable: Option<Renderable>,
    ) -> RuntimeEntityId {
        let meta = EntityMeta {
            id: id.clone(),
            name: name.into(),
            kind: kind.into(),
            enabled,
            alive: true,
            hierarchy: hierarchy.clone(),
        };
        let runtime_id = self.allocate_runtime_id(id);
        let mut values = vec![
            ComponentValue::EntityMeta(meta),
            ComponentValue::Hierarchy(hierarchy),
        ];
        if let Some(transform) = transform {
            values.push(ComponentValue::Transform(transform));
        }
        if let Some(renderable) = renderable {
            values.push(ComponentValue::Renderable(renderable));
        }
        self.insert_entity_row(runtime_id, values);
        runtime_id
    }

    pub(crate) fn insert_transform(&mut self, id: EntityId, transform: Transform) {
        self.insert_or_replace_component(&id, ComponentValue::Transform(transform));
        self.mark_dirty(id, DirtyType::Transform);
    }

    pub(crate) fn insert_renderable(&mut self, id: EntityId, renderable: Renderable) {
        self.insert_or_replace_component(&id, ComponentValue::Renderable(renderable));
        self.mark_dirty(id, DirtyType::RenderState);
    }

    pub(crate) fn insert_sprite_renderer2d(&mut self, id: EntityId, sprite: SpriteRenderer2D) {
        self.insert_or_replace_component(&id, ComponentValue::SpriteRenderer2D(sprite));
        self.mark_dirty(id, DirtyType::RenderState);
    }

    pub(crate) fn set_parent(&mut self, id: EntityId, parent_id: Option<EntityId>) {
        let runtime_id = *self
            .source_to_runtime
            .get(&id)
            .unwrap_or_else(|| panic!("missing entity: {}", id));
        let mut values = self.remove_runtime_row(runtime_id);
        let sibling_order = values
            .iter()
            .find_map(|value| match value {
                ComponentValue::Hierarchy(hierarchy) => Some(hierarchy.sibling_order),
                ComponentValue::EntityMeta(meta) => Some(meta.hierarchy.sibling_order),
                _ => None,
            })
            .unwrap_or(0);
        let hierarchy = Hierarchy {
            parent_id,
            sibling_order,
        };
        for value in &mut values {
            match value {
                ComponentValue::EntityMeta(meta) => meta.hierarchy = hierarchy.clone(),
                ComponentValue::Hierarchy(existing) => *existing = hierarchy.clone(),
                _ => {}
            }
        }
        self.insert_entity_row(runtime_id, values);
        self.mark_dirty(id, DirtyType::Transform);
    }

    pub(crate) fn insert_dynamic_component(
        &mut self,
        id: EntityId,
        component_type: ComponentTypeId,
        value: impl Into<RuntimeValue>,
    ) {
        self.insert_or_replace_component(
            &id,
            ComponentValue::Dynamic {
                component_type,
                value: value.into(),
            },
        );
        self.mark_dirty(id, DirtyType::DynamicData);
    }

    pub(crate) fn insert_component_value(&mut self, id: EntityId, value: ComponentValue) {
        let component_type = value.component_type();
        self.insert_or_replace_component(&id, value);
        self.mark_dirty(id, dirty_type_for_component(&component_type));
    }

    pub(crate) fn remove_renderable(&mut self, id: &EntityId) -> Option<Renderable> {
        let removed = self.remove_component(id, &ComponentTypeId::renderable())?;
        let ComponentValue::Renderable(renderable) = removed else {
            return None;
        };
        self.mark_dirty(id.clone(), DirtyType::RenderState);
        Some(renderable)
    }

    pub(crate) fn remove_sprite_renderer2d(&mut self, id: &EntityId) -> Option<SpriteRenderer2D> {
        let removed = self.remove_component(id, &ComponentTypeId::sprite_renderer2d())?;
        let ComponentValue::SpriteRenderer2D(sprite) = removed else {
            return None;
        };
        self.mark_dirty(id.clone(), DirtyType::RenderState);
        Some(sprite)
    }

    pub(crate) fn remove_component_value(
        &mut self,
        id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Option<ComponentValue> {
        let removed = self.remove_component(id, component_type)?;
        self.mark_dirty(id.clone(), dirty_type_for_component(component_type));
        Some(removed)
    }

    pub fn entity_count(&self) -> usize {
        self.entity_slots.iter().filter(|slot| slot.alive).count()
    }

    pub(crate) fn despawn_entity(&mut self, id: &EntityId) -> Option<RuntimeEntityId> {
        let runtime_id = *self.source_to_runtime.get(id)?;
        let values = self.remove_runtime_row(runtime_id);
        let had_render_facing_component = values.iter().any(|value| {
            let component_type = value.component_type();
            component_type == ComponentTypeId::renderable()
                || component_type == ComponentTypeId::sprite_renderer2d()
        });
        self.source_to_runtime.remove(id);
        if let Some(slot) = self.slot_mut(runtime_id) {
            slot.alive = false;
            slot.generation = slot.generation.saturating_add(1);
        }
        if had_render_facing_component {
            self.mark_dirty(id.clone(), DirtyType::RenderState);
        }
        Some(runtime_id)
    }

    pub fn archetype_count(&self) -> usize {
        self.archetypes.len()
    }

    pub fn entity_location(&self, id: &EntityId) -> Option<EntityLocation> {
        let runtime_id = self.source_to_runtime.get(id)?;
        Some(self.slot(*runtime_id)?.location)
    }

    pub fn runtime_id_for_source(&self, id: &EntityId) -> Option<RuntimeEntityId> {
        self.source_to_runtime.get(id).copied()
    }

    pub fn hierarchy(&self, id: &EntityId) -> Option<Hierarchy> {
        let value = self.component_value(id, &ComponentTypeId::hierarchy())?;
        let ComponentValue::Hierarchy(hierarchy) = value else {
            return None;
        };
        Some(hierarchy)
    }

    pub fn entity(&self, id: &EntityId) -> Option<&EntityMeta> {
        let (table, row) = self.table_row_for_source(id)?;
        table.entity_meta(row)
    }

    pub fn transform(&self, id: &EntityId) -> Option<&Transform> {
        let (table, row) = self.table_row_for_source(id)?;
        table.transform(row)
    }

    pub fn renderable(&self, id: &EntityId) -> Option<&Renderable> {
        let (table, row) = self.table_row_for_source(id)?;
        table.renderable(row)
    }

    pub fn sprite_renderer2d(&self, id: &EntityId) -> Option<&SpriteRenderer2D> {
        let (table, row) = self.table_row_for_source(id)?;
        table.sprite_renderer2d(row)
    }

    pub fn animator2d(&self, id: &EntityId) -> Option<&crate::components::Animator2D> {
        let (table, row) = self.table_row_for_source(id)?;
        table.animator2d(row)
    }

    pub fn collider2d(&self, id: &EntityId) -> Option<&crate::physics2d::Collider2D> {
        let (table, row) = self.table_row_for_source(id)?;
        table.collider2d(row)
    }

    pub fn component_value(
        &self,
        id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Option<ComponentValue> {
        let (table, row) = self.table_row_for_source(id)?;
        table.component_value(row, component_type)
    }

    pub fn alive_renderables(&self) -> Vec<(&EntityId, &Transform, &Renderable)> {
        let mut rows = Vec::new();
        for table in self.archetypes.values() {
            if !table.has_component(&ComponentTypeId::entity_meta())
                || !table.has_component(&ComponentTypeId::transform())
                || !table.has_component(&ComponentTypeId::renderable())
            {
                continue;
            }
            for row in 0..table.len() {
                let Some(meta) = table.entity_meta(row) else {
                    continue;
                };
                if !meta.alive || !meta.enabled {
                    continue;
                }
                let Some(transform) = table.transform(row) else {
                    continue;
                };
                let Some(renderable) = table.renderable(row) else {
                    continue;
                };
                rows.push((&meta.id, transform, renderable));
            }
        }
        rows.sort_by(|left, right| left.0.cmp(right.0));
        rows
    }

    pub fn entity_ids(&self) -> Vec<&EntityId> {
        let mut ids = self
            .entity_slots
            .iter()
            .filter(|slot| slot.alive)
            .map(|slot| &slot.source_id)
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub fn query_entities(&self, spec: &QuerySpec) -> Vec<EntityId> {
        let mut ids = Vec::new();
        for table in self.archetypes.values() {
            if spec
                .all
                .iter()
                .any(|component_type| !table.has_component(component_type))
            {
                continue;
            }
            if spec
                .none
                .iter()
                .any(|component_type| table.has_component(component_type))
            {
                continue;
            }
            for row in 0..table.len() {
                let Some(meta) = table.entity_meta(row) else {
                    continue;
                };
                if !meta.alive {
                    continue;
                }
                if !spec.include_disabled && !meta.enabled {
                    continue;
                }
                ids.push(meta.id.clone());
            }
        }
        match spec.stable_order {
            StableOrder::EntityId => ids.sort(),
        }
        if let Some(limit) = spec.limit {
            ids.truncate(limit);
        }
        ids
    }

    pub fn dirty_records(&self) -> &[DirtyRecord] {
        &self.dirty_records
    }

    pub fn take_dirty_records(&mut self) -> Vec<DirtyRecord> {
        std::mem::take(&mut self.dirty_records)
    }

    fn validate_new_entity(
        &self,
        id: &EntityId,
        hierarchy: &Hierarchy,
        operation: &'static str,
    ) -> Result<(), WorldMutationError> {
        if self.source_to_runtime.contains_key(id) {
            return Err(WorldMutationError::new(
                "world.entity.duplicate_id",
                operation,
                format!("entity already exists: {id}"),
                "Use a unique source entity id.",
            )
            .with_source_entity_id(id.clone()));
        }
        if let Some(parent_id) = hierarchy.parent_id.as_ref() {
            if parent_id == id {
                return Err(WorldMutationError::new(
                    "world.parent.self",
                    operation,
                    format!("entity cannot be its own parent: {id}"),
                    "Choose a different parent or no parent.",
                )
                .with_source_entity_id(id.clone())
                .with_parent_entity_id(parent_id.clone()));
            }
            if !self.source_to_runtime.contains_key(parent_id) {
                return Err(WorldMutationError::new(
                    "world.parent.missing",
                    operation,
                    format!("parent entity does not exist: {parent_id}"),
                    "Create the parent first or remove the parent reference.",
                )
                .with_source_entity_id(id.clone())
                .with_parent_entity_id(parent_id.clone()));
            }
        }
        Ok(())
    }

    fn ensure_entity_exists(
        &self,
        id: &EntityId,
        operation: &'static str,
    ) -> Result<(), WorldMutationError> {
        if self
            .source_to_runtime
            .get(id)
            .and_then(|runtime_id| self.slot(*runtime_id))
            .is_some_and(|slot| slot.alive)
        {
            Ok(())
        } else {
            Err(WorldMutationError::missing_entity(operation, id.clone()))
        }
    }

    fn validate_parent_change(
        &self,
        id: &EntityId,
        parent_id: Option<&EntityId>,
    ) -> Result<(), WorldMutationError> {
        self.ensure_entity_exists(id, "set_parent")?;
        let Some(parent_id) = parent_id else {
            return Ok(());
        };
        if parent_id == id {
            return Err(WorldMutationError::new(
                "world.parent.self",
                "set_parent",
                format!("entity cannot be its own parent: {id}"),
                "Choose a different parent or no parent.",
            )
            .with_source_entity_id(id.clone())
            .with_parent_entity_id(parent_id.clone()));
        }
        self.ensure_entity_exists(parent_id, "set_parent")
            .map_err(|_| {
                WorldMutationError::new(
                    "world.parent.missing",
                    "set_parent",
                    format!("parent entity does not exist: {parent_id}"),
                    "Use an alive parent entity from the current World.",
                )
                .with_source_entity_id(id.clone())
                .with_parent_entity_id(parent_id.clone())
            })?;

        let mut current = Some(parent_id.clone());
        let mut visited = BTreeSet::new();
        while let Some(candidate) = current {
            if candidate == *id || !visited.insert(candidate.clone()) {
                return Err(WorldMutationError::new(
                    "world.parent.cycle",
                    "set_parent",
                    format!("setting parent '{parent_id}' would create a hierarchy cycle"),
                    "Choose a parent outside the entity's descendant chain.",
                )
                .with_source_entity_id(id.clone())
                .with_parent_entity_id(parent_id.clone()));
            }
            current = self.hierarchy(&candidate).and_then(|value| value.parent_id);
        }
        Ok(())
    }

    fn allocate_runtime_id(&mut self, source_id: SourceEntityId) -> RuntimeEntityId {
        if self.source_to_runtime.contains_key(&source_id) {
            panic!("entity already exists: {}", source_id);
        }
        let runtime_id = RuntimeEntityId::new(self.entity_slots.len() as u32, 0);
        self.source_to_runtime.insert(source_id, runtime_id);
        runtime_id
    }

    fn insert_entity_row(&mut self, runtime_id: RuntimeEntityId, mut values: Vec<ComponentValue>) {
        values.sort_by_key(ComponentValue::component_type);
        let source_id = values
            .iter()
            .find_map(|value| match value {
                ComponentValue::EntityMeta(meta) => Some(meta.id.clone()),
                _ => None,
            })
            .expect("entity rows must contain EntityMeta");
        let archetype_id = self.archetype_id_for_values(&values);
        let table = self
            .archetypes
            .get_mut(&archetype_id)
            .expect("archetype should exist");
        let row = table.push_row(runtime_id, values);
        let slot = EntitySlot {
            generation: runtime_id.generation,
            alive: true,
            source_id,
            location: EntityLocation { archetype_id, row },
        };
        let index = runtime_id.index as usize;
        if index == self.entity_slots.len() {
            self.entity_slots.push(slot);
        } else {
            self.entity_slots[index] = slot;
        }
    }

    fn insert_or_replace_component(&mut self, source_id: &EntityId, value: ComponentValue) {
        let runtime_id = *self
            .source_to_runtime
            .get(source_id)
            .unwrap_or_else(|| panic!("missing entity: {}", source_id));
        let component_type = value.component_type();
        let mut values = self.remove_runtime_row(runtime_id);
        if let Some(existing) = values
            .iter_mut()
            .find(|existing| existing.component_type() == component_type)
        {
            *existing = value;
        } else {
            values.push(value);
        }
        values.sort_by_key(ComponentValue::component_type);
        self.insert_entity_row(runtime_id, values);
    }

    fn remove_component(
        &mut self,
        source_id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Option<ComponentValue> {
        let runtime_id = *self.source_to_runtime.get(source_id)?;
        let mut values = self.remove_runtime_row(runtime_id);
        let Some(index) = values
            .iter()
            .position(|value| value.component_type() == *component_type)
        else {
            self.insert_entity_row(runtime_id, values);
            return None;
        };
        let removed = values.remove(index);
        self.insert_entity_row(runtime_id, values);
        Some(removed)
    }

    fn remove_runtime_row(&mut self, runtime_id: RuntimeEntityId) -> Vec<ComponentValue> {
        let location = self
            .slot(runtime_id)
            .unwrap_or_else(|| panic!("missing runtime entity: {}", runtime_id))
            .location;
        let table = self
            .archetypes
            .get_mut(&location.archetype_id)
            .expect("entity location points to missing archetype");
        let removed = table.swap_remove_row(location.row);
        if let Some(moved_entity) = removed.moved_entity {
            let moved_slot = self
                .slot_mut(moved_entity)
                .expect("moved entity should have slot");
            moved_slot.location.row = location.row;
        }
        let slot = self
            .slot_mut(runtime_id)
            .expect("removed entity should have slot");
        slot.location.row = usize::MAX;
        removed.values
    }

    fn archetype_id_for_values(&mut self, values: &[ComponentValue]) -> ArchetypeId {
        let signature = ArchetypeSignature::new(values.iter().map(ComponentValue::component_type));
        if let Some(id) = self.signature_to_archetype.get(&signature) {
            return *id;
        }
        let id = ArchetypeId(self.next_archetype_id);
        self.next_archetype_id += 1;
        let table = ArchetypeTable::new(id, signature.clone());
        self.signature_to_archetype.insert(signature, id);
        self.archetypes.insert(id, table);
        id
    }

    fn table_row_for_source(&self, source_id: &EntityId) -> Option<(&ArchetypeTable, usize)> {
        let runtime_id = self.source_to_runtime.get(source_id)?;
        let slot = self.slot(*runtime_id)?;
        if !slot.alive {
            return None;
        }
        let table = self.archetypes.get(&slot.location.archetype_id)?;
        Some((table, slot.location.row))
    }

    fn slot(&self, runtime_id: RuntimeEntityId) -> Option<&EntitySlot> {
        let slot = self.entity_slots.get(runtime_id.index as usize)?;
        if slot.generation == runtime_id.generation {
            Some(slot)
        } else {
            None
        }
    }

    fn slot_mut(&mut self, runtime_id: RuntimeEntityId) -> Option<&mut EntitySlot> {
        let slot = self.entity_slots.get_mut(runtime_id.index as usize)?;
        if slot.generation == runtime_id.generation {
            Some(slot)
        } else {
            None
        }
    }

    fn mark_dirty(&mut self, entity_id: SourceEntityId, dirty_type: DirtyType) {
        self.dirty_records.push(DirtyRecord {
            entity_id,
            dirty_type,
        });
    }
}

pub fn dirty_type_for_component(component_type: &ComponentTypeId) -> DirtyType {
    if component_type == &ComponentTypeId::transform() {
        DirtyType::Transform
    } else if component_type == &ComponentTypeId::renderable()
        || component_type == &ComponentTypeId::sprite_renderer2d()
    {
        DirtyType::RenderState
    } else if component_type == &ComponentTypeId::collider2d() {
        DirtyType::Physics2D
    } else if component_type == &ComponentTypeId::animator2d() {
        DirtyType::InstanceData
    } else {
        DirtyType::DynamicData
    }
}

#[derive(Debug, Clone, Default)]
pub struct SparseSet {
    entities: BTreeSet<RuntimeEntityId>,
}

impl SparseSet {
    pub fn insert(&mut self, entity: RuntimeEntityId) {
        self.entities.insert(entity);
    }

    pub fn remove(&mut self, entity: &RuntimeEntityId) {
        self.entities.remove(entity);
    }

    pub fn contains(&self, entity: &RuntimeEntityId) -> bool {
        self.entities.contains(entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::SpriteRenderer2D;
    use crate::math::Vec3;

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn transform(y: f32) -> Transform {
        Transform {
            local_position: Vec3 { x: 0.0, y, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn renderable(mesh: &str) -> Renderable {
        Renderable {
            mesh_ref: Some(mesh.to_string()),
            material_ref: None,
            visible: true,
            layer: "default".to_string(),
        }
    }

    fn sprite(sprite_ref: &str) -> SpriteRenderer2D {
        SpriteRenderer2D {
            sprite_ref: Some(sprite_ref.to_string()),
            ..SpriteRenderer2D::default()
        }
    }

    #[test]
    fn world_spawn_creates_runtime_location_and_source_mapping() {
        let mut world = World::new();
        world.spawn_entity(EntityId::from("entity-a"), "A", "actor", true, hierarchy());
        let location = world.entity_location(&EntityId::from("entity-a")).unwrap();
        assert_eq!(world.entity_count(), 1);
        assert_eq!(location.row, 0);
        assert!(world.entity(&EntityId::from("entity-a")).is_some());
    }

    #[test]
    fn query_spec_all_returns_entities_with_all_components() {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-a"),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );
        world.spawn_entity(EntityId::from("entity-b"), "B", "actor", true, hierarchy());

        let result = world.query_entities(&QuerySpec::all([ComponentTypeId::transform()]));

        assert_eq!(result, vec![EntityId::from("entity-a")]);
    }

    #[test]
    fn query_spec_none_excludes_entities() {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-a"),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            Some(Renderable {
                mesh_ref: None,
                material_ref: None,
                visible: true,
                layer: "default".to_string(),
            }),
        );
        world.spawn_with_components(
            EntityId::from("entity-b"),
            "B",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );

        let result = world.query_entities(
            &QuerySpec::all([ComponentTypeId::transform()])
                .excluding([ComponentTypeId::renderable()]),
        );

        assert_eq!(result, vec![EntityId::from("entity-b")]);
    }

    #[test]
    fn query_spec_excludes_disabled_by_default() {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-a"),
            "A",
            "actor",
            false,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );

        let default_result = world.query_entities(&QuerySpec::all([ComponentTypeId::transform()]));
        let include_disabled_result = world
            .query_entities(&QuerySpec::all([ComponentTypeId::transform()]).include_disabled());

        assert!(default_result.is_empty());
        assert_eq!(include_disabled_result, vec![EntityId::from("entity-a")]);
    }

    #[test]
    fn query_spec_limit_is_applied_after_stable_order() {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-b"),
            "B",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );
        world.spawn_with_components(
            EntityId::from("entity-a"),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );

        let result = world.query_entities(&QuerySpec::all([ComponentTypeId::transform()]).limit(1));

        assert_eq!(result, vec![EntityId::from("entity-a")]);
    }

    #[test]
    fn world_insert_transform_migrates_entity_to_new_archetype() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_entity(id.clone(), "A", "actor", true, hierarchy());
        let before = world.entity_location(&id).unwrap().archetype_id;
        world.insert_transform(id.clone(), transform(3.0));
        let after = world.entity_location(&id).unwrap().archetype_id;
        assert_ne!(before, after);
        assert_eq!(world.transform(&id).unwrap().local_position.y, 3.0);
    }

    #[test]
    fn world_query_alive_renderables_uses_archetype_tables() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_with_components(
            id,
            "A",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0)),
            Some(renderable("mesh-a")),
        );
        let rows = world.alive_renderables();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0.as_str(), "entity-a");
        assert_eq!(rows[0].1.local_position.y, 2.0);
        assert_eq!(rows[0].2.mesh_ref.as_deref(), Some("mesh-a"));
    }

    #[test]
    fn world_remove_renderable_migrates_entity_and_returns_component() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_with_components(
            id.clone(),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0)),
            Some(renderable("mesh-a")),
        );
        let removed = world.remove_renderable(&id).unwrap();
        assert_eq!(removed.mesh_ref.as_deref(), Some("mesh-a"));
        assert!(world.renderable(&id).is_none());
        assert!(world.transform(&id).is_some());
    }

    #[test]
    fn world_insert_sprite_renderer2d_migrates_entity_and_marks_render_dirty() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_with_components(
            id.clone(),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0)),
            None,
        );

        world.insert_sprite_renderer2d(id.clone(), sprite("sprite-a"));

        assert_eq!(
            world.sprite_renderer2d(&id).unwrap().sprite_ref.as_deref(),
            Some("sprite-a")
        );
        assert!(world
            .dirty_records()
            .iter()
            .any(|record| record.dirty_type == DirtyType::RenderState));
    }

    #[test]
    fn world_remove_sprite_renderer2d_marks_render_dirty() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_with_components(
            id.clone(),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0)),
            None,
        );
        world.insert_sprite_renderer2d(id.clone(), sprite("sprite-a"));
        world.take_dirty_records();

        let removed = world.remove_sprite_renderer2d(&id).unwrap();

        assert_eq!(removed.sprite_ref.as_deref(), Some("sprite-a"));
        assert!(world.sprite_renderer2d(&id).is_none());
        assert_eq!(
            world.dirty_records(),
            &[DirtyRecord {
                entity_id: id,
                dirty_type: DirtyType::RenderState,
            }]
        );
    }

    #[test]
    fn world_remove_missing_component_preserves_entity() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_with_components(
            id.clone(),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(transform(2.0)),
            None,
        );
        assert!(world.remove_renderable(&id).is_none());
        assert_eq!(world.entity_count(), 1);
        assert!(world.entity(&id).is_some());
        assert!(world.transform(&id).is_some());
    }

    #[test]
    fn world_records_dirty_for_render_facing_writes() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world.spawn_entity(id.clone(), "A", "actor", true, hierarchy());
        world.insert_transform(id.clone(), transform(4.0));
        world.insert_renderable(id, renderable("mesh-a"));
        assert_eq!(
            world.dirty_records(),
            &[
                DirtyRecord {
                    entity_id: EntityId::from("entity-a"),
                    dirty_type: DirtyType::Transform,
                },
                DirtyRecord {
                    entity_id: EntityId::from("entity-a"),
                    dirty_type: DirtyType::RenderState,
                },
            ]
        );
    }

    #[test]
    fn try_spawn_duplicate_returns_diagnostic_without_mutating_world() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world
            .try_spawn_entity(id.clone(), "A", "actor", true, hierarchy())
            .expect("first spawn should succeed");
        world.take_dirty_records();

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.try_spawn_entity(id.clone(), "Duplicate", "actor", true, hierarchy())
        }));

        let error = outcome
            .expect("public mutation must not unwind")
            .expect_err("duplicate spawn must fail");
        assert_eq!(error.code, "world.entity.duplicate_id");
        assert_eq!(world.entity_count(), 1);
        assert_eq!(world.entity(&id).unwrap().name, "A");
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn try_insert_missing_entity_returns_diagnostic_without_dirty_record() {
        let mut world = World::new();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.try_insert_transform(EntityId::from("missing"), Transform::identity())
        }));

        let error = outcome
            .expect("public mutation must not unwind")
            .expect_err("missing entity insert must fail");
        assert_eq!(error.code, "world.entity.missing");
        assert_eq!(world.entity_count(), 0);
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn try_set_parent_rejects_missing_self_and_cycle_without_mutation() {
        let mut world = World::new();
        let root = EntityId::from("root");
        let child = EntityId::from("child");
        world
            .try_spawn_entity(root.clone(), "Root", "actor", true, hierarchy())
            .unwrap();
        world
            .try_spawn_entity(
                child.clone(),
                "Child",
                "actor",
                true,
                Hierarchy {
                    parent_id: Some(root.clone()),
                    sibling_order: 0,
                },
            )
            .unwrap();
        world.take_dirty_records();

        let missing = world
            .try_set_parent(child.clone(), Some(EntityId::from("missing")))
            .expect_err("missing parent must fail");
        assert_eq!(missing.code, "world.parent.missing");

        let self_parent = world
            .try_set_parent(child.clone(), Some(child.clone()))
            .expect_err("self parent must fail");
        assert_eq!(self_parent.code, "world.parent.self");

        let cycle = world
            .try_set_parent(root.clone(), Some(child.clone()))
            .expect_err("cycle must fail");
        assert_eq!(cycle.code, "world.parent.cycle");
        assert_eq!(world.hierarchy(&root).unwrap().parent_id, None);
        assert_eq!(
            world.hierarchy(&child).unwrap().parent_id,
            Some(root.clone())
        );
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn try_resolve_runtime_entity_rejects_stale_generation() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        let runtime_id = world
            .try_spawn_entity(id.clone(), "A", "actor", true, hierarchy())
            .unwrap();
        world.try_despawn_entity(&id).unwrap();

        let outcome = std::panic::catch_unwind(|| world.try_resolve_runtime_entity(runtime_id));
        let error = outcome
            .expect("stale handle resolution must not unwind")
            .expect_err("despawned handle must be stale");
        assert_eq!(error.code, "world.entity.stale_handle");
    }

    #[test]
    fn try_insert_component_rejects_type_mismatch_without_mutation() {
        let mut world = World::new();
        let id = EntityId::from("entity-a");
        world
            .try_spawn_entity(id.clone(), "A", "actor", true, hierarchy())
            .unwrap();
        world.take_dirty_records();

        let error = world
            .try_insert_component_value(
                id.clone(),
                ComponentTypeId::transform(),
                ComponentValue::Renderable(renderable("mesh-a")),
            )
            .expect_err("mismatched component must fail");
        assert_eq!(error.code, "world.component.type_mismatch");
        assert!(world.transform(&id).is_none());
        assert!(world.renderable(&id).is_none());
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn sparse_set_tracks_sparse_entities() {
        let mut set = SparseSet::default();
        let entity = RuntimeEntityId::new(1, 0);
        set.insert(entity);
        assert!(set.contains(&entity));
        set.remove(&entity);
        assert!(!set.contains(&entity));
    }
}
