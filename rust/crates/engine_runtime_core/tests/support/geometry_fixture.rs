//! Shared real Prepare/resource/neutral geometry fixture for owner and GPU tests.
use engine_runtime::render_asset_production::*;
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_graph::{RenderGraph, RenderPassCommand, RenderResource};
use engine_runtime::render_resource::*;

pub(crate) fn graph(frame: &GeometryFrame) -> RenderGraph {
    let mut graph = RenderGraph::new("u2", 1);
    graph.output_target = Some(frame.target.clone());
    graph.resources.push(RenderResource::surface_backbuffer(
        &frame.target,
        frame.extent[0],
        frame.extent[1],
    ));
    graph.passes.push(engine_runtime::render_graph::RenderPass {
        pass_id: "clear".into(),
        pass_name: "clear".into(),
        pass_kind: engine_runtime::render_graph::RenderPassKind::Clear,
        view_id: "global".into(),
        reads: vec![],
        writes: vec![frame.target.clone()],
        color_targets: vec![frame.target.clone()],
        depth_target: None,
        commands: vec![RenderPassCommand::Clear {
            target: frame.target.clone(),
            color: engine_runtime::render_graph::color([0., 0., 0., 1.]),
        }]
        .into(),
        debug_source: None,
    });
    frame.append_to_graph(&mut graph);
    graph
}

pub(crate) fn frame(source: &PreparedGeometry, manager: &RenderResourceManager) -> GeometryFrame {
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[instance(source, 1, [0., 0., -2.])],
        manager,
        DiagnosticLevel::Summary,
    )
    .unwrap()
}

pub(crate) fn triangle() -> CookedGeometry {
    CookedGeometry {
        vertex_bytes: [-0.8f32, -0.8, 0., 0.8, -0.8, 0., 0., 0.8, 0.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
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
        vertex_count: 3,
        index_count: 3,
        index_format: GeometryIndexFormat::Uint16,
        topology: GeometryTopology::TriangleList,
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
        coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        material_slots: vec!["surface".into()],
    }
}

pub(crate) fn unlit(color: [f32; 4]) -> CookedMaterial {
    CookedMaterial {
        decal_bias: None,
        base_color_texture_sampling: None,
        opaque_alpha: false,
        ignore_vertex_color: false,
        preset: MaterialPreset::Unlit,
        texture_sampling: Default::default(),
        parameters: [("baseColor".into(), MaterialParameter::Vector4(color))].into(),
        textures: Default::default(),
        blend: MaterialBlend::Opaque,
        cull: MaterialCull::None,
        depth: MaterialDepth::TestAndWrite,
        shader_source: UNLIT_SHADER_SOURCE.into(),
    }
}

pub(crate) fn prepare(
    id: &str,
    asset: RenderAssetData,
    manager: &mut RenderResourceManager,
) -> PreparedCookedRenderAsset {
    asset.validate().unwrap();
    let (kind, binding_kind, source, bytes) = match &asset {
        RenderAssetData::Mesh(m) => (
            RenderResourceKind::MeshBuffer,
            RenderBindingKind::Mesh,
            RenderResourceSource::MeshBufferDescriptor {
                vertex_count: m.vertex_count,
                index_count: m.index_count,
            },
            m.vertex_bytes.len() + m.index_bytes.len(),
        ),
        RenderAssetData::Material(m) => (
            RenderResourceKind::MaterialParams,
            RenderBindingKind::Material,
            RenderResourceSource::MaterialParamsDescriptor {
                param_count: m.parameters.len() as u32,
            },
            16,
        ),
    };
    let request = RenderResourceRequest {
        key: RenderAssetKey::new(id, "1", kind),
        source,
        byte_len: bytes,
        reason: "u2-test".into(),
    };
    let handle = manager.request_resource(0, request.clone());
    let cooked = CookedRenderAsset {
        source_digest: format!("sha256:{}", "1".repeat(64)),
        recipe: NATIVE_RENDER_COOK_RECIPE.into(),
        asset,
    };
    let cooked_digest = engine_runtime::canonical_digest::sha256_prefixed(
        &serde_json::to_vec(&RenderContract::new(cooked.clone())).unwrap(),
    );
    PreparedCookedRenderAsset {
        asset: cooked,
        binding: RenderBindingSet::single(
            id.into(),
            binding_kind,
            handle,
            "nearestClamp".into(),
            false,
            id.into(),
        )
        .unwrap(),
        state: RenderAssetPreparationState::Prepared,
        textures: vec![],
        resource_request: request,
        asset_guid: id.into(),
        cooked_digest,
        source_path: Some(format!("assets/{id}.asset")),
        pipeline_identity: None,
    }
}

pub(crate) fn source(manager: &mut RenderResourceManager) -> PreparedGeometry {
    let mesh = prepare("mesh", RenderAssetData::Mesh(triangle()), manager);
    let mat = prepare(
        "material",
        RenderAssetData::Material(unlit([1., 0., 0., 1.])),
        manager,
    );
    PreparedGeometry::from_assets(&mesh, &mat, 0, manager).unwrap()
}

pub(crate) fn instance(
    source: &PreparedGeometry,
    id: u64,
    position: [f32; 3],
) -> GeometryInstance<'_> {
    let mut transform = IDENTITY_MATRIX;
    transform[12..15].copy_from_slice(&position);
    GeometryInstance {
        instance_id: id,
        source,
        world_transform: transform,
        visibility: Visibility {
            authored: true,
            inherited: Evaluation::Known(true),
        },
        layers: LayerSelection::Named {
            names: vec!["main".into()],
        },
        provenance: Provenance::unspecified(ProducerStage::RenderProjection),
    }
}
