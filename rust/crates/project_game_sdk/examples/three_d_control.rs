//! `cargo run -p project_game_sdk --example three_d_control`.
//! SDK-only example: command emission is tested here; engine commit is tested by the runtime.
use project_game_sdk::*;

fn orbit(camera: &EntityHandle, angle: f32, writer: &mut DeferredMutationWriter) -> GameResult<()> {
    writer.write_local_transform3d(
        camera,
        Transform3D {
            translation: [6.0 * angle.sin(), 0.0, 6.0 * angle.cos()],
            rotation_xyzw: [0.0, (angle * 0.5).sin(), 0.0, (angle * 0.5).cos()],
            scale: [1.0; 3],
        },
    )
}
fn demonstrate() -> GameResult<()> {
    // A real project obtains these handles from ctx.world().resolve_entity/query.
    let camera = EntityHandle::new("camera", 1)?;
    let mesh = EntityHandle::new("cube", (1_u64 << 32) + 1)?;
    let mut writer = DeferredMutationWriter::default();
    orbit(&camera, std::f32::consts::FRAC_PI_2, &mut writer)?;
    writer.select_main_camera(&camera)?;
    writer.set_mesh_visible(&mesh, false)?;
    writer.set_material(
        &mesh,
        "surface",
        SpatialAssetRef {
            id: "blue".into(),
            asset_type: "material".into(),
            guid: Some("guid-blue".into()),
            sub_asset: None,
        },
    )?;
    assert_eq!(writer.as_slice().len(), 4);
    let DeferredMutation::Spatial {
        intent: project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform { transform },
        ..
    } = &writer.as_slice()[0]
    else {
        panic!("typed transform")
    };
    assert!((transform.translation[0] - 6.0).abs() < 1e-5);
    assert!(transform.translation[2].abs() < 1e-5);
    let length = transform.rotation_xyzw.iter().map(|v| v * v).sum::<f32>();
    assert!((length - 1.0).abs() < 1e-5);
    // Spawn requests are deferred. In the NEXT callback use world.resolve_spawn(ticket)
    // and keep that actual root handle for select_main_camera/despawn_spatial_prefab.
    let ticket = writer.instantiate_spatial_prefab(
        1,
        SpatialAssetRef {
            id: "camera-rig".into(),
            asset_type: "prefab".into(),
            guid: Some("guid-camera-rig".into()),
            sub_asset: None,
        },
    )?;
    assert_eq!(ticket.request_id(), 1);
    Ok(())
}
fn main() -> GameResult<()> {
    demonstrate()
}
#[test]
fn typed_orbit_and_deferred_prefab_example() {
    demonstrate().unwrap();
}
