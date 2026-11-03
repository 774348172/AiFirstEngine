//! Terminal failed demand stays deduplicated in the real streaming owner.
use super::*;
use crate::{
    runtime_package::*, runtime_package_builder::*, spatial_contract::*, world_streaming::*,
};
use serde_json::json;

#[test]
fn s4c_missing_cooked_demand_records_failed_once_and_owner_removal_cleans_projection() {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "s4c-stream-failure-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let entity = json!({"schemaVersion":"runtime-entity.v2","id":"owner","name":"Owner","enabled":true,"parentId":null,"siblingOrder":0,"spatial":{"domain":"world3d","transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":[]});
    let doc = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[entity.clone()]});
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&doc).unwrap()).unwrap()
    else {
        panic!()
    };
    let scene: RuntimeScene = scene.into();
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "failed-world",
        "Failed World",
        "1",
    ));
    let mapping = crate::input_mapping::InputMappingAsset::explicit_empty("input.none");
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    input.scenes.push(scene.clone());
    input.assets.push(RuntimePackageSourceAsset::new(
        "main",
        "Main",
        "scene",
        "Scenes/main.json",
        "scenes/main.json",
    ));
    input.prefabs.push(RuntimePackageSourcePrefab{prefab_id:"cell-a".into(),identity:Some(RuntimePackagePrefabIdentity{asset_guid:"cell-a".into(),sub_asset_id:"root".into(),source:"Prefabs/cell-a.json".into(),dependencies:vec![]}),document:json!({"schemaVersion":"runtime-prefab.v2","id":"cell-a","name":"A","rootEntityId":"owner","entities":[entity]})});
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
        &input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{:?}",
        report.diagnostics
    );
    let loaded = load_runtime_package(&root);
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    let package = loaded.value.unwrap();
    let record = package
        .runtime_asset_index
        .record_by_guid("cell-a")
        .unwrap();
    let cooked = package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    std::fs::remove_file(root.join(cooked.path.as_ref().unwrap())).unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    let mut world = crate::scene_loader::load_scene_into_world(&scene)
        .value
        .unwrap();
    let owner = SourceEntityId::from("owner");
    let binding = WorldStreaming3D {
        schema_version: "world-streaming3d.v1".into(),
        world_id: "failed-world".into(),
        revision: 1,
        cells: vec![WorldCell3D {
            id: "a".into(),
            prefab: SpatialAssetRef(RuntimeAssetRef {
                id: "cell-a".into(),
                asset_type: "prefab".into(),
                guid: Some("cell-a".into()),
                sub_asset: Some("root".into()),
            }),
            min: [-1.; 3],
            max: [1.; 3],
            dependencies: vec![],
            render_required: false,
        }],
        sources: vec![StreamingSource3D {
            id: "player".into(),
            position: [0.; 3],
            activation_radius: 1.,
            prefetch_radius: 2.,
            priority: 0,
        }],
        max_resident_cells: 1,
        max_loading_cells: 1,
        unload_margin: 3.,
    };
    let ty = ComponentTypeId::new("engine.world_streaming3d");
    world
        .try_insert_component_value(
            owner.clone(),
            ty.clone(),
            ComponentValue::Dynamic {
                component_type: ty.clone(),
                value: crate::character_animation::runtime_json(
                    serde_json::to_value(binding).unwrap(),
                ),
            },
        )
        .unwrap();
    let mut manager = RenderResourceManager::new();
    let mut physics = PhysicsModule::default();
    let mut nav = NavigationModule::default();
    let mut frame = 0;
    let start = std::time::Instant::now();
    loop {
        frame += 1;
        loader.advance_world_streaming(
            &mut world,
            &mut manager,
            frame,
            &mut physics,
            &mut nav,
            #[cfg(feature = "real-wgpu")]
            None,
        );
        let state = loader.streaming_state(&owner, &world);
        if let Some(ComponentValue::Dynamic { value, .. }) = state {
            let state = crate::character_animation::dynamic_json(&value);
            if state["failed"]["a"].is_string() {
                assert_eq!(state["cells"], json!({}));
                break;
            }
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "terminal failed demand must be projected"
        );
        std::thread::yield_now();
    }
    let dispatched = loader
        .asset_loader()
        .streaming_snapshot()
        .dispatched_operations;
    assert_eq!(dispatched, 1);
    for _ in 0..20 {
        frame += 1;
        loader.advance_world_streaming(
            &mut world,
            &mut manager,
            frame,
            &mut physics,
            &mut nav,
            #[cfg(feature = "real-wgpu")]
            None,
        );
    }
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .dispatched_operations,
        dispatched,
        "failed demand must not re-request unchanged source"
    );
    assert!(!loader.streaming_can_enter(&owner, [0.; 3], &world));
    assert_eq!(loader.cell_lifetime_counts(), (0, 0));
    world.try_remove_component_value(&owner, &ty).unwrap();
    frame += 1;
    loader.advance_world_streaming(
        &mut world,
        &mut manager,
        frame,
        &mut physics,
        &mut nav,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert!(loader.streaming_state(&owner, &world).is_none());
    assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    let s = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (
            s.pending_operations,
            s.pending_tickets,
            s.reserved_bytes,
            s.cpu_resident_bytes
        ),
        (0, 0, 0, 0)
    );
}
