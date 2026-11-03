//! Full original BM03 geometry with final three-map PBR/mip Cook consumers.
//! No runtime, Android, frame-time or whole BM14 qualification is implied.
use super::*;
use engine_runtime::{
    canonical_digest::sha256_prefixed,
    render_contract::{CookedRenderAsset, MaterialPreset, RenderAssetData, RenderContract},
    runtime_package_builder::RuntimePackageBuildInput,
    runtime_texture::{texture_mip_byte_len, TextureFilter},
    spatial_contract::RuntimeSpatial,
};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

fn inspect(input: &RuntimePackageBuildInput, project: &Path) -> Value {
    let mut meshes = BTreeSet::new();
    let mut geometry = BTreeSet::new();
    let mut materials = BTreeMap::new();
    let mut textures = BTreeMap::new();
    let mut geometry_bytes = 0;
    for asset in &input.assets {
        if !matches!(asset.asset_type.as_str(), "mesh" | "material") {
            continue;
        }
        let bytes = asset.runtime_payload.as_ref().unwrap();
        let contract: RenderContract<CookedRenderAsset> = serde_json::from_slice(bytes).unwrap();
        match contract.data.asset {
            RenderAssetData::Mesh(mesh) => {
                assert_eq!((mesh.vertex_count, mesh.index_count), (600, 3000));
                assert!(
                    geometry.insert(sha256_prefixed(&mesh.vertex_bytes)),
                    "duplicate actual geometry"
                );
                geometry_bytes += mesh.vertex_bytes.len() + mesh.index_bytes.len();
                assert!(meshes.insert(asset.asset_id.as_str()));
            }
            RenderAssetData::Material(material) => {
                assert_eq!(material.preset, MaterialPreset::StandardLit);
                assert_eq!(material.textures.len(), 4);
                let mut slots = Vec::new();
                for name in [
                    "baseColorTexture",
                    "normalTexture",
                    "metallicRoughnessTexture",
                    "occlusionTexture",
                ] {
                    let binding = &material.textures[name];
                    assert_eq!(
                        material.texture_sampling[name].min_filter,
                        TextureFilter::LinearMipmapLinear
                    );
                    slots.push(binding.reference.id.clone());
                }
                assert_eq!(slots[2], slots[3]);
                assert_ne!(slots[0], slots[1]);
                assert_ne!(slots[0], slots[2]);
                assert_ne!(slots[1], slots[2]);
                assert!(materials.insert(asset.asset_id.as_str(), slots).is_none());
            }
        }
    }
    let mut pixel_hashes = BTreeSet::new();
    let mut texture_bytes = 0;
    for texture in &input.texture_payloads {
        let metadata = &texture.metadata;
        assert_eq!(
            (metadata.width, metadata.height, metadata.mip_count),
            (2048, 2048, 12)
        );
        let expected_format = if metadata.asset_id.starts_with("texture-") {
            "bc3RgbaUnormSrgb"
        } else {
            "bc3RgbaUnorm"
        };
        assert_eq!(metadata.format, expected_format);
        let length = texture_mip_byte_len(&metadata.format, 2048, 2048, 12).unwrap();
        assert_eq!(texture.rgba8.len(), length);
        assert_eq!(metadata.byte_length, length);
        let base_length = texture_mip_byte_len(&metadata.format, 2048, 2048, 1).unwrap();
        let base = &texture.rgba8[..base_length];
        assert!(
            !base.chunks_exact(16).all(|block| block == &base[..16]),
            "constant encoded map {}",
            metadata.asset_id
        );
        let digest = sha256_prefixed(&texture.rgba8);
        assert!(
            pixel_hashes.insert(digest.clone()),
            "shared encoded maps {}",
            metadata.asset_id
        );
        // source_hash binds the encoded source PNG, while this digest binds the
        // target BC3 mip payload. They deliberately describe different bytes.
        let descriptor: Value = serde_json::from_slice(
            &fs::read(project.join(format!("Assets/{}.json", metadata.asset_id))).unwrap(),
        )
        .unwrap();
        let source = project.join(descriptor["sourceImage"].as_str().unwrap());
        assert_eq!(
            metadata.source_hash,
            sha256_prefixed(&fs::read(source).unwrap())
        );
        texture_bytes += length;
        assert!(textures.insert(metadata.asset_id.as_str(), json!({"format":metadata.format,"mipCount":metadata.mip_count,"bytes":length,"sha256":digest})).is_none());
    }
    assert_eq!(
        (meshes.len(), materials.len(), textures.len()),
        (10000, 256, 192)
    );
    let mut bound_meshes = BTreeSet::new();
    let mut bound_materials = BTreeSet::new();
    let mut bound_textures = BTreeSet::new();
    let mut instances = 0;
    for scene in &input.scenes {
        for entity in &scene.entities {
            let Some(RuntimeSpatial::World3D {
                mesh_renderer: Some(renderer),
                ..
            }) = &entity.spatial
            else {
                continue;
            };
            assert!(entity.enabled && renderer.visible);
            assert!(meshes.contains(renderer.mesh.0.id.as_str()));
            bound_meshes.insert(renderer.mesh.0.id.as_str());
            instances += 1;
            for reference in renderer.materials.values() {
                let id = reference.0.id.as_str();
                bound_materials.insert(id);
                for texture in &materials[id] {
                    assert!(textures.contains_key(texture.as_str()));
                    bound_textures.insert(texture.as_str());
                }
            }
        }
    }
    assert_eq!(
        (
            instances,
            bound_meshes.len(),
            bound_materials.len(),
            bound_textures.len()
        ),
        (10000, 10000, 256, 192)
    );
    let shared_normal_consumers = materials
        .values()
        .filter(|slots| slots[1] == "normal-000")
        .count();
    assert_eq!(shared_normal_consumers, 4);
    json!({"instances":instances,"uniqueMeshes":meshes.len(),"uniqueMaterials":materials.len(),
        "triangles":instances*1000,"geometryBytes":geometry_bytes,"textureSets":64,
        "textureCount":textures.len(),"textureBytes":texture_bytes,"textures":textures,
        "sharedNormalConsumers":shared_normal_consumers})
}

fn write(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
#[ignore = "explicit original10000-mesh final PBR input; five bounded Compiler Cook rounds"]
fn original_bm03_full_pbr_mips_incremental_cook_keeps_exact_payloads_and_shared_dependencies() {
    let workspace = PathBuf::from(std::env::var_os("AIFE_BM14_PBR_WORK").unwrap());
    let project = workspace.join("project");
    let evidence = PathBuf::from(std::env::var_os("AIFE_BM14_PBR_EVIDENCE").unwrap());
    fs::create_dir(&evidence).expect("fresh evidence identity");
    let mut previous = BTreeMap::new();
    let mut previous_digest = String::new();
    for round in ["cold", "unchanged", "material", "mesh", "shared-normal"] {
        match round {
            "material" => {
                let path = project.join("Assets/material-000.json");
                let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["renderData"]["data"]["payload"]["parameters"]["metallic"]["value"] =
                    json!(0.75);
                write(&path, &value);
            }
            "mesh" => {
                let path = project.join("Assets/mesh-000.json");
                let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                let byte = value["renderData"]["data"]["payload"]["vertexBytes"][8]
                    .as_u64()
                    .unwrap();
                value["renderData"]["data"]["payload"]["vertexBytes"][8] = json!(byte ^ 1);
                write(&path, &value);
            }
            "shared-normal" => {
                let path = project.join("Assets/normal-000.png");
                let mut image = image::load_from_memory(&fs::read(&path).unwrap())
                    .unwrap()
                    .to_rgba8();
                assert_eq!(image.dimensions(), (2048, 2048));
                for y in 0..4 {
                    for x in 0..4 {
                        image.put_pixel(x, y, image::Rgba([128, 150, 250, 255]));
                    }
                }
                image.save(&path).unwrap();
            }
            _ => {}
        }
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease(format!("bm14-final-pbr-{round}"), paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = Instant::now();
        write(
            &evidence.join("progress.json"),
            &json!({"round":round,"stage":"prepare","sourceDigest":source_digest(lease.snapshot())}),
        );
        eprintln!("BM14 final PBR {round}: prepare started");
        let result = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &workspace.join("cache"),
        );
        let wall_seconds = started.elapsed().as_secs_f64();
        if let Err(error) = &result {
            write(
                &evidence.join(format!("{round}-failure.json")),
                &json!({"code":error.code(),"message":error.message(),"diagnostics":error.diagnostics(),"prepareSeconds":wall_seconds}),
            );
        }
        let prepared = result.unwrap();
        let inspected = inspect(prepared.runtime_package_build_input(), &project);
        let assets: BTreeMap<_, _> = prepared
            .runtime_package_build_input()
            .assets
            .iter()
            .map(|asset| (asset.asset_id.clone(), asset.hash.clone()))
            .collect();
        let changed: Vec<_> = assets
            .iter()
            .filter(|(id, hash)| previous.get(*id) != Some(*hash))
            .map(|(id, _)| id.clone())
            .collect();
        let producers: Vec<_> = prepared
            .producer_reports()
            .iter()
            .filter(|report| {
                matches!(
                    report.producer_id.as_str(),
                    "render-asset-cook" | "texture-cook"
                )
            })
            .collect();
        write(
            &evidence.join(format!("{round}.json")),
            &json!({"round":round,"target":"windows-release","prepareSeconds":wall_seconds,
            "sourceDigest":source_digest(lease.snapshot()),"assemblyDigest":prepared.assembly_digest(),"inspected":inspected,
            "changedAssets":changed,"outputs":assets,"producerCalls":producers,
            "scope":"Original full BM03 geometry/counts plus final PBR/mip Cook incremental only; no Player/Android/performance or whole BM14 qualification"}),
        );
        assert_eq!(producers.len(), 10000 + 256 + 192);
        if round == "cold" {
            assert!(producers
                .iter()
                .all(|report| report.cache_status == Status::Produced));
        } else if round == "unchanged" {
            assert!(producers
                .iter()
                .all(|report| report.cache_status == Status::Hit));
            assert!(changed.is_empty());
            assert_eq!(prepared.assembly_digest(), previous_digest);
        } else {
            let id = match round {
                "material" => "material-000",
                "mesh" => "mesh-000",
                _ => "normal-000",
            };
            assert_eq!(changed, vec![id.to_owned()]);
            assert_eq!(
                producers
                    .iter()
                    .filter(|report| report.cache_status == Status::Produced)
                    .count(),
                1
            );
            assert_eq!(
                producers
                    .iter()
                    .filter(|report| report.cache_status == Status::Hit)
                    .count(),
                producers.len() - 1
            );
            assert_ne!(prepared.assembly_digest(), previous_digest);
        }
        previous = assets;
        previous_digest = prepared.assembly_digest().to_string();
        eprintln!("BM14 final PBR {round}: validated full outputs in {wall_seconds:.3}s");
        assert!(
            wall_seconds <= if round == "cold" { 1800. } else { 900. },
            "stop for cost review before another Cook"
        );
    }
    write(
        &evidence.join("completed.json"),
        &json!({"status":"passed","rounds":5,"scope":"Full BM03 PBR/mip incremental Cook only"}),
    );
}
