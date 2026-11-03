use engine_runtime::components::Hierarchy;
use engine_runtime::ids::{EntityId, RuntimeEntityId};
use engine_runtime::render_contract::{transform_point, CameraPose, DepthPolicy, RenderView};
use engine_runtime::runtime_time::RuntimeTime;
use engine_runtime::spatial_contract::*;
use engine_runtime::world::World;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[path = "../../../../samples/platform_3d_bm13_project/RuntimeModule/src/route.rs"]
mod bm13_project_route;

#[test]
fn bm13_same_tick_motion_and_twenty_keep_world_rebases_use_the_previous_coordinate_frame() {
    use engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer;
    use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;
    let doc = json!({"schemaVersion":"runtime-scene.v2","id":"bm13-motion","name":"BM13 motion","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,
        "entities":[{"schemaVersion":"runtime-entity.v2","id":"listener","name":"Listener","enabled":true,"parentId":null,"siblingOrder":0,
        "spatial":{"domain":"world3d","transform":{"translation":[16,18,16],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":[]}]});
    let RuntimeSceneDocument::Spatial(scene) = read(doc).unwrap() else {
        panic!()
    };
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&scene.into())
        .value
        .unwrap();
    let id = EntityId::from("listener");
    for phase in 1..=1000 {
        world.begin_spatial_tick(phase);
        let mut batch = ProjectRuntimeMutationBuffer::default();
        if phase <= 900 && phase.is_multiple_of(45) {
            batch.spatial_command(
                id.clone(),
                world.entity_generation(&id).unwrap(),
                Intent::SetWorldAnchor {
                    anchor: [(phase / 45 * 16) as f64, 0., 0.],
                    preserve_world: true,
                },
            );
        }
        batch.spatial_command(
            id.clone(),
            world.entity_generation(&id).unwrap(),
            Intent::SetLocalTransform {
                transform: bm13_project_route::command_pose(phase),
            },
        );
        batch.prepare(&world).unwrap().commit(&mut world).unwrap();
        world.finish_spatial_tick();
        let actual = world.spatial_world(&id).unwrap();
        let expected = bm13_project_route::sample(phase);
        for j in 0..3 {
            let absolute = actual.anchor[j] + f64::from(actual.world_matrix[12 + j]);
            assert!(
                (absolute - f64::from(expected.translation[j])).abs() < 0.001,
                "phase={phase}, axis={j}, actual={absolute}, expected={}",
                expected.translation[j]
            );
        }
    }
}

fn domain() -> CoordinateDomain {
    CoordinateDomain::new(1e12, 32768., 32768., 0.005).unwrap()
}
fn position(anchor: f64, local: f32) -> WorldPosition {
    WorldPosition::new([anchor, 0., 0.], [local, 0., 0.]).unwrap()
}
fn fixture() -> Value {
    serde_json::from_slice(include_bytes!("fixtures/three_d_a0/runtime-scene-v2.json")).unwrap()
}
fn read(value: Value) -> ContractResult<RuntimeSceneDocument> {
    decode_runtime_scene(&serde_json::to_vec(&value).unwrap())
}

#[test]
fn large_anchor_keeps_small_local_motion_and_cross_anchor_offset() {
    let camera = position(1e12, 0.125);
    assert_eq!(
        position(1e12, 0.25).relative_to(camera, domain()).unwrap(),
        [0.125, 0., 0.]
    );
    assert_eq!(
        position(1e12 - 16., 0.25)
            .relative_to(camera, domain())
            .unwrap(),
        [-15.875, 0., 0.]
    );
    // Even f64 global addition first would erase this displacement at this anchor.
    assert_eq!(
        position(1e12, 0.000_001)
            .relative_to(position(1e12, 0.), domain())
            .unwrap()[0],
        0.000_001
    );
}

#[test]
fn rebasing_both_anchors_preserves_relative_result() {
    let a = WorldPosition::new([512., -256., 64.], [0.125, -0.5, 1.]).unwrap();
    let b = WorldPosition::new([496., -240., 32.], [-0.125, 0.5, 2.]).unwrap();
    let shifted_a =
        WorldPosition::new([1e12 + 512., -1e12 - 256., 64.], [0.125, -0.5, 1.]).unwrap();
    let shifted_b =
        WorldPosition::new([1e12 + 496., -1e12 - 240., 32.], [-0.125, 0.5, 2.]).unwrap();
    let envelope = CoordinateDomain::new(2e12, 32768., 32768., 0.005).unwrap();
    assert_eq!(a.relative_to(b, envelope).unwrap(), [16.25, -17., 31.]);
    assert_eq!(
        shifted_a.relative_to(shifted_b, envelope).unwrap(),
        [16.25, -17., 31.]
    );
}

#[test]
fn non_finite_positions_and_invalid_envelopes_are_rejected() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            WorldPosition::new([value, 0., 0.], [0.; 3])
                .unwrap_err()
                .code,
            "three_d.coordinate.non_finite"
        );
    }
    assert!(WorldPosition::new([0.; 3], [0., f32::NAN, 0.]).is_err());
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(CoordinateDomain::new(value, 1., 1., 0.01).is_err());
        assert!(CoordinateDomain::new(1., 1., 1., value).is_err());
    }
    assert!(CoordinateDomain::new(1., 1., f64::MAX, 0.01).is_err());
}

#[test]
fn envelope_boundary_is_inclusive_but_never_clamps_out_of_range() {
    assert_eq!(
        position(1e12, 32768.)
            .relative_to(position(1e12, 0.), domain())
            .unwrap()[0],
        32768.
    );
    for (a, b) in [
        (position(1e12 + 1., 0.), position(1e12, 0.)),
        (position(0., 32769.), position(0., 0.)),
        (position(32769., 0.), position(0., 0.)),
    ] {
        let error = a.relative_to(b, domain()).unwrap_err();
        assert_eq!(error.code, "three_d.coordinate.out_of_range");
        assert_eq!(error.field_path, "relative[0]");
    }
}

#[test]
fn precision_budget_rejects_loss_even_inside_range() {
    let strict = CoordinateDomain::new(1e12, 32768., 32768., 1e-6).unwrap();
    assert_eq!(
        position(16384.0005, 0.)
            .relative_to(position(0., 0.), strict)
            .unwrap_err()
            .code,
        "three_d.coordinate.precision_budget"
    );
    assert_eq!(
        position(1e12, 0.125)
            .relative_to(position(1e12, 0.), strict)
            .unwrap_err()
            .code,
        "three_d.coordinate.precision_budget"
    );
}

#[test]
fn prefab_instances_and_worlds_have_distinct_persistent_identity() {
    let identity = |world: &str, instance: &str, local: &str| {
        PersistentEntityIdentity::new(world.into(), instance.into(), local.into()).unwrap()
    };
    let ids = [
        identity("world-a", "placement-1", "root"),
        identity("world-a", "placement-2", "root"),
        identity("world-b", "placement-1", "root"),
        identity("a:b", "c", "root"),
        identity("a", "b:c", "root"),
    ];
    assert_eq!(ids.into_iter().collect::<BTreeSet<_>>().len(), 5);
    assert_eq!(
        identity("world-a", "placement-1", "root"),
        identity("world-a", "placement-1", "root")
    );
    assert!(PersistentEntityIdentity::new("a".into(), " ".into(), "root".into()).is_err());
}

#[test]
fn despawn_respawn_cannot_revive_old_runtime_handle() {
    let mut world = World::new();
    let id = EntityId::new("placement-1/root");
    let hierarchy = || Hierarchy {
        parent_id: None,
        sibling_order: 0,
    };
    let old = world
        .try_spawn_entity(id.clone(), "Root", "entity", true, hierarchy())
        .unwrap();
    world.try_despawn_runtime_entity(old).unwrap();
    let new = world
        .try_spawn_entity(id, "Root", "entity", true, hierarchy())
        .unwrap();
    assert_ne!(old, new);
    assert!(world.try_resolve_runtime_entity(new).is_ok());
    for stale in [
        old,
        RuntimeEntityId::new(u32::MAX, 0),
        RuntimeEntityId::new(new.index, new.generation.wrapping_add(1)),
    ] {
        assert_eq!(
            world.try_resolve_runtime_entity(stale).unwrap_err().code,
            "world.entity.stale_handle"
        );
        assert!(world.try_despawn_runtime_entity(stale).is_err());
    }
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn perspective_and_orthographic_keep_forward_zero_to_one_depth() {
    for mut view in [
        RenderView::perspective(1, 800, 600, 60., 1., 101.),
        RenderView::orthographic(2, 800, 600, 20., 1., 101.),
    ] {
        view.depth_policy = DepthPolicy::TestAndWrite;
        let executable = view.evaluate(CameraPose::default()).unwrap();
        assert_eq!(executable.depth_policy, DepthPolicy::TestAndWrite);
        let matrix = executable.view_projection();
        let depth = |z| {
            let p = transform_point(matrix, [0., 0., z, 1.]);
            p[2] / p[3]
        };
        assert!(depth(-1.).abs() < 1e-6);
        assert!((depth(-101.) - 1.).abs() < 1e-6);
        assert!(depth(-2.) > depth(-1.));
        assert!(depth(-100.) < depth(-101.));
    }
    assert_eq!(
        RenderView::orthographic(3, 800, 600, 20., 1., 101.).depth_policy,
        DepthPolicy::Disabled
    );
}

#[test]
fn mirrored_nonuniform_trs_is_valid_but_shear_cannot_hide_in_trs_wire() {
    let mut scene = fixture();
    scene["entities"][0]["spatial"]["transform"]["scale"] = json!([-2., 3., 0.5]);
    assert!(read(scene.clone()).is_ok());
    scene["entities"][0]["spatial"]["transform"]["shear"] = json!([0.5, 0., 0.]);
    assert!(read(scene).is_err());
}

#[test]
fn current_reader_rejects_future_required_world_fields_and_versions() {
    for field in ["worldAnchor", "persistentIdentity", "requiredCapabilities"] {
        let mut scene = fixture();
        scene["entities"][0]["spatial"][field] = json!({"required": true});
        assert!(read(scene).is_err(), "must not silently ignore {field}");
    }
    let mut scene = fixture();
    scene["schemaVersion"] = json!("runtime-scene.v3");
    assert!(read(scene).is_err());
    assert!(
        decode_runtime_scene(include_bytes!("fixtures/three_d_a0/runtime-scene-v1.json")).is_ok()
    );
}

#[test]
fn presentation_frames_do_not_implicitly_advance_fixed_simulation() {
    let mut time = RuntimeTime::new();
    time.set_time_scale(0.);
    time.advance_frame(0.1);
    assert_eq!(time.context().fixed_frame_count, 0);
    assert_eq!(time.context().time, 0.);
    assert_eq!(time.context().unscaled_time, 0.1);
    time.advance_fixed_step();
    assert_eq!(time.context().fixed_frame_count, 1);
    assert_eq!(time.context().fixed_time, 0.);
    time.leave_fixed_step();
    assert!(!time.context().in_fixed_step);
    time.set_time_scale(1.);
    time.advance_fixed_step();
    assert!((time.context().fixed_time - 1. / 60.).abs() < 1e-7);
    assert_eq!(time.context().frame_count, 1);
}
