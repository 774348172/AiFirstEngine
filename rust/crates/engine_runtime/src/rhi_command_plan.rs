use serde::{Deserialize, Serialize};

use crate::font_bundle::FontBundleRenderMode;
use crate::render_asset_production::RenderBindingSet;
use crate::render_graph::{
    RenderDrawVertex, RenderGraph, RenderGraphDiagnostic, RenderGraphDiagnosticSeverity,
    RenderPassCommand, RenderPassKind,
};
use crate::render_resource::RenderResourceHandle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RhiCommandPlan {
    pub frame_index: u64,
    pub graph_id: String,
    pub target_kind: String,
    pub commands: Vec<RhiCommand>,
    pub diagnostics: Vec<RenderGraphDiagnostic>,
}

impl RhiCommandPlan {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == RenderGraphDiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "commandKind")]
pub enum RhiCommand {
    BeginFrame {
        target: String,
    },
    Clear {
        target: String,
        color: [crate::render_graph::OrderedF32; 4],
    },
    Draw {
        target: String,
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
        vertices: Vec<RenderDrawVertex>,
    },
}

pub fn compile_render_graph_to_rhi_plan(graph: &RenderGraph) -> RhiCommandPlan {
    let mut diagnostics = graph.validate();
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
            frame_index: graph.frame_index,
            graph_id: graph.graph_id.clone(),
            target_kind: "invalid".to_string(),
            commands,
            diagnostics,
        };
    }

    let mut has_present = false;
    for pass in &graph.passes {
        match pass.pass_kind {
            RenderPassKind::Clear
            | RenderPassKind::DrawTestGeometry
            | RenderPassKind::DrawMeshBasic
            | RenderPassKind::DrawSpriteBasic
            | RenderPassKind::DrawSpriteTextured
            | RenderPassKind::DrawUiOverlay
            | RenderPassKind::DrawUiComposition
            | RenderPassKind::Present => {}
        }

        for command in &pass.commands {
            match command {
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
                        target: target.clone(),
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
                        target: target.clone(),
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
                        target: target.clone(),
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
                        target: target.clone(),
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
                                    resources: vec![handle],
                                    material_handle: None,
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
                        target: target.clone(),
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
                        target: target.clone(),
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
        frame_index: graph.frame_index,
        graph_id: graph.graph_id.clone(),
        target_kind: "headlessTexture".to_string(),
        commands,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_graph::{
        color, RenderGraph, RenderPass, RenderPassCommand, RenderPassKind, RenderResource,
    };

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
            commands,
            debug_source: None,
        });
        graph
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
            vertices: Vec::new(),
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
