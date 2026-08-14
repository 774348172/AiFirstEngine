use crate::diagnostics::{RuntimeDiagnostic, RuntimeDiagnostics, RuntimeLoadResult};
use crate::runtime_entity_hydration::PreparedRuntimeEntities;
use crate::runtime_package::RuntimeScene;
use crate::world::World;

pub fn load_scene_into_world(scene: &RuntimeScene) -> RuntimeLoadResult<World> {
    let mut world = World::new();
    match PreparedRuntimeEntities::prepare_scene(&scene.entities, &world) {
        Ok(prepared) => {
            prepared.commit(&mut world);
            RuntimeLoadResult::ok(world, RuntimeDiagnostics::new())
        }
        Err(issues) => {
            let mut diagnostics = RuntimeDiagnostics::new();
            for issue in issues {
                let path = issue
                    .source_entity_id
                    .as_ref()
                    .map(|source| format!("scene.entity.{source}"))
                    .unwrap_or_else(|| "scene.entities".to_string());
                let mut diagnostic = RuntimeDiagnostic::error(path, issue.message)
                    .with_code(issue.kind)
                    .with_stage(issue.stage.as_str());
                if let Some(next_action) = issue.suggested_fix {
                    diagnostic = diagnostic.with_next_action(next_action);
                }
                diagnostics.push(diagnostic);
            }
            RuntimeLoadResult::failed(diagnostics)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ComponentTypeId;
    use crate::ids::EntityId;
    use crate::runtime_package::RuntimeProjectComponent;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use tests_support::scene_fixture;

    #[test]
    fn scene_loader_creates_entity_and_transform() {
        let scene = scene_fixture(false);
        let result = load_scene_into_world(&scene);
        assert!(
            result.diagnostics.is_ok(),
            "{:?}",
            result.diagnostics.issues
        );
        let world = result.value.expect("world should load");
        let entity_id = EntityId::from("entity-player");
        assert_eq!(world.entity_count(), 1);
        assert_eq!(world.transform(&entity_id).unwrap().local_position.y, 1.0);
    }

    #[test]
    fn world_queries_alive_renderables() {
        let scene = scene_fixture(true);
        let result = load_scene_into_world(&scene);
        let world = result.value.expect("world should load");
        let renderables = world.alive_renderables();
        assert_eq!(renderables.len(), 1);
        assert_eq!(renderables[0].0.as_str(), "entity-player");
        assert_eq!(renderables[0].2.mesh_ref.as_deref(), Some("model-player"));
    }

    #[test]
    fn duplicate_ids_at_any_position_fail_without_unwind_or_world() {
        for duplicate_index in 0..3 {
            let mut scene = scene_fixture(false);
            let template = scene.entities[0].clone();
            scene.entities = (0..3)
                .map(|index| {
                    let mut entity = template.clone();
                    entity.id = if index == duplicate_index || index == (duplicate_index + 1) % 3 {
                        "duplicate".to_string()
                    } else {
                        format!("unique-{index}")
                    };
                    entity
                })
                .collect();

            let result = catch_unwind(AssertUnwindSafe(|| load_scene_into_world(&scene)))
                .expect("invalid scene must not unwind");
            assert!(result.value.is_none());
            assert!(result
                .diagnostics
                .issues
                .iter()
                .any(|issue| issue.code == "world.entity.duplicate_id"));
        }
    }

    #[test]
    fn invalid_hierarchy_and_missing_transform_fail_closed() {
        let mut missing_parent = scene_fixture(false);
        missing_parent.entities[0].parent_id = Some("missing".to_string());
        assert_failed_with_code(missing_parent, "world.parent.missing");

        let mut cycle = scene_fixture(false);
        let mut second = cycle.entities[0].clone();
        cycle.entities[0].id = "first".to_string();
        cycle.entities[0].parent_id = Some("second".to_string());
        second.id = "second".to_string();
        second.parent_id = Some("first".to_string());
        cycle.entities.push(second);
        assert_failed_with_code(cycle, "world.parent.cycle");

        let mut missing_transform = scene_fixture(false);
        missing_transform.entities[0].transform = None;
        assert_failed_with_code(missing_transform, "world.component.missing");
    }

    #[test]
    fn invalid_component_and_entity_ref_fail_closed() {
        let mut invalid_collider = scene_fixture(false);
        invalid_collider.entities[0]
            .components
            .push(RuntimeProjectComponent {
                component_type: ComponentTypeId::collider2d().to_string(),
                data: serde_json::json!({ "shape": "triangle" }),
            });
        assert_failed_with_code(invalid_collider, "world.component.decode_failed");

        let mut missing_ref = scene_fixture(false);
        missing_ref.entities[0]
            .components
            .push(RuntimeProjectComponent {
                component_type: "project.target".to_string(),
                data: serde_json::json!({ "entityRef": "missing" }),
            });
        assert_failed_with_code(missing_ref, "world.entity_ref.missing_target");
    }

    fn assert_failed_with_code(scene: RuntimeScene, expected_code: &str) {
        let result = catch_unwind(AssertUnwindSafe(|| load_scene_into_world(&scene)))
            .expect("invalid scene must not unwind");
        assert!(result.value.is_none());
        let issue = result
            .diagnostics
            .issues
            .iter()
            .find(|issue| issue.code == expected_code)
            .unwrap_or_else(|| panic!("missing diagnostic {expected_code:?}"));
        assert_eq!(issue.stage.as_deref(), Some("PrepareEntities"));
        assert!(issue.next_action.is_some());
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use crate::runtime_package::{
        RuntimeAssetRef, RuntimeEntity, RuntimeMesh, RuntimeScene, RuntimeTransform, Vector3,
    };

    pub(crate) fn renderable_scene_fixture() -> RuntimeScene {
        scene_fixture(true)
    }

    pub(crate) fn scene_fixture(with_mesh: bool) -> RuntimeScene {
        RuntimeScene {
            schema_version: "runtime-scene.v1".to_string(),
            id: "scene-main".to_string(),
            name: "Main".to_string(),
            gravity: 0.0,
            background: "#000".to_string(),
            sky_color: "#111".to_string(),
            entities: vec![RuntimeEntity {
                schema_version: "runtime-entity.v1".to_string(),
                id: "entity-player".to_string(),
                name: "Player".to_string(),
                kind: "player".to_string(),
                enabled: true,
                parent_id: None,
                sibling_order: 0,
                transform: Some(RuntimeTransform {
                    local_position: Vector3 {
                        x: 0.0,
                        y: 1.0,
                        z: 2.0,
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
                mesh: with_mesh.then(|| RuntimeMesh {
                    primitive: Some("model".to_string()),
                    color: None,
                    label: None,
                    asset_ref: Some(RuntimeAssetRef {
                        id: "model-player".to_string(),
                        asset_type: "model".to_string(),
                        guid: None,
                        sub_asset: None,
                    }),
                    material_ref: None,
                    texture_ref: None,
                    visible: true,
                    layer: "default".to_string(),
                    metalness: None,
                    roughness: None,
                }),
                sprite_renderer2d: None,
                animator2d: None,
                components: Vec::new(),
            }],
        }
    }
}
