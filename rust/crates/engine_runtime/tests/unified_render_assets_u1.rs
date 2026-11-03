use engine_runtime::render_contract::*;
use engine_runtime::render_resource::*;

fn mesh() -> CookedGeometry {
    match RenderContract::<RenderAssetData>::decode(include_str!("fixtures/render_mesh_v1.json"))
        .unwrap()
        .data
    {
        RenderAssetData::Mesh(v) => v,
        _ => unreachable!(),
    }
}
fn material() -> CookedMaterial {
    match RenderContract::<RenderAssetData>::decode(include_str!(
        "fixtures/render_material_v1.json"
    ))
    .unwrap()
    .data
    {
        RenderAssetData::Material(v) => v,
        _ => unreachable!(),
    }
}
fn handle(kind: RenderResourceKind) -> RenderResourceHandle {
    RenderResourceHandle {
        kind,
        index: 12,
        generation: 3,
    }
}

#[test]
fn native_geometry_preserves_actual_bytes_ranges_and_bounds() {
    let m = mesh();
    assert_eq!(&m.vertex_bytes[12..16], &1f32.to_le_bytes());
    assert_eq!(&m.index_bytes, &[0, 0, 1, 0, 2, 0]);
    let wire =
        serde_json::to_string(&RenderContract::new(RenderAssetData::Mesh(m.clone()))).unwrap();
    assert_eq!(
        RenderContract::<RenderAssetData>::decode(&wire)
            .unwrap()
            .data,
        RenderAssetData::Mesh(m)
    );
}

#[test]
fn geometry_rejects_missing_truncated_and_nonfinite_vertex_data() {
    let mut m = mesh();
    m.vertex_bytes.pop();
    assert_eq!(m.validate().unwrap_err().field, "geometry.vertexBytes");
    let mut m = mesh();
    m.vertex_bytes[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(
        m.validate().unwrap_err().field,
        "geometry.vertexBytes.nonFinite"
    );
    let mut m = mesh();
    m.vertex_count = u32::MAX;
    m.vertex_stride = u32::MAX;
    assert!(m.validate().is_err());
}

#[test]
fn geometry_rejects_layout_overlap_duplicates_and_wrong_formats() {
    let mut m = mesh();
    m.attributes.push(m.attributes[0].clone());
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.attributes[0].offset = 4;
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.attributes[0].format = VertexFormat::Float32x2;
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.attributes.clear();
    assert!(m.validate().is_err());
}

#[test]
fn geometry_checks_index_content_and_both_index_formats() {
    let mut m = mesh();
    m.index_bytes[4] = 3;
    assert_eq!(m.validate().unwrap_err().field, "geometry.indexBytes.range");
    let mut m = mesh();
    m.index_bytes.pop();
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.index_format = GeometryIndexFormat::Uint32;
    m.index_bytes = [0u32, 1, 2]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    m.validate().unwrap();
}

#[test]
fn geometry_rejects_submesh_gaps_overlaps_and_unknown_material_slots() {
    let mut m = mesh();
    m.submeshes[0].first_index = 1;
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.submeshes.push(m.submeshes[0].clone());
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.submeshes[0].material_slot = "absent".into();
    assert!(m.validate().is_err());
    let mut m = mesh();
    m.material_slots.push("surface".into());
    assert!(m.validate().is_err());
}

#[test]
fn geometry_bounds_must_enclose_real_positions() {
    let mut m = mesh();
    m.bounds.max[0] = 0.9;
    assert_eq!(m.validate().unwrap_err().field, "geometry.bounds.aabb");
    let mut m = mesh();
    m.bounds.sphere_radius = 0.1;
    assert_eq!(m.validate().unwrap_err().field, "geometry.bounds.sphere");
    let mut m = mesh();
    m.bounds.min[0] = f32::NAN;
    assert!(m.validate().is_err());
}

#[test]
fn material_has_typed_values_and_pipeline_identity_tracks_only_pipeline_state() {
    let mut m = material();
    let first = m.pipeline_identity().unwrap();
    assert!(is_sha256_digest(&first));
    m.parameters
        .insert("baseColor".into(), MaterialParameter::Vector4([1.0; 4]));
    assert_eq!(first, m.pipeline_identity().unwrap());
    m.cull = MaterialCull::None;
    assert_ne!(first, m.pipeline_identity().unwrap());
}

#[test]
fn material_rejects_unsupported_parameters_types_and_shader_mapping() {
    let mut m = material();
    m.parameters
        .insert("roughness".into(), MaterialParameter::Float(0.4));
    assert_eq!(
        m.validate().unwrap_err().code,
        ContractErrorCode::UnsupportedMaterialParameter
    );
    let mut m = material();
    m.parameters
        .insert("baseColor".into(), MaterialParameter::Float(1.0));
    assert!(m.validate().is_err());
    let mut m = material();
    m.shader_source = "project/shader.wgsl".into();
    assert!(m.validate().is_err());
}

#[test]
fn native_input_rejects_unknown_schema_fields_and_tags() {
    let wire = include_str!("fixtures/render_mesh_v1.json");
    for invalid in [
        wire.replace("render-contract.v1", "render-contract.v2"),
        wire.replace("vertexCount", "vertxCount"),
        wire.replace("triangleList", "unknown"),
        wire.replace("\"payload\": {", "\"payload\": {\"unexpected\":true,"),
    ] {
        assert!(RenderContract::<RenderAssetData>::decode(&invalid).is_err());
    }
}

#[test]
fn typed_handles_reject_wrong_kinds_in_rust_and_wire() {
    assert!(GeometryHandle::try_from(handle(RenderResourceKind::Texture)).is_err());
    assert!(MaterialHandle::try_from(handle(RenderResourceKind::MeshBuffer)).is_err());
    let wire = serde_json::to_string(&handle(RenderResourceKind::MaterialParams)).unwrap();
    assert!(serde_json::from_str::<TextureHandle>(&wire).is_err());
    let h = TextureHandle::try_from(handle(RenderResourceKind::Texture)).unwrap();
    assert_eq!(h.raw().generation, 3);
}

#[test]
fn named_binding_ignores_order_and_rejects_duplicates_and_wrong_primary() {
    let mut binding = RenderBindingSet::single(
        "texture".into(),
        RenderBindingKind::Texture,
        handle(RenderResourceKind::Texture),
        "linearClamp".into(),
        false,
        "test".into(),
    )
    .unwrap();
    binding.slots.insert(
        0,
        NamedRenderBinding::new(
            "otherTexture",
            RenderResourceHandle {
                index: 99,
                ..handle(RenderResourceKind::Texture)
            },
        ),
    );
    assert_eq!(binding.texture("baseColorTexture").unwrap().raw().index, 12);
    binding.slots[0].name = "baseColorTexture".into();
    assert!(binding.validate().is_err());
    assert!(RenderBindingSet::single(
        "bad".into(),
        RenderBindingKind::Texture,
        handle(RenderResourceKind::MeshBuffer),
        "linearClamp".into(),
        false,
        "".into()
    )
    .is_err());
    let mut material = RenderBindingSet::single(
        "material".into(),
        RenderBindingKind::Material,
        handle(RenderResourceKind::MaterialParams),
        "linearClamp".into(),
        false,
        "".into(),
    )
    .unwrap();
    material.slots.push(NamedRenderBinding::new(
        "baseColorTexture",
        handle(RenderResourceKind::MeshBuffer),
    ));
    assert_eq!(
        material.validate().unwrap_err().code,
        ContractErrorCode::ResourceTypeMismatch
    );
}

#[test]
fn legacy_binding_normalizes_once_and_preserves_fallback_and_all_handles() {
    let wire = serde_json::json!({"bindingId":"old", "bindingKind":"texture", "resources":[handle(RenderResourceKind::Texture), handle(RenderResourceKind::MeshBuffer)], "materialHandle": handle(RenderResourceKind::MaterialParams), "sampler":"nearestClamp", "fallbackUsed":true, "debugLabel":"old"});
    let binding: RenderBindingSet = serde_json::from_value(wire).unwrap();
    assert!(binding.fallback_used);
    assert_eq!(binding.slots.len(), 3);
    let wire = serde_json::to_value(&binding).unwrap();
    assert!(wire.get("resources").is_none());
    assert_eq!(
        serde_json::from_value::<RenderBindingSet>(wire.clone()).unwrap(),
        binding
    );
    let mut mixed = wire;
    mixed["resources"] = serde_json::json!([]);
    assert!(serde_json::from_value::<RenderBindingSet>(mixed).is_err());
}

#[test]
fn logical_binding_detects_hot_update_release_recycled_generation_and_device_loss() {
    let mut manager = RenderResourceManager::with_safe_frame_delay(0);
    let h = manager.request_resource(0, RenderResourceRequest::texture("a", "1", 4));
    let binding = RenderBindingSet::single(
        "a".into(),
        RenderBindingKind::Texture,
        h,
        "linearClamp".into(),
        false,
        "".into(),
    )
    .unwrap();
    binding.validate_residency(&manager).unwrap();
    manager.request_resource(1, RenderResourceRequest::texture("a", "2", 4));
    assert_eq!(
        binding.validate_residency(&manager).unwrap_err().code,
        ContractErrorCode::StaleResourceHandle
    );
    manager
        .release_resource(RenderResourceReleaseRequest {
            handle: h,
            frame_index: 1,
        })
        .unwrap();
    manager.end_frame(1);
    let current = manager.request_resource(2, RenderResourceRequest::texture("b", "1", 4));
    assert_eq!(h.index, current.index);
    assert_ne!(h.generation, current.generation);
    assert_eq!(
        validate_resident(h, &manager).unwrap_err().code,
        ContractErrorCode::StaleResourceHandle
    );
    manager.mark_device_lost(2);
    assert_eq!(
        validate_resident(current, &manager).unwrap_err().code,
        ContractErrorCode::ResourceNotReady
    );
}

fn texture_payload() -> engine_runtime::runtime_texture::RuntimeTexturePayload {
    engine_runtime::runtime_texture::RuntimeTexturePayload {
        asset_id: "texture".into(),
        cooked_asset_id: "cooked-texture".into(),
        width: 1,
        height: 1,
        format: "rgba8Unorm".into(),
        color_space: "srgb".into(),
        mip_count: 1,
        rgba8: vec![1, 2, 3, 255].into(),
        sampler: "nearestClamp".into(),
        source_hash: "source".into(),
    }
}

#[test]
fn texture_sampling_variants_coexist_and_all_typed_settings_roundtrip() {
    use engine_runtime::render_asset_production::*;
    use engine_runtime::runtime_texture::{TextureFilter::*, TextureSampler, TextureWrap::*};
    let producer = RuntimeRenderAssetProducer::new();
    let mut manager = RenderResourceManager::new();
    let mut keys = std::collections::BTreeSet::new();
    let mut prepared = Vec::new();
    for min_filter in [Nearest, Linear] {
        for mag_filter in [Nearest, Linear] {
            for wrap_s in [Repeat, ClampToEdge, MirroredRepeat] {
                for wrap_t in [Repeat, ClampToEdge, MirroredRepeat] {
                    let sampler = TextureSampler {
                        min_filter,
                        mag_filter,
                        wrap_s,
                        wrap_t,
                    };
                    assert_eq!(TextureSampler::parse(&sampler.key()), Some(sampler));
                    assert!(keys.insert(sampler.key()));
                    let mut payload = texture_payload();
                    payload.sampler = sampler.key();
                    let item = producer
                        .prepare_texture_payload(
                            0,
                            payload.clone(),
                            RenderBindingKind::Texture,
                            &mut manager,
                        )
                        .unwrap();
                    let reused = producer
                        .prepare_texture_payload(
                            0,
                            payload,
                            RenderBindingKind::Texture,
                            &mut manager,
                        )
                        .unwrap();
                    assert_eq!(item.handle, reused.handle);
                    prepared.push(item);
                }
            }
        }
    }
    assert_eq!(keys.len(), 36);
    for item in &prepared {
        item.binding
            .validate_residency(&manager)
            .expect("simultaneous sampler variants must remain resident");
    }
    for invalid in [
        "nearest",
        "nearestClampExtra",
        "linear:linear:clampToEdge:clampToEdge",
        "nearest:linear:repeat:repeat:extra",
    ] {
        assert!(TextureSampler::parse(invalid).is_none());
        let mut payload = texture_payload();
        payload.sampler = invalid.into();
        assert!(producer
            .prepare_texture_payload(0, payload, RenderBindingKind::Texture, &mut manager)
            .is_err());
    }
}

#[test]
fn texture_and_font_share_preparation_and_invalidate_by_actual_pixels() {
    use engine_runtime::render_asset_production::*;
    let producer = RuntimeRenderAssetProducer::new();
    let mut manager = RenderResourceManager::new();
    let texture = producer
        .prepare_texture_payload(
            0,
            texture_payload(),
            RenderBindingKind::Texture,
            &mut manager,
        )
        .unwrap();
    let atlas = producer
        .prepare_texture_payload(
            0,
            texture_payload(),
            RenderBindingKind::FontAtlas,
            &mut manager,
        )
        .unwrap();
    assert_eq!(texture.handle, atlas.handle);
    assert_eq!(atlas.binding.texture("fontAtlas"), Some(texture.handle));
    assert_eq!(atlas.state, RenderAssetPreparationState::Prepared);
    let mut payload = texture_payload();
    std::sync::Arc::make_mut(&mut payload.rgba8)[0] = 9;
    let changed = producer
        .prepare_texture_payload(1, payload, RenderBindingKind::Texture, &mut manager)
        .unwrap();
    assert_ne!(texture.handle, changed.handle);
    assert_eq!(
        texture
            .binding
            .validate_residency(&manager)
            .unwrap_err()
            .code,
        ContractErrorCode::StaleResourceHandle
    );
    let mut bad = texture_payload();
    std::sync::Arc::make_mut(&mut bad.rgba8).pop();
    assert!(producer
        .prepare_texture_payload(2, bad, RenderBindingKind::Texture, &mut manager)
        .is_err());
}

#[test]
fn deferred_preparation_retries_the_same_resource_without_claiming_ready_or_leaking_handles() {
    use engine_runtime::render_asset_production::*;
    let producer = RuntimeRenderAssetProducer::new();
    let mut manager = RenderResourceManager::new();
    manager.set_upload_budget(RenderUploadBudget::new(4));
    manager.request_resource(0, RenderResourceRequest::texture("occupy", "1", 4));
    let first = producer
        .prepare_texture_payload(
            0,
            texture_payload(),
            RenderBindingKind::Texture,
            &mut manager,
        )
        .unwrap();
    assert_eq!(first.state, RenderAssetPreparationState::Deferred);
    let retry = producer
        .prepare_texture_payload(
            0,
            texture_payload(),
            RenderBindingKind::Texture,
            &mut manager,
        )
        .unwrap();
    assert_eq!(
        first.handle, retry.handle,
        "deferred requests must share their pending identity"
    );
    manager.end_frame(0);
    let ready = producer
        .prepare_texture_payload(
            1,
            texture_payload(),
            RenderBindingKind::Texture,
            &mut manager,
        )
        .unwrap();
    assert_eq!(ready.handle, first.handle);
    assert_eq!(ready.state, RenderAssetPreparationState::Prepared);
    ready.binding.validate_residency(&manager).unwrap();
}

#[test]
fn render_target_versions_follow_descriptor_and_deferred_status_is_honest() {
    use engine_runtime::render_asset_production::*;
    let producer = RuntimeRenderAssetProducer::new();
    let mut manager = RenderResourceManager::new();
    let request = RuntimeRenderAssetRequest::render_target(
        0,
        "main",
        20,
        20,
        "Bgra8UnormSrgb",
        RuntimeRenderAssetUsage::SurfaceRenderTarget,
    );
    let first = producer.produce_from_descriptor(&request, &mut manager);
    assert_eq!(first.handle.status, RuntimeRenderAssetStatus::Ready);
    let same = producer.produce_from_descriptor(&request, &mut manager);
    assert_eq!(same.handle.resource_handle, first.handle.resource_handle);
    assert_eq!(same.report.reused_count, 1);
    assert_eq!(same.report.uploaded_bytes, 0);
    let changed = RuntimeRenderAssetRequest::render_target(
        1,
        "main",
        21,
        20,
        "Bgra8UnormSrgb",
        RuntimeRenderAssetUsage::SurfaceRenderTarget,
    );
    let second = producer.produce_from_descriptor(&changed, &mut manager);
    assert_eq!(
        first
            .handle
            .binding
            .unwrap()
            .validate_residency(&manager)
            .unwrap_err()
            .code,
        ContractErrorCode::StaleResourceHandle
    );
    assert_ne!(first.handle.resource_handle, second.handle.resource_handle);
    manager.set_upload_budget(RenderUploadBudget {
        max_bytes_per_frame: Some(4),
        uploaded_bytes_this_frame: 4,
        deferred_request_count: 0,
    });
    let deferred = producer.produce_from_descriptor(
        &RuntimeRenderAssetRequest::render_target(
            1,
            "other",
            20,
            20,
            "Bgra8UnormSrgb",
            RuntimeRenderAssetUsage::SurfaceRenderTarget,
        ),
        &mut manager,
    );
    assert_eq!(deferred.handle.status, RuntimeRenderAssetStatus::Deferred);
    assert!(deferred.handle.binding.is_none());
    assert_eq!(deferred.report.uploaded_bytes, 0);
}

#[test]
fn resource_diagnostics_preserve_stage_and_retry_without_default_report_work() {
    let error = validate_resident(
        handle(RenderResourceKind::Texture),
        &RenderResourceManager::new(),
    )
    .unwrap_err();
    assert_eq!(error.stage, ProducerStage::AssetBinding);
    let provenance = Provenance::legacy(ProducerStage::ResourcePreparation);
    let mut off = ContractDiagnostics::default();
    off.record(error, RenderDomain::World3d, &provenance, || {
        panic!("Off must not build detail")
    });
    assert!(off.entries.is_empty());
    let mut summary = ContractDiagnostics::new(DiagnosticLevel::Summary);
    summary.record(error, RenderDomain::World3d, &provenance, || {
        DiagnosticDetails {
            actual: "generation 3 is absent".into(),
            dependency_chain: vec!["material -> texture".into()],
        }
    });
    assert_eq!(
        summary.entries[0].retryability,
        Retryability::AfterResourcePreparation
    );
    assert_eq!(summary.entries[0].stage, ProducerStage::AssetBinding);
}
