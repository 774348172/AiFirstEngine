//! Four SDK recipes. Run with `cargo run -p project_game_sdk --example generated_content`.
//! Actual native publication/storage is covered by the Compiler/Player integration consumer.
use project_game_sdk::generated_content::*;
use project_game_sdk::*;

fn material() -> SpatialAssetRef {
    SpatialAssetRef {
        id: "surface-material".into(),
        asset_type: "material".into(),
        guid: Some("surface-material-guid".into()),
        sub_asset: None,
    }
}
fn full_replacement(writer: &mut DeferredMutationWriter, target: &EntityHandle) -> GameResult<()> {
    let points = vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
    let geometry = GeometryBuilder3D::new(&points)
        .triangles_u16(
            &[0, 1, 2],
            vec![GeometrySection3D {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
                material: material(),
            }],
        )
        .unwrap();
    writer.replace_geometry3d(target, 1, 0, geometry, CollisionUpdate3D::Keep)?;
    Ok(()) // points are no longer needed; the deferred payload owns its bytes.
}
fn fixed_layout(
    writer: &mut DeferredMutationWriter,
    target: &EntityHandle,
    applied: u64,
) -> GameResult<()> {
    let patch =
        AttributePatch3D::positions(0, &[[2., 0., 0.], [3., 0., 0.], [2., 1., 0.]]).unwrap();
    writer.update_geometry_attributes3d(target, 2, applied, patch)?;
    Ok(())
}
fn paired_static_collision(
    writer: &mut DeferredMutationWriter,
    target: &EntityHandle,
    applied: u64,
) -> GameResult<()> {
    let points = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
    let geometry = GeometryBuilder3D::new(&points)
        .triangles_u16(
            &[0, 1, 2],
            vec![GeometrySection3D {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
                material: material(),
            }],
        )
        .unwrap();
    let body = StaticBody3D::new(StaticShape3D::triangles(&points, &[[0, 1, 2]]).unwrap());
    writer.replace_geometry3d(
        target,
        3,
        applied,
        geometry,
        CollisionUpdate3D::Replace { body },
    )?;
    Ok(())
}
fn byte_save(writer: &mut DeferredMutationWriter) -> GameResult<()> {
    writer.write_project_bytes(1, "parameters", vec![0, 1, 255])?;
    writer.read_project_bytes(2, "parameters")?;
    Ok(())
}
fn main() -> GameResult<()> {
    // In a real callback resolve this handle from ctx.world(), declare .generated_content()
    // and/or .project_storage(), and return Applied. Wait for each prior Applied before update.
    let target = EntityHandle::new("surface", 7)?;
    let mut writer = DeferredMutationWriter::default();
    full_replacement(&mut writer, &target)?;
    fixed_layout(&mut writer, &target, 1)?;
    paired_static_collision(&mut writer, &target, 2)?;
    byte_save(&mut writer)?;
    assert_eq!(writer.as_slice().len(), 5);
    let DeferredMutation::GeneratedContent { request } = &writer.as_slice()[0] else {
        panic!()
    };
    assert!(
        matches!(&request.operation,GeneratedOperation3D::Replace {geometry,..} if geometry.vertex_bytes.len()==36)
    );
    Ok(())
}
