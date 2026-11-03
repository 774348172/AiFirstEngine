use super::s3c::{body, command, entity, motor, world};
use engine_runtime::{
    ids::EntityId, navigation_3d::*, physics_3d::PhysicsModule,
    runtime_instance_loader::RuntimeInstanceLoader, runtime_package::*, runtime_package_builder::*,
    world::World,
};
use project_runtime_sdk::ProjectRuntimeSpatialIntent as I;
use serde_json::json;
pub(super) fn profile() -> AgentProfile3D {
    AgentProfile3D {
        key: "humanoid".into(),
        radius: 0.3,
        height: 1.8,
        climb: 0.3,
        max_slope_degrees: 45.,
        cell_size: 0.2,
        cell_height: 0.1,
        tile_width: 16.,
    }
}
pub(super) fn asset() -> CookedNavigation3D {
    asset_with_profile(profile())
}
fn asset_with_profile(profile: AgentProfile3D) -> CookedNavigation3D {
    let mut tiles = Vec::new();
    for x in [0, 1] {
        let config = engine_navigation_native::BakeConfig {
            cell_size: profile.cell_size,
            cell_height: profile.cell_height,
            radius: profile.radius,
            height: profile.height,
            climb: profile.climb,
            slope: profile.max_slope_degrees,
            width: 16.,
            min: [x as f32 * 16., -1., 0.],
            max: [(x + 1) as f32 * 16., 4., 16.],
            x,
            z: 0,
        };
        let data = engine_navigation_native::bake(
            &config,
            &[
                [-2., 0., -2.],
                [-2., 0., 18.],
                [34., 0., 18.],
                [34., 0., -2.],
            ],
            &[[0, 1, 2], [0, 2, 3]],
            &[],
        )
        .unwrap();
        tiles.push(CookedNavTile3D { x, z: 0, data });
    }
    CookedNavigation3D {
        schema_version: "navigation3d.v1".into(),
        asset_id: "navigation".into(),
        source_digest: format!("sha256:{}", "a".repeat(64)),
        recipe: NAV_RECIPE.into(),
        profile,
        tiles,
    }
}
pub(super) fn binding() -> serde_json::Value {
    json!({"componentType":"engine.navigation3d","data":{"schemaVersion":"navigation3d-binding.v1","navigation":{"id":"navigation","type":"navigation3d"}}})
}
pub(super) fn agent() -> serde_json::Value {
    json!({"componentType":"engine.navigation_agent3d","data":{"schemaVersion":"navigation3d-agent.v1","profile":"humanoid","speed":4,"arrivalDistance":0.1,"avoidanceRadius":1,"maxNeighbours":8}})
}
pub(super) struct Fixture {
    pub world: World,
    pub loader: RuntimeInstanceLoader,
    pub navigation: NavigationModule,
    pub physics: PhysicsModule,
    pub anchor: f64,
    pub tick: u64,
}
impl Fixture {
    pub fn new(anchor: f64) -> Self {
        Self::with_asset(anchor, asset())
    }
    fn with_asset(anchor: f64, asset: CookedNavigation3D) -> Self {
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "nav-s3d", "Nav S3D", "1",
        ));
        let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        input.assets.push(
            RuntimePackageSourceAsset::new(
                "navigation",
                "navigation",
                "navigation3d",
                "Assets/nav.json",
                "cooked/nav.asset",
            )
            .with_runtime_payload(serde_json::to_vec(&asset).unwrap()),
        );
        let mut other_profile = profile();
        other_profile.radius = 0.6;
        let mut incompatible = asset_with_profile(other_profile);
        incompatible.asset_id = "navigation-incompatible".into();
        input.assets.push(
            RuntimePackageSourceAsset::new(
                "navigation-incompatible",
                "Other profile",
                "navigation3d",
                "Assets/nav-other.json",
                "cooked/nav-other.asset",
            )
            .with_runtime_payload(serde_json::to_vec(&incompatible).unwrap()),
        );
        let empty = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[]});
        let engine_runtime::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
            engine_runtime::spatial_contract::decode_runtime_scene(
                &serde_json::to_vec(&empty).unwrap(),
            )
            .unwrap()
        else {
            unreachable!()
        };
        input.scenes.push(scene.into());
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../.artifacts/371/s3-simulation-20261026-084317")
            .join(format!(
                "s3d-nav-package-{}-{anchor}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&output, "main"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        let package = load_runtime_package(&output).value.unwrap();
        let loader = RuntimeInstanceLoader::from_package(&package);
        let mut world = world(vec![
            entity("nav", [0.; 3], vec![binding()]),
            entity(
                "floor",
                [16., -0.5, 8.],
                vec![body(
                    "static",
                    json!({"kind":"box","halfExtents":[20,0.5,12]}),
                )],
            ),
            entity("actor", [2., 0.9, 8.], vec![motor(), agent()]),
        ]);
        for id in ["nav", "floor", "actor"] {
            command(
                &mut world,
                id,
                I::SetWorldAnchor {
                    anchor: [anchor, 0., 0.],
                    preserve_world: false,
                },
            );
        }
        Self {
            world,
            loader,
            navigation: NavigationModule::default(),
            physics: PhysicsModule::default(),
            anchor,
            tick: 0,
        }
    }
    pub fn advance(&mut self) {
        self.tick += 1;
        self.world.begin_spatial_tick(self.tick);
        self.navigation
            .advance(
                &mut self.world,
                self.loader.asset_loader_mut(),
                self.tick,
                1. / 60.,
            )
            .unwrap();
        self.physics
            .advance(&mut self.world, self.tick, 1. / 60.)
            .unwrap();
        self.world.finish_spatial_tick();
    }
    pub fn state(&self) -> std::sync::Arc<NavigationState3D> {
        self.world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
    }
    pub fn wait_path(&mut self) {
        let start = std::time::Instant::now();
        while self.state().status == NavigationStatus3D::Pending {
            assert!(start.elapsed().as_secs() < 3);
            std::thread::sleep(std::time::Duration::from_millis(1));
            self.advance();
        }
    }
    pub fn goal(&mut self, x: f64) {
        let handle = project_game_sdk::EntityHandle::new(
            "actor",
            self.world
                .entity_generation(&EntityId::from("actor"))
                .unwrap(),
        )
        .unwrap();
        let mut sdk = project_game_sdk::DeferredMutationWriter::default();
        sdk.navigation_target3d(&handle, [self.anchor + x, 0., 8.], 120)
            .unwrap();
        for mutation in sdk.into_inner() {
            let project_game_sdk::DeferredMutation::Spatial {
                entity_id,
                generation,
                intent,
            } = mutation
            else {
                unreachable!()
            };
            let mut batch =
                engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::default();
            batch.spatial_command(entity_id.into(), generation, intent);
            batch
                .prepare(&self.world)
                .unwrap()
                .commit(&mut self.world)
                .unwrap();
        }
    }
}
#[test]
fn despawn_retires_navigation_projection_before_native_synchronization() {
    let mut f = Fixture::new(0.);
    f.advance();
    f.goal(25.);
    f.advance();
    f.wait_path();
    let actor = EntityId::from("actor");
    let old = f.world.navigation3d_state(&actor).unwrap();
    assert_eq!(old.status, NavigationStatus3D::Following);
    f.world.try_despawn_entity(&actor).unwrap();
    assert!(f.world.navigation3d_state(&actor).is_none());
    assert_eq!(old.status, NavigationStatus3D::Following);
}

#[test]
fn requested_navigation_component_exposes_actual_path_without_expanding_stored_state() {
    use engine_runtime::{
        archetype::ComponentValue, component_value::RuntimeValue, components::ComponentTypeId,
        world_api::WorldReadApi,
    };
    for anchor in [0., 10_000., 1_000_000.] {
        let mut f = Fixture::new(anchor);
        f.goal(29.);
        f.advance();
        f.wait_path();
        let id = EntityId::from("actor");
        let kind = ComponentTypeId::new("engine.navigation3d_state");
        let stored = f.world.component_value(&id, &kind).unwrap();
        let ComponentValue::Dynamic {
            value: RuntimeValue::Object(compact),
            ..
        } = &stored
        else {
            panic!("Expected compact committed navigation state");
        };
        assert!(!compact.contains_key("points"));
        let ComponentValue::Dynamic {
            value: RuntimeValue::Object(actual),
            ..
        } = WorldReadApi::new(&f.world)
            .read_component(&id, &kind)
            .unwrap()
        else {
            panic!("Expected requested navigation state");
        };
        let expected = RuntimeValue::Array(
            f.state()
                .points
                .iter()
                .map(|p| RuntimeValue::Array(p.iter().map(|n| RuntimeValue::F64(*n)).collect()))
                .collect(),
        );
        assert_eq!(
            actual.get("points"),
            Some(&expected),
            "actual Detour path at anchor {anchor}"
        );
        assert_eq!(actual.get("pointCount"), compact.get("pointCount"));
        assert_eq!(actual.get("nextPoint"), compact.get("nextPoint"));
        assert_eq!(f.world.component_value(&id, &kind), Some(stored));
    }
}

#[test]
fn real_async_detour_request_sdk_to_motor_accepted_displacement_and_far_anchor() {
    for anchor in [0., 1_000_000_000.] {
        let mut f = Fixture::new(anchor);
        f.goal(29.);
        f.advance();
        f.wait_path();
        assert_eq!(f.state().status, NavigationStatus3D::Following);
        let points = f.state().points.clone();
        assert!((points.last().unwrap()[0] - (anchor + 29.)).abs() < 0.01);
        for _ in 0..440 {
            f.advance();
            if f.state().status == NavigationStatus3D::Arrived {
                break;
            }
        }
        assert_eq!(f.state().status, NavigationStatus3D::Arrived);
        let actor = f.world.spatial_world(&EntityId::from("actor")).unwrap();
        assert!(
            (actor.anchor[0] + f64::from(actor.world_matrix[12]) - (anchor + 29.)).abs() < 0.15
        );
        assert!(f.physics.solver_creation_count() == 1);
    }
}
#[test]
fn changed_goal_cancel_unload_and_stale_result_cannot_resume_old_movement() {
    let mut f = Fixture::new(0.);
    f.goal(30.);
    f.advance();
    f.goal(4.);
    f.advance();
    f.wait_path();
    assert_eq!(f.state().revision, 2);
    assert!(f.state().points.last().unwrap()[0] < 4.01);
    command(&mut f.world, "actor", I::NavigationCancel {});
    f.advance();
    assert_eq!(f.state().status, NavigationStatus3D::Cancelled);
    let revision = f.state().revision;
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        f.advance();
    }
    assert_eq!(f.state().revision, revision);
    assert_eq!(f.state().status, NavigationStatus3D::Cancelled);
    f.goal(30.);
    f.advance();
    f.world.try_despawn_entity(&EntityId::from("nav")).unwrap();
    f.advance();
    assert_eq!(f.state().status, NavigationStatus3D::Invalidated);
    f.goal(3.);
    f.advance();
    assert_eq!(f.state().status, NavigationStatus3D::NotResident);
    assert_eq!(f.navigation.resident_surface_count(), 0);
    f.world
        .try_despawn_entity(&EntityId::from("actor"))
        .unwrap();
    f.advance();
    assert!(f
        .world
        .navigation3d_state(&EntityId::from("actor"))
        .is_none());
}
#[test]
fn loaded_dynamic_obstacle_invalidates_path_and_malformed_profile_is_rejected() {
    let mut f = Fixture::new(0.);
    f.goal(29.);
    f.advance();
    f.wait_path();
    let old = f.state();
    f.world
        .try_insert_dynamic_component(
            EntityId::from("nav"),
            engine_runtime::components::ComponentTypeId::new("engine.navigation_obstacle3d"),
            engine_runtime::component_value::RuntimeValue::object([
                (
                    "schemaVersion",
                    engine_runtime::component_value::RuntimeValue::from("navigation3d-obstacle.v1"),
                ),
                (
                    "halfExtents",
                    engine_runtime::component_value::RuntimeValue::Array(vec![
                        40f64.into(),
                        2f64.into(),
                        40f64.into(),
                    ]),
                ),
            ]),
        )
        .unwrap();
    f.advance();
    assert_eq!(f.state().status, NavigationStatus3D::Invalidated);
    assert_eq!(old.status, NavigationStatus3D::Following);
    f.goal(29.);
    f.advance();
    f.wait_path();
    assert_eq!(f.state().status, NavigationStatus3D::NotResident);
    let mut bad = profile();
    bad.radius = f32::NAN;
    assert!(bad.validate().is_err());
    let mut bytes = asset();
    bytes.profile.key.clear();
    assert!(bytes.validate().is_err());
}

#[test]
fn same_profile_key_with_different_bake_parameters_and_late_result_are_rejected() {
    let mut f = Fixture::new(0.);
    f.goal(29.);
    f.advance();
    f.tick += 121;
    f.advance();
    assert_eq!(f.state().status, NavigationStatus3D::TimedOut);
    for _ in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        f.advance();
    }
    assert_eq!(f.state().status, NavigationStatus3D::TimedOut);
    let mut incompatible = binding();
    incompatible["data"]["navigation"]["id"] = "navigation-incompatible".into();
    f.world = world(vec![
        entity("nav", [0.; 3], vec![binding()]),
        entity("other", [32., 0., 0.], vec![incompatible]),
    ]);
    f.world.begin_spatial_tick(1);
    let error = f
        .navigation
        .advance(&mut f.world, f.loader.asset_loader_mut(), 1, 1. / 60.)
        .unwrap_err();
    assert!(error.message.contains("profile key"), "{error}");
    assert_eq!(f.navigation.resident_surface_count(), 1);
}

#[test]
fn off_mesh_entry_requires_project_action_instead_of_walking_into_the_gap() {
    let mut asset = asset();
    let profile = &asset.profile;
    let config = engine_navigation_native::BakeConfig {
        cell_size: profile.cell_size,
        cell_height: profile.cell_height,
        radius: profile.radius,
        height: profile.height,
        climb: profile.climb,
        slope: profile.max_slope_degrees,
        width: 16.,
        min: [0., -1., 0.],
        max: [16., 4., 16.],
        x: 0,
        z: 0,
    };
    let link = engine_navigation_native::OffMeshLink {
        start: [5., 0., 8.],
        end: [11., 0., 8.],
        radius: 0.6,
        bidirectional: 1,
        id: 1,
    };
    let data = engine_navigation_native::bake(
        &config,
        &[
            [0., 0., 0.],
            [0., 0., 16.],
            [6., 0., 16.],
            [6., 0., 0.],
            [10., 0., 0.],
            [10., 0., 16.],
            [16., 0., 16.],
            [16., 0., 0.],
        ],
        &[[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
        &[link],
    )
    .unwrap();
    asset.tiles = vec![CookedNavTile3D { x: 0, z: 0, data }];
    let mut f = Fixture::with_asset(0., asset);
    f.goal(14.);
    f.advance();
    f.wait_path();
    for _ in 0..100 {
        f.advance();
        if f.state().status == NavigationStatus3D::LinkRequired {
            break;
        }
    }
    assert_eq!(f.state().status, NavigationStatus3D::LinkRequired);
    let x = f
        .world
        .transform3d(&EntityId::from("actor"))
        .unwrap()
        .translation[0];
    assert!((x - 5.).abs() < 0.15);
    for _ in 0..20 {
        f.advance();
    }
    assert!(
        (f.world
            .transform3d(&EntityId::from("actor"))
            .unwrap()
            .translation[0]
            - x)
            .abs()
            < 0.001
    );
    assert!(f.state().off_mesh.iter().any(|v| *v));
}
#[test]
fn local_avoidance_outputs_separation_through_the_actual_motor() {
    let mut f = Fixture::new(0.);
    f.world = world(vec![
        entity("nav", [0.; 3], vec![binding()]),
        entity(
            "floor",
            [16., -0.5, 8.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[20,0.5,12]}),
            )],
        ),
        entity("actor", [2., 0.9, 8.], vec![motor(), agent()]),
        entity("neighbour", [2.5, 0.9, 8.5], vec![motor(), agent()]),
    ]);
    f.goal(29.);
    f.advance();
    f.wait_path();
    f.advance();
    let accepted = f.world.physics3d_frame().motors["actor"].accepted;
    assert!(accepted[0] > 0. && accepted[2] < -0.0001, "{accepted:?}");
}

#[test]
fn bm07_original_and_pressure_agent_capacity_remains_bounded() {
    let mut f = Fixture::new(0.);
    for count in [1000, 2000, 2049] {
        let mut entities = vec![entity("nav", [0.; 3], vec![binding()])];
        entities.extend((0..count).map(|i| {
            entity(
                &format!("agent-{i:04}"),
                [2., 0.9, 8.],
                vec![motor(), agent()],
            )
        }));
        f.world = world(entities);
        let result = f
            .navigation
            .advance(&mut f.world, f.loader.asset_loader_mut(), 1, 1. / 60.);
        if count <= 2000 {
            result.unwrap();
            assert_eq!(
                (0..count)
                    .filter(|i| f
                        .world
                        .navigation3d_state(&EntityId::from(format!("agent-{i:04}")))
                        .is_some())
                    .count(),
                count
            );
        } else {
            assert!(result.unwrap_err().message.contains("budget exceeded"));
        }
    }
}

#[test]
fn bm07_steering_is_ten_hz_while_motor_accepts_sixty_hz_and_manual_cancel_win() {
    let mut f = Fixture::new(0.);
    f.world = world(vec![
        entity("nav", [0.; 3], vec![binding()]),
        entity(
            "floor",
            [16., -0.5, 8.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[20,0.5,12]}),
            )],
        ),
        entity("actor", [2., 0.9, 8.], vec![motor(), agent()]),
        entity("neighbour", [12., 0.9, 12.], vec![motor(), agent()]),
    ]);
    f.goal(29.);
    f.advance();
    f.wait_path();
    let actor = f
        .world
        .transform3d(&EntityId::from("actor"))
        .unwrap()
        .translation;
    command(
        &mut f.world,
        "neighbour",
        I::Teleport {
            transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                translation: [actor[0], 0.9, actor[2] + 0.8],
                rotation_xyzw: [0., 0., 0., 1.],
                scale: [1.; 3],
            },
        },
    );
    for _ in 0..5 {
        f.advance();
        let requested = f.world.physics3d_frame().motors["actor"].requested;
        assert!(
            requested[0] > 0.06 && requested[2].abs() < 1e-6,
            "steering refreshed before 0.1s: {requested:?}"
        );
    }
    f.advance();
    assert!(f.world.physics3d_frame().motors["actor"].requested[2] < -0.0001);
    command(
        &mut f.world,
        "actor",
        I::MotorMove {
            displacement: [0., 0., 0.02],
        },
    );
    f.advance();
    assert!((f.world.physics3d_frame().motors["actor"].requested[2] - 0.02).abs() < 1e-6);
    command(&mut f.world, "actor", I::NavigationCancel {});
    f.advance();
    assert_eq!(f.state().status, NavigationStatus3D::Cancelled);
    let p = f
        .world
        .transform3d(&EntityId::from("actor"))
        .unwrap()
        .translation;
    for _ in 0..8 {
        f.advance();
    }
    assert!(
        (f.world
            .transform3d(&EntityId::from("actor"))
            .unwrap()
            .translation[0]
            - p[0])
            .abs()
            < 1e-5
    );
}

#[test]
fn bm07_pressure_obstacles_reach_native_owner_and_overflow_is_rejected() {
    let mut f = Fixture::new(0.);
    for count in [100, 129] {
        let mut entities = vec![entity("nav", [0.; 3], vec![binding()])];
        entities.extend((0..count).map(|i| entity(&format!("obstacle-{i:03}"), [2.,1.,8.], vec![json!({"componentType":"engine.navigation_obstacle3d","data":{"schemaVersion":"navigation3d-obstacle.v1","halfExtents":[0.1,1,0.1]}})])));
        f.world = world(entities);
        let result = f
            .navigation
            .advance(&mut f.world, f.loader.asset_loader_mut(), 1, 1. / 60.);
        if count == 100 {
            result.unwrap();
        } else {
            assert!(result.unwrap_err().message.contains("budget exceeded"));
        }
    }
}
