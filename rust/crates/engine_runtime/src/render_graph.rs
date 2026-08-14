use crate::font_bundle::FontBundleRenderMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::render_asset_production::RenderBindingSet;
use crate::render_resource::RenderResourceHandle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraph {
    pub graph_id: String,
    pub frame_index: u64,
    pub views: Vec<RenderGraphView>,
    pub resources: Vec<RenderResource>,
    pub passes: Vec<RenderPass>,
    pub output_target: Option<String>,
    pub diagnostics: Vec<RenderGraphDiagnostic>,
}

impl RenderGraph {
    pub fn new(graph_id: impl Into<String>, frame_index: u64) -> Self {
        Self {
            graph_id: graph_id.into(),
            frame_index,
            views: Vec::new(),
            resources: Vec::new(),
            passes: Vec::new(),
            output_target: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn validate(&self) -> Vec<RenderGraphDiagnostic> {
        let mut diagnostics = self.diagnostics.clone();
        let resource_ids = self
            .resources
            .iter()
            .map(|resource| resource.resource_id.as_str())
            .collect::<BTreeSet<_>>();

        match self.output_target.as_deref() {
            Some(target) if !resource_ids.contains(target) => {
                diagnostics.push(RenderGraphDiagnostic::error(
                    "missing_output_target_resource",
                    format!("output target '{target}' is not declared as a render resource"),
                ));
            }
            Some(_) => {}
            None => diagnostics.push(RenderGraphDiagnostic::error(
                "missing_output_target",
                "render graph must declare an output target",
            )),
        }

        let mut referenced = BTreeSet::<String>::new();
        if let Some(target) = &self.output_target {
            referenced.insert(target.clone());
        }

        for pass in &self.passes {
            for resource_id in pass.resource_refs() {
                referenced.insert(resource_id.clone());
                if !resource_ids.contains(resource_id.as_str()) {
                    diagnostics.push(RenderGraphDiagnostic::error(
                        "missing_referenced_resource",
                        format!(
                            "pass '{}' references missing resource '{}'",
                            pass.pass_id, resource_id
                        ),
                    ));
                }
            }
        }

        for resource in &self.resources {
            if !referenced.contains(&resource.resource_id) {
                diagnostics.push(RenderGraphDiagnostic::warning(
                    "unused_resource",
                    format!(
                        "resource '{}' is declared but not used",
                        resource.resource_id
                    ),
                ));
            }
        }

        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraphView {
    pub view_id: String,
    pub view_kind: String,
    pub width: u32,
    pub height: u32,
    pub clear_color: [OrderedF32; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPass {
    pub pass_id: String,
    pub pass_name: String,
    pub pass_kind: RenderPassKind,
    pub view_id: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub color_targets: Vec<String>,
    pub depth_target: Option<String>,
    pub commands: Vec<RenderPassCommand>,
    pub debug_source: Option<String>,
}

impl RenderPass {
    pub fn resource_refs(&self) -> Vec<&String> {
        let mut refs = Vec::new();
        refs.extend(self.reads.iter());
        refs.extend(self.writes.iter());
        refs.extend(self.color_targets.iter());
        if let Some(depth_target) = &self.depth_target {
            refs.push(depth_target);
        }
        for command in &self.commands {
            refs.push(command.target());
        }
        refs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderPassKind {
    Clear,
    DrawTestGeometry,
    DrawMeshBasic,
    DrawSpriteBasic,
    DrawSpriteTextured,
    DrawUiOverlay,
    DrawUiComposition,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderDrawVertex {
    pub position: [OrderedF32; 2],
    pub color: [OrderedF32; 4],
    pub uv: [OrderedF32; 2],
}

impl RenderDrawVertex {
    pub fn new(position: [f32; 2], color: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            position: [position[0].into(), position[1].into()],
            color: crate::render_graph::color(color),
            uv: [uv[0].into(), uv[1].into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "commandKind")]
pub enum RenderPassCommand {
    Clear {
        target: String,
        color: [OrderedF32; 4],
    },
    DrawTestGeometry {
        target: String,
        vertex_count: u32,
    },
    DrawMeshBasic {
        target: String,
        mesh_ref: String,
        material_ref: Option<String>,
    },
    DrawSpriteBasic {
        target: String,
        sprite_ref: String,
        material_ref: Option<String>,
        sort_key: String,
    },
    DrawSpriteTextured {
        target: String,
        sprite_ref: String,
        material_ref: Option<String>,
        sort_key: String,
        texture: Option<RenderResourceHandle>,
        binding: Option<RenderBindingSet>,
        fallback_used: bool,
        vertices: Vec<RenderDrawVertex>,
    },
    DrawUiOverlay {
        target: String,
        item_count: usize,
        text_count: usize,
        image_count: usize,
        glyph_count: usize,
        font_atlas_id: Option<String>,
        text_pass_inserted: bool,
        debug_label: String,
    },
    DrawUiComposition {
        target: String,
        stage: String,
        item_count: usize,
        text_count: usize,
        image_count: usize,
        glyph_count: usize,
        font_atlas_id: Option<String>,
        text_pass_inserted: bool,
        debug_label: String,
        texture: Option<RenderResourceHandle>,
        font_render_mode: Option<FontBundleRenderMode>,
        font_page_index: Option<u32>,
        vertices: Vec<RenderDrawVertex>,
    },
    Present {
        target: String,
    },
}

impl RenderPassCommand {
    pub fn target(&self) -> &String {
        match self {
            Self::Clear { target, .. }
            | Self::DrawTestGeometry { target, .. }
            | Self::DrawMeshBasic { target, .. }
            | Self::DrawSpriteBasic { target, .. }
            | Self::DrawSpriteTextured { target, .. }
            | Self::DrawUiOverlay { target, .. }
            | Self::DrawUiComposition { target, .. }
            | Self::Present { target } => target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResource {
    pub resource_id: String,
    pub resource_name: String,
    pub resource_kind: RenderResourceKind,
    pub format: RenderResourceFormat,
    pub size: RenderResourceSize,
    pub usage: Vec<RenderResourceUsage>,
    pub lifetime: RenderResourceLifetime,
}

impl RenderResource {
    pub fn surface_backbuffer(resource_id: impl Into<String>, width: u32, height: u32) -> Self {
        let resource_id = resource_id.into();
        Self {
            resource_name: resource_id.clone(),
            resource_id,
            resource_kind: RenderResourceKind::SurfaceBackbuffer,
            format: RenderResourceFormat::Rgba8Unorm,
            size: RenderResourceSize { width, height },
            usage: vec![
                RenderResourceUsage::ColorTarget,
                RenderResourceUsage::Present,
            ],
            lifetime: RenderResourceLifetime::External,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceKind {
    Texture,
    Buffer,
    SurfaceBackbuffer,
    ExternalTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceFormat {
    Rgba8Unorm,
    Bgra8Unorm,
    Depth24Plus,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceUsage {
    ColorTarget,
    DepthTarget,
    Sampled,
    Vertex,
    Index,
    Uniform,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceLifetime {
    FrameLocal,
    External,
    Persistent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraphDiagnostic {
    pub severity: RenderGraphDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl RenderGraphDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RenderGraphDiagnosticSeverity::Info,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RenderGraphDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RenderGraphDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderGraphDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderedF32(u32);

impl OrderedF32 {
    pub fn from_f32(value: f32) -> Self {
        Self(value.to_bits())
    }

    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.0)
    }
}

impl From<f32> for OrderedF32 {
    fn from(value: f32) -> Self {
        Self::from_f32(value)
    }
}

pub fn color(values: [f32; 4]) -> [OrderedF32; 4] {
    [
        values[0].into(),
        values[1].into(),
        values[2].into(),
        values[3].into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_graph() -> RenderGraph {
        let target = "surface-main".to_string();
        let mut graph = RenderGraph::new("graph-1", 1);
        graph.output_target = Some(target.clone());
        graph
            .resources
            .push(RenderResource::surface_backbuffer(target.clone(), 640, 480));
        graph.passes.push(RenderPass {
            pass_id: "clear-main".to_string(),
            pass_name: "Clear Main".to_string(),
            pass_kind: RenderPassKind::Clear,
            view_id: "view-1".to_string(),
            reads: Vec::new(),
            writes: vec![target.clone()],
            color_targets: vec![target.clone()],
            depth_target: None,
            commands: vec![RenderPassCommand::Clear {
                target,
                color: color([0.1, 0.2, 0.3, 1.0]),
            }],
            debug_source: None,
        });
        graph
    }

    #[test]
    fn empty_graph_reports_missing_output_target() {
        let graph = RenderGraph::new("graph-1", 1);

        let diagnostics = graph.validate();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_output_target"));
    }

    #[test]
    fn clear_graph_validation_passes_without_errors() {
        let diagnostics = clear_graph().validate();

        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == RenderGraphDiagnosticSeverity::Error));
    }

    #[test]
    fn missing_referenced_resource_reports_error() {
        let mut graph = clear_graph();
        graph.passes[0].reads.push("missing-texture".to_string());

        let diagnostics = graph.validate();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_referenced_resource"));
    }

    #[test]
    fn unused_resource_reports_warning() {
        let mut graph = clear_graph();
        graph.resources.push(RenderResource {
            resource_id: "unused".to_string(),
            resource_name: "Unused".to_string(),
            resource_kind: RenderResourceKind::Texture,
            format: RenderResourceFormat::Rgba8Unorm,
            size: RenderResourceSize {
                width: 16,
                height: 16,
            },
            usage: vec![RenderResourceUsage::Sampled],
            lifetime: RenderResourceLifetime::FrameLocal,
        });

        let diagnostics = graph.validate();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unused_resource"));
    }

    #[test]
    fn resource_fields_are_json_serializable() {
        let json = serde_json::to_string(&clear_graph().resources[0]).expect("serialize resource");

        assert!(json.contains("resourceId"));
        assert!(json.contains("surfaceBackbuffer"));
    }

    #[test]
    fn render_graph_ui_overlay_command_references_target() {
        let target = "surface-main".to_string();
        let command = RenderPassCommand::DrawUiOverlay {
            target: target.clone(),
            item_count: 3,
            text_count: 1,
            image_count: 1,
            glyph_count: 5,
            font_atlas_id: Some("ui-default-cmin".to_string()),
            text_pass_inserted: true,
            debug_label: "aui-overlay".to_string(),
        };

        assert_eq!(command.target(), &target);
    }

    #[test]
    fn render_graph_ui_composition_command_references_target() {
        let target = "surface-main".to_string();
        let command = RenderPassCommand::DrawUiComposition {
            target: target.clone(),
            stage: "BeforeWorld".to_string(),
            item_count: 3,
            text_count: 1,
            image_count: 1,
            glyph_count: 5,
            font_atlas_id: Some("ui-default-cmin".to_string()),
            text_pass_inserted: true,
            debug_label: "aui-before-world".to_string(),
            texture: None,
            font_render_mode: None,
            font_page_index: None,
            vertices: Vec::new(),
        };

        assert_eq!(command.target(), &target);
    }

    #[test]
    fn render_graph_textured_sprite_command_references_target() {
        let target = "surface-main".to_string();
        let command = RenderPassCommand::DrawSpriteTextured {
            target: target.clone(),
            sprite_ref: "sprite-a".to_string(),
            material_ref: None,
            sort_key: "sort-1".to_string(),
            texture: None,
            binding: None,
            fallback_used: true,
            vertices: Vec::new(),
        };

        assert_eq!(command.target(), &target);
    }
}
