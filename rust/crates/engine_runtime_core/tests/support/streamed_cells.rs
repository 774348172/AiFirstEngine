use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_contract::*;
use engine_runtime::runtime_package::*;
use engine_runtime::runtime_package_builder::*;
use std::path::PathBuf;

pub struct Fixture {
    pub root: PathBuf,
    pub package: RuntimePackage,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn build(rendered: bool) -> Fixture {
    build_with_identity(rendered, true)
}
pub fn build_with_identity(rendered: bool, explicit_identity: bool) -> Fixture {
    build_with_texture_extent(rendered, explicit_identity, 4)
}
pub fn build_with_texture_extent(rendered: bool, explicit_identity: bool, extent: u32) -> Fixture {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("371-streamed-cells-{}-{stamp}", std::process::id()));
    let package_dir = root.join("package");
    let mut scene: RuntimeScene = serde_json::from_slice(include_bytes!(
        "../fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    let mut template = scene.entities.remove(0);
    template.id = "node".into();
    scene.entities.clear();
    scene.main_camera = None;
    let mut input =
        RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("cells", "Cells", "1"));
    input.assets.push(RuntimePackageSourceAsset::new(
        &scene.id,
        &scene.name,
        "scene",
        "Scenes/main.json",
        format!("scenes/{}.json", scene.id),
    ));
    input.scenes.push(scene);
    input.component_schema =
        Some(serde_json::json!({"schemaVersion":"component-schema.v1", "components":[]}));
    input.input_mappings.push(RuntimePackageSourceJson {
        id: "benchmark.input".into(),
        document: serde_json::from_slice(include_bytes!(
            "../../../../../samples/platform_3d_benchmark_project/Input/default.json"
        ))
        .unwrap(),
    });
    if rendered {
        add_render(
            &mut input,
            "shared-mesh",
            RenderAssetData::Mesh(super::geometry_fixture::triangle()),
            vec![],
        );
        for name in ["a", "b"] {
            let mut material = super::geometry_fixture::unlit(if name == "a" {
                [1., 0., 0., 1.]
            } else {
                [0., 0., 1., 1.]
            });
            material.textures.insert(
                "baseColorTexture".into(),
                RenderAssetReference {
                    expected_kind: RenderAssetKind::Texture,
                    reference: reference("shared-texture", "texture"),
                },
            );
            add_render(
                &mut input,
                &format!("material-{name}"),
                RenderAssetData::Material(material),
                vec!["shared-texture".into()],
            );
        }
        let pixels = vec![255; (extent * extent * 4) as usize];
        input.assets.push(RuntimePackageSourceAsset::new(
            "shared-texture",
            "Texture",
            "texture",
            "Assets/texture.png",
            "cooked-assets/texture.json",
        ));
        input.texture_payloads.push(RuntimePackageSourceTexture {
            metadata: CookedTextureAsset {
                schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
                asset_id: "shared-texture".into(),
                cooked_asset_id: "cooked-shared-texture".into(),
                source_hash: sha256_prefixed(&pixels),
                width: extent,
                height: extent,
                format: "rgba8UnormSrgb".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                byte_length: pixels.len(),
                pixel_data_path: "cooked-assets/texture.rgba8".into(),
                sampler: "nearestClamp".into(),
            },
            rgba8: pixels.into(),
        });
    }
    for name in ["a", "b"] {
        let mut entity = template.clone();
        if let Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
            transform,
            mesh_renderer,
            ..
        }) = &mut entity.spatial
        {
            transform.translation = [if name == "a" { -0.6 } else { 0.6 }, 0., -2.];
            if rendered {
                let renderer = mesh_renderer.as_mut().unwrap();
                renderer.mesh.0 = reference("shared-mesh", "mesh");
                renderer.materials.get_mut("surface").unwrap().0 =
                    reference(&format!("material-{name}"), "material");
            } else {
                *mesh_renderer = None;
            }
        }
        let id = format!("cell-{name}");
        input.prefabs.push(RuntimePackageSourcePrefab {
            identity: explicit_identity.then(|| RuntimePackagePrefabIdentity {
                asset_guid: id.clone(),
                sub_asset_id: "root".into(),
                source: format!("Prefabs/{id}.json"),
                dependencies: if rendered {
                    vec!["shared-mesh".into(), format!("material-{name}")]
                } else {
                    vec![]
                },
            }),
            prefab_id: id.clone(),
            document: serde_json::json!({ "schemaVersion":"runtime-prefab.v2", "id":id,
            "name":id, "rootEntityId":"node", "entities":[entity] }),
        });
    }
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&package_dir, "main"),
        &input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{:?}",
        report.diagnostics
    );
    let loaded = load_runtime_package(&package_dir);
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    Fixture {
        root,
        package: loaded.value.unwrap(),
    }
}
fn add_render(
    input: &mut RuntimePackageBuildInput,
    id: &str,
    asset: RenderAssetData,
    dependencies: Vec<String>,
) {
    let bytes = serde_json::to_vec(&RenderContract::new(CookedRenderAsset {
        source_digest: format!("sha256:{}", "1".repeat(64)),
        recipe: NATIVE_RENDER_COOK_RECIPE.into(),
        asset,
    }))
    .unwrap();
    let mut source = RuntimePackageSourceAsset::new(
        id,
        id,
        if id == "shared-mesh" {
            "mesh"
        } else {
            "material"
        },
        format!("Assets/{id}.json"),
        format!("cooked-assets/{id}.json"),
    )
    .with_runtime_payload(bytes.clone());
    source.hash = Some(sha256_prefixed(&bytes));
    source.dependencies = dependencies;
    input.assets.push(source);
}
pub fn reference(id: &str, kind: &str) -> RuntimeAssetRef {
    RuntimeAssetRef {
        id: id.into(),
        asset_type: kind.into(),
        guid: Some(id.into()),
        sub_asset: None,
    }
}
pub fn request(
    name: &str,
    revision: u64,
    rendered: bool,
) -> engine_runtime::runtime_instance_loader::CellActivationRequest {
    use engine_runtime::runtime_instance_loader::{CellActivationRequest, CellDomain};
    CellActivationRequest {
        schema_version: "cell-activation.v1".into(),
        placement_id: format!("placement-{name}"),
        prefab: reference(&format!("cell-{name}"), "prefab"),
        asset_revision_epoch: revision,
        required_domains: if rendered {
            [CellDomain::World, CellDomain::Asset, CellDomain::Render].into()
        } else {
            [CellDomain::World, CellDomain::Asset].into()
        },
        parent: None,
    }
}
