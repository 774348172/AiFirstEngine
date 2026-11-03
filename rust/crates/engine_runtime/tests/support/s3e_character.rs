use super::s3c::{body, entity, floor, motor};
use engine_runtime::{
    character_animation::session::CharacterModule, character_animation::*, ids::EntityId,
    physics_3d::PhysicsModule, runtime_instance_loader::RuntimeInstanceLoader, runtime_package::*,
    runtime_package_builder::*, scene_loader::load_scene_into_world,
};
use serde_json::json;
use std::sync::Arc;
pub(super) struct Fixture {
    pub world: engine_runtime::world::World,
    pub loader: RuntimeInstanceLoader,
    pub character: CharacterModule,
    pub physics: PhysicsModule,
    pub tick: u64,
}
impl Fixture {
    pub fn new() -> Self {
        let asset = super::s3a::asset();
        let mut input =
            RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("s3e", "S3 E", "1"));
        let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        input.assets.push(
            RuntimePackageSourceAsset::new(
                "rig",
                "rig",
                "character3d",
                "Assets/rig.asset",
                "cooked/rig.asset",
            )
            .with_runtime_payload(serde_json::to_vec(&asset).unwrap()),
        );
        let mut actor = entity(
            "actor",
            [0., 1., 0.],
            vec![
                motor(),
                json!({"componentType":"engine.character3d","data":{"schemaVersion":"character3d-binding.v1","character":{"id":"rig","type":"character3d"},"joints":[{"entityRef":"root"},{"entityRef":"hand"}],"playOnAwake":true,"rootMotion":{"joint":"root","axes":[true,false,true]},"ragdoll":{"joints":["root","hand"]},"initialMode":"rootMotion"}}),
            ],
        );
        actor["name"] = "actor".into();
        let bone = |name: &str, parent: &str, p: [f32; 3], joint: bool| {
            let mut b = body("dynamic", json!({"kind":"sphere","radius":0.15}));
            b["data"]["enabled"] = false.into();
            let mut components = vec![b];
            if joint {
                components.push(json!({"componentType":"engine.joint3d","data":{"schemaVersion":"joint3d.v1","kind":"point","other":{"entityRef":"root"},"pivot":[0,-1,0]}}));
            }
            let mut e = entity(name, p, components);
            e["parentId"] = parent.into();
            e
        };
        let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[floor(),entity("wall",[2.,2.,0.],vec![body("static",json!({"kind":"box","halfExtents":[0.2,2,5]}))]),actor,bone("root","actor",[0.;3],false),bone("hand","root",[0.,1.,0.],true)]});
        let engine_runtime::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
            engine_runtime::spatial_contract::decode_runtime_scene(
                &serde_json::to_vec(&scene).unwrap(),
            )
            .unwrap()
        else {
            unreachable!()
        };
        input.scenes.push(scene.into());
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../.artifacts/371/s3-simulation-20261026-084317")
            .join(format!(
                "s3e-character-{}-{}",
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
        let world = load_scene_into_world(&package.active_scene).value.unwrap();
        Self {
            world,
            loader: RuntimeInstanceLoader::from_package(&package),
            character: CharacterModule::default(),
            physics: PhysicsModule::default(),
            tick: 0,
        }
    }
    pub fn advance(&mut self) {
        self.tick += 1;
        self.world.begin_spatial_tick(self.tick);
        self.character
            .prepare(
                &mut self.world,
                self.loader.asset_loader_mut(),
                self.tick,
                1. / 60.,
            )
            .unwrap();
        self.physics
            .advance_with_assets(
                &mut self.world,
                self.loader.asset_loader_mut(),
                self.tick,
                1. / 60.,
            )
            .unwrap();
        self.character.publish(&mut self.world, self.tick).unwrap();
        self.world.finish_spatial_tick();
    }
    pub fn mode(&mut self, mode: project_game_sdk::CharacterMode3D) {
        let handle = project_game_sdk::EntityHandle::new(
            "actor",
            self.world
                .entity_generation(&EntityId::from("actor"))
                .unwrap(),
        )
        .unwrap();
        let mut sdk = project_game_sdk::DeferredMutationWriter::default();
        sdk.character_mode3d(&handle, mode).unwrap();
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
fn s3e_root_motion_blocked_motor_then_ragdoll_joint_handoff_and_return() {
    let mut f = Fixture::new();
    let id = EntityId::from("actor");
    let mut events = 0;
    for _ in 0..180 {
        f.advance();
        events += f.world.character_pose(&id).unwrap().events.len();
    }
    let frame = f.world.character_pose(&id).unwrap();
    let x = f.world.transform3d(&id).unwrap().translation[0];
    assert!(x > 1. && x < 1.48, "{x}");
    assert!(f.world.physics3d_frame().motors["actor"].accepted[0] < 0.001);
    assert_eq!(frame.current.joints[0].translation[0], 0.);
    assert_eq!(f.world.character_joint_matrix(&id, "root").unwrap()[12], 0.);
    assert!(events >= 6, "{events}");
    assert_eq!(f.physics.body_count(), 2);
    f.mode(project_game_sdk::CharacterMode3D::Ragdoll);
    f.advance();
    assert_eq!(f.physics.motor_count(), 0);
    assert_eq!(f.physics.body_count(), 4);
    assert_eq!(f.physics.joint_count(), 1);
    let old = f.world.character_pose(&id).unwrap();
    for _ in 0..40 {
        f.advance();
        assert!(f.world.character_pose(&id).unwrap().events.is_empty());
    }
    let pose = f.world.character_pose(&id).unwrap();
    assert_eq!(pose.mode, CharacterMode3D::Ragdoll);
    assert_ne!(old.current, pose.current);
    assert_eq!(
        pose.current.joints[0].translation,
        f.world
            .transform3d(&EntityId::from("root"))
            .unwrap()
            .translation
    );
    let a = f.world.physics3d_frame().bodies["root"].position;
    let b = f.world.physics3d_frame().bodies["hand"].position;
    assert!(((0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt() - 1.).abs() < 0.03);
    f.mode(project_game_sdk::CharacterMode3D::Animated);
    f.advance();
    assert_eq!(f.physics.body_count(), 2);
    assert_eq!(f.physics.motor_count(), 1);
    assert_eq!(f.physics.joint_count(), 0);
    let pose = f.world.character_pose(&id).unwrap();
    assert!(pose.history_reset);
    assert_eq!(pose.current, pose.previous);
    assert_eq!(old.mode, CharacterMode3D::Ragdoll);
    assert!(Arc::strong_count(&old) >= 1);
}
#[test]
fn s3e_root_interval_crosses_loops_and_seek_does_not_teleport_motor() {
    let mut a = Animator3D::new(
        Arc::new(super::s3a::asset()),
        AnimationGraph3D {
            initial: "walk".into(),
            parameters: Default::default(),
            states: vec![AnimationState3D {
                key: "walk".into(),
                clip: "walk".into(),
                looping: true,
            }],
            transitions: vec![],
            layers: vec![],
        },
    )
    .unwrap();
    a.track_root_motion(Some(0)).unwrap();
    a.seek(1.95).unwrap();
    a.tick(1, 0.1).unwrap();
    assert!(
        (a.root_displacement[0] - 0.4).abs() < 1e-5,
        "{:?}",
        a.root_displacement
    );
    a.seek(100.).unwrap();
    a.tick(2, 0.05).unwrap();
    assert!((a.root_displacement[0] - 0.2).abs() < 1e-5);
    a.set_paused(true);
    a.tick(3, 1.).unwrap();
    assert_eq!(a.root_displacement, [0.; 3]);
    assert!(a.events.is_empty());
}

#[test]
fn animation_rejects_competing_dynamic_joint_before_world_pose_publication() {
    let mut f = Fixture::new();
    f.advance();
    let id = EntityId::from("root");
    let ty = engine_runtime::components::ComponentTypeId::new("engine.body3d");
    let Some(engine_runtime::archetype::ComponentValue::Dynamic { mut value, .. }) =
        f.world.component_value(&id, &ty)
    else {
        unreachable!()
    };
    let engine_runtime::component_value::RuntimeValue::Object(fields) = &mut value else {
        unreachable!()
    };
    fields.insert("enabled".into(), true.into());
    f.world
        .try_insert_dynamic_component(id.clone(), ty, value)
        .unwrap();
    let before = f.world.transform3d(&id).unwrap().clone();
    f.world.begin_spatial_tick(2);
    let error = f
        .character
        .prepare(&mut f.world, f.loader.asset_loader_mut(), 2, 1. / 60.)
        .unwrap_err();
    assert!(error.message.contains("dynamic bodies"));
    assert_eq!(&before, f.world.transform3d(&id).unwrap());
}
