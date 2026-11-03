//! Bounded shader/layout coverage, not a Cartesian product of all project values.
#[cfg(feature = "real-wgpu")]
mod gpu_s2d {
    use super::super::*;
    use engine_runtime::color_pipeline::{AntiAliasing3D, ColorPipeline3D};
    use engine_runtime::render_asset_production::*;
    use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;

    const SLOTS: [&str; 5] = [
        "baseColorTexture",
        "normalTexture",
        "metallicRoughnessTexture",
        "occlusionTexture",
        "emissiveTexture",
    ];
    const TEXELS: [[u8; 4]; 5] = [
        [128, 192, 64, 255],
        [128, 128, 255, 255],
        [255, 192, 64, 255],
        [128, 255, 255, 255],
        [128, 64, 192, 255],
    ];

    fn display(x: f64) -> u8 {
        let tone = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
        let encoded = if tone <= 0.0031308 {
            tone * 12.92
        } else {
            1.055 * tone.powf(1. / 2.4) - 0.055
        };
        (encoded * 255.).round() as u8
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn all_five_slot_layout_masks_draw_reference_pixels_and_reuse_warm_resources() {
        layout_masks(false);
    }

    #[test]
    #[ignore = "requires real GPU"]
    fn all_ibl_five_slot_layout_masks_draw_reference_pixels_and_reuse_warm_resources() {
        layout_masks(true);
    }

    fn layout_masks(with_ibl: bool) {
        let environment = with_ibl.then(|| s2c_ibl::map(1.));
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let mut manager = RenderResourceManager::new();
        let textures: Vec<_> = TEXELS
            .iter()
            .enumerate()
            .map(|(i, texel)| {
                RuntimeRenderAssetProducer::new()
                    .prepare_texture_payload(
                        0,
                        engine_runtime::runtime_texture::RuntimeTexturePayload {
                            asset_id: format!("variant-texture-{i}"),
                            cooked_asset_id: format!("variant-texture-{i}-cooked"),
                            width: 1,
                            height: 1,
                            format: "rgba8Unorm".into(),
                            color_space: "linear".into(),
                            mip_count: 1,
                            rgba8: texel.to_vec().into(),
                            sampler: "nearestClamp".into(),
                            source_hash: format!("variant-texel-{i}"),
                        },
                        RenderBindingKind::Texture,
                        &mut manager,
                    )
                    .unwrap()
            })
            .collect();
        for texture in &textures {
            backend
                .register_cooked_texture(texture.handle.into(), &texture.payload)
                .unwrap();
        }
        let mut geometry = standard_lit_fixture::lit_triangle();
        geometry.vertex_bytes = geometry
            .vertex_bytes
            .chunks_exact(24)
            .zip([[0f32, 0.], [1., 0.], [0.5, 1.]])
            .flat_map(|(v, uv)| {
                v.iter()
                    .copied()
                    .chain(uv.into_iter().flat_map(f32::to_le_bytes))
            })
            .collect();
        geometry.vertex_stride = 32;
        geometry.attributes.push(VertexAttribute {
            semantic: VertexSemantic::TexCoord0,
            format: VertexFormat::Float32x2,
            offset: 24,
        });
        let mesh = geometry_fixture::prepare(
            "variant-mesh",
            RenderAssetData::Mesh(geometry),
            &mut manager,
        );
        let mut cold_ms = Vec::new();
        let mut warm_ms = Vec::new();
        for mask in 0u8..32 {
            let mut material = standard_lit_fixture::standard_lit([0.8, 0.4, 0.2, 1.]);
            material
                .parameters
                .insert("metallic".into(), MaterialParameter::Float(0.5));
            material.parameters.insert(
                "emissive".into(),
                MaterialParameter::Vector4([0.2, 0.1, 0.05, 0.]),
            );
            for (i, slot) in SLOTS.iter().enumerate() {
                if mask & (1 << i) == 0 {
                    continue;
                }
                material.textures.insert(
                    (*slot).into(),
                    RenderAssetReference {
                        expected_kind: RenderAssetKind::Texture,
                        reference: engine_runtime::runtime_package::RuntimeAssetRef {
                            id: format!("variant-texture-{i}"),
                            asset_type: "texture".into(),
                            guid: Some(format!("variant-guid-{i}")),
                            sub_asset: None,
                        },
                    },
                );
                material.texture_sampling.insert(
                    (*slot).into(),
                    engine_runtime::runtime_texture::TextureSampler::parse("nearestClamp").unwrap(),
                );
            }
            let mut prepared = geometry_fixture::prepare(
                &format!("variant-mat-{mask}"),
                RenderAssetData::Material(material),
                &mut manager,
            );
            for (i, slot) in SLOTS.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    prepared.binding.slots.push(NamedRenderBinding {
                        name: (*slot).into(),
                        resource: BindingResource::Texture(textures[i].handle),
                    });
                }
            }
            let source = PreparedGeometry::from_assets(&mesh, &prepared, 0, &manager).unwrap();
            let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
            if with_ibl {
                view.projection = Projection::Orthographic {
                    size: 2.,
                    near: 0.1,
                    far: 10.,
                };
            }
            view.color_pipeline = Some(ColorPipeline3D {
                ibl: with_ibl.then(s2c_ibl::config),
                anti_aliasing: AntiAliasing3D::None,
                ambient_diffuse: [0.25, 0.5, 0.75].map(Into::into),
                ..Default::default()
            });
            let mut frame = GeometryFrame::build(
                "main",
                [64, 64],
                &[GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                }],
                &[geometry_fixture::instance(&source, 1, [0., 0., -2.])],
                &manager,
                DiagnosticLevel::Off,
            )
            .unwrap();
            if let Some(map) = &environment {
                for draw in &mut frame.views[0].draws {
                    std::sync::Arc::make_mut(draw).environment = Some(std::sync::Arc::new(
                        engine_runtime::environment_lighting::EnvironmentLighting {
                            map: map.clone(),
                            intensity: 1.0.into(),
                        },
                    ));
                }
            }
            let plan = compile_render_graph_to_rhi_plan(&geometry_fixture::graph(&frame));
            assert!(!plan.has_errors(), "mask={mask}: {:?}", plan.diagnostics);
            let cold = std::time::Instant::now();
            let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
            cold_ms.push(cold.elapsed().as_secs_f64() * 1000.);
            let center = &pixels[(32 * 64 + 32) * 4..][..4];
            for c in 0..3 {
                let base = [0.8, 0.4, 0.2][c]
                    * if mask & 1 != 0 {
                        f64::from(TEXELS[0][c]) / 255.
                    } else {
                        1.
                    };
                let metal = 0.5
                    * if mask & 4 != 0 {
                        f64::from(TEXELS[2][2]) / 255.
                    } else {
                        1.
                    };
                let ao = if mask & 8 != 0 {
                    f64::from(TEXELS[3][0]) / 255.
                } else {
                    1.
                };
                let emission = [0.2, 0.1, 0.05][c]
                    * if mask & 16 != 0 {
                        f64::from(TEXELS[4][c]) / 255.
                    } else {
                        1.
                    };
                let f0 = 0.04 * (1. - metal) + base * metal;
                let mut linear =
                    (1. - f0) * (1. - metal) * base * [0.25, 0.5, 0.75][c] * ao + emission;
                if with_ibl {
                    // Orthographic +Z eye direction, flat geometry, explicit
                    // normal texel. SH is constant and each cubemap face is
                    // constant, so this f64 oracle does not copy GPU indexing.
                    let nv: f64 = if mask & 2 == 0 {
                        1.
                    } else {
                        1. / (1.0f64 + 2. / (255. * 255.)).sqrt()
                    };
                    let rough = 0.5
                        * if mask & 4 == 0 {
                            1.
                        } else {
                            f64::from(TEXELS[2][1]) / 255.
                        };
                    let fresnel = f0 + ((1. - rough).max(f0) - f0) * (1. - nv).powi(5);
                    let radiance = s2c_ibl::face_color(4, 0)[c] * (1. - rough)
                        + s2c_ibl::face_color(4, 1)[c] * rough;
                    let lut_a = 0.2 + 0.3 * nv + 0.1 * rough;
                    let lut_b = 0.05 * rough;
                    linear += ((1. - fresnel) * (1. - metal) * base * [0.4, 0.6, 0.8][c]
                        + radiance * (f0 * lut_a + lut_b))
                        * ao;
                }
                assert!(
                    center[c].abs_diff(display(linear)) <= 2,
                    "mask={mask} channel={c} actual={center:?} expected={}",
                    display(linear)
                );
            }
            assert_eq!(center[3], 255);
            let before = backend.retained_geometry_stats();
            let resident = backend.geometry_residency();
            let warm = std::time::Instant::now();
            assert_eq!(
                backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap(),
                pixels
            );
            warm_ms.push(warm.elapsed().as_secs_f64() * 1000.);
            let after = backend.retained_geometry_stats();
            assert_eq!(after.bind_groups_created, before.bind_groups_created);
            assert_eq!(
                after.parameter_uploaded_bytes,
                before.parameter_uploaded_bytes
            );
            assert_eq!(backend.geometry_residency().pipelines, resident.pipelines);
            assert_eq!(backend.geometry_upload_count(), 1);
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
        println!(
            "s2d_slot_masks=32 ibl={with_ibl} cold_draw_readback_ms={cold_ms:?} warm_draw_readback_ms={warm_ms:?}"
        );
    }
}
