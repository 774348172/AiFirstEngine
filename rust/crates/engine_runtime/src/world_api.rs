use crate::archetype::ComponentValue;
use crate::component_value::RuntimeValue;
use crate::components::{ComponentTypeId, Transform};
use crate::field_path::FieldPath;
use crate::ids::EntityId;
use crate::math::Vec3;
use crate::query::QuerySpec;
use crate::world::World;
pub use crate::world::WorldMutationError as WorldApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldWriteRecord {
    pub entity_id: EntityId,
    pub component_type: ComponentTypeId,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug)]
pub struct WorldReadApi<'a> {
    world: &'a World,
}

impl<'a> WorldReadApi<'a> {
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    pub fn read_transform(&self, entity_id: &EntityId) -> Result<Transform, WorldApiError> {
        self.world.transform(entity_id).cloned().ok_or_else(|| {
            WorldApiError::missing_component(
                "read_transform",
                entity_id.clone(),
                ComponentTypeId::transform(),
            )
        })
    }

    pub fn read_component(
        &self,
        entity_id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Result<ComponentValue, WorldApiError> {
        self.world
            .component_value(entity_id, component_type)
            .ok_or_else(|| {
                WorldApiError::missing_component(
                    "read_component",
                    entity_id.clone(),
                    component_type.clone(),
                )
            })
    }

    pub fn query(&self, spec: &QuerySpec) -> Vec<EntityId> {
        self.world.query_entities(spec)
    }
}

#[derive(Debug)]
pub struct WorldWriteApi<'a> {
    world: &'a mut World,
}

impl<'a> WorldWriteApi<'a> {
    pub fn new(world: &'a mut World) -> Self {
        Self { world }
    }

    pub fn read_transform(&self, entity_id: &EntityId) -> Result<Transform, WorldApiError> {
        self.world.transform(entity_id).cloned().ok_or_else(|| {
            WorldApiError::missing_component(
                "read_transform",
                entity_id.clone(),
                ComponentTypeId::transform(),
            )
        })
    }

    pub fn read_component(
        &self,
        entity_id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Result<ComponentValue, WorldApiError> {
        self.world
            .component_value(entity_id, component_type)
            .ok_or_else(|| {
                WorldApiError::missing_component(
                    "read_component",
                    entity_id.clone(),
                    component_type.clone(),
                )
            })
    }

    pub fn query(&self, spec: &QuerySpec) -> Vec<EntityId> {
        self.world.query_entities(spec)
    }

    pub fn write_transform(
        &mut self,
        entity_id: EntityId,
        transform: Transform,
    ) -> Result<WorldWriteRecord, WorldApiError> {
        let before = self.world.transform(&entity_id).cloned();
        self.world
            .try_insert_transform(entity_id.clone(), transform.clone())?;
        Ok(WorldWriteRecord {
            entity_id,
            component_type: ComponentTypeId::transform(),
            field: "*".to_string(),
            before: before.map(|value| format!("{:?}", value)),
            after: Some(format!("{:?}", transform)),
        })
    }

    pub fn write_component(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    ) -> Result<WorldWriteRecord, WorldApiError> {
        if value.component_type() != component_type {
            return Err(WorldApiError::invalid_component_value(
                "write_component",
                entity_id,
                component_type,
            ));
        }
        let before = self.world.component_value(&entity_id, &component_type);
        self.world.try_insert_component_value(
            entity_id.clone(),
            component_type.clone(),
            value.clone(),
        )?;
        Ok(WorldWriteRecord {
            entity_id,
            component_type,
            field: "*".to_string(),
            before: before.map(|value| format!("{:?}", value)),
            after: Some(format!("{:?}", value)),
        })
    }

    pub fn write_component_field(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        field_path: &FieldPath,
        value: RuntimeValue,
    ) -> Result<WorldWriteRecord, WorldApiError> {
        if component_type == ComponentTypeId::transform() {
            return self.write_transform_field(entity_id, field_path, value);
        }
        let mut component = self.read_component(&entity_id, &component_type)?;
        let before = field_value(&component, field_path);
        set_component_field(&mut component, field_path, value.clone()).map_err(|()| {
            WorldApiError::unsupported_field(entity_id.clone(), component_type.clone())
        })?;
        self.world.try_insert_component_value(
            entity_id.clone(),
            component_type.clone(),
            component,
        )?;
        Ok(WorldWriteRecord {
            entity_id,
            component_type,
            field: field_path.as_str().to_string(),
            before: before.map(|value| format!("{:?}", value)),
            after: Some(format!("{:?}", value)),
        })
    }

    fn write_transform_field(
        &mut self,
        entity_id: EntityId,
        field_path: &FieldPath,
        value: RuntimeValue,
    ) -> Result<WorldWriteRecord, WorldApiError> {
        let mut transform = self.read_transform(&entity_id)?;
        let before = transform_field_value(&transform, field_path);
        let RuntimeValue::F64(number) = value else {
            return Err(WorldApiError::unsupported_field(
                entity_id,
                ComponentTypeId::transform(),
            ));
        };
        let number = number as f32;
        match field_path.as_str() {
            "local_position.x" => transform.local_position.x = number,
            "local_position.y" => transform.local_position.y = number,
            "local_position.z" => transform.local_position.z = number,
            _ => {
                return Err(WorldApiError::unsupported_field(
                    entity_id,
                    ComponentTypeId::transform(),
                ));
            }
        }
        self.world
            .try_insert_transform(entity_id.clone(), transform)?;
        Ok(WorldWriteRecord {
            entity_id,
            component_type: ComponentTypeId::transform(),
            field: field_path.as_str().to_string(),
            before: before.map(|value| format!("{:?}", value)),
            after: Some(format!("{:?}", RuntimeValue::F64(number as f64))),
        })
    }

    pub fn write_transform_local_position(
        &mut self,
        entity_id: EntityId,
        local_position: Vec3,
    ) -> Result<WorldWriteRecord, WorldApiError> {
        let mut transform = self.read_transform(&entity_id)?;
        let before = transform.local_position;
        transform.local_position = local_position;
        self.world
            .try_insert_transform(entity_id.clone(), transform)?;
        Ok(WorldWriteRecord {
            entity_id,
            component_type: ComponentTypeId::transform(),
            field: "local_position".to_string(),
            before: Some(format!("{:?}", before)),
            after: Some(format!("{:?}", local_position)),
        })
    }
}

fn field_value(component: &ComponentValue, field_path: &FieldPath) -> Option<RuntimeValue> {
    match component {
        ComponentValue::Dynamic { value, .. } => runtime_field_value(value, field_path),
        _ => None,
    }
}

fn runtime_field_value(value: &RuntimeValue, field_path: &FieldPath) -> Option<RuntimeValue> {
    let mut current = value;
    for segment in field_path.segments() {
        let RuntimeValue::Object(fields) = current else {
            return None;
        };
        current = fields.get(segment)?;
    }
    Some(current.clone())
}

fn transform_field_value(transform: &Transform, field_path: &FieldPath) -> Option<RuntimeValue> {
    match field_path.as_str() {
        "local_position.x" => Some(RuntimeValue::F64(transform.local_position.x as f64)),
        "local_position.y" => Some(RuntimeValue::F64(transform.local_position.y as f64)),
        "local_position.z" => Some(RuntimeValue::F64(transform.local_position.z as f64)),
        _ => None,
    }
}

fn set_component_field(
    component: &mut ComponentValue,
    field_path: &FieldPath,
    value: RuntimeValue,
) -> Result<(), ()> {
    match component {
        ComponentValue::Dynamic {
            value: dynamic_value,
            ..
        } => set_runtime_field(dynamic_value, field_path, value),
        _ => Err(()),
    }
}

pub(crate) fn prepare_component_field_write(
    entity_id: &EntityId,
    component_type: &ComponentTypeId,
    mut component: ComponentValue,
    field_path: &FieldPath,
    value: RuntimeValue,
) -> Result<ComponentValue, WorldApiError> {
    if component_type == &ComponentTypeId::transform() {
        let ComponentValue::Transform(mut transform) = component else {
            return Err(WorldApiError::invalid_component_value(
                "prepare_component_field_write",
                entity_id.clone(),
                component_type.clone(),
            ));
        };
        let RuntimeValue::F64(number) = value else {
            return Err(WorldApiError::unsupported_field(
                entity_id.clone(),
                component_type.clone(),
            ));
        };
        let number = number as f32;
        match field_path.as_str() {
            "local_position.x" => transform.local_position.x = number,
            "local_position.y" => transform.local_position.y = number,
            "local_position.z" => transform.local_position.z = number,
            _ => {
                return Err(WorldApiError::unsupported_field(
                    entity_id.clone(),
                    component_type.clone(),
                ));
            }
        }
        return Ok(ComponentValue::Transform(transform));
    }
    set_component_field(&mut component, field_path, value).map_err(|()| {
        WorldApiError::unsupported_field(entity_id.clone(), component_type.clone())
    })?;
    Ok(component)
}

fn set_runtime_field(
    root: &mut RuntimeValue,
    field_path: &FieldPath,
    value: RuntimeValue,
) -> Result<(), ()> {
    let mut segments = field_path.segments().peekable();
    let mut current = root;
    while let Some(segment) = segments.next() {
        let RuntimeValue::Object(fields) = current else {
            return Err(());
        };
        if segments.peek().is_none() {
            fields.insert(segment.to_string(), value);
            return Ok(());
        }
        current = fields.get_mut(segment).ok_or(())?;
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_value::RuntimeValue;
    use crate::components::Hierarchy;
    use crate::field_path::FieldPath;
    use crate::world::DirtyType;

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    #[test]
    fn world_write_api_write_transform_local_position_marks_dirty() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-a");
        world.spawn_entity(entity_id.clone(), "A", "actor", true, hierarchy());
        world.insert_transform(entity_id.clone(), Transform::identity());
        world.take_dirty_records();
        let mut api = WorldWriteApi::new(&mut world);

        let record = api
            .write_transform_local_position(
                entity_id.clone(),
                Vec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                },
            )
            .expect("write should succeed");

        assert_eq!(record.field, "local_position");
        assert_eq!(world.dirty_records()[0].dirty_type, DirtyType::Transform);
        assert_eq!(world.transform(&entity_id).unwrap().local_position.x, 2.0);
    }

    #[test]
    fn world_read_api_reads_dynamic_component() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-a");
        let component_type = ComponentTypeId::from("project.marker");
        world.spawn_entity(entity_id.clone(), "A", "actor", true, hierarchy());
        world.insert_dynamic_component(
            entity_id.clone(),
            component_type.clone(),
            RuntimeValue::object([("count", RuntimeValue::I64(1))]),
        );
        let api = WorldReadApi::new(&world);

        let component = api
            .read_component(&entity_id, &component_type)
            .expect("component should exist");

        assert_eq!(
            component,
            ComponentValue::Dynamic {
                component_type,
                value: RuntimeValue::object([("count", RuntimeValue::I64(1))]),
            }
        );
    }

    #[test]
    fn world_read_api_reports_missing_component() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-a");
        world.spawn_entity(entity_id.clone(), "A", "actor", true, hierarchy());
        let api = WorldReadApi::new(&world);

        let error = api
            .read_component(&entity_id, &ComponentTypeId::from("project.marker"))
            .expect_err("component should be missing");

        assert_eq!(error.code, "world.component.missing");
    }

    #[test]
    fn world_write_api_missing_entity_returns_error_without_unwind() {
        let mut world = World::new();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            WorldWriteApi::new(&mut world)
                .write_transform(EntityId::from("missing"), Transform::identity())
        }));

        let error = outcome
            .expect("WorldWriteApi must not unwind")
            .expect_err("missing entity write must fail");
        assert_eq!(error.code, "world.entity.missing");
        assert_eq!(world.entity_count(), 0);
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn write_component_field_updates_dynamic_object_field() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-a");
        let component_type = ComponentTypeId::from("project.marker");
        world.spawn_entity(entity_id.clone(), "A", "actor", true, hierarchy());
        world.insert_dynamic_component(
            entity_id.clone(),
            component_type.clone(),
            RuntimeValue::object([("count", RuntimeValue::I64(1))]),
        );
        world.take_dirty_records();
        let mut api = WorldWriteApi::new(&mut world);

        let record = api
            .write_component_field(
                entity_id.clone(),
                component_type.clone(),
                &FieldPath::parse("count").unwrap(),
                RuntimeValue::I64(2),
            )
            .expect("write should succeed");

        assert_eq!(record.before, Some("I64(1)".to_string()));
        assert_eq!(record.after, Some("I64(2)".to_string()));
        assert_eq!(world.dirty_records()[0].dirty_type, DirtyType::DynamicData);
        assert_eq!(
            world.component_value(&entity_id, &component_type),
            Some(ComponentValue::Dynamic {
                component_type,
                value: RuntimeValue::object([("count", RuntimeValue::I64(2))]),
            })
        );
    }

    #[test]
    fn write_component_field_updates_transform_local_position_x() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-a");
        world.spawn_entity(entity_id.clone(), "A", "actor", true, hierarchy());
        world.insert_transform(entity_id.clone(), Transform::identity());
        world.take_dirty_records();
        let mut api = WorldWriteApi::new(&mut world);

        let record = api
            .write_component_field(
                entity_id.clone(),
                ComponentTypeId::transform(),
                &FieldPath::parse("local_position.x").unwrap(),
                RuntimeValue::F64(3.0),
            )
            .expect("write should succeed");

        assert_eq!(record.before, Some("F64(0.0)".to_string()));
        assert_eq!(record.after, Some("F64(3.0)".to_string()));
        assert_eq!(world.transform(&entity_id).unwrap().local_position.x, 3.0);
        assert_eq!(world.dirty_records()[0].dirty_type, DirtyType::Transform);
    }

    #[test]
    fn write_component_replaces_dynamic_component() {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-a");
        let component_type = ComponentTypeId::from("project.marker");
        world.spawn_entity(entity_id.clone(), "A", "actor", true, hierarchy());
        world.insert_dynamic_component(
            entity_id.clone(),
            component_type.clone(),
            RuntimeValue::I64(1),
        );
        let mut api = WorldWriteApi::new(&mut world);

        api.write_component(
            entity_id.clone(),
            component_type.clone(),
            ComponentValue::Dynamic {
                component_type: component_type.clone(),
                value: RuntimeValue::I64(2),
            },
        )
        .expect("write should succeed");

        assert_eq!(
            world.component_value(&entity_id, &component_type),
            Some(ComponentValue::Dynamic {
                component_type,
                value: RuntimeValue::I64(2),
            })
        );
    }
}
