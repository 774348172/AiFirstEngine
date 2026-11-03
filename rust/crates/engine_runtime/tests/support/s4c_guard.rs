//! Real streaming readiness -> SDK motor command -> native character solver.
use super::s4a::{runtime_json, Fixture};
use engine_runtime::{
    archetype::ComponentValue,
    components::ComponentTypeId,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_instance_loader::CellStatus,
    spatial_contract::SpatialAssetRef,
    world_streaming::{StreamingSource3D, WorldCell3D, WorldStreaming3D},
};
use project_game_sdk::{DeferredMutation, DeferredMutationWriter, EntityHandle};
use project_runtime_sdk::{ProjectRuntimeSpatialIntent, ProjectRuntimeTransform3D};
use serde_json::json;

fn insert(f: &mut Fixture, ty: &str, value: serde_json::Value) {
    let ty = ComponentTypeId::new(ty);
    f.world
        .try_insert_component_value(
            f.owner.clone(),
            ty.clone(),
            ComponentValue::Dynamic {
                component_type: ty,
                value: runtime_json(value),
            },
        )
        .unwrap();
}
fn tick(f: &mut Fixture, tick: u64) {
    f.world.begin_spatial_tick(tick);
    f.physics.advance(&mut f.world, tick, 1. / 60.).unwrap();
    f.world.finish_spatial_tick();
}
fn sdk_move(f: &mut Fixture, displacement: [f32; 3]) {
    let handle = EntityHandle::new(
        f.owner.as_str(),
        f.world.entity_generation(&f.owner).unwrap(),
    )
    .unwrap();
    let mut writer = DeferredMutationWriter::default();
    writer.motor_move3d(&handle, displacement).unwrap();
    let mut commands = ProjectRuntimeMutationBuffer::new();
    for mutation in writer.into_inner() {
        let DeferredMutation::Spatial {
            entity_id,
            generation,
            intent,
        } = mutation
        else {
            panic!("motor SDK must emit spatial intent")
        };
        commands.spatial_command(entity_id.into(), generation, intent);
    }
    commands
        .prepare(&f.world)
        .unwrap()
        .commit(&mut f.world)
        .unwrap();
}
fn x(f: &Fixture) -> f32 {
    f.world.transform3d(&f.owner).unwrap().translation[0]
}

#[test]
fn s4c_native_motor_fails_closed_before_active_and_after_required_cell_retires() {
    let mut f = Fixture::new(None);
    // A persistent actor also hosts the streaming declaration. Its guard refers
    // to that ordinary live owner, rather than test-supplied readiness values.
    let mut transform = ProjectRuntimeMutationBuffer::new();
    transform.spatial_command(
        f.owner.clone(),
        f.world.entity_generation(&f.owner).unwrap(),
        ProjectRuntimeSpatialIntent::SetLocalTransform {
            transform: ProjectRuntimeTransform3D {
                translation: [-0.25, 0., 0.],
                rotation_xyzw: [0., 0., 0., 1.],
                scale: [1.; 3],
            },
        },
    );
    transform
        .prepare(&f.world)
        .unwrap()
        .commit(&mut f.world)
        .unwrap();
    insert(
        &mut f,
        "engine.motor3d",
        json!({"schemaVersion":"motor3d.v1","radius":0.2,"halfHeight":0.3,"maxSlopeDegrees":45,"stepHeight":0,"mass":70,"gravity":0}),
    );
    let owner = f.owner.to_string();
    insert(&mut f, "engine.streaming_guard3d", json!({"owner":owner}));
    let declaration = WorldStreaming3D {
        schema_version: "world-streaming3d.v1".into(),
        world_id: "guard-world".into(),
        revision: 1,
        cells: vec![
            WorldCell3D {
                id: "safe".into(),
                prefab: SpatialAssetRef(super::streamed_cells::reference("cell-a", "prefab")),
                min: [-2., -2., -2.],
                max: [0., 2., 2.],
                dependencies: vec![],
                render_required: false,
            },
            WorldCell3D {
                id: "target".into(),
                prefab: SpatialAssetRef(super::streamed_cells::reference("cell-b", "prefab")),
                min: [0., -2., -2.],
                max: [2., 2., 2.],
                dependencies: vec!["required".into()],
                render_required: false,
            },
            WorldCell3D {
                id: "required".into(),
                prefab: SpatialAssetRef(super::streamed_cells::reference("cell-a", "prefab")),
                min: [100., -2., -2.],
                max: [102., 2., 2.],
                dependencies: vec![],
                render_required: false,
            },
        ],
        sources: vec![
            StreamingSource3D {
                id: "player".into(),
                position: [1., 0., 0.],
                activation_radius: 0.1,
                prefetch_radius: 0.2,
                priority: 0,
            },
            StreamingSource3D {
                id: "safe-source".into(),
                position: [-0.5, 0., 0.],
                activation_radius: 0.1,
                prefetch_radius: 0.2,
                priority: 0,
            },
        ],
        max_resident_cells: 4,
        max_loading_cells: 2,
        unload_margin: 0.1,
    };
    f.binding(&declaration);
    // No driver advance: target and its required cell cannot be Active yet.
    sdk_move(&mut f, [0.5, 0., 0.]);
    tick(&mut f, 1);
    assert!(
        (x(&f) + 0.25).abs() < 0.002,
        "unready entry must be denied by the native motor"
    );
    assert!(f.world.physics3d_frame().motors[f.owner.as_str()].accepted[0].abs() < 0.002);
    f.wait(|f| f.active("safe") && f.active("target") && f.active("required"));
    assert!(f
        .loader
        .streaming_can_enter(&f.owner, [0.25, 0., 0.], &f.world));
    sdk_move(&mut f, [0.5, 0., 0.]);
    tick(&mut f, 2);
    assert!(
        (x(&f) - 0.25).abs() < 0.01,
        "ready entry must reach the actual native character position: {}",
        x(&f)
    );
    assert!(f.world.physics3d_frame().motors[f.owner.as_str()].accepted[0] > 0.45);
    let dependency = f
        .loader
        .streaming_cell_ticket(&f.owner, "required", &f.world)
        .unwrap();
    f.release(dependency);
    assert_eq!(f.loader.cell_status(dependency), Ok(CellStatus::Released));
    // The ordinary driver recomputes cached regions from real dependency status.
    f.advance();
    assert!(
        f.active("target"),
        "the target itself remains Active; its prerequisite is what retired"
    );
    assert!(!f
        .loader
        .streaming_can_enter(&f.owner, [0.75, 0., 0.], &f.world));
    let before = x(&f);
    sdk_move(&mut f, [0.5, 0., 0.]);
    tick(&mut f, 3);
    assert!(
        (x(&f) - before).abs() < 0.002,
        "dependency loss must block motion even inside the target bounds"
    );
    assert!(f.world.physics3d_frame().motors[f.owner.as_str()].accepted[0].abs() < 0.002);
    // Removing the actual owner declaration also fails closed for stale guards.
    f.world
        .try_remove_component_value(&f.owner, &ComponentTypeId::new("engine.world_streaming3d"))
        .unwrap();
    f.advance();
    sdk_move(&mut f, [0.5, 0., 0.]);
    tick(&mut f, 4);
    assert!((x(&f) - before).abs() < 0.002);
}
