use crate::font_bundle::FontBundleRenderMode;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::render_asset_production::RenderBindingSet;
use crate::render_resource::RenderResourceHandle;
#[cfg_attr(test, path = "render_graph/schedule.rs")]
mod schedule;
pub use schedule::*;
#[cfg_attr(test, path = "render_graph/shared_list.rs")]
mod shared_list;
pub use shared_list::SharedRenderList;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraph {
    #[serde(default, skip_serializing_if = "GraphContracts::is_empty")]
    pub contracts: GraphContracts,
    pub graph_id: String,
    pub frame_index: u64,
    pub views: Vec<RenderGraphView>,
    pub resources: Vec<RenderResource>,
    pub passes: Vec<RenderPass>,
    pub output_target: Option<String>,
    pub diagnostics: Vec<RenderGraphDiagnostic>,
}

#[cfg(test)]
thread_local! {
    static DECLARATION_WRITE_CHECKS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static DECLARATION_RESOURCE_CHECKS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl RenderGraph {
    pub fn new(graph_id: impl Into<String>, frame_index: u64) -> Self {
        Self {
            contracts: GraphContracts::default(),
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
        let mut diagnostics = self.validate_declarations();
        if let Err(errors) = self.compile_schedule() {
            diagnostics.extend(errors);
        }
        diagnostics
    }

    pub(crate) fn validate_declarations(&self) -> Vec<RenderGraphDiagnostic> {
        self.validate_declarations_with_geometry_errors(
            crate::render_geometry::validate_geometry_graph(self),
        )
    }

    pub(crate) fn validate_declarations_with_cache(
        &self,
        cache: &mut GraphScheduleCache,
    ) -> Vec<RenderGraphDiagnostic> {
        let (errors, _visits) = crate::render_geometry::validate_geometry_graph_with_cache(
            self,
            &mut cache.geometry_draw_proofs,
        );
        #[cfg(test)]
        {
            cache.geometry_validation_visits += _visits;
        }
        self.validate_declarations_with_geometry_errors(errors)
    }

    fn validate_declarations_with_geometry_errors(
        &self,
        geometry_errors: Vec<RenderGraphDiagnostic>,
    ) -> Vec<RenderGraphDiagnostic> {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(geometry_errors);
        diagnostics.extend(crate::color_pipeline::validate_graph(self));
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

        let mut referenced = BTreeSet::<&str>::new();
        if let Some(target) = &self.output_target {
            referenced.insert(target.as_str());
        }

        for pass in &self.passes {
            // Only successful declarations are reused, within this immutable pass.
            let mut declared_write = None;
            for command in &pass.commands {
                let target = command.target();
                let writes_target = !matches!(
                    command,
                    RenderPassCommand::Present { .. }
                        | RenderPassCommand::ReconcileParticles { .. }
                        | RenderPassCommand::SimulateParticles { .. }
                );
                if writes_target && declared_write != Some(target) {
                    #[cfg(test)]
                    DECLARATION_WRITE_CHECKS.with(|n| n.set(n.get() + 1));
                    if !pass.writes.iter().any(|v| v == target)
                        && !pass.color_targets.iter().any(|v| v == target)
                    {
                        diagnostics.push(RenderGraphDiagnostic::error(
                            "graph.undeclared_command_write",
                            format!("{}:{target}", pass.pass_id),
                        ));
                    } else {
                        declared_write = Some(target);
                    }
                }
                if let RenderPassCommand::DrawParticles { view, .. } = command {
                    if view.depth_target != pass.depth_target {
                        diagnostics.push(RenderGraphDiagnostic::error(
                            "particle_render.undeclared_depth",
                            &pass.pass_id,
                        ));
                    }
                    if let Err(error) = view.validate() {
                        diagnostics.push(RenderGraphDiagnostic::error(
                            "particle_render.invalid_view",
                            error,
                        ));
                    }
                }
            }
            let mut declared_reference = None;
            for resource_id in pass.resource_refs_iter() {
                if declared_reference == Some(resource_id) {
                    continue;
                }
                #[cfg(test)]
                DECLARATION_RESOURCE_CHECKS.with(|n| n.set(n.get() + 1));
                referenced.insert(resource_id);
                if !resource_ids.contains(resource_id) {
                    diagnostics.push(RenderGraphDiagnostic::error(
                        "missing_referenced_resource",
                        format!(
                            "pass '{}' references missing resource '{}'",
                            pass.pass_id, resource_id
                        ),
                    ));
                } else {
                    declared_reference = Some(resource_id);
                }
            }
        }

        for resource in &self.resources {
            if !referenced.contains(resource.resource_id.as_str()) {
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
    pub commands: SharedRenderList<RenderPassCommand>,
    pub debug_source: Option<String>,
}

impl RenderPass {
    pub fn resource_refs(&self) -> Vec<&str> {
        self.resource_refs_iter().collect()
    }

    fn resource_refs_iter(&self) -> impl Iterator<Item = &str> {
        self.reads
            .iter()
            .chain(&self.writes)
            .chain(&self.color_targets)
            .chain(&self.depth_target)
            .map(String::as_str)
            .chain(self.commands.iter().map(RenderPassCommand::target))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderPassKind {
    ResolveColor3d,
    BeginView,
    DrawGeometry,
    ParticleSimulation,
    DrawParticles,
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

static NEXT_UI_VERTEX_STREAM_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub struct UiVertexRange {
    pub(crate) first_vertex: usize,
    pub(crate) vertex_count: usize,
}

#[derive(Debug, Clone)]
struct UiVertexStreamData {
    chunks: Vec<Arc<Vec<RenderDrawVertex>>>,
    vertex_count: usize,
}

#[derive(Debug, Clone)]
struct UiVertexStreamLineage {
    stream_id: u64,
    revision: u64,
    previous: Option<Arc<UiVertexStreamData>>,
    changed_ranges: Arc<Vec<UiVertexRange>>,
}

/// Ordered immutable UI vertex chunks with process-local upload lineage.
///
/// Serde intentionally preserves the previous flat vertex-array wire shape. A
/// deserialized stream starts a new lineage and therefore cannot request a
/// partial GPU update.
#[derive(Debug, Clone)]
pub struct UiVertexStream {
    data: Arc<UiVertexStreamData>,
    lineage: UiVertexStreamLineage,
}

impl UiVertexStream {
    fn next_id() -> u64 {
        NEXT_UI_VERTEX_STREAM_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn from_chunks(chunks: Vec<Arc<Vec<RenderDrawVertex>>>) -> Self {
        let vertex_count = chunks.iter().map(|chunk| chunk.len()).sum();
        Self {
            data: Arc::new(UiVertexStreamData {
                chunks,
                vertex_count,
            }),
            lineage: UiVertexStreamLineage {
                stream_id: Self::next_id(),
                revision: 1,
                previous: None,
                changed_ranges: Arc::new(Vec::new()),
            },
        }
    }

    pub fn len(&self) -> usize {
        self.data.vertex_count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &RenderDrawVertex> {
        self.data.chunks.iter().flat_map(|chunk| chunk.iter())
    }

    pub(crate) fn allocated_vertex_capacity(&self) -> usize {
        self.data.chunks.iter().map(|chunk| chunk.capacity()).sum()
    }

    #[doc(hidden)]
    pub fn shares_data_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }

    #[cfg(any(feature = "real-wgpu", test))]
    #[doc(hidden)]
    pub fn stream_id(&self) -> u64 {
        self.lineage.stream_id
    }

    pub(crate) fn extend_chunks_from(&mut self, other: &Self) {
        let data = Arc::make_mut(&mut self.data);
        data.vertex_count += other.len();
        data.chunks.extend(other.data.chunks.iter().cloned());
        self.lineage = UiVertexStreamLineage {
            stream_id: Self::next_id(),
            revision: 1,
            previous: None,
            changed_ranges: Arc::new(Vec::new()),
        };
    }

    /// Reuses the previous stable stream when shape permits a byte-range patch.
    /// Returns `(unchanged_chunks, changed_chunks)`.
    pub(crate) fn continue_from(&mut self, previous: &Self) -> (usize, usize) {
        if self.len() != previous.len()
            || self.data.chunks.len() != previous.data.chunks.len()
            || self
                .data
                .chunks
                .iter()
                .zip(&previous.data.chunks)
                .any(|(next, old)| next.len() != old.len())
        {
            return (0, self.data.chunks.len());
        }

        let mut changed_ranges = Vec::<UiVertexRange>::new();
        let mut offset = 0usize;
        let mut unchanged = 0usize;
        let mut changed = 0usize;
        for (next, old) in self.data.chunks.iter().zip(&previous.data.chunks) {
            let differs = !Arc::ptr_eq(next, old) && next.as_slice() != old.as_slice();
            if differs {
                changed += 1;
                if let Some(range) = changed_ranges
                    .last_mut()
                    .filter(|range| range.first_vertex + range.vertex_count == offset)
                {
                    range.vertex_count += next.len();
                } else {
                    changed_ranges.push(UiVertexRange {
                        first_vertex: offset,
                        vertex_count: next.len(),
                    });
                }
            } else {
                unchanged += 1;
            }
            offset += next.len();
        }
        if changed == 0 {
            *self = previous.clone();
            return (unchanged, 0);
        }
        let Some(revision) = previous.lineage.revision.checked_add(1) else {
            return (0, self.data.chunks.len());
        };
        self.lineage = UiVertexStreamLineage {
            stream_id: previous.lineage.stream_id,
            revision,
            previous: Some(previous.data.clone()),
            changed_ranges: Arc::new(changed_ranges),
        };
        debug_assert!(self.patch_ranges_from(previous).is_some());
        (unchanged, changed)
    }

    #[doc(hidden)]
    pub fn patch_ranges_from(&self, previous: &Self) -> Option<&[UiVertexRange]> {
        let expected_revision = previous.lineage.revision.checked_add(1)?;
        let previous_data = self.lineage.previous.as_ref()?;
        let ranges = self.lineage.changed_ranges.as_slice();
        (self.lineage.stream_id == previous.lineage.stream_id
            && self.lineage.revision == expected_revision
            && Arc::ptr_eq(previous_data, &previous.data)
            && self.len() == previous.len()
            && !ranges.is_empty()
            && ranges.iter().all(|range| {
                range.vertex_count > 0
                    && range
                        .first_vertex
                        .checked_add(range.vertex_count)
                        .is_some_and(|end| end <= self.len())
            }))
        .then_some(ranges)
    }

    #[cfg(feature = "real-wgpu")]
    #[doc(hidden)]
    pub fn vertices_in_range(
        &self,
        range: UiVertexRange,
    ) -> impl Iterator<Item = &RenderDrawVertex> {
        self.iter()
            .skip(range.first_vertex)
            .take(range.vertex_count)
    }
}

impl Default for UiVertexStream {
    fn default() -> Self {
        Self::from(Vec::new())
    }
}

impl From<Vec<RenderDrawVertex>> for UiVertexStream {
    fn from(vertices: Vec<RenderDrawVertex>) -> Self {
        Self::from_chunks(vec![Arc::new(vertices)])
    }
}

impl From<Arc<Vec<RenderDrawVertex>>> for UiVertexStream {
    fn from(vertices: Arc<Vec<RenderDrawVertex>>) -> Self {
        Self::from_chunks(vec![vertices])
    }
}

impl PartialEq for UiVertexStream {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && (self.shares_data_with(other) || self.iter().eq(other.iter()))
    }
}

impl Eq for UiVertexStream {}

impl std::ops::Index<usize> for UiVertexStream {
    type Output = RenderDrawVertex;

    fn index(&self, mut index: usize) -> &Self::Output {
        for chunk in &self.data.chunks {
            if index < chunk.len() {
                return &chunk[index];
            }
            index -= chunk.len();
        }
        panic!("UI vertex index out of bounds");
    }
}

impl Hash for UiVertexStream {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for vertex in self.iter() {
            vertex.hash(state);
        }
    }
}

impl Serialize for UiVertexStream {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.len()))?;
        for vertex in self.iter() {
            sequence.serialize_element(vertex)?;
        }
        sequence.end()
    }
}

impl<'de> Deserialize<'de> for UiVertexStream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<RenderDrawVertex>::deserialize(deserializer).map(Self::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "commandKind")]
pub enum RenderPassCommand {
    ResolveColor3d {
        resolve: crate::color_pipeline::ColorResolve3D,
    },
    BeginView {
        target: String,
        view: std::sync::Arc<crate::render_contract::ExecutableRenderView>,
        depth_target: Option<String>,
    },
    DrawGeometryBatch {
        batch: Arc<crate::render_geometry::GeometryBatch>,
    },
    DrawGeometry {
        draw: Arc<crate::render_geometry::DrawGeometry>,
    },
    ReconcileParticles {
        target: String,
        sources: Vec<crate::particle_render_contract::ParticleSourceFrame>,
    },
    SimulateParticles {
        target: String,
        instance: u64,
        step: crate::particle_render_contract::ParticleRenderStep,
    },
    DrawParticles {
        target: String,
        instance: u64,
        emitter: u32,
        view: crate::particle_render_contract::ParticleRenderView,
        texture: Option<RenderResourceHandle>,
        mesh: Option<RenderResourceHandle>,
    },
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
    DrawUiRetained {
        target: String,
        stage: String,
        item_count: usize,
        draws: Vec<crate::aui_retained::UiRetainedDraw>,
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
        vertices: UiVertexStream,
    },
    Present {
        target: String,
    },
}

impl RenderPassCommand {
    pub(crate) fn geometry_draw(&self) -> Option<&Arc<crate::render_geometry::DrawGeometry>> {
        match self {
            Self::DrawGeometry { draw } => Some(draw),
            Self::DrawGeometryBatch { batch } => Some(&batch.draw),
            _ => None,
        }
    }
    pub fn target(&self) -> &str {
        match self {
            Self::ResolveColor3d { resolve } => &resolve.target,
            Self::DrawGeometry { draw } => &draw.target,
            Self::DrawGeometryBatch { batch } => &batch.draw.target,
            Self::BeginView { target, .. } => target,
            Self::Clear { target, .. }
            | Self::ReconcileParticles { target, .. }
            | Self::SimulateParticles { target, .. }
            | Self::DrawParticles { target, .. }
            | Self::DrawTestGeometry { target, .. }
            | Self::DrawMeshBasic { target, .. }
            | Self::DrawSpriteBasic { target, .. }
            | Self::DrawSpriteTextured { target, .. }
            | Self::DrawUiOverlay { target, .. }
            | Self::DrawUiComposition { target, .. }
            | Self::DrawUiRetained { target, .. }
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
    Rgba16Float,
    Depth32Float,
    Rgba8Unorm,
    Bgra8Unorm,
    Rgba8UnormSrgb,
    Bgra8UnormSrgb,
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
            }]
            .into(),
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
    fn repeated_valid_command_target_uses_one_pass_local_declaration_check() {
        let mut graph = clear_graph();
        let command = graph.passes[0].commands[0].clone();
        graph.passes[0].commands.resize(128, command);
        DECLARATION_WRITE_CHECKS.with(|n| n.set(0));
        DECLARATION_RESOURCE_CHECKS.with(|n| n.set(0));
        assert!(graph.validate_declarations().is_empty());
        assert_eq!(
            DECLARATION_WRITE_CHECKS.with(|n| n.get()),
            1,
            "identical successful write declaration was checked for each draw"
        );
        assert_eq!(
            DECLARATION_RESOURCE_CHECKS.with(|n| n.get()),
            1,
            "identical successful resource declaration was checked for every reference"
        );
        graph.passes[0].commands[64] = RenderPassCommand::Clear {
            target: "missing-target".into(),
            color: color([0.; 4]),
        };
        let errors = graph.validate_declarations();
        assert_eq!(
            errors
                .iter()
                .filter(|d| d.code == "graph.undeclared_command_write")
                .count(),
            1
        );
        assert_eq!(
            errors
                .iter()
                .filter(|d| d.code == "missing_referenced_resource")
                .count(),
            1
        );
        // Failed declarations must retain all per-command diagnostics and their original ordering.
        graph.passes[0].commands.fill(RenderPassCommand::Clear {
            target: "missing-target".into(),
            color: color([0.; 4]),
        });
        let errors = graph.validate_declarations();
        assert_eq!(errors.len(), 256);
        assert!(errors[..128]
            .iter()
            .all(|d| d.code == "graph.undeclared_command_write"));
        assert!(errors[128..]
            .iter()
            .all(|d| d.code == "missing_referenced_resource"));
        graph.passes[0].writes.clear();
        graph.passes[0].color_targets.clear();
        graph.passes[0].commands.fill(RenderPassCommand::Clear {
            target: "surface-main".into(),
            color: color([0.; 4]),
        });
        assert_eq!(
            graph
                .validate_declarations()
                .iter()
                .filter(|d| d.code == "graph.undeclared_command_write")
                .count(),
            128,
            "successful target memo crossed a changed declaration"
        );
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
            vertices: Default::default(),
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

impl UiVertexRange {
    #[doc(hidden)]
    pub fn first_vertex(self) -> usize {
        self.first_vertex
    }
    #[doc(hidden)]
    pub fn vertex_count(self) -> usize {
        self.vertex_count
    }
}
