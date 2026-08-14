use crate::application::{NativeEditorApplication, NativeEditorApplicationReport};
use editor_ui_model::EditorUiMode;
use editor_wgpu_renderer::{UiGpuDrawPlan, UiGpuDrawableRectSource, UiGpuPaintBatchKind};
use serde::{Deserialize, Serialize};

pub const EDITOR_VISUAL_REGRESSION_REPORT_SCHEMA_VERSION: &str =
    "editor-visual-regression-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorVisualRegressionStatus {
    Passed,
    Failed,
    BaselineMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorVisualRegressionDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorVisualRegressionDiagnostic {
    pub severity: EditorVisualRegressionDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorVisualRegressionScenario {
    pub scenario_id: String,
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub baseline: Option<EditorVisualRegressionBaseline>,
}

impl EditorVisualRegressionScenario {
    pub fn new(scenario_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            title: title.into(),
            width: 1280.0,
            height: 720.0,
            baseline: None,
        }
    }

    pub fn with_surface_size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_baseline(mut self, baseline: EditorVisualRegressionBaseline) -> Self {
        self.baseline = Some(baseline);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorVisualRegressionBaseline {
    pub scenario_id: String,
    pub surface_width: u32,
    pub surface_height: u32,
    pub structural_hash: String,
    pub draw_command_count: usize,
    pub hit_region_count: usize,
    pub rect_count: usize,
    pub text_command_count: usize,
    pub rendered_glyph_count: usize,
    pub viewport_slot_count: usize,
}

impl EditorVisualRegressionBaseline {
    pub fn from_evidence(evidence: &EditorVisualRegressionEvidence) -> Self {
        Self {
            scenario_id: evidence.scenario_id.clone(),
            surface_width: evidence.surface_width,
            surface_height: evidence.surface_height,
            structural_hash: evidence.structural_hash.clone(),
            draw_command_count: evidence.draw_command_count,
            hit_region_count: evidence.hit_region_count,
            rect_count: evidence.rect_count,
            text_command_count: evidence.text_command_count,
            rendered_glyph_count: evidence.rendered_glyph_count,
            viewport_slot_count: evidence.viewport_slot_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorVisualRegressionEvidence {
    pub scenario_id: String,
    pub surface_width: u32,
    pub surface_height: u32,
    pub mode: EditorUiMode,
    pub frame_index: u64,
    pub model_revision: u64,
    pub draw_command_count: usize,
    pub hit_region_count: usize,
    pub rect_count: usize,
    pub text_command_count: usize,
    pub rendered_glyph_count: usize,
    pub viewport_slot_count: usize,
    pub font_backend: String,
    pub font_loaded: bool,
    pub structural_hash: String,
    pub artifact_path: Option<String>,
    pub png_hash: Option<String>,
    pub real_window_screenshot_hash: Option<String>,
}

impl EditorVisualRegressionEvidence {
    pub fn from_app_report_and_draw_plan(
        scenario_id: impl Into<String>,
        app_report: &NativeEditorApplicationReport,
        draw_plan: &UiGpuDrawPlan,
    ) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            surface_width: draw_plan.surface_width,
            surface_height: draw_plan.surface_height,
            mode: app_report.mode.clone(),
            frame_index: app_report.frame_index,
            model_revision: app_report.model_revision,
            draw_command_count: draw_plan.draw_command_count,
            hit_region_count: draw_plan.hit_region_count,
            rect_count: draw_plan.rect_count,
            text_command_count: draw_plan.text_command_count,
            rendered_glyph_count: draw_plan.rendered_glyph_count,
            viewport_slot_count: draw_plan.viewport_slot_count,
            font_backend: draw_plan.font_backend.clone(),
            font_loaded: draw_plan.font_loaded,
            structural_hash: structural_hash_for_draw_plan(draw_plan),
            artifact_path: None,
            png_hash: None,
            real_window_screenshot_hash: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorVisualRegressionReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub title: String,
    pub status: EditorVisualRegressionStatus,
    pub backend: String,
    pub evidence: Option<EditorVisualRegressionEvidence>,
    pub baseline: Option<EditorVisualRegressionBaseline>,
    pub diagnostics: Vec<EditorVisualRegressionDiagnostic>,
}

pub struct EditorVisualRegressionRunner {
    backend: String,
}

impl Default for EditorVisualRegressionRunner {
    fn default() -> Self {
        Self::headless()
    }
}

impl EditorVisualRegressionRunner {
    pub fn headless() -> Self {
        Self {
            backend: "headless-deterministic-visual".to_string(),
        }
    }

    pub fn run(
        &self,
        app: &mut NativeEditorApplication,
        scenario: EditorVisualRegressionScenario,
    ) -> EditorVisualRegressionReport {
        let app_report = app.frame(scenario.width, scenario.height);
        let draw_plan = match UiGpuDrawPlan::from_draw_list(app.latest_draw_list()) {
            Ok(draw_plan) => draw_plan,
            Err(error) => {
                return EditorVisualRegressionReport {
                    schema_version: EDITOR_VISUAL_REGRESSION_REPORT_SCHEMA_VERSION.to_string(),
                    scenario_id: scenario.scenario_id,
                    title: scenario.title,
                    status: EditorVisualRegressionStatus::Failed,
                    backend: self.backend.clone(),
                    evidence: None,
                    baseline: scenario.baseline,
                    diagnostics: vec![diagnostic(
                        EditorVisualRegressionDiagnosticSeverity::Error,
                        "visual_regression.draw_plan_failed",
                        error,
                        None,
                    )],
                };
            }
        };
        let evidence = EditorVisualRegressionEvidence::from_app_report_and_draw_plan(
            scenario.scenario_id.clone(),
            &app_report,
            &draw_plan,
        );
        let (status, diagnostics) = compare_evidence_to_baseline(&evidence, &scenario.baseline);

        EditorVisualRegressionReport {
            schema_version: EDITOR_VISUAL_REGRESSION_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: scenario.scenario_id,
            title: scenario.title,
            status,
            backend: self.backend.clone(),
            evidence: Some(evidence),
            baseline: scenario.baseline,
            diagnostics,
        }
    }
}

pub fn compare_evidence_to_baseline(
    evidence: &EditorVisualRegressionEvidence,
    baseline: &Option<EditorVisualRegressionBaseline>,
) -> (
    EditorVisualRegressionStatus,
    Vec<EditorVisualRegressionDiagnostic>,
) {
    let Some(baseline) = baseline else {
        return (
            EditorVisualRegressionStatus::BaselineMissing,
            vec![diagnostic(
                EditorVisualRegressionDiagnosticSeverity::Warning,
                "visual_regression.baseline_missing",
                "No golden baseline was provided for this scenario.",
                None,
            )],
        );
    };

    let mut diagnostics = Vec::new();
    compare_field(
        &mut diagnostics,
        "scenario_id",
        &baseline.scenario_id,
        &evidence.scenario_id,
    );
    compare_field(
        &mut diagnostics,
        "surface_width",
        baseline.surface_width,
        evidence.surface_width,
    );
    compare_field(
        &mut diagnostics,
        "surface_height",
        baseline.surface_height,
        evidence.surface_height,
    );
    compare_field(
        &mut diagnostics,
        "structural_hash",
        &baseline.structural_hash,
        &evidence.structural_hash,
    );
    compare_field(
        &mut diagnostics,
        "draw_command_count",
        baseline.draw_command_count,
        evidence.draw_command_count,
    );
    compare_field(
        &mut diagnostics,
        "hit_region_count",
        baseline.hit_region_count,
        evidence.hit_region_count,
    );
    compare_field(
        &mut diagnostics,
        "rect_count",
        baseline.rect_count,
        evidence.rect_count,
    );
    compare_field(
        &mut diagnostics,
        "text_command_count",
        baseline.text_command_count,
        evidence.text_command_count,
    );
    compare_field(
        &mut diagnostics,
        "rendered_glyph_count",
        baseline.rendered_glyph_count,
        evidence.rendered_glyph_count,
    );
    compare_field(
        &mut diagnostics,
        "viewport_slot_count",
        baseline.viewport_slot_count,
        evidence.viewport_slot_count,
    );

    if diagnostics.is_empty() {
        (EditorVisualRegressionStatus::Passed, diagnostics)
    } else {
        (EditorVisualRegressionStatus::Failed, diagnostics)
    }
}

fn compare_field<T: std::fmt::Debug + PartialEq>(
    diagnostics: &mut Vec<EditorVisualRegressionDiagnostic>,
    field: &str,
    expected: T,
    actual: T,
) {
    if expected != actual {
        diagnostics.push(diagnostic(
            EditorVisualRegressionDiagnosticSeverity::Error,
            "visual_regression.baseline_mismatch",
            format!(
                "Field `{field}` expected `{:?}`, got `{:?}`.",
                expected, actual
            ),
            Some(field.to_string()),
        ));
    }
}

fn diagnostic(
    severity: EditorVisualRegressionDiagnosticSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
    field: Option<String>,
) -> EditorVisualRegressionDiagnostic {
    EditorVisualRegressionDiagnostic {
        severity,
        code: code.into(),
        message: message.into(),
        field,
    }
}

fn structural_hash_for_draw_plan(draw_plan: &UiGpuDrawPlan) -> String {
    let mut hash = Fnv1a64::new();
    hash.write_str(&draw_plan.schema_version);
    hash.write_u32(draw_plan.surface_width);
    hash.write_u32(draw_plan.surface_height);
    hash.write_usize(draw_plan.draw_command_count);
    hash.write_usize(draw_plan.rect_count);
    hash.write_usize(draw_plan.text_command_count);
    hash.write_usize(draw_plan.skipped_text_count);
    hash.write_usize(draw_plan.rendered_glyph_count);
    hash.write_usize(draw_plan.unsupported_glyph_count);
    hash.write_usize(draw_plan.viewport_slot_count);
    hash.write_usize(draw_plan.hit_region_count);
    hash.write_str(&draw_plan.font_backend);
    hash.write_bool(draw_plan.font_loaded);
    hash.write_u32(draw_plan.glyph_atlas_width);
    hash.write_u32(draw_plan.glyph_atlas_height);
    hash.write_usize(draw_plan.glyph_cache_count);
    hash.write_usize(draw_plan.missing_glyph_count);

    for rect in &draw_plan.drawable_rects {
        hash.write_i64(quantize(rect.rect.x));
        hash.write_i64(quantize(rect.rect.y));
        hash.write_i64(quantize(rect.rect.width));
        hash.write_i64(quantize(rect.rect.height));
        hash.write_u8(rect.color.r);
        hash.write_u8(rect.color.g);
        hash.write_u8(rect.color.b);
        hash.write_u8(rect.color.a);
        hash.write_u8(source_kind_id(rect.source_kind));
    }

    for glyph in &draw_plan.text_glyphs {
        hash.write_i64(quantize(glyph.rect.x));
        hash.write_i64(quantize(glyph.rect.y));
        hash.write_i64(quantize(glyph.rect.width));
        hash.write_i64(quantize(glyph.rect.height));
        hash.write_i64(quantize(glyph.uv.u0));
        hash.write_i64(quantize(glyph.uv.v0));
        hash.write_i64(quantize(glyph.uv.u1));
        hash.write_i64(quantize(glyph.uv.v1));
        hash.write_u8(glyph.color.r);
        hash.write_u8(glyph.color.g);
        hash.write_u8(glyph.color.b);
        hash.write_u8(glyph.color.a);
    }

    for batch in &draw_plan.paint_batches {
        hash.write_u8(paint_batch_kind_id(batch.kind));
        hash.write_usize(batch.first_item);
        hash.write_usize(batch.item_count);
    }

    format!("{:016x}", hash.finish())
}

fn paint_batch_kind_id(kind: UiGpuPaintBatchKind) -> u8 {
    match kind {
        UiGpuPaintBatchKind::Rects => 1,
        UiGpuPaintBatchKind::Text => 2,
        UiGpuPaintBatchKind::ViewportTextures => 3,
        UiGpuPaintBatchKind::ImageTextures => 4,
    }
}

fn source_kind_id(kind: UiGpuDrawableRectSource) -> u8 {
    match kind {
        UiGpuDrawableRectSource::Rect => 1,
        UiGpuDrawableRectSource::ViewportPlaceholder => 2,
        UiGpuDrawableRectSource::ImageTexturePlaceholder => 3,
        UiGpuDrawableRectSource::TextGlyph => 4,
    }
}

fn quantize(value: f32) -> i64 {
    (value * 100.0).round() as i64
}

struct Fnv1a64 {
    value: u64,
}

impl Fnv1a64 {
    fn new() -> Self {
        Self {
            value: 0xcbf29ce484222325,
        }
    }

    fn finish(&self) -> u64 {
        self.value
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.value ^= u64::from(*byte);
            self.value = self.value.wrapping_mul(0x100000001b3);
        }
    }

    fn write_str(&mut self, value: &str) {
        self.write_usize(value.len());
        self.write_bytes(value.as_bytes());
    }

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_u8(&mut self, value: u8) {
        self.write_bytes(&[value]);
    }

    fn write_u32(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_bytes(&(value as u64).to_le_bytes());
    }
}
