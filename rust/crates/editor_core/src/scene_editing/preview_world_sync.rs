use engine_runtime::archetype::ComponentValue;
use engine_runtime::components::{ComponentTypeId, Hierarchy, Renderable, Transform};
use engine_runtime::ids::EntityId;
use engine_runtime::math::{Vec2, Vec3};
use engine_runtime::physics2d::{Collider2D, PhysicsLayer, PhysicsMask, Shape2D};
use engine_runtime::world::World;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use super::{
    EditorSceneComponent, EditorSceneDocument, EditorVec3, SceneEditDiagnostic,
    SceneEditDiagnosticSeverity, PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION,
};
pub struct PreviewWorldSync;

impl PreviewWorldSync {
    pub fn full_rebuild(
        document: &EditorSceneDocument,
    ) -> Result<(World, PreviewWorldSyncReport), PreviewWorldSyncReport> {
        let mut diagnostics = document.validate();
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SceneEditDiagnosticSeverity::Error)
        {
            return Err(PreviewWorldSyncReport {
                schema_version: PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION.to_string(),
                scene_id: document.scene_id.clone(),
                sync_mode: "full_rebuild".to_string(),
                entity_count: 0,
                component_count: 0,
                diagnostics,
            });
        }

        let mut world = World::new();
        let mut component_count = 0;
        for entity in &document.entities {
            let transform = entity.transform.map(|value| Transform {
                local_position: to_runtime_vec3(value.local_position),
                local_rotation: to_runtime_vec3(value.local_rotation),
                local_scale: to_runtime_vec3(value.local_scale),
            });
            if transform.is_some() {
                component_count += 1;
            }
            let renderable = entity.mesh.as_ref().map(|mesh| Renderable {
                mesh_ref: mesh
                    .asset_ref
                    .as_ref()
                    .map(|asset_ref| asset_ref.asset_id.clone()),
                material_ref: mesh
                    .material_ref
                    .as_ref()
                    .map(|asset_ref| asset_ref.asset_id.clone()),
                visible: mesh.visible,
                layer: mesh.layer.clone(),
            });
            if renderable.is_some() {
                component_count += 1;
            }
            component_count += entity.components.len();
            if let Err(error) = world.try_spawn_with_components(
                EntityId::new(entity.entity_id.clone()),
                entity.name.clone(),
                entity.kind.clone(),
                entity.enabled,
                Hierarchy {
                    parent_id: entity.parent_id.clone().map(EntityId::new),
                    sibling_order: entity.sibling_order,
                },
                transform,
                renderable,
            ) {
                diagnostics.push(world_mutation_diagnostic(error));
                return Err(failed_preview_report(document, diagnostics));
            }
            for component in &entity.components {
                let entity_id = EntityId::new(entity.entity_id.clone());
                if let Some(collider) = editor_component_to_collider2d(component) {
                    let value = ComponentValue::Collider2D(collider);
                    if let Err(error) = world.try_insert_component_value(
                        entity_id,
                        ComponentTypeId::collider2d(),
                        value,
                    ) {
                        diagnostics.push(world_mutation_diagnostic(error));
                        return Err(failed_preview_report(document, diagnostics));
                    }
                } else {
                    if let Err(error) = world.try_insert_dynamic_component(
                        entity_id,
                        ComponentTypeId::new(component.component_type.clone()),
                        component.fields.to_string(),
                    ) {
                        diagnostics.push(world_mutation_diagnostic(error));
                        return Err(failed_preview_report(document, diagnostics));
                    }
                }
            }
        }
        diagnostics.push(SceneEditDiagnostic::info(
            "scene.preview_world.full_rebuild",
            "scene.preview_world",
            format!(
                "PreviewWorld full rebuild completed: entities={}",
                document.entities.len()
            ),
        ));
        let report = PreviewWorldSyncReport {
            schema_version: PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION.to_string(),
            scene_id: document.scene_id.clone(),
            sync_mode: "full_rebuild".to_string(),
            entity_count: document.entities.len(),
            component_count,
            diagnostics,
        };
        Ok((world, report))
    }
}

fn world_mutation_diagnostic(
    error: engine_runtime::world::WorldMutationError,
) -> SceneEditDiagnostic {
    let entity_id = error.source_entity_id.as_ref().map(ToString::to_string);
    let mut diagnostic = SceneEditDiagnostic::error(
        error.code,
        "scene.preview_world",
        match error.suggested_fix {
            Some(next_action) => format!("{} Next action: {next_action}", error.message),
            None => error.message,
        },
    );
    if let Some(entity_id) = entity_id {
        diagnostic = diagnostic.with_entity_id(entity_id);
    }
    diagnostic
}

fn failed_preview_report(
    document: &EditorSceneDocument,
    diagnostics: Vec<SceneEditDiagnostic>,
) -> PreviewWorldSyncReport {
    PreviewWorldSyncReport {
        schema_version: PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION.to_string(),
        scene_id: document.scene_id.clone(),
        sync_mode: "full_rebuild".to_string(),
        entity_count: 0,
        component_count: 0,
        diagnostics,
    }
}

fn editor_component_to_collider2d(component: &EditorSceneComponent) -> Option<Collider2D> {
    if component.component_type != ComponentTypeId::collider2d().as_str() {
        return None;
    }
    let fields = component.fields.as_object()?;
    let shape_name = fields
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("aabb")
        .to_ascii_lowercase();
    let shape = match shape_name.as_str() {
        "circle" => Shape2D::Circle {
            radius: fields.get("radius").and_then(Value::as_f64).unwrap_or(0.5) as f32,
        },
        _ => {
            let half_extents = fields.get("halfExtents");
            Shape2D::Aabb {
                half_extents: Vec2 {
                    x: half_extents
                        .and_then(|value| value.get("x"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.5) as f32,
                    y: half_extents
                        .and_then(|value| value.get("y"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.5) as f32,
                },
            }
        }
    };
    let offset = fields.get("offset");
    Some(Collider2D {
        shape,
        offset: Vec2 {
            x: offset
                .and_then(|value| value.get("x"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            y: offset
                .and_then(|value| value.get("y"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
        },
        layer: fields
            .get("layer")
            .and_then(Value::as_u64)
            .map(|value| PhysicsLayer(value as u32))
            .unwrap_or(PhysicsLayer::DEFAULT),
        mask: fields
            .get("mask")
            .and_then(Value::as_u64)
            .map(|value| PhysicsMask(value as u32))
            .unwrap_or(PhysicsMask::ALL),
        enabled: fields
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        is_sensor: fields
            .get("isSensor")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}


fn to_runtime_vec3(value: EditorVec3) -> Vec3 {
    Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}
