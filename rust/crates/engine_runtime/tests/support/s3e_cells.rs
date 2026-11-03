use super::{
    s3c::{body, entity, motor},
    s3d::{agent, asset, binding},
};
use engine_runtime::{
    ids::EntityId, navigation_3d::*, physics_3d::*, render_resource::RenderResourceManager,
    runtime_instance_loader::*, runtime_package::*, runtime_package_builder::*,
    scene_loader::load_scene_into_world, world::World,
};
use serde_json::json;
struct Fixture {
    world: World,
    control: World,
    loader: RuntimeInstanceLoader,
    physics: PhysicsModule,
    reference: PhysicsModule,
    navigation: NavigationModule,
    manager: RenderResourceManager,
    tick: u64,
}
impl Fixture {
    fn new() -> Self {
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "s3e-cells",
            "S3 E Cells",
            "1",
        ));
        let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        input.assets.push(
            RuntimePackageSourceAsset::new(
                "navigation",
                "Navigation",
                "navigation3d",
                "Assets/nav.asset",
                "cooked/nav.asset",
            )
            .with_runtime_payload(serde_json::to_vec(&asset()).unwrap()),
        );
        let floor = entity(
            "floor",
            [48., -0.5, 8.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[64,0.5,16]}),
            )],
        );
        let ball = entity(
            "ball",
            [33., 4., 8.],
            vec![body("dynamic", json!({"kind":"sphere","radius":0.25}))],
        );
        let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[floor,ball,entity("nav",[0.;3],vec![binding()]),entity("actor",[2.,0.9,8.],vec![motor(),agent()])]});
        let engine_runtime::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
            engine_runtime::spatial_contract::decode_runtime_scene(
                &serde_json::to_vec(&scene).unwrap(),
            )
            .unwrap()
        else {
            unreachable!()
        };
        input.scenes.push(scene.into());
        for (id, x) in [("cell-a", 32.), ("cell-b", 64.), ("cell-conflict", 32.)] {
            let root = entity("root", [x, 0., 0.], vec![binding()]);
            let mut collider = entity(
                "collider",
                [1., 1., 8.],
                vec![body(
                    "static",
                    json!({"kind":"box","halfExtents":[0.5,1,1]}),
                )],
            );
            collider["parentId"] = "root".into();
            input.prefabs.push(RuntimePackageSourcePrefab{identity:None,prefab_id:id.into(),document:json!({"schemaVersion":"runtime-prefab.v2","id":id,"name":id,"rootEntityId":"root","entities":[root,collider]})});
        }
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../.artifacts/371/s3-simulation-20261026-084317")
            .join(format!(
                "s3e-cells-{}-{}",
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
        assert!(package
            .runtime_asset_index
            .record_by_guid("cell-a")
            .unwrap()
            .dependencies
            .contains(&"navigation".to_string()));
        let world = load_scene_into_world(&package.active_scene).value.unwrap();
        let control = load_scene_into_world(&package.active_scene).value.unwrap();
        Self {
            world,
            control,
            loader: RuntimeInstanceLoader::from_package(&package),
            physics: PhysicsModule::default(),
            reference: PhysicsModule::default(),
            navigation: NavigationModule::default(),
            manager: RenderResourceManager::new(),
            tick: 0,
        }
    }
    fn request(&mut self, id: &str, placement: &str) -> CellTicket {
        self.loader
            .request_cell(
                CellActivationRequest {
                    schema_version: "cell-activation.v1".into(),
                    placement_id: placement.into(),
                    prefab: RuntimeAssetRef {
                        id: id.into(),
                        asset_type: "prefab".into(),
                        guid: None,
                        sub_asset: None,
                    },
                    asset_revision_epoch: self.loader.asset_loader().asset_revision_epoch(),
                    required_domains: [
                        CellDomain::World,
                        CellDomain::Asset,
                        CellDomain::Physics,
                        CellDomain::Navigation,
                    ]
                    .into(),
                    parent: None,
                },
                &self.world,
            )
            .unwrap()
    }
    fn prepare(&mut self, t: CellTicket) -> Result<CellStatus, CellError> {
        let start = std::time::Instant::now();
        loop {
            match self.loader.prepare_cell_with_simulation(
                t,
                &self.world,
                self.tick,
                &mut self.manager,
                &mut self.physics,
                &mut self.navigation,
            ) {
                Err(CellError::NotReady) => {
                    assert!(start.elapsed().as_secs() < 5);
                    std::thread::yield_now();
                }
                other => return other,
            }
        }
    }
    fn advance(&mut self) {
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
        self.control.begin_spatial_tick(self.tick);
        self.reference
            .advance(&mut self.control, self.tick, 1. / 60.)
            .unwrap();
        self.control.finish_spatial_tick();
    }
    fn ray(&self) -> PhysicsHit3D {
        self.physics
            .raycast(&PhysicsRay3D {
                origin: [33., 3., 8.],
                displacement: [0., -4., 0.],
            })
            .unwrap()
            .unwrap()
    }
    fn goal(&mut self, x: f64) {
        super::s3c::command(
            &mut self.world,
            "actor",
            project_runtime_sdk::ProjectRuntimeSpatialIntent::NavigationTarget {
                target: [x, 0., 8.],
                deadline_ticks: 60,
            },
        );
    }
    fn wait_path(&mut self) {
        let start = std::time::Instant::now();
        while self
            .world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
            .status
            == NavigationStatus3D::Pending
        {
            assert!(start.elapsed().as_secs() < 3);
            std::thread::sleep(std::time::Duration::from_millis(1));
            self.advance();
        }
    }
    fn commit(
        &mut self,
        t: CellTicket,
    ) -> Result<engine_runtime::runtime_instance::RuntimePrefabInstance, CellError> {
        self.loader.commit_cell_with_simulation(
            t,
            &mut self.world,
            &mut self.physics,
            &mut self.navigation,
        )
    }
    fn release(&mut self, t: CellTicket) {
        self.loader
            .release_cell_with_simulation(
                t,
                &mut self.world,
                &mut self.manager,
                self.tick,
                &mut self.physics,
                &mut self.navigation,
            )
            .unwrap();
    }
}
#[test]
fn s3e_two_real_domains_prepare_abort_commit_reload_and_immediate_retirement() {
    let mut f = Fixture::new();
    f.advance();
    f.goal(29.);
    f.advance();
    f.wait_path();
    let path = f
        .world
        .navigation3d_state(&EntityId::from("actor"))
        .unwrap();
    assert_eq!(path.status, NavigationStatus3D::Following);
    let a = f.request("cell-a", "a");
    let b = f.request("cell-b", "b");
    assert_eq!(f.prepare(a).unwrap(), CellStatus::ReadyToCommit);
    assert_eq!(f.prepare(b).unwrap(), CellStatus::ReadyToCommit);
    assert_eq!(f.physics.body_count(), 2);
    assert_eq!(f.navigation.resident_surface_count(), 1);
    assert_eq!(f.ray().entity, "floor");
    for _ in 0..10 {
        f.advance();
        assert_eq!(
            f.world.physics3d_frame().bodies["ball"].position,
            f.control.physics3d_frame().bodies["ball"].position
        );
    }
    assert_eq!(
        path.points,
        f.world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
            .points
    );
    f.goal(50.);
    f.advance();
    f.wait_path();
    assert_eq!(
        f.world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
            .status,
        NavigationStatus3D::NotResident
    );
    f.release(b);
    assert_eq!(f.navigation.resident_surface_count(), 1);
    assert_eq!(f.physics.body_count(), 2);
    let first = f.commit(a).unwrap();
    assert!(matches!(
        f.loader
            .release_cell(a, &mut f.world, &mut f.manager, f.tick),
        Err(CellError::UnsupportedDomain(_))
    ));
    assert_eq!(f.loader.cell_status(a).unwrap(), CellStatus::Active);
    assert_eq!(f.physics.body_count(), 3);
    assert_eq!(f.navigation.resident_surface_count(), 2);
    assert!(f.ray().entity.contains("collider"));
    f.goal(50.);
    f.advance();
    f.wait_path();
    assert_eq!(
        f.world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
            .status,
        NavigationStatus3D::Following
    );
    let b = f.request("cell-b", "b2");
    f.prepare(b).unwrap();
    let second = f.commit(b).unwrap();
    assert_ne!(first.root_entity, second.root_entity);
    assert_eq!(f.physics.body_count(), 4);
    assert_eq!(f.navigation.resident_surface_count(), 3);
    f.goal(85.);
    f.advance();
    f.wait_path();
    assert_eq!(
        f.world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
            .status,
        NavigationStatus3D::Following
    );
    f.release(a);
    assert_eq!(f.physics.body_count(), 3);
    assert_eq!(f.navigation.resident_surface_count(), 2);
    assert_eq!(
        f.world
            .navigation3d_state(&EntityId::from("actor"))
            .unwrap()
            .status,
        NavigationStatus3D::Invalidated
    );
    assert!(!f.ray().entity.contains("collider"));
    assert!(!f.world.physics3d_frame().bodies.keys().any(|k| first
        .source_to_world_entity
        .values()
        .any(|id| id.as_str() == k)));
    let reloaded = f.request("cell-a", "a2");
    f.prepare(reloaded).unwrap();
    let new = f.commit(reloaded).unwrap();
    assert_ne!(new.root_entity, first.root_entity);
    f.release(reloaded);
    f.release(b);
    assert_eq!(f.navigation.resident_surface_count(), 1);
    assert_eq!(f.physics.body_count(), 2);
    assert!(f
        .loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}
#[test]
fn s3e_commit_native_failure_rolls_back_new_bodies_and_bad_prepare_never_activates() {
    let mut f = Fixture::new();
    f.advance();
    let a = f.request("cell-a", "a");
    let conflict = f.request("cell-conflict", "conflict");
    f.prepare(a).unwrap();
    f.prepare(conflict).unwrap();
    f.commit(a).unwrap();
    let count = f.world.entity_ids().len();
    let bodies = f.physics.body_count();
    assert!(matches!(f.commit(conflict), Err(CellError::Simulation(_))));
    assert_eq!(f.world.entity_ids().len(), count);
    assert_eq!(f.physics.body_count(), bodies);
    assert_eq!(f.navigation.resident_surface_count(), 2);
    f.release(conflict);
    let bad = f.request("cell-conflict", "bad");
    assert!(matches!(f.prepare(bad), Err(CellError::Simulation(_))));
    assert_eq!(f.loader.cell_status(bad).unwrap(), CellStatus::Failed);
    assert_eq!(f.physics.body_count(), bodies);
    f.release(bad);
    f.release(a);
    assert_eq!(f.physics.body_count(), 2);
}
