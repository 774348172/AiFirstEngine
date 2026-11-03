use project_game_sdk::{DeferredMutationWriter, EntityHandle, Transform3D};
#[test]
fn typed_spatial_bulk_rejects_invalid_tail_and_preserves_raw_generation_zero() {
    use project_game_sdk::SpatialEntityHandle;
    use project_runtime_sdk::spatial_bulk::{SpatialEntityKey, MAX_SPATIAL_ITEMS};
    let key = SpatialEntityKey {
        world_epoch: 9,
        index: 1,
        generation: 0,
    };
    let handle = SpatialEntityHandle::from_runtime_key(key).unwrap();
    assert_eq!(handle.runtime_key(), key);
    assert!(SpatialEntityHandle::from_runtime_key(SpatialEntityKey {
        world_epoch: 0,
        ..key
    })
    .is_err());
    let mut writer = DeferredMutationWriter::default();
    writer
        .write_spatial_transforms3d(&[(handle, Transform3D::identity())])
        .unwrap();
    for bad in [
        Transform3D {
            scale: [0.; 3],
            ..Transform3D::identity()
        },
        Transform3D {
            translation: [f32::NAN, 0., 0.],
            ..Transform3D::identity()
        },
    ] {
        assert!(writer
            .write_spatial_transforms3d(&[(handle, Transform3D::identity()), (handle, bad)])
            .is_err());
    }
    assert!(writer
        .write_spatial_transforms3d(&vec![
            (handle, Transform3D::identity());
            MAX_SPATIAL_ITEMS + 1
        ])
        .is_err());
    assert_eq!(writer.into_inner().len(), 1);
}
#[test]
fn spatial_bulk_writer_validates_whole_input_before_appending() {
    let live = EntityHandle::new("cube", 4294967297).unwrap();
    let mut writer = DeferredMutationWriter::default();
    writer
        .write_local_transforms3d(&[(live.clone(), Transform3D::identity())])
        .unwrap();
    let mut bad = Transform3D::identity();
    bad.scale = [0.; 3];
    assert!(writer
        .write_local_transforms3d(&[(live.clone(), Transform3D::identity()), (live, bad)])
        .is_err());
    assert_eq!(writer.into_inner().len(), 1);
}
#[test]
fn spatial_project_uses_high_level_batch_teleport_anchor_reparent_and_mode() {
    let live = EntityHandle::new("cube", 4294967297).unwrap();
    let parent = EntityHandle::new("parent", 8589934593).unwrap();
    let mut w = DeferredMutationWriter::default();
    w.write_local_transforms3d(&[
        (live.clone(), Transform3D::identity()),
        (parent.clone(), Transform3D::identity()),
    ])
    .unwrap();
    w.teleport3d(&live, Transform3D::identity()).unwrap();
    w.set_spatial_interpolation(&live, false).unwrap();
    w.set_world_anchor3d(&live, [1e12, 0., 0.], false).unwrap();
    w.reparent3d(&live, Some(&parent), true).unwrap();
    assert_eq!(w.into_inner().len(), 6);
}

#[test]
fn cell_writer_rejects_bad_load_and_migration_without_appending_partial_intents() {
    use project_game_sdk::SpatialAssetRef;
    let prefab = SpatialAssetRef {
        id: "cell-a".into(),
        asset_type: "prefab".into(),
        guid: Some("cell-a".into()),
        sub_asset: None,
    };
    let mut writer = DeferredMutationWriter::default();
    assert!(writer
        .load_cell(0, "a", prefab.clone(), None, true)
        .is_err());
    assert!(writer.load_cell(1, "", prefab.clone(), None, true).is_err());
    let ticket = writer.load_cell(1, "a", prefab, None, true).unwrap();
    assert!(writer
        .migrate_cell_subtree(ticket, 2, &EntityHandle::new("node", 7).unwrap(), "", None)
        .is_err());
    assert_eq!(writer.into_inner().len(), 1);
}
#[test]
fn cell_ticket_can_only_query_its_session_request_and_release_is_deferred() {
    let mut writer = DeferredMutationWriter::default();
    let prefab = project_game_sdk::SpatialAssetRef {
        id: "cell-a".into(),
        asset_type: "prefab".into(),
        guid: Some("cell-a".into()),
        sub_asset: None,
    };
    let ticket = writer
        .load_cell(42, "placement", prefab, None, true)
        .unwrap();
    assert_eq!(ticket.query().request_id, Some(42));
    assert!(ticket.query().source_entity_id.is_none());
    writer.release_cell(ticket);
    assert_eq!(writer.into_inner().len(), 2);
}

#[test]
fn s4_streaming_sources_reject_bad_tail_duplicate_and_capacity_atomically() {
    use project_game_sdk::StreamingSource3D;
    let owner = EntityHandle::new("world", 7).unwrap();
    let source = StreamingSource3D {
        id: "camera".into(),
        position: [1e9, 0., 0.],
        activation_radius: 32.,
        prefetch_radius: 64.,
        priority: 10,
    };
    let mut writer = DeferredMutationWriter::default();
    writer
        .world_streaming_sources3d(&owner, vec![source.clone()])
        .unwrap();
    let mut invalid = source.clone();
    invalid.id = "bad".into();
    invalid.position[2] = f64::NAN;
    assert!(writer
        .world_streaming_sources3d(&owner, vec![source.clone(), invalid])
        .is_err());
    assert!(writer
        .world_streaming_sources3d(&owner, vec![source.clone(), source.clone()])
        .is_err());
    assert!(writer
        .world_streaming_sources3d(
            &owner,
            (0..17)
                .map(|i| {
                    let mut value = source.clone();
                    value.id = format!("source-{i}");
                    value
                })
                .collect()
        )
        .is_err());
    assert_eq!(writer.into_inner().len(), 1);
}

#[test]
fn s4_promotion_rejects_invalid_source_before_queuing_and_keeps_ticket_correlation() {
    use project_game_sdk::VegetationPromotionFields3D;
    let owner = EntityHandle::new("forest", 7).unwrap();
    let target = EntityHandle::new("tree", 9).unwrap();
    let fields = VegetationPromotionFields3D {
        component_type: "project.tree".into(),
        source_id: "sourceId".into(),
        cluster_key: "cluster".into(),
        cluster_asset_id: "asset".into(),
    };
    let mut writer = DeferredMutationWriter::default();
    assert!(writer
        .promote_vegetation3d(0, &owner, "tree-1", &target, fields.clone())
        .is_err());
    assert!(writer
        .promote_vegetation3d(1, &owner, "", &target, fields.clone())
        .is_err());
    let ticket = writer
        .promote_vegetation3d(1, &owner, "tree-1", &target, fields)
        .unwrap();
    assert_eq!(ticket.query().request_id, Some(1));
    writer.release_cell(ticket);
    assert_eq!(writer.into_inner().len(), 2);
}
