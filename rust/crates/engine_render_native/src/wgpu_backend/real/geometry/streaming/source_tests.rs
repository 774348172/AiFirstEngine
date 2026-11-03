use super::*;
use crate::runtime_asset::*;
use crate::runtime_asset_loader::RuntimeAssetLoader;
use std::sync::Mutex;

struct Sources {
    root: std::path::PathBuf,
    owners: Vec<Arc<GeometryBuffers>>,
}

impl Sources {
    fn new(count: usize) -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("371-mesh-source-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let mut owners = Vec::new();
        for index in 0..count {
            let mesh = CookedGeometry {
                vertex_bytes: [-0.8f32, -0.8, 0., 0.8, -0.8, 0., 0., 0.8, 0.]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
                index_bytes: [0u16, 1, 2]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
                vertex_stride: 12,
                vertex_count: 3,
                index_count: 3,
                index_format: GeometryIndexFormat::Uint16,
                topology: GeometryTopology::TriangleList,
                attributes: vec![VertexAttribute {
                    semantic: VertexSemantic::Position,
                    format: VertexFormat::Float32x3,
                    offset: 0,
                }],
                submeshes: vec![SubmeshRange {
                    first_index: 0,
                    index_count: 3,
                    material_slot: "surface".into(),
                }],
                bounds: GeometryBounds {
                    min: [-0.8, -0.8, 0.],
                    max: [0.8, 0.8, 0.],
                    sphere_center: [0.; 3],
                    sphere_radius: 1.2,
                },
                coordinate_convention:
                    CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
                material_slots: vec!["surface".into()],
            };
            let bytes = serde_json::to_vec(&RenderContract::new(CookedRenderAsset {
                source_digest: crate::canonical_digest::sha256_prefixed(b"mesh-source-test"),
                recipe: NATIVE_RENDER_COOK_RECIPE.into(),
                asset: RenderAssetData::Mesh(mesh.clone()),
            }))
            .unwrap();
            let hash = crate::canonical_digest::sha256_prefixed(&bytes);
            let id = format!("mesh-{index}");
            let path = format!("{index}.json");
            std::fs::write(root.join(&path), &bytes).unwrap();
            let record = serde_json::from_value(serde_json::json!({
                "assetGuid":id, "assetId":id, "type":"mesh",
                "cookedAssetId":id, "bundleId":"startup", "loaderKind":"mesh",
                "hash":hash, "size":bytes.len()
            }))
            .unwrap();
            let cooked = serde_json::from_value(serde_json::json!({
                "cookedAssetId":id, "bundleId":"startup", "path":path,
                "hash":hash, "size":bytes.len()
            }))
            .unwrap();
            let loader = RuntimeAssetLoader::new(
                &root,
                RuntimeAssetIndex::new(vec![record], vec![cooked]),
                RuntimePackageMountTable::new(vec![]),
            );
            let mut provenance = Provenance::unspecified(ProducerStage::AssetBinding);
            provenance.asset_id = Some(id);
            provenance.content_digest = Some(hash);
            owners.push(crate::render_geometry::package_streamable_for_test(
                &Arc::new(GeometryBuffers::from_cooked(&mesh).unwrap()),
                &loader,
                &provenance,
            ));
        }
        Self { root, owners }
    }
}

impl Drop for Sources {
    fn drop(&mut self) {
        for index in 0..self.owners.len() {
            let path = self.root.join(format!("{index}.json"));
            if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
        std::fs::remove_dir(&self.root).unwrap();
    }
}

#[test]
fn admitted_mesh_sources_use_bounded_workers_and_preserve_indexed_results() {
    let sources = Sources::new(12);
    for owner in &sources.owners {
        owner.release_upload_source();
    }
    let threads = Mutex::new(std::collections::HashSet::new());
    let results = prepare_mesh_sources(sources.owners.len(), true, &|index| {
        threads.lock().unwrap().insert(std::thread::current().id());
        let owner = &sources.owners[index];
        let lease = MeshUploadSource {
            owner: owner.clone(),
            payload: owner.acquire_upload_source()?,
        };
        assert_eq!(lease.payload.vertex_bytes().len(), 36);
        Ok((index, lease))
    })
    .unwrap();
    assert_eq!(
        results
            .iter()
            .map(|r| r.as_ref().unwrap().0)
            .collect::<Vec<_>>(),
        (0..12).collect::<Vec<_>>()
    );
    let workers = threads.into_inner().unwrap().len();
    assert!(
        (2..=4).contains(&workers),
        "actual source workers: {workers}"
    );
}

#[test]
fn unused_package_source_lease_releases_its_cpu_cache() {
    let sources = Sources::new(1);
    let owner = &sources.owners[0];
    let payload = owner.acquire_upload_source().unwrap();
    let weak = Arc::downgrade(payload.vertex_storage());
    drop(MeshUploadSource {
        owner: owner.clone(),
        payload,
    });
    assert!(weak.upgrade().is_none(), "unqueued source still cached");
}

#[test]
#[ignore = "requires a real GPU"]
fn parallel_source_failure_keeps_original_gpu_prefix_and_releases_unused_payloads() {
    let sources = Sources::new(4);
    let first = sources.owners[0].acquire_upload_source().unwrap();
    let first_bytes = Arc::downgrade(first.vertex_storage());
    drop(first);
    let later = sources.owners[2].acquire_upload_source().unwrap();
    let later_bytes = Arc::downgrade(later.vertex_storage());
    drop(later);
    sources.owners[1].release_upload_source();
    sources.owners[3].release_upload_source();
    std::fs::write(sources.root.join("1.json"), b"corrupt").unwrap();
    std::fs::remove_file(sources.root.join("3.json")).unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 16 * 1024 * 1024,
            staging_bytes: 4096,
            bytes_per_frame: 4096,
            max_tickets: 8,
        })
        .unwrap();
    let inputs: Vec<_> = sources
        .owners
        .iter()
        .enumerate()
        .map(|(index, source)| {
            (
                RenderResourceHandle {
                    kind: RenderResourceKind::MeshBuffer,
                    index: index as u64 + 1,
                    generation: 1,
                },
                source.clone(),
            )
        })
        .collect();
    let reserved = backend.reserve_geometry_meshes(&inputs).unwrap();
    let error = backend.upload_geometry_meshes(1, reserved).unwrap_err();
    assert_eq!(error, "geometry.source_identity_mismatch");
    assert_eq!(backend.geometry.meshes.len(), 1);
    assert!(backend.geometry.meshes.contains_key(&inputs[0].0));
    assert_eq!(backend.geometry.uploaded_meshes, 1);
    assert_eq!(backend.geometry.streaming.queued_mesh_sources.len(), 1);
    assert!(
        first_bytes.upgrade().is_some(),
        "queued prefix lost its lease"
    );
    assert!(
        later_bytes.upgrade().is_none(),
        "unused later payload leaked"
    );
    drop(backend);
    assert!(
        first_bytes.upgrade().is_none(),
        "destroyed queue retained payload"
    );
}

#[test]
fn mesh_source_worker_panic_joins_and_releases_other_preparations() {
    let sources = Sources::new(4);
    let weak: Vec<_> = sources
        .owners
        .iter()
        .map(|owner| {
            let reader = owner.acquire_upload_source().unwrap();
            Arc::downgrade(reader.vertex_storage())
        })
        .collect();
    let completed = std::sync::atomic::AtomicUsize::new(0);
    let result = prepare_mesh_sources(4, true, &|index| {
        let lease = MeshUploadSource::acquire(sources.owners[index].clone())?;
        completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert_ne!(index, 1, "injected source worker failure");
        Ok(lease)
    });
    assert!(result
        .err()
        .unwrap()
        .contains("geometry.prepare_task_panicked"));
    assert_eq!(completed.load(std::sync::atomic::Ordering::Relaxed), 4);
    assert!(weak.iter().all(|bytes| bytes.upgrade().is_none()));
}
