use engine_runtime::{
    render_contract::{CookedGeometry, RenderAssetKind, RenderAssetReference},
    runtime_package::RuntimeAssetRef,
    terrain3d::*,
};
use std::collections::BTreeMap;

#[test]
#[ignore = "explicit published BM13 package; validates real async tickets at the frozen desktop budget"]
fn s5d_bm13_published_vegetation_loads_under_original_decode_cap() {
    use engine_runtime::{
        runtime_asset::RuntimeAssetLoadState,
        runtime_asset_loader::{RuntimeAssetLoader, StreamingBudget},
        runtime_package::load_runtime_package,
    };
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(&root).value.unwrap();
    let records: Vec<engine_runtime::runtime_asset::RuntimeAssetRecord> = serde_json::from_slice(
        &std::fs::read(root.join("assets/runtime-asset-index.json")).unwrap(),
    )
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
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    assert!(loader.mount_bundle("startup"));
    let mut instances = 0;
    for record in records.iter().filter(|r| r.asset_type == "vegetation3d") {
        let handle = loader
            .request_async(&RuntimeAssetRef {
                id: record.asset_id.clone(),
                asset_type: record.asset_type.clone(),
                guid: Some(record.asset_guid.clone()),
                sub_asset: None,
            })
            .unwrap();
        let started = std::time::Instant::now();
        while loader.get_handle_state(&handle) == Some(RuntimeAssetLoadState::Loading)
            && started.elapsed().as_secs() < 10
        {
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
        let asset: VegetationAsset3D =
            serde_json::from_slice(&loader.get_asset_bytes(&handle).unwrap()).unwrap();
        asset.validate().unwrap();
        instances += asset
            .clusters
            .iter()
            .map(|c| c.instances.len())
            .sum::<usize>();
        loader.release(&handle).unwrap();
    }
    assert_eq!(
        instances, 50_000,
        "partitioning may not shrink the actual workload"
    );
}

fn identity(id: &str) -> TerrainAssetIdentity3D {
    TerrainAssetIdentity3D {
        asset_id: id.into(),
        asset_guid: format!("guid-{id}"),
    }
}
fn terrain() -> HeightfieldAsset3D {
    let tiles = (0..2)
        .map(|tx| HeightfieldTile3D {
            key: format!("tile-{tx}"),
            coordinate: [tx, 0],
            heights: (0..=8)
                .flat_map(|z| {
                    (0..=8).map(move |x| {
                        let gx = (tx * 8 + x) as f32;
                        0.01 * gx * gx + 0.03 * (z * z) as f32
                    })
                })
                .collect(),
            cell_layers: (0..8)
                .flat_map(|z| (0..8).map(move |_| u8::from(z >= 4)))
                .collect(),
            lods: vec![
                TerrainLodOutput3D {
                    step: 1,
                    mesh: identity(&format!("mesh-{tx}-1")),
                },
                TerrainLodOutput3D {
                    step: 4,
                    mesh: identity(&format!("mesh-{tx}-4")),
                },
            ],
            collision: Some(identity(&format!("collision-{tx}"))),
            navigation: None,
        })
        .collect();
    HeightfieldAsset3D {
        schema_version: "terrain3d-asset.v1".into(),
        asset_id: "terrain".into(),
        asset_guid: "terrain-guid".into(),
        quads: 8,
        sample_spacing: 1.,
        material_slots: vec!["soil".into(), "rock".into()],
        tiles,
    }
}
fn vertices(mesh: &CookedGeometry) -> Vec<([f32; 3], [f32; 3])> {
    mesh.vertex_bytes
        .chunks_exact(32)
        .map(|bytes| {
            let read = |i| f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
            ([read(0), read(4), read(8)], [read(12), read(16), read(20)])
        })
        .collect()
}
fn indices(mesh: &CookedGeometry) -> Vec<[usize; 3]> {
    mesh.index_bytes
        .chunks_exact(12)
        .map(|b| {
            std::array::from_fn(|i| {
                u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize
            })
        })
        .collect()
}

#[test]
fn s4c_real_lod_geometry_keeps_shared_positions_normals_materials_and_upward_winding() {
    let source = terrain();
    let fine = source.build_tile_mesh("tile-0", 1).unwrap();
    let coarse = source.build_tile_mesh("tile-1", 4).unwrap();
    assert!(
        coarse.index_count < fine.index_count,
        "LOD must actually reduce geometry"
    );
    assert_eq!(
        coarse.submeshes.len(),
        2,
        "both materials have consumed index ranges"
    );
    assert!(coarse.submeshes.iter().all(|s| s.index_count > 0));
    let edge = |mesh: &CookedGeometry, x: f32| -> BTreeMap<u32, ([f32; 3], [f32; 3])> {
        vertices(mesh)
            .into_iter()
            .filter(|(p, _)| p[0] == x)
            .map(|(p, n)| (p[2].to_bits(), ([0., p[1], p[2]], n)))
            .collect()
    };
    assert_eq!(
        edge(&fine, 8.),
        edge(&coarse, 0.),
        "neighbour LOD edge and lighting must agree exactly"
    );
    assert_eq!(
        edge(&fine, 8.).len(),
        9,
        "all shared source samples survive coarse LOD"
    );
    for mesh in [&fine, &coarse] {
        let points = vertices(mesh);
        for [a, b, c] in indices(mesh) {
            let [a, b, c] = [points[a].0, points[b].0, points[c].0];
            let y = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]);
            assert!(y > 0., "triangle must face +Y");
        }
    }
}

#[test]
fn s4c_render_collision_surface_is_identical_and_material_refinement_keeps_dense_edges() {
    let mut source = terrain();
    source.tiles[0].cell_layers[0] = 1;
    let mesh = source.build_tile_mesh("tile-0", 1).unwrap();
    let collision = source.build_tile_collision("tile-0").unwrap();
    assert_eq!(
        collision.vertices,
        vertices(&mesh).into_iter().map(|v| v.0).collect::<Vec<_>>()
    );
    assert_eq!(
        collision.triangles,
        indices(&mesh)
            .into_iter()
            .map(|v| v.map(|i| i as u32))
            .collect::<Vec<_>>()
    );
    let coarse = source.build_tile_mesh("tile-0", 4).unwrap();
    let edge_samples: std::collections::BTreeSet<_> = vertices(&coarse)
        .into_iter()
        .filter(|(p, _)| p[0] == 4. && p[2] <= 4.)
        .map(|(p, _)| p[2].to_bits())
        .collect();
    assert_eq!(
        edge_samples.len(),
        5,
        "refined/coarse interior interface keeps every lattice sample"
    );
}

#[test]
fn s4c_terrain_rejects_corrupt_samples_outputs_and_layer_indices() {
    let source = terrain();
    let mut bad = source.clone();
    bad.tiles[1].heights[0] += 1.;
    assert!(bad.validate().is_err());
    let mut bad = source.clone();
    bad.tiles[0].heights.pop();
    assert!(bad.validate().is_err());
    let mut bad = source.clone();
    bad.tiles[0].heights[0] = f32::NAN;
    assert!(bad.validate().is_err());
    let mut bad = source.clone();
    bad.tiles[0].cell_layers[0] = 2;
    assert!(bad.validate().is_err());
    let mut bad = source.clone();
    bad.tiles[1].lods[0].mesh = bad.tiles[0].lods[0].mesh.clone();
    assert!(bad.validate().is_err());
    let mut bad = source;
    bad.tiles[0].lods[1].step = 3;
    assert!(bad.validate().is_err());
}

fn reference(id: &str, kind: RenderAssetKind) -> RenderAssetReference {
    RenderAssetReference {
        expected_kind: kind,
        reference: RuntimeAssetRef {
            id: id.into(),
            asset_type: kind.as_str().into(),
            guid: Some(format!("guid-{id}")),
            sub_asset: None,
        },
    }
}
fn vegetation() -> (VegetationAsset3D, VegetationBinding3D) {
    let asset = VegetationAsset3D {
        schema_version: "vegetation3d-asset.v1".into(),
        asset_id: "trees".into(),
        asset_guid: "guid-trees".into(),
        clusters: vec![VegetationCluster3D {
            key: "pines".into(),
            mesh: reference("pine", RenderAssetKind::Mesh),
            material: reference("leaves", RenderAssetKind::Material),
            lods: vec![],
            mesh_bounds: [[-0.5, 0., -0.5], [0.5, 3., 0.5]],
            wind: VegetationWind3D {
                direction: [1., 0.],
                amplitude: 0.2,
                frequency_hz: 1.,
            },
            instances: (0..64)
                .map(|i| VegetationInstance3D {
                    source_id: format!("tree-{i}"),
                    translation: [i as f32 * 2., 0., 0.],
                    yaw_radians: 0.3,
                    scale: 1.,
                    wind_phase: i as f32 * 0.1,
                })
                .collect(),
        }],
    };
    let binding = VegetationBinding3D {
        cluster_asset: identity("trees"),
        promoted_instance_ids: vec![],
    };
    (asset, binding)
}
fn transform(m: [f32; 16], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i] * p[0] + m[4 + i] * p[1] + m[8 + i] * p[2] + m[12 + i])
}

#[test]
fn s4c_wind_changes_actual_instance_vertices_preserves_roots_and_conservative_bounds() {
    let (asset, binding) = vegetation();
    let still = asset.project_instances(&binding, 0.).unwrap();
    let windy = asset.project_instances(&binding, 0.25).unwrap();
    assert_eq!(still.len(), 64);
    assert_eq!(
        transform(still[0].matrix, [0., 0., 0.]),
        transform(windy[0].matrix, [0., 0., 0.])
    );
    assert!(
        (transform(windy[0].matrix, [0., 3., 0.])[0]
            - transform(still[0].matrix, [0., 3., 0.])[0]
            - 0.6)
            .abs()
            < 1e-5
    );
    assert_ne!(
        windy[0].matrix[4], windy[1].matrix[4],
        "per-source phase affects consumed matrices"
    );
    for tick in 0..80 {
        for draw in asset
            .project_instances(&binding, tick as f64 / 80.)
            .unwrap()
        {
            for x in [-0.5, 0.5] {
                for y in [0., 3.] {
                    for z in [-0.5, 0.5] {
                        let point = transform(draw.matrix, [x, y, z]);
                        assert!((0..3).all(|i| point[i] >= draw.bounds[0][i] - 1e-5
                            && point[i] <= draw.bounds[1][i] + 1e-5));
                    }
                }
            }
        }
    }
}

#[test]
fn s4c_promotion_preserves_source_identity_and_removes_the_decoration() {
    let (asset, mut binding) = vegetation();
    binding.promoted_instance_ids.push("tree-37".into());
    let promoted = asset.promote(&binding, "tree-37").unwrap();
    assert_eq!(promoted.source_id, "tree-37");
    assert_eq!(promoted.matrix[12], 74.);
    let instances = asset.project_instances(&binding, 1.).unwrap();
    assert_eq!(instances.len(), 63);
    assert!(instances.iter().all(|i| i.source_id != "tree-37"));
    binding.promoted_instance_ids.push("tree-37".into());
    assert!(asset.validate_binding(&binding).is_err());
    binding.promoted_instance_ids = vec!["missing".into()];
    assert!(asset.validate_binding(&binding).is_err());
}

#[test]
fn s4c_vegetation_budgets_and_stable_identity_fail_before_projection() {
    let (asset, binding) = vegetation();
    let mut bad = asset.clone();
    bad.clusters[0].instances.pop();
    assert!(bad.project_instances(&binding, 0.).is_err());
    let mut bad = asset.clone();
    bad.clusters[0].instances[1].source_id = "tree-0".into();
    assert!(bad.validate().is_err());
    let mut bad = asset.clone();
    bad.clusters[0].wind.amplitude = 1.;
    assert!(bad.validate().is_err());
    assert!(asset.project_instances(&binding, f64::INFINITY).is_err());
    assert!(asset.project_instances(&binding, -1.).is_err());
    assert!(asset
        .project_instances(&binding, f64::MAX)
        .unwrap()
        .iter()
        .all(|draw| draw.matrix.iter().all(|v| v.is_finite())));
}

#[test]
fn s5d_vegetation_lod_schema_keeps_legacy_and_rejects_unordered_or_wrong_meshes() {
    let (asset, binding) = vegetation();
    let legacy = serde_json::to_value(&asset).unwrap();
    assert!(legacy["clusters"][0].get("lods").is_none());
    let mut lod = asset.clone();
    lod.clusters[0].lods.push(VegetationLodLevel3D {
        distance: 20.,
        mesh: reference("coarse-pine", RenderAssetKind::Mesh),
    });
    assert_eq!(
        lod.project_instances(&binding, 0.).unwrap(),
        asset.project_instances(&binding, 0.).unwrap(),
        "LOD does not duplicate or alter authored instances"
    );
    let mut promoted_binding = binding.clone();
    promoted_binding.promoted_instance_ids.push("tree-0".into());
    let promoted = lod.promote(&promoted_binding, "tree-0").unwrap();
    assert_eq!(promoted.mesh, asset.clusters[0].mesh);
    for distance in [0., -1., f64::INFINITY, f64::NAN, 1_000_001.] {
        let mut bad = lod.clone();
        bad.clusters[0].lods[0].distance = distance;
        assert!(bad.validate().is_err());
    }
    let mut bad = lod.clone();
    let duplicate = bad.clusters[0].lods[0].clone();
    bad.clusters[0].lods.push(duplicate);
    assert!(bad.validate().is_err());
    let mut bad = lod;
    bad.clusters[0].lods[0].mesh = reference("wrong", RenderAssetKind::Material);
    assert!(bad.validate().is_err());
}

#[test]
#[ignore = "explicit ordinary BM13 K2 sources; real Recast/Detour periodic boundary consumer"]
fn s5d_bm13_k2_periodic_sources_keep_real_paths_across_both_wrap_seams() {
    use engine_navigation_native::{bake, translated_tile, BakeConfig, NativeNav};
    use engine_runtime::navigation_3d::AgentProfile3D;
    #[derive(serde::Deserialize)]
    struct Source {
        profile: AgentProfile3D,
        vertices: Vec<[f32; 3]>,
        triangles: Vec<[u32; 3]>,
    }
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM13_K2_PROJECT").unwrap());
    let config = |p: &AgentProfile3D| BakeConfig {
        cell_size: p.cell_size,
        cell_height: p.cell_height,
        radius: p.radius,
        height: p.height,
        climb: p.climb,
        slope: p.max_slope_degrees,
        width: p.tile_width,
        min: [0., -3., 0.],
        max: [32., 4., 32.],
        x: 0,
        z: 0,
    };
    let mut cooked = Vec::new();
    let mut without_halo = BTreeMap::new();
    for i in 0..9 {
        let bytes = std::fs::read(root.join(format!("Assets/World/navigation-{i}.asset"))).unwrap();
        let source: Source = serde_json::from_slice(&bytes).unwrap();
        source.profile.validate().unwrap();
        assert_eq!(source.profile.tile_width, 32.);
        assert_eq!(source.vertices.len(), 74529);
        assert_eq!(source.triangles.len(), 147968);
        let data = bake(
            &config(&source.profile),
            &source.vertices,
            &source.triangles,
            &[],
        )
        .unwrap();
        engine_navigation_native::validate_tile(&data).unwrap();
        eprintln!(
            "BM13 K2 nav-source{i} {} cookedBytes{}",
            engine_runtime::canonical_digest::sha256_prefixed(&bytes),
            data.len()
        );
        cooked.push(data);
        if i == 0 || i == 2 {
            let inside: Vec<_> = source
                .triangles
                .iter()
                .copied()
                .filter(|t| {
                    t.iter().all(|index| {
                        let p = source.vertices[*index as usize];
                        (0.0..=32.).contains(&p[0]) && (0.0..=32.).contains(&p[2])
                    })
                })
                .collect();
            without_halo.insert(
                i,
                bake(&config(&source.profile), &source.vertices, &inside, &[]).unwrap(),
            );
        }
    }
    let mut actual = NativeNav::new(32.).unwrap();
    for z in 0..4 {
        for x in 0..4 {
            let index = ((z % 3) * 3 + x % 3) as usize;
            let data = translated_tile(
                &cooked[index],
                [(x * 32) as f32, 0., (z * 32) as f32],
                [x, z],
            )
            .unwrap();
            actual.add_tile(&data).unwrap();
        }
    }
    for (start, end) in [
        ([80., 0., 16.], [112., 0., 16.]),
        ([16., 0., 80.], [16., 0., 112.]),
        ([16., 0., 16.], [80., 0., 16.]),
        ([16., 0., 16.], [16., 0., 80.]),
    ] {
        let path = actual.find_path(start, end, [0.5, 2., 0.5]).unwrap();
        let last = path.points.last().unwrap();
        assert!((last[0] - end[0]).abs() < 0.1 && (last[2] - end[2]).abs() < 0.1);
    }
    let mut finite_edges = NativeNav::new(32.).unwrap();
    for (index, x) in [(2, 2), (0, 3)] {
        let data =
            translated_tile(&without_halo[&index], [(x * 32) as f32, 0., 0.], [x, 0]).unwrap();
        finite_edges.add_tile(&data).unwrap();
    }
    let incomplete = finite_edges.find_path([80., 0., 16.], [112., 0., 16.], [0.5, 2., 0.5]);
    assert!(
        match incomplete {
            Err(_) => true,
            Ok(p) => p.points.last().is_none_or(|p| (p[0] - 112.).abs() > 0.5),
        },
        "Removing real neighbour geometry must expose the original false-cliff boundary"
    );
}
