use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_graph::OrderedF32;
use engine_runtime::render_resource::*;
#[cfg(feature="real-wgpu")]
#[path="support/s3b_skin.rs"]
mod gpu_s3b;

#[path = "support/geometry_fixture.rs"]
mod geometry_fixture;
#[path = "support/s2c_environment.rs"]
mod s2c_environment;
#[path = "support/s2c_ibl.rs"]
mod s2c_ibl;
#[path = "support/standard_lit_fixture.rs"]
mod standard_lit_fixture;

#[cfg(feature = "real-wgpu")]
#[path = "support/s2_work_granularity.rs"]
mod s2_work_granularity;

#[path = "support/s2b_shadows.rs"]
mod s2b_shadows;
#[path = "support/s2c_particle_views.rs"]
mod s2c_particle_views;
#[path = "support/s2c_presentation.rs"]
mod s2c_presentation;

fn masked(alpha: f32, cutoff: f32) -> CookedMaterial {
    let mut value = serde_json::to_value(geometry_fixture::unlit([1., 0., 0., alpha])).unwrap();
    value["blend"] = "alphaMask".into();
    value["parameters"]["alphaCutoff"] = serde_json::json!({"type":"float","value":cutoff});
    serde_json::from_value(value).expect("S2-A alphaMask is a real material mode")
}

mod s2a {
    use super::*;

    #[test]
    fn mask_parameters_reject_missing_out_of_range_and_non_mask_cutoff() {
        let good = masked(0.4, 0.5);
        good.validate().unwrap();
        for cutoff in [-0.1, 1.1, f32::NAN] {
            let mut bad = good.clone();
            bad.parameters
                .insert("alphaCutoff".into(), MaterialParameter::Float(cutoff));
            assert!(bad.validate().is_err());
        }
        let mut bad = good.clone();
        bad.parameters.remove("alphaCutoff");
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.blend = MaterialBlend::AlphaBlend;
        assert!(bad.validate().is_err());
        bad = good;
        bad.opaque_alpha = true;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn mask_cutoff_is_immutable_draw_data_and_not_a_pipeline_variant() {
        let mut manager = RenderResourceManager::new();
        let mesh = geometry_fixture::prepare(
            "mesh",
            RenderAssetData::Mesh(geometry_fixture::triangle()),
            &mut manager,
        );
        let mut draws = Vec::new();
        for (id, cutoff) in [("mask-a", 0.3), ("mask-b", 0.7)] {
            let material = geometry_fixture::prepare(
                id,
                RenderAssetData::Material(masked(0.5, cutoff)),
                &mut manager,
            );
            let prepared = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
            draws.push(geometry_fixture::frame(&prepared, &manager).views[0].draws[0].clone());
        }
        assert_ne!(draws[0], draws[1]);
        assert_eq!(
            draws[0].pipeline_identity("rgba8"),
            draws[1].pipeline_identity("rgba8")
        );
        let legacy = geometry_fixture::source(&mut manager);
        let legacy = geometry_fixture::frame(&legacy, &manager);
        assert_ne!(
            draws[0].pipeline_identity("rgba8"),
            legacy.views[0].draws[0].pipeline_identity("rgba8"),
            "mask discard must never reuse an opaque Unlit pipeline"
        );
        let wire = serde_json::to_value(draws[0].as_ref()).unwrap();
        assert_eq!(
            wire["alphaCutoff"],
            serde_json::to_value(OrderedF32::from(0.3)).unwrap()
        );
        let restored: DrawGeometry = serde_json::from_value(wire).unwrap();
        restored.validate().unwrap();
        assert_eq!(&restored, draws[0].as_ref());
    }
    #[test]
    fn standard_lit_rejects_bad_factors_unknown_slots_and_incomplete_geometry() {
        let m = standard_lit_fixture::standard_lit([0.2, 0.4, 0.6, 1.]);
        m.validate().unwrap();
        for (name, value) in [
            ("metallic", -0.1),
            ("roughness", 1.1),
            ("normalScale", -1.),
            ("occlusionStrength", 1.01),
        ] {
            let mut bad = m.clone();
            bad.parameters
                .insert(name.into(), MaterialParameter::Float(value));
            assert!(bad.validate().is_err());
        }
        let mut bad = m.clone();
        bad.parameters.remove("emissive");
        assert!(bad.validate().is_err());
        let mut bad = m.clone();
        bad.shader_source = UNLIT_SHADER_SOURCE.into();
        assert!(bad.validate().is_err());
        assert!(geometry_fixture::triangle()
            .validate_standard_lit_inputs(0..3, false, false)
            .is_err());
        let g = standard_lit_fixture::lit_triangle();
        g.validate_standard_lit_inputs(0..3, false, false).unwrap();
        assert!(g.validate_standard_lit_inputs(0..3, true, true).is_err());
        assert!(g.validate_standard_lit_inputs(0..4, false, false).is_err());
    }
}

#[cfg(feature = "real-wgpu")]
mod gpu_s2a {
    use super::*;
    use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;

    fn mip_texture(
        manager: &mut RenderResourceManager,
        sampler: &str,
    ) -> engine_runtime::render_asset_production::PreparedTextureAsset {
        let mut pixels = [255, 0, 0, 255].repeat(16);
        pixels.extend([0, 255, 0, 255].repeat(4));
        pixels.extend([0, 0, 255, 255]);
        engine_runtime::render_asset_production::RuntimeRenderAssetProducer::new()
            .prepare_texture_payload(
                0,
                engine_runtime::runtime_texture::RuntimeTexturePayload {
                    asset_id: "mip-texture".into(),
                    cooked_asset_id: "mip-texture-cooked".into(),
                    width: 4,
                    height: 4,
                    format: "rgba8Unorm".into(),
                    color_space: "linear".into(),
                    mip_count: 3,
                    rgba8: pixels.into(),
                    sampler: sampler.into(),
                    source_hash: "mip-oracle-v1".into(),
                },
                RenderBindingKind::Texture,
                manager,
            )
            .unwrap()
    }

    fn textured_plan(
        manager: &mut RenderResourceManager,
        texture: &engine_runtime::render_asset_production::PreparedTextureAsset,
    ) -> engine_runtime::rhi_command_plan::RhiCommandPlan {
        let mut geometry = geometry_fixture::triangle();
        let positions = geometry.vertex_bytes.clone();
        geometry.vertex_bytes.clear();
        for (position, uv) in positions
            .chunks_exact(12)
            .zip([[0.0f32, 0.], [64., 0.], [0., 64.]])
        {
            geometry.vertex_bytes.extend_from_slice(position);
            geometry
                .vertex_bytes
                .extend(uv.into_iter().flat_map(f32::to_le_bytes));
        }
        geometry.vertex_stride = 20;
        geometry.attributes.push(VertexAttribute {
            semantic: VertexSemantic::TexCoord0,
            format: VertexFormat::Float32x2,
            offset: 12,
        });
        let mesh = geometry_fixture::prepare("mip-mesh", RenderAssetData::Mesh(geometry), manager);
        let mut data = geometry_fixture::unlit([1.; 4]);
        data.textures.insert(
            "baseColorTexture".into(),
            RenderAssetReference {
                expected_kind: RenderAssetKind::Texture,
                reference: engine_runtime::runtime_package::RuntimeAssetRef {
                    id: "mip-texture".into(),
                    asset_type: "texture".into(),
                    guid: Some("mip-guid".into()),
                    sub_asset: None,
                },
            },
        );
        let mut material =
            geometry_fixture::prepare("mip-material", RenderAssetData::Material(data), manager);
        material.binding.sampler = texture.payload.sampler.clone();
        material.binding.slots.push(NamedRenderBinding {
            name: "baseColorTexture".into(),
            resource: BindingResource::Texture(texture.handle),
        });
        let source = PreparedGeometry::from_assets(&mesh, &material, 0, manager).unwrap();
        compile_render_graph_to_rhi_plan(&geometry_fixture::graph(&geometry_fixture::frame(
            &source, manager,
        )))
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn mip_sampling_uses_uploaded_lower_levels_and_segmented_upload_respects_full_budget() {
        use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, GeometryUploadState};
        let mut manager = RenderResourceManager::new();
        let texture = mip_texture(
            &mut manager,
            "nearestMipmapNearest:nearest:clampToEdge:clampToEdge",
        );
        let plan = textured_plan(&mut manager, &texture);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: 83,
                staging_bytes: 8,
                bytes_per_frame: 8,
                max_tickets: 2,
            })
            .unwrap();
        assert!(
            backend
                .begin_geometry_upload(&[], std::slice::from_ref(&texture))
                .unwrap_err()
                .contains("resident_budget"),
            "84-byte chain must not be charged as its 64-byte base"
        );
        // Prepare identical mesh/parameters/clears without a texture binding.
        // The later 8-byte transfer profile must still render its own uploaded
        // mip chain, without an unrelated first-use arena upload hiding it.
        backend
            .set_geometry_upload_budget(GeometryUploadBudget::default())
            .unwrap();
        let mut untextured = plan.clone();
        for command in &mut untextured.commands {
            if let engine_runtime::rhi_command_plan::RhiCommand::Draw {
                payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Geometry { draw },
                ..
            } = command
            {
                std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(draw).material_binding)
                    .slots
                    .retain(|slot| slot.name != "baseColorTexture");
            }
        }
        backend
            .render_plan_to_rgba_bytes(&untextured, 64, 64)
            .unwrap();
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: GeometryUploadBudget::default().resident_bytes,
                staging_bytes: 8,
                bytes_per_frame: 8,
                max_tickets: 2,
            })
            .unwrap();
        let uploaded_before = backend.geometry_upload_snapshot().uploaded_bytes;
        let a = backend
            .begin_geometry_upload(&[], std::slice::from_ref(&texture))
            .unwrap();
        let b = backend
            .begin_geometry_upload(&[], std::slice::from_ref(&texture))
            .unwrap();
        backend.release_geometry_upload(a).unwrap();
        for frame in 1..=32 {
            let written = backend.pump_geometry_uploads(frame).unwrap();
            assert!(written <= 8);
            assert!(backend.geometry_upload_snapshot().staging_bytes <= 8);
            if let Some(token) = backend.last_gpu_submission() {
                backend
                    .wait_gpu_submission(token, std::time::Duration::from_secs(5))
                    .unwrap();
            }
            if backend.geometry_upload_state(b).unwrap() == GeometryUploadState::Ready {
                break;
            }
        }
        assert_eq!(
            backend.geometry_upload_state(b).unwrap(),
            GeometryUploadState::Ready
        );
        assert_eq!(
            backend.geometry_upload_snapshot().uploaded_bytes - uploaded_before,
            84
        );
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(
            &pixels[(32 * 64 + 32) * 4..(32 * 64 + 33) * 4],
            [0, 0, 255, 255],
            "minification must read the 1x1 blue mip, not the red base"
        );
        backend.release_geometry_upload(b).unwrap();
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let base_only = mip_texture(&mut manager, "nearestClamp");
        let base_plan = textured_plan(&mut manager, &base_only);
        backend
            .prepare_geometry_textures(std::slice::from_ref(&base_only))
            .unwrap();
        let pixels = backend
            .render_plan_to_rgba_bytes(&base_plan, 64, 64)
            .unwrap();
        assert_eq!(
            &pixels[(32 * 64 + 32) * 4..(32 * 64 + 33) * 4],
            [255, 0, 0, 255],
            "legacy non-mip sampler must stay on base level"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn alpha_mask_discards_below_cutoff_keeps_equality_and_preserves_old_packet() {
        let mut manager = RenderResourceManager::new();
        let mesh = geometry_fixture::prepare(
            "mesh",
            RenderAssetData::Mesh(geometry_fixture::triangle()),
            &mut manager,
        );
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let mut plans = Vec::new();
        for (id, alpha) in [("discard", 0.49), ("equal", 0.5), ("keep", 0.51)] {
            let material = geometry_fixture::prepare(
                id,
                RenderAssetData::Material(masked(alpha, 0.5)),
                &mut manager,
            );
            let prepared = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
            let frame = geometry_fixture::frame(&prepared, &manager);
            plans.push(compile_render_graph_to_rhi_plan(&geometry_fixture::graph(
                &frame,
            )));
        }
        for index in [0, 1, 2, 0, 1] {
            let pixels = backend
                .render_plan_to_rgba_bytes(&plans[index], 64, 64)
                .unwrap();
            let center = &pixels[(32 * 64 + 32) * 4..(32 * 64 + 33) * 4];
            if index == 0 {
                assert_eq!(
                    center,
                    [0, 0, 0, 255],
                    "discard must leave the clear/depth untouched"
                );
            } else {
                assert_eq!(
                    &center[..3],
                    [255, 0, 0],
                    "mask must not become alpha blending"
                );
                assert!(
                    (i32::from(center[3]) - (255.0f32 * [0.49, 0.5, 0.51][index]).round() as i32)
                        .abs()
                        <= 1
                );
            }
            if index == 1 {
                if let Some(root) = std::env::var_os("AIFE_S2A_EVIDENCE") {
                    let root = std::path::PathBuf::from(root);
                    std::fs::create_dir_all(&root).unwrap();
                    let file = std::fs::File::create(root.join("mask-equality.png")).unwrap();
                    let mut encoder = png::Encoder::new(file, 64, 64);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .unwrap()
                        .write_image_data(&pixels)
                        .unwrap();
                }
            }
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
}

mod s2b_wire {
    #[test]
    fn light_wire_accepts_directional_shadows_and_rejects_unknown_or_old_reader_loss() {
        let value = serde_json::json!({"domain":"world3d", "transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]},"light":{"schemaVersion":"light3d.v1","enabled":true,"color":[1,1,1],"kind":{"type":"directional","illuminanceLux":1},"layers":["world"]}});
        let light: engine_runtime::spatial_contract::RuntimeSpatial =
            serde_json::from_value(value.clone())
                .expect("scene light must be accepted by typed runtime DTO");
        assert_eq!(
            serde_json::to_value(light).unwrap()["light"]["kind"]["type"],
            "directional"
        );
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct PreviousLightReader {
            schema_version: String,
            enabled: bool,
            color: [f32; 3],
            kind: engine_runtime::spatial_contract::LightKind,
            layers: Vec<String>,
        }
        // The preceding Light3D reader's exact DTO shape, before castShadows.
        assert!(serde_json::from_value::<PreviousLightReader>(value["light"].clone()).is_ok());
        let mut shadow = value;
        shadow["light"]["castShadows"] = true.into();
        let decoded = serde_json::from_value::<engine_runtime::spatial_contract::RuntimeSpatial>(
            shadow.clone(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["light"]["castShadows"],
            true
        );
        assert!(serde_json::from_value::<PreviousLightReader>(shadow["light"].clone()).is_err());
        shadow["light"]["shadowMagic"] = true.into();
        assert!(
            serde_json::from_value::<engine_runtime::spatial_contract::RuntimeSpatial>(shadow)
                .is_err()
        );
    }
}

#[path = "support/s2b_lights.rs"]
mod s2b;

#[path = "support/s2b_targets.rs"]
mod s2b_targets;

#[path = "support/s2c_color.rs"]
mod s2c_color;

#[path = "support/s2d_variants.rs"]
mod s2d_variants;

mod s2d {
    use super::*;

    #[test]
    fn hdr_shader_mode_is_part_of_pipeline_identity_even_with_same_attachment_format() {
        let mut manager = RenderResourceManager::new();
        let source = geometry_fixture::source(&mut manager);
        let frame = geometry_fixture::frame(&source, &manager);
        let direct = frame.views[0].draws[0].as_ref().clone();
        let mut processed = direct.clone();
        std::sync::Arc::make_mut(&mut processed.view).color_pipeline = Some(Default::default());
        assert_ne!(
            direct.pipeline_identity("Rgba16Float"),
            processed.pipeline_identity("Rgba16Float"),
            "fog-enabled shader must not reuse direct shader at identical attachment format"
        );
        let mut changed_exposure = processed.clone();
        std::sync::Arc::make_mut(&mut changed_exposure.view)
            .color_pipeline
            .as_mut()
            .unwrap()
            .exposure_ev = 1.0.into();
        assert_eq!(
            processed.pipeline_identity("Rgba16Float"),
            changed_exposure.pipeline_identity("Rgba16Float"),
            "exposure is data, not a new shader variant"
        );
    }
}
