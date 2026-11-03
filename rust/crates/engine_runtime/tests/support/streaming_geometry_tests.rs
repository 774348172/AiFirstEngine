//! Same private parser as production, plus its public asynchronous consumer.
#[path = "../../../engine_runtime_core/src/runtime_asset_loader/streaming_geometry.rs"]
mod geometry_decode;
#[path = "../../../engine_runtime_core/src/runtime_asset_loader/streaming_prefab.rs"]
mod prefab_decode;
use engine_runtime::{physics_3d::*, render_contract::*, runtime_asset_loader::*};

const CEILING: usize = 32 * 1024 * 1024;

#[test]
fn bounded_prefab_preserves_entities_and_rejects_invalid_tree_or_budget() {
    let entities: Vec<_> = (0..2048)
        .map(|i| serde_json::json!({
            "schemaVersion":"runtime-entity.v2", "id":format!("e{i}"),
            "name":format!("entity{i}"), "enabled":true,
            "parentId":if i == 0 { None } else { Some("e0") }, "siblingOrder":i,
            "spatial": {
                "domain":"world3d", "transform": {
                    "translation":[0.0,0.0,0.0], "rotationXyzw":[0.0,0.0,0.0,1.0], "scale":[1.0,1.0,1.0]
                },
                "meshRenderer": {
                    "schemaVersion":"mesh-renderer3d.v1", "visible":true, "layers":["world"],
                    "mesh":{"id":"mesh", "type":"mesh", "guid":"mesh-guid"},
                    "materials":{"surface":{"id":"material", "type":"material", "guid":"material-guid"}}
                }
            },
            "components":[{"componentType":"project.stats", "data":{"flags":[true,12],"meta":{"text":"abc"}}}]
        }))
        .collect();
    let input = serde_json::json!({
        "schemaVersion":"runtime-prefab.v2", "id":"prefab", "name":"prefab",
        "rootEntityId":"e0", "entities":entities
    });
    let bytes = serde_json::to_vec(&input).unwrap();
    assert!(bytes.len() * 64 > CEILING);
    let (prefab, charge) = prefab_decode::decode_prefab(&bytes, CEILING).unwrap();
    assert_eq!(prefab.entities.len(), 2048);
    assert_eq!(prefab.entities[2047].id, "e2047");
    assert_eq!(prefab.entities[2047].parent_id.as_deref(), Some("e0"));
    assert_eq!(
        prefab.entities[0].components[0].data,
        input["entities"][0]["components"][0]["data"]
    );
    assert!(
        charge
            >= prefab.entities.capacity()
                * std::mem::size_of::<engine_runtime::runtime_package::RuntimeEntity>()
    );
    assert!(charge <= CEILING);
    assert!(matches!(
        prefab_decode::decode_prefab(&bytes, charge - 1),
        Err(StreamingError::BudgetExceeded)
    ));
    let text = String::from_utf8(bytes).unwrap();
    for invalid in [
        text.replacen("runtime-prefab.v2", "runtime-prefab.v3", 1),
        text.replacen("\"entities\":[", "\"unexpected\":0,\"entities\":[", 1),
        text.replacen("\"entities\":[", "\"entities\":[],\"entities\":[", 1),
        text.replacen("\"id\":\"e0\"", "\"id\":\"e0\",\"id\":\"e0\"", 1),
        text.replacen("\"parentId\":\"e0\"", "\"parentId\":\"missing\"", 1),
        text.replacen("\"parentId\":\"e0\"", "\"parentId\":\"e1\"", 1),
        text.replacen("\"rootEntityId\":\"e0\"", "\"rootEntityId\":\"e1\"", 1),
        text.replacen("\"text\":\"abc\"", "\"text\":\"abc\",\"text\":\"def\"", 1),
    ] {
        assert_ne!(invalid, text, "negative fixture must change input");
        assert!(prefab_decode::decode_prefab(invalid.as_bytes(), CEILING).is_err());
    }
}

#[test]
#[ignore = "explicit original437 BM10 package; prefab async admission only"]
fn bounded_prefab_original_bm10_assets_use_the_real_async_worker() {
    use engine_runtime::{runtime_asset::*, runtime_package::*, spatial_contract::*};
    eprintln!(
        "prefab inline sizes: entity={}, document={}, spatial={}",
        std::mem::size_of::<RuntimeEntity>(),
        std::mem::size_of::<RuntimeEntityDocument>(),
        std::mem::size_of::<RuntimeEntityV2>()
    );
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let mut loader = RuntimeAssetLoader::new(
        &package.package_dir,
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    );
    assert_eq!(loader.streaming_budget().max_decode_bytes, CEILING);
    for (variant, count) in [6863, 6202, 6202, 6202, 6201, 6201, 6201, 6201, 6201]
        .into_iter()
        .enumerate()
    {
        let id = format!("bm10-k2-{variant}");
        let record = package.runtime_asset_index.record_by_guid(&id).unwrap();
        assert!(loader.mount_bundle(&record.bundle_id));
        let handle = loader
            .request_async(&RuntimeAssetRef {
                id: id.clone(),
                asset_type: "prefab".into(),
                guid: Some(record.asset_guid.clone()),
                sub_asset: None,
            })
            .unwrap();
        let start = std::time::Instant::now();
        while loader.get_handle_state(&handle) == Some(RuntimeAssetLoadState::Loading) {
            loader.poll_async();
            assert!(start.elapsed().as_secs() < 20, "{id} timeout");
            std::thread::yield_now();
        }
        assert_eq!(
            loader.get_handle_state(&handle),
            Some(RuntimeAssetLoadState::Ready),
            "{id}: {:?}",
            loader.streaming_error(&handle)
        );
        let payload = loader.decoded_payload(&handle).unwrap();
        let RuntimeDecodedPayload::Prefab(prefab) = payload.as_ref() else {
            panic!("prefab typed payload missing")
        };
        assert_eq!(prefab.entities.len(), count);
        assert_eq!(prefab.root_entity_id.as_deref(), Some("root"));
        eprintln!("original {id}: {count} committed typed entities ready");
        drop(payload);
        loader.release(&handle).unwrap();
    }
    assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn bounded_character_influences_preserve_data_validation_and_budget() {
    use engine_runtime::character_animation::*;
    let skin = CharacterSkin {
        mesh_joint: 0,
        mesh_asset_id: "mesh".into(),
        joints: vec![0],
        inverse_bind: vec![glam::Mat4::IDENTITY.to_cols_array()],
        influences: vec![
            VertexInfluence {
                joints: [0; 4],
                weights: [1., 0., 0., 0.],
            };
            15000
        ],
        morph_targets: vec![],
        morph_weights: vec![],
    };
    let expected = CharacterAsset {
        schema_version: "character3d.v1".into(),
        asset_id: "character".into(),
        source_digest: format!("sha256:{}", "a".repeat(64)),
        recipe: CHARACTER_RECIPE.into(),
        joints: vec![CharacterJoint {
            key: "root".into(),
            parent: None,
            bind: JointPose::default(),
        }],
        skins: vec![skin.clone(), skin],
        clips: vec![],
    };
    let bytes = serde_json::to_vec(&expected).unwrap();
    assert!(bytes.len() * 64 > CEILING);
    let (actual, charge) = geometry_decode::decode_character(&bytes, CEILING).unwrap();
    assert_eq!(actual, expected);
    let capacity: usize = actual
        .skins
        .iter()
        .map(|s| s.influences.capacity() * std::mem::size_of::<VertexInfluence>())
        .sum();
    assert!(charge >= capacity && charge <= CEILING);
    assert!(matches!(
        geometry_decode::decode_character(&bytes, charge - 1),
        Err(StreamingError::BudgetExceeded)
    ));
    let text = String::from_utf8(bytes).unwrap();
    for invalid in [
        text.replacen("character3d.v1", "character3d.v2", 1),
        text.replacen("\"skins\":[", "\"skins\":[],\"skins\":[", 1),
        text.replacen("\"influences\":[", "\"influences\":[],\"influences\":[", 1),
        text.replacen("\"weights\":[1.0,", "\"weights\":[0.0,", 1),
        text.replacen("\"joints\":[0,0,0,0]", "\"joints\":[1,0,0,0]", 1),
        text.replacen("\"meshJoint\":0", "\"extra\":0,\"meshJoint\":0", 1),
    ] {
        assert_ne!(invalid, text, "negative fixture must change input");
        assert!(geometry_decode::decode_character(invalid.as_bytes(), CEILING).is_err());
    }
    let mut metadata = expected;
    metadata.asset_id = "x".repeat(600_000);
    assert!(matches!(
        geometry_decode::decode_character(&serde_json::to_vec(&metadata).unwrap(), CEILING),
        Err(StreamingError::BudgetExceeded)
    ));
}

#[test]
#[ignore = "explicit original437 BM10 package; character async decode admission only"]
fn bounded_character_original_bm10_assets_use_the_real_async_worker() {
    use engine_runtime::{
        character_animation::CharacterAsset,
        runtime_asset::RuntimeAssetLoadState,
        runtime_package::{load_runtime_package, RuntimeAssetRef},
    };
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let mut loader = RuntimeAssetLoader::new(
        &package.package_dir,
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    );
    assert_eq!(loader.streaming_budget().max_decode_bytes, CEILING);
    for rig in 0..10 {
        let id = format!("rig-{rig:02}.character");
        let record = package
            .runtime_asset_index
            .record_by_guid(&format!("guid-{id}"))
            .expect("original full rig");
        assert!(loader.mount_bundle(&record.bundle_id));
        let handle = loader
            .request_async(&RuntimeAssetRef {
                id: id.clone(),
                asset_type: "character3d".into(),
                guid: Some(record.asset_guid.clone()),
                sub_asset: record.sub_asset_id.clone(),
            })
            .unwrap();
        let started = std::time::Instant::now();
        while loader.get_handle_state(&handle) == Some(RuntimeAssetLoadState::Loading) {
            loader.poll_async();
            assert!(started.elapsed().as_secs() < 20, "{id} worker timeout");
            std::thread::yield_now();
        }
        assert_eq!(
            loader.get_handle_state(&handle),
            Some(RuntimeAssetLoadState::Ready),
            "original {id}: {:?}",
            loader.streaming_error(&handle)
        );
        let bytes = loader.get_asset_bytes(&handle).unwrap();
        assert!(bytes.len() * 64 > CEILING);
        let character: CharacterAsset = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(character.joints.len(), 64);
        assert_eq!(character.skins[0].influences.len(), 15000);
        character.validate().unwrap();
        assert!(loader.decoded_payload(&handle).is_none());
        eprintln!(
            "original {id} ready: {} bytes, 64 joints/15000 influences",
            bytes.len()
        );
        loader.release(&handle).unwrap();
    }
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
#[ignore = "explicit published vegetation source; separates temporary parse peak from retained cache bytes"]
fn bounded_k2_vegetation_validation_drops_parse_peak_before_cache_publication() {
    use engine_runtime::{runtime_asset::*, runtime_package::*};
    fn ready(loader: &mut RuntimeAssetLoader, record: &RuntimeAssetRecord) -> RuntimeAssetHandle {
        assert!(loader.mount_bundle(&record.bundle_id));
        let handle = loader
            .request_async(&RuntimeAssetRef {
                id: record.asset_id.clone(),
                asset_type: record.asset_type.clone(),
                guid: Some(record.asset_guid.clone()),
                sub_asset: None,
            })
            .unwrap();
        let start = std::time::Instant::now();
        while loader.get_handle_state(&handle) == Some(RuntimeAssetLoadState::Loading) {
            assert!(start.elapsed().as_secs() < 10);
            loader.poll_async();
            std::thread::yield_now();
        }
        assert_eq!(
            loader.get_handle_state(&handle),
            Some(RuntimeAssetLoadState::Ready),
            "{}: {:?}",
            record.asset_id,
            loader.streaming_error(&handle)
        );
        handle
    }
    let package = load_runtime_package(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let records: Vec<RuntimeAssetRecord> = serde_json::from_slice(
        &std::fs::read(package.package_dir.join("assets/runtime-asset-index.json")).unwrap(),
    )
    .unwrap();
    let record = records
        .iter()
        .find(|r| r.asset_type == "vegetation3d")
        .unwrap();
    let mut loader = RuntimeAssetLoader::new(
        package.package_dir,
        package.runtime_asset_index,
        package.runtime_asset_mount_table,
    );
    loader
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut dependencies = Vec::new();
    for guid in &record.dependencies {
        dependencies.push(ready(
            &mut loader,
            records.iter().find(|r| &r.asset_guid == guid).unwrap(),
        ));
    }
    let before = loader.streaming_snapshot().cpu_resident_bytes;
    let handle = ready(&mut loader, record);
    assert!(
        loader.decoded_payload(&handle).is_none(),
        "vegetation parser retains no typed payload"
    );
    let bytes = loader.get_asset_bytes(&handle).unwrap();
    let raw = bytes.len();
    let delta = loader.streaming_snapshot().cpu_resident_bytes - before;
    eprintln!("actual vegetation raw={raw}, cache residency delta={delta}; typed payload absent");
    assert_eq!(
        delta, raw,
        "discarded validation DOM must not remain permanently resident"
    );
    drop(bytes);
    loader.release(&handle).unwrap();
    assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, before);
    let mut low = loader.streaming_budget();
    low.max_decode_bytes = 1024 * 1024;
    loader.set_streaming_budget(low).unwrap();
    let rejected = loader
        .request_async(&RuntimeAssetRef {
            id: record.asset_id.clone(),
            asset_type: record.asset_type.clone(),
            guid: Some(record.asset_guid.clone()),
            sub_asset: None,
        })
        .unwrap();
    let start = std::time::Instant::now();
    while loader.get_handle_state(&rejected) == Some(RuntimeAssetLoadState::Loading) {
        assert!(start.elapsed().as_secs() < 10);
        loader.poll_async();
        std::thread::yield_now();
    }
    assert_eq!(
        loader.get_handle_state(&rejected),
        Some(RuntimeAssetLoadState::Failed)
    );
    assert_eq!(
        loader.streaming_error(&rejected),
        Some(&StreamingError::BudgetExceeded)
    );
    loader.release(&rejected).unwrap();
    for handle in dependencies {
        loader.release(&handle).unwrap();
    }
    assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
}

fn mesh() -> CookedRenderAsset {
    CookedRenderAsset {
        source_digest: format!("sha256:{}", "1".repeat(64)),
        recipe: NATIVE_RENDER_COOK_RECIPE.into(),
        asset: RenderAssetData::Mesh(CookedGeometry {
            vertex_bytes: vec![0; 65536 * 12],
            index_bytes: [0u16, 1, 2]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
            vertex_stride: 12,
            attributes: vec![VertexAttribute {
                semantic: VertexSemantic::Position,
                format: VertexFormat::Float32x3,
                offset: 0,
            }],
            vertex_count: 65536,
            index_count: 3,
            index_format: GeometryIndexFormat::Uint16,
            topology: GeometryTopology::TriangleList,
            submeshes: vec![SubmeshRange {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
            }],
            bounds: GeometryBounds {
                min: [0.; 3],
                max: [0.; 3],
                sphere_center: [0.; 3],
                sphere_radius: 0.,
            },
            coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
            material_slots: vec!["surface".into()],
        }),
    }
}

#[test]
fn bounded_geometry_large_mesh_preserves_arrays_schema_and_capacity_charge() {
    let expected = mesh();
    let bytes = serde_json::to_vec(&RenderContract::new(expected.clone())).unwrap();
    assert!(
        bytes.len() * 64 > CEILING,
        "old JSON envelope must reject this input"
    );
    let (actual, charge) = geometry_decode::decode_mesh(&bytes, CEILING).unwrap();
    assert_eq!(actual, expected);
    assert!(geometry_decode::mesh_metadata_copy_bound(&actual).unwrap() <= charge);
    let RenderAssetData::Mesh(geometry) = actual.asset else {
        unreachable!()
    };
    assert!(charge >= geometry.vertex_bytes.capacity() + geometry.index_bytes.capacity());
    assert!(charge <= CEILING);
    assert!(matches!(
        geometry_decode::decode_mesh(&bytes, charge - 1),
        Err(StreamingError::BudgetExceeded)
    ));

    // Payload precedes its kind and schemaVersion comes last, as allowed by the contract.
    let text = String::from_utf8(bytes).unwrap();
    let data: CookedRenderAsset = expected;
    let RenderAssetData::Mesh(geometry) = &data.asset else {
        unreachable!()
    };
    let reordered = format!(
        "{{\"data\":{{\"asset\":{{\"payload\":{},\"kind\":\"mesh\"}},\"recipe\":{},\"sourceDigest\":{}}},\"schemaVersion\":\"render-contract.v1\"}}",
        serde_json::to_string(geometry).unwrap(),
        serde_json::to_string(&data.recipe).unwrap(),
        serde_json::to_string(&data.source_digest).unwrap(),
    );
    assert_eq!(
        geometry_decode::decode_mesh(reordered.as_bytes(), CEILING)
            .unwrap()
            .0,
        data
    );
    for invalid in [
        text.replacen("render-contract.v1", "render-contract.v2", 1),
        text.replacen("\"data\":{", "\"data\":{\"unexpected\":0,", 1),
        text.replacen("\"vertexBytes\":[0,", "\"vertexBytes\":[256,", 1),
        text.replacen("\"vertexStride\":12", "\"vertexStride\":16", 1),
        text.replacen(
            "\"kind\":\"mesh\"",
            "\"kind\":\"mesh\",\"kind\":\"mesh\"",
            1,
        ),
    ] {
        assert!(geometry_decode::decode_mesh(invalid.as_bytes(), CEILING).is_err());
    }
}

#[test]
fn bounded_mesh_copy_metadata_tracks_capacity_without_numeric_json_scratch() {
    let mut asset = mesh();
    let metadata = geometry_decode::mesh_metadata_copy_bound(&asset).unwrap();
    let RenderAssetData::Mesh(geometry) = &mut asset.asset else {
        unreachable!()
    };
    geometry.vertex_bytes = vec![0; 36];
    geometry.vertex_count = 3;
    asset.validate().unwrap();
    assert_eq!(
        geometry_decode::mesh_metadata_copy_bound(&asset),
        Some(metadata)
    );
    let before = asset.recipe.capacity();
    asset.recipe.reserve(4096);
    let growth = asset.recipe.capacity() - before;
    assert_eq!(
        geometry_decode::mesh_metadata_copy_bound(&asset),
        Some(metadata + growth * 4)
    );
}

#[test]
fn bounded_geometry_large_collision_keeps_validation_and_bounded_allocations() {
    let expected = CookedCollision3D {
        schema_version: "collision3d.v1".into(),
        shape: CollisionShape3D::StaticMesh {
            vertices: vec![[0., 0., 0.]; 100_000],
            triangles: vec![[0, 1, 2]; 30_000],
        },
    };
    let bytes = serde_json::to_vec(&expected).unwrap();
    assert!(bytes.len() * 64 > CEILING);
    let (actual, charge) = geometry_decode::decode_collision(&bytes, CEILING).unwrap();
    assert_eq!(actual, expected);
    let CollisionShape3D::StaticMesh {
        vertices,
        triangles,
    } = &actual.shape
    else {
        unreachable!()
    };
    assert!(charge >= (vertices.capacity() + triangles.capacity()) * 12);
    assert!(matches!(
        geometry_decode::decode_collision(&bytes, charge - 1),
        Err(StreamingError::BudgetExceeded)
    ));
    let text = String::from_utf8(bytes).unwrap();
    for invalid in [
        text.replacen("collision3d.v1", "collision3d.v2", 1),
        text.replacen("\"shape\":{", "\"shape\":{\"extra\":0,", 1),
        text.replacen("[0,1,2]", "[0,0,2]", 1),
        text.replacen("[0,1,2]", "[0,1,100000]", 1),
        text.replacen("\"vertices\":[", "\"vertices\":[],\"vertices\":[", 1),
    ] {
        assert!(geometry_decode::decode_collision(invalid.as_bytes(), CEILING).is_err());
    }
}

#[test]
fn bounded_geometry_rejects_metadata_string_and_nesting_before_typed_allocation() {
    let string = format!("\"{}\"", "\\u0061".repeat(CEILING / 64 / 6 + 1));
    assert!(matches!(
        geometry_decode::decode_mesh(string.as_bytes(), CEILING),
        Err(StreamingError::BudgetExceeded)
    ));
    let nesting = format!("{}0{}", "[".repeat(129), "]".repeat(129));
    assert!(matches!(
        geometry_decode::decode_collision(nesting.as_bytes(), CEILING),
        Err(StreamingError::BudgetExceeded)
    ));
    let mut asset = mesh();
    asset.recipe = "x".repeat(600_000);
    let bytes = serde_json::to_vec(&RenderContract::new(asset)).unwrap();
    assert!(matches!(
        geometry_decode::decode_mesh(&bytes, CEILING),
        Err(StreamingError::BudgetExceeded)
    ));
}

#[test]
#[ignore = "explicit original published BM13 package; not a replacement for K2"]
fn bounded_geometry_original_bm13_assets_use_the_real_async_worker() {
    use engine_runtime::{
        runtime_asset::{RuntimeAssetLoadState, RuntimeDecodedPayload},
        runtime_package::*,
    };
    let package = load_runtime_package(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let mut loader = RuntimeAssetLoader::new(
        &package.package_dir,
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    );
    loader
        .set_streaming_budget(StreamingBudget {
            max_asset_bytes: 64 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    for (id, asset_type) in [
        ("bm13-ground-0-fine", "mesh"),
        ("bm13-ground-0-coarse", "mesh"),
        ("bm13-ground-0-collision", "collision3d"),
    ] {
        let record = package
            .runtime_asset_index
            .record_by_guid(&format!("guid-{id}"))
            .unwrap();
        assert!(loader.mount_bundle(&record.bundle_id));
        let handle = loader
            .request_async(&RuntimeAssetRef {
                id: id.into(),
                asset_type: asset_type.into(),
                guid: Some(record.asset_guid.clone()),
                sub_asset: None,
            })
            .unwrap();
        let started = std::time::Instant::now();
        while loader.get_handle_state(&handle) == Some(RuntimeAssetLoadState::Loading) {
            loader.poll_async();
            assert!(
                started.elapsed() < std::time::Duration::from_secs(60),
                "{id}"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            loader.get_handle_state(&handle),
            Some(RuntimeAssetLoadState::Ready),
            "{id}: {:?}",
            loader.streaming_error(&handle)
        );
        if id.ends_with("fine") {
            let payload = loader.decoded_payload(&handle).unwrap();
            let RuntimeDecodedPayload::Render(asset) = payload.as_ref() else {
                panic!("mesh omitted")
            };
            let RenderAssetData::Mesh(geometry) = &asset.asset else {
                panic!("not geometry")
            };
            assert_eq!(
                (geometry.vertex_count, geometry.index_count),
                (327680, 786432)
            );
            assert_eq!(
                (geometry.vertex_bytes.len(), geometry.index_bytes.len()),
                (10485760, 3145728)
            );
        }
        loader.release(&handle).unwrap();
        eprintln!("original async {id} ready in {:?}", started.elapsed());
    }
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
#[ignore = "explicit original BM13 package; cell preparation admission, not GPU or K2"]
fn bounded_geometry_original_bm13_nine_visual_cells_prepare_with_windows_budget() {
    use engine_runtime::{
        render_resource::RenderResourceManager, runtime_instance_loader::*, runtime_package::*,
        world::World,
    };
    let package = load_runtime_package(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let world = World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut manager = RenderResourceManager::new();
    let mut tickets = Vec::new();
    // The last placement shares the already prepared mesh, whose raw JSON has
    // been shed by acquire_render_asset_bytes. It must still be admissible.
    for tile in (0..9).chain(std::iter::once(0)) {
        let id = format!("bm13-v-{tile}");
        let record = package.runtime_asset_index.record_by_guid(&id).unwrap();
        assert!(loader.asset_loader_mut().mount_bundle(&record.bundle_id));
        eprintln!(
            "before {id}: {:?}",
            loader.asset_loader().streaming_snapshot()
        );
        let ticket = loader
            .request_cell(
                CellActivationRequest {
                    schema_version: "cell-activation.v1".into(),
                    placement_id: format!("{id}-{}", tickets.len()),
                    prefab: RuntimeAssetRef {
                        id: id.clone(),
                        asset_type: "prefab".into(),
                        guid: Some(record.asset_guid.clone()),
                        sub_asset: None,
                    },
                    asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                    required_domains: [CellDomain::World, CellDomain::Asset, CellDomain::Render]
                        .into(),
                    parent: None,
                },
                &world,
            )
            .unwrap();
        let started = std::time::Instant::now();
        loop {
            match loader.prepare_cell(ticket, &world, 1, &mut manager) {
                Ok(status) => {
                    assert_eq!(status, CellStatus::Prepared);
                    break;
                }
                Err(CellError::NotReady) => assert!(started.elapsed().as_secs() < 60, "{id}"),
                Err(error) => panic!(
                    "{id}: {error:?}; loader={:?}",
                    loader.asset_loader().streaming_snapshot()
                ),
            }
            std::thread::yield_now();
        }
        tickets.push(ticket);
        eprintln!(
            "prepared {id}: {:?}; logical={:?}",
            loader.asset_loader().streaming_snapshot(),
            manager.report(1).resident_bytes
        );
    }
    assert_eq!(tickets.len(), 10);
    let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(1, 1, 0);
    loader.observe_memory(&mut sample);
    let backing = engine_runtime::resource_memory::memory_totals(&sample.allocations).unwrap();
    let charged = loader
        .asset_loader()
        .streaming_snapshot()
        .cpu_resident_bytes;
    eprintln!(
        "nine cells observed CPU backing={} charged={charged}",
        backing.cpu_allocated
    );
    assert!(
        charged as u64 >= backing.cpu_allocated,
        "known backing must remain charged"
    );
    let mut world = world;
    for ticket in tickets {
        loader
            .release_cell(ticket, &mut world, &mut manager, 2)
            .unwrap();
    }
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
#[ignore = "explicit full K2 package; reproduces the actual ordered multi-domain admission"]
fn bounded_k2_partitioned_roles_fit_windows_residency() {
    use engine_runtime::{
        navigation_3d::NavigationModule, physics_3d::PhysicsModule,
        render_resource::RenderResourceManager, runtime_instance_loader::*, runtime_package::*,
    };
    let package = load_runtime_package(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut manager = RenderResourceManager::new();
    let mut physics = PhysicsModule::default();
    let mut navigation = NavigationModule::default();
    let mut tickets = Vec::new();
    for role in ['p', 'n', 'v', 'a', 'b'] {
        for x in -1_i32..=2 {
            for z in -1_i32..=1 {
                let variant = z.rem_euclid(3) * 3 + x.rem_euclid(3);
                let id = format!("bm13-k2-{role}-{variant}");
                let record = package.runtime_asset_index.record_by_guid(&id).unwrap();
                assert!(loader.asset_loader_mut().mount_bundle(&record.bundle_id));
                let domain = match role {
                    'p' => CellDomain::Physics,
                    'n' => CellDomain::Navigation,
                    _ => CellDomain::Render,
                };
                eprintln!(
                    "before {id}: {:?}",
                    loader.asset_loader().streaming_snapshot()
                );
                let ticket = loader
                    .request_cell(
                        CellActivationRequest {
                            schema_version: "cell-activation.v1".into(),
                            placement_id: format!("k2/{role}/{x}/{z}"),
                            prefab: RuntimeAssetRef {
                                id: id.clone(),
                                asset_type: "prefab".into(),
                                guid: Some(record.asset_guid.clone()),
                                sub_asset: None,
                            },
                            asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                            required_domains: [CellDomain::World, CellDomain::Asset, domain].into(),
                            parent: Some(format!("k2-parent-{x}-{z}")),
                        },
                        &world,
                    )
                    .unwrap();
                let start = std::time::Instant::now();
                loop {
                    match loader.prepare_cell_with_simulation(
                        ticket,
                        &world,
                        1,
                        &mut manager,
                        &mut physics,
                        &mut navigation,
                    ) {
                        Ok(status) => {
                            if matches!(role, 'v' | 'a' | 'b') {
                                assert_eq!(status, CellStatus::Prepared);
                            } else {
                                assert_eq!(status, CellStatus::ReadyToCommit);
                                loader
                                    .commit_cell_with_simulation(
                                        ticket,
                                        &mut world,
                                        &mut physics,
                                        &mut navigation,
                                    )
                                    .unwrap();
                            }
                            break;
                        }
                        Err(CellError::NotReady) => assert!(start.elapsed().as_secs() < 30, "{id}"),
                        Err(error) => panic!(
                            "{id}: {error:?}; {:?}",
                            loader.asset_loader().streaming_snapshot()
                        ),
                    }
                    std::thread::yield_now();
                }
                tickets.push(ticket);
                eprintln!(
                    "prepared {id}: {:?}",
                    loader.asset_loader().streaming_snapshot()
                );
            }
        }
        if role == 'p' {
            for x in [15.9375, 16.0625, 31.9375, 32.0625] {
                let hit = physics
                    .raycast(&engine_runtime::physics_3d::PhysicsRay3D {
                        origin: [x, 5., 8.],
                        displacement: [0., -10., 0.],
                    })
                    .unwrap()
                    .unwrap();
                eprintln!("K2 actual collision x={x}: {hit:?}");
            }
        }
    }
    assert_eq!(tickets.len(), 60);
    for ticket in tickets {
        loader
            .release_cell_with_simulation(
                ticket,
                &mut world,
                &mut manager,
                2,
                &mut physics,
                &mut navigation,
            )
            .unwrap();
    }
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
#[ignore = "explicit original437 BM10 package; complete closure and CPU Prepare, not GPU ACK or K2"]
fn bounded_bm10_original_cell_prepares_with_windows_budget() {
    use engine_runtime::{
        navigation_3d::NavigationModule, physics_3d::PhysicsModule,
        render_resource::RenderResourceManager, runtime_instance_loader::*, runtime_package::*,
    };
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut manager = RenderResourceManager::new();
    let mut physics = PhysicsModule::default();
    let mut navigation = NavigationModule::default();
    let id = "bm10-k2-0";
    let record = package.runtime_asset_index.record_by_guid(id).unwrap();
    assert!(loader.asset_loader_mut().mount_bundle(&record.bundle_id));
    let ticket = loader
        .request_cell(
            CellActivationRequest {
                schema_version: "cell-activation.v1".into(),
                placement_id: "bm10/cpu-prepare/0/0".into(),
                prefab: RuntimeAssetRef {
                    id: id.into(),
                    asset_type: "prefab".into(),
                    guid: Some(record.asset_guid.clone()),
                    sub_asset: None,
                },
                asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                required_domains: [
                    CellDomain::World,
                    CellDomain::Asset,
                    CellDomain::Render,
                    CellDomain::Physics,
                    CellDomain::Navigation,
                ]
                .into(),
                parent: Some("k2-parent-0-0".into()),
            },
            &world,
        )
        .unwrap();
    let start = std::time::Instant::now();
    let result = loop {
        let result = loader.prepare_cell_with_simulation(
            ticket,
            &world,
            1,
            &mut manager,
            &mut physics,
            &mut navigation,
        );
        if result != Err(CellError::NotReady) || start.elapsed().as_secs() >= 60 {
            break result;
        }
        std::thread::yield_now();
    };
    eprintln!(
        "original {id} CPU Prepare={result:?}; elapsed={:?}; snapshot={:?}",
        start.elapsed(),
        loader.asset_loader().streaming_snapshot()
    );
    let backing = if result == Ok(CellStatus::Prepared) {
        let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(1, 1, 0);
        loader.observe_memory(&mut sample);
        let known = engine_runtime::resource_memory::memory_totals(&sample.allocations)
            .unwrap()
            .cpu_allocated;
        let charged = loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes;
        eprintln!("prepared known CPU backing={known}, charged={charged}");
        Some((known, charged))
    } else {
        None
    };
    let released = loader.release_cell_with_simulation(
        ticket,
        &mut world,
        &mut manager,
        2,
        &mut physics,
        &mut navigation,
    );
    let after = loader.asset_loader().streaming_snapshot();
    let stopped = loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2));
    eprintln!("release={released:?}; after={after:?}; workerStopped={stopped}");
    assert_eq!(result, Ok(CellStatus::Prepared));
    let (known, charged) = backing.unwrap();
    assert!(charged as u64 >= known, "known backing must remain charged");
    released.unwrap();
    assert_eq!(after.cpu_resident_bytes, 0);
    assert!(stopped);
}

#[test]
#[ignore = "explicit original BM10 package and real GPU; bounded original nine-cell admission diagnosis"]
fn bounded_bm10_original_project_neighborhood_reaches_active() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        project_runtime_module::ProjectRuntimeBootstrap,
        project_runtime_native_adapter::linked_project_runtime_set_from_dll,
        project_runtime_session::ProjectRuntimeSessionReportLevel,
        runtime_instance_loader::RuntimeInstanceLoader,
        runtime_package::load_runtime_package,
        wgpu_backend::real::RealWgpuBackend,
        world::World,
    };
    use std::time::{Duration, Instant};
    let package_path =
        std::path::PathBuf::from(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(&package_path).value.unwrap();
    let module = package_path
        .parent()
        .unwrap()
        .join("bin/sample_371_bm10_k2_runtime.dll");
    let linked = linked_project_runtime_set_from_dll(&module).unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let start = Instant::now();
    let mut active = false;
    let mut active_at = None;
    let mut terminal_fault = false;
    for frame in 1..=3000 {
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        if frame % 120 == 0 || receipts.values().any(|r| r.error_code.is_some()) {
            eprintln!("first-cell frame={frame} elapsed={:?}; receipts={receipts:?}; streaming={:?}; uploads={:?}",
                start.elapsed(), loader.asset_loader().streaming_snapshot(), backend.geometry_upload_snapshot());
        }
        if receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9
        {
            active = true;
            active_at.get_or_insert(frame);
        }
        terminal_fault = output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|r| r.terminal_fault);
        if terminal_fault {
            eprintln!(
                "terminal project fault frame={frame}: {:?}",
                output.project_runtime_session_report
            );
        }
        if terminal_fault
            || active_at.is_some_and(|first| frame >= first + 120)
            || start.elapsed() > Duration::from_secs(120)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "terminal receipts={:?}; streaming={:?}",
        loader.project_cell_snapshot(&world).receipts,
        loader.asset_loader().streaming_snapshot()
    );
    if !active || terminal_fault {
        let mut printed = 0;
        for id in world.query_entities(&engine_runtime::query::QuerySpec::all([
            engine_runtime::components::ComponentTypeId::new("engine.navigation_agent3d"),
        ])) {
            if let Some(state) = world.navigation3d_state(&id).filter(|s| {
                s.revision > 0
                    && s.status == engine_runtime::navigation_3d::NavigationStatus3D::NotResident
            }) {
                let pose = world.spatial_world(&id).map(|s| {
                    std::array::from_fn::<_, 3, _>(|i| {
                        s.anchor[i] + f64::from(s.world_matrix[12 + i])
                    })
                });
                eprintln!("not-resident witness id={id:?} pose={pose:?} state={state:?} control={:?} agent={:?}", world.component_value(&id,&engine_runtime::components::ComponentTypeId::new("engine.navigation3d_control")),world.component_value(&id,&engine_runtime::components::ComponentTypeId::new("engine.navigation_agent3d")));
                printed += 1;
                if printed == 1000 {
                    break;
                }
            }
        }
    }
    let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(1, 1, 0);
    loader.observe_memory(&mut sample);
    let known = engine_runtime::resource_memory::memory_totals(&sample.allocations)
        .unwrap()
        .cpu_allocated;
    let charged = loader
        .asset_loader()
        .streaming_snapshot()
        .cpu_resident_bytes;
    eprintln!("neighborhood known loader backing={known}, charged={charged}");
    if active {
        assert!(
            charged as u64 >= known,
            "retained known backing must remain charged"
        );
    }
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(2)).unwrap();
    assert!(active && !terminal_fault,
        "original project neighborhood must become Active and survive120 further ticks without terminal fault");
}

#[test]
fn async_mesh_upgrades_sync_cache_without_losing_existing_refs() {
    use engine_runtime::{
        canonical_digest::sha256_prefixed, runtime_asset::*, runtime_package::RuntimeAssetRef,
    };
    let root = std::env::temp_dir().join(format!(
        "aife-sync-mesh-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut expected = mesh();
    let RenderAssetData::Mesh(geometry) = &mut expected.asset else {
        unreachable!()
    };
    geometry.vertex_bytes.truncate(36);
    geometry.vertex_count = 3;
    let bytes = serde_json::to_vec(&RenderContract::new(expected.clone())).unwrap();
    std::fs::write(root.join("mesh.json"), &bytes).unwrap();
    let record: RuntimeAssetRecord = serde_json::from_value(serde_json::json!({
        "assetGuid":"guid-mesh", "assetId":"mesh", "type":"mesh", "cookedAssetId":"cooked-mesh",
        "bundleId":"startup", "loaderKind":"mesh", "hash":sha256_prefixed(&bytes), "size":bytes.len()
    })).unwrap();
    let cooked: CookedAssetRecord = serde_json::from_value(serde_json::json!({
        "cookedAssetId":"cooked-mesh", "bundleId":"startup", "path":"mesh.json",
        "hash":sha256_prefixed(&bytes), "size":bytes.len()
    }))
    .unwrap();
    let mut loader = RuntimeAssetLoader::new(
        &root,
        RuntimeAssetIndex::new(vec![record], vec![cooked]),
        RuntimePackageMountTable::new(vec![BundleRecord {
            bundle_id: "startup".into(),
            mount_id: None,
            uri: "startup".into(),
            hash: None,
            version: None,
            mounted: true,
        }]),
    );
    let reference = RuntimeAssetRef {
        id: "mesh".into(),
        asset_type: "mesh".into(),
        guid: Some("guid-mesh".into()),
        sub_asset: None,
    };
    let sync = loader.load(&reference).unwrap();
    assert!(
        loader.decoded_payload(&sync).is_none(),
        "synchronous mesh cache contains no typed geometry"
    );
    let normal_budget = loader.streaming_budget();
    let mut low = normal_budget;
    low.max_asset_bytes = bytes.len();
    low.max_in_flight_bytes = bytes.len();
    loader.set_streaming_budget(low).unwrap();
    assert_eq!(
        loader.request_async(&reference),
        Err(StreamingError::BudgetExceeded),
        "upgrade must reserve real worker bytes even when the identity is cached"
    );
    assert_eq!(
        loader.get_handle_state(&sync),
        Some(RuntimeAssetLoadState::Ready)
    );
    assert_eq!(loader.streaming_snapshot().pending_operations, 0);
    loader.set_streaming_budget(normal_budget).unwrap();
    let first = loader.request_async(&reference).unwrap();
    let joined = loader.request_async(&reference).unwrap();
    let start = std::time::Instant::now();
    while [&first, &joined]
        .iter()
        .any(|h| loader.get_handle_state(h) == Some(RuntimeAssetLoadState::Loading))
    {
        assert!(start.elapsed().as_secs() < 3);
        loader.poll_async();
        std::thread::yield_now();
    }
    let payload = loader.decoded_payload(&first);
    let snapshot = loader.streaming_snapshot();
    loader.release(&first).unwrap();
    assert_eq!(
        loader.get_handle_state(&sync),
        Some(RuntimeAssetLoadState::Ready)
    );
    assert!(loader.get_asset_bytes(&sync).is_some());
    loader.release(&joined).unwrap();
    assert_eq!(
        loader.decoded_cache_len(),
        1,
        "original synchronous ref must survive async releases"
    );
    loader.release(&sync).unwrap();
    assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
    std::fs::remove_dir_all(&root).unwrap();
    let payload =
        payload.expect("async cache hit must publish typed geometry after a synchronous load");
    let RuntimeDecodedPayload::Render(actual) = payload.as_ref() else {
        panic!("mesh payload missing")
    };
    assert_eq!(actual, &expected);
    assert_eq!(
        snapshot.dispatched_operations, 1,
        "joining consumers share one upgrade worker operation"
    );
    assert_eq!(snapshot.pending_operations, 0);
    assert_eq!(snapshot.pending_tickets, 0);
}

#[test]
fn cell_sequential_mesh_scratch_keeps_all_copies_and_enforces_real_budget() {
    use engine_runtime::{
        render_resource::RenderResourceManager, runtime_instance_loader::*, world::World,
    };
    let fixture = super::streamed_cells::build_repeated_mesh_cell(16);
    for should_prepare in [false, true] {
        let mut world = World::new();
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let mut manager = RenderResourceManager::new();
        let ticket = loader
            .request_cell(super::streamed_cells::request("a", 1, true), &world)
            .unwrap();
        let start = std::time::Instant::now();
        while loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations
            != 0
        {
            loader.asset_loader_mut().poll_async();
            assert!(start.elapsed().as_secs() < 5);
            std::thread::yield_now();
        }
        let mut budget = loader.asset_loader().streaming_budget();
        let limit = if should_prepare {
            32 * 1024 * 1024
        } else {
            loader
                .asset_loader()
                .streaming_snapshot()
                .cpu_resident_bytes
        };
        budget.max_cpu_resident_bytes = limit;
        budget.max_asset_bytes = budget.max_asset_bytes.min(limit);
        budget.max_decode_bytes = budget.max_decode_bytes.min(limit);
        budget.max_in_flight_bytes = budget.max_in_flight_bytes.min(limit);
        loader
            .asset_loader_mut()
            .set_streaming_budget(budget)
            .unwrap();
        let result = loader.prepare_cell(ticket, &world, 1, &mut manager);
        eprintln!(
            "16 meshes limit={limit} Prepare={result:?}; {:?}",
            loader.asset_loader().streaming_snapshot()
        );
        assert!(
            world.entity_ids().is_empty(),
            "Prepare must remain invisible"
        );
        if should_prepare {
            assert_eq!(result, Ok(CellStatus::Prepared));
            let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(1, 1, 0);
            loader.observe_memory(&mut sample);
            let known = engine_runtime::resource_memory::memory_totals(&sample.allocations)
                .unwrap()
                .cpu_allocated;
            assert!(
                known >= 16 * 65_536 * 12,
                "every renderer retains its own geometry"
            );
            assert!(
                loader
                    .asset_loader()
                    .streaming_snapshot()
                    .cpu_resident_bytes as u64
                    >= known
            );
            loader
                .release_cell(ticket, &mut world, &mut manager, 2)
                .unwrap();
        } else {
            assert_eq!(
                result,
                Err(CellError::Asset(StreamingError::BudgetExceeded))
            );
            assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Failed));
        }
        assert_eq!(
            loader
                .asset_loader()
                .streaming_snapshot()
                .cpu_resident_bytes,
            0
        );
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(std::time::Duration::from_secs(2)));
    }
}

#[test]
fn bounded_material_copy_metadata_tracks_capacity_without_decoder_scratch() {
    let mut asset: CookedRenderAsset = serde_json::from_value(serde_json::json!({
        "sourceDigest":format!("sha256:{}","1".repeat(64)),"recipe":NATIVE_RENDER_COOK_RECIPE,
        "asset":{"kind":"material","payload":{
            "preset":"unlit","parameters":{"baseColor":{"type":"vector4","value":[1.,1.,1.,1.]}},
            "textures":{},"blend":"opaque","cull":"back","depth":"testAndWrite",
            "shaderSource":UNLIT_SHADER_SOURCE
        }}
    }))
    .unwrap();
    asset.validate().unwrap();
    let before = geometry_decode::material_metadata_copy_bound(&asset).unwrap();
    let parser_bound = serde_json::to_vec(&RenderContract::new(asset.clone()))
        .unwrap()
        .len()
        * 64
        * 4;
    assert!(
        before < parser_bound,
        "copy metadata must exclude discarded parser scratch"
    );
    let RenderAssetData::Material(material) = &mut asset.asset else {
        unreachable!()
    };
    let old = material.shader_source.capacity();
    material.shader_source.reserve(4096);
    let growth = material.shader_source.capacity() - old;
    assert_eq!(
        geometry_decode::material_metadata_copy_bound(&asset),
        Some(before + 4 * growth)
    );
    assert!(geometry_decode::material_metadata_copy_bound(&mesh()).is_none());
}

#[test]
#[ignore = "explicit440 package and authored input; exact captured path and layout oracle only"]
fn bm10_k2_captured_navigation_and_layout_oracle() {
    use engine_navigation_native::{NativeNav, NavError};
    use engine_runtime::{navigation_3d::CookedNavigation3D, runtime_package::*};
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let input: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("AIFE_BM10_NAV_ORACLE").unwrap()).unwrap(),
    )
    .unwrap();
    let record = package
        .runtime_asset_index
        .record_by_guid("guid-bm10-k2-navigation")
        .unwrap();
    let cooked = package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    let asset: CookedNavigation3D = serde_json::from_slice(
        &std::fs::read(package.package_dir.join(cooked.path.as_ref().unwrap())).unwrap(),
    )
    .unwrap();
    let extent = [
        asset.profile.radius.max(0.25),
        asset.profile.height,
        asset.profile.radius.max(0.25),
    ];
    let mut nav = NativeNav::new(asset.profile.tile_width).unwrap();
    for x in -1..=1 {
        for z in -1..=1 {
            for tile in &asset.tiles {
                let bytes = engine_navigation_native::translated_tile(
                    &tile.data,
                    [x as f32 * 128., 0., z as f32 * 128.],
                    [x * 4, z * 4],
                )
                .unwrap();
                nav.add_tile(&bytes).unwrap();
            }
        }
    }
    let from = [-89.5, 0.85, -94.4];
    let to = [-163., 0., -94.4];
    assert!(
        nav.find_path(from, to, extent).is_ok(),
        "captured origin is resident before obstacles"
    );
    for revised in [false, true] {
        let boxes: Vec<_> = input["obstacles"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, obstacle)| {
                let c: [f32; 2] =
                    std::array::from_fn(|j| obstacle["coord"][j].as_i64().unwrap() as f32 * 128.);
                let mut p: [f32; 3] =
                    std::array::from_fn(|j| obstacle["position"][j].as_f64().unwrap() as f32);
                if revised {
                    p = std::array::from_fn(|j| {
                        input["revisedObstacles"][i]["position"][j]
                            .as_f64()
                            .unwrap() as f32
                    });
                }
                p[0] += c[0];
                p[2] += c[1];
                std::array::from_fn(|j| {
                    let k = j % 3;
                    let h = obstacle["halfExtents"][k].as_f64().unwrap() as f32;
                    p[k] + if j < 3 { -h } else { h }
                })
            })
            .collect();
        nav.set_obstacles(&boxes).unwrap();
        let captured = nav.find_path(from, to, extent);
        eprintln!("revised={revised} captured result={captured:?}");
        if !revised {
            assert_eq!(captured, Err(NavError::NotResident));
            continue;
        }
        let mut counts = [0; 3];
        for agent in input["agents"].as_array().unwrap() {
            let i = agent["index"].as_u64().unwrap() as usize;
            let c: [f32; 2] =
                std::array::from_fn(|j| agent["coord"][j].as_i64().unwrap() as f32 * 128.);
            let z = -24.5 + ((i / 5) % 50) as f32;
            let from = [
                c[0] + if i < 500 {
                    -24. + (i % 5) as f32 * 1.5
                } else {
                    18. + (i % 5) as f32 * 1.5
                },
                0.85,
                c[1] + z,
            ];
            for class in 0..3 {
                let to = match class {
                    0 => [c[0] + if i < 500 { 22. } else { -22. }, 0., c[1] + z],
                    1 => [c[0] + 56., 0., c[1] + 56.],
                    _ => [c[0] + 10000., 0., c[1] + 10000.],
                };
                let path = nav.find_path(from, to, extent);
                match class {
                    0 => assert!(
                        path.is_ok(),
                        "agent={i} from={from:?} target={to:?} {path:?}"
                    ),
                    1 => assert_eq!(path, Err(NavError::Unreachable), "agent={i}"),
                    _ => assert_eq!(path, Err(NavError::NotResident), "agent={i}"),
                }
                counts[class] += 1;
            }
        }
        assert_eq!(counts, [999; 3]);
        eprintln!("real144 translated tiles /50 obstacles /999 authored cohort origins each three classes={counts:?}");
    }
}

#[test]
#[ignore = "explicit BM10 authored floor; captured falling motor causal oracle"]
fn bm10_k2_authored_floor_keeps_captured_motor_grounded() {
    use engine_runtime::{
        physics_3d::PhysicsModule, scene_loader::load_scene_into_world, spatial_contract::*,
    };
    let path = std::path::PathBuf::from(std::env::var_os("AIFE_BM10_SOURCE_PROJECT").unwrap());
    let prefab: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path.join("Prefabs/K2/bm10-k2-2.prefab.json")).unwrap(),
    )
    .unwrap();
    let entities = prefab["entities"].as_array().unwrap();
    let ground = entities.iter().find(|e| e["id"] == "static-0003").unwrap();
    let agent = entities.iter().find(|e| e["id"] == "agent-0533").unwrap();
    for revised in [false, true] {
        let nodes: Vec<_> = [ground, agent].iter().map(|entity| {
            let components = entity["components"].as_array().unwrap();
            let transform = &components.iter().find(|c| c["componentType"] == "Transform3D").unwrap()["data"];
            let rotation = if !revised && entity["id"] == "static-0003" { serde_json::json!([0.,0.,0.4718579255320243,0.8816745987679437]) } else { transform["rotation"]["xyzw"].clone() };
            serde_json::json!({"schemaVersion":"runtime-entity.v2","id":entity["id"],"name":entity["id"],"enabled":true,"parentId":null,"siblingOrder":0,
                "spatial":{"domain":"world3d","transform":{"translation":transform["translation"],"rotationXyzw":rotation,"scale":transform["scale"]}},
                "components":components.iter().filter(|c| c["componentType"] == "engine.body3d" || c["componentType"] == "engine.motor3d").cloned().collect::<Vec<_>>()})
        }).collect();
        let scene = serde_json::json!({"schemaVersion":"runtime-scene.v2","id":"captured","name":"captured","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":nodes});
        let RuntimeSceneDocument::Spatial(scene) =
            decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
        else {
            unreachable!()
        };
        let mut world = load_scene_into_world(&scene.into()).value.unwrap();
        let mut physics = PhysicsModule::default();
        for tick in 1..=130 {
            world.begin_spatial_tick(tick);
            physics.advance(&mut world, tick, 1. / 60.).unwrap();
            world.finish_spatial_tick();
        }
        let frame = world.physics3d_frame();
        let motor = &frame.motors["agent-0533"];
        let position = world
            .transform3d(&engine_runtime::ids::EntityId::from("agent-0533"))
            .unwrap()
            .translation;
        eprintln!(
            "captured position={position:?} authored floor revised={revised} motor={motor:?}"
        );
        assert!(
            if revised {
                motor.grounded && position[1] > 0.7
            } else {
                position[1] < -2.
            },
            "{motor:?}"
        );
    }
}

#[test]
#[ignore = "explicit BM10 package; captured traveller and original frozen route waypoint oracle"]
fn bm10_k2_traveller_obstacle_layout_keeps_route_resident() {
    use engine_navigation_native::NativeNav;
    use engine_runtime::{navigation_3d::CookedNavigation3D, runtime_package::*};
    use std::collections::{BTreeMap, BTreeSet};
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let input: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("AIFE_BM10_NAV_ORACLE").unwrap()).unwrap(),
    )
    .unwrap();
    let record = package
        .runtime_asset_index
        .record_by_guid("guid-bm10-k2-navigation")
        .unwrap();
    let cooked = package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    let mut asset: CookedNavigation3D = serde_json::from_slice(
        &std::fs::read(package.package_dir.join(cooked.path.as_ref().unwrap())).unwrap(),
    )
    .unwrap();
    let extent = [
        asset.profile.radius.max(0.25),
        asset.profile.height,
        asset.profile.radius.max(0.25),
    ];
    let project = std::path::PathBuf::from(std::env::var_os("AIFE_BM10_SOURCE_PROJECT").unwrap());
    if std::env::var_os("AIFE_BM10_TEST_HALO").is_some() {
        let src: serde_json::Value = serde_json::from_slice(
            &std::fs::read(project.join("Assets/K2/navigation.json")).unwrap(),
        )
        .unwrap();
        let vertices: Vec<[f32; 3]> = src["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                std::array::from_fn(|i| {
                    let v = p[i].as_f64().unwrap() as f32;
                    if i != 1 && v.abs() == 64. {
                        v.signum() * 65.
                    } else {
                        v
                    }
                })
            })
            .collect();
        let triangles: Vec<[u32; 3]> = serde_json::from_value(src["triangles"].clone()).unwrap();
        let p = &asset.profile;
        for t in &mut asset.tiles {
            t.data = engine_navigation_native::bake(
                &engine_navigation_native::BakeConfig {
                    cell_size: p.cell_size,
                    cell_height: p.cell_height,
                    radius: p.radius,
                    height: p.height,
                    climb: p.climb,
                    slope: p.max_slope_degrees,
                    width: p.tile_width,
                    min: [t.x as f32 * p.tile_width, -1., t.z as f32 * p.tile_width],
                    max: [
                        (t.x + 1) as f32 * p.tile_width,
                        4.,
                        (t.z + 1) as f32 * p.tile_width,
                    ],
                    x: t.x,
                    z: t.z,
                },
                &vertices,
                &triangles,
                &[],
            )
            .unwrap();
        }
    }
    let route: serde_json::Value =
        serde_json::from_slice(&std::fs::read(project.join("Benchmark/k2-input.json")).unwrap())
            .unwrap();
    let keys: Vec<_> = route["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            (
                k["tick"].as_u64().unwrap(),
                k["moveStartTick"].as_u64().unwrap(),
                std::array::from_fn::<_, 3, _>(|i| k["position"][i].as_f64().unwrap() as f32),
                k["operation"] == "teleport",
            )
        })
        .collect();
    let end = route["durationTicks"].as_u64().unwrap();
    let sample = |tick: u64| -> [f32; 3] {
        if let Some(pair) = keys.windows(2).find(|p| tick < p[1].0) {
            let (a, b) = (pair[0], pair[1]);
            if b.3 {
                a.2
            } else {
                let t = if b.0 == b.1 {
                    1.
                } else {
                    (tick.saturating_sub(b.1) as f64 / (b.0 - b.1) as f64).clamp(0., 1.)
                };
                std::array::from_fn(|i| {
                    (f64::from(a.2[i]) + (f64::from(b.2[i]) - f64::from(a.2[i])) * t) as f32
                })
            }
        } else {
            keys.last().unwrap().2
        }
    };
    let cell = |p: [f32; 3]| {
        (
            ((p[0] + 64.) / 128.).floor() as i32,
            ((p[2] + 64.) / 128.).floor() as i32,
        )
    };
    let variant = |(x, z): (i32, i32)| (z.rem_euclid(3) * 3 + x.rem_euclid(3)) as usize;
    let target_at = |tick: u64| {
        let p = sample(tick);
        let (x, z) = cell(p);
        let lx = p[0] - (x * 128) as f32;
        let lz = p[2] - (z * 128) as f32;
        [
            if lx < -31. && lz < -31. {
                (x * 128) as f32 - 30.
            } else if (51.5..60.5).contains(&lx) && (51.5..60.5).contains(&lz) {
                (x * 128) as f32 + 50.
            } else {
                p[0]
            },
            0.,
            p[2],
        ]
    };
    let teleport = |tick: u64| keys.iter().any(|k| k.0 == tick && k.3);
    let mut nav = NativeNav::new(asset.profile.tile_width).unwrap();
    let mut loaded = BTreeMap::<(i32, i32), Vec<u64>>::new();
    let mut previous = [0., 0.85, 0.];
    let mut checked = 0;
    let started = std::time::Instant::now();
    for tick in 0..=end {
        let (cx, cz) = cell(sample(tick));
        let needed: BTreeSet<_> = (-1..=1)
            .flat_map(|z| (-1..=1).map(move |x| (cx + x, cz + z)))
            .collect();
        let removed: Vec<_> = loaded
            .keys()
            .copied()
            .filter(|c| !needed.contains(c))
            .collect();
        for c in removed {
            for tile in loaded.remove(&c).unwrap() {
                nav.remove_tile(tile).unwrap();
            }
        }
        if needed.iter().any(|c| !loaded.contains_key(c)) {
            for &(x, z) in &needed {
                if loaded.contains_key(&(x, z)) {
                    continue;
                }
                let handles = asset
                    .tiles
                    .iter()
                    .map(|t| {
                        let bytes = engine_navigation_native::translated_tile(
                            &t.data,
                            [x as f32 * 128., 0., z as f32 * 128.],
                            [x * 4, z * 4],
                        )
                        .unwrap();
                        nav.add_tile(&bytes).unwrap()
                    })
                    .collect();
                loaded.insert((x, z), handles);
            }
            let mut boxes = Vec::new();
            for &(x, z) in &needed {
                let v = variant((x, z));
                for (_i, o) in input["obstacles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|o| {
                        let c = (
                            o["coord"][0].as_i64().unwrap() as i32,
                            o["coord"][1].as_i64().unwrap() as i32,
                        );
                        variant(c) == v
                    })
                    .enumerate()
                {
                    let revised = input["revisedObstacles"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["id"] == o["id"])
                        .unwrap();
                    let pos = [
                        x as f32 * 128. + revised["position"][0].as_f64().unwrap() as f32,
                        revised["position"][1].as_f64().unwrap() as f32,
                        z as f32 * 128. + revised["position"][2].as_f64().unwrap() as f32,
                    ];
                    boxes.push(std::array::from_fn(|j| {
                        let k = j % 3;
                        let h = o["halfExtents"][k].as_f64().unwrap() as f32;
                        pos[k] + if j < 3 { -h } else { h }
                    }));
                }
            }
            assert_eq!(boxes.len(), 50);
            nav.set_obstacles(&boxes).unwrap();
        }
        let future = if (tick..=(tick + 60).min(end)).any(teleport) {
            tick
        } else {
            (tick + 60).min(end)
        };
        let target = target_at(future);
        if teleport(tick) {
            previous = target_at(tick);
        }
        let path = nav.find_path(previous, target, extent);
        assert!(
            path.is_ok(),
            "tick={tick} from={previous:?} target={target:?} result={path:?}"
        );
        previous = target;
        checked += 1;
        assert!(
            started.elapsed().as_secs() < 20,
            "bounded route waypoint oracle"
        );
    }
    eprintln!("original route waypoint checks={checked}, native elapsed={:?}; no motion/consumer/qualification claim",started.elapsed());
}
