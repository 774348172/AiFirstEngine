use engine_runtime::{
    archetype::ComponentValue, component_value::RuntimeValue, components::ComponentTypeId,
    field_path::FieldPath, ids::EntityId, navigation_3d::NavigationModule,
    physics_3d::PhysicsModule, project_runtime_session::ProjectRuntimeMutationBuffer,
    render_resource::RenderResourceManager, runtime_instance_loader::*,
    scene_loader::load_scene_into_world, spatial_contract::SpatialAssetRef, world::World,
    world_persistence::*, world_streaming::*,
};
use serde_json::json;
use std::collections::BTreeSet;

fn source(id: &str, x: f64) -> StreamingSource3D {
    StreamingSource3D {
        id: id.into(),
        position: [x, 0., 0.],
        activation_radius: 1.,
        prefetch_radius: 2.,
        priority: 0,
    }
}
fn definition() -> WorldStreaming3D {
    WorldStreaming3D {
        schema_version: "world-streaming3d.v1".into(),
        world_id: "test-world".into(),
        revision: 1,
        cells: vec![cell("a", 0.), cell("b", 100.)],
        sources: vec![source("player", 0.), source("observer", 100.)],
        max_resident_cells: 4,
        max_loading_cells: 2,
        unload_margin: 3.,
    }
}
fn cell(id: &str, x: f64) -> WorldCell3D {
    WorldCell3D {
        id: id.into(),
        prefab: SpatialAssetRef(super::streamed_cells::reference(
            &format!("cell-{id}"),
            "prefab",
        )),
        min: [x - 1., -1., -1.],
        max: [x + 1., 1., 1.],
        dependencies: vec![],
        render_required: false,
    }
}

#[test]
fn multiple_sources_hysteresis_priority_dependencies_and_invalid_graphs() {
    let mut d = definition();
    d.cells.push(cell("required", 1000.));
    d.cells[0].dependencies = vec!["required".into()];
    d.sources[0].priority = 10;
    let p = d.plan(&BTreeSet::new()).unwrap();
    assert!(p.desired["required"].activate);
    assert_eq!(&p.order[..2], ["required", "a"]);
    d.sources.remove(0);
    let p = d.plan(&BTreeSet::new()).unwrap();
    assert_eq!(p.order, ["b"]);
    d.sources[0].position[0] = 106.;
    assert!(d.plan(&BTreeSet::new()).unwrap().order.is_empty());
    assert_eq!(d.plan(&BTreeSet::from(["b".into()])).unwrap().order, ["b"]);
    d.cells[2].dependencies = vec!["a".into()];
    assert!(d.validate().is_err());
    d.cells[2].dependencies = vec!["missing".into()];
    assert!(d.validate().is_err());
    d.cells[2].dependencies.clear();
    d.sources[0].position[0] = f64::NAN;
    assert!(d.validate().is_err());
}
pub(super) struct Fixture {
    pub _files: super::streamed_cells::Fixture,
    pub world: World,
    pub loader: RuntimeInstanceLoader,
    pub manager: RenderResourceManager,
    pub physics: PhysicsModule,
    pub nav: NavigationModule,
    pub owner: EntityId,
    pub frame: u64,
}
impl Fixture {
    pub fn new(component: Option<serde_json::Value>) -> Self {
        let files = super::streamed_cells::build_with_component(false, true, 4, component);
        let doc = json!({"schemaVersion":"runtime-scene.v2","id":"world","name":"World","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[{"schemaVersion":"runtime-entity.v2","id":"owner","name":"Owner","enabled":true,"parentId":null,"siblingOrder":0,"spatial":{"domain":"world3d","transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":[]}]});
        let engine_runtime::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
            engine_runtime::spatial_contract::decode_runtime_scene(
                &serde_json::to_vec(&doc).unwrap(),
            )
            .unwrap()
        else {
            panic!()
        };
        let world = load_scene_into_world(&scene.into()).value.unwrap();
        let owner = EntityId::from("owner");
        let loader = RuntimeInstanceLoader::from_package(&files.package);
        Self {
            _files: files,
            world,
            loader,
            manager: RenderResourceManager::new(),
            physics: PhysicsModule::default(),
            nav: NavigationModule::default(),
            owner,
            frame: 0,
        }
    }
    pub fn binding(&mut self, d: &WorldStreaming3D) {
        let ty = ComponentTypeId::new("engine.world_streaming3d");
        self.world
            .try_insert_component_value(
                self.owner.clone(),
                ty.clone(),
                ComponentValue::Dynamic {
                    component_type: ty,
                    value: runtime_json(serde_json::to_value(d).unwrap()),
                },
            )
            .unwrap();
    }
    pub fn advance(&mut self) {
        self.frame += 1;
        self.loader.advance_world_streaming(
            &mut self.world,
            &mut self.manager,
            self.frame,
            &mut self.physics,
            &mut self.nav,
            #[cfg(feature = "real-wgpu")]
            None,
        );
    }
    pub fn wait(&mut self, ready: impl Fn(&Self) -> bool) {
        let start = std::time::Instant::now();
        while !ready(self) {
            assert!(
                start.elapsed().as_secs() < 10,
                "streaming timeout at frame {}",
                self.frame
            );
            self.advance();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    pub fn active(&self, id: &str) -> bool {
        self.loader
            .streaming_cell_ticket(&self.owner, id, &self.world)
            .is_some_and(|t| self.loader.cell_status(t) == Ok(CellStatus::Active))
    }
    pub fn load(&mut self, placement: &str) -> CellTicket {
        let mut r = super::streamed_cells::request(
            "a",
            self.loader.asset_loader().asset_revision_epoch(),
            false,
        );
        r.placement_id = placement.into();
        r.required_domains
            .extend([CellDomain::Physics, CellDomain::Navigation]);
        let t = self.loader.request_cell(r, &self.world).unwrap();
        let start = std::time::Instant::now();
        loop {
            match self.loader.prepare_cell_with_simulation(
                t,
                &self.world,
                self.frame,
                &mut self.manager,
                &mut self.physics,
                &mut self.nav,
            ) {
                Ok(_) => break,
                Err(CellError::NotReady) => {
                    assert!(start.elapsed().as_secs() < 10);
                    std::thread::yield_now();
                }
                Err(e) => panic!("{e:?}"),
            }
        }
        self.loader
            .commit_cell_with_simulation(t, &mut self.world, &mut self.physics, &mut self.nav)
            .unwrap();
        t
    }
    pub fn release(&mut self, t: CellTicket) {
        self.loader
            .release_cell_with_simulation(
                t,
                &mut self.world,
                &mut self.manager,
                self.frame,
                &mut self.physics,
                &mut self.nav,
            )
            .unwrap();
    }
    pub fn node(&self, t: CellTicket) -> EntityId {
        self.loader
            .prefab_instance(t.instance_id())
            .unwrap()
            .source_to_world_entity[&EntityId::from("node")]
            .clone()
    }
}
pub(super) fn runtime_json(v: serde_json::Value) -> RuntimeValue {
    match v {
        serde_json::Value::Null => RuntimeValue::Null,
        serde_json::Value::Bool(v) => RuntimeValue::Bool(v),
        serde_json::Value::Number(v) => v
            .as_i64()
            .map(RuntimeValue::I64)
            .unwrap_or_else(|| RuntimeValue::F64(v.as_f64().unwrap())),
        serde_json::Value::String(v) => RuntimeValue::String(v),
        serde_json::Value::Array(v) => {
            RuntimeValue::Array(v.into_iter().map(runtime_json).collect())
        }
        serde_json::Value::Object(v) => {
            RuntimeValue::Object(v.into_iter().map(|(k, v)| (k, runtime_json(v))).collect())
        }
    }
}
#[test]
fn actual_streaming_union_cancel_revision_and_owner_retirement() {
    let mut f = Fixture::new(None);
    let mut d = definition();
    f.binding(&d);
    f.wait(|f| f.active("a") && f.active("b"));
    assert!(f.loader.streaming_can_enter(&f.owner, [0.; 3], &f.world));
    let a = f
        .loader
        .streaming_cell_ticket(&f.owner, "a", &f.world)
        .unwrap();
    let b = f
        .loader
        .streaming_cell_ticket(&f.owner, "b", &f.world)
        .unwrap();
    d.sources.remove(0);
    f.binding(&d);
    f.wait(|f| f.loader.cell_status(a) == Ok(CellStatus::Released));
    assert_eq!(f.loader.cell_status(b), Ok(CellStatus::Active));
    assert!(!f.loader.streaming_can_enter(&f.owner, [0.; 3], &f.world));
    d.revision += 1;
    f.binding(&d);
    f.wait(|f| f.loader.cell_status(b) == Ok(CellStatus::Released) && f.active("b"));
    let replacement = f
        .loader
        .streaming_cell_ticket(&f.owner, "b", &f.world)
        .unwrap();
    assert_ne!(b, replacement);
    f.world
        .try_remove_component_value(&f.owner, &ComponentTypeId::new("engine.world_streaming3d"))
        .unwrap();
    f.wait(|f| f.loader.cell_status(replacement) == Ok(CellStatus::Released));
    assert_eq!(f.loader.cell_lifetime_counts().1, 0);
}
fn health(f: &Fixture, id: &EntityId) -> RuntimeValue {
    let ComponentValue::Dynamic { value, .. } = f
        .world
        .component_value(id, &ComponentTypeId::new("project.stats"))
        .unwrap()
    else {
        panic!()
    };
    let RuntimeValue::Object(v) = value else {
        panic!()
    };
    v["health"].clone()
}
fn set_health(f: &mut Fixture, id: &EntityId, n: i64) {
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.write_component_field(
        id.clone(),
        ComponentTypeId::new("project.stats"),
        FieldPath::parse("health").unwrap(),
        RuntimeValue::I64(n),
    );
    b.prepare(&f.world).unwrap().commit(&mut f.world).unwrap();
}
#[test]
fn duplicate_prefab_save_reload_new_incarnations_and_corruption_reject_atomically() {
    let mut f = Fixture::new(Some(
        json!({"componentType":"project.stats","data":{"health":10}}),
    ));
    let a = f.load("left");
    let b = f.load("right");
    let ai = f.node(a);
    let bi = f.node(b);
    let old = f.world.runtime_id_for_source(&ai).unwrap();
    set_health(&mut f, &ai, 31);
    set_health(&mut f, &bi, 79);
    let d = WorldLogicalDeclaration3D {
        world_id: "save-world".into(),
        revision: 1,
        placements: vec!["left".into(), "right".into()],
        fields: vec![WorldLogicalField3D {
            component: "project.stats".into(),
            field: "health".into(),
        }],
    };
    let save = f.loader.capture_partition_logical(&d, &f.world).unwrap();
    let bytes = serde_json::to_vec(&save).unwrap();
    f.release(a);
    f.release(b);
    let a = f.load("left");
    let b = f.load("right");
    let ai = f.node(a);
    let bi = f.node(b);
    assert!(f.world.try_resolve_runtime_entity(old).is_err());
    f.loader
        .restore_partition_logical(&d, &bytes, &mut f.world)
        .unwrap();
    assert_eq!(health(&f, &ai), RuntimeValue::I64(31));
    assert_eq!(health(&f, &bi), RuntimeValue::I64(79));
    let mut bad = save.clone();
    bad.records[1].source_entity = "absent".into();
    bad.records[0].value = project_runtime_sdk::ProjectRuntimeValue::Integer(999);
    assert!(f
        .loader
        .restore_partition_logical(&d, &serde_json::to_vec(&bad).unwrap(), &mut f.world)
        .is_err());
    assert_eq!(health(&f, &ai), RuntimeValue::I64(31));
    bad = save.clone();
    bad.revision = 2;
    assert!(f
        .loader
        .restore_partition_logical(&d, &serde_json::to_vec(&bad).unwrap(), &mut f.world)
        .is_err());
    assert!(f
        .loader
        .restore_partition_logical(&d, b"{bad json", &mut f.world)
        .is_err());
    bad = save;
    bad.records[0].value = project_runtime_sdk::ProjectRuntimeValue::EntityRef(ai.to_string());
    assert!(f
        .loader
        .restore_partition_logical(&d, &serde_json::to_vec(&bad).unwrap(), &mut f.world)
        .is_err());
}
#[test]
fn native_simulation_migration_keeps_generation_pose_and_survives_source_release() {
    let mut f = Fixture::new(Some(
        json!({"componentType":"engine.body3d","data":{"schemaVersion":"body3d.v1","motion":"static","shape":{"kind":"sphere","radius":0.4},"mass":1,"friction":0.5,"restitution":0,"ccd":true,"sensor":false,"allowSleeping":true}}),
    ));
    let t = f.load("moving");
    let id = f.node(t);
    f.world.begin_spatial_tick(1);
    f.physics.advance(&mut f.world, 1, 1. / 60.).unwrap();
    f.world.finish_spatial_tick();
    let pose = f.world.spatial_world(&id).unwrap().clone();
    let generation = f.world.entity_generation(&id).unwrap();
    let request = CellMigrationRequest {
        source_entity_id: "node".into(),
        source_generation: generation,
        persistent_placement_id: "persistent".into(),
        parent: None,
        preserve_world: true,
    };
    assert!(f
        .loader
        .migrate_cell_subtree(t, request.clone(), &mut f.world, &mut f.manager)
        .is_err());
    let persistent = f
        .loader
        .migrate_cell_subtree_with_simulation(
            t,
            request,
            &mut f.world,
            &mut f.manager,
            &mut f.physics,
            &mut f.nav,
        )
        .unwrap();
    assert_eq!(f.world.entity_generation(&id), Some(generation));
    assert_eq!(
        f.world.spatial_world(&id).unwrap().world_matrix,
        pose.world_matrix
    );
    f.release(t);
    assert!(f.world.entity(&id).is_some());
    let hit = f
        .physics
        .raycast(&engine_runtime::physics_3d::PhysicsRay3D {
            origin: [-0.6, 2., -2.],
            displacement: [0., -4., 0.],
        })
        .unwrap();
    assert_eq!(hit.unwrap().entity, id.to_string());
    f.release(persistent);
    assert!(f.world.entity(&id).is_none());
    assert!(f
        .physics
        .raycast(&engine_runtime::physics_3d::PhysicsRay3D {
            origin: [-0.6, 2., -2.],
            displacement: [0., -4., 0.]
        })
        .unwrap()
        .is_none());
}
