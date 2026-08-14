use crate::archetype::ComponentValue;
use crate::component_value::RuntimeValue;
use crate::components::{ComponentTypeId, Hierarchy, Renderable, SpriteRenderer2D, Transform};
use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::math::{Vec2, Vec3};
use crate::physics2d::{Collider2D, PhysicsLayer, PhysicsMask, Shape2D};
use crate::runtime_instance::RuntimeInstanceId;
use crate::runtime_instance_diagnostics::{InstanceDiagnostic, InstanceStage};
use crate::runtime_package::{
    RuntimeEntity, RuntimeMesh, RuntimeSpriteRenderer2D, RuntimeTransform,
};
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeEntityNamespace {
    Source,
    Instance(RuntimeInstanceId),
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedRuntimeEntity {
    pub source_id: SourceEntityId,
    pub world_id: SourceEntityId,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub hierarchy: Hierarchy,
    pub components: Vec<ComponentValue>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedRuntimeEntities {
    pub entities: Vec<PreparedRuntimeEntity>,
    pub source_to_world: BTreeMap<SourceEntityId, SourceEntityId>,
    pub root_sources: Vec<SourceEntityId>,
    pub remapped_reference_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedRuntimeCommit {
    pub source_to_runtime: BTreeMap<SourceEntityId, RuntimeEntityId>,
}

impl PreparedRuntimeEntities {
    pub(crate) fn prepare_scene(
        entities: &[RuntimeEntity],
        world: &World,
    ) -> Result<Self, Vec<InstanceDiagnostic>> {
        prepare(entities, RuntimeEntityNamespace::Source, None, None, world)
    }

    pub(crate) fn prepare_scene_instance(
        entities: &[RuntimeEntity],
        world: &World,
    ) -> Result<Self, Vec<InstanceDiagnostic>> {
        Self::prepare_scene(entities, world)
    }

    pub(crate) fn prepare_prefab_instance(
        entities: &[RuntimeEntity],
        instance_id: RuntimeInstanceId,
        declared_root: Option<&str>,
        external_parent: Option<SourceEntityId>,
        world: &World,
    ) -> Result<Self, Vec<InstanceDiagnostic>> {
        prepare(
            entities,
            RuntimeEntityNamespace::Instance(instance_id),
            declared_root,
            external_parent,
            world,
        )
    }

    pub(crate) fn commit(self, world: &mut World) -> PreparedRuntimeCommit {
        let mut source_to_runtime = BTreeMap::new();
        for entity in self.entities {
            let runtime_id = world.commit_prepared_entity(
                entity.world_id,
                entity.name,
                entity.kind,
                entity.enabled,
                entity.hierarchy,
                entity.components,
            );
            source_to_runtime.insert(entity.source_id, runtime_id);
        }
        PreparedRuntimeCommit { source_to_runtime }
    }
}

fn prepare(
    entities: &[RuntimeEntity],
    namespace: RuntimeEntityNamespace,
    declared_root: Option<&str>,
    external_parent: Option<SourceEntityId>,
    world: &World,
) -> Result<PreparedRuntimeEntities, Vec<InstanceDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut source_to_world = BTreeMap::new();
    let mut seen = BTreeSet::new();

    for entity in entities {
        let source_id = SourceEntityId::from(entity.id.clone());
        if entity.id.trim().is_empty() {
            diagnostics.push(diagnostic(
                "world.entity.invalid_id",
                "entity id must not be empty",
                Some(source_id),
                "Assign a stable non-empty entity id.",
            ));
            continue;
        }
        if !seen.insert(source_id.clone()) {
            diagnostics.push(diagnostic(
                "world.entity.duplicate_id",
                format!("duplicate source entity id: {source_id}"),
                Some(source_id),
                "Use a unique entity id in the Scene or Prefab.",
            ));
            continue;
        }
        let world_id = world_id_for(&source_id, namespace);
        if world.runtime_id_for_source(&world_id).is_some() {
            diagnostics.push(diagnostic(
                "world.entity.duplicate_id",
                format!("world entity already exists: {world_id}"),
                Some(source_id.clone()),
                "Unload the previous instance or use a unique instance namespace.",
            ));
        }
        source_to_world.insert(source_id, world_id);
    }

    let parent_by_source = entities
        .iter()
        .map(|entity| {
            (
                SourceEntityId::from(entity.id.clone()),
                entity.parent_id.clone().map(SourceEntityId::from),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (source_id, parent) in &parent_by_source {
        let Some(parent) = parent else {
            continue;
        };
        if parent == source_id {
            diagnostics.push(diagnostic(
                "world.parent.self",
                format!("entity cannot be its own parent: {source_id}"),
                Some(source_id.clone()),
                "Choose a different parent or no parent.",
            ));
        } else if !source_to_world.contains_key(parent) {
            diagnostics.push(diagnostic(
                "world.parent.missing",
                format!("parent entity does not exist: {parent}"),
                Some(source_id.clone()),
                "Fix parent_id to reference an entity in the same Scene or Prefab.",
            ));
        }
    }
    validate_parent_cycles(&parent_by_source, &mut diagnostics);

    let roots = entities
        .iter()
        .filter(|entity| entity.parent_id.is_none())
        .map(|entity| SourceEntityId::from(entity.id.clone()))
        .collect::<Vec<_>>();
    let prefab_root = match namespace {
        RuntimeEntityNamespace::Source => None,
        RuntimeEntityNamespace::Instance(_) => {
            let root = declared_root.map(SourceEntityId::from).or_else(|| {
                if roots.len() == 1 {
                    roots.first().cloned()
                } else {
                    None
                }
            });
            if roots.len() != 1 || root.as_ref().is_none_or(|id| !roots.contains(id)) {
                diagnostics.push(diagnostic(
                    "world.prefab.invalid_root",
                    "prefab must declare exactly one root entity",
                    root.clone(),
                    "Set root_entity_id to the Prefab's single root entity.",
                ));
            }
            root
        }
    };

    if let Some(parent) = external_parent.as_ref() {
        if world.runtime_id_for_source(parent).is_none() {
            diagnostics.push(diagnostic(
                "world.parent.missing",
                format!("destination parent entity does not exist: {parent}"),
                prefab_root.clone(),
                "Instantiate under an alive entity in the current World.",
            ));
        }
    }

    let mut prepared = Vec::new();
    let mut remapped_reference_count = 0;
    for entity in entities {
        let source_id = SourceEntityId::from(entity.id.clone());
        let Some(world_id) = source_to_world.get(&source_id).cloned() else {
            continue;
        };
        let Some(transform) = entity.transform.clone() else {
            diagnostics.push(diagnostic(
                "world.component.missing",
                format!("Transform is required for entity: {source_id}"),
                Some(source_id),
                "Cook every runtime Scene/Prefab entity with a Transform.",
            ));
            continue;
        };

        let mut component_types = BTreeSet::new();
        let mut components = vec![ComponentValue::Transform(convert_transform(transform))];
        component_types.insert(ComponentTypeId::transform());
        if let Some(mesh) = &entity.mesh {
            components.push(ComponentValue::Renderable(convert_renderable(mesh)));
            component_types.insert(ComponentTypeId::renderable());
        }
        if let Some(sprite) = &entity.sprite_renderer2d {
            components.push(ComponentValue::SpriteRenderer2D(convert_sprite(sprite)));
            component_types.insert(ComponentTypeId::sprite_renderer2d());
        }
        if let Some(animator) = &entity.animator2d {
            if entity.sprite_renderer2d.is_none() {
                diagnostics.push(diagnostic(
                    "world.animator2d.missing_sprite_renderer2d",
                    format!("Animator2D requires SpriteRenderer2D on entity: {source_id}"),
                    Some(source_id.clone()),
                    "Add a typed SpriteRenderer2D component to the same entity.",
                ));
            } else {
                components.push(ComponentValue::Animator2D(animator.clone()));
                component_types.insert(ComponentTypeId::animator2d());
            }
        }

        for component in &entity.components {
            let component_type = ComponentTypeId::from(component.component_type.clone());
            if !component_types.insert(component_type.clone()) {
                diagnostics.push(diagnostic(
                    "world.component.duplicate_type",
                    format!(
                        "entity '{source_id}' contains duplicate component type '{component_type}'"
                    ),
                    Some(source_id.clone()),
                    "Keep only one component value for each component type.",
                ));
                continue;
            }
            if component_type == ComponentTypeId::entity_meta()
                || component_type == ComponentTypeId::hierarchy()
                || component_type == ComponentTypeId::transform()
                || component_type == ComponentTypeId::renderable()
                || component_type == ComponentTypeId::sprite_renderer2d()
                || component_type == ComponentTypeId::animator2d()
            {
                diagnostics.push(diagnostic(
                    "world.component.type_mismatch",
                    format!("reserved engine component cannot be supplied dynamically: {component_type}"),
                    Some(source_id.clone()),
                    "Use the typed RuntimeEntity field for this engine component.",
                ));
                continue;
            }
            if component_type == ComponentTypeId::collider2d() {
                match decode_collider2d(&component.data) {
                    Ok(collider) => components.push(ComponentValue::Collider2D(collider)),
                    Err(message) => diagnostics.push(diagnostic(
                        "world.component.decode_failed",
                        format!("Collider2D decode failed for '{source_id}': {message}"),
                        Some(source_id.clone()),
                        "Fix engine.collider2d shape/radius/halfExtents fields.",
                    )),
                }
                continue;
            }
            match remap_json_value(
                component.data.clone(),
                &source_to_world,
                &mut remapped_reference_count,
            ) {
                Ok(value) => components.push(ComponentValue::Dynamic {
                    component_type,
                    value: runtime_value_from_json(value),
                }),
                Err(missing) => diagnostics.push(diagnostic(
                    "world.entity_ref.missing_target",
                    format!("entityRef target does not exist: {missing}"),
                    Some(source_id.clone()),
                    "Fix entityRef to target an entity in the same Scene or Prefab.",
                )),
            }
        }

        let parent_id = if prefab_root.as_ref() == Some(&source_id) {
            external_parent.clone()
        } else {
            entity.parent_id.as_ref().and_then(|parent| {
                source_to_world
                    .get(&SourceEntityId::from(parent.clone()))
                    .cloned()
            })
        };
        prepared.push(PreparedRuntimeEntity {
            source_id,
            world_id,
            name: entity.name.clone(),
            kind: entity.kind.clone(),
            enabled: entity.enabled,
            hierarchy: Hierarchy {
                parent_id,
                sibling_order: entity.sibling_order,
            },
            components,
        });
    }

    if external_parent.is_some() && prefab_root.is_some() {
        remapped_reference_count += 1;
    }

    if diagnostics.is_empty() {
        Ok(PreparedRuntimeEntities {
            entities: prepared,
            source_to_world,
            root_sources: roots,
            remapped_reference_count,
        })
    } else {
        Err(diagnostics)
    }
}

fn validate_parent_cycles(
    parent_by_source: &BTreeMap<SourceEntityId, Option<SourceEntityId>>,
    diagnostics: &mut Vec<InstanceDiagnostic>,
) {
    for source in parent_by_source.keys() {
        let mut visited = BTreeSet::new();
        let mut current = Some(source.clone());
        while let Some(candidate) = current {
            if !visited.insert(candidate.clone()) {
                diagnostics.push(diagnostic(
                    "world.parent.cycle",
                    format!("hierarchy cycle contains entity: {candidate}"),
                    Some(source.clone()),
                    "Break the parent cycle in the Scene or Prefab.",
                ));
                break;
            }
            current = parent_by_source.get(&candidate).cloned().flatten();
        }
    }
}

fn world_id_for(source_id: &SourceEntityId, namespace: RuntimeEntityNamespace) -> SourceEntityId {
    match namespace {
        RuntimeEntityNamespace::Source => source_id.clone(),
        RuntimeEntityNamespace::Instance(instance_id) => {
            SourceEntityId::from(format!("instance:{}:{}", instance_id.0, source_id))
        }
    }
}

fn diagnostic(
    kind: impl Into<String>,
    message: impl Into<String>,
    source_entity_id: Option<SourceEntityId>,
    suggested_fix: impl Into<String>,
) -> InstanceDiagnostic {
    let mut diagnostic = InstanceDiagnostic::error(kind, message, InstanceStage::PrepareEntities)
        .with_suggested_fix(suggested_fix);
    if let Some(source_entity_id) = source_entity_id {
        diagnostic = diagnostic.with_source_entity_id(source_entity_id);
    }
    diagnostic
}

fn convert_transform(transform: RuntimeTransform) -> Transform {
    Transform {
        local_position: Vec3::from(transform.local_position),
        local_rotation: Vec3::from(transform.local_rotation),
        local_scale: Vec3::from(transform.local_scale),
    }
}

fn convert_renderable(mesh: &RuntimeMesh) -> Renderable {
    Renderable {
        mesh_ref: mesh.asset_ref.as_ref().map(|value| value.id.clone()),
        material_ref: mesh.material_ref.as_ref().map(|value| value.id.clone()),
        visible: mesh.visible,
        layer: mesh.layer.clone(),
    }
}

fn convert_sprite(sprite: &RuntimeSpriteRenderer2D) -> SpriteRenderer2D {
    SpriteRenderer2D {
        sprite_ref: sprite.sprite_ref.as_ref().map(|value| value.id.clone()),
        material_ref: sprite.material_ref.as_ref().map(|value| value.id.clone()),
        color: sprite.color.unwrap_or([1.0, 1.0, 1.0, 1.0]),
        flip_x: sprite.flip_x.unwrap_or(false),
        flip_y: sprite.flip_y.unwrap_or(false),
        sorting_layer: sprite.sorting_layer.unwrap_or(0),
        order_in_layer: sprite.order_in_layer.unwrap_or(0),
        sort_z: sprite.sort_z.unwrap_or(0.0),
        visible: sprite.visible.unwrap_or(true),
    }
}

fn remap_json_value(
    value: serde_json::Value,
    source_to_world: &BTreeMap<SourceEntityId, SourceEntityId>,
    remapped_count: &mut usize,
) -> Result<serde_json::Value, SourceEntityId> {
    match value {
        serde_json::Value::Object(mut object) => {
            if let Some(serde_json::Value::String(source)) = object.get("entityRef") {
                let source_id = SourceEntityId::from(source.clone());
                let Some(world_id) = source_to_world.get(&source_id) else {
                    return Err(source_id);
                };
                object.insert(
                    "entityRef".to_string(),
                    serde_json::Value::String(world_id.to_string()),
                );
                *remapped_count += 1;
            }
            let mut remapped = serde_json::Map::new();
            for (key, value) in object {
                remapped.insert(
                    key,
                    remap_json_value(value, source_to_world, remapped_count)?,
                );
            }
            Ok(serde_json::Value::Object(remapped))
        }
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| remap_json_value(value, source_to_world, remapped_count))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        value => Ok(value),
    }
}

fn runtime_value_from_json(value: serde_json::Value) -> RuntimeValue {
    match value {
        serde_json::Value::Null => RuntimeValue::Null,
        serde_json::Value::Bool(value) => RuntimeValue::Bool(value),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(RuntimeValue::I64)
            .or_else(|| {
                value
                    .as_u64()
                    .and_then(|number| i64::try_from(number).ok().map(RuntimeValue::I64))
            })
            .unwrap_or_else(|| RuntimeValue::F64(value.as_f64().unwrap_or_default())),
        serde_json::Value::String(value) => RuntimeValue::String(value),
        serde_json::Value::Array(values) => {
            RuntimeValue::Array(values.into_iter().map(runtime_value_from_json).collect())
        }
        serde_json::Value::Object(object) => RuntimeValue::Object(
            object
                .into_iter()
                .map(|(key, value)| {
                    let value = if key == "entityRef" {
                        value
                            .as_str()
                            .map(|id| RuntimeValue::EntityRef(SourceEntityId::from(id.to_string())))
                            .unwrap_or_else(|| runtime_value_from_json(value))
                    } else {
                        runtime_value_from_json(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
    }
}

fn decode_collider2d(value: &serde_json::Value) -> Result<Collider2D, &'static str> {
    let fields = value
        .as_object()
        .ok_or("component payload must be an object")?;
    let shape_name = fields
        .get("shape")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("aabb")
        .to_ascii_lowercase();
    let shape = match shape_name.as_str() {
        "circle" => Shape2D::Circle {
            radius: optional_f32(fields.get("radius"), 0.5, "radius must be a number")?,
        },
        "aabb" => {
            let half_extents = fields.get("halfExtents");
            Shape2D::Aabb {
                half_extents: Vec2 {
                    x: nested_optional_f32(half_extents, "x", 0.5)?,
                    y: nested_optional_f32(half_extents, "y", 0.5)?,
                },
            }
        }
        _ => return Err("shape must be 'circle' or 'aabb'"),
    };
    let offset = fields.get("offset");
    Ok(Collider2D {
        shape,
        offset: Vec2 {
            x: nested_optional_f32(offset, "x", 0.0)?,
            y: nested_optional_f32(offset, "y", 0.0)?,
        },
        layer: fields
            .get("layer")
            .and_then(serde_json::Value::as_u64)
            .map(|value| PhysicsLayer(value as u32))
            .unwrap_or(PhysicsLayer::DEFAULT),
        mask: fields
            .get("mask")
            .and_then(serde_json::Value::as_u64)
            .map(|value| PhysicsMask(value as u32))
            .unwrap_or(PhysicsMask::ALL),
        enabled: fields
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        is_sensor: fields
            .get("isSensor")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

fn optional_f32(
    value: Option<&serde_json::Value>,
    default: f32,
    error: &'static str,
) -> Result<f32, &'static str> {
    value
        .map(|value| value.as_f64().map(|value| value as f32).ok_or(error))
        .unwrap_or(Ok(default))
}

fn nested_optional_f32(
    value: Option<&serde_json::Value>,
    key: &str,
    default: f32,
) -> Result<f32, &'static str> {
    let Some(value) = value else {
        return Ok(default);
    };
    let object = value.as_object().ok_or("vector value must be an object")?;
    optional_f32(object.get(key), default, "vector field must be a number")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_package::{RuntimeProjectComponent, RuntimeSpriteRenderer2D, Vector3};

    fn entity(id: &str, parent: Option<&str>) -> RuntimeEntity {
        RuntimeEntity {
            schema_version: "runtime-entity.v1".to_string(),
            id: id.to_string(),
            name: id.to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: parent.map(str::to_string),
            sibling_order: 0,
            transform: Some(RuntimeTransform {
                local_position: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                local_rotation: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                local_scale: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }),
            mesh: None,
            sprite_renderer2d: None,
            animator2d: None,
            components: Vec::new(),
        }
    }

    #[test]
    fn prepare_rejects_duplicate_before_world_mutation() {
        let world = World::new();
        let error = PreparedRuntimeEntities::prepare_scene(
            &[entity("same", None), entity("same", None)],
            &world,
        )
        .expect_err("duplicate must fail");
        assert!(error
            .iter()
            .any(|diagnostic| diagnostic.kind == "world.entity.duplicate_id"));
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn animator2d_hydration_requires_renderer_and_commits_without_advancing_state() {
        let world = World::new();
        let mut missing_renderer = entity("animated", None);
        missing_renderer.animator2d = Some(crate::animator2d::RuntimeAnimator2D {
            controller_id: "controller-main".to_string(),
            controller_index: 0,
            registry_digest: "sha256:registry".to_string(),
            enabled: true,
            initial_bools: BTreeMap::from([("moving".to_string(), true)]),
        });
        let failure = PreparedRuntimeEntities::prepare_scene(&[missing_renderer.clone()], &world)
            .expect_err("Animator2D without SpriteRenderer2D must fail");
        assert!(failure
            .iter()
            .any(|diagnostic| { diagnostic.kind == "world.animator2d.missing_sprite_renderer2d" }));

        missing_renderer.sprite_renderer2d = Some(RuntimeSpriteRenderer2D {
            sprite_ref: None,
            material_ref: None,
            color: None,
            flip_x: None,
            flip_y: None,
            sorting_layer: None,
            order_in_layer: None,
            sort_z: None,
            visible: None,
        });
        let mut world = World::new();
        PreparedRuntimeEntities::prepare_scene(&[missing_renderer], &world)
            .expect("valid Animator2D entity")
            .commit(&mut world);
        let animator = world
            .animator2d(&SourceEntityId::from("animated"))
            .expect("typed Animator2D component");
        assert_eq!(animator.controller_id, "controller-main");
        assert_eq!(animator.initial_bools.get("moving"), Some(&true));
    }

    #[test]
    fn prepare_rejects_missing_parent_and_cycle() {
        let world = World::new();
        let missing =
            PreparedRuntimeEntities::prepare_scene(&[entity("child", Some("missing"))], &world)
                .expect_err("missing parent must fail");
        assert!(missing
            .iter()
            .any(|diagnostic| diagnostic.kind == "world.parent.missing"));

        let cycle = PreparedRuntimeEntities::prepare_scene(
            &[entity("a", Some("b")), entity("b", Some("a"))],
            &world,
        )
        .expect_err("cycle must fail");
        assert!(cycle
            .iter()
            .any(|diagnostic| diagnostic.kind == "world.parent.cycle"));
    }

    #[test]
    fn prepare_rejects_missing_entity_ref_and_invalid_collider() {
        let world = World::new();
        let mut with_ref = entity("a", None);
        with_ref.components.push(RuntimeProjectComponent {
            component_type: "project.link".to_string(),
            data: serde_json::json!({"target": {"entityRef": "missing"}}),
        });
        let missing_ref = PreparedRuntimeEntities::prepare_scene(&[with_ref], &world)
            .expect_err("missing ref must fail");
        assert!(missing_ref
            .iter()
            .any(|diagnostic| diagnostic.kind == "world.entity_ref.missing_target"));

        let mut invalid_collider = entity("a", None);
        invalid_collider.components.push(RuntimeProjectComponent {
            component_type: "engine.collider2d".to_string(),
            data: serde_json::json!({"shape": "triangle"}),
        });
        let decode = PreparedRuntimeEntities::prepare_scene(&[invalid_collider], &world)
            .expect_err("invalid collider must fail");
        assert!(decode
            .iter()
            .any(|diagnostic| diagnostic.kind == "world.component.decode_failed"));
    }

    #[test]
    fn prepared_commit_hydrates_parent_and_entity_ref() {
        let mut world = World::new();
        let mut child = entity("child", Some("root"));
        child.components.push(RuntimeProjectComponent {
            component_type: "project.link".to_string(),
            data: serde_json::json!({"target": {"entityRef": "root"}}),
        });
        let prepared =
            PreparedRuntimeEntities::prepare_scene(&[entity("root", None), child], &world)
                .expect("valid scene should prepare");
        assert_eq!(prepared.remapped_reference_count, 1);
        prepared.commit(&mut world);
        assert_eq!(world.entity_count(), 2);
        assert_eq!(
            world
                .hierarchy(&SourceEntityId::from("child"))
                .unwrap()
                .parent_id,
            Some(SourceEntityId::from("root"))
        );
    }
}
