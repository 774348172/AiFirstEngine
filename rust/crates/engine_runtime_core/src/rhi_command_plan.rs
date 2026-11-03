use serde::{Deserialize, Serialize};

use crate::font_bundle::FontBundleRenderMode;
use crate::render_asset_production::RenderBindingSet;
use crate::render_graph::{
    RenderDrawVertex, RenderGraph, RenderGraphDiagnostic, RenderGraphDiagnosticSeverity,
    RenderPassCommand, RenderPassKind, UiVertexStream,
};
use crate::render_resource::RenderResourceHandle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RhiCommandPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_identity: Option<std::sync::Arc<crate::render_thread::RenderFrameIdentity>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_schedule: Option<std::sync::Arc<crate::render_graph::CompiledGraphSchedule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_content_version: Option<u64>,
    pub frame_index: u64,
    pub graph_id: String,
    pub target_kind: String,
    pub commands: crate::render_graph::SharedRenderList<RhiCommand>,
    pub diagnostics: Vec<RenderGraphDiagnostic>,
}

impl RhiCommandPlan {
    pub fn has_errors(&self) -> bool {
        self.frame_identity
            .as_ref()
            .is_some_and(|id| !id.matches_frame(self.frame_index))
            || self
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == RenderGraphDiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "commandKind")]
pub enum RhiCommand {
    ResolveColor3d {
        resolve: crate::color_pipeline::ColorResolve3D,
    },
    BeginView {
        target: String,
        view: std::sync::Arc<crate::render_contract::ExecutableRenderView>,
        depth_target: Option<String>,
    },
    ReconcileParticles {
        sources: Vec<crate::particle_render_contract::ParticleSourceFrame>,
    },
    SimulateParticles {
        instance: u64,
        step: crate::particle_render_contract::ParticleRenderStep,
    },
    BeginFrame {
        target: String,
    },
    Clear {
        target: String,
        color: [crate::render_graph::OrderedF32; 4],
    },
    Draw {
        target: std::sync::Arc<str>,
        draw_kind: RhiDrawKind,
        vertex_count: u32,
        payload: RhiDrawPayload,
    },
    Submit,
    Present {
        target: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RhiDrawKind {
    Geometry,
    Particles,
    TestGeometry,
    MeshBasic,
    SpriteBasic,
    SpriteTextured,
    UiOverlay,
    UiComposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "payloadKind")]
pub enum RhiDrawPayload {
    GeometryBatch {
        batch: std::sync::Arc<crate::render_geometry::GeometryBatch>,
    },
    Geometry {
        draw: std::sync::Arc<crate::render_geometry::DrawGeometry>,
    },
    Particles {
        instance: u64,
        emitter: u32,
        view: crate::particle_render_contract::ParticleRenderView,
        texture: Option<RenderResourceHandle>,
        mesh: Option<RenderResourceHandle>,
    },
    TestGeometry {
        debug_label: String,
    },
    MeshBasic {
        mesh_ref: String,
        material_ref: Option<String>,
        pipeline_key: String,
    },
    SpriteBasic {
        sprite_ref: String,
        material_ref: Option<String>,
        sort_key: String,
        pipeline_key: String,
    },
    SpriteTextured {
        sprite_ref: String,
        material_ref: Option<String>,
        sort_key: String,
        texture: Option<RenderResourceHandle>,
        binding: Option<RenderBindingSet>,
        fallback_used: bool,
        pipeline_key: String,
        vertices: Vec<RenderDrawVertex>,
    },
    UiOverlay {
        item_count: usize,
        text_count: usize,
        image_count: usize,
        glyph_count: usize,
        font_atlas_id: Option<String>,
        text_pass_inserted: bool,
        debug_label: String,
        pipeline_key: String,
    },
    UiRetained {
        draws: Vec<crate::aui_retained::UiRetainedDraw>,
    },
    UiComposition {
        stage: String,
        item_count: usize,
        text_count: usize,
        image_count: usize,
        glyph_count: usize,
        font_atlas_id: Option<String>,
        text_pass_inserted: bool,
        debug_label: String,
        pipeline_key: String,
        texture: Option<RenderResourceHandle>,
        font_render_mode: Option<FontBundleRenderMode>,
        font_page_index: Option<u32>,
        vertices: UiVertexStream,
    },
}

impl RhiDrawPayload {
    pub fn geometry_draw(&self) -> Option<&std::sync::Arc<crate::render_geometry::DrawGeometry>> {
        match self {
            Self::Geometry { draw } => Some(draw),
            Self::GeometryBatch { batch } => Some(&batch.draw),
            _ => None,
        }
    }
    /// Selected instance values and their shared execution head, in draw order.
    /// This iterator borrows table backing and never expands legacy descriptors.
    /// Call `GeometryBatch::validate` before consuming untrusted batch payloads.
    pub fn geometry_instances(
        &self,
    ) -> impl Iterator<
        Item = (
            &crate::render_geometry::DrawGeometry,
            &std::sync::Arc<crate::render_geometry::DrawInstance>,
            crate::render_graph::OrderedF32,
        ),
    > {
        let legacy = match self {
            Self::Geometry { draw } => Some((draw.as_ref(), &draw.instance, draw.camera_depth)),
            _ => None,
        };
        let batch = match self {
            Self::GeometryBatch { batch } => Some(batch),
            _ => None,
        };
        legacy
            .into_iter()
            .chain(batch.into_iter().flat_map(|batch| {
                batch.visible.iter().map(|entry| {
                    (
                        batch.draw.as_ref(),
                        &batch.instances[entry.instance_index as usize],
                        entry.camera_depth,
                    )
                })
            }))
    }
    pub fn geometry_instance_count(&self) -> usize {
        match self {
            Self::Geometry { .. } => 1,
            Self::GeometryBatch { batch } => batch.instance_count(),
            _ => 0,
        }
    }
}
impl RhiCommand {
    pub fn geometry_payload(&self) -> Option<&RhiDrawPayload> {
        match self {
            Self::Draw { payload, .. } if payload.geometry_draw().is_some() => Some(payload),
            _ => None,
        }
    }
    pub fn geometry_draw(&self) -> Option<&std::sync::Arc<crate::render_geometry::DrawGeometry>> {
        self.geometry_payload()
            .and_then(RhiDrawPayload::geometry_draw)
    }
    pub fn geometry_instances(
        &self,
    ) -> impl Iterator<
        Item = (
            &crate::render_geometry::DrawGeometry,
            &std::sync::Arc<crate::render_geometry::DrawInstance>,
            crate::render_graph::OrderedF32,
        ),
    > {
        self.geometry_payload()
            .into_iter()
            .flat_map(RhiDrawPayload::geometry_instances)
    }
    pub fn geometry_instance_count(&self) -> usize {
        self.geometry_payload()
            .map_or(0, RhiDrawPayload::geometry_instance_count)
    }
}

pub fn compile_render_graph_to_rhi_plan(graph: &RenderGraph) -> RhiCommandPlan {
    compile_render_graph_with_cache(
        graph,
        &mut crate::render_graph::GraphScheduleCache::default(),
    )
}
pub fn compile_render_graph_with_cache(
    graph: &RenderGraph,
    cache: &mut crate::render_graph::GraphScheduleCache,
) -> RhiCommandPlan {
    let mut diagnostics = graph.validate_declarations_with_cache(cache);
    let schedule = match cache.compile(graph) {
        Ok(plan) => Some(plan),
        Err(errors) => {
            diagnostics.extend(errors);
            None
        }
    };
    let target = graph
        .output_target
        .clone()
        .unwrap_or_else(|| "missing-output-target".to_string());
    let mut commands = vec![RhiCommand::BeginFrame {
        target: target.clone(),
    }];

    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == RenderGraphDiagnosticSeverity::Error)
    {
        return RhiCommandPlan {
            frame_identity: None,
            packet_content_version: None,
            graph_schedule: None,
            frame_index: graph.frame_index,
            graph_id: graph.graph_id.clone(),
            target_kind: "invalid".to_string(),
            commands: commands.into(),
            diagnostics,
        };
    }

    // One immutable target allocation per pass; no global interning or cache.
    // The valid graph already supplies the complete command count. Reserve once
    // before filling the wide RHI enum instead of moving every prior command on growth.
    commands.reserve_exact(
        graph
            .passes
            .iter()
            .map(|pass| pass.commands.len())
            .sum::<usize>()
            .saturating_add(2),
    );
    let mut has_present = false;
    let schedule = schedule.expect("validated graph");
    for &pass_index in &schedule.order {
        let pass = &graph.passes[pass_index];
        match pass.pass_kind {
            RenderPassKind::ResolveColor3d
            | RenderPassKind::BeginView
            | RenderPassKind::DrawGeometry
            | RenderPassKind::Clear
            | RenderPassKind::ParticleSimulation
            | RenderPassKind::DrawParticles
            | RenderPassKind::DrawTestGeometry
            | RenderPassKind::DrawMeshBasic
            | RenderPassKind::DrawSpriteBasic
            | RenderPassKind::DrawSpriteTextured
            | RenderPassKind::DrawUiOverlay
            | RenderPassKind::DrawUiComposition
            | RenderPassKind::Present => {}
        }

        let mut draw_target: Option<std::sync::Arc<str>> = None;
        let mut share_target = |target: &str| {
            if draw_target.as_deref() != Some(target) {
                draw_target = Some(std::sync::Arc::from(target));
            }
            draw_target.as_ref().unwrap().clone()
        };
        for command in &pass.commands {
            match command {
                RenderPassCommand::ResolveColor3d { resolve } => {
                    commands.push(RhiCommand::ResolveColor3d {
                        resolve: resolve.clone(),
                    })
                }
                RenderPassCommand::BeginView {
                    target,
                    view,
                    depth_target,
                } => {
                    commands.push(RhiCommand::BeginView {
                        target: target.clone(),
                        view: view.clone(),
                        depth_target: depth_target.clone(),
                    });
                }
                RenderPassCommand::DrawGeometry { draw } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(&draw.target),
                        draw_kind: RhiDrawKind::Geometry,
                        vertex_count: draw.index_count,
                        payload: RhiDrawPayload::Geometry { draw: draw.clone() },
                    });
                }
                RenderPassCommand::DrawGeometryBatch { batch } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(&batch.draw.target),
                        draw_kind: RhiDrawKind::Geometry,
                        vertex_count: batch.draw.index_count,
                        payload: RhiDrawPayload::GeometryBatch {
                            batch: batch.clone(),
                        },
                    });
                }
                RenderPassCommand::ReconcileParticles { sources, .. } => {
                    commands.push(RhiCommand::ReconcileParticles {
                        sources: sources.clone(),
                    })
                }
                RenderPassCommand::SimulateParticles { instance, step, .. } => {
                    commands.push(RhiCommand::SimulateParticles {
                        instance: *instance,
                        step: step.clone(),
                    })
                }
                RenderPassCommand::DrawParticles {
                    target,
                    instance,
                    emitter,
                    view,
                    texture,
                    mesh,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::Particles,
                        vertex_count: 0,
                        payload: RhiDrawPayload::Particles {
                            instance: *instance,
                            emitter: *emitter,
                            view: view.clone(),
                            texture: *texture,
                            mesh: *mesh,
                        },
                    });
                }
                RenderPassCommand::Clear { target, color } => {
                    commands.push(RhiCommand::Clear {
                        target: target.clone(),
                        color: *color,
                    });
                }
                RenderPassCommand::DrawTestGeometry {
                    target,
                    vertex_count,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::TestGeometry,
                        vertex_count: *vertex_count,
                        payload: RhiDrawPayload::TestGeometry {
                            debug_label: "test-geometry".to_string(),
                        },
                    });
                }
                RenderPassCommand::DrawMeshBasic {
                    target,
                    mesh_ref,
                    material_ref,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::MeshBasic,
                        vertex_count: 3,
                        payload: RhiDrawPayload::MeshBasic {
                            mesh_ref: mesh_ref.clone(),
                            material_ref: material_ref.clone(),
                            pipeline_key: "mesh-basic.default".to_string(),
                        },
                    });
                }
                RenderPassCommand::DrawSpriteBasic {
                    target,
                    sprite_ref,
                    material_ref,
                    sort_key,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::SpriteBasic,
                        vertex_count: 6,
                        payload: RhiDrawPayload::SpriteBasic {
                            sprite_ref: sprite_ref.clone(),
                            material_ref: material_ref.clone(),
                            sort_key: sort_key.clone(),
                            pipeline_key: "sprite-basic.default".to_string(),
                        },
                    });
                }
                RenderPassCommand::DrawSpriteTextured {
                    target,
                    sprite_ref,
                    material_ref,
                    sort_key,
                    texture,
                    binding,
                    fallback_used,
                    vertices,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::SpriteTextured,
                        vertex_count: vertices.len() as u32,
                        payload: RhiDrawPayload::SpriteTextured {
                            sprite_ref: sprite_ref.clone(),
                            material_ref: material_ref.clone(),
                            sort_key: sort_key.clone(),
                            texture: *texture,
                            binding: binding.clone().or_else(|| {
                                texture.map(|handle| RenderBindingSet {
                                    binding_id: format!("binding:sprite:{sprite_ref}"),
                                    binding_kind:
                                        crate::render_asset_production::RenderBindingKind::Texture,
                                    slots: vec![crate::render_contract::NamedRenderBinding::new(
                                        "baseColorTexture",
                                        handle,
                                    )],
                                    sampler: "linearClamp".to_string(),
                                    fallback_used: *fallback_used,
                                    debug_label: "sprite textured binding".to_string(),
                                })
                            }),
                            fallback_used: *fallback_used,
                            pipeline_key: "sprite-basic.default".to_string(),
                            vertices: vertices.clone(),
                        },
                    });
                }
                RenderPassCommand::DrawUiOverlay {
                    target,
                    item_count,
                    text_count,
                    image_count,
                    glyph_count,
                    font_atlas_id,
                    text_pass_inserted,
                    debug_label,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::UiOverlay,
                        vertex_count: if *glyph_count > 0 {
                            (*glyph_count as u32) * 6
                        } else {
                            (*item_count as u32).max(1)
                        },
                        payload: RhiDrawPayload::UiOverlay {
                            item_count: *item_count,
                            text_count: *text_count,
                            image_count: *image_count,
                            glyph_count: *glyph_count,
                            font_atlas_id: font_atlas_id.clone(),
                            text_pass_inserted: *text_pass_inserted,
                            debug_label: debug_label.clone(),
                            pipeline_key: "ui-overlay.default".to_string(),
                        },
                    });
                }
                RenderPassCommand::DrawUiRetained { target, draws, .. } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::UiComposition,
                        vertex_count: draws.iter().map(|d| d.geometry.indices.len() as u32).sum(),
                        payload: RhiDrawPayload::UiRetained {
                            draws: draws.clone(),
                        },
                    });
                }
                RenderPassCommand::DrawUiComposition {
                    target,
                    stage,
                    item_count,
                    text_count,
                    image_count,
                    glyph_count,
                    font_atlas_id,
                    text_pass_inserted,
                    debug_label,
                    texture,
                    font_render_mode,
                    font_page_index,
                    vertices,
                } => {
                    commands.push(RhiCommand::Draw {
                        target: share_target(target),
                        draw_kind: RhiDrawKind::UiComposition,
                        vertex_count: if !vertices.is_empty() {
                            vertices.len() as u32
                        } else if *glyph_count > 0 {
                            (*glyph_count as u32) * 6
                        } else {
                            (*item_count as u32).max(1)
                        },
                        payload: RhiDrawPayload::UiComposition {
                            stage: stage.clone(),
                            item_count: *item_count,
                            text_count: *text_count,
                            image_count: *image_count,
                            glyph_count: *glyph_count,
                            font_atlas_id: font_atlas_id.clone(),
                            text_pass_inserted: *text_pass_inserted,
                            debug_label: debug_label.clone(),
                            pipeline_key: format!("ui-composition.{}", stage.to_lowercase()),
                            texture: *texture,
                            font_render_mode: *font_render_mode,
                            font_page_index: *font_page_index,
                            vertices: vertices.clone(),
                        },
                    });
                }
                RenderPassCommand::Present { target } => {
                    has_present = true;
                    commands.push(RhiCommand::Present {
                        target: target.clone(),
                    });
                }
            }
        }
    }

    commands.push(RhiCommand::Submit);
    if !has_present {
        diagnostics.push(RenderGraphDiagnostic::info(
            "present_inserted",
            "compiler inserted present for graph output target",
        ));
        commands.push(RhiCommand::Present {
            target: target.clone(),
        });
    }

    RhiCommandPlan {
        frame_identity: None,
        packet_content_version: None,
        graph_schedule: Some(schedule),
        frame_index: graph.frame_index,
        graph_id: graph.graph_id.clone(),
        target_kind: "headlessTexture".to_string(),
        commands: commands.into(),
        diagnostics,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::render_graph::{
        color, RenderGraph, RenderPass, RenderPassCommand, RenderPassKind, RenderResource,
    };

    #[test]
    fn command_layout_does_not_inline_large_view_into_every_draw() {
        for (owner, bytes) in [
            ("graph", std::mem::size_of::<RenderPassCommand>()),
            ("rhi", std::mem::size_of::<RhiCommand>()),
        ] {
            eprintln!("{owner} command: {bytes} bytes");
            assert!(
                bytes <= 384,
                "{owner} command uses {bytes} bytes per draw; view metadata dominates the stream"
            );
        }
    }

    #[test]
    fn retained_command_lists_share_backing_and_detach_mutation() {
        let graph = retained_geometry_graph(128);
        let plan = compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors());
        let mut graph_copy = graph.clone();
        let at = graph
            .passes
            .iter()
            .position(|p| p.commands.len() >= 128)
            .unwrap();
        assert_eq!(
            graph.passes[at].commands.as_ptr(),
            graph_copy.passes[at].commands.as_ptr(),
            "immutable graph clone copied the full command backing"
        );
        let mut copy = plan.clone();
        assert_eq!(
            plan.commands.as_ptr(),
            copy.commands.as_ptr(),
            "immutable RHI clone copied the full command backing"
        );
        copy.frame_index += 1;
        assert_eq!(
            plan.commands.as_ptr(),
            copy.commands.as_ptr(),
            "root tags must not detach commands"
        );
        let wire = serde_json::to_vec(&plan).unwrap();
        let graph_wire = serde_json::to_vec(&graph).unwrap();
        graph_copy.passes[at].commands.pop();
        copy.commands.pop();
        assert_ne!(plan.commands.as_ptr(), copy.commands.as_ptr());
        assert_ne!(
            graph.passes[at].commands.as_ptr(),
            graph_copy.passes[at].commands.as_ptr()
        );
        assert_eq!(serde_json::to_vec(&plan).unwrap(), wire);
        assert_eq!(serde_json::to_vec(&graph).unwrap(), graph_wire);
        let roundtrip: RhiCommandPlan = serde_json::from_slice(&wire).unwrap();
        assert_eq!(roundtrip, plan);
        assert_eq!(
            serde_json::to_value(&plan.commands).unwrap(),
            serde_json::to_value(plan.commands.as_slice()).unwrap()
        );
    }

    #[test]
    fn shared_begin_view_keeps_wire_values_and_detaches_changed_packets() {
        use std::sync::Arc;
        let graph = retained_geometry_graph(4);
        let plan = compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors());
        let original = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| {
                if let RenderPassCommand::BeginView { view, .. } = c {
                    Some(view)
                } else {
                    None
                }
            })
            .unwrap();
        let command = plan
            .commands
            .iter()
            .find(|c| matches!(c, RhiCommand::BeginView { .. }))
            .unwrap();
        let RhiCommand::BeginView { view, .. } = command else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(original, view));
        let wire = serde_json::to_value(command).unwrap();
        assert_eq!(wire["view"], serde_json::to_value(view.as_ref()).unwrap());
        let restored: RhiCommand = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(&restored, command);
        let mut next = plan.clone();
        let next_view = next
            .commands
            .iter_mut()
            .find_map(|c| {
                if let RhiCommand::BeginView { view, .. } = c {
                    Some(view)
                } else {
                    None
                }
            })
            .unwrap();
        Arc::make_mut(next_view).clear_color = Some(crate::render_graph::color([0.3, 0., 0., 1.]));
        assert!(!Arc::ptr_eq(view, next_view));
        assert_ne!(view.clear_color, next_view.clear_color);
        assert_eq!(serde_json::to_value(command).unwrap(), wire);
        // A changed BeginView must still fail attachment equality against the frozen draws.
        let mut changed = graph.clone();
        for c in changed.passes.iter_mut().flat_map(|p| &mut p.commands) {
            if let RenderPassCommand::BeginView { view, .. } = c {
                Arc::make_mut(view).clear_color = next_view.clear_color;
            }
        }
        assert!(compile_render_graph_to_rhi_plan(&changed).has_errors());
        assert!(!compile_render_graph_to_rhi_plan(&graph).has_errors());
    }

    fn graph_with_commands(commands: Vec<RenderPassCommand>) -> RenderGraph {
        let target = "surface-main".to_string();
        let mut graph = RenderGraph::new("graph-1", 1);
        graph.output_target = Some(target.clone());
        graph
            .resources
            .push(RenderResource::surface_backbuffer(target.clone(), 640, 480));
        graph.passes.push(RenderPass {
            pass_id: "pass-main".to_string(),
            pass_name: "Pass Main".to_string(),
            pass_kind: RenderPassKind::Clear,
            view_id: "view-1".to_string(),
            reads: Vec::new(),
            writes: vec![target.clone()],
            color_targets: vec![target],
            depth_target: None,
            commands: commands.into(),
            debug_source: None,
        });
        graph
    }

    pub(crate) fn retained_geometry_graph(count: usize) -> RenderGraph {
        use crate::render_contract::*;
        use crate::render_geometry::*;
        use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
        use std::sync::Arc;
        let mut view = RenderView::perspective(1, 640, 480, 60., 0.1, 10.)
            .evaluate(CameraPose::default())
            .unwrap();
        view.depth_policy = DepthPolicy::Disabled;
        let binding = |kind, raw_kind| {
            Arc::new(
                RenderBindingSet::single(
                    "owner".into(),
                    kind,
                    RenderResourceHandle {
                        kind: raw_kind,
                        index: 1,
                        generation: 1,
                    },
                    "nearestClamp".into(),
                    false,
                    "owner".into(),
                )
                .unwrap(),
            )
        };
        let geometry: GeometryBuffers = serde_json::from_value(serde_json::json!({
            "vertexBytes": vec![0u8; 36], "indexBytes": [0,0,1,0,2,0],
            "vertexStride": 12, "vertexCount": 3, "indexCount": 3, "indexFormat": "uint16",
            "attributes": [{"semantic":"position","format":"float32x3","offset":0}]
        }))
        .unwrap();
        let origin = Arc::new(Provenance::unspecified(ProducerStage::RenderProjection));
        let template = crate::render_geometry::data::DrawGeometryWire {
            deformation: None,
            decal_bias: None,
            environment: None,
            resource_lease: Arc::new(()),
            view: Arc::new(view.clone()),
            target: "surface-main".into(),
            depth_target: None,
            geometry: Arc::new(geometry),
            geometry_binding: binding(RenderBindingKind::Mesh, RenderResourceKind::MeshBuffer),
            material_binding: binding(
                RenderBindingKind::Material,
                RenderResourceKind::MaterialParams,
            ),
            geometry_source: origin.clone(),
            material_source: origin.clone(),
            provenance: origin,
            first_index: 0,
            index_count: 3,
            world_transform: std::array::from_fn(|i| {
                crate::render_graph::OrderedF32::from(if i % 5 == 0 { 1. } else { 0. })
            }),
            base_color: color([1.; 4]),
            alpha_cutoff: None,
            standard_lit: None,
            lighting: None,
            shadows: None,
            opaque_alpha: false,
            ignore_vertex_color: false,
            blend: MaterialBlend::Opaque,
            cull: MaterialCull::None,
            depth: MaterialDepth::Disabled,
            camera_depth: 2f32.into(),
            instance_id: 0,
        };
        let template = DrawGeometry::from(template);
        let frame = GeometryFrame {
            target: "surface-main".into(),
            extent: [640, 480],
            views: vec![GeometryViewFrame {
                coordinate_origin: [0.; 3],
                color_target: None,
                view,
                depth_target: None,
                draws: (0..count)
                    .map(|id| {
                        let mut draw = template.clone();
                        draw.instance_id = id as u64;
                        Arc::new(draw)
                    })
                    .collect(),
                batches: None,
                summary: None,
                trace: Vec::new(),
            }],
        };
        let mut graph = graph_with_commands(vec![RenderPassCommand::Clear {
            target: "surface-main".into(),
            color: color([0., 0., 0., 1.]),
        }]);
        frame.append_to_graph(&mut graph);
        graph
    }

    #[test]
    fn retained_draw_success_proofs_reuse_and_changed_inputs_revalidate() {
        let graph = retained_geometry_graph(128);
        let mut cache = crate::render_graph::GraphScheduleCache::default();
        assert!(!compile_render_graph_with_cache(&graph, &mut cache).has_errors());
        assert_eq!(cache.geometry_validation_visits, 128);
        assert!(!compile_render_graph_with_cache(&graph, &mut cache).has_errors());
        assert_eq!(
            cache.geometry_validation_visits, 128,
            "unchanged immutable draws revalidated per frame"
        );
        let mut changed = graph.clone();
        let draw_pass = changed
            .passes
            .iter_mut()
            .find(|p| p.pass_kind == RenderPassKind::DrawGeometry)
            .unwrap();
        let RenderPassCommand::DrawGeometry { draw } = &mut draw_pass.commands[64] else {
            panic!()
        };
        std::sync::Arc::make_mut(draw).index_count = 6;
        for expected in [129, 130] {
            assert!(compile_render_graph_with_cache(&changed, &mut cache).has_errors());
            assert_eq!(
                cache.geometry_validation_visits, expected,
                "invalid draw proof reused or all draws rechecked"
            );
        }
        let mut attachments = graph.clone();
        attachments.views.clear();
        let plan = compile_render_graph_with_cache(&attachments, &mut cache);
        assert!(plan.has_errors());
        assert!(
            plan.diagnostics
                .iter()
                .any(|d| d.code == "geometry.view_missing"),
            "cached draws bypassed attachment checks"
        );
        assert_eq!(
            cache.geometry_validation_visits, 131,
            "valid original draw at changed slot not rechecked"
        );
        assert!(graph
            .validate()
            .iter()
            .all(|d| d.severity != RenderGraphDiagnosticSeverity::Error));
        let RenderPassCommand::DrawGeometry { draw } = &graph
            .passes
            .iter()
            .find(|p| p.pass_kind == RenderPassKind::DrawGeometry)
            .unwrap()
            .commands[0]
        else {
            panic!()
        };
        let lease = std::sync::Arc::downgrade(&draw.resource_lease);
        drop(plan);
        drop(attachments);
        drop(changed);
        drop(graph);
        assert!(
            lease.upgrade().is_none(),
            "graph proof extended native resource lifetime"
        );
    }

    #[test]
    fn retained_draw_proof_budget_falls_back_without_rejecting_draws() {
        let graph = retained_geometry_graph(1);
        let mut cache = crate::render_graph::GraphScheduleCache::default();
        assert!(!compile_render_graph_with_cache(&graph, &mut cache).has_errors());
        let mut oversized = graph.clone();
        let pass = oversized
            .passes
            .iter_mut()
            .find(|p| p.pass_kind == RenderPassKind::DrawGeometry)
            .unwrap();
        let repeated = pass.commands[0].clone();
        pass.commands.resize(65_537, repeated);
        let first = compile_render_graph_with_cache(&oversized, &mut cache);
        assert!(
            !first.has_errors(),
            "proof budget must not reject a valid graph"
        );
        assert_eq!(cache.geometry_validation_visits, 65_538);
        let second = compile_render_graph_with_cache(&oversized, &mut cache);
        assert!(!second.has_errors());
        assert_eq!(
            cache.geometry_validation_visits, 131_075,
            "oversized proof was retained"
        );
        assert!(!compile_render_graph_with_cache(&graph, &mut cache).has_errors());
        assert_eq!(
            cache.geometry_validation_visits, 131_076,
            "old proof survived oversized graph"
        );
    }

    #[test]
    fn repeated_draw_targets_share_storage_and_preserve_wire() {
        let graph = graph_with_commands(
            (0..128)
                .map(|_| RenderPassCommand::DrawTestGeometry {
                    target: "surface-main".into(),
                    vertex_count: 3,
                })
                .collect(),
        );
        let plan = compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors());
        let targets: Vec<_> = plan
            .commands
            .iter()
            .filter_map(|c| match c {
                RhiCommand::Draw { target, .. } => Some(target),
                _ => None,
            })
            .collect();
        assert_eq!(targets.len(), 128);
        assert!(
            targets.iter().all(|t| t.as_ptr() == targets[0].as_ptr()),
            "each draw separately allocated the immutable target"
        );
        let wire = serde_json::to_value(&plan).unwrap();
        let roundtrip: RhiCommandPlan = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(roundtrip, plan);
        assert_eq!(wire["commands"][1]["target"], "surface-main");
        let cloned = plan.clone();
        let RhiCommand::Draw {
            target: cloned_target,
            ..
        } = &cloned.commands[1]
        else {
            panic!()
        };
        assert_eq!(
            cloned_target.as_ptr(),
            targets[0].as_ptr(),
            "plan clone copied target"
        );
        let mut changed = graph.clone();
        if let RenderPassCommand::DrawTestGeometry { target, .. } =
            &mut changed.passes[0].commands[64]
        {
            *target = "undeclared-target".into();
        }
        assert!(
            compile_render_graph_to_rhi_plan(&changed).has_errors(),
            "shared targets must not mask changed command declarations"
        );
    }

    #[test]
    fn clear_graph_compiles_to_clear_submit_present() {
        let graph = graph_with_commands(vec![RenderPassCommand::Clear {
            target: "surface-main".to_string(),
            color: color([0.0, 0.0, 0.0, 1.0]),
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(!plan.has_errors());
        assert!(plan
            .commands
            .iter()
            .any(|command| matches!(command, RhiCommand::Clear { .. })));
        assert!(plan
            .commands
            .iter()
            .any(|command| matches!(command, RhiCommand::Present { .. })));
    }

    #[test]
    fn draw_test_geometry_compiles_to_draw() {
        let graph = graph_with_commands(vec![RenderPassCommand::DrawTestGeometry {
            target: "surface-main".to_string(),
            vertex_count: 3,
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::TestGeometry,
                payload: RhiDrawPayload::TestGeometry { .. },
                ..
            }
        )));
    }

    #[test]
    fn draw_mesh_basic_preserves_typed_payload() {
        let graph = graph_with_commands(vec![RenderPassCommand::DrawMeshBasic {
            target: "surface-main".to_string(),
            mesh_ref: "mesh-a".to_string(),
            material_ref: Some("material-a".to_string()),
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::MeshBasic,
                payload: RhiDrawPayload::MeshBasic {
                    mesh_ref,
                    material_ref: Some(material_ref),
                    ..
                },
                ..
            } if mesh_ref == "mesh-a" && material_ref == "material-a"
        )));
    }

    #[test]
    fn draw_sprite_basic_preserves_typed_payload() {
        let graph = graph_with_commands(vec![RenderPassCommand::DrawSpriteBasic {
            target: "surface-main".to_string(),
            sprite_ref: "sprite-a".to_string(),
            material_ref: Some("material-sprite".to_string()),
            sort_key: "sort-1".to_string(),
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::SpriteBasic,
                vertex_count: 6,
                payload: RhiDrawPayload::SpriteBasic {
                    sprite_ref,
                    material_ref: Some(material_ref),
                    sort_key,
                    ..
                },
                ..
            } if sprite_ref == "sprite-a" && material_ref == "material-sprite" && sort_key == "sort-1"
        )));
    }

    #[test]
    fn draw_sprite_textured_preserves_binding_payload() {
        let graph = graph_with_commands(vec![RenderPassCommand::DrawSpriteTextured {
            target: "surface-main".to_string(),
            sprite_ref: "sprite-a".to_string(),
            material_ref: None,
            sort_key: "sort-1".to_string(),
            texture: None,
            binding: None,
            fallback_used: true,
            vertices: vec![RenderDrawVertex::new([0.0, 0.0], [1.0, 1.0, 1.0, 1.0], [0.0, 0.0],); 6],
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::SpriteTextured,
                vertex_count: 6,
                payload: RhiDrawPayload::SpriteTextured {
                    sprite_ref,
                    material_ref: None,
                    sort_key,
                    texture: None,
                    fallback_used: true,
                    ..
                },
                ..
            } if sprite_ref == "sprite-a" && sort_key == "sort-1"
        )));
    }

    #[test]
    fn draw_ui_overlay_compiles_to_ui_overlay_draw() {
        let graph = graph_with_commands(vec![RenderPassCommand::DrawUiOverlay {
            target: "surface-main".to_string(),
            item_count: 2,
            text_count: 1,
            image_count: 0,
            glyph_count: 5,
            font_atlas_id: Some("ui-default-cmin".to_string()),
            text_pass_inserted: true,
            debug_label: "aui-overlay".to_string(),
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::UiOverlay,
                vertex_count: 30,
                payload: RhiDrawPayload::UiOverlay {
                    item_count: 2,
                    text_count: 1,
                    image_count: 0,
                    glyph_count: 5,
                    text_pass_inserted: true,
                    ..
                },
                ..
            }
        )));
    }

    #[test]
    fn draw_ui_composition_preserves_stage_payload() {
        let graph = graph_with_commands(vec![RenderPassCommand::DrawUiComposition {
            target: "surface-main".to_string(),
            stage: "BeforeWorld".to_string(),
            item_count: 2,
            text_count: 1,
            image_count: 0,
            glyph_count: 5,
            font_atlas_id: Some("ui-default-cmin".to_string()),
            text_pass_inserted: true,
            debug_label: "aui-before-world".to_string(),
            texture: None,
            font_render_mode: None,
            font_page_index: None,
            vertices: Default::default(),
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::UiComposition,
                vertex_count: 30,
                payload: RhiDrawPayload::UiComposition {
                    stage,
                    item_count: 2,
                    text_count: 1,
                    glyph_count: 5,
                    text_pass_inserted: true,
                    ..
                },
                ..
            } if stage == "BeforeWorld"
        )));
    }

    #[test]
    fn b18_ui_vertex_stream_keeps_lineage_through_compile_and_flat_wire_semantics() {
        let vertices = vec![
            RenderDrawVertex::new([0.0, 0.0], [1.0; 4], [0.0, 0.0]),
            RenderDrawVertex::new([1.0, 0.0], [1.0; 4], [1.0, 0.0]),
            RenderDrawVertex::new([0.0, 1.0], [1.0; 4], [0.0, 1.0]),
        ];
        let stream = UiVertexStream::from(vertices.clone());
        let graph = graph_with_commands(vec![RenderPassCommand::DrawUiComposition {
            target: "surface-main".to_string(),
            stage: "ScreenOverlay".to_string(),
            item_count: 1,
            text_count: 0,
            image_count: 0,
            glyph_count: 0,
            font_atlas_id: None,
            text_pass_inserted: false,
            debug_label: "b18".to_string(),
            texture: None,
            font_render_mode: None,
            font_page_index: None,
            vertices: stream.clone(),
        }]);

        let plan = compile_render_graph_to_rhi_plan(&graph);
        let RhiDrawPayload::UiComposition {
            vertices: compiled, ..
        } = plan
            .commands
            .iter()
            .find_map(|command| match command {
                RhiCommand::Draw { payload, .. } => Some(payload),
                _ => None,
            })
            .expect("expected draw")
        else {
            panic!("expected UI composition payload");
        };
        assert!(compiled.shares_data_with(&stream));
        assert_eq!(compiled.stream_id(), stream.stream_id());

        let json = serde_json::to_value(&plan).unwrap();
        assert!(json.to_string().contains("\"vertices\":["));
        let decoded: RhiCommandPlan = serde_json::from_value(json).unwrap();
        let decoded_stream = decoded
            .commands
            .iter()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::UiComposition { vertices, .. },
                    ..
                } => Some(vertices),
                _ => None,
            })
            .expect("expected decoded UI draw");
        assert_eq!(decoded_stream.iter().copied().collect::<Vec<_>>(), vertices);
        assert_ne!(decoded_stream.stream_id(), stream.stream_id());
        assert!(decoded_stream.patch_ranges_from(&stream).is_none());

        let mut resized = UiVertexStream::from(vec![vertices[0], vertices[1]]);
        let resized_id = resized.stream_id();
        assert_eq!(resized.continue_from(&stream), (0, 1));
        assert_eq!(resized.stream_id(), resized_id);
        assert_ne!(resized.stream_id(), stream.stream_id());
        assert!(resized.patch_ranges_from(&stream).is_none());

        let mut reshaped = UiVertexStream::from_chunks(vec![
            std::sync::Arc::new(vec![vertices[0]]),
            std::sync::Arc::new(vec![vertices[1], vertices[2]]),
        ]);
        let reshaped_id = reshaped.stream_id();
        assert_eq!(reshaped.continue_from(&stream), (0, 2));
        assert_eq!(reshaped.stream_id(), reshaped_id);
        assert!(reshaped.patch_ranges_from(&stream).is_none());
    }

    #[test]
    fn missing_target_stops_after_begin_frame_with_error() {
        let graph = RenderGraph::new("graph-1", 1);

        let plan = compile_render_graph_to_rhi_plan(&graph);

        assert!(plan.has_errors());
        assert_eq!(plan.commands.len(), 1);
    }

    #[test]
    fn command_plan_is_json_serializable() {
        let graph = graph_with_commands(vec![RenderPassCommand::Clear {
            target: "surface-main".to_string(),
            color: color([0.0, 0.0, 0.0, 1.0]),
        }]);
        let plan = compile_render_graph_to_rhi_plan(&graph);

        let json = serde_json::to_string(&plan).expect("serialize plan");

        assert!(json.contains("commandKind"));
        assert!(json.contains("clear"));
    }
}
