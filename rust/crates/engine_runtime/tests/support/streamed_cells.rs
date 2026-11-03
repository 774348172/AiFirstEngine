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
    build_with_component(rendered, explicit_identity, extent, None)
}

pub fn build_with_component(
    rendered: bool,
    explicit_identity: bool,
    extent: u32,
    component: Option<serde_json::Value>,
) -> Fixture {
    build_content(rendered, explicit_identity, extent, component, 0, 0)
}

pub fn build_spatial_scene(count: usize) -> Fixture {
    build_content(true, true, 4, None, count, 0)
}

pub fn build_spatial_scene_with_texture_extent(count: usize, extent: u32) -> Fixture {
    build_content(true, true, extent, None, count, 0)
}

#[cfg(feature = "real-wgpu")]
pub fn build_spatial_scene_with_mip_sampling(count: usize) -> Fixture {
    build_content_variant(true, true, 4, None, count, 0, false, true, 1)
}

pub fn build_pure_spatial_scene(count: usize) -> Fixture {
    build_content(false, true, 4, None, count, 0)
}

pub fn build_burst_scene(count: usize) -> Fixture {
    build_content(false, true, 4, None, count, 100)
}

fn build_content(
    rendered: bool,
    explicit_identity: bool,
    extent: u32,
    component: Option<serde_json::Value>,
    scene_count: usize,
    extra_prefabs: usize,
) -> Fixture {
    build_content_variant(
        rendered,
        explicit_identity,
        extent,
        component,
        scene_count,
        extra_prefabs,
        false,
        false,
        1,
    )
}

#[cfg(feature = "real-wgpu")]
pub fn build_dual_32mib_cells() -> Fixture {
    build_content_variant(true, true, 4, None, 0, 0, true, false, 1)
}

pub fn build_repeated_mesh_cell(count: usize) -> Fixture {
    build_content_variant(true, true, 4, None, 0, 0, false, false, count)
}

fn build_content_variant(
    rendered: bool,
    explicit_identity: bool,
    extent: u32,
    component: Option<serde_json::Value>,
    scene_count: usize,
    extra_prefabs: usize,
    dual_32mib: bool,
    mip_sampling: bool,
    renderer_count: usize,
) -> Fixture {
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
    let mut camera = scene.entities[0].clone();
    template.id = "node".into();
    scene.entities.clear();
    scene.main_camera = None;
    if scene_count > 0 {
        for i in 0..scene_count {
            let mut e = template.clone();
            e.id = format!("node-{i}");
            if let Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
                transform,
                mesh_renderer,
                ..
            }) = &mut e.spatial
            {
                transform.translation = [
                    (i % 40) as f32 * 0.08 - 1.56,
                    (i / 40) as f32 * 0.08 - 0.96,
                    -4.,
                ];
                transform.scale = [0.025; 3];
                if rendered {
                    let m = mesh_renderer.as_mut().unwrap();
                    m.mesh.0 = reference("shared-mesh", "mesh");
                    m.materials.get_mut("surface").unwrap().0 = reference("material-a", "material");
                } else {
                    *mesh_renderer = None;
                }
            }
            scene.entities.push(e);
        }
        if let Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
            transform, ..
        }) = &mut camera.spatial
        {
            transform.translation = [0.; 3];
        }
        scene.main_camera = Some(camera.id.clone());
        scene.entities.push(camera);
    }
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
    if scene_count > 0 && !rendered {
        use engine_runtime::aui::*;
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        let mut nodes = Vec::new();
        for i in 0..20 {
            let id = format!("hud-{i}");
            root.children.push(id.clone());
            let mut node = AuiNode::new(
                id,
                AuiNodeKind::Panel,
                AuiRect::fixed_position(
                    8. + (i % 5) as f32 * 48.,
                    8. + (i / 5) as f32 * 48.,
                    24.,
                    24.,
                ),
            );
            node.parent = Some("root".into());
            node.style = Some(AuiStyle::color(if i % 2 == 0 {
                "#ff0000"
            } else {
                "#00ff00"
            }));
            nodes.push(node);
        }
        nodes.insert(0, root);
        let mut document = AuiDocument::new(
            "hud",
            vec![AuiCanvas::screen_overlay("main", 256., 256., "root")],
            nodes,
        );
        document.schema_version =
            engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
        input.aui_documents.push(RuntimePackageSourceJson {
            id: "hud".into(),
            document: serde_json::to_value(document).unwrap(),
        });
        input.aui_manifest = Some(RuntimeAuiManifest {
            schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.into(),
            documents: vec![RuntimeAuiManifestEntry {
                document_id: "hud".into(),
                path: "aui/documents/hud.aui.json".into(),
                canvas_count: 1,
                node_count: 21,
                binding_count: 0,
                action_count: 0,
                asset_refs: Vec::new(),
            }],
        });
    }
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
        let mut mesh = super::geometry_fixture::triangle();
        if renderer_count > 1 {
            mesh.vertex_bytes
                .resize(65_536 * mesh.vertex_stride as usize, 0);
            mesh.vertex_count = 65_536;
        }
        add_render(
            &mut input,
            "shared-mesh",
            RenderAssetData::Mesh(mesh),
            vec![],
        );
        for name in ["a", "b"] {
            let mut material = super::geometry_fixture::unlit(if name == "a" {
                [1., 0., 0., 1.]
            } else {
                [0., 0., 1., 1.]
            });
            if mip_sampling {
                material.base_color_texture_sampling =
                    engine_runtime::runtime_texture::TextureSampler::parse(
                        "nearestMipmapNearest:nearest:repeat:clampToEdge",
                    );
                assert!(material.base_color_texture_sampling.is_some());
            }
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
    let names = ["a".to_owned(), "b".to_owned()]
        .into_iter()
        .chain((0..extra_prefabs).map(|i| format!("burst-{i}")));
    for name in names {
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
        if let Some(component) = &component {
            entity
                .components
                .push(serde_json::from_value(component.clone()).unwrap());
        }
        let mut entities = vec![entity.clone()];
        for i in 1..renderer_count {
            let mut child = entity.clone();
            child.id = format!("node-{i}");
            child.parent_id = Some(entity.id.clone());
            entities.push(child);
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
            "name":id, "rootEntityId":"node", "entities":entities }),
        });
    }
    if dual_32mib {
        // Each cell consumes two different 2048² RGBA allocations. No shared
        // miniature texture or reservation is counted as its 32 MiB payload.
        input
            .assets
            .retain(|a| !matches!(a.asset_type.as_str(), "texture" | "material"));
        input.texture_payloads.clear();
        for name in ["a", "b"] {
            for slot in 0..2 {
                let texture = format!("texture-{name}-{slot}");
                let material_id = format!("material-{name}-{slot}");
                let mut material = super::geometry_fixture::unlit([1.; 4]);
                material.textures.insert(
                    "baseColorTexture".into(),
                    RenderAssetReference {
                        expected_kind: RenderAssetKind::Texture,
                        reference: reference(&texture, "texture"),
                    },
                );
                add_render(
                    &mut input,
                    &material_id,
                    RenderAssetData::Material(material),
                    vec![texture.clone()],
                );
                let mut pixels = vec![0; 2048 * 2048 * 4];
                for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
                    let bright = 160 + ((i / 2048 + i % 2048 + slot * 31) % 96) as u8;
                    let color = if name == "a" {
                        [bright, 0, 0, 255]
                    } else {
                        [0, 0, bright, 255]
                    };
                    p.copy_from_slice(&color);
                }
                input.assets.push(RuntimePackageSourceAsset::new(
                    &texture,
                    &texture,
                    "texture",
                    format!("Assets/{texture}.png"),
                    format!("cooked-assets/{texture}.json"),
                ));
                input.texture_payloads.push(RuntimePackageSourceTexture {
                    metadata: CookedTextureAsset {
                        schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
                        asset_id: texture.clone(),
                        cooked_asset_id: format!("cooked-{texture}"),
                        source_hash: sha256_prefixed(&pixels),
                        width: 2048,
                        height: 2048,
                        format: "rgba8UnormSrgb".into(),
                        color_space: "srgb".into(),
                        mip_count: 1,
                        byte_length: pixels.len(),
                        pixel_data_path: format!("cooked-assets/{texture}.rgba8"),
                        sampler: "nearestClamp".into(),
                    },
                    rgba8: pixels.into(),
                });
            }
        }
        for (prefab, name) in input.prefabs.iter_mut().zip(["a", "b"]) {
            let mut root: engine_runtime::spatial_contract::RuntimeEntityV2 =
                serde_json::from_value(prefab.document["entities"][0].clone()).unwrap();
            let mut entities = Vec::new();
            for slot in 0..2 {
                {
                    let engine_runtime::spatial_contract::RuntimeSpatial::World3D {
                        mesh_renderer,
                        transform,
                        ..
                    } = &mut root.spatial;
                    mesh_renderer
                        .as_mut()
                        .unwrap()
                        .materials
                        .get_mut("surface")
                        .unwrap()
                        .0 = reference(&format!("material-{name}-{slot}"), "material");
                    transform.translation = if slot == 0 {
                        [if name == "a" { -0.6 } else { 0.6 }, -0.45, -2.]
                    } else {
                        [0., 2.25, 0.]
                    };
                    transform.scale = if slot == 0 { [0.4; 3] } else { [1.; 3] };
                }
                root.id = if slot == 0 {
                    "node".into()
                } else {
                    "detail".into()
                };
                root.parent_id = (slot == 1).then(|| "node".into());
                entities.push(root.clone());
            }
            prefab.document["entities"] = serde_json::to_value(entities).unwrap();
            prefab.identity.as_mut().unwrap().dependencies = vec![
                "shared-mesh".into(),
                format!("material-{name}-0"),
                format!("material-{name}-1"),
            ];
        }
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
