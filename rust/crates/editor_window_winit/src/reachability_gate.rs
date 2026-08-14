use crate::{NativeEditorApplication, NativeEditorWindowConfig};
use editor_ui_renderer::{
    pick_widget, EditorWidgetTree, UiPoint, UiRect, WidgetId, WidgetRole, WidgetVisibility,
};
use serde::{Deserialize, Serialize};

pub const EDITOR_WIDGET_TREE_SNAPSHOT_SCHEMA_VERSION: &str = "editor-widget-tree-snapshot.v1";
pub const EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION: &str = "editor-ui-reachability-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorReachabilityReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorReachabilityStatus {
    Passed,
    Failed,
    EnvironmentBlocked,
    NotEvaluated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorReachabilityDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorReachabilityDiagnostic {
    pub severity: EditorReachabilityDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub widget_id: Option<String>,
    pub source_stage: String,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EditorReachabilityViewport {
    pub logical_width: f64,
    pub logical_height: f64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
}

impl EditorReachabilityViewport {
    pub fn from_physical(physical_width: u32, physical_height: u32, scale_factor: f64) -> Self {
        let scale_factor = scale_factor.max(f64::EPSILON);
        Self {
            logical_width: f64::from(physical_width) / scale_factor,
            logical_height: f64::from(physical_height) / scale_factor,
            physical_width,
            physical_height,
            scale_factor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EditorReachabilityRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl EditorReachabilityRect {
    fn logical(rect: UiRect) -> Self {
        Self {
            x: f64::from(rect.x),
            y: f64::from(rect.y),
            width: f64::from(rect.width),
            height: f64::from(rect.height),
        }
    }

    fn physical(rect: UiRect, scale_factor: f64) -> Self {
        Self {
            x: f64::from(rect.x) * scale_factor,
            y: f64::from(rect.y) * scale_factor,
            width: f64::from(rect.width) * scale_factor,
            height: f64::from(rect.height) * scale_factor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWidgetReachabilityEntry {
    pub widget_id: String,
    pub parent_id: Option<String>,
    pub role: WidgetRole,
    pub visibility: WidgetVisibility,
    pub enabled: bool,
    pub focusable: bool,
    pub reachable: bool,
    pub command_id: Option<String>,
    pub disabled_reason: Option<String>,
    pub logical_rect: EditorReachabilityRect,
    pub physical_rect: EditorReachabilityRect,
    pub effective_clip: Option<EditorReachabilityRect>,
    pub pick_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWidgetTreeSnapshot {
    pub schema_version: String,
    pub frame_index: u64,
    pub model_revision: u64,
    pub root_widget_id: String,
    pub viewport: EditorReachabilityViewport,
    pub widget_count: usize,
    pub visible_widget_count: usize,
    pub focusable_widget_count: usize,
    pub reachable_widget_count: usize,
    pub duplicate_widget_count: usize,
    pub keyboard_focus_widget_id: Option<String>,
    pub pointer_capture_widget_id: Option<String>,
    pub widgets: Vec<EditorWidgetReachabilityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorScreenshotEvidenceKind {
    MetadataOnly,
    ActualOffscreenRgba,
    ActualWindowRgba,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorScreenshotEvidence {
    pub kind: EditorScreenshotEvidenceKind,
    pub width: u32,
    pub height: u32,
    pub frame_index: u64,
    pub tree_revision: u64,
    pub rgba_sha256: Option<String>,
    pub artifact_path: Option<String>,
    pub backend: String,
    pub font: String,
    pub os: String,
    pub gpu: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorInputReplayEvidence {
    pub input_kind: String,
    pub target_widget_id: String,
    pub command_id: Option<String>,
    pub client_logical_x: f64,
    pub client_logical_y: f64,
    pub client_physical_x: i32,
    pub client_physical_y: i32,
    pub screen_physical_x: Option<i32>,
    pub screen_physical_y: Option<i32>,
    pub target_pid: Option<u32>,
    pub foreground_verified: bool,
    pub pointer_down_observed: bool,
    pub pointer_up_observed: bool,
    pub wheel_observed: bool,
    pub before_command_id: Option<String>,
    pub after_command_id: Option<String>,
    pub before_model_revision: u64,
    pub after_model_revision: u64,
    pub focused_widget_id: Option<String>,
    pub route_status: EditorReachabilityStatus,
    pub diagnostics: Vec<EditorReachabilityDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorUiReachabilityReport {
    pub schema_version: String,
    pub report_level: EditorReachabilityReportLevel,
    pub scenario_id: String,
    pub status: EditorReachabilityStatus,
    pub snapshot: Option<EditorWidgetTreeSnapshot>,
    pub screenshot: Option<EditorScreenshotEvidence>,
    pub input_replay: Option<EditorInputReplayEvidence>,
    pub diagnostics: Vec<EditorReachabilityDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorReachabilityScenario {
    pub scenario_id: String,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
}

impl EditorReachabilityScenario {
    pub fn new(
        scenario_id: impl Into<String>,
        physical_width: u32,
        physical_height: u32,
        scale_factor: f64,
    ) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            physical_width,
            physical_height,
            scale_factor,
        }
    }
}

pub fn deterministic_reachability_scenarios() -> Vec<EditorReachabilityScenario> {
    let mut scenarios = Vec::with_capacity(9);
    for (width, height) in [(1280, 720), (1600, 900), (1920, 1080)] {
        for scale in [1.0, 1.5, 2.0] {
            scenarios.push(EditorReachabilityScenario::new(
                format!("{width}x{height}@{scale:.1}"),
                width,
                height,
                scale,
            ));
        }
    }
    scenarios
}

pub fn run_deterministic_reachability_scenario(
    scenario: &EditorReachabilityScenario,
    level: EditorReachabilityReportLevel,
) -> EditorUiReachabilityReport {
    if level == EditorReachabilityReportLevel::Off {
        return EditorUiReachabilityReport {
            schema_version: EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION.to_string(),
            report_level: level,
            scenario_id: scenario.scenario_id.clone(),
            status: EditorReachabilityStatus::NotEvaluated,
            snapshot: None,
            screenshot: None,
            input_replay: None,
            diagnostics: Vec::new(),
        };
    }
    let viewport = EditorReachabilityViewport::from_physical(
        scenario.physical_width,
        scenario.physical_height,
        scenario.scale_factor,
    );
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig {
        width: viewport.logical_width.round() as u32,
        height: viewport.logical_height.round() as u32,
        scale_factor: scenario.scale_factor,
        ..NativeEditorWindowConfig::default()
    });
    let app_report = app.frame(
        viewport.logical_width as f32,
        viewport.logical_height as f32,
    );
    let Some(tree) = app.retained_ui_renderer().tree() else {
        return failed_without_snapshot(
            scenario,
            level,
            "reachability.widget_tree_missing",
            "The retained editor frame did not produce a WidgetTree.",
        );
    };
    let (snapshot, diagnostics) = snapshot_widget_tree(
        tree,
        EditorWidgetSnapshotContext {
            frame_index: app_report.frame_index,
            model_revision: app_report.model_revision,
            viewport,
            keyboard_focus: app.focus_input().keyboard_focus.as_ref(),
            pointer_capture: app.focus_input().pointer_capture.as_ref(),
            level,
        },
    );
    let status = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == EditorReachabilityDiagnosticSeverity::Error)
    {
        EditorReachabilityStatus::Failed
    } else {
        EditorReachabilityStatus::Passed
    };
    EditorUiReachabilityReport {
        schema_version: EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION.to_string(),
        report_level: level,
        scenario_id: scenario.scenario_id.clone(),
        status,
        snapshot: Some(snapshot),
        screenshot: None,
        input_replay: None,
        diagnostics,
    }
}

pub struct EditorWidgetSnapshotContext<'a> {
    pub frame_index: u64,
    pub model_revision: u64,
    pub viewport: EditorReachabilityViewport,
    pub keyboard_focus: Option<&'a WidgetId>,
    pub pointer_capture: Option<&'a WidgetId>,
    pub level: EditorReachabilityReportLevel,
}

pub fn snapshot_widget_tree(
    tree: &EditorWidgetTree,
    context: EditorWidgetSnapshotContext<'_>,
) -> (EditorWidgetTreeSnapshot, Vec<EditorReachabilityDiagnostic>) {
    let EditorWidgetSnapshotContext {
        frame_index,
        model_revision,
        viewport,
        keyboard_focus,
        pointer_capture,
        level,
    } = context;
    let mut diagnostics = Vec::new();
    let mut widgets = Vec::with_capacity(tree.nodes.len());
    let mut visible_widget_count = 0;
    let mut focusable_widget_count = 0;
    let mut reachable_widget_count = 0;
    let popup_open = tree
        .nodes
        .values()
        .any(|node| node.hit_region_id.as_deref() == Some("hit.toolbar.overflow.barrier"));
    for node in tree.nodes.values() {
        let interactive = node.binding.is_some();
        let clipped_rect = node.effective_clip.map_or(Some(node.logical_rect), |clip| {
            node.logical_rect.intersection(clip)
        });
        let visible = node.visibility == WidgetVisibility::Visible
            && clipped_rect.is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0);
        let in_modal_scope = !popup_open
            || node
                .id
                .as_str()
                .starts_with("editor/shell/toolbar/overflow/");
        let focusable = visible && node.enabled && interactive && in_modal_scope;
        visible_widget_count += usize::from(visible);
        focusable_widget_count += usize::from(focusable);
        let candidate_picks = clipped_rect
            .map(reachability_probe_points)
            .into_iter()
            .flatten()
            .filter_map(|point| pick_widget(tree, point, None))
            .collect::<Vec<_>>();
        let successful_pick = candidate_picks
            .iter()
            .find(|pick| pick.path.0.contains(&node.id));
        let diagnostic_pick = candidate_picks.first();
        let reachable = visible
            && clipped_rect.is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0)
            && (!interactive || successful_pick.is_some());
        reachable_widget_count += usize::from(reachable);
        if visible && interactive && in_modal_scope && !reachable {
            let picked_widget = diagnostic_pick
                .map(|pick| pick.target.as_str())
                .unwrap_or("none");
            diagnostics.push(widget_diagnostic(
                EditorReachabilityDiagnosticSeverity::Error,
                "reachability.control_unreachable",
                format!(
                    "A visible interactive widget cannot be picked through its retained geometry; center pick resolved to {picked_widget}; logical_rect={:?}; effective_clip={:?}; tested_rect={:?}.",
                    node.logical_rect, node.effective_clip, clipped_rect
                ),
                &node.id,
                "widget_pick",
                "Inspect the widget clip, overlay order, and parent layout.",
            ));
        }
        if visible
            && !node.enabled
            && node.binding.is_some()
            && node
                .binding
                .as_ref()
                .and_then(|binding| binding.reason_disabled.as_ref())
                .is_none()
        {
            diagnostics.push(widget_diagnostic(
                EditorReachabilityDiagnosticSeverity::Error,
                "reachability.disabled_reason_missing",
                "A disabled interactive widget does not explain why it is unavailable.",
                &node.id,
                "widget_contract",
                "Provide a user-facing disabled reason in the command binding.",
            ));
        }
        let pick_path = successful_pick
            .or(diagnostic_pick)
            .map(|pick| {
                pick.path
                    .0
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default();
        if level == EditorReachabilityReportLevel::Trace || interactive {
            widgets.push(EditorWidgetReachabilityEntry {
                widget_id: node.id.as_str().to_string(),
                parent_id: node.parent.as_ref().map(|id| id.as_str().to_string()),
                role: node.role,
                visibility: node.visibility,
                enabled: node.enabled,
                focusable,
                reachable,
                command_id: node
                    .binding
                    .as_ref()
                    .map(|binding| binding.command_id.clone()),
                disabled_reason: node
                    .binding
                    .as_ref()
                    .and_then(|binding| binding.reason_disabled.clone()),
                logical_rect: EditorReachabilityRect::logical(node.logical_rect),
                physical_rect: EditorReachabilityRect::physical(
                    node.logical_rect,
                    viewport.scale_factor,
                ),
                effective_clip: node.effective_clip.map(EditorReachabilityRect::logical),
                pick_path,
            });
        }
    }
    let snapshot = EditorWidgetTreeSnapshot {
        schema_version: EDITOR_WIDGET_TREE_SNAPSHOT_SCHEMA_VERSION.to_string(),
        frame_index,
        model_revision,
        root_widget_id: tree.root.as_str().to_string(),
        viewport,
        widget_count: tree.nodes.len(),
        visible_widget_count,
        focusable_widget_count,
        reachable_widget_count,
        duplicate_widget_count: 0,
        keyboard_focus_widget_id: keyboard_focus.map(|id| id.as_str().to_string()),
        pointer_capture_widget_id: pointer_capture.map(|id| id.as_str().to_string()),
        widgets,
    };
    (snapshot, diagnostics)
}

fn reachability_probe_points(rect: UiRect) -> [UiPoint; 5] {
    let inset_x = (rect.width * 0.1).clamp(1.0, 4.0);
    let inset_y = (rect.height * 0.1).clamp(1.0, 4.0);
    [
        UiPoint {
            x: rect.x + rect.width * 0.5,
            y: rect.y + rect.height * 0.5,
        },
        UiPoint {
            x: rect.x + inset_x,
            y: rect.y + inset_y,
        },
        UiPoint {
            x: rect.x + rect.width - inset_x,
            y: rect.y + inset_y,
        },
        UiPoint {
            x: rect.x + inset_x,
            y: rect.y + rect.height - inset_y,
        },
        UiPoint {
            x: rect.x + rect.width - inset_x,
            y: rect.y + rect.height - inset_y,
        },
    ]
}

fn failed_without_snapshot(
    scenario: &EditorReachabilityScenario,
    level: EditorReachabilityReportLevel,
    code: &str,
    message: &str,
) -> EditorUiReachabilityReport {
    EditorUiReachabilityReport {
        schema_version: EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION.to_string(),
        report_level: level,
        scenario_id: scenario.scenario_id.clone(),
        status: EditorReachabilityStatus::Failed,
        snapshot: None,
        screenshot: None,
        input_replay: None,
        diagnostics: vec![EditorReachabilityDiagnostic {
            severity: EditorReachabilityDiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.to_string(),
            widget_id: None,
            source_stage: "reachability_gate".to_string(),
            next_action: Some("Render one retained editor frame and retry.".to_string()),
        }],
    }
}

fn widget_diagnostic(
    severity: EditorReachabilityDiagnosticSeverity,
    code: &str,
    message: impl Into<String>,
    widget_id: &WidgetId,
    source_stage: &str,
    next_action: &str,
) -> EditorReachabilityDiagnostic {
    EditorReachabilityDiagnostic {
        severity,
        code: code.to_string(),
        message: message.into(),
        widget_id: Some(widget_id.as_str().to_string()),
        source_stage: source_stage.to_string(),
        next_action: Some(next_action.to_string()),
    }
}
