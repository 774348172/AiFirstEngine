use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::canonical_digest::sha256_prefixed;
use crate::font_bundle::{
    FontBundleRenderMode, FontBundleStyle, RuntimeFontBundleRegistry, RuntimeFontRegistry,
    RuntimeFontResolveRequest,
};
use crate::game_view_presentation::{
    CanvasReferenceFact, GameViewPoint, ResolvedGameViewPresentation,
};
use crate::input_mapping::{
    RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton, RuntimePointerDeviceKind,
    RuntimePointerPhase,
};
use crate::projection::{ProjectionDomain, ProjectionKind, ProjectionReport};
use crate::runtime_package::{RuntimeAuiFontAtlasRegistry, RuntimePackage};
use crate::world::World;

pub const AUI_DOCUMENT_SCHEMA_VERSION: &str = "aui-document.v2";
pub const LEGACY_AUI_DOCUMENT_SCHEMA_VERSION: &str = "aui-document.v1";
pub const AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID: &str = "button.default.v1";
pub const AUI_ASSET_MANIFEST_SCHEMA_VERSION: &str = "aui-asset-manifest.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuiFeedbackSelection(String);

impl AuiFeedbackSelection {
    pub fn auto() -> Self {
        Self("auto".to_string())
    }

    pub fn none() -> Self {
        Self("none".to_string())
    }

    pub fn profile(profile_id: impl Into<String>) -> Self {
        Self(profile_id.into())
    }

    pub fn profile_id(&self) -> Option<&str> {
        match self.0.as_str() {
            "auto" | "none" => None,
            profile_id => Some(profile_id),
        }
    }

    pub fn is_auto(&self) -> bool {
        self.0 == "auto"
    }

    pub fn is_none(&self) -> bool {
        self.0 == "none"
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AuiFeedbackSelection {
    fn default() -> Self {
        Self::auto()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiVec2 {
    pub x: f32,
    pub y: f32,
}

impl AuiVec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiFeedbackEasing {
    Linear,
    #[default]
    EaseOutCubic,
    EaseOutBack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuiInteractionFeedbackProfile {
    pub profile_id: String,
    pub hover_scale_permille: u16,
    pub hover_brightness_permille: i16,
    pub hover_opacity_permille: u16,
    pub pressed_scale_permille: u16,
    pub pressed_brightness_permille: i16,
    pub pressed_opacity_permille: u16,
    pub pressed_offset: AuiVec2,
    pub activated_scale_permille: u16,
    pub activated_brightness_permille: i16,
    pub activated_opacity_permille: u16,
    pub disabled_opacity_permille: u16,
    pub hover_in_ms: u16,
    pub hover_out_ms: u16,
    pub press_in_ms: u16,
    pub release_ms: u16,
    pub activated_ms: u16,
    pub cancel_ms: u16,
    pub hover_easing: AuiFeedbackEasing,
    pub press_easing: AuiFeedbackEasing,
    pub release_easing: AuiFeedbackEasing,
    pub activated_easing: AuiFeedbackEasing,
}

impl Default for AuiInteractionFeedbackProfile {
    fn default() -> Self {
        Self::new("")
    }
}

impl AuiInteractionFeedbackProfile {
    pub fn new(profile_id: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            hover_scale_permille: 1010,
            hover_brightness_permille: 40,
            hover_opacity_permille: 1000,
            pressed_scale_permille: 970,
            pressed_brightness_permille: -80,
            pressed_opacity_permille: 1000,
            pressed_offset: AuiVec2::new(0.0, 1.0),
            activated_scale_permille: 1020,
            activated_brightness_permille: 60,
            activated_opacity_permille: 1000,
            disabled_opacity_permille: 550,
            hover_in_ms: 70,
            hover_out_ms: 80,
            press_in_ms: 45,
            release_ms: 80,
            activated_ms: 120,
            cancel_ms: 80,
            hover_easing: AuiFeedbackEasing::EaseOutCubic,
            press_easing: AuiFeedbackEasing::EaseOutCubic,
            release_easing: AuiFeedbackEasing::EaseOutCubic,
            activated_easing: AuiFeedbackEasing::EaseOutBack,
        }
    }
}

pub fn builtin_button_feedback_profile_v1() -> AuiInteractionFeedbackProfile {
    AuiInteractionFeedbackProfile::new(AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID)
}

fn default_feedback_motion_scale_permille() -> u16 {
    1000
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuiInteractionFeedbackRegistry {
    #[serde(default = "default_feedback_motion_scale_permille")]
    pub motion_scale_permille: u16,
    pub default_button_profile: Option<String>,
    pub profiles: Vec<AuiInteractionFeedbackProfile>,
}

impl Default for AuiInteractionFeedbackRegistry {
    fn default() -> Self {
        Self {
            motion_scale_permille: default_feedback_motion_scale_permille(),
            default_button_profile: None,
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiComputedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl AuiComputedRect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.width).min(other.x + other.width);
        let y1 = (self.y + self.height).min(other.y + other.height);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Self {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }

    pub fn approximately_full_screen(self, reference_resolution: AuiVec2) -> bool {
        self.x.abs() <= 0.01
            && self.y.abs() <= 0.01
            && (self.width - reference_resolution.x).abs() <= 0.01
            && (self.height - reference_resolution.y).abs() <= 0.01
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AuiRect {
    pub anchor_min: AuiVec2,
    pub anchor_max: AuiVec2,
    pub offset_min: AuiVec2,
    pub offset_max: AuiVec2,
    pub pivot: AuiVec2,
    pub size: AuiVec2,
}

impl AuiRect {
    pub fn fixed_position(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            anchor_min: AuiVec2::new(0.0, 0.0),
            anchor_max: AuiVec2::new(0.0, 0.0),
            offset_min: AuiVec2::new(x, y),
            offset_max: AuiVec2::new(0.0, 0.0),
            pivot: AuiVec2::new(0.0, 0.0),
            size: AuiVec2::new(width, height),
        }
    }

    pub fn stretch_full() -> Self {
        Self {
            anchor_min: AuiVec2::new(0.0, 0.0),
            anchor_max: AuiVec2::new(1.0, 1.0),
            offset_min: AuiVec2::new(0.0, 0.0),
            offset_max: AuiVec2::new(0.0, 0.0),
            pivot: AuiVec2::new(0.5, 0.5),
            size: AuiVec2::new(0.0, 0.0),
        }
    }

    pub fn resolve(self, parent: AuiComputedRect) -> AuiComputedRect {
        let anchored_min_x = parent.x + parent.width * self.anchor_min.x + self.offset_min.x;
        let anchored_min_y = parent.y + parent.height * self.anchor_min.y + self.offset_min.y;
        let anchored_max_x = parent.x + parent.width * self.anchor_max.x - self.offset_max.x;
        let anchored_max_y = parent.y + parent.height * self.anchor_max.y - self.offset_max.y;

        if self.anchor_min != self.anchor_max {
            return AuiComputedRect {
                x: anchored_min_x,
                y: anchored_min_y,
                width: (anchored_max_x - anchored_min_x).max(0.0),
                height: (anchored_max_y - anchored_min_y).max(0.0),
            };
        }

        AuiComputedRect {
            x: anchored_min_x - self.size.x * self.pivot.x,
            y: anchored_min_y - self.size.y * self.pivot.y,
            width: self.size.x.max(0.0),
            height: self.size.y.max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiCanvasMode {
    ScreenOverlay,
    ScreenCamera,
    WorldSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AuiCompositionStage {
    BeforeWorld,
    ScreenOverlay,
    Modal,
}

impl Default for AuiCompositionStage {
    fn default() -> Self {
        Self::ScreenOverlay
    }
}

impl AuiCompositionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeWorld => "BeforeWorld",
            Self::ScreenOverlay => "ScreenOverlay",
            Self::Modal => "Modal",
        }
    }

    pub fn debug_label(self) -> &'static str {
        match self {
            Self::BeforeWorld => "AUI BeforeWorld",
            Self::ScreenOverlay => "AUI ScreenOverlay",
            Self::Modal => "AUI Modal",
        }
    }

    pub fn pass_id_suffix(self) -> &'static str {
        match self {
            Self::BeforeWorld => "before-world",
            Self::ScreenOverlay => "screen-overlay",
            Self::Modal => "modal",
        }
    }

    pub fn ordered() -> [Self; 3] {
        [Self::BeforeWorld, Self::ScreenOverlay, Self::Modal]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiScaleMode {
    ConstantPixelSize,
    ScaleWithScreenSize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiCanvas {
    pub canvas_id: String,
    pub mode: AuiCanvasMode,
    #[serde(default)]
    pub composition_stage: AuiCompositionStage,
    #[serde(default = "default_aui_visible")]
    pub visible: bool,
    pub layer: i32,
    pub sorting_order: i32,
    pub reference_resolution: AuiVec2,
    pub scale_mode: AuiScaleMode,
    pub root_node: String,
    #[serde(default)]
    pub screen_id: Option<String>,
    #[serde(default)]
    pub default_focus_node_id: Option<String>,
    #[serde(default)]
    pub cancel_action_id: Option<String>,
    #[serde(default)]
    pub submit_action_id: Option<String>,
}

impl AuiCanvas {
    pub fn screen_overlay(
        canvas_id: impl Into<String>,
        width: f32,
        height: f32,
        root_node: impl Into<String>,
    ) -> Self {
        Self {
            canvas_id: canvas_id.into(),
            mode: AuiCanvasMode::ScreenOverlay,
            composition_stage: AuiCompositionStage::ScreenOverlay,
            visible: true,
            layer: 0,
            sorting_order: 0,
            reference_resolution: AuiVec2::new(width, height),
            scale_mode: AuiScaleMode::ConstantPixelSize,
            root_node: root_node.into(),
            screen_id: None,
            default_focus_node_id: None,
            cancel_action_id: None,
            submit_action_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiNodeKind {
    Panel,
    Image,
    Text,
    Button,
    ProgressBar,
    Toggle,
    Slider,
    List,
    ScrollView,
    InputField,
    Custom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiClipPolicy {
    #[default]
    None,
    Rect,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiScrollbarPolicy {
    None,
    #[default]
    Auto,
    Always,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiScrollbarAxis {
    #[default]
    Vertical,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiNavigationMode {
    None,
    #[default]
    Auto,
    Vertical,
    Horizontal,
    Explicit,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiNavigationRef {
    pub mode: AuiNavigationMode,
    pub up: Option<String>,
    pub down: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
    pub next: Option<String>,
    pub previous: Option<String>,
}

impl AuiNavigationRef {
    pub fn auto() -> Self {
        Self {
            mode: AuiNavigationMode::Auto,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiStyle {
    pub color: Option<String>,
    pub text_color: Option<String>,
    pub font_size: Option<f32>,
    #[serde(default)]
    pub font: Option<AuiFontStyle>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiFontStyleKind {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiFontRasterPolicy {
    #[default]
    AutoHybrid,
    Bitmap,
    Msdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiFontStyle {
    #[serde(default)]
    pub font_bundle_id: Option<String>,
    #[serde(default)]
    pub font_family_id: Option<String>,
    #[serde(default)]
    pub style: AuiFontStyleKind,
    #[serde(default = "default_font_weight")]
    pub weight: u16,
    #[serde(default)]
    pub raster_policy: AuiFontRasterPolicy,
}

fn default_font_weight() -> u16 {
    400
}

impl AuiStyle {
    pub fn color(color: impl Into<String>) -> Self {
        Self {
            color: Some(color.into()),
            text_color: None,
            font_size: None,
            font: None,
        }
    }

    pub fn text(color: impl Into<String>, font_size: f32) -> Self {
        Self {
            color: None,
            text_color: Some(color.into()),
            font_size: Some(font_size),
            font: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiAssetRef {
    pub asset_id: String,
}

impl AuiAssetRef {
    pub fn new(asset_id: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiBindingRef {
    pub binding_id: String,
    pub target_field: AuiBindingTarget,
    pub path: String,
    pub fallback: Option<AuiBindingValue>,
}

impl AuiBindingRef {
    pub fn new(
        binding_id: impl Into<String>,
        target_field: AuiBindingTarget,
        path: impl Into<String>,
        fallback: Option<AuiBindingValue>,
    ) -> Self {
        Self {
            binding_id: binding_id.into(),
            target_field,
            path: path.into(),
            fallback,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiBindingTarget {
    TextText,
    InputFieldText,
    ProgressBarValue,
    PanelVisible,
    ImageVisible,
    ImageAssetRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AuiBindingValue {
    Bool(bool),
    Number(f32),
    String(String),
    Color(String),
    AssetRef(AuiAssetRef),
}

impl AuiBindingValue {
    fn type_name(&self) -> &'static str {
        match self {
            AuiBindingValue::Bool(_) => "bool",
            AuiBindingValue::Number(_) => "number",
            AuiBindingValue::String(_) => "string",
            AuiBindingValue::Color(_) => "color",
            AuiBindingValue::AssetRef(_) => "asset_ref",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectUiStateSnapshot {
    pub frame_index: u64,
    pub values: HashMap<String, AuiBindingValue>,
}

impl ProjectUiStateSnapshot {
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            values: HashMap::new(),
        }
    }

    pub fn with_value(mut self, path: impl Into<String>, value: AuiBindingValue) -> Self {
        self.values.insert(path.into(), value);
        self
    }

    pub fn package_smoke_snapshot(frame_index: u64) -> Self {
        Self::new(frame_index)
            .with_value(
                "game.score_text",
                AuiBindingValue::String("SCORE 000000".to_string()),
            )
            .with_value("player.hp_ratio", AuiBindingValue::Number(1.0))
            .with_value("game.paused", AuiBindingValue::Bool(false))
            .with_value(
                "player.ship_icon",
                AuiBindingValue::AssetRef(AuiAssetRef::new("tex-player-ship")),
            )
    }
}

pub const PROJECT_UI_STATE_SNAPSHOT_REPORT_SCHEMA_VERSION: &str =
    "project-ui-state-snapshot-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectUiStateSnapshotStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectUiStateSnapshotDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUiStateSnapshotDiagnostic {
    pub severity: ProjectUiStateSnapshotDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl ProjectUiStateSnapshotDiagnostic {
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: ProjectUiStateSnapshotDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path,
        }
    }

    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: ProjectUiStateSnapshotDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUiStateSnapshotReport {
    pub schema_version: String,
    pub status: ProjectUiStateSnapshotStatus,
    pub producer_id: String,
    pub frame_index: u64,
    pub snapshot_source: AuiSnapshotSource,
    pub value_count: usize,
    pub active_binding_paths: Vec<String>,
    pub produced_paths: Vec<String>,
    pub declared_binding_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub type_mismatch_paths: Vec<String>,
    pub dirty_domains: Vec<String>,
    pub cache_status: String,
    pub cache_hit_paths: Vec<String>,
    pub cache_miss_paths: Vec<String>,
    pub source_paths: Vec<String>,
    pub diagnostics: Vec<ProjectUiStateSnapshotDiagnostic>,
}

impl ProjectUiStateSnapshotReport {
    pub fn from_snapshot(
        producer_id: impl Into<String>,
        snapshot_source: AuiSnapshotSource,
        snapshot: &ProjectUiStateSnapshot,
    ) -> Self {
        let produced_paths = sorted_snapshot_paths(snapshot);
        Self {
            schema_version: PROJECT_UI_STATE_SNAPSHOT_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectUiStateSnapshotStatus::Passed,
            producer_id: producer_id.into(),
            frame_index: snapshot.frame_index,
            snapshot_source,
            value_count: snapshot.values.len(),
            active_binding_paths: Vec::new(),
            produced_paths,
            declared_binding_paths: Vec::new(),
            missing_paths: Vec::new(),
            type_mismatch_paths: Vec::new(),
            dirty_domains: Vec::new(),
            cache_status: "not_reported".to_string(),
            cache_hit_paths: Vec::new(),
            cache_miss_paths: Vec::new(),
            source_paths: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn with_binding_report(
        mut self,
        document: &AuiDocument,
        binding_report: &AuiBindingReport,
    ) -> Self {
        self.declared_binding_paths = declared_binding_paths(document);
        self.missing_paths = binding_report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "aui_binding.missing_path")
            .map(|diagnostic| diagnostic.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.type_mismatch_paths = binding_report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "aui_binding.type_mismatch")
            .map(|diagnostic| diagnostic.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        for path in &self.missing_paths {
            self.diagnostics
                .push(ProjectUiStateSnapshotDiagnostic::error(
                    "project_ui_state_snapshot.missing_path",
                    format!(
                        "ProjectUiStateSnapshot did not produce required binding path '{path}'."
                    ),
                    Some(path.clone()),
                ));
        }
        for path in &self.type_mismatch_paths {
            self.diagnostics
                .push(ProjectUiStateSnapshotDiagnostic::error(
                    "project_ui_state_snapshot.type_mismatch",
                    format!(
                        "ProjectUiStateSnapshot produced a value with the wrong type for '{path}'."
                    ),
                    Some(path.clone()),
                ));
        }

        let has_error = self.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == ProjectUiStateSnapshotDiagnosticSeverity::Error
        });
        self.status = if has_error {
            ProjectUiStateSnapshotStatus::Failed
        } else if binding_report.fallback_count > 0 {
            ProjectUiStateSnapshotStatus::Partial
        } else {
            ProjectUiStateSnapshotStatus::Passed
        };
        self
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == ProjectUiStateSnapshotDiagnosticSeverity::Error
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUiStateSnapshotOutput {
    pub snapshot: ProjectUiStateSnapshot,
    pub report: ProjectUiStateSnapshotReport,
}

impl ProjectUiStateSnapshotOutput {
    pub fn new(
        producer_id: impl Into<String>,
        snapshot_source: AuiSnapshotSource,
        snapshot: ProjectUiStateSnapshot,
    ) -> Self {
        let report =
            ProjectUiStateSnapshotReport::from_snapshot(producer_id, snapshot_source, &snapshot);
        Self { snapshot, report }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectUiStateReportMode {
    Off,
    #[default]
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUiBindingSetIdentity {
    pub digest: String,
}

impl ProjectUiBindingSetIdentity {
    pub fn from_paths(paths: &[String]) -> Self {
        let mut canonical = paths
            .iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut bytes = Vec::new();
        for path in canonical.drain(..) {
            bytes.extend_from_slice(&(path.len() as u64).to_be_bytes());
            bytes.extend_from_slice(path.as_bytes());
        }
        Self {
            digest: sha256_prefixed(&bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUiStateIdentity {
    pub producer_epoch: u64,
    pub visible_revision: u64,
    pub binding_set: ProjectUiBindingSetIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectUiBindingSet {
    Known(ProjectUiBindingSetIdentity),
    Replace {
        identity: ProjectUiBindingSetIdentity,
        active_binding_paths: Vec<String>,
    },
}

impl ProjectUiBindingSet {
    pub fn identity(&self) -> &ProjectUiBindingSetIdentity {
        match self {
            Self::Known(identity) | Self::Replace { identity, .. } => identity,
        }
    }

    pub fn active_binding_paths(&self) -> Option<&[String]> {
        match self {
            Self::Known(_) => None,
            Self::Replace {
                active_binding_paths,
                ..
            } => Some(active_binding_paths),
        }
    }
}

pub struct ProjectUiStateProducerContext<'a> {
    pub frame_index: u64,
    pub package: &'a RuntimePackage,
    pub world: &'a World,
    pub binding_set: ProjectUiBindingSet,
    pub previous_identity: Option<ProjectUiStateIdentity>,
    pub report_mode: ProjectUiStateReportMode,
}

impl<'a> ProjectUiStateProducerContext<'a> {
    pub fn new(frame_index: u64, package: &'a RuntimePackage, world: &'a World) -> Self {
        Self {
            frame_index,
            package,
            world,
            binding_set: ProjectUiBindingSet::Replace {
                identity: ProjectUiBindingSetIdentity::from_paths(&[]),
                active_binding_paths: Vec::new(),
            },
            previous_identity: None,
            report_mode: ProjectUiStateReportMode::Summary,
        }
    }

    pub fn with_active_binding_paths(mut self, paths: impl IntoIterator<Item = String>) -> Self {
        let active_binding_paths = sorted_strings(paths);
        self.binding_set = ProjectUiBindingSet::Replace {
            identity: ProjectUiBindingSetIdentity::from_paths(&active_binding_paths),
            active_binding_paths,
        };
        self
    }

    pub fn with_binding_set(mut self, binding_set: ProjectUiBindingSet) -> Self {
        self.binding_set = binding_set;
        self
    }

    pub fn with_previous_identity(mut self, identity: Option<ProjectUiStateIdentity>) -> Self {
        self.previous_identity = identity;
        self
    }

    pub fn with_report_mode(mut self, report_mode: ProjectUiStateReportMode) -> Self {
        self.report_mode = report_mode;
        self
    }
}

pub trait ProjectUiStateSnapshotProducer {
    fn producer_id(&self) -> &str;

    /// Source-level fixture compatibility. Production consumers call `resolve`.
    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput;

    fn resolve(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
        Ok(ProjectUiStateResolve::Uncacheable {
            output: self.produce(context),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectUiStateResolve {
    Reuse {
        identity: ProjectUiStateIdentity,
    },
    Replace {
        identity: ProjectUiStateIdentity,
        output: ProjectUiStateSnapshotOutput,
    },
    Uncacheable {
        output: ProjectUiStateSnapshotOutput,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectUiStateResolveError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectUiStateSnapshotCacheResult {
    Reuse,
    Replace(ProjectUiStateSnapshotOutput),
}

pub struct ProjectUiStateSnapshotCache {
    active_binding_paths: Vec<String>,
    binding_set_identity: ProjectUiBindingSetIdentity,
    binding_set_registered: bool,
    previous_identity: Option<ProjectUiStateIdentity>,
}

impl ProjectUiStateSnapshotCache {
    pub fn new(paths: impl IntoIterator<Item = String>) -> Self {
        let active_binding_paths = sorted_strings(paths);
        let binding_set_identity = ProjectUiBindingSetIdentity::from_paths(&active_binding_paths);
        Self {
            active_binding_paths,
            binding_set_identity,
            binding_set_registered: false,
            previous_identity: None,
        }
    }

    pub fn active_binding_paths(&self) -> &[String] {
        &self.active_binding_paths
    }

    pub fn resolve(
        &mut self,
        producer: &mut dyn ProjectUiStateSnapshotProducer,
        frame_index: u64,
        package: &RuntimePackage,
        world: &World,
        report_mode: ProjectUiStateReportMode,
    ) -> Result<ProjectUiStateSnapshotCacheResult, ProjectUiStateResolveError> {
        let binding_set = if self.binding_set_registered {
            ProjectUiBindingSet::Known(self.binding_set_identity.clone())
        } else {
            ProjectUiBindingSet::Replace {
                identity: self.binding_set_identity.clone(),
                active_binding_paths: self.active_binding_paths.clone(),
            }
        };
        let result = producer.resolve(
            ProjectUiStateProducerContext::new(frame_index, package, world)
                .with_binding_set(binding_set)
                .with_previous_identity(self.previous_identity.clone())
                .with_report_mode(report_mode),
        )?;
        match result {
            ProjectUiStateResolve::Reuse { identity } => {
                if self.previous_identity.as_ref() != Some(&identity) {
                    return Err(ProjectUiStateResolveError::new(
                        "project_ui_state.reuse_without_baseline",
                        "producer returned Reuse without the caller's current identity",
                    ));
                }
                self.binding_set_registered = true;
                Ok(ProjectUiStateSnapshotCacheResult::Reuse)
            }
            ProjectUiStateResolve::Replace {
                identity,
                mut output,
            } => {
                if identity.binding_set != self.binding_set_identity {
                    return Err(ProjectUiStateResolveError::new(
                        "project_ui_state.resolve_contract_fault",
                        "producer returned an identity for a different binding set",
                    ));
                }
                output.report.active_binding_paths = self.active_binding_paths.clone();
                self.previous_identity = Some(identity);
                self.binding_set_registered = true;
                Ok(ProjectUiStateSnapshotCacheResult::Replace(output))
            }
            ProjectUiStateResolve::Uncacheable { mut output } => {
                output.report.active_binding_paths = self.active_binding_paths.clone();
                self.previous_identity = None;
                self.binding_set_registered = false;
                Ok(ProjectUiStateSnapshotCacheResult::Replace(output))
            }
        }
    }
}

impl ProjectUiStateResolveError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

fn sorted_snapshot_paths(snapshot: &ProjectUiStateSnapshot) -> Vec<String> {
    snapshot
        .values
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sorted_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn declared_binding_paths(document: &AuiDocument) -> Vec<String> {
    document
        .nodes
        .iter()
        .flat_map(|node| node.binding_refs.iter().map(|binding| binding.path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiActionRef {
    pub event: AuiActionEvent,
    pub action_id: String,
}

impl AuiActionRef {
    pub fn click(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Click,
            action_id: action_id.into(),
        }
    }

    pub fn drag_start(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::DragStart,
            action_id: action_id.into(),
        }
    }

    pub fn drag_move(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::DragMove,
            action_id: action_id.into(),
        }
    }

    pub fn drop(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Drop,
            action_id: action_id.into(),
        }
    }

    pub fn focus(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Focus,
            action_id: action_id.into(),
        }
    }

    pub fn blur(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Blur,
            action_id: action_id.into(),
        }
    }

    pub fn submit(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Submit,
            action_id: action_id.into(),
        }
    }

    pub fn cancel(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Cancel,
            action_id: action_id.into(),
        }
    }

    pub fn scroll(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Scroll,
            action_id: action_id.into(),
        }
    }

    pub fn text_changed(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::TextChanged,
            action_id: action_id.into(),
        }
    }

    pub fn text_submitted(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::TextSubmitted,
            action_id: action_id.into(),
        }
    }

    pub fn text_cancelled(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::TextCancelled,
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiActionEvent {
    Click,
    DragStart,
    DragMove,
    Drop,
    Focus,
    Blur,
    Submit,
    Cancel,
    Scroll,
    TextChanged,
    TextSubmitted,
    TextCancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiAction {
    pub action_id: String,
    pub node_id: String,
    pub event: AuiActionEvent,
    pub payload: Option<String>,
}

fn default_aui_consume_input() -> bool {
    true
}

fn default_aui_visible() -> bool {
    true
}

fn default_aui_submit_behavior() -> AuiInputSubmitBehavior {
    AuiInputSubmitBehavior::Submit
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiInputSubmitBehavior {
    #[default]
    Submit,
    InsertNewline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiNode {
    pub node_id: String,
    pub name: String,
    pub kind: AuiNodeKind,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub rect: AuiRect,
    pub visible: bool,
    pub interactable: bool,
    #[serde(default = "default_aui_consume_input")]
    pub consume_input: bool,
    #[serde(default)]
    pub draggable: bool,
    #[serde(default)]
    pub drop_target: bool,
    #[serde(default)]
    pub focusable: Option<bool>,
    #[serde(default)]
    pub clip_policy: AuiClipPolicy,
    #[serde(default)]
    pub scrollbar_policy: AuiScrollbarPolicy,
    #[serde(default)]
    pub navigation: AuiNavigationRef,
    #[serde(default)]
    pub feedback: AuiFeedbackSelection,
    pub style: Option<AuiStyle>,
    pub text: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default = "default_aui_submit_behavior")]
    pub submit_behavior: AuiInputSubmitBehavior,
    pub image: Option<AuiAssetRef>,
    pub progress_value: Option<f32>,
    pub binding_refs: Vec<AuiBindingRef>,
    pub action_refs: Vec<AuiActionRef>,
}

impl AuiNode {
    pub fn new(node_id: impl Into<String>, kind: AuiNodeKind, rect: AuiRect) -> Self {
        let node_id = node_id.into();
        Self {
            name: node_id.clone(),
            node_id,
            kind,
            parent: None,
            children: Vec::new(),
            rect,
            visible: true,
            interactable: false,
            consume_input: true,
            draggable: false,
            drop_target: false,
            focusable: None,
            clip_policy: AuiClipPolicy::None,
            scrollbar_policy: AuiScrollbarPolicy::Auto,
            navigation: AuiNavigationRef::default(),
            feedback: AuiFeedbackSelection::auto(),
            style: None,
            text: None,
            placeholder: None,
            max_length: None,
            read_only: false,
            submit_behavior: AuiInputSubmitBehavior::Submit,
            image: None,
            progress_value: None,
            binding_refs: Vec::new(),
            action_refs: Vec::new(),
        }
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_children(mut self, children: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_style(mut self, style: AuiStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn with_image(mut self, asset_id: impl Into<String>) -> Self {
        self.image = Some(AuiAssetRef::new(asset_id));
        self
    }

    pub fn with_interactable(mut self, consume_input: bool) -> Self {
        self.interactable = true;
        self.consume_input = consume_input;
        self
    }

    pub fn with_draggable(mut self) -> Self {
        self.draggable = true;
        self.interactable = true;
        self
    }

    pub fn with_drop_target(mut self) -> Self {
        self.drop_target = true;
        self.interactable = true;
        self
    }

    pub fn with_clip_policy(mut self, clip_policy: AuiClipPolicy) -> Self {
        self.clip_policy = clip_policy;
        self
    }

    pub fn with_scrollbar_policy(mut self, scrollbar_policy: AuiScrollbarPolicy) -> Self {
        self.scrollbar_policy = scrollbar_policy;
        self
    }

    pub fn with_navigation(mut self, navigation: AuiNavigationRef) -> Self {
        self.navigation = navigation;
        self
    }

    pub fn with_progress_value(mut self, progress_value: f32) -> Self {
        self.progress_value = Some(progress_value.clamp(0.0, 1.0));
        self
    }

    pub fn with_binding(mut self, binding: AuiBindingRef) -> Self {
        self.binding_refs.push(binding);
        self
    }

    pub fn with_action(mut self, action: AuiActionRef) -> Self {
        self.action_refs.push(action);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiDocument {
    pub schema_version: String,
    pub document_id: String,
    #[serde(default)]
    pub interaction_feedback: Option<AuiInteractionFeedbackRegistry>,
    pub canvases: Vec<AuiCanvas>,
    pub nodes: Vec<AuiNode>,
}

impl AuiDocument {
    pub fn new(
        document_id: impl Into<String>,
        canvases: Vec<AuiCanvas>,
        nodes: Vec<AuiNode>,
    ) -> Self {
        Self {
            schema_version: AUI_DOCUMENT_SCHEMA_VERSION.to_string(),
            document_id: document_id.into(),
            interaction_feedback: None,
            canvases,
            nodes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiAssetManifest {
    pub schema_version: String,
    pub manifest_id: String,
    pub assets: Vec<AuiAssetManifestEntry>,
}

impl AuiAssetManifest {
    pub fn new(manifest_id: impl Into<String>, assets: Vec<AuiAssetManifestEntry>) -> Self {
        Self {
            schema_version: AUI_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            manifest_id: manifest_id.into(),
            assets,
        }
    }

    pub fn asset_ids(&self) -> HashSet<&str> {
        self.assets
            .iter()
            .map(|asset| asset.asset_id.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiAssetManifestEntry {
    pub asset_id: String,
    pub asset_ref: String,
    pub used_by_nodes: Vec<String>,
    pub sprite_border: Option<[f32; 4]>,
    pub text_policy: AuiAssetTextPolicy,
}

impl AuiAssetManifestEntry {
    pub fn image(
        asset_id: impl Into<String>,
        asset_ref: impl Into<String>,
        used_by_nodes: Vec<String>,
    ) -> Self {
        Self {
            asset_id: asset_id.into(),
            asset_ref: asset_ref.into(),
            used_by_nodes,
            sprite_border: None,
            text_policy: AuiAssetTextPolicy::RuntimeText,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiAssetTextPolicy {
    RuntimeText,
    BitmapAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiValidationItem {
    pub severity: AuiValidationSeverity,
    pub code: String,
    pub node_id: Option<String>,
    pub asset_id: Option<String>,
    pub message: String,
    pub suggested_fix: Option<String>,
}

impl AuiValidationItem {
    pub fn error(
        code: impl Into<String>,
        node_id: Option<String>,
        asset_id: Option<String>,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiValidationSeverity::Error,
            code: code.into(),
            node_id,
            asset_id,
            message: message.into(),
            suggested_fix: Some(suggested_fix.into()),
        }
    }

    pub fn warning(
        code: impl Into<String>,
        node_id: Option<String>,
        asset_id: Option<String>,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiValidationSeverity::Warning,
            code: code.into(),
            node_id,
            asset_id,
            message: message.into(),
            suggested_fix: Some(suggested_fix.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiValidationReport {
    pub ok: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub missing_asset_count: usize,
    pub invalid_node_count: usize,
    pub text_baked_warning_count: usize,
    pub full_screen_image_rejected: bool,
    pub report_items: Vec<AuiValidationItem>,
}

impl AuiValidationReport {
    pub fn from_items(
        items: Vec<AuiValidationItem>,
        missing_asset_count: usize,
        full_screen_image_rejected: bool,
    ) -> Self {
        let error_count = items
            .iter()
            .filter(|item| item.severity == AuiValidationSeverity::Error)
            .count();
        let warning_count = items
            .iter()
            .filter(|item| item.severity == AuiValidationSeverity::Warning)
            .count();
        let invalid_node_count = items
            .iter()
            .filter(|item| item.node_id.is_some() && item.severity == AuiValidationSeverity::Error)
            .count();
        Self {
            ok: error_count == 0,
            error_count,
            warning_count,
            missing_asset_count,
            invalid_node_count,
            text_baked_warning_count: 0,
            full_screen_image_rejected,
            report_items: items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiBindingDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiBindingDiagnostic {
    pub severity: AuiBindingDiagnosticSeverity,
    pub code: String,
    pub node_id: String,
    pub binding_id: String,
    pub path: String,
    pub message: String,
}

impl AuiBindingDiagnostic {
    fn warning(
        code: impl Into<String>,
        node_id: impl Into<String>,
        binding: &AuiBindingRef,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiBindingDiagnosticSeverity::Warning,
            code: code.into(),
            node_id: node_id.into(),
            binding_id: binding.binding_id.clone(),
            path: binding.path.clone(),
            message: message.into(),
        }
    }

    fn error(
        code: impl Into<String>,
        node_id: impl Into<String>,
        binding: &AuiBindingRef,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiBindingDiagnosticSeverity::Error,
            code: code.into(),
            node_id: node_id.into(),
            binding_id: binding.binding_id.clone(),
            path: binding.path.clone(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiBindingReport {
    pub frame_index: u64,
    pub binding_count: usize,
    pub resolved_count: usize,
    pub fallback_count: usize,
    pub missing_binding_count: usize,
    pub type_mismatch_count: usize,
    pub diagnostics: Vec<AuiBindingDiagnostic>,
}

impl AuiBindingReport {
    pub fn ok(&self) -> bool {
        self.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != AuiBindingDiagnosticSeverity::Error)
    }
}

pub struct AuiRuntimeResolver;

impl AuiRuntimeResolver {
    pub fn resolve_bindings(
        document: &AuiDocument,
        snapshot: &ProjectUiStateSnapshot,
    ) -> (AuiDocument, AuiBindingReport) {
        let mut resolved = document.clone();
        let mut report = AuiBindingReport {
            frame_index: snapshot.frame_index,
            ..AuiBindingReport::default()
        };

        for node in &mut resolved.nodes {
            for binding in node.binding_refs.clone() {
                report.binding_count += 1;
                let value = if let Some(value) = snapshot.values.get(binding.path.as_str()) {
                    Some(value.clone())
                } else if let Some(fallback) = binding.fallback.clone() {
                    report.fallback_count += 1;
                    report.diagnostics.push(AuiBindingDiagnostic::warning(
                        "aui_binding.fallback_used",
                        node.node_id.clone(),
                        &binding,
                        format!(
                            "ProjectUiStateSnapshot has no value for '{}'; fallback was used.",
                            binding.path
                        ),
                    ));
                    Some(fallback)
                } else {
                    None
                };

                let Some(value) = value else {
                    report.missing_binding_count += 1;
                    report.diagnostics.push(AuiBindingDiagnostic::error(
                        "aui_binding.missing_path",
                        node.node_id.clone(),
                        &binding,
                        format!(
                            "ProjectUiStateSnapshot has no value for '{}'.",
                            binding.path
                        ),
                    ));
                    continue;
                };

                match apply_binding_value(node, &binding, value) {
                    Ok(()) => report.resolved_count += 1,
                    Err(message) => {
                        report.type_mismatch_count += 1;
                        report.diagnostics.push(AuiBindingDiagnostic::error(
                            "aui_binding.type_mismatch",
                            node.node_id.clone(),
                            &binding,
                            message,
                        ));
                    }
                }
            }
        }

        (resolved, report)
    }
}

fn apply_binding_value(
    node: &mut AuiNode,
    binding: &AuiBindingRef,
    value: AuiBindingValue,
) -> Result<(), String> {
    match (binding.target_field, value) {
        (AuiBindingTarget::TextText, AuiBindingValue::String(value))
        | (AuiBindingTarget::InputFieldText, AuiBindingValue::String(value)) => {
            node.text = Some(value);
            Ok(())
        }
        (AuiBindingTarget::ProgressBarValue, AuiBindingValue::Number(value)) => {
            node.progress_value = Some(value.clamp(0.0, 1.0));
            Ok(())
        }
        (AuiBindingTarget::PanelVisible, AuiBindingValue::Bool(value))
        | (AuiBindingTarget::ImageVisible, AuiBindingValue::Bool(value)) => {
            node.visible = value;
            Ok(())
        }
        (AuiBindingTarget::ImageAssetRef, AuiBindingValue::AssetRef(value)) => {
            node.image = Some(value);
            Ok(())
        }
        (target, value) => Err(format!(
            "Binding target {:?} cannot consume {}.",
            target,
            value.type_name()
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiComputedNode {
    pub canvas_id: String,
    pub composition_stage: AuiCompositionStage,
    pub node_id: String,
    pub kind: AuiNodeKind,
    pub rect: AuiComputedRect,
    pub effective_clip_rect: Option<AuiComputedRect>,
    pub clipped_by_node: Option<String>,
    pub tree_order: usize,
    pub local_visible: bool,
    pub effective_visible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiLayoutReport {
    pub frame: u64,
    pub canvas_count: usize,
    pub node_count: usize,
    pub visible_node_count: usize,
    pub clipped_node_count: usize,
    pub clip_root_count: usize,
    pub effective_clip_node_count: usize,
    pub overflow_count: usize,
    pub invalid_binding_count: usize,
    pub scroll_offset_applied: bool,
    pub scroll_applied_node_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiLayoutResult {
    pub computed_nodes: Vec<AuiComputedNode>,
    pub scrollbar_metrics: Vec<AuiScrollbarMetrics>,
    pub report: AuiLayoutReport,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiPointer {
    pub x: f32,
    pub y: f32,
}

impl AuiPointer {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiInteractionEventKind {
    PointerDown,
    PointerUp,
    PointerMove,
    PointerCancel,
    PointerLeave,
    MouseWheel,
    KeyDown,
    KeyUp,
    KeyHeld,
    TextInput,
    ImePreedit,
    ImeCommit,
    ImeCancel,
    GamepadButtonDown,
    GamepadButtonUp,
    GamepadButtonHeld,
    GamepadAxis2d,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiHitTestReason {
    HitInteractable,
    HitNonInteractable,
    OutsideUi,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiHitTestResult {
    pub pointer: AuiPointer,
    pub hit_node: Option<String>,
    pub consumed: bool,
    pub reason: AuiHitTestReason,
    pub clip_rejected_count: usize,
}

impl AuiHitTestResult {
    fn outside(pointer: AuiPointer) -> Self {
        Self {
            pointer,
            hit_node: None,
            consumed: false,
            reason: AuiHitTestReason::OutsideUi,
            clip_rejected_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiCommandKind {
    PointerDown,
    PointerUp,
    PointerMove,
    PointerCancel,
    Hover,
    Click,
    DragStart,
    DragMove,
    Drop,
    DragCancel,
    Focus,
    Blur,
    Submit,
    Cancel,
    Scroll,
    TextChanged,
    TextSubmitted,
    TextCancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiCommand {
    pub command_id: String,
    pub source_node: String,
    pub command_kind: AuiCommandKind,
    pub payload: Option<String>,
}

impl AuiCommand {
    fn new(index: usize, source_node: impl Into<String>, command_kind: AuiCommandKind) -> Self {
        Self {
            command_id: format!("aui-command-{}", index + 1),
            source_node: source_node.into(),
            command_kind,
            payload: None,
        }
    }

    fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AuiInteractionConfig {
    pub drag_threshold_px: f32,
    pub modal_blocks_pointer_outside: bool,
    pub modal_blocks_wheel_outside: bool,
    pub modal_blocks_keyboard: bool,
    pub wheel_scroll_px_per_delta: f32,
    pub drag_scroll_threshold_px: f32,
}

impl Default for AuiInteractionConfig {
    fn default() -> Self {
        Self {
            drag_threshold_px: 4.0,
            modal_blocks_pointer_outside: true,
            modal_blocks_wheel_outside: true,
            modal_blocks_keyboard: true,
            wheel_scroll_px_per_delta: 48.0,
            drag_scroll_threshold_px: 4.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AuiPrimaryPressCapture {
    node_id: String,
    pointer_id: u64,
    device_kind: RuntimePointerDeviceKind,
    hover_capable: bool,
    inside: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiInteractionState {
    active_drag: Option<AuiActiveDrag>,
    #[serde(default)]
    primary_press: Option<AuiPrimaryPressCapture>,
    #[serde(default)]
    hovered_node: Option<String>,
    #[serde(default)]
    interaction_session_id: Option<String>,
    #[serde(default)]
    pending_control_reconciliation_count: usize,
    pub focus: AuiFocusState,
    pub active_modal_root: Option<String>,
    pub scroll_offsets: BTreeMap<String, AuiScrollState>,
    pub input_mode: AuiInputMode,
    pub screen_stack: AuiScreenStackState,
    pub input_field: Option<AuiInputFieldState>,
    pub canvas_visibility_overrides: BTreeMap<String, bool>,
    active_scroll_capture: Option<AuiActiveScrollCapture>,
}

impl AuiInteractionState {
    pub fn active_drag_source(&self) -> Option<&str> {
        self.active_drag
            .as_ref()
            .map(|drag| drag.source_node.as_str())
    }

    pub fn active_scroll_capture(&self) -> Option<&str> {
        self.active_scroll_capture
            .as_ref()
            .map(|capture| capture.node_id.as_str())
    }

    #[cfg(test)]
    fn pressed_node(&self) -> Option<&str> {
        self.primary_press
            .as_ref()
            .map(|capture| capture.node_id.as_str())
    }

    fn clear_control_transients(&mut self) -> usize {
        [
            self.hovered_node.take().is_some(),
            self.primary_press.take().is_some(),
            self.active_drag.take().is_some(),
            self.active_scroll_capture.take().is_some(),
        ]
        .into_iter()
        .map(usize::from)
        .sum()
    }

    fn queue_control_reconciliation(&mut self) {
        self.pending_control_reconciliation_count += self.clear_control_transients();
    }

    fn drain_control_reconciliation_count(&mut self) -> usize {
        std::mem::take(&mut self.pending_control_reconciliation_count)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiInputMode {
    #[default]
    Navigation,
    TextEditing {
        node_id: String,
    },
    ModalBlocking {
        modal_root: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiUiIntentKind {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    FocusNext,
    FocusPrevious,
    Submit,
    Cancel,
    TextInput,
    TextCompositionStart,
    TextCompositionUpdate,
    TextCompositionCommit,
    TextCompositionCancel,
    TextEditCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiTextEditCommand {
    MoveCaretLeft,
    MoveCaretRight,
    MoveCaretHome,
    MoveCaretEnd,
    Backspace,
    Delete,
    SelectLeft,
    SelectRight,
    SelectAll,
}

impl AuiInputMode {
    pub fn label(&self) -> String {
        match self {
            Self::Navigation => "Navigation".to_string(),
            Self::TextEditing { node_id } => format!("TextEditing:{node_id}"),
            Self::ModalBlocking { modal_root } => format!("ModalBlocking:{modal_root}"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiFocusState {
    pub focused_node: Option<String>,
    pub focus_scope_root: Option<String>,
    pub focus_reason: AuiFocusReason,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiScreenStackState {
    pub active_stack: Vec<AuiScreenStackEntry>,
    pub last_popped_screen_id: Option<String>,
    pub focus_restore_count: usize,
    #[serde(default)]
    pub push_count: usize,
    #[serde(default)]
    pub default_focus_applied_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiScreenStackEntry {
    pub screen_id: String,
    pub document_path: Option<String>,
    pub canvas_id: String,
    pub root_node_id: String,
    pub default_focus_node_id: Option<String>,
    pub previous_focus_node_id: Option<String>,
    pub modal: bool,
    pub can_cancel: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiInputFieldState {
    pub node_id: String,
    pub original_text: String,
    pub draft_text: String,
    pub caret_index: usize,
    pub selection_anchor: usize,
    pub selection_focus: usize,
    pub composition: Option<AuiTextCompositionState>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTextCompositionState {
    pub preedit_text: String,
    pub cursor_start: usize,
    pub cursor_end: usize,
    pub active: bool,
}

impl AuiInputFieldState {
    fn start(node: &AuiNode) -> Self {
        let text = node.text.clone().unwrap_or_default();
        let caret_index = text.chars().count();
        Self {
            node_id: node.node_id.clone(),
            original_text: text.clone(),
            draft_text: text,
            caret_index,
            selection_anchor: caret_index,
            selection_focus: caret_index,
            composition: None,
            dirty: false,
        }
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = self.caret_index;
        self.selection_focus = self.caret_index;
    }

    fn selected_range(&self) -> Option<(usize, usize)> {
        if self.selection_anchor == self.selection_focus {
            return None;
        }
        Some((
            self.selection_anchor.min(self.selection_focus),
            self.selection_anchor.max(self.selection_focus),
        ))
    }

    fn replace_range(&mut self, start: usize, end: usize, insert: &str, max_length: Option<usize>) {
        let mut chars = self.draft_text.chars().collect::<Vec<_>>();
        let start = start.min(chars.len());
        let end = end.min(chars.len()).max(start);
        chars.splice(start..end, insert.chars());
        if let Some(max_length) = max_length {
            chars.truncate(max_length);
        }
        self.draft_text = chars.into_iter().collect();
        self.caret_index = (start + insert.chars().count()).min(self.draft_text.chars().count());
        self.clear_selection();
        self.dirty = true;
    }

    fn insert_text(&mut self, text: &str, max_length: Option<usize>) -> bool {
        let previous = self.draft_text.clone();
        if let Some((start, end)) = self.selected_range() {
            self.replace_range(start, end, text, max_length);
        } else {
            self.replace_range(self.caret_index, self.caret_index, text, max_length);
        }
        previous != self.draft_text
    }

    fn backspace(&mut self) -> bool {
        let previous = self.draft_text.clone();
        if let Some((start, end)) = self.selected_range() {
            self.replace_range(start, end, "", None);
        } else if self.caret_index > 0 {
            self.replace_range(self.caret_index - 1, self.caret_index, "", None);
        }
        previous != self.draft_text
    }

    fn delete(&mut self) -> bool {
        let previous = self.draft_text.clone();
        let len = self.draft_text.chars().count();
        if let Some((start, end)) = self.selected_range() {
            self.replace_range(start, end, "", None);
        } else if self.caret_index < len {
            self.replace_range(self.caret_index, self.caret_index + 1, "", None);
        }
        previous != self.draft_text
    }

    fn move_caret(&mut self, command: AuiTextEditCommand, extend_selection: bool) -> bool {
        let previous = (
            self.caret_index,
            self.selection_anchor,
            self.selection_focus,
        );
        let len = self.draft_text.chars().count();
        match command {
            AuiTextEditCommand::MoveCaretLeft | AuiTextEditCommand::SelectLeft => {
                self.caret_index = self.caret_index.saturating_sub(1);
            }
            AuiTextEditCommand::MoveCaretRight | AuiTextEditCommand::SelectRight => {
                self.caret_index = (self.caret_index + 1).min(len);
            }
            AuiTextEditCommand::MoveCaretHome => {
                self.caret_index = 0;
            }
            AuiTextEditCommand::MoveCaretEnd => {
                self.caret_index = len;
            }
            AuiTextEditCommand::SelectAll => {
                self.selection_anchor = 0;
                self.selection_focus = len;
                self.caret_index = len;
                return previous
                    != (
                        self.caret_index,
                        self.selection_anchor,
                        self.selection_focus,
                    );
            }
            AuiTextEditCommand::Backspace | AuiTextEditCommand::Delete => {}
        }
        if extend_selection {
            self.selection_focus = self.caret_index;
        } else {
            self.clear_selection();
        }
        previous
            != (
                self.caret_index,
                self.selection_anchor,
                self.selection_focus,
            )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiFocusReason {
    #[default]
    Cleared,
    Pointer,
    Keyboard,
    ModalOpen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiScrollState {
    pub node_id: String,
    pub offset_y: f32,
    pub max_offset_y: f32,
    pub last_delta_y: f32,
}

impl AuiScrollState {
    fn new(node_id: impl Into<String>, max_offset_y: f32) -> Self {
        Self {
            node_id: node_id.into(),
            offset_y: 0.0,
            max_offset_y,
            last_delta_y: 0.0,
        }
    }

    fn apply_delta(&mut self, delta_y: f32) -> bool {
        self.last_delta_y = delta_y;
        let previous = self.offset_y;
        self.offset_y = (self.offset_y + delta_y).clamp(0.0, self.max_offset_y.max(0.0));
        (self.offset_y - previous).abs() > f32::EPSILON
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiScrollbarMetrics {
    pub scroll_node_id: String,
    pub axis: AuiScrollbarAxis,
    pub track_rect: AuiComputedRect,
    pub thumb_rect: AuiComputedRect,
    pub offset_y: f32,
    pub max_offset_y: f32,
    pub viewport_height: f32,
    pub content_height: f32,
    pub visible: bool,
}

impl AuiScrollbarMetrics {
    pub fn thumb_node_id(&self) -> String {
        format!("{}:scrollbar-thumb", self.scroll_node_id)
    }

    pub fn track_node_id(&self) -> String {
        format!("{}:scrollbar-track", self.scroll_node_id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AuiActiveScrollCapture {
    node_id: String,
    captured_node_id: String,
    start_pointer: AuiPointer,
    last_pointer: AuiPointer,
    started: bool,
    scroll_delta_per_pointer_delta_y: Option<f32>,
    pointer_id: u64,
    device_kind: RuntimePointerDeviceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuiNavigationDirection {
    Next,
    Previous,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AuiActiveDrag {
    source_node: String,
    start_pointer: AuiPointer,
    current_pointer: AuiPointer,
    started: bool,
    pointer_id: u64,
    device_kind: RuntimePointerDeviceKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiDragDropPayload {
    pub schema_version: String,
    pub source_node: String,
    pub target_node: Option<String>,
    pub start_pointer: AuiPointer,
    pub current_pointer: AuiPointer,
    pub delta: AuiPointer,
    pub drag_phase: String,
}

impl AuiDragDropPayload {
    fn new(
        source_node: impl Into<String>,
        target_node: Option<String>,
        start_pointer: AuiPointer,
        current_pointer: AuiPointer,
        drag_phase: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "aui-drag-drop-payload.v1".to_string(),
            source_node: source_node.into(),
            target_node,
            start_pointer,
            current_pointer,
            delta: AuiPointer::new(
                current_pointer.x - start_pointer.x,
                current_pointer.y - start_pointer.y,
            ),
            drag_phase: drag_phase.into(),
        }
    }

    fn to_payload_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiScrollPayload {
    pub schema_version: String,
    pub node_id: String,
    pub offset_y: f32,
    pub max_offset_y: f32,
    pub delta_y: f32,
    pub input_kind: String,
}

impl AuiScrollPayload {
    fn new(
        node_id: impl Into<String>,
        offset_y: f32,
        max_offset_y: f32,
        delta_y: f32,
        input_kind: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "aui-scroll-payload.v1".to_string(),
            node_id: node_id.into(),
            offset_y,
            max_offset_y,
            delta_y,
            input_kind: input_kind.into(),
        }
    }

    fn to_payload_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiInteractionTrace {
    pub frame: u64,
    pub event_index: usize,
    pub event_kind: AuiInteractionEventKind,
    pub pointer: AuiPointer,
    pub hit_node: Option<String>,
    pub captured_node: Option<String>,
    pub drop_target: Option<String>,
    pub consumed: bool,
    pub reason: AuiHitTestReason,
    pub command_count: usize,
    pub action_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiControlInteractionSnapshot {
    pub frame_id: u64,
    pub session_id: Option<String>,
    pub hovered_node: Option<String>,
    pub pressed_node: Option<String>,
    pub pressed_inside: bool,
    pub pointer_id: Option<u64>,
    pub pointer_device_kind: Option<RuntimePointerDeviceKind>,
    pub pointer_hover_capable: bool,
    pub focused_node: Option<String>,
    pub focus_visible: bool,
    pub active_modal_root: Option<String>,
    pub active_screen_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiInteractionResult {
    pub consumed: bool,
    pub consumed_event_indices: Vec<usize>,
    pub consumed_event_count_by_kind: BTreeMap<String, usize>,
    pub commands: Vec<AuiCommand>,
    pub actions: Vec<AuiAction>,
    pub traces: Vec<AuiInteractionTrace>,
    pub active_modal_root: Option<String>,
    pub focus_change_count: usize,
    pub scroll_offset_change_count: usize,
    pub hit_test_clip_rejected_count: usize,
    pub keyboard_navigation_event_count: usize,
    pub focus_visible_scroll_count: usize,
    pub input_mode_before: String,
    pub input_mode_after: String,
    pub normalized_ui_intent_count: usize,
    pub keyboard_intent_count: usize,
    pub gamepad_intent_count: usize,
    pub submit_count: usize,
    pub cancel_count: usize,
    pub screen_stack_push_count: usize,
    pub screen_stack_pop_count: usize,
    pub active_screen_id: Option<String>,
    pub default_focus_applied_count: usize,
    pub focus_restore_count: usize,
    pub text_edit_session_count: usize,
    pub text_changed_count: usize,
    pub text_submitted_count: usize,
    pub text_cancelled_count: usize,
    pub caret_move_count: usize,
    pub selection_change_count: usize,
    pub ime_preedit_count: usize,
    pub ime_commit_count: usize,
    pub ime_cancel_count: usize,
    pub action_prompt_reported: bool,
    pub ime_platform_coverage: String,
    pub focusable_derived_from_interactable: bool,
    pub visibility_reconciliation_count: usize,
    pub control_reconciliation_count: usize,
    pub pointer_cancel_count: usize,
    pub control_snapshot: AuiControlInteractionSnapshot,
}

pub const AUI_INTERACTION_PRODUCTIZATION_REPORT_SCHEMA_VERSION: &str =
    "aui-interaction-productization-report.v1";

pub const AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION: &str =
    "aui-runtime-navigation-screenflow-textentry-productization-report.v1";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiInteractionProductizationReport {
    pub schema_version: String,
    pub frame: u64,
    pub document_id: String,
    pub drag_threshold_px: f32,
    pub wheel_scroll_px_per_delta: f32,
    pub drag_scroll_threshold_px: f32,
    pub snapshot_frame_lag: u64,
    pub authoring_action_payload_deferred: bool,
    pub modal_input_blocking_deferred: bool,
    pub editor_hit_test_deferred_to_209: bool,
    pub control_style_deferred: bool,
    pub slider_toggle_binding_target_deferred: bool,
    pub modal_blocking_status: String,
    pub focus_trap_status: String,
    pub scroll_status: String,
    pub input_event_count: usize,
    pub filtered_input_event_count: usize,
    pub consumed_pointer_event_count: usize,
    pub consumed_wheel_event_count: usize,
    pub consumed_keyboard_event_count: usize,
    pub consumed_event_count_by_kind: BTreeMap<String, usize>,
    pub command_count: usize,
    pub action_count: usize,
    pub click_action_count: usize,
    pub focus_action_count: usize,
    pub blur_action_count: usize,
    pub cancel_action_count: usize,
    pub scroll_action_count: usize,
    pub drag_start_count: usize,
    pub drag_move_count: usize,
    pub drop_count: usize,
    pub drag_cancel_count: usize,
    pub active_drag_source: Option<String>,
    pub active_modal_root: Option<String>,
    pub focused_node: Option<String>,
    pub focus_scope_root: Option<String>,
    pub focus_change_count: usize,
    pub scroll_offset_change_count: usize,
    pub scroll_offset_applied: bool,
    pub scroll_applied_node_count: usize,
    pub clipped_node_count: usize,
    pub hit_test_clip_rejected_count: usize,
    pub keyboard_navigation_event_count: usize,
    pub focus_visible_scroll_count: usize,
    pub traces: Vec<AuiInteractionTrace>,
    pub diagnostics: Vec<String>,
}

impl AuiInteractionProductizationReport {
    pub fn from_result(
        document: &AuiDocument,
        input_event_count: usize,
        filtered_input_event_count: usize,
        result: &AuiInteractionResult,
        config: AuiInteractionConfig,
        active_drag_source: Option<String>,
    ) -> Self {
        let click_action_count = result
            .actions
            .iter()
            .filter(|action| action.event == AuiActionEvent::Click)
            .count();
        let drag_start_count = result
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::DragStart)
            .count();
        let drag_move_count = result
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::DragMove)
            .count();
        let drop_count = result
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::Drop)
            .count();
        let drag_cancel_count = result
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::DragCancel)
            .count();
        let focus_action_count = result
            .actions
            .iter()
            .filter(|action| action.event == AuiActionEvent::Focus)
            .count();
        let blur_action_count = result
            .actions
            .iter()
            .filter(|action| action.event == AuiActionEvent::Blur)
            .count();
        let cancel_action_count = result
            .actions
            .iter()
            .filter(|action| action.event == AuiActionEvent::Cancel)
            .count();
        let scroll_action_count = result
            .actions
            .iter()
            .filter(|action| action.event == AuiActionEvent::Scroll)
            .count();
        let consumed_pointer_event_count = result
            .consumed_event_count_by_kind
            .iter()
            .filter(|(kind, _)| kind.starts_with("Pointer"))
            .map(|(_, count)| *count)
            .sum();
        let consumed_wheel_event_count = result
            .consumed_event_count_by_kind
            .get("MouseWheel")
            .copied()
            .unwrap_or_default();
        let consumed_keyboard_event_count = result
            .consumed_event_count_by_kind
            .iter()
            .filter(|(kind, _)| kind.starts_with("Key"))
            .map(|(_, count)| *count)
            .sum();
        let mut diagnostics = Vec::new();
        if result.consumed && input_event_count == filtered_input_event_count {
            diagnostics.push("aui_input.consumed_events_not_filtered".to_string());
        }
        if result.commands.iter().any(|command| {
            command.command_kind == AuiCommandKind::Drop && command.payload.is_none()
        }) {
            diagnostics.push("aui_drag.drop_without_payload".to_string());
        }
        Self {
            schema_version: AUI_INTERACTION_PRODUCTIZATION_REPORT_SCHEMA_VERSION.to_string(),
            frame: result
                .traces
                .first()
                .map(|trace| trace.frame)
                .unwrap_or_default(),
            document_id: document.document_id.clone(),
            drag_threshold_px: config.drag_threshold_px,
            wheel_scroll_px_per_delta: config.wheel_scroll_px_per_delta,
            drag_scroll_threshold_px: config.drag_scroll_threshold_px,
            snapshot_frame_lag: 1,
            authoring_action_payload_deferred: true,
            modal_input_blocking_deferred: false,
            editor_hit_test_deferred_to_209: false,
            control_style_deferred: false,
            slider_toggle_binding_target_deferred: true,
            modal_blocking_status: if result.active_modal_root.is_some() {
                "active".to_string()
            } else {
                "inactive".to_string()
            },
            focus_trap_status: if result.focus_change_count > 0 {
                "changed".to_string()
            } else {
                "stable".to_string()
            },
            scroll_status: if result.scroll_offset_change_count > 0 {
                "changed".to_string()
            } else {
                "stable".to_string()
            },
            input_event_count,
            filtered_input_event_count,
            consumed_pointer_event_count,
            consumed_wheel_event_count,
            consumed_keyboard_event_count,
            consumed_event_count_by_kind: result.consumed_event_count_by_kind.clone(),
            command_count: result.commands.len(),
            action_count: result.actions.len(),
            click_action_count,
            focus_action_count,
            blur_action_count,
            cancel_action_count,
            scroll_action_count,
            drag_start_count,
            drag_move_count,
            drop_count,
            drag_cancel_count,
            active_drag_source,
            active_modal_root: result.active_modal_root.clone(),
            focused_node: None,
            focus_scope_root: None,
            focus_change_count: result.focus_change_count,
            scroll_offset_change_count: result.scroll_offset_change_count,
            scroll_offset_applied: false,
            scroll_applied_node_count: 0,
            clipped_node_count: 0,
            hit_test_clip_rejected_count: result.hit_test_clip_rejected_count,
            keyboard_navigation_event_count: result.keyboard_navigation_event_count,
            focus_visible_scroll_count: result.focus_visible_scroll_count,
            traces: result.traces.clone(),
            diagnostics,
        }
    }

    pub fn with_focus_state(mut self, focus: &AuiFocusState) -> Self {
        self.focused_node = focus.focused_node.clone();
        self.focus_scope_root = focus.focus_scope_root.clone();
        self
    }

    pub fn with_layout_report(mut self, layout_report: &AuiLayoutReport) -> Self {
        self.scroll_offset_applied = layout_report.scroll_offset_applied;
        self.scroll_applied_node_count = layout_report.scroll_applied_node_count;
        self.clipped_node_count = layout_report.clipped_node_count;
        if self.scroll_offset_applied {
            self.scroll_status = "layout_applied".to_string();
        }
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiRuntimeNavigationScreenFlowTextEntryProductizationReport {
    pub schema_version: String,
    pub status: String,
    pub document_id: String,
    pub input_mode_before: String,
    pub input_mode_after: String,
    pub normalized_ui_intent_count: usize,
    pub keyboard_intent_count: usize,
    pub gamepad_intent_count: usize,
    pub submit_count: usize,
    pub cancel_count: usize,
    pub screen_stack_push_count: usize,
    pub screen_stack_pop_count: usize,
    pub active_screen_id: Option<String>,
    pub default_focus_applied_count: usize,
    pub focus_restore_count: usize,
    pub text_edit_session_count: usize,
    pub text_changed_count: usize,
    pub text_submitted_count: usize,
    pub text_cancelled_count: usize,
    pub caret_move_count: usize,
    pub selection_change_count: usize,
    pub ime_preedit_count: usize,
    pub ime_commit_count: usize,
    pub ime_cancel_count: usize,
    pub ime_platform_coverage: String,
    pub consumed_event_count_by_kind: BTreeMap<String, usize>,
    pub gameplay_input_filtered_count: usize,
    pub action_prompt_reported: bool,
    pub focusable_derived_from_interactable: bool,
    pub rich_text_deferred: bool,
    pub ime_candidate_window_deferred: bool,
    pub accessibility_deferred: bool,
    pub screen_transition_animation_deferred: bool,
    pub clipboard_full_deferred: bool,
    pub multi_line_text_edit_deferred: bool,
    pub common_ui_action_bar_deferred: bool,
    pub dirty_cache_batch_deferred: bool,
    pub touch_virtual_keyboard_deferred: bool,
    pub multi_user_input_deferred: bool,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl AuiRuntimeNavigationScreenFlowTextEntryProductizationReport {
    pub fn from_result(
        document: &AuiDocument,
        input_event_count: usize,
        filtered_input_event_count: usize,
        result: &AuiInteractionResult,
    ) -> Self {
        let gameplay_input_filtered_count =
            input_event_count.saturating_sub(filtered_input_event_count);
        let mut diagnostics = Vec::new();
        if result.submit_count > 0
            && !result
                .commands
                .iter()
                .any(|command| matches!(command.command_kind, AuiCommandKind::Submit))
        {
            diagnostics.push("aui_navigation.submit_count_without_command".to_string());
        }
        if result.text_changed_count > 0 && result.text_edit_session_count == 0 {
            diagnostics.push("aui_text_entry.changed_without_session".to_string());
        }
        let status = if diagnostics.is_empty() {
            "passed"
        } else {
            "partial"
        };
        Self {
            schema_version: AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION
                .to_string(),
            status: status.to_string(),
            document_id: document.document_id.clone(),
            input_mode_before: result.input_mode_before.clone(),
            input_mode_after: result.input_mode_after.clone(),
            normalized_ui_intent_count: result.normalized_ui_intent_count,
            keyboard_intent_count: result.keyboard_intent_count,
            gamepad_intent_count: result.gamepad_intent_count,
            submit_count: result.submit_count,
            cancel_count: result.cancel_count,
            screen_stack_push_count: result.screen_stack_push_count,
            screen_stack_pop_count: result.screen_stack_pop_count,
            active_screen_id: result.active_screen_id.clone(),
            default_focus_applied_count: result.default_focus_applied_count,
            focus_restore_count: result.focus_restore_count,
            text_edit_session_count: result.text_edit_session_count,
            text_changed_count: result.text_changed_count,
            text_submitted_count: result.text_submitted_count,
            text_cancelled_count: result.text_cancelled_count,
            caret_move_count: result.caret_move_count,
            selection_change_count: result.selection_change_count,
            ime_preedit_count: result.ime_preedit_count,
            ime_commit_count: result.ime_commit_count,
            ime_cancel_count: result.ime_cancel_count,
            ime_platform_coverage: result.ime_platform_coverage.clone(),
            consumed_event_count_by_kind: result.consumed_event_count_by_kind.clone(),
            gameplay_input_filtered_count,
            action_prompt_reported: result.action_prompt_reported,
            focusable_derived_from_interactable: result.focusable_derived_from_interactable,
            rich_text_deferred: true,
            ime_candidate_window_deferred: true,
            accessibility_deferred: true,
            screen_transition_animation_deferred: true,
            clipboard_full_deferred: true,
            multi_line_text_edit_deferred: true,
            common_ui_action_bar_deferred: true,
            dirty_cache_batch_deferred: true,
            touch_virtual_keyboard_deferred: true,
            multi_user_input_deferred: true,
            diagnostics,
            next_actions: vec![
                "Keep IME candidate window and full multiline editing deferred.".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuiDrawCommand {
    DrawRect {
        node_id: String,
        rect: AuiComputedRect,
        effective_clip_rect: Option<AuiComputedRect>,
        color: Option<String>,
    },
    DrawImage {
        node_id: String,
        rect: AuiComputedRect,
        effective_clip_rect: Option<AuiComputedRect>,
        asset_id: String,
        color: Option<String>,
    },
    DrawText {
        node_id: String,
        rect: AuiComputedRect,
        effective_clip_rect: Option<AuiComputedRect>,
        text: String,
        color: Option<String>,
        font_size: Option<f32>,
        font: Option<AuiFontStyle>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiDrawList {
    pub commands: Vec<AuiDrawCommand>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiRenderReport {
    pub draw_command_count: usize,
    pub text_count: usize,
    pub image_count: usize,
    pub effective_clip_item_count: usize,
    pub culled_draw_item_count: usize,
    pub scrollbar_visible_count: usize,
    pub batch_hint_count: usize,
}

impl AuiRenderReport {
    pub fn projection_summary(&self) -> ProjectionReport {
        ProjectionReport::new(
            ProjectionKind::Ui,
            ProjectionDomain::Ui,
            ProjectionDomain::Render,
            "UiProjectionAdapter<AuiDrawList>",
        )
        .with_counts(self.draw_command_count, 0, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiOverlayItemKind {
    Rect,
    Image,
    Text,
    ScrollbarTrack,
    ScrollbarThumb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuiOverlaySortKey {
    pub canvas_layer: i32,
    pub canvas_sorting_order: i32,
    pub tree_order: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiOverlayDrawItem {
    pub item_id: String,
    pub canvas_id: String,
    pub composition_stage: AuiCompositionStage,
    pub node_id: String,
    pub item_kind: AuiOverlayItemKind,
    pub rect: AuiComputedRect,
    pub effective_clip_rect: Option<AuiComputedRect>,
    pub color: Option<String>,
    pub asset_id: Option<String>,
    pub text: Option<String>,
    pub font_size: Option<f32>,
    pub font: Option<AuiFontStyle>,
    pub sort_key: AuiOverlaySortKey,
}

pub const AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION: &str =
    "aui-rectclip-scrollbar-navigation-productization-report.v1";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiRectClipScrollbarNavigationProductizationReport {
    pub schema_version: String,
    pub status: String,
    pub clip_root_count: usize,
    pub effective_clip_item_count: usize,
    pub culled_draw_item_count: usize,
    pub hit_test_clip_rejected_count: usize,
    pub scrollbar_visible_count: usize,
    pub scrollbar_thumb_drag_count: usize,
    pub scrollbar_offset_change_count: usize,
    pub keyboard_navigation_event_count: usize,
    pub focus_move_count: usize,
    pub focus_visible_scroll_count: usize,
    pub focused_node_before: Option<String>,
    pub focused_node_after: Option<String>,
    pub stencil_mask_deferred: bool,
    pub nested_scroll_deferred: bool,
    pub inertia_elastic_deferred: bool,
    pub virtualized_list_deferred: bool,
    pub input_field_ime_deferred: bool,
    pub full_gamepad_navigation_deferred: bool,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl AuiRectClipScrollbarNavigationProductizationReport {
    pub fn from_parts(
        layout_report: &AuiLayoutReport,
        render_report: &AuiRenderReport,
        interaction: &AuiInteractionResult,
        focused_node_before: Option<String>,
        focused_node_after: Option<String>,
    ) -> Self {
        let passed = layout_report.clip_root_count > 0
            && render_report.effective_clip_item_count > 0
            && interaction.hit_test_clip_rejected_count > 0;
        Self {
            schema_version: AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION.to_string(),
            status: if passed { "passed" } else { "partial" }.to_string(),
            clip_root_count: layout_report.clip_root_count,
            effective_clip_item_count: render_report.effective_clip_item_count,
            culled_draw_item_count: render_report.culled_draw_item_count,
            hit_test_clip_rejected_count: interaction.hit_test_clip_rejected_count,
            scrollbar_visible_count: render_report.scrollbar_visible_count,
            scrollbar_thumb_drag_count: interaction
                .traces
                .iter()
                .filter(|trace| {
                    trace
                        .captured_node
                        .as_deref()
                        .is_some_and(|node| node.ends_with(":scrollbar-thumb"))
                })
                .count(),
            scrollbar_offset_change_count: interaction.scroll_offset_change_count,
            keyboard_navigation_event_count: interaction.keyboard_navigation_event_count,
            focus_move_count: interaction.focus_change_count,
            focus_visible_scroll_count: interaction.focus_visible_scroll_count,
            focused_node_before,
            focused_node_after,
            stencil_mask_deferred: true,
            nested_scroll_deferred: true,
            inertia_elastic_deferred: true,
            virtualized_list_deferred: true,
            input_field_ime_deferred: true,
            full_gamepad_navigation_deferred: true,
            diagnostics: Vec::new(),
            next_actions: vec![
                "stencil_mask_deferred".to_string(),
                "nested_scroll_deferred".to_string(),
                "input_field_ime_deferred".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiTextGlyphQuad {
    pub item_id: String,
    pub node_id: String,
    pub codepoint: u32,
    pub glyph_id: String,
    pub rect: AuiComputedRect,
    pub uv_rect: [f32; 4],
    pub page_index: u32,
    pub render_mode: FontBundleRenderMode,
    pub clipped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiTextGlyphPlan {
    pub font_atlas_id: String,
    pub font_source_kind: String,
    pub font_asset_id: String,
    pub font_asset_status: String,
    pub fallback_used: bool,
    pub requested_glyph_count: usize,
    pub rendered_glyph_count: usize,
    pub unsupported_glyph_count: usize,
    pub clipped_glyph_count: usize,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub atlas_generation: u64,
    pub glyph_plan_hash: String,
    pub quads: Vec<AuiTextGlyphQuad>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiOverlayFrame {
    pub frame_index: u64,
    pub draw_items: Vec<AuiOverlayDrawItem>,
    pub report: AuiRenderReport,
    pub glyph_plan: Option<AuiTextGlyphPlan>,
}

pub const AUI_COMPOSITION_REPORT_SCHEMA_VERSION: &str = "aui-composition-report.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiCompositionStageFrame {
    pub stage: AuiCompositionStage,
    pub draw_items: Vec<AuiOverlayDrawItem>,
    pub item_count: usize,
    pub text_count: usize,
    pub image_count: usize,
    pub glyph_count: usize,
    pub canvas_count: usize,
    pub layer_group_count: usize,
    pub debug_label: String,
}

impl AuiCompositionStageFrame {
    pub fn empty(stage: AuiCompositionStage) -> Self {
        Self {
            stage,
            draw_items: Vec::new(),
            item_count: 0,
            text_count: 0,
            image_count: 0,
            glyph_count: 0,
            canvas_count: 0,
            layer_group_count: 0,
            debug_label: stage.debug_label().to_string(),
        }
    }

    fn from_items(stage: AuiCompositionStage, draw_items: Vec<AuiOverlayDrawItem>) -> Self {
        let text_count = draw_items
            .iter()
            .filter(|item| item.item_kind == AuiOverlayItemKind::Text)
            .count();
        let image_count = draw_items
            .iter()
            .filter(|item| item.item_kind == AuiOverlayItemKind::Image)
            .count();
        let canvas_count = draw_items
            .iter()
            .map(|item| item.canvas_id.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        Self {
            stage,
            item_count: draw_items.len(),
            text_count,
            image_count,
            glyph_count: 0,
            canvas_count,
            layer_group_count: 0,
            debug_label: stage.debug_label().to_string(),
            draw_items,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.draw_items.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiCompositionReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub stage_count: usize,
    pub before_world_item_count: usize,
    pub screen_overlay_item_count: usize,
    pub modal_item_count: usize,
    pub unsupported_stage_count: usize,
    pub rejected_node_interleave_count: usize,
    pub glyph_present: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiCompositionFrame {
    pub frame_index: u64,
    pub stages: Vec<AuiCompositionStageFrame>,
    pub report: AuiCompositionReport,
    pub glyph_plan: Option<AuiTextGlyphPlan>,
    #[serde(default)]
    pub canvas_references: Vec<CanvasReferenceFact>,
}

impl AuiCompositionFrame {
    pub fn from_overlay_frame(overlay: &AuiOverlayFrame) -> Self {
        let stage = AuiCompositionStageFrame::from_items(
            AuiCompositionStage::ScreenOverlay,
            overlay.draw_items.clone(),
        );
        let mut frame =
            Self::from_stage_frames(overlay.frame_index, vec![stage], overlay.glyph_plan.clone());
        frame.canvas_references = vec![CanvasReferenceFact::new("legacy-overlay", 1280, 720)];
        frame
    }

    pub fn to_overlay_frame(&self) -> AuiOverlayFrame {
        let mut draw_items = self
            .stages
            .iter()
            .flat_map(|stage| stage.draw_items.iter().cloned())
            .collect::<Vec<_>>();
        draw_items.sort_by_key(|item| item.sort_key);
        let text_count = draw_items
            .iter()
            .filter(|item| item.item_kind == AuiOverlayItemKind::Text)
            .count();
        let image_count = draw_items
            .iter()
            .filter(|item| item.item_kind == AuiOverlayItemKind::Image)
            .count();
        AuiOverlayFrame {
            frame_index: self.frame_index,
            report: AuiRenderReport {
                draw_command_count: draw_items.len(),
                text_count,
                image_count,
                effective_clip_item_count: draw_items
                    .iter()
                    .filter(|item| item.effective_clip_rect.is_some())
                    .count(),
                culled_draw_item_count: 0,
                scrollbar_visible_count: draw_items
                    .iter()
                    .filter(|item| {
                        matches!(
                            item.item_kind,
                            AuiOverlayItemKind::ScrollbarTrack | AuiOverlayItemKind::ScrollbarThumb
                        )
                    })
                    .count()
                    / 2,
                batch_hint_count: 0,
            },
            draw_items,
            glyph_plan: self.glyph_plan.clone(),
        }
    }

    pub fn stage(&self, stage: AuiCompositionStage) -> Option<&AuiCompositionStageFrame> {
        self.stages.iter().find(|frame| frame.stage == stage)
    }

    pub fn stage_or_empty(&self, stage: AuiCompositionStage) -> AuiCompositionStageFrame {
        self.stage(stage)
            .cloned()
            .unwrap_or_else(|| AuiCompositionStageFrame::empty(stage))
    }

    fn from_stage_frames(
        frame_index: u64,
        mut stages: Vec<AuiCompositionStageFrame>,
        glyph_plan: Option<AuiTextGlyphPlan>,
    ) -> Self {
        stages.sort_by_key(|stage| stage.stage);
        let before_world_item_count = stages
            .iter()
            .find(|stage| stage.stage == AuiCompositionStage::BeforeWorld)
            .map(|stage| stage.item_count)
            .unwrap_or_default();
        let screen_overlay_item_count = stages
            .iter()
            .find(|stage| stage.stage == AuiCompositionStage::ScreenOverlay)
            .map(|stage| stage.item_count)
            .unwrap_or_default();
        let modal_item_count = stages
            .iter()
            .find(|stage| stage.stage == AuiCompositionStage::Modal)
            .map(|stage| stage.item_count)
            .unwrap_or_default();
        let glyph_present = glyph_plan
            .as_ref()
            .is_some_and(|plan| plan.rendered_glyph_count > 0);
        Self {
            frame_index,
            report: AuiCompositionReport {
                schema_version: AUI_COMPOSITION_REPORT_SCHEMA_VERSION.to_string(),
                frame_index,
                stage_count: stages.iter().filter(|stage| !stage.is_empty()).count(),
                before_world_item_count,
                screen_overlay_item_count,
                modal_item_count,
                unsupported_stage_count: 0,
                rejected_node_interleave_count: 0,
                glyph_present,
                diagnostics: Vec::new(),
            },
            stages,
            glyph_plan,
            canvas_references: Vec::new(),
        }
    }
}

pub struct AuiRendererBridge;

impl AuiRendererBridge {
    pub fn build_overlay_frame(frame_index: u64, draw_list: &AuiDrawList) -> AuiOverlayFrame {
        let mut draw_items = Vec::new();
        let mut text_count = 0;
        let mut image_count = 0;

        for (tree_order, command) in draw_list.commands.iter().enumerate() {
            match command {
                AuiDrawCommand::DrawRect {
                    node_id,
                    rect,
                    effective_clip_rect,
                    color,
                } => draw_items.push(AuiOverlayDrawItem {
                    item_id: format!("aui-item-{}", tree_order + 1),
                    canvas_id: "legacy-overlay".to_string(),
                    composition_stage: AuiCompositionStage::ScreenOverlay,
                    node_id: node_id.clone(),
                    item_kind: overlay_rect_item_kind(node_id),
                    rect: *rect,
                    effective_clip_rect: *effective_clip_rect,
                    color: color.clone(),
                    asset_id: None,
                    text: None,
                    font_size: None,
                    font: None,
                    sort_key: AuiOverlaySortKey {
                        canvas_layer: 0,
                        canvas_sorting_order: 0,
                        tree_order,
                    },
                }),
                AuiDrawCommand::DrawImage {
                    node_id,
                    rect,
                    effective_clip_rect,
                    asset_id,
                    color,
                } => {
                    image_count += 1;
                    draw_items.push(AuiOverlayDrawItem {
                        item_id: format!("aui-item-{}", tree_order + 1),
                        canvas_id: "legacy-overlay".to_string(),
                        composition_stage: AuiCompositionStage::ScreenOverlay,
                        node_id: node_id.clone(),
                        item_kind: AuiOverlayItemKind::Image,
                        rect: *rect,
                        effective_clip_rect: *effective_clip_rect,
                        color: color.clone(),
                        asset_id: Some(asset_id.clone()),
                        text: None,
                        font_size: None,
                        font: None,
                        sort_key: AuiOverlaySortKey {
                            canvas_layer: 0,
                            canvas_sorting_order: 0,
                            tree_order,
                        },
                    });
                }
                AuiDrawCommand::DrawText {
                    node_id,
                    rect,
                    effective_clip_rect,
                    text,
                    color,
                    font_size,
                    font,
                } => {
                    text_count += 1;
                    draw_items.push(AuiOverlayDrawItem {
                        item_id: format!("aui-item-{}", tree_order + 1),
                        canvas_id: "legacy-overlay".to_string(),
                        composition_stage: AuiCompositionStage::ScreenOverlay,
                        node_id: node_id.clone(),
                        item_kind: AuiOverlayItemKind::Text,
                        rect: *rect,
                        effective_clip_rect: *effective_clip_rect,
                        color: color.clone(),
                        asset_id: None,
                        text: Some(text.clone()),
                        font_size: *font_size,
                        font: font.clone(),
                        sort_key: AuiOverlaySortKey {
                            canvas_layer: 0,
                            canvas_sorting_order: 0,
                            tree_order,
                        },
                    });
                }
            }
        }

        AuiOverlayFrame {
            frame_index,
            report: AuiRenderReport {
                draw_command_count: draw_items.len(),
                text_count,
                image_count,
                effective_clip_item_count: draw_items
                    .iter()
                    .filter(|item| item.effective_clip_rect.is_some())
                    .count(),
                culled_draw_item_count: 0,
                scrollbar_visible_count: draw_items
                    .iter()
                    .filter(|item| {
                        matches!(
                            item.item_kind,
                            AuiOverlayItemKind::ScrollbarTrack | AuiOverlayItemKind::ScrollbarThumb
                        )
                    })
                    .count()
                    / 2,
                batch_hint_count: 0,
            },
            draw_items,
            glyph_plan: None,
        }
    }

    pub fn build_composition_frame(
        frame_index: u64,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        draw_list: &AuiDrawList,
    ) -> AuiCompositionFrame {
        let canvases_by_id = document
            .canvases
            .iter()
            .map(|canvas| (canvas.canvas_id.as_str(), canvas))
            .collect::<HashMap<_, _>>();
        let computed_by_node = layout
            .computed_nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let mut stage_items = AuiCompositionStage::ordered()
            .into_iter()
            .map(|stage| (stage, Vec::<AuiOverlayDrawItem>::new()))
            .collect::<HashMap<_, _>>();
        let mut diagnostics = document
            .canvases
            .iter()
            .filter(|canvas| canvas.mode != AuiCanvasMode::ScreenOverlay)
            .map(|canvas| {
                format!(
                    "aui_canvas_render_space_deferred:{}:{:?}",
                    canvas.canvas_id, canvas.mode
                )
            })
            .collect::<Vec<_>>();

        for (command_index, command) in draw_list.commands.iter().enumerate() {
            let command_node_id = draw_command_node_id(command);
            let source_node_id = draw_command_source_node_id(command_node_id);
            let Some(computed) = computed_by_node.get(source_node_id) else {
                diagnostics.push(format!(
                    "aui_draw_command_missing_computed_node:{}",
                    command_node_id
                ));
                continue;
            };
            let Some(canvas) = canvases_by_id.get(computed.canvas_id.as_str()) else {
                diagnostics.push(format!(
                    "aui_draw_command_missing_canvas:{}:{}",
                    computed.canvas_id, command_node_id
                ));
                continue;
            };
            let stage = computed.composition_stage;
            let sort_key = AuiOverlaySortKey {
                canvas_layer: canvas.layer,
                canvas_sorting_order: canvas.sorting_order,
                tree_order: computed.tree_order.saturating_mul(1000) + command_index,
            };
            let item = overlay_item_from_draw_command(
                command,
                command_index,
                canvas.canvas_id.as_str(),
                stage,
                sort_key,
            );
            if let Some(items) = stage_items.get_mut(&stage) {
                items.push(item);
            }
        }

        let mut stages = Vec::new();
        for stage in AuiCompositionStage::ordered() {
            let mut draw_items = stage_items.remove(&stage).unwrap_or_default();
            draw_items.sort_by_key(|item| item.sort_key);
            if !draw_items.is_empty() {
                stages.push(AuiCompositionStageFrame::from_items(stage, draw_items));
            }
        }

        let mut frame = AuiCompositionFrame::from_stage_frames(frame_index, stages, None);
        frame.canvas_references = document
            .canvases
            .iter()
            .map(|canvas| {
                CanvasReferenceFact::new(
                    canvas.canvas_id.clone(),
                    canvas.reference_resolution.x.round().max(0.0) as u32,
                    canvas.reference_resolution.y.round().max(0.0) as u32,
                )
            })
            .collect();
        frame.report.unsupported_stage_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.starts_with("aui_canvas_render_space_deferred:"))
            .count();
        frame.report.diagnostics = diagnostics;
        frame
    }
}

fn draw_command_node_id(command: &AuiDrawCommand) -> &str {
    match command {
        AuiDrawCommand::DrawRect { node_id, .. }
        | AuiDrawCommand::DrawImage { node_id, .. }
        | AuiDrawCommand::DrawText { node_id, .. } => node_id.as_str(),
    }
}

fn draw_command_source_node_id(node_id: &str) -> &str {
    node_id.split_once(':').map_or(node_id, |(base, _)| base)
}

fn draw_command_effective_clip_rect(command: &AuiDrawCommand) -> Option<AuiComputedRect> {
    match command {
        AuiDrawCommand::DrawRect {
            effective_clip_rect,
            ..
        }
        | AuiDrawCommand::DrawImage {
            effective_clip_rect,
            ..
        }
        | AuiDrawCommand::DrawText {
            effective_clip_rect,
            ..
        } => *effective_clip_rect,
    }
}

fn draw_command_count_for_node(node: &AuiNode) -> usize {
    match node.kind {
        AuiNodeKind::Panel => 1,
        AuiNodeKind::Button => 1 + usize::from(node.text.is_some()),
        AuiNodeKind::ProgressBar => 2,
        AuiNodeKind::Image => usize::from(node.image.is_some()),
        AuiNodeKind::Text => usize::from(node.text.is_some()),
        _ => 0,
    }
}

fn aui_visual_override_owner<'a>(
    nodes_by_id: &HashMap<&str, &AuiNode>,
    node_id: &str,
    overrides: &'a crate::aui_control_feedback::AuiVisualOverrideSet,
) -> Option<(
    String,
    &'a crate::aui_control_feedback::AuiControlVisualOverride,
)> {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        if let Some(visual) = overrides.get(current_id) {
            return Some((current_id.to_string(), visual));
        }
        current = nodes_by_id
            .get(current_id)
            .and_then(|node| node.parent.as_deref());
    }
    None
}

fn aui_feedback_transform_rect(
    rect: AuiComputedRect,
    owner_rect: AuiComputedRect,
    visual: crate::aui_control_feedback::AuiControlVisualOverride,
) -> AuiComputedRect {
    let center_x = owner_rect.x + owner_rect.width * 0.5;
    let center_y = owner_rect.y + owner_rect.height * 0.5;
    AuiComputedRect {
        x: center_x + (rect.x - center_x) * visual.scale + visual.translation.x,
        y: center_y + (rect.y - center_y) * visual.scale + visual.translation.y,
        width: rect.width * visual.scale,
        height: rect.height * visual.scale,
    }
}

fn aui_feedback_transform_color(
    color: Option<String>,
    visual: crate::aui_control_feedback::AuiControlVisualOverride,
) -> Option<String> {
    let color = color?;
    if (visual.brightness_multiplier - 1.0).abs() < f32::EPSILON
        && (visual.opacity_multiplier - 1.0).abs() < f32::EPSILON
    {
        return Some(color);
    }
    let raw = color.strip_prefix('#')?;
    if raw.len() != 6 && raw.len() != 8 {
        return Some(color);
    }
    let parse = |start| u8::from_str_radix(&raw[start..start + 2], 16).ok();
    let scale = |component: u8, multiplier: f32| {
        (f32::from(component) * multiplier)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    let red = scale(parse(0)?, visual.brightness_multiplier);
    let green = scale(parse(2)?, visual.brightness_multiplier);
    let blue = scale(parse(4)?, visual.brightness_multiplier);
    let authored_alpha = if raw.len() == 8 { parse(6)? } else { 255 };
    let alpha = scale(authored_alpha, visual.opacity_multiplier);
    if alpha == 255 && raw.len() == 6 {
        Some(format!("#{red:02x}{green:02x}{blue:02x}"))
    } else {
        Some(format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}"))
    }
}

fn overlay_rect_item_kind(node_id: &str) -> AuiOverlayItemKind {
    if node_id.ends_with(":scrollbar-track") {
        AuiOverlayItemKind::ScrollbarTrack
    } else if node_id.ends_with(":scrollbar-thumb") {
        AuiOverlayItemKind::ScrollbarThumb
    } else {
        AuiOverlayItemKind::Rect
    }
}

fn overlay_item_from_draw_command(
    command: &AuiDrawCommand,
    command_index: usize,
    canvas_id: &str,
    composition_stage: AuiCompositionStage,
    sort_key: AuiOverlaySortKey,
) -> AuiOverlayDrawItem {
    match command {
        AuiDrawCommand::DrawRect {
            node_id,
            rect,
            effective_clip_rect,
            color,
        } => AuiOverlayDrawItem {
            item_id: format!(
                "aui-{}-item-{}",
                composition_stage.pass_id_suffix(),
                command_index + 1
            ),
            canvas_id: canvas_id.to_string(),
            composition_stage,
            node_id: node_id.clone(),
            item_kind: overlay_rect_item_kind(node_id),
            rect: *rect,
            effective_clip_rect: *effective_clip_rect,
            color: color.clone(),
            asset_id: None,
            text: None,
            font_size: None,
            font: None,
            sort_key,
        },
        AuiDrawCommand::DrawImage {
            node_id,
            rect,
            effective_clip_rect,
            asset_id,
            color,
        } => AuiOverlayDrawItem {
            item_id: format!(
                "aui-{}-item-{}",
                composition_stage.pass_id_suffix(),
                command_index + 1
            ),
            canvas_id: canvas_id.to_string(),
            composition_stage,
            node_id: node_id.clone(),
            item_kind: AuiOverlayItemKind::Image,
            rect: *rect,
            effective_clip_rect: *effective_clip_rect,
            color: color.clone(),
            asset_id: Some(asset_id.clone()),
            text: None,
            font_size: None,
            font: None,
            sort_key,
        },
        AuiDrawCommand::DrawText {
            node_id,
            rect,
            effective_clip_rect,
            text,
            color,
            font_size,
            font,
        } => AuiOverlayDrawItem {
            item_id: format!(
                "aui-{}-item-{}",
                composition_stage.pass_id_suffix(),
                command_index + 1
            ),
            canvas_id: canvas_id.to_string(),
            composition_stage,
            node_id: node_id.clone(),
            item_kind: AuiOverlayItemKind::Text,
            rect: *rect,
            effective_clip_rect: *effective_clip_rect,
            color: color.clone(),
            asset_id: None,
            text: Some(text.clone()),
            font_size: *font_size,
            font: font.clone(),
            sort_key,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuiSnapshotSource {
    EmptyDefaultSnapshot,
    PackageSmokeSnapshot,
    ProjectProducer,
    TestSnapshot,
    ProjectRuleSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuiRuntimePresentStatus {
    Success,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiRuntimePresentDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiRuntimePresentDiagnostic {
    pub severity: AuiRuntimePresentDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl AuiRuntimePresentDiagnostic {
    fn warning(code: impl Into<String>, message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            severity: AuiRuntimePresentDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path,
        }
    }

    fn error(code: impl Into<String>, message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            severity: AuiRuntimePresentDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiRuntimePresentReport {
    pub schema_version: String,
    pub status: AuiRuntimePresentStatus,
    pub frame_index: u64,
    pub document_id: String,
    pub snapshot_source: AuiSnapshotSource,
    pub snapshot_value_count: usize,
    pub binding_status: String,
    pub layout_status: String,
    pub draw_item_count: usize,
    pub text_command_count: usize,
    pub image_command_count: usize,
    pub glyph_present: bool,
    pub font_atlas_present: bool,
    pub font_atlas_id: Option<String>,
    pub font_source_kind: Option<String>,
    pub font_asset_id: Option<String>,
    pub font_asset_status: Option<String>,
    pub font_fallback_used: bool,
    pub requested_glyph_count: usize,
    pub rendered_glyph_count: usize,
    pub unsupported_glyph_count: usize,
    pub clipped_glyph_count: usize,
    pub glyph_atlas_width: Option<u32>,
    pub glyph_atlas_height: Option<u32>,
    pub glyph_atlas_generation: Option<u64>,
    pub text_pass_inserted: bool,
    pub glyph_plan_hash: Option<String>,
    pub ui_pass_inserted: bool,
    pub ui_composition_stage_count: usize,
    pub ui_before_world_item_count: usize,
    pub ui_screen_overlay_item_count: usize,
    pub ui_modal_item_count: usize,
    pub modal_rendering_only: bool,
    pub ui_state_snapshot_report: Option<ProjectUiStateSnapshotReport>,
    pub diagnostics: Vec<AuiRuntimePresentDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiRuntimePresentOutput {
    pub overlay: AuiOverlayFrame,
    pub composition: AuiCompositionFrame,
    pub resolved_document: AuiDocument,
    pub layout: AuiLayoutResult,
    pub report: AuiRuntimePresentReport,
}

pub struct AuiRuntimePresenter;

impl AuiRuntimePresenter {
    pub fn package_smoke_snapshot(frame_index: u64) -> ProjectUiStateSnapshot {
        ProjectUiStateSnapshot::package_smoke_snapshot(frame_index)
    }

    pub fn present_package_smoke(
        document: &AuiDocument,
        frame_index: u64,
    ) -> AuiRuntimePresentOutput {
        let snapshot = Self::package_smoke_snapshot(frame_index);
        Self::present(document, AuiSnapshotSource::PackageSmokeSnapshot, &snapshot)
    }

    pub fn present(
        document: &AuiDocument,
        snapshot_source: AuiSnapshotSource,
        snapshot: &ProjectUiStateSnapshot,
    ) -> AuiRuntimePresentOutput {
        let snapshot_report = ProjectUiStateSnapshotReport::from_snapshot(
            "direct_snapshot",
            snapshot_source,
            snapshot,
        );
        Self::present_with_snapshot_output(
            document,
            ProjectUiStateSnapshotOutput {
                snapshot: snapshot.clone(),
                report: snapshot_report,
            },
        )
    }

    pub fn present_project_snapshot(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output(document, snapshot_output)
    }

    pub fn present_project_snapshot_with_font_atlases(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_font_atlases(document, snapshot_output, font_atlases)
    }

    pub fn present_project_snapshot_with_fonts(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
        )
    }

    pub fn present_project_snapshot_with_fonts_for_presentation(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        presentation: &ResolvedGameViewPresentation,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts_for_presentation(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            Some(presentation),
        )
    }

    pub fn present_with_snapshot_output(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
    ) -> AuiRuntimePresentOutput {
        let empty_font_atlases = RuntimeAuiFontAtlasRegistry::empty("aui-present-direct");
        Self::present_with_snapshot_output_and_font_atlases(
            document,
            snapshot_output,
            &empty_font_atlases,
        )
    }

    pub fn present_with_snapshot_output_and_font_atlases(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts(
            document,
            snapshot_output,
            font_atlases,
            &RuntimeFontBundleRegistry::default(),
        )
    }

    pub fn present_with_snapshot_output_and_fonts(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts_for_presentation(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            None,
        )
    }

    fn present_with_snapshot_output_and_fonts_for_presentation(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> AuiRuntimePresentOutput {
        let snapshot_source = snapshot_output.report.snapshot_source;
        let snapshot = snapshot_output.snapshot;
        let (resolved_document, binding_report) =
            AuiRuntimeResolver::resolve_bindings(document, &snapshot);
        let snapshot_report = snapshot_output
            .report
            .with_binding_report(document, &binding_report);
        let layout = AuiLayoutEngine::layout(&resolved_document, snapshot.frame_index);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&resolved_document, &layout);
        let mut composition = AuiRendererBridge::build_composition_frame(
            snapshot.frame_index,
            &resolved_document,
            &layout,
            &draw_list,
        );
        let mut overlay = composition.to_overlay_frame();
        let glyph_plan = if font_bundles.default_bundle().is_some() {
            build_text_glyph_plan_from_bundles_for_presentation(
                &overlay,
                font_bundles,
                presentation,
            )
        } else {
            build_text_glyph_plan(&overlay, font_atlases)
        };
        overlay.glyph_plan = glyph_plan.clone();
        composition.glyph_plan = glyph_plan.clone();
        composition.report.glyph_present = glyph_plan
            .as_ref()
            .is_some_and(|plan| plan.rendered_glyph_count > 0);
        let ui_pass_inserted = !overlay.draw_items.is_empty();
        let font_atlas_present =
            font_bundles.default_bundle().is_some() || font_atlases.default_atlas().is_some();
        let glyph_present = glyph_plan
            .as_ref()
            .is_some_and(|plan| plan.rendered_glyph_count > 0 && font_atlas_present);
        let text_pass_inserted = glyph_present;
        let mut diagnostics = binding_report
            .diagnostics
            .iter()
            .map(|diagnostic| AuiRuntimePresentDiagnostic {
                severity: match diagnostic.severity {
                    AuiBindingDiagnosticSeverity::Warning => {
                        AuiRuntimePresentDiagnosticSeverity::Warning
                    }
                    AuiBindingDiagnosticSeverity::Error => {
                        AuiRuntimePresentDiagnosticSeverity::Error
                    }
                },
                code: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                path: Some(format!(
                    "nodes.{}.bindingRefs.{}",
                    diagnostic.node_id, diagnostic.binding_id
                )),
            })
            .collect::<Vec<_>>();
        diagnostics.extend(snapshot_report.diagnostics.iter().map(|diagnostic| {
            AuiRuntimePresentDiagnostic {
                severity: match diagnostic.severity {
                    ProjectUiStateSnapshotDiagnosticSeverity::Warning => {
                        AuiRuntimePresentDiagnosticSeverity::Warning
                    }
                    ProjectUiStateSnapshotDiagnosticSeverity::Error => {
                        AuiRuntimePresentDiagnosticSeverity::Error
                    }
                },
                code: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                path: diagnostic.path.clone(),
            }
        }));
        if !ui_pass_inserted {
            diagnostics.push(AuiRuntimePresentDiagnostic::error(
                "aui_present.draw_list_empty",
                "AUI document produced no draw items for the overlay.",
                Some(document.document_id.clone()),
            ));
        }
        if overlay.report.text_count > 0 && !glyph_present {
            if font_atlas_present {
                diagnostics.push(AuiRuntimePresentDiagnostic::warning(
                    "aui_text.glyph_not_rendered",
                    "AUI text draw commands exist, but no glyph quads were produced from the loaded FontAtlas.",
                    Some(document.document_id.clone()),
                ));
            } else {
                diagnostics.push(AuiRuntimePresentDiagnostic::warning(
                    "aui_text.font_atlas_missing",
                    "AUI text draw commands exist, but RuntimePackage did not load a usable FontAtlas.",
                    Some(document.document_id.clone()),
                ));
            }
        }
        if let Some(plan) = &glyph_plan {
            if plan.unsupported_glyph_count > 0 {
                diagnostics.push(AuiRuntimePresentDiagnostic::warning(
                    "aui_text.unsupported_glyph_fallback",
                    format!(
                        "{} requested glyphs used the FontAtlas fallback glyph.",
                        plan.unsupported_glyph_count
                    ),
                    Some(document.document_id.clone()),
                ));
            }
            if plan.clipped_glyph_count > 0 {
                diagnostics.push(AuiRuntimePresentDiagnostic::warning(
                    "aui_text.glyph_clipped",
                    format!(
                        "{} glyph quads exceeded their text item rect.",
                        plan.clipped_glyph_count
                    ),
                    Some(document.document_id.clone()),
                ));
            }
        }

        let has_error = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AuiRuntimePresentDiagnosticSeverity::Error);
        let status = if has_error {
            AuiRuntimePresentStatus::Failed
        } else if overlay.report.text_count > 0 && !glyph_present {
            AuiRuntimePresentStatus::Partial
        } else {
            AuiRuntimePresentStatus::Success
        };

        let draw_item_count = overlay.draw_items.len();
        let text_command_count = overlay.report.text_count;
        let image_command_count = overlay.report.image_count;
        let font_atlas_id = glyph_plan.as_ref().map(|plan| plan.font_atlas_id.clone());
        let font_source_kind = glyph_plan
            .as_ref()
            .map(|plan| plan.font_source_kind.clone());
        let font_asset_id = glyph_plan.as_ref().map(|plan| plan.font_asset_id.clone());
        let font_asset_status = glyph_plan
            .as_ref()
            .map(|plan| plan.font_asset_status.clone());
        let font_fallback_used = glyph_plan.as_ref().is_some_and(|plan| plan.fallback_used);
        let requested_glyph_count = glyph_plan
            .as_ref()
            .map(|plan| plan.requested_glyph_count)
            .unwrap_or_default();
        let rendered_glyph_count = glyph_plan
            .as_ref()
            .map(|plan| plan.rendered_glyph_count)
            .unwrap_or_default();
        let unsupported_glyph_count = glyph_plan
            .as_ref()
            .map(|plan| plan.unsupported_glyph_count)
            .unwrap_or_default();
        let clipped_glyph_count = glyph_plan
            .as_ref()
            .map(|plan| plan.clipped_glyph_count)
            .unwrap_or_default();
        let glyph_atlas_width = glyph_plan.as_ref().map(|plan| plan.atlas_width);
        let glyph_atlas_height = glyph_plan.as_ref().map(|plan| plan.atlas_height);
        let glyph_atlas_generation = glyph_plan.as_ref().map(|plan| plan.atlas_generation);
        let glyph_plan_hash = glyph_plan.as_ref().map(|plan| plan.glyph_plan_hash.clone());
        let ui_composition_stage_count = composition.report.stage_count;
        let ui_before_world_item_count = composition.report.before_world_item_count;
        let ui_screen_overlay_item_count = composition.report.screen_overlay_item_count;
        let ui_modal_item_count = composition.report.modal_item_count;
        let modal_rendering_only = ui_modal_item_count > 0;
        let layout_status = if layout.computed_nodes.is_empty() {
            "empty".to_string()
        } else {
            "ok".to_string()
        };

        AuiRuntimePresentOutput {
            overlay,
            composition,
            resolved_document,
            layout,
            report: AuiRuntimePresentReport {
                schema_version: "aui-runtime-present-report.v1".to_string(),
                status,
                frame_index: snapshot.frame_index,
                document_id: document.document_id.clone(),
                snapshot_source,
                snapshot_value_count: snapshot.values.len(),
                binding_status: if binding_report.ok() {
                    "ok".to_string()
                } else {
                    "error".to_string()
                },
                layout_status,
                draw_item_count,
                text_command_count,
                image_command_count,
                glyph_present,
                font_atlas_present,
                font_atlas_id,
                font_source_kind,
                font_asset_id,
                font_asset_status,
                font_fallback_used,
                requested_glyph_count,
                rendered_glyph_count,
                unsupported_glyph_count,
                clipped_glyph_count,
                glyph_atlas_width,
                glyph_atlas_height,
                glyph_atlas_generation,
                text_pass_inserted,
                glyph_plan_hash,
                ui_pass_inserted,
                ui_composition_stage_count,
                ui_before_world_item_count,
                ui_screen_overlay_item_count,
                ui_modal_item_count,
                modal_rendering_only,
                ui_state_snapshot_report: Some(snapshot_report),
                diagnostics,
            },
        }
    }

    pub fn apply_control_feedback_with_fonts(
        output: &mut AuiRuntimePresentOutput,
        interaction: &AuiInteractionResult,
        state: &mut crate::aui_control_feedback::AuiControlFeedbackState,
        presentation_delta_us: u64,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> crate::aui_control_feedback::AuiControlFeedbackFrame {
        Self::apply_control_feedback_with_fonts_for_presentation(
            output,
            interaction,
            state,
            presentation_delta_us,
            font_atlases,
            font_bundles,
            None,
        )
    }

    pub fn apply_control_feedback_with_fonts_for_presentation(
        output: &mut AuiRuntimePresentOutput,
        interaction: &AuiInteractionResult,
        state: &mut crate::aui_control_feedback::AuiControlFeedbackState,
        presentation_delta_us: u64,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> crate::aui_control_feedback::AuiControlFeedbackFrame {
        let feedback = crate::aui_control_feedback::AuiControlFeedbackModule::advance(
            state,
            crate::aui_control_feedback::AuiControlFeedbackFrameInput {
                document: &output.resolved_document,
                interaction: &interaction.control_snapshot,
                commands: &interaction.commands,
                presentation_delta_us,
                diagnostics: crate::aui_control_feedback::AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list_with_visual_overrides(
            &output.resolved_document,
            &output.layout,
            &feedback.overrides,
        );
        let mut composition = AuiRendererBridge::build_composition_frame(
            output.report.frame_index,
            &output.resolved_document,
            &output.layout,
            &draw_list,
        );
        let mut overlay = composition.to_overlay_frame();
        let glyph_plan = if font_bundles.default_bundle().is_some() {
            build_text_glyph_plan_from_bundles_for_presentation(
                &overlay,
                font_bundles,
                presentation,
            )
        } else {
            build_text_glyph_plan(&overlay, font_atlases)
        };
        overlay.glyph_plan = glyph_plan.clone();
        composition.glyph_plan = glyph_plan.clone();
        composition.report.glyph_present = glyph_plan
            .as_ref()
            .is_some_and(|plan| plan.rendered_glyph_count > 0);
        output.report.draw_item_count = overlay.draw_items.len();
        output.report.text_command_count = overlay.report.text_count;
        output.report.image_command_count = overlay.report.image_count;
        output.report.ui_pass_inserted = !overlay.draw_items.is_empty();
        output.report.ui_composition_stage_count = composition.report.stage_count;
        output.report.ui_before_world_item_count = composition.report.before_world_item_count;
        output.report.ui_screen_overlay_item_count = composition.report.screen_overlay_item_count;
        output.report.ui_modal_item_count = composition.report.modal_item_count;
        output.overlay = overlay;
        output.composition = composition;
        feedback
    }
}

fn build_text_glyph_plan(
    overlay: &AuiOverlayFrame,
    font_atlases: &RuntimeAuiFontAtlasRegistry,
) -> Option<AuiTextGlyphPlan> {
    if overlay.report.text_count == 0 {
        return None;
    }
    let atlas = font_atlases.default_atlas()?;
    let metadata = &atlas.metadata;
    let mut quads = Vec::new();
    let mut requested_glyph_count = 0;
    let mut unsupported_glyph_count = 0;
    let mut clipped_glyph_count = 0;

    for item in overlay
        .draw_items
        .iter()
        .filter(|item| item.item_kind == AuiOverlayItemKind::Text)
    {
        let Some(text) = item.text.as_deref() else {
            continue;
        };
        let scale = item.font_size.unwrap_or(12.0).max(1.0) / 8.0;
        let mut cursor_x = item.rect.x;
        for ch in text.chars() {
            requested_glyph_count += 1;
            let exact_supported = metadata
                .glyphs
                .iter()
                .any(|glyph| glyph.codepoint == ch as u32);
            if !exact_supported {
                unsupported_glyph_count += 1;
            }
            let Some(glyph) = metadata.glyph(ch) else {
                continue;
            };
            let width = glyph.pixel_rect[2] as f32 * scale;
            let height = glyph.pixel_rect[3] as f32 * scale;
            let rect = AuiComputedRect {
                x: cursor_x + glyph.bearing_x * scale,
                y: item.rect.y,
                width,
                height,
            };
            let clipped = rect.x + rect.width > item.rect.x + item.rect.width
                || rect.y + rect.height > item.rect.y + item.rect.height;
            if clipped {
                clipped_glyph_count += 1;
            }
            quads.push(AuiTextGlyphQuad {
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint: ch as u32,
                glyph_id: glyph.glyph_id.clone(),
                rect,
                uv_rect: glyph.uv_rect,
                page_index: glyph.page_index,
                render_mode: FontBundleRenderMode::BitmapR8,
                clipped,
            });
            cursor_x += glyph.advance * scale;
        }
    }

    let glyph_plan_hash = stable_glyph_plan_hash(metadata.font_atlas_id.as_str(), &quads);
    Some(AuiTextGlyphPlan {
        font_atlas_id: metadata.font_atlas_id.clone(),
        font_source_kind: metadata.font_source_kind.clone(),
        font_asset_id: metadata.font_asset_id.clone(),
        font_asset_status: metadata.font_asset_status.clone(),
        fallback_used: metadata.fallback_used,
        requested_glyph_count,
        rendered_glyph_count: quads.len(),
        unsupported_glyph_count,
        clipped_glyph_count,
        atlas_width: metadata.atlas_width,
        atlas_height: metadata.atlas_height,
        atlas_generation: metadata.atlas_generation,
        glyph_plan_hash,
        quads,
    })
}

pub fn build_text_glyph_plan_from_bundles(
    overlay: &AuiOverlayFrame,
    font_bundles: &RuntimeFontBundleRegistry,
) -> Option<AuiTextGlyphPlan> {
    build_text_glyph_plan_from_bundles_for_presentation(overlay, font_bundles, None)
}

pub fn build_text_glyph_plan_from_bundles_for_presentation(
    overlay: &AuiOverlayFrame,
    font_bundles: &RuntimeFontBundleRegistry,
    presentation: Option<&ResolvedGameViewPresentation>,
) -> Option<AuiTextGlyphPlan> {
    if overlay.report.text_count == 0 {
        return None;
    }
    let registry = RuntimeFontRegistry::new(font_bundles);
    let default_bundle = font_bundles.default_bundle()?;
    let mut quads = Vec::new();
    let mut requested_glyph_count = 0;
    let mut unsupported_glyph_count = 0;
    let mut clipped_glyph_count = 0;
    let mut fallback_used = false;

    for item in overlay
        .draw_items
        .iter()
        .filter(|item| item.item_kind == AuiOverlayItemKind::Text)
    {
        let Some(text) = item.text.as_deref() else {
            continue;
        };
        let font_size = item.font_size.unwrap_or(12.0).max(1.0);
        let target_scale = presentation
            .and_then(|value| value.canvas_reference_to_target_scale(&item.canvas_id))
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(1.0);
        let physical_font_size = font_size * target_scale;
        let requested_pixel_size =
            physical_font_size.round().clamp(1.0, f32::from(u16::MAX)) as u16;
        let raster_policy = item
            .font
            .as_ref()
            .map(|font| font.raster_policy)
            .unwrap_or_default();
        let bundle_id = item
            .font
            .as_ref()
            .and_then(|font| font.font_bundle_id.clone());
        let mut cursor_x = item.rect.x;
        let mut previous = None;
        for ch in text.chars() {
            requested_glyph_count += 1;
            let font_family_id = item
                .font
                .as_ref()
                .and_then(|font| font.font_family_id.clone());
            let style = match item
                .font
                .as_ref()
                .map(|font| font.style)
                .unwrap_or_default()
            {
                AuiFontStyleKind::Normal => FontBundleStyle::Normal,
                AuiFontStyleKind::Italic => FontBundleStyle::Italic,
                AuiFontStyleKind::Oblique => FontBundleStyle::Oblique,
            };
            let weight = item.font.as_ref().map(|font| font.weight).unwrap_or(400);
            let resolve = |render_mode| {
                registry.resolve(RuntimeFontResolveRequest {
                    font_bundle_id: bundle_id.clone(),
                    font_family_id: font_family_id.clone(),
                    style,
                    weight,
                    codepoint: u32::from(ch),
                    render_mode,
                    pixel_size: requested_pixel_size,
                })
            };
            let resolved = match raster_policy {
                AuiFontRasterPolicy::Bitmap => resolve(FontBundleRenderMode::BitmapR8),
                AuiFontRasterPolicy::Msdf => resolve(FontBundleRenderMode::MsdfRgba8),
                AuiFontRasterPolicy::AutoHybrid if physical_font_size > 32.0 => {
                    resolve(FontBundleRenderMode::MsdfRgba8)
                }
                AuiFontRasterPolicy::AutoHybrid => {
                    let bitmap = resolve(FontBundleRenderMode::BitmapR8);
                    match bitmap {
                        Some(bitmap)
                            if (0.875..=1.125).contains(
                                &(physical_font_size / f32::from(bitmap.glyph.pixel_size.max(1))),
                            ) =>
                        {
                            Some(bitmap)
                        }
                        _ => resolve(FontBundleRenderMode::MsdfRgba8),
                    }
                }
            }
            .or_else(|| {
                ch.is_whitespace()
                    .then(|| resolve(FontBundleRenderMode::BitmapR8))
                    .flatten()
            });
            let Some(resolved) = resolved else {
                unsupported_glyph_count += 1;
                previous = None;
                continue;
            };
            if resolved.fallback_used {
                unsupported_glyph_count += 1;
                fallback_used = true;
            }
            if let Some(previous_glyph) = previous.as_ref() {
                let kerning =
                    registry.kerning(&resolved.font_bundle_id, previous_glyph, &resolved.glyph);
                cursor_x += kerning as f32 / 1_000_000.0 * font_size;
            }
            let page = font_bundles
                .bundles_by_id
                .get(&resolved.font_bundle_id)
                .and_then(|bundle| {
                    bundle
                        .metadata
                        .pages
                        .get(resolved.glyph.page_index as usize)
                })?;
            let scale = font_size / f32::from(resolved.glyph.pixel_size.max(1));
            let rect = AuiComputedRect {
                x: cursor_x,
                y: item.rect.y,
                width: resolved.glyph.pixel_rect[2] as f32 * scale,
                height: resolved.glyph.pixel_rect[3] as f32 * scale,
            };
            let clipped = rect.x + rect.width > item.rect.x + item.rect.width
                || rect.y + rect.height > item.rect.y + item.rect.height;
            if clipped {
                clipped_glyph_count += 1;
            }
            let [x, y, width, height] = resolved.glyph.pixel_rect;
            quads.push(AuiTextGlyphQuad {
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint: u32::from(ch),
                glyph_id: format!(
                    "{}:{}",
                    resolved.glyph.font_face_id, resolved.glyph.glyph_id
                ),
                rect,
                uv_rect: [
                    x as f32 / page.width as f32,
                    y as f32 / page.height as f32,
                    (x + width) as f32 / page.width as f32,
                    (y + height) as f32 / page.height as f32,
                ],
                page_index: resolved.glyph.page_index,
                render_mode: resolved.glyph.render_mode,
                clipped,
            });
            cursor_x += resolved.glyph.advance_per_em_millionths as f32 / 1_000_000.0 * font_size;
            previous = Some(resolved.glyph);
        }
    }
    let glyph_plan_hash =
        stable_glyph_plan_hash(default_bundle.metadata.font_bundle_id.as_str(), &quads);
    let first_page = default_bundle.metadata.pages.first()?;
    Some(AuiTextGlyphPlan {
        font_atlas_id: default_bundle.metadata.font_bundle_id.clone(),
        font_source_kind: "project_font_bundle_v2".to_string(),
        font_asset_id: default_bundle.metadata.font_stack_id.clone(),
        font_asset_status: "qualified".to_string(),
        fallback_used,
        requested_glyph_count,
        rendered_glyph_count: quads.len(),
        unsupported_glyph_count,
        clipped_glyph_count,
        atlas_width: first_page.width,
        atlas_height: first_page.height,
        atlas_generation: default_bundle.metadata.generation,
        glyph_plan_hash,
        quads,
    })
}

fn stable_glyph_plan_hash(font_atlas_id: &str, quads: &[AuiTextGlyphQuad]) -> String {
    let mut parts = vec![font_atlas_id.to_string()];
    for quad in quads {
        parts.push(format!(
            "{}:{}:{}:{:.3}:{:.3}:{:.3}:{:.3}:{:.6}:{:.6}:{:.6}:{:.6}:{}:{:?}:{}",
            quad.item_id,
            quad.node_id,
            quad.codepoint,
            quad.rect.x,
            quad.rect.y,
            quad.rect.width,
            quad.rect.height,
            quad.uv_rect[0],
            quad.uv_rect[1],
            quad.uv_rect[2],
            quad.uv_rect[3],
            quad.page_index,
            quad.render_mode,
            quad.clipped
        ));
    }
    stable_fnv1a_hash(&parts.join("|"))
}

fn stable_fnv1a_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub struct AuiInteractionSystem;

impl AuiInteractionSystem {
    fn topmost_modal_root(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        state: &AuiInteractionState,
    ) -> Option<String> {
        document
            .canvases
            .iter()
            .filter(|canvas| {
                canvas.mode == AuiCanvasMode::ScreenOverlay
                    && canvas.composition_stage == AuiCompositionStage::Modal
                    && Self::canvas_effective_visible(state, canvas)
                    && Self::node_effectively_visible(layout, canvas.root_node.as_str())
            })
            .max_by_key(|canvas| (canvas.layer, canvas.sorting_order))
            .map(|canvas| canvas.root_node.clone())
    }

    fn node_effectively_visible(layout: &AuiLayoutResult, node_id: &str) -> bool {
        layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == node_id)
            .is_some_and(|computed| computed.effective_visible)
    }

    fn reconcile_effectively_hidden_state(
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
    ) {
        let hidden = |node_id: &str| !Self::node_effectively_visible(layout, node_id);

        if state.focus.focused_node.as_deref().is_some_and(hidden) {
            state.focus.focused_node = None;
            state.focus.focus_reason = AuiFocusReason::Cleared;
            result.focus_change_count += 1;
            result.visibility_reconciliation_count += 1;
        }
        if state.focus.focus_scope_root.as_deref().is_some_and(hidden) {
            state.focus.focus_scope_root = None;
            result.visibility_reconciliation_count += 1;
        }
        if state
            .input_field
            .as_ref()
            .is_some_and(|input| hidden(input.node_id.as_str()))
        {
            if state
                .input_field
                .as_ref()
                .and_then(|input| input.composition.as_ref())
                .is_some()
            {
                result.ime_cancel_count += 1;
            }
            state.input_field = None;
            state.input_mode = AuiInputMode::Navigation;
            result.visibility_reconciliation_count += 1;
        }
        if matches!(
            &state.input_mode,
            AuiInputMode::ModalBlocking { modal_root } if hidden(modal_root)
        ) {
            state.input_mode = AuiInputMode::Navigation;
            result.visibility_reconciliation_count += 1;
        }
        if state.active_modal_root.as_deref().is_some_and(hidden) {
            state.active_modal_root = None;
            result.visibility_reconciliation_count += 1;
        }
    }

    fn node_in_subtree(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        root_node: &str,
        node_id: &str,
    ) -> bool {
        if root_node == node_id {
            return true;
        }
        let mut current = node_id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let Some(node) = nodes_by_id.get(current) else {
                return false;
            };
            let Some(parent) = node.parent.as_deref() else {
                return false;
            };
            if parent == root_node {
                return true;
            }
            current = parent;
        }
        false
    }

    fn reconcile_session_identity(
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
        session_id: Option<&str>,
    ) {
        let Some(session_id) = session_id else {
            return;
        };
        if state.interaction_session_id.as_deref() == Some(session_id) {
            return;
        }
        if state.interaction_session_id.is_some() {
            result.control_reconciliation_count += state.clear_control_transients();
        }
        state.interaction_session_id = Some(session_id.to_string());
    }

    fn control_node_status(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        layout: &AuiLayoutResult,
        node_id: &str,
        scope_root: Option<&str>,
        require_interactable: bool,
    ) -> (bool, bool) {
        let hidden = !Self::node_effectively_visible(layout, node_id);
        let valid = nodes_by_id.get(node_id).is_some_and(|node| {
            !hidden
                && (!require_interactable || node.interactable)
                && scope_root
                    .map(|root| Self::node_in_subtree(nodes_by_id, root, node_id))
                    .unwrap_or(true)
        });
        (valid, hidden)
    }

    fn reconcile_control_state(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
        active_modal_root: Option<&str>,
    ) {
        let active_screen_root = state
            .screen_stack
            .active_stack
            .last()
            .map(|entry| entry.root_node_id.as_str());
        let scope_root = active_modal_root.or(active_screen_root);

        if let Some(node_id) = state.hovered_node.as_deref() {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, true);
            if !valid {
                state.hovered_node = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
        if let Some(node_id) = state
            .primary_press
            .as_ref()
            .map(|press| press.node_id.as_str())
        {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, true);
            if !valid {
                state.primary_press = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
        if let Some(node_id) = state
            .active_drag
            .as_ref()
            .map(|drag| drag.source_node.as_str())
        {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, true);
            if !valid {
                state.active_drag = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
        if let Some(node_id) = state
            .active_scroll_capture
            .as_ref()
            .map(|capture| capture.node_id.as_str())
        {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, false);
            if !valid {
                state.active_scroll_capture = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
    }

    fn control_snapshot(
        frame_id: u64,
        state: &AuiInteractionState,
    ) -> AuiControlInteractionSnapshot {
        let primary_press = state.primary_press.as_ref();
        AuiControlInteractionSnapshot {
            frame_id,
            session_id: state.interaction_session_id.clone(),
            hovered_node: state.hovered_node.clone(),
            pressed_node: primary_press.map(|press| press.node_id.clone()),
            pressed_inside: primary_press.is_some_and(|press| press.inside),
            pointer_id: primary_press.map(|press| press.pointer_id),
            pointer_device_kind: primary_press.map(|press| press.device_kind),
            pointer_hover_capable: primary_press.is_some_and(|press| press.hover_capable),
            focused_node: state.focus.focused_node.clone(),
            focus_visible: state.focus.focused_node.is_some()
                && state.focus.focus_reason != AuiFocusReason::Pointer,
            active_modal_root: state.active_modal_root.clone(),
            active_screen_id: state
                .screen_stack
                .active_stack
                .last()
                .map(|entry| entry.screen_id.clone()),
        }
    }

    fn find_scroll_container(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        hit_node: Option<&str>,
    ) -> Option<String> {
        let mut current = hit_node?;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let node = nodes_by_id.get(current)?;
            if matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List) {
                return Some(node.node_id.clone());
            }
            current = node.parent.as_deref()?;
        }
        None
    }

    fn computed_rect_for<'a>(
        layout: &'a AuiLayoutResult,
        node_id: &str,
    ) -> Option<&'a AuiComputedRect> {
        layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == node_id)
            .map(|computed| &computed.rect)
    }

    fn estimate_scroll_max_offset_y(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        layout: &AuiLayoutResult,
        scroll_node: &str,
    ) -> f32 {
        let Some(viewport) = Self::computed_rect_for(layout, scroll_node) else {
            return 0.0;
        };
        let mut max_bottom = viewport.y + viewport.height;
        for computed in &layout.computed_nodes {
            if computed.node_id == scroll_node {
                continue;
            }
            if Self::node_in_subtree(nodes_by_id, scroll_node, computed.node_id.as_str()) {
                max_bottom = max_bottom.max(computed.rect.y + computed.rect.height);
            }
        }
        (max_bottom - (viewport.y + viewport.height)).max(0.0)
    }

    fn update_scroll_offset(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        scroll_node: &str,
        delta_y: f32,
    ) -> Option<AuiScrollPayload> {
        let max_offset_y = Self::estimate_scroll_max_offset_y(nodes_by_id, layout, scroll_node);
        let scroll_state = state
            .scroll_offsets
            .entry(scroll_node.to_string())
            .or_insert_with(|| AuiScrollState::new(scroll_node, max_offset_y));
        scroll_state.max_offset_y = max_offset_y;
        if !scroll_state.apply_delta(delta_y) {
            return None;
        }
        Some(AuiScrollPayload::new(
            scroll_node,
            scroll_state.offset_y,
            scroll_state.max_offset_y,
            delta_y,
            "runtime_input",
        ))
    }

    fn focusable_nodes_in_scope(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        nodes_by_id: &HashMap<&str, &AuiNode>,
        scope_root: &str,
    ) -> Vec<String> {
        layout
            .computed_nodes
            .iter()
            .filter(|computed| computed.effective_visible)
            .filter_map(|computed| {
                let node = nodes_by_id.get(computed.node_id.as_str())?;
                let focusable = node.focusable.unwrap_or(node.interactable);
                if focusable
                    && computed.effective_visible
                    && Self::node_in_subtree(nodes_by_id, scope_root, node.node_id.as_str())
                    && document
                        .nodes
                        .iter()
                        .any(|candidate| candidate.node_id == node.node_id)
                {
                    Some(node.node_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn focus_scope_root(
        document: &AuiDocument,
        nodes_by_id: &HashMap<&str, &AuiNode>,
        state: &AuiInteractionState,
    ) -> Option<String> {
        if let Some(scope_root) = &state.focus.focus_scope_root {
            return Some(scope_root.clone());
        }
        if let Some(focused_node) = &state.focus.focused_node {
            if let Some(canvas) = document.canvases.iter().find(|canvas| {
                Self::node_in_subtree(nodes_by_id, canvas.root_node.as_str(), focused_node)
            }) {
                return Some(canvas.root_node.clone());
            }
        }
        document
            .canvases
            .iter()
            .filter(|canvas| Self::canvas_effective_visible(state, canvas))
            .max_by_key(|canvas| (canvas.layer, canvas.sorting_order))
            .map(|canvas| canvas.root_node.clone())
    }

    fn apply_default_focus_if_needed(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        nodes_by_id: &HashMap<&str, &AuiNode>,
        state: &mut AuiInteractionState,
    ) -> bool {
        if state.focus.focused_node.is_some() {
            return false;
        }
        let Some(canvas) = document
            .canvases
            .iter()
            .filter(|canvas| Self::canvas_effective_visible(state, canvas))
            .filter(|canvas| canvas.default_focus_node_id.is_some())
            .max_by_key(|canvas| (canvas.layer, canvas.sorting_order))
        else {
            return false;
        };
        let Some(default_focus) = canvas.default_focus_node_id.as_deref() else {
            return false;
        };
        let focusable = Self::focusable_nodes_in_scope(
            document,
            layout,
            nodes_by_id,
            canvas.root_node.as_str(),
        );
        if !focusable.iter().any(|node| node == default_focus) {
            return false;
        }
        state.focus.focused_node = Some(default_focus.to_string());
        state.focus.focus_scope_root = Some(canvas.root_node.clone());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        state.screen_stack.default_focus_applied_count += 1;
        true
    }

    fn navigation_direction_for_key(key: &str, reverse: bool) -> Option<AuiNavigationDirection> {
        if key.eq_ignore_ascii_case("Tab") {
            return Some(if reverse {
                AuiNavigationDirection::Previous
            } else {
                AuiNavigationDirection::Next
            });
        }
        if key.eq_ignore_ascii_case("ArrowUp") || key.eq_ignore_ascii_case("Up") {
            return Some(AuiNavigationDirection::Up);
        }
        if key.eq_ignore_ascii_case("ArrowDown") || key.eq_ignore_ascii_case("Down") {
            return Some(AuiNavigationDirection::Down);
        }
        if key.eq_ignore_ascii_case("ArrowLeft") || key.eq_ignore_ascii_case("Left") {
            return Some(AuiNavigationDirection::Left);
        }
        if key.eq_ignore_ascii_case("ArrowRight") || key.eq_ignore_ascii_case("Right") {
            return Some(AuiNavigationDirection::Right);
        }
        None
    }

    fn explicit_navigation_target<'a>(
        node: &'a AuiNode,
        direction: AuiNavigationDirection,
    ) -> Option<&'a str> {
        match direction {
            AuiNavigationDirection::Next => node.navigation.next.as_deref(),
            AuiNavigationDirection::Previous => node.navigation.previous.as_deref(),
            AuiNavigationDirection::Up => node.navigation.up.as_deref(),
            AuiNavigationDirection::Down => node.navigation.down.as_deref(),
            AuiNavigationDirection::Left => node.navigation.left.as_deref(),
            AuiNavigationDirection::Right => node.navigation.right.as_deref(),
        }
    }

    fn navigation_mode_allows(mode: AuiNavigationMode, direction: AuiNavigationDirection) -> bool {
        match mode {
            AuiNavigationMode::None => matches!(
                direction,
                AuiNavigationDirection::Next | AuiNavigationDirection::Previous
            ),
            AuiNavigationMode::Auto | AuiNavigationMode::Explicit => true,
            AuiNavigationMode::Vertical => matches!(
                direction,
                AuiNavigationDirection::Next
                    | AuiNavigationDirection::Previous
                    | AuiNavigationDirection::Up
                    | AuiNavigationDirection::Down
            ),
            AuiNavigationMode::Horizontal => matches!(
                direction,
                AuiNavigationDirection::Next
                    | AuiNavigationDirection::Previous
                    | AuiNavigationDirection::Left
                    | AuiNavigationDirection::Right
            ),
        }
    }

    fn computed_node_for<'a>(
        layout: &'a AuiLayoutResult,
        node_id: &str,
    ) -> Option<&'a AuiComputedNode> {
        layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == node_id)
    }

    fn choose_directional_focus_candidate(
        layout: &AuiLayoutResult,
        focusable: &[String],
        current: &str,
        direction: AuiNavigationDirection,
    ) -> Option<String> {
        let current_computed = Self::computed_node_for(layout, current)?;
        let current_center_x = current_computed.rect.x + current_computed.rect.width * 0.5;
        let current_center_y = current_computed.rect.y + current_computed.rect.height * 0.5;
        focusable
            .iter()
            .filter(|candidate| candidate.as_str() != current)
            .filter_map(|candidate| {
                let computed = Self::computed_node_for(layout, candidate)?;
                let center_x = computed.rect.x + computed.rect.width * 0.5;
                let center_y = computed.rect.y + computed.rect.height * 0.5;
                let epsilon = 0.001;
                let (primary, secondary) = match direction {
                    AuiNavigationDirection::Down if center_y > current_center_y + epsilon => (
                        center_y - current_center_y,
                        (center_x - current_center_x).abs(),
                    ),
                    AuiNavigationDirection::Up if center_y < current_center_y - epsilon => (
                        current_center_y - center_y,
                        (center_x - current_center_x).abs(),
                    ),
                    AuiNavigationDirection::Right if center_x > current_center_x + epsilon => (
                        center_x - current_center_x,
                        (center_y - current_center_y).abs(),
                    ),
                    AuiNavigationDirection::Left if center_x < current_center_x - epsilon => (
                        current_center_x - center_x,
                        (center_y - current_center_y).abs(),
                    ),
                    _ => return None,
                };
                Some((candidate.clone(), primary, secondary, computed.tree_order))
            })
            .min_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
                    .then_with(|| a.3.cmp(&b.3))
            })
            .map(|(node_id, _, _, _)| node_id)
    }

    fn move_focus(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        nodes_by_id: &HashMap<&str, &AuiNode>,
        state: &mut AuiInteractionState,
        direction: AuiNavigationDirection,
    ) -> Option<(Option<String>, String)> {
        let scope_root = Self::focus_scope_root(document, nodes_by_id, state)?;
        let focusable = Self::focusable_nodes_in_scope(document, layout, nodes_by_id, &scope_root);
        if focusable.is_empty() {
            return None;
        }
        let previous = state.focus.focused_node.clone();
        let current_index = previous
            .as_ref()
            .and_then(|node| focusable.iter().position(|candidate| candidate == node));
        let explicit_target = previous.as_ref().and_then(|node_id| {
            let node = nodes_by_id.get(node_id.as_str())?;
            let target = Self::explicit_navigation_target(node, direction)?;
            focusable
                .iter()
                .any(|candidate| candidate == target)
                .then(|| target.to_string())
        });
        let next = if let Some(target) = explicit_target {
            target
        } else {
            if let Some(current) = previous.as_ref() {
                if let Some(node) = nodes_by_id.get(current.as_str()) {
                    if !Self::navigation_mode_allows(node.navigation.mode, direction) {
                        return None;
                    }
                }
            }
            match direction {
                AuiNavigationDirection::Next | AuiNavigationDirection::Previous => {
                    let reverse = direction == AuiNavigationDirection::Previous;
                    let next_index = match (current_index, reverse) {
                        (Some(index), false) => (index + 1) % focusable.len(),
                        (Some(0), true) => focusable.len() - 1,
                        (Some(index), true) => index - 1,
                        (None, false) => 0,
                        (None, true) => focusable.len() - 1,
                    };
                    focusable[next_index].clone()
                }
                AuiNavigationDirection::Up
                | AuiNavigationDirection::Down
                | AuiNavigationDirection::Left
                | AuiNavigationDirection::Right => {
                    if let Some(current) = previous.as_ref() {
                        Self::choose_directional_focus_candidate(
                            layout, &focusable, current, direction,
                        )?
                    } else {
                        focusable[0].clone()
                    }
                }
            }
        };
        if previous.as_deref() == Some(next.as_str()) {
            return None;
        }
        state.focus.focused_node = Some(next.clone());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        Some((previous, next))
    }

    fn scroll_container_for_node(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        node_id: &str,
    ) -> Option<String> {
        let mut current = node_id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let node = nodes_by_id.get(current)?;
            if matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List) {
                return Some(node.node_id.clone());
            }
            current = node.parent.as_deref()?;
        }
        None
    }

    fn ensure_focused_node_visible(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        focused_node: &str,
    ) -> Option<AuiScrollPayload> {
        let scroll_node = Self::scroll_container_for_node(nodes_by_id, focused_node)?;
        if scroll_node == focused_node {
            return None;
        }
        let viewport = *Self::computed_rect_for(layout, &scroll_node)?;
        let focused_rect = Self::computed_rect_for(layout, focused_node)?;
        let delta_y = if focused_rect.y < viewport.y {
            focused_rect.y - viewport.y
        } else if focused_rect.y + focused_rect.height > viewport.y + viewport.height {
            focused_rect.y + focused_rect.height - (viewport.y + viewport.height)
        } else {
            return None;
        };
        Self::update_scroll_offset(nodes_by_id, layout, state, &scroll_node, delta_y)
    }

    fn scrollbar_thumb_at_pointer<'a>(
        layout: &'a AuiLayoutResult,
        pointer: AuiPointer,
    ) -> Option<&'a AuiScrollbarMetrics> {
        layout
            .scrollbar_metrics
            .iter()
            .rev()
            .find(|metrics| metrics.visible && metrics.thumb_rect.contains(pointer.x, pointer.y))
    }

    pub fn canvas_effective_visible(state: &AuiInteractionState, canvas: &AuiCanvas) -> bool {
        state
            .canvas_visibility_overrides
            .get(canvas.canvas_id.as_str())
            .copied()
            .unwrap_or(canvas.visible)
    }

    pub fn push_screen(
        document: &AuiDocument,
        state: &mut AuiInteractionState,
        screen_id: &str,
    ) -> Option<AuiScreenStackEntry> {
        let canvas = document.canvases.iter().find(|canvas| {
            canvas.screen_id.as_deref() == Some(screen_id) || canvas.canvas_id == screen_id
        })?;
        state.queue_control_reconciliation();
        let previous_focus_node_id = state.focus.focused_node.clone();
        let default_focus = canvas
            .default_focus_node_id
            .clone()
            .or_else(|| Some(canvas.root_node.clone()));
        state
            .canvas_visibility_overrides
            .insert(canvas.canvas_id.clone(), true);
        state.focus.focused_node = default_focus.clone();
        state.focus.focus_scope_root = Some(canvas.root_node.clone());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        let default_focus_applied = default_focus.is_some();
        let entry = AuiScreenStackEntry {
            screen_id: canvas
                .screen_id
                .clone()
                .unwrap_or_else(|| canvas.canvas_id.clone()),
            document_path: None,
            canvas_id: canvas.canvas_id.clone(),
            root_node_id: canvas.root_node.clone(),
            default_focus_node_id: default_focus,
            previous_focus_node_id,
            modal: canvas.composition_stage == AuiCompositionStage::Modal,
            can_cancel: canvas.cancel_action_id.is_some()
                || canvas.composition_stage == AuiCompositionStage::Modal,
        };
        state.screen_stack.push_count += 1;
        state.screen_stack.default_focus_applied_count += usize::from(default_focus_applied);
        state.screen_stack.active_stack.push(entry.clone());
        Some(entry)
    }

    pub fn pop_screen(state: &mut AuiInteractionState) -> Option<AuiScreenStackEntry> {
        let entry = state.screen_stack.active_stack.pop()?;
        state.queue_control_reconciliation();
        state
            .canvas_visibility_overrides
            .insert(entry.canvas_id.clone(), false);
        state.screen_stack.last_popped_screen_id = Some(entry.screen_id.clone());
        state.focus.focused_node = entry.previous_focus_node_id.clone();
        state.focus.focus_scope_root = state
            .screen_stack
            .active_stack
            .last()
            .map(|entry| entry.root_node_id.clone());
        state.focus.focus_reason = if state.focus.focused_node.is_some() {
            AuiFocusReason::Keyboard
        } else {
            AuiFocusReason::Cleared
        };
        state.screen_stack.focus_restore_count += 1;
        Some(entry)
    }

    pub fn hit_test(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
    ) -> AuiHitTestResult {
        Self::hit_test_with_presentation(document, layout, x, y, None)
    }

    pub fn hit_test_target_space(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
        presentation: &ResolvedGameViewPresentation,
    ) -> AuiHitTestResult {
        Self::hit_test_with_presentation(document, layout, x, y, Some(presentation))
    }

    fn hit_test_with_presentation(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> AuiHitTestResult {
        let pointer = AuiPointer::new(x, y);
        let nodes_by_id: HashMap<&str, &AuiNode> = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect();
        let mut blocked_by_non_interactable = None;
        let mut clip_rejected_count = 0;

        for computed in layout.computed_nodes.iter().rev() {
            let candidate_pointer = match presentation {
                Some(presentation) => {
                    let Ok(point) = presentation
                        .target_to_reference(computed.canvas_id.as_str(), GameViewPoint::new(x, y))
                    else {
                        continue;
                    };
                    AuiPointer::new(point.x, point.y)
                }
                None => pointer,
            };
            if computed.clipped_by_node.is_some() {
                match computed.effective_clip_rect {
                    Some(clip_rect)
                        if !clip_rect.contains(candidate_pointer.x, candidate_pointer.y) =>
                    {
                        clip_rejected_count += 1;
                        continue;
                    }
                    None => {
                        clip_rejected_count += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            if !computed.effective_visible
                || !computed
                    .rect
                    .contains(candidate_pointer.x, candidate_pointer.y)
            {
                continue;
            }
            let Some(node) = nodes_by_id.get(computed.node_id.as_str()) else {
                continue;
            };
            if node.interactable {
                return AuiHitTestResult {
                    pointer: candidate_pointer,
                    hit_node: Some(node.node_id.clone()),
                    consumed: node.consume_input,
                    reason: AuiHitTestReason::HitInteractable,
                    clip_rejected_count,
                };
            }
            blocked_by_non_interactable
                .get_or_insert_with(|| (node.node_id.clone(), candidate_pointer));
        }

        if let Some((node_id, candidate_pointer)) = blocked_by_non_interactable {
            return AuiHitTestResult {
                pointer: candidate_pointer,
                hit_node: Some(node_id),
                consumed: false,
                reason: AuiHitTestReason::HitNonInteractable,
                clip_rejected_count,
            };
        }

        let mut outside = AuiHitTestResult::outside(pointer);
        outside.clip_rejected_count = clip_rejected_count;
        outside
    }

    fn is_submit_key(key: &str) -> bool {
        key.eq_ignore_ascii_case("Enter")
            || key.eq_ignore_ascii_case("Return")
            || key.eq_ignore_ascii_case("Space")
            || key == " "
    }

    fn is_cancel_key(key: &str) -> bool {
        key.eq_ignore_ascii_case("Escape") || key.eq_ignore_ascii_case("Esc")
    }

    fn text_edit_command_for_key(
        key: &str,
        shift: bool,
        control: bool,
    ) -> Option<AuiTextEditCommand> {
        if control && key.eq_ignore_ascii_case("A") {
            return Some(AuiTextEditCommand::SelectAll);
        }
        if key.eq_ignore_ascii_case("Backspace") {
            return Some(AuiTextEditCommand::Backspace);
        }
        if key.eq_ignore_ascii_case("Delete") {
            return Some(AuiTextEditCommand::Delete);
        }
        if key.eq_ignore_ascii_case("Home") {
            return Some(AuiTextEditCommand::MoveCaretHome);
        }
        if key.eq_ignore_ascii_case("End") {
            return Some(AuiTextEditCommand::MoveCaretEnd);
        }
        if key.eq_ignore_ascii_case("ArrowLeft") || key.eq_ignore_ascii_case("Left") {
            return Some(if shift {
                AuiTextEditCommand::SelectLeft
            } else {
                AuiTextEditCommand::MoveCaretLeft
            });
        }
        if key.eq_ignore_ascii_case("ArrowRight") || key.eq_ignore_ascii_case("Right") {
            return Some(if shift {
                AuiTextEditCommand::SelectRight
            } else {
                AuiTextEditCommand::MoveCaretRight
            });
        }
        None
    }

    fn gamepad_direction_for_event(event: &RuntimeInputEvent) -> Option<AuiNavigationDirection> {
        match event {
            RuntimeInputEvent::GamepadButtonDown { button, .. }
            | RuntimeInputEvent::GamepadButtonHeld { button, .. } => {
                match button.to_ascii_lowercase().as_str() {
                    "dpadup" | "up" => Some(AuiNavigationDirection::Up),
                    "dpaddown" | "down" => Some(AuiNavigationDirection::Down),
                    "dpadleft" | "left" => Some(AuiNavigationDirection::Left),
                    "dpadright" | "right" => Some(AuiNavigationDirection::Right),
                    _ => None,
                }
            }
            RuntimeInputEvent::GamepadAxis2d { x, y, .. } => {
                if y.abs() >= x.abs() && y.abs() > 0.5 {
                    Some(if *y > 0.0 {
                        AuiNavigationDirection::Down
                    } else {
                        AuiNavigationDirection::Up
                    })
                } else if x.abs() > 0.5 {
                    Some(if *x > 0.0 {
                        AuiNavigationDirection::Right
                    } else {
                        AuiNavigationDirection::Left
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn gamepad_submit(event: &RuntimeInputEvent) -> bool {
        matches!(
            event,
            RuntimeInputEvent::GamepadButtonDown { button, .. }
                if matches!(button.to_ascii_lowercase().as_str(), "south" | "a" | "submit")
        )
    }

    fn gamepad_cancel(event: &RuntimeInputEvent) -> bool {
        matches!(
            event,
            RuntimeInputEvent::GamepadButtonDown { button, .. }
                if matches!(button.to_ascii_lowercase().as_str(), "east" | "b" | "cancel")
        )
    }

    fn start_text_editing(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        state: &mut AuiInteractionState,
        node_id: &str,
    ) -> bool {
        let Some(node) = nodes_by_id.get(node_id) else {
            return false;
        };
        if node.kind != AuiNodeKind::InputField || node.read_only {
            return false;
        }
        let already_editing = matches!(
            &state.input_mode,
            AuiInputMode::TextEditing { node_id: active } if active == node_id
        );
        if !already_editing {
            state.input_field = Some(AuiInputFieldState::start(node));
            state.input_mode = AuiInputMode::TextEditing {
                node_id: node_id.to_string(),
            };
        }
        true
    }

    fn text_payload(state: &AuiInputFieldState) -> String {
        serde_json::json!({
            "node_id": state.node_id,
            "draft_text": state.draft_text,
            "caret_index": state.caret_index,
            "selection_anchor": state.selection_anchor,
            "selection_focus": state.selection_focus,
        })
        .to_string()
    }

    pub fn process(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
    ) -> AuiInteractionResult {
        let mut state = AuiInteractionState::default();
        Self::process_with_state(
            document,
            layout,
            input_frame,
            &mut state,
            AuiInteractionConfig::default(),
        )
    }

    pub fn process_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(document, layout, input_frame, state, config, None, None)
    }

    pub fn process_session_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        session_id: &str,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(
            document,
            layout,
            input_frame,
            state,
            config,
            None,
            Some(session_id),
        )
    }

    pub fn process_target_space_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        presentation: &ResolvedGameViewPresentation,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(
            document,
            layout,
            input_frame,
            state,
            config,
            Some(presentation),
            None,
        )
    }

    pub fn process_target_space_session_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        presentation: &ResolvedGameViewPresentation,
        session_id: &str,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(
            document,
            layout,
            input_frame,
            state,
            config,
            Some(presentation),
            Some(session_id),
        )
    }

    fn process_with_state_in_space(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        presentation: Option<&ResolvedGameViewPresentation>,
        session_id: Option<&str>,
    ) -> AuiInteractionResult {
        let mut result = AuiInteractionResult::default();
        result.input_mode_before = state.input_mode.label();
        result.ime_platform_coverage = "schema_headless_and_winit_cmin".to_string();
        result.focusable_derived_from_interactable = document
            .nodes
            .iter()
            .any(|node| node.focusable.is_none() && node.interactable);
        let nodes_by_id: HashMap<&str, &AuiNode> = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect();
        result.control_reconciliation_count += state.drain_control_reconciliation_count();
        Self::reconcile_session_identity(state, &mut result, session_id);
        Self::reconcile_effectively_hidden_state(layout, state, &mut result);
        let active_modal_root = Self::topmost_modal_root(document, layout, state);
        Self::reconcile_control_state(
            &nodes_by_id,
            layout,
            state,
            &mut result,
            active_modal_root.as_deref(),
        );
        state.active_modal_root = active_modal_root.clone();
        result.active_modal_root = active_modal_root.clone();
        match active_modal_root.as_ref() {
            Some(root) if state.focus.focus_scope_root.as_deref() != Some(root.as_str()) => {
                state.focus.focus_scope_root = Some(root.clone());
                state.focus.focus_reason = AuiFocusReason::ModalOpen;
                result.focus_change_count += 1;
            }
            None if state.focus.focus_scope_root.is_some() => {
                state.focus.focus_scope_root = None;
                state.focus.focus_reason = AuiFocusReason::Cleared;
                result.focus_change_count += 1;
            }
            _ => {}
        }
        if Self::apply_default_focus_if_needed(document, layout, &nodes_by_id, state) {
            result.default_focus_applied_count += 1;
            result.focus_change_count += 1;
        }

        for (event_index, event) in input_frame.events.iter().enumerate() {
            let text_event_kind = match event {
                RuntimeInputEvent::TextInput { .. } => Some(AuiInteractionEventKind::TextInput),
                RuntimeInputEvent::ImePreedit { .. } => Some(AuiInteractionEventKind::ImePreedit),
                RuntimeInputEvent::ImeCommit { .. } => Some(AuiInteractionEventKind::ImeCommit),
                RuntimeInputEvent::ImeCancel => Some(AuiInteractionEventKind::ImeCancel),
                _ => None,
            };
            if let Some(event_kind) = text_event_kind {
                let mut event_commands = Vec::new();
                let mut consumed = false;
                let mut source_node = None;
                if let Some(input_field) = state.input_field.as_mut() {
                    source_node = Some(input_field.node_id.clone());
                    if let Some(node) = nodes_by_id.get(input_field.node_id.as_str()) {
                        consumed = true;
                        result.normalized_ui_intent_count += 1;
                        match event {
                            RuntimeInputEvent::TextInput { text } => {
                                if !node.read_only && input_field.insert_text(text, node.max_length)
                                {
                                    result.text_changed_count += 1;
                                    event_commands.push(
                                        AuiCommand::new(
                                            result.commands.len() + event_commands.len(),
                                            input_field.node_id.clone(),
                                            AuiCommandKind::TextChanged,
                                        )
                                        .with_payload(Self::text_payload(input_field)),
                                    );
                                }
                            }
                            RuntimeInputEvent::ImePreedit {
                                text,
                                cursor_start,
                                cursor_end,
                            } => {
                                input_field.composition = Some(AuiTextCompositionState {
                                    preedit_text: text.clone(),
                                    cursor_start: *cursor_start,
                                    cursor_end: *cursor_end,
                                    active: true,
                                });
                                result.ime_preedit_count += 1;
                            }
                            RuntimeInputEvent::ImeCommit { text } => {
                                input_field.composition = None;
                                result.ime_commit_count += 1;
                                if !node.read_only && input_field.insert_text(text, node.max_length)
                                {
                                    result.text_changed_count += 1;
                                    event_commands.push(
                                        AuiCommand::new(
                                            result.commands.len() + event_commands.len(),
                                            input_field.node_id.clone(),
                                            AuiCommandKind::TextChanged,
                                        )
                                        .with_payload(Self::text_payload(input_field)),
                                    );
                                }
                            }
                            RuntimeInputEvent::ImeCancel => {
                                input_field.composition = None;
                                result.ime_cancel_count += 1;
                            }
                            _ => {}
                        }
                    }
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind,
                    pointer: AuiPointer::default(),
                    hit_node: source_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: AuiHitTestReason::OutsideUi,
                    command_count,
                    action_count,
                });
                continue;
            }

            if let Some(delta) = wheel_event_delta(event) {
                let pointer = input_frame
                    .pointer_position
                    .map(|position| AuiPointer::new(position.x, position.y))
                    .unwrap_or_default();
                let hit = Self::hit_test_with_presentation(
                    document,
                    layout,
                    pointer.x,
                    pointer.y,
                    presentation,
                );
                let pointer = hit.pointer;
                result.hit_test_clip_rejected_count += hit.clip_rejected_count;
                let mut event_commands = Vec::new();
                let mut consumed = false;
                let modal_inside = active_modal_root.as_deref().is_some_and(|root| {
                    hit.hit_node
                        .as_deref()
                        .is_some_and(|node| Self::node_in_subtree(&nodes_by_id, root, node))
                });
                let scroll_node =
                    Self::find_scroll_container(&nodes_by_id, hit.hit_node.as_deref());
                if let Some(scroll_node) = scroll_node {
                    let delta_y = -delta * config.wheel_scroll_px_per_delta;
                    if let Some(payload) = Self::update_scroll_offset(
                        &nodes_by_id,
                        layout,
                        state,
                        &scroll_node,
                        delta_y,
                    ) {
                        result.scroll_offset_change_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                scroll_node.clone(),
                                AuiCommandKind::Scroll,
                            )
                            .with_payload(payload.to_payload_string()),
                        );
                    }
                    consumed = true;
                } else if active_modal_root.is_some()
                    && config.modal_blocks_wheel_outside
                    && !modal_inside
                {
                    consumed = true;
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind: AuiInteractionEventKind::MouseWheel,
                    pointer,
                    hit_node: hit.hit_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: hit.reason,
                    command_count,
                    action_count,
                });
                continue;
            }

            if let Some(key_info) = key_event_info(event) {
                let mut event_commands = Vec::new();
                let modal_keyboard_consumed =
                    active_modal_root.is_some() && config.modal_blocks_keyboard;
                let mut consumed = false;
                let source_node = state.focus.focused_node.clone();
                let trace_source_node = source_node.clone().or_else(|| active_modal_root.clone());
                if key_info.kind == AuiInteractionEventKind::KeyDown {
                    let shift = input_frame
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.eq_ignore_ascii_case("Shift"));
                    let control = input_frame
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.eq_ignore_ascii_case("Control"));
                    let mut clear_text_editing = false;
                    if let Some(input_field) = state.input_field.as_mut() {
                        if Self::is_submit_key(&key_info.key) {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            result.text_submitted_count += 1;
                            event_commands.push(
                                AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    input_field.node_id.clone(),
                                    AuiCommandKind::TextSubmitted,
                                )
                                .with_payload(Self::text_payload(input_field)),
                            );
                            clear_text_editing = true;
                            consumed = true;
                        } else if Self::is_cancel_key(&key_info.key) {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            result.cancel_count += 1;
                            result.text_cancelled_count += 1;
                            event_commands.push(
                                AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    input_field.node_id.clone(),
                                    AuiCommandKind::TextCancelled,
                                )
                                .with_payload(input_field.original_text.clone()),
                            );
                            clear_text_editing = true;
                            consumed = true;
                        } else if let Some(command) =
                            Self::text_edit_command_for_key(&key_info.key, shift, control)
                        {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            let mut changed = false;
                            let mut moved = false;
                            match command {
                                AuiTextEditCommand::Backspace => {
                                    changed = input_field.backspace();
                                }
                                AuiTextEditCommand::Delete => {
                                    changed = input_field.delete();
                                }
                                AuiTextEditCommand::MoveCaretLeft
                                | AuiTextEditCommand::MoveCaretRight
                                | AuiTextEditCommand::MoveCaretHome
                                | AuiTextEditCommand::MoveCaretEnd
                                | AuiTextEditCommand::SelectLeft
                                | AuiTextEditCommand::SelectRight
                                | AuiTextEditCommand::SelectAll => {
                                    moved = input_field.move_caret(
                                        command,
                                        matches!(
                                            command,
                                            AuiTextEditCommand::SelectLeft
                                                | AuiTextEditCommand::SelectRight
                                        ),
                                    );
                                }
                            }
                            if changed {
                                result.text_changed_count += 1;
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        input_field.node_id.clone(),
                                        AuiCommandKind::TextChanged,
                                    )
                                    .with_payload(Self::text_payload(input_field)),
                                );
                            }
                            if moved {
                                result.caret_move_count += 1;
                                if input_field.selection_anchor != input_field.selection_focus {
                                    result.selection_change_count += 1;
                                }
                            }
                            consumed = true;
                        }
                    } else if Self::is_submit_key(&key_info.key) {
                        result.normalized_ui_intent_count += 1;
                        result.keyboard_intent_count += 1;
                        if let Some(source_node) = source_node.clone() {
                            if nodes_by_id
                                .get(source_node.as_str())
                                .is_some_and(|node| node.kind == AuiNodeKind::InputField)
                                && Self::start_text_editing(&nodes_by_id, state, &source_node)
                            {
                                result.text_edit_session_count += 1;
                            } else {
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    source_node,
                                    AuiCommandKind::Submit,
                                ));
                                result.submit_count += 1;
                            }
                            consumed = true;
                        }
                    } else if Self::is_cancel_key(&key_info.key) {
                        result.normalized_ui_intent_count += 1;
                        result.keyboard_intent_count += 1;
                        result.cancel_count += 1;
                        if state.screen_stack.active_stack.last().is_some() {
                            if let Some(popped) = Self::pop_screen(state) {
                                result.screen_stack_pop_count += 1;
                                result.control_reconciliation_count +=
                                    state.drain_control_reconciliation_count();
                                result.focus_restore_count = state.screen_stack.focus_restore_count;
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    popped.root_node_id,
                                    AuiCommandKind::Cancel,
                                ));
                            }
                            consumed = true;
                        } else if active_modal_root.is_some() {
                            if let Some(source_node) =
                                source_node.clone().or_else(|| active_modal_root.clone())
                            {
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    source_node,
                                    AuiCommandKind::Cancel,
                                ));
                            }
                            consumed = true;
                        }
                    }

                    if !consumed {
                        if let Some(direction) =
                            Self::navigation_direction_for_key(&key_info.key, shift)
                        {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            if let Some((previous, next)) =
                                Self::move_focus(document, layout, &nodes_by_id, state, direction)
                            {
                                if let Some(previous) = previous {
                                    event_commands.push(AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        previous,
                                        AuiCommandKind::Blur,
                                    ));
                                }
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    next,
                                    AuiCommandKind::Focus,
                                ));
                                result.focus_change_count += 1;
                                result.keyboard_navigation_event_count += 1;
                                let focused_after = state.focus.focused_node.clone();
                                if let Some(payload) = Self::ensure_focused_node_visible(
                                    &nodes_by_id,
                                    layout,
                                    state,
                                    focused_after.as_deref().unwrap_or_default(),
                                ) {
                                    result.focus_visible_scroll_count += 1;
                                    result.scroll_offset_change_count += 1;
                                    event_commands.push(
                                        AuiCommand::new(
                                            result.commands.len() + event_commands.len(),
                                            payload.node_id.clone(),
                                            AuiCommandKind::Scroll,
                                        )
                                        .with_payload(payload.to_payload_string()),
                                    );
                                }
                            }
                            consumed = true;
                        }
                    }

                    if clear_text_editing {
                        state.input_field = None;
                        state.input_mode = AuiInputMode::Navigation;
                    }
                }
                if !consumed && modal_keyboard_consumed {
                    consumed = true;
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind: key_info.kind,
                    pointer: AuiPointer::default(),
                    hit_node: trace_source_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: AuiHitTestReason::OutsideUi,
                    command_count,
                    action_count,
                });
                continue;
            }

            let gamepad_event_kind = match event {
                RuntimeInputEvent::GamepadButtonDown { .. } => {
                    Some(AuiInteractionEventKind::GamepadButtonDown)
                }
                RuntimeInputEvent::GamepadButtonUp { .. } => {
                    Some(AuiInteractionEventKind::GamepadButtonUp)
                }
                RuntimeInputEvent::GamepadButtonHeld { .. } => {
                    Some(AuiInteractionEventKind::GamepadButtonHeld)
                }
                RuntimeInputEvent::GamepadAxis2d { .. } => {
                    Some(AuiInteractionEventKind::GamepadAxis2d)
                }
                _ => None,
            };
            if let Some(event_kind) = gamepad_event_kind {
                let mut event_commands = Vec::new();
                let mut consumed = false;
                let source_node = state.focus.focused_node.clone();
                let trace_source_node = source_node.clone().or_else(|| active_modal_root.clone());
                let mut clear_text_editing = false;
                if Self::gamepad_submit(event) {
                    result.normalized_ui_intent_count += 1;
                    result.gamepad_intent_count += 1;
                    if let Some(input_field) = state.input_field.as_mut() {
                        result.text_submitted_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                input_field.node_id.clone(),
                                AuiCommandKind::TextSubmitted,
                            )
                            .with_payload(Self::text_payload(input_field)),
                        );
                        clear_text_editing = true;
                    } else if let Some(source_node) = source_node.clone() {
                        if nodes_by_id
                            .get(source_node.as_str())
                            .is_some_and(|node| node.kind == AuiNodeKind::InputField)
                            && Self::start_text_editing(&nodes_by_id, state, &source_node)
                        {
                            result.text_edit_session_count += 1;
                        } else {
                            result.submit_count += 1;
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                source_node,
                                AuiCommandKind::Submit,
                            ));
                        }
                    }
                    consumed = true;
                } else if Self::gamepad_cancel(event) {
                    result.normalized_ui_intent_count += 1;
                    result.gamepad_intent_count += 1;
                    result.cancel_count += 1;
                    if let Some(input_field) = state.input_field.as_ref() {
                        result.text_cancelled_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                input_field.node_id.clone(),
                                AuiCommandKind::TextCancelled,
                            )
                            .with_payload(input_field.original_text.clone()),
                        );
                        clear_text_editing = true;
                    } else if let Some(popped) = Self::pop_screen(state) {
                        result.screen_stack_pop_count += 1;
                        result.control_reconciliation_count +=
                            state.drain_control_reconciliation_count();
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            popped.root_node_id,
                            AuiCommandKind::Cancel,
                        ));
                    } else if let Some(source_node) =
                        source_node.clone().or_else(|| active_modal_root.clone())
                    {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            source_node,
                            AuiCommandKind::Cancel,
                        ));
                    }
                    consumed = true;
                } else if let Some(direction) = Self::gamepad_direction_for_event(event) {
                    result.normalized_ui_intent_count += 1;
                    result.gamepad_intent_count += 1;
                    if let Some((previous, next)) =
                        Self::move_focus(document, layout, &nodes_by_id, state, direction)
                    {
                        if let Some(previous) = previous {
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                previous,
                                AuiCommandKind::Blur,
                            ));
                        }
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            next,
                            AuiCommandKind::Focus,
                        ));
                        result.focus_change_count += 1;
                        result.keyboard_navigation_event_count += 1;
                    }
                    consumed = true;
                }
                if !consumed && active_modal_root.is_some() && config.modal_blocks_keyboard {
                    consumed = true;
                }
                if clear_text_editing {
                    state.input_field = None;
                    state.input_mode = AuiInputMode::Navigation;
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind,
                    pointer: AuiPointer::default(),
                    hit_node: trace_source_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: AuiHitTestReason::OutsideUi,
                    command_count,
                    action_count,
                });
                continue;
            }

            let Some(event_info) = pointer_event_info(event) else {
                continue;
            };
            let event_kind = event_info.kind;
            let target_pointer = event_info.pointer;
            let hit = Self::hit_test_with_presentation(
                document,
                layout,
                target_pointer.x,
                target_pointer.y,
                presentation,
            );
            let pointer = hit.pointer;
            result.hit_test_clip_rejected_count += hit.clip_rejected_count;
            let mut event_commands = Vec::new();
            let mut consumed = hit.consumed;
            let mut captured_node = state.active_drag_source().map(ToOwned::to_owned);
            let mut drop_target = None;
            let modal_inside = active_modal_root.as_deref().is_some_and(|root| {
                hit.hit_node
                    .as_deref()
                    .is_some_and(|node| Self::node_in_subtree(&nodes_by_id, root, node))
            });
            let pointer_blocked_by_modal =
                active_modal_root.is_some() && config.modal_blocks_pointer_outside && !modal_inside;
            if active_modal_root.is_some() && modal_inside {
                consumed = true;
            }
            let scroll_node = Self::find_scroll_container(&nodes_by_id, hit.hit_node.as_deref());
            let eligible_hit_node = (!pointer_blocked_by_modal
                && hit.reason == AuiHitTestReason::HitInteractable)
                .then(|| hit.hit_node.clone())
                .flatten();

            if event_info.hover_capable
                && matches!(
                    event_kind,
                    AuiInteractionEventKind::PointerDown
                        | AuiInteractionEventKind::PointerMove
                        | AuiInteractionEventKind::PointerUp
                )
            {
                state.hovered_node = eligible_hit_node.clone();
                if event_kind == AuiInteractionEventKind::PointerMove
                    && state.focus.focused_node.is_some()
                {
                    state.focus.focus_reason = AuiFocusReason::Pointer;
                }
            } else if !event_info.hover_capable
                || matches!(
                    event_kind,
                    AuiInteractionEventKind::PointerCancel | AuiInteractionEventKind::PointerLeave
                )
            {
                state.hovered_node = None;
            }

            if let Some(press) = state.primary_press.as_mut() {
                if event_info.matches_primary_press(press)
                    && event_kind == AuiInteractionEventKind::PointerMove
                {
                    press.inside = eligible_hit_node.as_deref() == Some(press.node_id.as_str());
                }
            }

            if event_kind == AuiInteractionEventKind::PointerCancel {
                let matching_press = state
                    .primary_press
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_primary_press(capture));
                let matching_drag = state
                    .active_drag
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_drag(capture));
                let matching_scroll = state
                    .active_scroll_capture
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_scroll(capture));
                if matching_press || matching_drag || matching_scroll {
                    consumed = true;
                    result.pointer_cancel_count += 1;
                    if matching_press {
                        if let Some(press) = state.primary_press.take() {
                            captured_node = Some(press.node_id.clone());
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                press.node_id,
                                AuiCommandKind::PointerCancel,
                            ));
                        }
                    }
                    if matching_drag {
                        if let Some(active_drag) = state.active_drag.take() {
                            captured_node = Some(active_drag.source_node.clone());
                            if active_drag.started {
                                let payload = AuiDragDropPayload::new(
                                    active_drag.source_node.clone(),
                                    None,
                                    active_drag.start_pointer,
                                    pointer,
                                    "cancel",
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        active_drag.source_node,
                                        AuiCommandKind::DragCancel,
                                    )
                                    .with_payload(payload),
                                );
                            }
                        }
                    }
                    if matching_scroll {
                        if let Some(capture) = state.active_scroll_capture.take() {
                            captured_node = Some(capture.captured_node_id);
                        }
                    }
                }
            } else if event_kind == AuiInteractionEventKind::PointerLeave {
                if let Some(press) = state.primary_press.as_mut() {
                    if event_info.matches_primary_press(press) {
                        press.inside = false;
                        captured_node = Some(press.node_id.clone());
                        consumed = true;
                    }
                }
                if state
                    .active_drag
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_drag(capture))
                    || state
                        .active_scroll_capture
                        .as_ref()
                        .is_some_and(|capture| event_info.matches_scroll(capture))
                {
                    consumed = true;
                }
            } else if state
                .active_scroll_capture
                .as_ref()
                .is_some_and(|capture| event_info.matches_scroll(capture))
            {
                consumed = true;
                let mut release_scroll = false;
                let mut pending_scroll_delta: Option<(String, f32)> = None;
                if let Some(capture) = state.active_scroll_capture.as_mut() {
                    captured_node = Some(capture.captured_node_id.clone());
                    match event_kind {
                        AuiInteractionEventKind::PointerMove => {
                            if !capture.started
                                && pointer_distance(capture.start_pointer, pointer)
                                    >= config.drag_scroll_threshold_px
                            {
                                capture.started = true;
                            }
                            if capture.started {
                                let pointer_delta_y = pointer.y - capture.last_pointer.y;
                                let delta_y = capture
                                    .scroll_delta_per_pointer_delta_y
                                    .map(|ratio| pointer_delta_y * ratio)
                                    .unwrap_or_else(|| -pointer_delta_y);
                                capture.last_pointer = pointer;
                                pending_scroll_delta = Some((capture.node_id.clone(), delta_y));
                            }
                        }
                        AuiInteractionEventKind::PointerUp => {
                            release_scroll = true;
                        }
                        _ => {}
                    }
                }
                if let Some((scroll_node, delta_y)) = pending_scroll_delta {
                    if let Some(payload) = Self::update_scroll_offset(
                        &nodes_by_id,
                        layout,
                        state,
                        &scroll_node,
                        delta_y,
                    ) {
                        result.scroll_offset_change_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                scroll_node,
                                AuiCommandKind::Scroll,
                            )
                            .with_payload(payload.to_payload_string()),
                        );
                    }
                }
                if release_scroll {
                    state.active_scroll_capture = None;
                }
            } else if event_kind == AuiInteractionEventKind::PointerDown
                && event_info.is_primary_pointer()
                && scroll_node.is_some()
            {
                let scrollbar_thumb = Self::scrollbar_thumb_at_pointer(layout, pointer);
                let scroll_node = scrollbar_thumb
                    .map(|metrics| metrics.scroll_node_id.clone())
                    .or_else(|| scroll_node.clone())
                    .expect("scroll node");
                let (captured_node_id, started, scroll_delta_per_pointer_delta_y) =
                    if let Some(metrics) = scrollbar_thumb {
                        let track_travel =
                            (metrics.track_rect.height - metrics.thumb_rect.height).max(0.0);
                        let ratio = if track_travel <= f32::EPSILON {
                            0.0
                        } else {
                            metrics.max_offset_y / track_travel
                        };
                        (metrics.thumb_node_id(), true, Some(ratio))
                    } else {
                        (scroll_node.clone(), false, None)
                    };
                state.active_scroll_capture = Some(AuiActiveScrollCapture {
                    node_id: scroll_node.clone(),
                    captured_node_id: captured_node_id.clone(),
                    start_pointer: pointer,
                    last_pointer: pointer,
                    started,
                    scroll_delta_per_pointer_delta_y,
                    pointer_id: event_info.pointer_id,
                    device_kind: event_info.device_kind,
                });
                captured_node = Some(captured_node_id);
                consumed = true;
            } else if state
                .active_drag
                .as_ref()
                .is_some_and(|capture| event_info.matches_drag(capture))
            {
                consumed = true;
                let mut release_drag = false;
                let mut click_source: Option<String> = None;
                if let Some(active_drag) = state.active_drag.as_mut() {
                    captured_node = Some(active_drag.source_node.clone());
                    active_drag.current_pointer = pointer;
                    match event_kind {
                        AuiInteractionEventKind::PointerMove => {
                            if !active_drag.started
                                && pointer_distance(active_drag.start_pointer, pointer)
                                    >= config.drag_threshold_px
                            {
                                active_drag.started = true;
                                let payload = AuiDragDropPayload::new(
                                    active_drag.source_node.clone(),
                                    None,
                                    active_drag.start_pointer,
                                    pointer,
                                    "start",
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        active_drag.source_node.clone(),
                                        AuiCommandKind::DragStart,
                                    )
                                    .with_payload(payload),
                                );
                            }
                            if active_drag.started {
                                let payload = AuiDragDropPayload::new(
                                    active_drag.source_node.clone(),
                                    None,
                                    active_drag.start_pointer,
                                    pointer,
                                    "move",
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        active_drag.source_node.clone(),
                                        AuiCommandKind::DragMove,
                                    )
                                    .with_payload(payload),
                                );
                            }
                        }
                        AuiInteractionEventKind::PointerUp => {
                            let source_node = active_drag.source_node.clone();
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                source_node.clone(),
                                AuiCommandKind::PointerUp,
                            ));
                            if active_drag.started {
                                drop_target = hit.hit_node.clone().filter(|node_id| {
                                    nodes_by_id
                                        .get(node_id.as_str())
                                        .map(|node| node.drop_target)
                                        .unwrap_or(false)
                                });
                                let phase = if drop_target.is_some() {
                                    "drop"
                                } else {
                                    "cancel"
                                };
                                let payload = AuiDragDropPayload::new(
                                    source_node.clone(),
                                    drop_target.clone(),
                                    active_drag.start_pointer,
                                    pointer,
                                    phase,
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        source_node,
                                        if drop_target.is_some() {
                                            AuiCommandKind::Drop
                                        } else {
                                            AuiCommandKind::DragCancel
                                        },
                                    )
                                    .with_payload(payload),
                                );
                            } else if hit.hit_node.as_deref() == Some(source_node.as_str()) {
                                click_source = Some(source_node);
                            }
                            release_drag = true;
                        }
                        AuiInteractionEventKind::PointerDown => {}
                        _ => {}
                    }
                }
                if let Some(source_node) = click_source {
                    event_commands.push(AuiCommand::new(
                        result.commands.len() + event_commands.len(),
                        source_node,
                        AuiCommandKind::Click,
                    ));
                }
                if release_drag {
                    state.active_drag = None;
                    state.primary_press = None;
                }
            } else if state
                .primary_press
                .as_ref()
                .is_some_and(|capture| event_info.matches_primary_press(capture))
                && matches!(
                    event_kind,
                    AuiInteractionEventKind::PointerMove | AuiInteractionEventKind::PointerUp
                )
            {
                consumed = true;
                let press = state.primary_press.as_mut().expect("primary press");
                press.inside = eligible_hit_node.as_deref() == Some(press.node_id.as_str());
                let owner = press.node_id.clone();
                captured_node = Some(owner.clone());
                let command_kind = if event_kind == AuiInteractionEventKind::PointerUp {
                    AuiCommandKind::PointerUp
                } else {
                    AuiCommandKind::PointerMove
                };
                event_commands.push(AuiCommand::new(
                    result.commands.len() + event_commands.len(),
                    owner.clone(),
                    command_kind,
                ));
                if event_kind == AuiInteractionEventKind::PointerUp {
                    if press.inside {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            owner,
                            AuiCommandKind::Click,
                        ));
                    }
                    state.primary_press = None;
                }
            } else if pointer_blocked_by_modal {
                consumed = true;
            } else if let Some(hit_node) = hit.hit_node.clone() {
                if hit.consumed {
                    let command_kind = match event_kind {
                        AuiInteractionEventKind::PointerDown => AuiCommandKind::PointerDown,
                        AuiInteractionEventKind::PointerUp => AuiCommandKind::PointerUp,
                        AuiInteractionEventKind::PointerMove => AuiCommandKind::PointerMove,
                        _ => AuiCommandKind::PointerMove,
                    };
                    event_commands.push(AuiCommand::new(
                        result.commands.len() + event_commands.len(),
                        hit_node.clone(),
                        command_kind,
                    ));

                    if event_kind == AuiInteractionEventKind::PointerMove {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            hit_node.clone(),
                            AuiCommandKind::Hover,
                        ));
                    }

                    if event_kind == AuiInteractionEventKind::PointerDown {
                        if event_info.is_primary_pointer() {
                            state.primary_press = Some(AuiPrimaryPressCapture {
                                node_id: hit_node.clone(),
                                pointer_id: event_info.pointer_id,
                                device_kind: event_info.device_kind,
                                hover_capable: event_info.hover_capable,
                                inside: true,
                            });
                            captured_node = Some(hit_node.clone());
                        }
                        if state.focus.focused_node.as_deref() != Some(hit_node.as_str()) {
                            if let Some(previous) =
                                state.focus.focused_node.replace(hit_node.clone())
                            {
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    previous,
                                    AuiCommandKind::Blur,
                                ));
                            }
                            state.focus.focus_reason = AuiFocusReason::Pointer;
                            result.focus_change_count += 1;
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                hit_node.clone(),
                                AuiCommandKind::Focus,
                            ));
                        }
                        if nodes_by_id
                            .get(hit_node.as_str())
                            .is_some_and(|node| node.kind == AuiNodeKind::InputField)
                            && Self::start_text_editing(&nodes_by_id, state, &hit_node)
                        {
                            result.text_edit_session_count += 1;
                        }
                        if event_info.is_primary_pointer()
                            && nodes_by_id
                                .get(hit_node.as_str())
                                .map(|node| node.draggable)
                                .unwrap_or(false)
                        {
                            state.active_drag = Some(AuiActiveDrag {
                                source_node: hit_node.clone(),
                                start_pointer: pointer,
                                current_pointer: pointer,
                                started: false,
                                pointer_id: event_info.pointer_id,
                                device_kind: event_info.device_kind,
                            });
                            captured_node = Some(hit_node.clone());
                        }
                    }
                }
            }

            let command_count = event_commands.len();
            let action_count = AuiActionMapper::map(document, &event_commands).len();
            if consumed {
                record_consumed_event(&mut result, event_index, event);
            }
            result.commands.extend(event_commands);
            result.traces.push(AuiInteractionTrace {
                frame: input_frame.frame_id,
                event_index,
                event_kind,
                pointer,
                hit_node: hit.hit_node,
                captured_node,
                drop_target,
                consumed,
                reason: hit.reason,
                command_count,
                action_count,
            });
        }

        result.actions = AuiActionMapper::map(document, &result.commands);
        result.input_mode_after = state.input_mode.label();
        result.active_screen_id = state
            .screen_stack
            .active_stack
            .last()
            .map(|entry| entry.screen_id.clone());
        result.screen_stack_push_count = state.screen_stack.push_count;
        result.default_focus_applied_count = state.screen_stack.default_focus_applied_count;
        result.focus_restore_count = state.screen_stack.focus_restore_count;
        result.action_prompt_reported = state.focus.focused_node.is_some();
        result.control_snapshot = Self::control_snapshot(input_frame.frame_id, state);
        result
    }
}

pub struct AuiActionMapper;

impl AuiActionMapper {
    pub fn map(document: &AuiDocument, commands: &[AuiCommand]) -> Vec<AuiAction> {
        let nodes_by_id: HashMap<&str, &AuiNode> = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect();
        let mut actions = Vec::new();

        for command in commands {
            let Some(action_event) = action_event_for_command(command.command_kind) else {
                continue;
            };
            let Some(node) = nodes_by_id.get(command.source_node.as_str()) else {
                continue;
            };
            for action_ref in node
                .action_refs
                .iter()
                .filter(|action_ref| action_ref.event == action_event)
            {
                actions.push(AuiAction {
                    action_id: action_ref.action_id.clone(),
                    node_id: node.node_id.clone(),
                    event: action_ref.event,
                    payload: command.payload.clone(),
                });
            }
        }

        actions
    }
}

fn action_event_for_command(command_kind: AuiCommandKind) -> Option<AuiActionEvent> {
    match command_kind {
        AuiCommandKind::Click => Some(AuiActionEvent::Click),
        AuiCommandKind::DragStart => Some(AuiActionEvent::DragStart),
        AuiCommandKind::DragMove => Some(AuiActionEvent::DragMove),
        AuiCommandKind::Drop => Some(AuiActionEvent::Drop),
        AuiCommandKind::Focus => Some(AuiActionEvent::Focus),
        AuiCommandKind::Blur => Some(AuiActionEvent::Blur),
        AuiCommandKind::Submit => Some(AuiActionEvent::Submit),
        AuiCommandKind::Cancel => Some(AuiActionEvent::Cancel),
        AuiCommandKind::Scroll => Some(AuiActionEvent::Scroll),
        AuiCommandKind::TextChanged => Some(AuiActionEvent::TextChanged),
        AuiCommandKind::TextSubmitted => Some(AuiActionEvent::TextSubmitted),
        AuiCommandKind::TextCancelled => Some(AuiActionEvent::TextCancelled),
        AuiCommandKind::PointerDown
        | AuiCommandKind::PointerUp
        | AuiCommandKind::PointerMove
        | AuiCommandKind::PointerCancel
        | AuiCommandKind::Hover
        | AuiCommandKind::DragCancel => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AuiPointerEventInfo {
    kind: AuiInteractionEventKind,
    pointer: AuiPointer,
    button: Option<RuntimePointerButton>,
    pointer_id: u64,
    device_kind: RuntimePointerDeviceKind,
    hover_capable: bool,
}

impl AuiPointerEventInfo {
    fn is_primary_pointer(self) -> bool {
        self.button == Some(RuntimePointerButton::Primary)
    }

    fn matches_primary_press(self, capture: &AuiPrimaryPressCapture) -> bool {
        self.pointer_id == capture.pointer_id && self.device_kind == capture.device_kind
    }

    fn matches_drag(self, capture: &AuiActiveDrag) -> bool {
        self.pointer_id == capture.pointer_id && self.device_kind == capture.device_kind
    }

    fn matches_scroll(self, capture: &AuiActiveScrollCapture) -> bool {
        self.pointer_id == capture.pointer_id && self.device_kind == capture.device_kind
    }
}

fn pointer_event_info(event: &RuntimeInputEvent) -> Option<AuiPointerEventInfo> {
    let pointer = event.pointer_event()?;
    let kind = match pointer.phase {
        RuntimePointerPhase::Down => AuiInteractionEventKind::PointerDown,
        RuntimePointerPhase::Move | RuntimePointerPhase::Held => {
            AuiInteractionEventKind::PointerMove
        }
        RuntimePointerPhase::Up => AuiInteractionEventKind::PointerUp,
        RuntimePointerPhase::Cancel => AuiInteractionEventKind::PointerCancel,
        RuntimePointerPhase::Leave => AuiInteractionEventKind::PointerLeave,
    };
    Some(AuiPointerEventInfo {
        kind,
        pointer: AuiPointer::new(pointer.x, pointer.y),
        button: pointer.button,
        pointer_id: pointer.pointer_id,
        device_kind: pointer.device_kind,
        hover_capable: pointer.hover_capable,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct AuiKeyEventInfo {
    kind: AuiInteractionEventKind,
    key: String,
}

fn key_event_info(event: &RuntimeInputEvent) -> Option<AuiKeyEventInfo> {
    match event {
        RuntimeInputEvent::KeyDown { key } => Some(AuiKeyEventInfo {
            kind: AuiInteractionEventKind::KeyDown,
            key: key.clone(),
        }),
        RuntimeInputEvent::KeyUp { key } => Some(AuiKeyEventInfo {
            kind: AuiInteractionEventKind::KeyUp,
            key: key.clone(),
        }),
        RuntimeInputEvent::KeyHeld { key } => Some(AuiKeyEventInfo {
            kind: AuiInteractionEventKind::KeyHeld,
            key: key.clone(),
        }),
        _ => None,
    }
}

fn wheel_event_delta(event: &RuntimeInputEvent) -> Option<f32> {
    match event {
        RuntimeInputEvent::MouseWheel { delta } => Some(*delta),
        _ => None,
    }
}

fn record_consumed_event(
    result: &mut AuiInteractionResult,
    event_index: usize,
    event: &RuntimeInputEvent,
) {
    result.consumed = true;
    if !result.consumed_event_indices.contains(&event_index) {
        result.consumed_event_indices.push(event_index);
    }
    *result
        .consumed_event_count_by_kind
        .entry(event.kind().to_string())
        .or_insert(0) += 1;
}

fn pointer_distance(a: AuiPointer, b: AuiPointer) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

pub struct AuiLayoutEngine;

#[derive(Debug, Clone)]
struct AuiActiveClip {
    node_id: String,
    rect: Option<AuiComputedRect>,
}

impl AuiLayoutEngine {
    pub fn validate(
        document: &AuiDocument,
        manifest: Option<&AuiAssetManifest>,
    ) -> AuiValidationReport {
        let node_ids: HashSet<&str> = document
            .nodes
            .iter()
            .map(|node| node.node_id.as_str())
            .collect();
        let mut seen = HashSet::new();
        let asset_ids = manifest
            .map(AuiAssetManifest::asset_ids)
            .unwrap_or_default();
        let mut items = Vec::new();
        let mut missing_asset_count = 0;

        for node in &document.nodes {
            if !seen.insert(node.node_id.as_str()) {
                items.push(AuiValidationItem::error(
                    "duplicate_node_id",
                    Some(node.node_id.clone()),
                    None,
                    format!("AUI node '{}' is duplicated.", node.node_id),
                    "Keep node_id unique inside one AuiDocument.",
                ));
            }

            match node.kind {
                AuiNodeKind::Image if node.text.is_some() => {
                    items.push(AuiValidationItem::warning(
                        "image_node_text_field_ignored",
                        Some(node.node_id.clone()),
                        None,
                        format!(
                            "AUI Image node '{}' contains text, but Image nodes generate DrawImage only.",
                            node.node_id
                        ),
                        "Move text into a Text child node inside the same AUI subtree.",
                    ));
                }
                AuiNodeKind::Text if node.image.is_some() => {
                    items.push(AuiValidationItem::warning(
                        "text_node_image_field_ignored",
                        Some(node.node_id.clone()),
                        node.image.as_ref().map(|image| image.asset_id.clone()),
                        format!(
                            "AUI Text node '{}' contains image, but Text nodes generate DrawText only.",
                            node.node_id
                        ),
                        "Move image into an Image child node inside the same AUI subtree.",
                    ));
                }
                _ => {}
            }

            if let Some(parent) = &node.parent {
                if !node_ids.contains(parent.as_str()) {
                    items.push(AuiValidationItem::error(
                        "missing_parent",
                        Some(node.node_id.clone()),
                        None,
                        format!(
                            "AUI node '{}' references missing parent '{}'.",
                            node.node_id, parent
                        ),
                        "Create the parent node or update the node parent field.",
                    ));
                }
            }

            if let Some(image) = &node.image {
                if !asset_ids.contains(image.asset_id.as_str()) {
                    missing_asset_count += 1;
                    items.push(AuiValidationItem::error(
                        "missing_image_asset",
                        Some(node.node_id.clone()),
                        Some(image.asset_id.clone()),
                        format!(
                            "AUI image node '{}' references missing asset '{}'.",
                            node.node_id, image.asset_id
                        ),
                        "Add the asset to AuiAssetManifest or update node.image.",
                    ));
                }
            }
        }

        for canvas in &document.canvases {
            if !node_ids.contains(canvas.root_node.as_str()) {
                items.push(AuiValidationItem::error(
                    "missing_canvas_root",
                    Some(canvas.root_node.clone()),
                    None,
                    format!(
                        "AUI canvas '{}' references missing root node '{}'.",
                        canvas.canvas_id, canvas.root_node
                    ),
                    "Create the canvas root node or update canvas.root_node.",
                ));
            }
        }

        let full_screen_image_rejected = Self::is_full_screen_single_image_ui(document);
        if full_screen_image_rejected {
            items.push(AuiValidationItem::error(
                "full_screen_image_ui_rejected",
                None,
                None,
                "AUI document looks like a full-screen image UI shortcut.",
                "Split the UI into AuiNode Image/Text/Button structures instead of using one screenshot.",
            ));
        }

        AuiValidationReport::from_items(items, missing_asset_count, full_screen_image_rejected)
    }

    pub fn layout(document: &AuiDocument, frame: u64) -> AuiLayoutResult {
        let scroll_offsets = BTreeMap::new();
        Self::layout_with_scroll_offsets(document, frame, &scroll_offsets)
    }

    pub fn layout_with_scroll_offsets(
        document: &AuiDocument,
        frame: u64,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
    ) -> AuiLayoutResult {
        Self::layout_with_scroll_offsets_and_canvas_visibility(
            document,
            frame,
            scroll_offsets,
            &BTreeMap::new(),
        )
    }

    pub fn layout_with_interaction_state(
        document: &AuiDocument,
        frame: u64,
        state: &AuiInteractionState,
    ) -> AuiLayoutResult {
        Self::layout_with_scroll_offsets_and_canvas_visibility(
            document,
            frame,
            &state.scroll_offsets,
            &state.canvas_visibility_overrides,
        )
    }

    fn layout_with_scroll_offsets_and_canvas_visibility(
        document: &AuiDocument,
        frame: u64,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
        canvas_visibility_overrides: &BTreeMap<String, bool>,
    ) -> AuiLayoutResult {
        let nodes_by_id: HashMap<&str, &AuiNode> = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect();
        let mut computed_nodes = Vec::new();
        let mut tree_order = 0;
        let mut scroll_offset_applied = false;
        let mut scroll_applied_node_count = 0;
        let mut clipped_node_count = 0;
        let mut clip_root_count = 0;

        for canvas in &document.canvases {
            if canvas.mode != AuiCanvasMode::ScreenOverlay {
                continue;
            }
            let visible = canvas_visibility_overrides
                .get(canvas.canvas_id.as_str())
                .copied()
                .unwrap_or(canvas.visible);
            if !visible {
                continue;
            }
            let root_rect = AuiComputedRect {
                x: 0.0,
                y: 0.0,
                width: canvas.reference_resolution.x,
                height: canvas.reference_resolution.y,
            };
            Self::layout_node(
                canvas,
                &nodes_by_id,
                canvas.root_node.as_str(),
                root_rect,
                &mut tree_order,
                &mut computed_nodes,
                scroll_offsets,
                None,
                &mut scroll_offset_applied,
                &mut scroll_applied_node_count,
                &mut clipped_node_count,
                &mut clip_root_count,
                true,
            );
        }

        let visible_node_count = computed_nodes
            .iter()
            .filter(|node| node.effective_visible)
            .count();
        let scrollbar_metrics =
            Self::compute_scrollbar_metrics(&nodes_by_id, &computed_nodes, scroll_offsets);
        AuiLayoutResult {
            scrollbar_metrics,
            report: AuiLayoutReport {
                frame,
                canvas_count: document.canvases.len(),
                node_count: computed_nodes.len(),
                visible_node_count,
                clipped_node_count,
                clip_root_count,
                effective_clip_node_count: computed_nodes
                    .iter()
                    .filter(|node| node.effective_clip_rect.is_some())
                    .count(),
                overflow_count: 0,
                invalid_binding_count: 0,
                scroll_offset_applied,
                scroll_applied_node_count,
            },
            computed_nodes,
        }
    }

    pub fn extract_draw_list(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
    ) -> (AuiDrawList, AuiRenderReport) {
        Self::extract_draw_list_internal(document, layout, None)
    }

    pub fn extract_draw_list_with_visual_overrides(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        overrides: &crate::aui_control_feedback::AuiVisualOverrideSet,
    ) -> (AuiDrawList, AuiRenderReport) {
        Self::extract_draw_list_internal(document, layout, Some(overrides))
    }

    fn extract_draw_list_internal(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        overrides: Option<&crate::aui_control_feedback::AuiVisualOverrideSet>,
    ) -> (AuiDrawList, AuiRenderReport) {
        let nodes_by_id: HashMap<&str, &AuiNode> = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect();
        let computed_by_id: HashMap<&str, &AuiComputedNode> = layout
            .computed_nodes
            .iter()
            .map(|computed| (computed.node_id.as_str(), computed))
            .collect();
        let mut commands = Vec::new();
        let mut culled_draw_item_count = 0;

        for computed in layout
            .computed_nodes
            .iter()
            .filter(|node| node.effective_visible)
        {
            let Some(node) = nodes_by_id.get(computed.node_id.as_str()) else {
                continue;
            };
            if computed.clipped_by_node.is_some() && computed.effective_clip_rect.is_none() {
                culled_draw_item_count += draw_command_count_for_node(node);
                continue;
            }
            let visual_owner = overrides.and_then(|overrides| {
                aui_visual_override_owner(&nodes_by_id, node.node_id.as_str(), overrides)
            });
            let visual = visual_owner.as_ref().map(|(_, visual)| **visual);
            let owner_rect = visual_owner.as_ref().and_then(|(owner_id, _)| {
                computed_by_id
                    .get(owner_id.as_str())
                    .map(|computed| computed.rect)
            });
            let transform_rect = |rect: AuiComputedRect| match (visual, owner_rect) {
                (Some(visual), Some(owner_rect)) => {
                    aui_feedback_transform_rect(rect, owner_rect, visual)
                }
                _ => rect,
            };
            let transform_color = |color: Option<String>| match visual {
                Some(visual) => aui_feedback_transform_color(color, visual),
                None => color,
            };
            match node.kind {
                AuiNodeKind::Panel | AuiNodeKind::Button | AuiNodeKind::InputField => {
                    commands.push(AuiDrawCommand::DrawRect {
                        node_id: node.node_id.clone(),
                        rect: transform_rect(computed.rect),
                        effective_clip_rect: computed.effective_clip_rect,
                        color: transform_color(
                            node.style.as_ref().and_then(|style| style.color.clone()),
                        ),
                    });
                    if matches!(node.kind, AuiNodeKind::Button | AuiNodeKind::InputField) {
                        let text = node.text.as_ref().or(node.placeholder.as_ref());
                        if let Some(text) = text {
                            commands.push(AuiDrawCommand::DrawText {
                                node_id: format!("{}:label", node.node_id),
                                rect: transform_rect(computed.rect),
                                effective_clip_rect: computed.effective_clip_rect,
                                text: text.clone(),
                                color: transform_color(
                                    node.style
                                        .as_ref()
                                        .and_then(|style| style.text_color.clone()),
                                ),
                                font_size: node.style.as_ref().and_then(|style| style.font_size),
                                font: node.style.as_ref().and_then(|style| style.font.clone()),
                            });
                        }
                    }
                }
                AuiNodeKind::ProgressBar => {
                    commands.push(AuiDrawCommand::DrawRect {
                        node_id: format!("{}:track", node.node_id),
                        rect: transform_rect(computed.rect),
                        effective_clip_rect: computed.effective_clip_rect,
                        color: transform_color(
                            node.style
                                .as_ref()
                                .and_then(|style| style.color.clone())
                                .or_else(|| Some("#303030".to_string())),
                        ),
                    });
                    let value = node.progress_value.unwrap_or(0.0).clamp(0.0, 1.0);
                    let fill_rect = AuiComputedRect {
                        x: computed.rect.x,
                        y: computed.rect.y,
                        width: computed.rect.width * value,
                        height: computed.rect.height,
                    };
                    commands.push(AuiDrawCommand::DrawRect {
                        node_id: format!("{}:fill", node.node_id),
                        rect: transform_rect(fill_rect),
                        effective_clip_rect: computed.effective_clip_rect,
                        color: transform_color(Some("#41d17d".to_string())),
                    });
                }
                AuiNodeKind::Image => {
                    if let Some(image) = &node.image {
                        commands.push(AuiDrawCommand::DrawImage {
                            node_id: node.node_id.clone(),
                            rect: transform_rect(computed.rect),
                            effective_clip_rect: computed.effective_clip_rect,
                            asset_id: image.asset_id.clone(),
                            color: transform_color(
                                node.style.as_ref().and_then(|style| style.color.clone()),
                            ),
                        });
                    }
                }
                AuiNodeKind::Text => {
                    if let Some(text) = &node.text {
                        commands.push(AuiDrawCommand::DrawText {
                            node_id: node.node_id.clone(),
                            rect: transform_rect(computed.rect),
                            effective_clip_rect: computed.effective_clip_rect,
                            text: text.clone(),
                            color: transform_color(
                                node.style
                                    .as_ref()
                                    .and_then(|style| style.text_color.clone()),
                            ),
                            font_size: node.style.as_ref().and_then(|style| style.font_size),
                            font: node.style.as_ref().and_then(|style| style.font.clone()),
                        });
                    }
                }
                _ => {}
            }
        }

        for metrics in layout
            .scrollbar_metrics
            .iter()
            .filter(|metrics| metrics.visible)
        {
            commands.push(AuiDrawCommand::DrawRect {
                node_id: metrics.track_node_id(),
                rect: metrics.track_rect,
                effective_clip_rect: None,
                color: Some("#1c232b".to_string()),
            });
            commands.push(AuiDrawCommand::DrawRect {
                node_id: metrics.thumb_node_id(),
                rect: metrics.thumb_rect,
                effective_clip_rect: None,
                color: Some("#6f7d8c".to_string()),
            });
        }

        let text_count = commands
            .iter()
            .filter(|command| matches!(command, AuiDrawCommand::DrawText { .. }))
            .count();
        let image_count = commands
            .iter()
            .filter(|command| matches!(command, AuiDrawCommand::DrawImage { .. }))
            .count();
        let effective_clip_item_count = commands
            .iter()
            .filter(|command| draw_command_effective_clip_rect(command).is_some())
            .count();
        let report = AuiRenderReport {
            draw_command_count: commands.len(),
            text_count,
            image_count,
            effective_clip_item_count,
            culled_draw_item_count,
            scrollbar_visible_count: layout
                .scrollbar_metrics
                .iter()
                .filter(|metrics| metrics.visible)
                .count(),
            batch_hint_count: 0,
        };

        (AuiDrawList { commands }, report)
    }

    fn layout_node(
        canvas: &AuiCanvas,
        nodes_by_id: &HashMap<&str, &AuiNode>,
        node_id: &str,
        parent_rect: AuiComputedRect,
        tree_order: &mut usize,
        computed_nodes: &mut Vec<AuiComputedNode>,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
        active_clip: Option<AuiActiveClip>,
        scroll_offset_applied: &mut bool,
        scroll_applied_node_count: &mut usize,
        clipped_node_count: &mut usize,
        clip_root_count: &mut usize,
        parent_effective_visible: bool,
    ) {
        let Some(node) = nodes_by_id.get(node_id) else {
            return;
        };
        let rect = node.rect.resolve(parent_rect);
        let effective_clip_rect = active_clip
            .as_ref()
            .and_then(|clip| clip.rect.and_then(|clip_rect| rect.intersection(clip_rect)));
        let clipped_by_node = active_clip.as_ref().map(|clip| clip.node_id.clone());
        if active_clip.is_some() {
            *scroll_applied_node_count += 1;
            if effective_clip_rect.is_none() {
                *clipped_node_count += 1;
            }
        }
        let current_order = *tree_order;
        *tree_order += 1;
        let effective_visible = parent_effective_visible && node.visible;
        computed_nodes.push(AuiComputedNode {
            canvas_id: canvas.canvas_id.clone(),
            composition_stage: canvas.composition_stage,
            node_id: node.node_id.clone(),
            kind: node.kind,
            rect,
            effective_clip_rect,
            clipped_by_node: clipped_by_node.clone(),
            tree_order: current_order,
            local_visible: node.visible,
            effective_visible,
        });

        let offset_y = if matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List) {
            scroll_offsets
                .get(node.node_id.as_str())
                .map(|state| state.offset_y)
                .unwrap_or_default()
        } else {
            0.0
        };
        let child_parent_rect = if offset_y.abs() > f32::EPSILON {
            *scroll_offset_applied = true;
            AuiComputedRect {
                x: rect.x,
                y: rect.y - offset_y,
                width: rect.width,
                height: rect.height,
            }
        } else {
            rect
        };
        let node_is_clip_root = matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List)
            || node.clip_policy == AuiClipPolicy::Rect;
        if node_is_clip_root {
            *clip_root_count += 1;
        }
        let child_clip = if node_is_clip_root {
            let rect = active_clip
                .as_ref()
                .and_then(|clip| clip.rect)
                .map_or(Some(rect), |parent_clip| rect.intersection(parent_clip));
            Some(AuiActiveClip {
                node_id: node.node_id.clone(),
                rect,
            })
        } else {
            active_clip
        };

        for child_id in &node.children {
            Self::layout_node(
                canvas,
                nodes_by_id,
                child_id,
                child_parent_rect,
                tree_order,
                computed_nodes,
                scroll_offsets,
                child_clip.clone(),
                scroll_offset_applied,
                scroll_applied_node_count,
                clipped_node_count,
                clip_root_count,
                effective_visible,
            );
        }
    }

    fn compute_scrollbar_metrics(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        computed_nodes: &[AuiComputedNode],
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
    ) -> Vec<AuiScrollbarMetrics> {
        let computed_by_id = computed_nodes
            .iter()
            .map(|computed| (computed.node_id.as_str(), computed))
            .collect::<HashMap<_, _>>();
        let mut metrics = Vec::new();
        for computed in computed_nodes
            .iter()
            .filter(|computed| matches!(computed.kind, AuiNodeKind::ScrollView | AuiNodeKind::List))
        {
            let Some(node) = nodes_by_id.get(computed.node_id.as_str()) else {
                continue;
            };
            if node.scrollbar_policy == AuiScrollbarPolicy::None {
                continue;
            }
            let viewport = computed.rect;
            let mut max_bottom = viewport.y + viewport.height;
            for child in computed_nodes {
                if child.node_id == computed.node_id {
                    continue;
                }
                if Self::layout_node_in_subtree(
                    nodes_by_id,
                    computed.node_id.as_str(),
                    child.node_id.as_str(),
                ) {
                    max_bottom = max_bottom.max(child.rect.y + child.rect.height);
                }
            }
            let content_height = (max_bottom - viewport.y).max(viewport.height);
            let max_offset_y = (content_height - viewport.height).max(0.0);
            let offset_y = scroll_offsets
                .get(computed.node_id.as_str())
                .map(|state| state.offset_y.clamp(0.0, max_offset_y))
                .unwrap_or_default();
            let visible = computed.effective_visible
                && match node.scrollbar_policy {
                    AuiScrollbarPolicy::None => false,
                    AuiScrollbarPolicy::Auto => max_offset_y > f32::EPSILON,
                    AuiScrollbarPolicy::Always => true,
                };
            let track_width = 8.0_f32.min(viewport.width.max(0.0));
            let track_rect = AuiComputedRect {
                x: viewport.x + viewport.width - track_width,
                y: viewport.y,
                width: track_width,
                height: viewport.height,
            };
            let thumb_height = if content_height <= f32::EPSILON {
                viewport.height
            } else {
                (viewport.height * (viewport.height / content_height))
                    .clamp(18.0_f32.min(viewport.height), viewport.height)
            };
            let travel = (viewport.height - thumb_height).max(0.0);
            let thumb_y = if max_offset_y <= f32::EPSILON {
                viewport.y
            } else {
                viewport.y + travel * (offset_y / max_offset_y)
            };
            let thumb_rect = AuiComputedRect {
                x: track_rect.x,
                y: thumb_y,
                width: track_rect.width,
                height: thumb_height,
            };
            if computed_by_id.contains_key(computed.node_id.as_str()) {
                metrics.push(AuiScrollbarMetrics {
                    scroll_node_id: computed.node_id.clone(),
                    axis: AuiScrollbarAxis::Vertical,
                    track_rect,
                    thumb_rect,
                    offset_y,
                    max_offset_y,
                    viewport_height: viewport.height,
                    content_height,
                    visible,
                });
            }
        }
        metrics
    }

    fn layout_node_in_subtree(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        root_node: &str,
        node_id: &str,
    ) -> bool {
        if root_node == node_id {
            return true;
        }
        let mut current = node_id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let Some(node) = nodes_by_id.get(current) else {
                return false;
            };
            let Some(parent) = node.parent.as_deref() else {
                return false;
            };
            if parent == root_node {
                return true;
            }
            current = parent;
        }
        false
    }

    fn is_full_screen_single_image_ui(document: &AuiDocument) -> bool {
        if document.canvases.len() != 1 || document.nodes.len() != 1 {
            return false;
        }
        let canvas = &document.canvases[0];
        let node = &document.nodes[0];
        node.kind == AuiNodeKind::Image
            && node.node_id == canvas.root_node
            && node
                .rect
                .resolve(AuiComputedRect {
                    x: 0.0,
                    y: 0.0,
                    width: canvas.reference_resolution.x,
                    height: canvas.reference_resolution.y,
                })
                .approximately_full_screen(canvas.reference_resolution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui_control_feedback::{AuiControlVisualOverride, AuiVisualOverrideSet};
    use crate::font_bundle::{
        CookedFontBundleAsset, CookedFontBundleGlyph, CookedFontBundleKerning,
        CookedFontBundlePage, RuntimeFontBundleRegistry, RuntimeLoadedFontBundle,
        COOKED_FONT_BUNDLE_SCHEMA_VERSION,
    };
    use crate::game_view_presentation::{
        GameViewExtent, GameViewPresentationModule, GameViewPresentationSpec, GameViewRect,
        GameViewScalePolicy,
    };
    use crate::input_mapping::{
        RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton, RuntimePointerDeviceKind,
        RuntimePointerEvent, RuntimePointerPhase,
    };
    use crate::runtime_package::{
        CookedFontAtlasAsset, CookedFontAtlasGlyph, RuntimeAuiFontAtlasRegistry,
        RuntimeLoadedFontAtlas, COOKED_FONT_ATLAS_SCHEMA_VERSION,
    };

    #[test]
    fn aui_feedback_schema_selector_round_trip_is_stable() {
        for selection in [
            AuiFeedbackSelection::auto(),
            AuiFeedbackSelection::none(),
            AuiFeedbackSelection::profile("ink.button"),
        ] {
            let json = serde_json::to_string(&selection).unwrap();
            let round_trip: AuiFeedbackSelection = serde_json::from_str(&json).unwrap();
            assert_eq!(round_trip, selection);
        }
        assert_eq!(AuiFeedbackSelection::default().as_str(), "auto");
    }

    #[test]
    fn aui_feedback_schema_registry_rejects_unknown_fields() {
        let invalid_registry = serde_json::json!({
            "motion_scale_permille": 1000,
            "profiles": [],
            "arbitrary_script": "pulse_forever"
        });
        assert!(
            serde_json::from_value::<AuiInteractionFeedbackRegistry>(invalid_registry).is_err()
        );

        let mut profile =
            serde_json::to_value(AuiInteractionFeedbackProfile::new("ink.button")).unwrap();
        profile
            .as_object_mut()
            .unwrap()
            .insert("keyframes".to_string(), serde_json::json!([]));
        assert!(serde_json::from_value::<AuiInteractionFeedbackProfile>(profile).is_err());
    }

    fn sample_document() -> (AuiDocument, AuiAssetManifest) {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["panel", "icon", "title"])
            .with_style(AuiStyle::color("#101820"));
        let panel = AuiNode::new(
            "panel",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(100.0, 80.0, 320.0, 180.0),
        )
        .with_parent("root")
        .with_style(AuiStyle::color("#223344"));
        let icon = AuiNode::new(
            "icon",
            AuiNodeKind::Image,
            AuiRect::fixed_position(120.0, 100.0, 64.0, 64.0),
        )
        .with_parent("root")
        .with_image("icon_asset");
        let title = AuiNode::new(
            "title",
            AuiNodeKind::Text,
            AuiRect::fixed_position(200.0, 112.0, 220.0, 40.0),
        )
        .with_parent("root")
        .with_text("Start")
        .with_style(AuiStyle::text("#ffffff", 24.0));
        let canvas = AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root");
        let manifest = AuiAssetManifest::new(
            "main_assets",
            vec![AuiAssetManifestEntry::image(
                "icon_asset",
                "asset://ui/icon.png",
                vec!["icon".to_string()],
            )],
        );
        (
            AuiDocument::new("sample", vec![canvas], vec![root, panel, icon, title]),
            manifest,
        )
    }

    fn v2_font_registry() -> RuntimeFontBundleRegistry {
        let pages = vec![
            CookedFontBundlePage {
                page_index: 0,
                render_mode: FontBundleRenderMode::BitmapR8,
                format: "r8Unorm".to_string(),
                width: 64,
                height: 64,
                byte_len: 4096,
                sha256: "sha256:bitmap".to_string(),
                payload_path: "bitmap.r8".to_string(),
            },
            CookedFontBundlePage {
                page_index: 1,
                render_mode: FontBundleRenderMode::MsdfRgba8,
                format: "rgba8Unorm".to_string(),
                width: 64,
                height: 64,
                byte_len: 16384,
                sha256: "sha256:msdf".to_string(),
                payload_path: "msdf.rgba8".to_string(),
            },
        ];
        let mut glyphs = Vec::new();
        for (index, character) in ['A', 'V', '中'].into_iter().enumerate() {
            for (render_mode, pixel_size, page_index, y) in [
                (FontBundleRenderMode::BitmapR8, 16, 0, 0),
                (FontBundleRenderMode::BitmapR8, 24, 0, 16),
                (FontBundleRenderMode::BitmapR8, 32, 0, 40),
                (FontBundleRenderMode::MsdfRgba8, 64, 1, 0),
            ] {
                glyphs.push(CookedFontBundleGlyph {
                    font_family_id: "family-ui".to_string(),
                    font_face_id: "face-ui".to_string(),
                    style: FontBundleStyle::Normal,
                    weight: 400,
                    glyph_id: index as u16 + 1,
                    codepoint: u32::from(character),
                    render_mode,
                    pixel_size,
                    page_index,
                    pixel_rect: [index as u32 * 8, y, 8, 12],
                    bearing_x: 0,
                    bearing_y: 12,
                    advance_per_em_millionths: 600_000,
                });
            }
        }
        for (pixel_size, y) in [(16, 0), (24, 16), (32, 40)] {
            glyphs.push(CookedFontBundleGlyph {
                font_family_id: "family-ui".to_string(),
                font_face_id: "face-ui".to_string(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id: 4,
                codepoint: u32::from(' '),
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size,
                page_index: 0,
                pixel_rect: [24, y, 0, 0],
                bearing_x: 0,
                bearing_y: 0,
                advance_per_em_millionths: 300_000,
            });
        }
        let metadata = CookedFontBundleAsset {
            schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
            font_bundle_id: "font-ui".to_string(),
            font_stack_id: "stack-ui".to_string(),
            generation: 7,
            max_bitmap_pages: 2,
            max_msdf_pages: 2,
            legacy_mode: false,
            fallback_used: false,
            quality_gate_eligible: true,
            pages,
            glyphs,
            kerning_adjustments: vec![CookedFontBundleKerning {
                font_face_id: "face-ui".to_string(),
                left_glyph_id: 1,
                right_glyph_id: 2,
                adjustment_per_em_millionths: -100_000,
            }],
            bundle_digest: "sha256:test".to_string(),
        };
        let mut registry = RuntimeFontBundleRegistry::default();
        registry.default_ui_font_bundle_id = Some("font-ui".to_string());
        registry.bundles_by_id.insert(
            "font-ui".to_string(),
            RuntimeLoadedFontBundle {
                metadata,
                page_payloads: vec![vec![0; 4096], vec![0; 16384]],
            },
        );
        registry
    }

    fn v2_text_overlay(font_size: f32, raster_policy: AuiFontRasterPolicy) -> AuiOverlayFrame {
        AuiOverlayFrame {
            frame_index: 1,
            draw_items: vec![AuiOverlayDrawItem {
                item_id: "text-1".to_string(),
                canvas_id: "main".to_string(),
                composition_stage: AuiCompositionStage::ScreenOverlay,
                node_id: "label".to_string(),
                item_kind: AuiOverlayItemKind::Text,
                rect: AuiComputedRect {
                    x: 0.0,
                    y: 0.0,
                    width: 300.0,
                    height: 80.0,
                },
                effective_clip_rect: None,
                color: Some("#fff".to_string()),
                asset_id: None,
                text: Some("AV中".to_string()),
                font_size: Some(font_size),
                font: Some(AuiFontStyle {
                    font_bundle_id: Some("font-ui".to_string()),
                    font_family_id: Some("family-ui".to_string()),
                    style: AuiFontStyleKind::Normal,
                    weight: 400,
                    raster_policy,
                }),
                sort_key: AuiOverlaySortKey {
                    canvas_layer: 0,
                    canvas_sorting_order: 0,
                    tree_order: 0,
                },
            }],
            report: AuiRenderReport {
                draw_command_count: 1,
                text_count: 1,
                ..AuiRenderReport::default()
            },
            glyph_plan: None,
        }
    }

    #[test]
    fn aui_font_style_round_trips_stack_override_style_weight_and_policy() {
        let style = AuiStyle {
            color: None,
            text_color: Some("#fff".to_string()),
            font_size: Some(40.0),
            font: Some(AuiFontStyle {
                font_bundle_id: Some("font-ui".to_string()),
                font_family_id: Some("family-ui".to_string()),
                style: AuiFontStyleKind::Italic,
                weight: 700,
                raster_policy: AuiFontRasterPolicy::Msdf,
            }),
        };
        let round_trip: AuiStyle =
            serde_json::from_slice(&serde_json::to_vec(&style).unwrap()).unwrap();
        assert_eq!(round_trip, style);
    }

    #[test]
    fn aui_text_glyph_plan_uses_v2_bundle_chinese_mode_page_and_kerning() {
        let registry = v2_font_registry();
        let bitmap = build_text_glyph_plan_from_bundles(
            &v2_text_overlay(16.0, AuiFontRasterPolicy::AutoHybrid),
            &registry,
        )
        .unwrap();
        assert_eq!(bitmap.requested_glyph_count, 3);
        assert_eq!(bitmap.rendered_glyph_count, 3);
        assert_eq!(bitmap.unsupported_glyph_count, 0);
        assert_eq!(bitmap.quads[1].rect.x, 8.0);
        assert!(
            bitmap
                .quads
                .iter()
                .all(|quad| quad.render_mode == FontBundleRenderMode::BitmapR8
                    && quad.page_index == 0)
        );

        let msdf = build_text_glyph_plan_from_bundles(
            &v2_text_overlay(40.0, AuiFontRasterPolicy::AutoHybrid),
            &registry,
        )
        .unwrap();
        assert!(msdf.quads.iter().all(
            |quad| quad.render_mode == FontBundleRenderMode::MsdfRgba8 && quad.page_index == 1
        ));
    }

    #[test]
    fn aui_text_glyph_plan_uses_bitmap_metrics_for_whitespace_without_msdf_outline() {
        let registry = v2_font_registry();
        let mut overlay = v2_text_overlay(40.0, AuiFontRasterPolicy::AutoHybrid);
        overlay.draw_items[0].text = Some("A V".to_string());

        let plan = build_text_glyph_plan_from_bundles(&overlay, &registry).unwrap();

        assert_eq!(plan.requested_glyph_count, 3);
        assert_eq!(plan.rendered_glyph_count, 3);
        assert_eq!(plan.unsupported_glyph_count, 0);
        assert_eq!(plan.quads[0].render_mode, FontBundleRenderMode::MsdfRgba8);
        assert_eq!(plan.quads[1].codepoint, u32::from(' '));
        assert_eq!(plan.quads[1].render_mode, FontBundleRenderMode::BitmapR8);
        assert_eq!(plan.quads[1].rect.width, 0.0);
        assert_eq!(plan.quads[2].render_mode, FontBundleRenderMode::MsdfRgba8);
        assert!(plan.quads[2].rect.x > plan.quads[1].rect.x);
    }

    #[test]
    fn aui_text_glyph_plan_uses_target_physical_pixels_for_auto_hybrid() {
        let registry = v2_font_registry();
        let presentation_720 = crate::game_view_presentation::GameViewPresentationModule::resolve(
            crate::game_view_presentation::GameViewPresentationSpec {
                session_id: "font-720".to_string(),
                target_id: "portrait".to_string(),
                target_extent: crate::game_view_presentation::GameViewExtent::new(720, 1280),
                display_rect: crate::game_view_presentation::GameViewRect::new(
                    0.0, 0.0, 720.0, 1280.0,
                ),
                scale_policy: crate::game_view_presentation::GameViewScalePolicy::Contain,
                surface_generation: 1,
                presentation_revision: 1,
                canvas_references: vec![crate::game_view_presentation::CanvasReferenceFact::new(
                    "main", 1080, 1920,
                )],
            },
        )
        .unwrap();
        let presentation_1080 = crate::game_view_presentation::GameViewPresentationModule::resolve(
            crate::game_view_presentation::GameViewPresentationSpec {
                session_id: "font-1080".to_string(),
                target_id: "portrait".to_string(),
                target_extent: crate::game_view_presentation::GameViewExtent::new(1080, 1920),
                display_rect: crate::game_view_presentation::GameViewRect::new(
                    0.0, 0.0, 1080.0, 1920.0,
                ),
                scale_policy: crate::game_view_presentation::GameViewScalePolicy::Contain,
                surface_generation: 1,
                presentation_revision: 1,
                canvas_references: vec![crate::game_view_presentation::CanvasReferenceFact::new(
                    "main", 1080, 1920,
                )],
            },
        )
        .unwrap();

        let plan_720 = build_text_glyph_plan_from_bundles_for_presentation(
            &v2_text_overlay(36.0, AuiFontRasterPolicy::AutoHybrid),
            &registry,
            Some(&presentation_720),
        )
        .unwrap();
        assert!(plan_720.quads.iter().all(|quad| {
            quad.render_mode == FontBundleRenderMode::BitmapR8
                && (quad.uv_rect[1] - 0.25).abs() < f32::EPSILON
        }));

        let plan_1080 = build_text_glyph_plan_from_bundles_for_presentation(
            &v2_text_overlay(36.0, AuiFontRasterPolicy::AutoHybrid),
            &registry,
            Some(&presentation_1080),
        )
        .unwrap();
        assert!(plan_1080
            .quads
            .iter()
            .all(|quad| quad.render_mode == FontBundleRenderMode::MsdfRgba8));

        let body_720 = build_text_glyph_plan_from_bundles_for_presentation(
            &v2_text_overlay(24.0, AuiFontRasterPolicy::AutoHybrid),
            &registry,
            Some(&presentation_720),
        )
        .unwrap();
        assert!(body_720.quads.iter().all(|quad| {
            quad.render_mode == FontBundleRenderMode::BitmapR8
                && quad.uv_rect[1].abs() < f32::EPSILON
        }));
    }

    #[test]
    fn aui_text_kerning_changes_second_glyph_position() {
        let registry = v2_font_registry();
        let plan = build_text_glyph_plan_from_bundles(
            &v2_text_overlay(16.0, AuiFontRasterPolicy::Bitmap),
            &registry,
        )
        .unwrap();
        let unkerned_second_x = 16.0 * 600_000.0 / 1_000_000.0;
        assert_eq!(plan.quads[1].rect.x, 8.0);
        assert!(plan.quads[1].rect.x < unkerned_second_x);
    }

    fn interaction_document() -> AuiDocument {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["button_a", "button_b", "label"]);
        let button_a = AuiNode::new(
            "button_a",
            AuiNodeKind::Button,
            AuiRect::fixed_position(100.0, 100.0, 240.0, 80.0),
        )
        .with_parent("root")
        .with_interactable(true);
        let button_b = AuiNode::new(
            "button_b",
            AuiNodeKind::Button,
            AuiRect::fixed_position(140.0, 120.0, 240.0, 80.0),
        )
        .with_parent("root")
        .with_interactable(true)
        .with_action(AuiActionRef::click("ui.pause"));
        let label = AuiNode::new(
            "label",
            AuiNodeKind::Text,
            AuiRect::fixed_position(20.0, 20.0, 100.0, 30.0),
        )
        .with_parent("root")
        .with_text("Info");
        AuiDocument::new(
            "interaction",
            vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
            vec![root, button_a, button_b, label],
        )
    }

    fn pointer_frame(events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
        let mut frame = RuntimeInputFrame::new(7, "game-view");
        frame.events = events;
        frame
    }

    fn unified_mouse(phase: RuntimePointerPhase, x: f32, y: f32) -> RuntimeInputEvent {
        let button = matches!(
            phase,
            RuntimePointerPhase::Down
                | RuntimePointerPhase::Up
                | RuntimePointerPhase::Held
                | RuntimePointerPhase::Cancel
        )
        .then_some(RuntimePointerButton::Primary);
        RuntimeInputEvent::Pointer {
            pointer: RuntimePointerEvent::mouse(phase, 0, x, y, button),
        }
    }

    fn unified_touch(
        phase: RuntimePointerPhase,
        pointer_id: u64,
        x: f32,
        y: f32,
    ) -> RuntimeInputEvent {
        RuntimeInputEvent::Pointer {
            pointer: RuntimePointerEvent::touch(phase, pointer_id, x, y),
        }
    }

    fn drag_document() -> AuiDocument {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["drag_source", "drop_target", "other_target"]);
        let drag_source = AuiNode::new(
            "drag_source",
            AuiNodeKind::Button,
            AuiRect::fixed_position(40.0, 40.0, 80.0, 80.0),
        )
        .with_parent("root")
        .with_draggable()
        .with_action(AuiActionRef::click("ui.source_click"))
        .with_action(AuiActionRef::drag_start("ui.drag_start"))
        .with_action(AuiActionRef::drag_move("ui.drag_move"))
        .with_action(AuiActionRef::drop("ui.drop"));
        let drop_target = AuiNode::new(
            "drop_target",
            AuiNodeKind::Button,
            AuiRect::fixed_position(180.0, 40.0, 100.0, 100.0),
        )
        .with_parent("root")
        .with_drop_target();
        let other_target = AuiNode::new(
            "other_target",
            AuiNodeKind::Button,
            AuiRect::fixed_position(320.0, 40.0, 100.0, 100.0),
        )
        .with_parent("root")
        .with_interactable(true);
        AuiDocument::new(
            "drag-doc",
            vec![AuiCanvas::screen_overlay("main", 640.0, 480.0, "root")],
            vec![root, drag_source, drop_target, other_target],
        )
    }

    fn modal_document() -> AuiDocument {
        let background_root = AuiNode::new(
            "background_root",
            AuiNodeKind::Panel,
            AuiRect::stretch_full(),
        )
        .with_children(["background_button"]);
        let background_button = AuiNode::new(
            "background_button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 20.0, 160.0, 60.0),
        )
        .with_parent("background_root")
        .with_interactable(true)
        .with_action(AuiActionRef::click("game.fire"));
        let modal_root = AuiNode::new(
            "modal_root",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(200.0, 120.0, 300.0, 240.0),
        )
        .with_children(["modal_button_a", "modal_button_b"]);
        let modal_button_a = AuiNode::new(
            "modal_button_a",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 20.0, 120.0, 50.0),
        )
        .with_parent("modal_root")
        .with_interactable(true)
        .with_action(AuiActionRef::focus("ui.focus_a"));
        let modal_button_b = AuiNode::new(
            "modal_button_b",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 90.0, 120.0, 50.0),
        )
        .with_parent("modal_root")
        .with_interactable(true)
        .with_action(AuiActionRef::focus("ui.focus_b"))
        .with_action(AuiActionRef::cancel("ui.cancel"));
        let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "background_root");
        let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal_root");
        modal.composition_stage = AuiCompositionStage::Modal;
        modal.layer = 10;
        AuiDocument::new(
            "modal-doc",
            vec![overlay, modal],
            vec![
                background_root,
                background_button,
                modal_root,
                modal_button_a,
                modal_button_b,
            ],
        )
    }

    fn scroll_document() -> AuiDocument {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["list"]);
        let list = AuiNode::new(
            "list",
            AuiNodeKind::ScrollView,
            AuiRect::fixed_position(10.0, 10.0, 120.0, 100.0),
        )
        .with_parent("root")
        .with_children(["item_0", "item_1", "item_2"])
        .with_action(AuiActionRef::scroll("ui.scroll"));
        let item_0 = AuiNode::new(
            "item_0",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0.0, 0.0, 120.0, 80.0),
        )
        .with_parent("list");
        let item_1 = AuiNode::new(
            "item_1",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0.0, 90.0, 120.0, 80.0),
        )
        .with_parent("list");
        let item_2 = AuiNode::new(
            "item_2",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0.0, 180.0, 120.0, 80.0),
        )
        .with_parent("list");
        AuiDocument::new(
            "scroll-doc",
            vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
            vec![root, list, item_0, item_1, item_2],
        )
    }

    fn navigation_text_entry_document() -> AuiDocument {
        let main_root = AuiNode::new("main_root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["play_button", "settings_button"]);
        let play_button = AuiNode::new(
            "play_button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 20.0, 160.0, 40.0),
        )
        .with_parent("main_root")
        .with_interactable(true)
        .with_action(AuiActionRef::submit("ui.play"));
        let settings_button = AuiNode::new(
            "settings_button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 80.0, 160.0, 40.0),
        )
        .with_parent("main_root")
        .with_interactable(true)
        .with_action(AuiActionRef::submit("ui.settings"));
        let pause_root = AuiNode::new(
            "pause_root",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(220.0, 40.0, 260.0, 180.0),
        )
        .with_children(["name_input", "resume_button"]);
        let name_input = AuiNode::new(
            "name_input",
            AuiNodeKind::InputField,
            AuiRect::fixed_position(20.0, 20.0, 180.0, 36.0),
        )
        .with_parent("pause_root")
        .with_interactable(true)
        .with_text("A")
        .with_action(AuiActionRef::text_changed("ui.name_changed"))
        .with_action(AuiActionRef::text_submitted("ui.name_submitted"))
        .with_action(AuiActionRef::text_cancelled("ui.name_cancelled"));
        let resume_button = AuiNode::new(
            "resume_button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 76.0, 180.0, 36.0),
        )
        .with_parent("pause_root")
        .with_interactable(true)
        .with_action(AuiActionRef::submit("ui.resume"));
        let mut main_canvas = AuiCanvas::screen_overlay("main", 640.0, 360.0, "main_root");
        main_canvas.default_focus_node_id = Some("play_button".to_string());
        let mut pause_canvas = AuiCanvas::screen_overlay("pause", 640.0, 360.0, "pause_root");
        pause_canvas.composition_stage = AuiCompositionStage::Modal;
        pause_canvas.layer = 10;
        pause_canvas.visible = false;
        pause_canvas.screen_id = Some("pause_screen".to_string());
        pause_canvas.default_focus_node_id = Some("name_input".to_string());
        pause_canvas.cancel_action_id = Some("ui.pause_cancel".to_string());
        AuiDocument::new(
            "navigation-text-entry-doc",
            vec![main_canvas, pause_canvas],
            vec![
                main_root,
                play_button,
                settings_button,
                pause_root,
                name_input,
                resume_button,
            ],
        )
    }

    fn font_registry_for_text(text: &str) -> RuntimeAuiFontAtlasRegistry {
        let mut chars = text.chars().collect::<std::collections::BTreeSet<_>>();
        chars.insert('?');
        let glyphs = chars
            .into_iter()
            .map(|ch| CookedFontAtlasGlyph {
                codepoint: ch as u32,
                glyph_id: format!("builtin-{:04X}", ch as u32),
                uv_rect: [0.0, 0.0, 0.625, 0.875],
                pixel_rect: [0, 0, 5, 7],
                bearing_x: 0.0,
                bearing_y: 7.0,
                advance: 6.0,
                page_index: 0,
            })
            .collect::<Vec<_>>();
        let metadata = CookedFontAtlasAsset {
            schema_version: COOKED_FONT_ATLAS_SCHEMA_VERSION.to_string(),
            font_atlas_id: "ui-default-cmin".to_string(),
            font_asset_id: "font-main".to_string(),
            font_source_kind: "engine_builtin_cooked_fallback".to_string(),
            font_asset_status: "placeholder".to_string(),
            atlas_image_path: "fonts/ui-default-cmin.fontatlas.r8".to_string(),
            atlas_format: "r8Alpha".to_string(),
            atlas_width: 8,
            atlas_height: 8,
            atlas_generation: 1,
            atlas_alpha_byte_len: 64,
            glyphs,
            fallback_used: true,
            diagnostics: Vec::new(),
        };
        let mut registry = RuntimeAuiFontAtlasRegistry::empty("test-package");
        registry.default_ui_font_atlas_id = Some(metadata.font_atlas_id.clone());
        registry.atlases_by_id.insert(
            metadata.font_atlas_id.clone(),
            RuntimeLoadedFontAtlas {
                metadata,
                atlas_alpha: vec![255; 64],
            },
        );
        registry
    }

    #[test]
    fn aui_layout_resolves_anchor_offsets_for_screen_overlay() {
        let (document, _) = sample_document();

        let layout = AuiLayoutEngine::layout(&document, 7);

        let root = layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "root")
            .unwrap();
        let panel = layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "panel")
            .unwrap();
        assert_eq!(
            root.rect,
            AuiComputedRect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0
            }
        );
        assert_eq!(
            panel.rect,
            AuiComputedRect {
                x: 100.0,
                y: 80.0,
                width: 320.0,
                height: 180.0
            }
        );
        assert_eq!(layout.report.frame, 7);
        assert_eq!(layout.report.visible_node_count, 4);
    }

    #[test]
    fn aui_draw_list_keeps_tree_order_for_panel_image_text() {
        let (document, manifest) = sample_document();
        assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);

        let layout = AuiLayoutEngine::layout(&document, 0);
        let (draw_list, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);

        assert_eq!(report.draw_command_count, 4);
        assert_eq!(report.image_count, 1);
        assert_eq!(report.text_count, 1);
        assert!(
            matches!(draw_list.commands[0], AuiDrawCommand::DrawRect { ref node_id, .. } if node_id == "root")
        );
        assert!(
            matches!(draw_list.commands[1], AuiDrawCommand::DrawRect { ref node_id, .. } if node_id == "panel")
        );
        assert!(
            matches!(draw_list.commands[2], AuiDrawCommand::DrawImage { ref node_id, .. } if node_id == "icon")
        );
        assert!(
            matches!(draw_list.commands[3], AuiDrawCommand::DrawText { ref node_id, .. } if node_id == "title")
        );
    }

    #[test]
    fn aui_render_report_exposes_projection_summary() {
        let (document, manifest) = sample_document();
        assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);
        let layout = AuiLayoutEngine::layout(&document, 0);

        let (_, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        let projection = report.projection_summary();

        assert_eq!(projection.kind, ProjectionKind::Ui);
        assert_eq!(projection.source_domain, ProjectionDomain::Ui);
        assert_eq!(projection.target_domain, ProjectionDomain::Render);
        assert_eq!(projection.projected_count, 4);
    }

    #[test]
    fn aui_validation_reports_missing_image_asset() {
        let (document, _) = sample_document();
        let manifest = AuiAssetManifest::new("empty", Vec::new());

        let report = AuiLayoutEngine::validate(&document, Some(&manifest));

        assert!(!report.ok);
        assert_eq!(report.missing_asset_count, 1);
        assert!(report
            .report_items
            .iter()
            .any(|item| item.code == "missing_image_asset"
                && item.node_id.as_deref() == Some("icon")
                && item.asset_id.as_deref() == Some("icon_asset")));
    }

    #[test]
    fn aui_node_kind_field_mismatch_validation() {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["icon_with_text", "text_with_image"]);
        let icon_with_text = AuiNode::new(
            "icon_with_text",
            AuiNodeKind::Image,
            AuiRect::fixed_position(10.0, 10.0, 64.0, 64.0),
        )
        .with_parent("root")
        .with_image("icon_asset")
        .with_text("Ignored");
        let text_with_image = AuiNode::new(
            "text_with_image",
            AuiNodeKind::Text,
            AuiRect::fixed_position(80.0, 10.0, 120.0, 32.0),
        )
        .with_parent("root")
        .with_text("Text")
        .with_image("text_icon_asset");
        let document = AuiDocument::new(
            "field_mismatch",
            vec![AuiCanvas::screen_overlay("main", 320.0, 200.0, "root")],
            vec![root, icon_with_text, text_with_image],
        );
        let manifest = AuiAssetManifest::new(
            "assets",
            vec![
                AuiAssetManifestEntry::image(
                    "icon_asset",
                    "asset://ui/icon.png",
                    vec!["icon_with_text".to_string()],
                ),
                AuiAssetManifestEntry::image(
                    "text_icon_asset",
                    "asset://ui/text-icon.png",
                    vec!["text_with_image".to_string()],
                ),
            ],
        );

        let report = AuiLayoutEngine::validate(&document, Some(&manifest));

        assert!(report.ok);
        assert_eq!(report.warning_count, 2);
        assert!(report.report_items.iter().any(|item| {
            item.code == "image_node_text_field_ignored"
                && item.node_id.as_deref() == Some("icon_with_text")
        }));
        assert!(report.report_items.iter().any(|item| {
            item.code == "text_node_image_field_ignored"
                && item.node_id.as_deref() == Some("text_with_image")
        }));
    }

    #[test]
    fn aui_validation_rejects_full_screen_single_image_ui() {
        let canvas = AuiCanvas::screen_overlay("main", 1920.0, 1080.0, "screenshot");
        let screenshot = AuiNode::new("screenshot", AuiNodeKind::Image, AuiRect::stretch_full())
            .with_image("full_screen_art");
        let document = AuiDocument::new("bad_ui", vec![canvas], vec![screenshot]);
        let manifest = AuiAssetManifest::new(
            "assets",
            vec![AuiAssetManifestEntry::image(
                "full_screen_art",
                "asset://ui/screenshot.png",
                vec!["screenshot".to_string()],
            )],
        );

        let report = AuiLayoutEngine::validate(&document, Some(&manifest));

        assert!(!report.ok);
        assert!(report.full_screen_image_rejected);
        assert!(report
            .report_items
            .iter()
            .any(|item| item.code == "full_screen_image_ui_rejected"));
    }

    #[test]
    fn aui_asset_manifest_maps_asset_to_node() {
        let (_, manifest) = sample_document();

        let asset = manifest
            .assets
            .iter()
            .find(|asset| asset.asset_id == "icon_asset")
            .unwrap();

        assert_eq!(asset.asset_ref, "asset://ui/icon.png");
        assert_eq!(asset.used_by_nodes, vec!["icon".to_string()]);
        assert_eq!(asset.text_policy, AuiAssetTextPolicy::RuntimeText);
    }

    #[test]
    fn aui_renderer_bridge_converts_draw_list_to_overlay_frame() {
        let (document, manifest) = sample_document();
        assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);
        let layout = AuiLayoutEngine::layout(&document, 3);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

        let overlay = AuiRendererBridge::build_overlay_frame(3, &draw_list);

        assert_eq!(overlay.frame_index, 3);
        assert_eq!(overlay.report.draw_command_count, 4);
        assert_eq!(overlay.report.image_count, 1);
        assert_eq!(overlay.report.text_count, 1);
        assert_eq!(overlay.draw_items[2].item_kind, AuiOverlayItemKind::Image);
        assert_eq!(
            overlay.draw_items[2].asset_id.as_deref(),
            Some("icon_asset")
        );
        assert_eq!(overlay.draw_items[3].item_kind, AuiOverlayItemKind::Text);
        assert_eq!(overlay.draw_items[3].text.as_deref(), Some("Start"));
    }

    #[test]
    fn aui_renderer_bridge_keeps_overlay_tree_order() {
        let (document, manifest) = sample_document();
        assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);
        let layout = AuiLayoutEngine::layout(&document, 0);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

        let overlay = AuiRendererBridge::build_overlay_frame(0, &draw_list);

        let order = overlay
            .draw_items
            .iter()
            .map(|item| (item.node_id.as_str(), item.sort_key.tree_order))
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![("root", 0), ("panel", 1), ("icon", 2), ("title", 3)]
        );
    }

    #[test]
    fn aui_composition_frame_splits_screen_space_canvas_stages() {
        let mut before = AuiCanvas::screen_overlay("before", 800.0, 600.0, "before-root");
        before.composition_stage = AuiCompositionStage::BeforeWorld;
        let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "overlay-root");
        let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal-root");
        modal.composition_stage = AuiCompositionStage::Modal;
        let document = AuiDocument::new(
            "stage-doc",
            vec![before, overlay, modal],
            vec![
                AuiNode::new("before-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
                AuiNode::new("overlay-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
                AuiNode::new("modal-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            ],
        );
        let layout = AuiLayoutEngine::layout(&document, 1);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

        let composition =
            AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw_list);

        assert_eq!(composition.report.stage_count, 3);
        assert_eq!(composition.report.before_world_item_count, 1);
        assert_eq!(composition.report.screen_overlay_item_count, 1);
        assert_eq!(composition.report.modal_item_count, 1);
        assert_eq!(
            composition
                .canvas_references
                .iter()
                .map(|fact| (
                    fact.canvas_id.as_str(),
                    fact.reference_extent.width,
                    fact.reference_extent.height,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("before", 800, 600),
                ("overlay", 800, 600),
                ("modal", 800, 600),
            ]
        );
        assert_eq!(
            composition
                .stage(AuiCompositionStage::BeforeWorld)
                .expect("before world stage")
                .draw_items[0]
                .composition_stage,
            AuiCompositionStage::BeforeWorld
        );
        assert!(composition.report.diagnostics.is_empty());
    }

    #[test]
    fn aui_composition_frame_uses_canvas_layer_and_sorting_order() {
        let mut later = AuiCanvas::screen_overlay("later", 800.0, 600.0, "later-root");
        later.layer = 3;
        later.sorting_order = 20;
        let mut earlier = AuiCanvas::screen_overlay("earlier", 800.0, 600.0, "earlier-root");
        earlier.layer = -1;
        earlier.sorting_order = 2;
        let document = AuiDocument::new(
            "sort-doc",
            vec![later, earlier],
            vec![
                AuiNode::new("later-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
                AuiNode::new("earlier-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            ],
        );
        let layout = AuiLayoutEngine::layout(&document, 1);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

        let composition =
            AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw_list);
        let items = &composition
            .stage(AuiCompositionStage::ScreenOverlay)
            .expect("screen overlay stage")
            .draw_items;

        assert_eq!(items[0].canvas_id, "earlier");
        assert_eq!(items[0].sort_key.canvas_layer, -1);
        assert_eq!(items[0].sort_key.canvas_sorting_order, 2);
        assert_eq!(items[1].canvas_id, "later");
        assert_eq!(items[1].sort_key.canvas_layer, 3);
        assert_eq!(items[1].sort_key.canvas_sorting_order, 20);
    }

    #[test]
    fn aui_runtime_presenter_generates_glyph_plan_from_loaded_font_atlas() {
        let (document, _) = sample_document();
        let registry = font_registry_for_text("Start");
        let snapshot_output = ProjectUiStateSnapshotOutput::new(
            "test_snapshot",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(9),
        );

        let output = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
            &document,
            snapshot_output,
            &registry,
        );

        assert_eq!(output.report.status, AuiRuntimePresentStatus::Success);
        assert!(output.report.glyph_present);
        assert!(output.report.font_atlas_present);
        assert_eq!(
            output.report.font_atlas_id.as_deref(),
            Some("ui-default-cmin")
        );
        assert_eq!(output.report.requested_glyph_count, 5);
        assert_eq!(output.report.rendered_glyph_count, 5);
        assert_eq!(output.report.unsupported_glyph_count, 0);
        assert!(output.report.text_pass_inserted);
        assert!(output.report.glyph_plan_hash.is_some());
        assert!(output.overlay.glyph_plan.is_some());
    }

    #[test]
    fn aui_runtime_resolver_updates_text_progress_visible_and_image() {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["score", "hp", "pause_overlay", "warning"]);
        let score = AuiNode::new(
            "score",
            AuiNodeKind::Text,
            AuiRect::fixed_position(16.0, 16.0, 180.0, 32.0),
        )
        .with_parent("root")
        .with_text("Score: 0")
        .with_binding(AuiBindingRef::new(
            "bind.score",
            AuiBindingTarget::TextText,
            "game.score_text",
            Some(AuiBindingValue::String("Score: 0".to_string())),
        ));
        let hp = AuiNode::new(
            "hp",
            AuiNodeKind::ProgressBar,
            AuiRect::fixed_position(16.0, 56.0, 200.0, 20.0),
        )
        .with_parent("root")
        .with_progress_value(1.0)
        .with_binding(AuiBindingRef::new(
            "bind.hp",
            AuiBindingTarget::ProgressBarValue,
            "player.hp_ratio",
            Some(AuiBindingValue::Number(1.0)),
        ));
        let pause_overlay =
            AuiNode::new("pause_overlay", AuiNodeKind::Panel, AuiRect::stretch_full())
                .with_parent("root")
                .with_binding(AuiBindingRef::new(
                    "bind.pause_visible",
                    AuiBindingTarget::PanelVisible,
                    "game.paused",
                    Some(AuiBindingValue::Bool(false)),
                ));
        let warning = AuiNode::new(
            "warning",
            AuiNodeKind::Image,
            AuiRect::fixed_position(240.0, 16.0, 48.0, 48.0),
        )
        .with_parent("root")
        .with_image("warning_default")
        .with_binding(AuiBindingRef::new(
            "bind.warning_asset",
            AuiBindingTarget::ImageAssetRef,
            "warning.icon",
            None,
        ));
        let document = AuiDocument::new(
            "hud",
            vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
            vec![root, score, hp, pause_overlay, warning],
        );
        let snapshot = ProjectUiStateSnapshot::new(42)
            .with_value(
                "game.score_text",
                AuiBindingValue::String("Score: 1200".to_string()),
            )
            .with_value("player.hp_ratio", AuiBindingValue::Number(0.35))
            .with_value("game.paused", AuiBindingValue::Bool(true))
            .with_value(
                "warning.icon",
                AuiBindingValue::AssetRef(AuiAssetRef::new("warning_low_hp")),
            );

        let (resolved, report) = AuiRuntimeResolver::resolve_bindings(&document, &snapshot);

        assert!(report.ok());
        assert_eq!(report.binding_count, 4);
        assert_eq!(report.resolved_count, 4);
        let score = resolved
            .nodes
            .iter()
            .find(|node| node.node_id == "score")
            .unwrap();
        let hp = resolved
            .nodes
            .iter()
            .find(|node| node.node_id == "hp")
            .unwrap();
        let overlay = resolved
            .nodes
            .iter()
            .find(|node| node.node_id == "pause_overlay")
            .unwrap();
        let warning = resolved
            .nodes
            .iter()
            .find(|node| node.node_id == "warning")
            .unwrap();
        assert_eq!(score.text.as_deref(), Some("Score: 1200"));
        assert_eq!(hp.progress_value, Some(0.35));
        assert!(overlay.visible);
        assert_eq!(
            warning.image.as_ref().map(|image| image.asset_id.as_str()),
            Some("warning_low_hp")
        );
    }

    #[test]
    fn aui_progress_bar_uses_composite_draw_commands() {
        let root =
            AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["hp"]);
        let hp = AuiNode::new(
            "hp",
            AuiNodeKind::ProgressBar,
            AuiRect::fixed_position(10.0, 10.0, 200.0, 20.0),
        )
        .with_parent("root")
        .with_progress_value(0.25);
        let document = AuiDocument::new(
            "hud",
            vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
            vec![root, hp],
        );

        let layout = AuiLayoutEngine::layout(&document, 1);
        let (draw_list, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);

        assert_eq!(report.draw_command_count, 3);
        assert!(matches!(
            draw_list.commands[1],
            AuiDrawCommand::DrawRect { ref node_id, .. } if node_id == "hp:track"
        ));
        match &draw_list.commands[2] {
            AuiDrawCommand::DrawRect { node_id, rect, .. } => {
                assert_eq!(node_id, "hp:fill");
                assert_eq!(rect.width, 50.0);
            }
            other => panic!("expected progress fill rect, got {:?}", other),
        }
    }

    #[test]
    fn aui_hit_test_picks_topmost_interactable_node() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);

        let hit = AuiInteractionSystem::hit_test(&document, &layout, 150.0, 130.0);

        assert_eq!(hit.hit_node.as_deref(), Some("button_b"));
        assert!(hit.consumed);
        assert_eq!(hit.reason, AuiHitTestReason::HitInteractable);
    }

    #[test]
    fn aui_hit_test_ignores_invisible_or_non_interactable_nodes() {
        let mut document = interaction_document();
        document
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "button_b")
            .expect("button_b")
            .visible = false;
        document
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "button_a")
            .expect("button_a")
            .interactable = false;
        let layout = AuiLayoutEngine::layout(&document, 1);

        let hit = AuiInteractionSystem::hit_test(&document, &layout, 150.0, 130.0);

        assert_eq!(hit.hit_node.as_deref(), Some("button_a"));
        assert!(!hit.consumed);
        assert_eq!(hit.reason, AuiHitTestReason::HitNonInteractable);
    }

    #[test]
    fn aui_control_interaction_mouse_capture_tracks_inside_and_exactly_one_click() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let mut state = AuiInteractionState::default();

        let hover = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(
            hover.control_snapshot.hovered_node.as_deref(),
            Some("button_b")
        );
        assert_eq!(hover.control_snapshot.pressed_node, None);

        let down = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(
            down.control_snapshot.pressed_node.as_deref(),
            Some("button_b")
        );
        assert!(down.control_snapshot.pressed_inside);
        assert_eq!(
            down.control_snapshot.pointer_device_kind,
            Some(RuntimePointerDeviceKind::Mouse)
        );

        let outside = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 700.0, 500.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(outside.control_snapshot.hovered_node, None);
        assert_eq!(
            outside.control_snapshot.pressed_node.as_deref(),
            Some("button_b")
        );
        assert!(!outside.control_snapshot.pressed_inside);
        assert_eq!(outside.consumed_event_indices, vec![0]);

        let inside = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert!(inside.control_snapshot.pressed_inside);

        let up = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Up, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(up.control_snapshot.pressed_node, None);
        assert_eq!(
            up.commands
                .iter()
                .filter(|command| command.command_kind == AuiCommandKind::Click)
                .count(),
            1
        );
        assert_eq!(
            up.actions
                .iter()
                .filter(|action| action.action_id == "ui.pause")
                .count(),
            1
        );
    }

    #[test]
    fn aui_control_interaction_pointer_up_outside_clears_capture_without_click() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let mut state = AuiInteractionState::default();
        AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );

        let up = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Up, 700.0, 500.0)]),
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(up.control_snapshot.pressed_node, None);
        assert_eq!(up.consumed_event_indices, vec![0]);
        assert!(!up
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::Click));
    }

    #[test]
    fn aui_control_interaction_touch_click_and_cancel_leave_no_hover_residue() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let mut click_state = AuiInteractionState::default();
        let down = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_touch(
                RuntimePointerPhase::Down,
                42,
                150.0,
                130.0,
            )]),
            &mut click_state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(down.control_snapshot.hovered_node, None);
        assert_eq!(down.control_snapshot.pointer_id, Some(42));
        assert_eq!(
            down.control_snapshot.pointer_device_kind,
            Some(RuntimePointerDeviceKind::Touch)
        );
        let up = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_touch(
                RuntimePointerPhase::Up,
                42,
                150.0,
                130.0,
            )]),
            &mut click_state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(up.control_snapshot.hovered_node, None);
        assert_eq!(up.control_snapshot.pressed_node, None);
        assert_eq!(
            up.commands
                .iter()
                .filter(|command| command.command_kind == AuiCommandKind::Click)
                .count(),
            1
        );

        let mut cancel_state = AuiInteractionState::default();
        AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_touch(
                RuntimePointerPhase::Down,
                43,
                150.0,
                130.0,
            )]),
            &mut cancel_state,
            AuiInteractionConfig::default(),
        );
        let cancel = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_touch(
                RuntimePointerPhase::Cancel,
                43,
                150.0,
                130.0,
            )]),
            &mut cancel_state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(cancel.control_snapshot.hovered_node, None);
        assert_eq!(cancel.control_snapshot.pressed_node, None);
        assert_eq!(cancel.consumed_event_indices, vec![0]);
        assert!(cancel
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::PointerCancel));
        assert!(!cancel
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::Click));
    }

    #[test]
    fn aui_control_interaction_reconciles_disabled_removed_modal_and_session_change() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let mut state = AuiInteractionState::default();
        AuiInteractionSystem::process_session_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
            "session-a",
        );

        let session_changed = AuiInteractionSystem::process_session_with_state(
            &document,
            &layout,
            &pointer_frame(Vec::new()),
            &mut state,
            AuiInteractionConfig::default(),
            "session-b",
        );
        assert_eq!(session_changed.control_snapshot.pressed_node, None);
        assert!(session_changed.control_reconciliation_count >= 1);

        AuiInteractionSystem::process_session_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
            "session-b",
        );
        let mut disabled = document.clone();
        disabled
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "button_b")
            .unwrap()
            .interactable = false;
        let disabled_layout = AuiLayoutEngine::layout(&disabled, 2);
        let disabled_result = AuiInteractionSystem::process_session_with_state(
            &disabled,
            &disabled_layout,
            &pointer_frame(Vec::new()),
            &mut state,
            AuiInteractionConfig::default(),
            "session-b",
        );
        assert_eq!(disabled_result.control_snapshot.pressed_node, None);
        assert!(disabled_result.control_reconciliation_count >= 1);

        AuiInteractionSystem::process_session_with_state(
            &document,
            &layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
            &mut state,
            AuiInteractionConfig::default(),
            "session-b",
        );
        let mut removed = document.clone();
        removed.nodes.retain(|node| node.node_id != "button_b");
        removed
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "root")
            .unwrap()
            .children
            .retain(|node_id| node_id != "button_b");
        let removed_layout = AuiLayoutEngine::layout(&removed, 3);
        let removed_result = AuiInteractionSystem::process_session_with_state(
            &removed,
            &removed_layout,
            &pointer_frame(Vec::new()),
            &mut state,
            AuiInteractionConfig::default(),
            "session-b",
        );
        assert_eq!(removed_result.control_snapshot.pressed_node, None);
        assert!(removed_result.control_reconciliation_count >= 1);

        let modal = modal_document();
        let mut modal_state = AuiInteractionState::default();
        modal_state
            .canvas_visibility_overrides
            .insert("modal".to_string(), false);
        let background_layout =
            AuiLayoutEngine::layout_with_interaction_state(&modal, 1, &modal_state);
        AuiInteractionSystem::process_with_state(
            &modal,
            &background_layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 40.0, 40.0)]),
            &mut modal_state,
            AuiInteractionConfig::default(),
        );
        modal_state
            .canvas_visibility_overrides
            .insert("modal".to_string(), true);
        let modal_layout = AuiLayoutEngine::layout_with_interaction_state(&modal, 2, &modal_state);
        let modal_result = AuiInteractionSystem::process_with_state(
            &modal,
            &modal_layout,
            &pointer_frame(Vec::new()),
            &mut modal_state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(modal_result.control_snapshot.pressed_node, None);
        assert!(modal_result.control_reconciliation_count >= 1);

        let mut screen_document = navigation_text_entry_document();
        screen_document
            .canvases
            .iter_mut()
            .find(|canvas| canvas.canvas_id == "pause")
            .unwrap()
            .composition_stage = AuiCompositionStage::ScreenOverlay;
        let mut screen_state = AuiInteractionState::default();
        let main_layout =
            AuiLayoutEngine::layout_with_interaction_state(&screen_document, 1, &screen_state);
        AuiInteractionSystem::process_with_state(
            &screen_document,
            &main_layout,
            &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 40.0, 40.0)]),
            &mut screen_state,
            AuiInteractionConfig::default(),
        );
        AuiInteractionSystem::push_screen(&screen_document, &mut screen_state, "pause_screen")
            .expect("pause screen");
        let pause_layout =
            AuiLayoutEngine::layout_with_interaction_state(&screen_document, 2, &screen_state);
        let screen_result = AuiInteractionSystem::process_with_state(
            &screen_document,
            &pause_layout,
            &pointer_frame(Vec::new()),
            &mut screen_state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(screen_result.control_snapshot.pressed_node, None);
        assert_eq!(
            screen_result.control_snapshot.active_screen_id.as_deref(),
            Some("pause_screen")
        );
        assert!(screen_result.control_reconciliation_count >= 1);
    }

    #[test]
    fn aui_control_interaction_keyboard_and_gamepad_submit_remain_exactly_once() {
        let document = navigation_text_entry_document();
        let layout = AuiLayoutEngine::layout(&document, 1);

        for event in [
            RuntimeInputEvent::KeyDown {
                key: "Enter".to_string(),
            },
            RuntimeInputEvent::GamepadButtonDown {
                gamepad_id: 0,
                button: "South".to_string(),
            },
        ] {
            let mut state = AuiInteractionState::default();
            state.focus.focused_node = Some("play_button".to_string());
            state.focus.focus_reason = AuiFocusReason::Keyboard;
            let result = AuiInteractionSystem::process_with_state(
                &document,
                &layout,
                &pointer_frame(vec![event]),
                &mut state,
                AuiInteractionConfig::default(),
            );
            assert_eq!(
                result
                    .commands
                    .iter()
                    .filter(|command| command.command_kind == AuiCommandKind::Submit)
                    .count(),
                1
            );
            assert_eq!(
                result
                    .actions
                    .iter()
                    .filter(|action| action.action_id == "ui.play")
                    .count(),
                1
            );
            assert_eq!(
                result.control_snapshot.focused_node.as_deref(),
                Some("play_button")
            );
            assert!(result.control_snapshot.focus_visible);
        }
    }

    #[test]
    fn aui_pointer_move_keeps_default_focus_logical_without_keyboard_focus_visual() {
        let document = navigation_text_entry_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::PointerMove { x: 40.0, y: 40.0 }]);
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(result.default_focus_applied_count, 1);
        assert_eq!(state.focus.focused_node.as_deref(), Some("play_button"));
        assert_eq!(state.focus.focus_reason, AuiFocusReason::Pointer);
        assert!(!result.control_snapshot.focus_visible);
        assert_eq!(
            result.control_snapshot.hovered_node.as_deref(),
            Some("play_button")
        );
    }

    #[test]
    fn aui_interaction_consumes_pointer_down_on_button() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::PointerDown {
            x: 150.0,
            y: 130.0,
            button: RuntimePointerButton::Primary,
        }]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert!(result.consumed);
        assert!(result.commands.iter().any(|command| {
            command.source_node == "button_b" && command.command_kind == AuiCommandKind::PointerDown
        }));
        assert!(result.commands.iter().any(|command| {
            command.source_node == "button_b" && command.command_kind == AuiCommandKind::Focus
        }));
    }

    #[test]
    fn aui_interaction_does_not_consume_pointer_outside_ui() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::PointerDown {
            x: 700.0,
            y: 500.0,
            button: RuntimePointerButton::Primary,
        }]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert!(!result.consumed);
        assert!(result.commands.is_empty());
        assert_eq!(
            result.traces[0].reason,
            AuiHitTestReason::HitNonInteractable
        );
    }

    #[test]
    fn aui_interaction_pointer_up_generates_click_when_pressed_same_node() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 150.0,
                y: 130.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerUp {
                x: 150.0,
                y: 130.0,
                button: RuntimePointerButton::Primary,
            },
        ]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert!(result.consumed);
        assert!(result
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::Click
                && command.source_node == "button_b"));
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_id, "ui.pause");
        assert_eq!(result.actions[0].node_id, "button_b");
    }

    #[test]
    fn aui_interaction_maps_target_space_per_canvas_and_filters_original_indices() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "session-portrait".to_string(),
            target_id: "game-view".to_string(),
            target_extent: GameViewExtent::new(720, 1280),
            display_rect: GameViewRect::new(0.0, 0.0, 720.0, 1280.0),
            scale_policy: GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![CanvasReferenceFact::new("main", 800, 600)],
        })
        .expect("portrait presentation");
        let target_point = presentation
            .reference_to_target("main", GameViewPoint::new(150.0, 130.0))
            .expect("button point maps to target");
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerMove { x: 10.0, y: 10.0 },
            RuntimeInputEvent::PointerDown {
                x: target_point.x,
                y: target_point.y,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerUp {
                x: target_point.x,
                y: target_point.y,
                button: RuntimePointerButton::Primary,
            },
        ]);
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_target_space_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
            &presentation,
        );
        let gameplay_frame = frame.filter_consumed_events(&result.consumed_event_indices);

        assert_eq!(result.consumed_event_indices, vec![1, 2]);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_id, "ui.pause");
        assert_eq!(gameplay_frame.events, vec![frame.events[0].clone()]);
        assert_eq!(
            frame.events[1],
            RuntimeInputEvent::PointerDown {
                x: target_point.x,
                y: target_point.y,
                button: RuntimePointerButton::Primary,
            }
        );
    }

    #[test]
    fn aui_interaction_trace_records_consumed_reason() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::PointerMove { x: 150.0, y: 130.0 }]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert_eq!(result.traces.len(), 1);
        assert_eq!(result.traces[0].frame, 7);
        assert_eq!(
            result.traces[0].event_kind,
            AuiInteractionEventKind::PointerMove
        );
        assert_eq!(result.traces[0].hit_node.as_deref(), Some("button_b"));
        assert!(result.traces[0].consumed);
        assert_eq!(result.traces[0].reason, AuiHitTestReason::HitInteractable);
        assert_eq!(result.traces[0].command_count, 2);
    }

    #[test]
    fn aui_drag_generates_drop_payload_and_actions() {
        let document = drag_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 60.0,
                y: 60.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerMove { x: 120.0, y: 60.0 },
            RuntimeInputEvent::PointerUp {
                x: 220.0,
                y: 80.0,
                button: RuntimePointerButton::Primary,
            },
        ]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert!(result.consumed);
        assert_eq!(result.consumed_event_indices, vec![0, 1, 2]);
        assert!(result
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::DragStart));
        let drop_command = result
            .commands
            .iter()
            .find(|command| command.command_kind == AuiCommandKind::Drop)
            .expect("drop command");
        let payload: AuiDragDropPayload =
            serde_json::from_str(drop_command.payload.as_deref().unwrap()).unwrap();
        assert_eq!(payload.schema_version, "aui-drag-drop-payload.v1");
        assert_eq!(payload.source_node, "drag_source");
        assert_eq!(payload.target_node.as_deref(), Some("drop_target"));
        assert_eq!(payload.drag_phase, "drop");
        assert!(result
            .actions
            .iter()
            .any(|action| action.event == AuiActionEvent::Drop
                && action.action_id == "ui.drop"
                && action.payload.is_some()));
    }

    #[test]
    fn aui_drag_cancel_to_empty_area_does_not_generate_click() {
        let document = drag_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 60.0,
                y: 60.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerMove { x: 140.0, y: 60.0 },
            RuntimeInputEvent::PointerUp {
                x: 500.0,
                y: 400.0,
                button: RuntimePointerButton::Primary,
            },
        ]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert!(result
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::DragCancel));
        assert!(!result
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::Click));
        assert!(result
            .actions
            .iter()
            .all(|action| action.event != AuiActionEvent::Click));
    }

    #[test]
    fn aui_drag_requires_primary_pointer() {
        let document = drag_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 60.0,
                y: 60.0,
                button: RuntimePointerButton::Secondary,
            },
            RuntimeInputEvent::PointerMove { x: 140.0, y: 60.0 },
            RuntimeInputEvent::PointerUp {
                x: 220.0,
                y: 80.0,
                button: RuntimePointerButton::Secondary,
            },
        ]);

        let result = AuiInteractionSystem::process(&document, &layout, &frame);

        assert!(!result.commands.iter().any(|command| matches!(
            command.command_kind,
            AuiCommandKind::DragStart | AuiCommandKind::DragMove | AuiCommandKind::Drop
        )));
    }

    #[test]
    fn aui_modal_blocks_pointer_wheel_and_key_outside_modal_root() {
        let document = modal_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let mut frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 40.0,
                y: 40.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::MouseWheel { delta: -1.0 },
            RuntimeInputEvent::KeyDown {
                key: "Space".to_string(),
            },
        ]);
        frame.pointer_position = Some(crate::input_action::PointerPosition { x: 40.0, y: 40.0 });
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(state.active_modal_root.as_deref(), Some("modal_root"));
        assert_eq!(state.focus.focus_scope_root.as_deref(), Some("modal_root"));
        assert_eq!(result.consumed_event_indices, vec![0, 1, 2]);
        assert_eq!(
            result.consumed_event_count_by_kind.get("PointerDown"),
            Some(&1)
        );
        assert_eq!(
            result.consumed_event_count_by_kind.get("MouseWheel"),
            Some(&1)
        );
        assert_eq!(result.consumed_event_count_by_kind.get("KeyDown"), Some(&1));
        assert!(result.commands.is_empty());
    }

    #[test]
    fn aui_focus_trap_cycles_tab_and_escape_generates_cancel() {
        let document = modal_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 230.0,
                y: 150.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::KeyDown {
                key: "Tab".to_string(),
            },
            RuntimeInputEvent::KeyDown {
                key: "Escape".to_string(),
            },
        ]);
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(state.focus.focus_scope_root.as_deref(), Some("modal_root"));
        assert_eq!(state.focus.focused_node.as_deref(), Some("modal_button_b"));
        assert!(result
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::Focus
                && command.source_node == "modal_button_b"));
        assert!(result
            .commands
            .iter()
            .any(|command| command.command_kind == AuiCommandKind::Cancel
                && command.source_node == "modal_button_b"));
        assert!(result.actions.iter().any(
            |action| action.event == AuiActionEvent::Cancel && action.action_id == "ui.cancel"
        ));
    }

    #[test]
    fn aui_scroll_wheel_updates_offset_and_layout_rects() {
        let document = scroll_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let before_item_0_y = layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "item_0")
            .expect("item_0")
            .rect
            .y;
        let mut frame = pointer_frame(vec![RuntimeInputEvent::MouseWheel { delta: -1.0 }]);
        frame.pointer_position = Some(crate::input_action::PointerPosition { x: 20.0, y: 20.0 });
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        let layout_after =
            AuiLayoutEngine::layout_with_scroll_offsets(&document, 2, &state.scroll_offsets);
        let after_item_0_y = layout_after
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "item_0")
            .expect("item_0")
            .rect
            .y;

        assert_eq!(
            result.consumed_event_count_by_kind.get("MouseWheel"),
            Some(&1)
        );
        assert!(result.scroll_offset_change_count > 0);
        assert!(layout_after.report.scroll_offset_applied);
        assert!(layout_after.report.scroll_applied_node_count >= 3);
        assert!(layout_after.report.clipped_node_count > 0);
        assert!(after_item_0_y < before_item_0_y);
    }

    #[test]
    fn aui_scroll_drag_updates_offset() {
        let document = scroll_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 20.0,
                y: 80.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerMove { x: 20.0, y: 20.0 },
            RuntimeInputEvent::PointerUp {
                x: 20.0,
                y: 20.0,
                button: RuntimePointerButton::Primary,
            },
        ]);
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(result.consumed_event_indices, vec![0, 1, 2]);
        assert!(result.scroll_offset_change_count > 0);
        assert!(state
            .scroll_offsets
            .get("list")
            .is_some_and(|scroll| scroll.offset_y > 0.0));
        assert!(state.active_scroll_capture().is_none());
    }

    #[test]
    fn aui_clip_layout_extracts_effective_clip_and_culls_offscreen_items() {
        let document = scroll_document();

        let layout = AuiLayoutEngine::layout(&document, 1);
        let (draw_list, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        let overlay = AuiRendererBridge::build_overlay_frame(1, &draw_list);

        assert_eq!(layout.report.clip_root_count, 1);
        assert!(layout.report.effective_clip_node_count > 0);
        assert!(layout.report.clipped_node_count > 0);
        assert!(report.effective_clip_item_count > 0);
        assert!(report.culled_draw_item_count > 0);
        assert_eq!(report.scrollbar_visible_count, 1);
        assert_eq!(overlay.report.scrollbar_visible_count, 1);
        assert!(overlay
            .draw_items
            .iter()
            .any(|item| item.item_kind == AuiOverlayItemKind::ScrollbarTrack));
        assert!(overlay
            .draw_items
            .iter()
            .any(|item| item.item_kind == AuiOverlayItemKind::ScrollbarThumb));
        assert!(layout.scrollbar_metrics.iter().any(|metrics| {
            metrics.scroll_node_id == "list" && metrics.visible && metrics.max_offset_y > 0.0
        }));
        assert!(!draw_list.commands.iter().any(|command| {
            matches!(command, AuiDrawCommand::DrawRect { node_id, .. } if node_id == "item_2")
        }));
    }

    #[test]
    fn aui_hit_test_rejects_clipped_scroll_content() {
        let document = scroll_document();
        let layout = AuiLayoutEngine::layout(&document, 1);

        let hit = AuiInteractionSystem::hit_test(&document, &layout, 20.0, 200.0);

        assert_ne!(hit.hit_node.as_deref(), Some("item_2"));
        assert!(hit.clip_rejected_count > 0);
    }

    #[test]
    fn aui_scrollbar_thumb_drag_updates_offset() {
        let document = scroll_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let metrics = layout
            .scrollbar_metrics
            .iter()
            .find(|metrics| metrics.scroll_node_id == "list" && metrics.visible)
            .expect("visible list scrollbar");
        let thumb_x = metrics.thumb_rect.x + metrics.thumb_rect.width * 0.5;
        let thumb_y = metrics.thumb_rect.y + metrics.thumb_rect.height * 0.5;
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: thumb_x,
                y: thumb_y,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerMove {
                x: thumb_x,
                y: thumb_y + 20.0,
            },
            RuntimeInputEvent::PointerUp {
                x: thumb_x,
                y: thumb_y + 20.0,
                button: RuntimePointerButton::Primary,
            },
        ]);
        let mut state = AuiInteractionState::default();

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert!(result.scroll_offset_change_count > 0);
        assert!(result.traces.iter().any(|trace| {
            trace
                .captured_node
                .as_deref()
                .is_some_and(|node| node.ends_with(":scrollbar-thumb"))
        }));
        assert!(state
            .scroll_offsets
            .get("list")
            .is_some_and(|scroll| scroll.offset_y > 0.0));
        assert!(state.active_scroll_capture().is_none());
    }

    #[test]
    fn aui_navigation_arrow_down_moves_focus_and_scrolls_visible() {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["list"]);
        let list = AuiNode::new(
            "list",
            AuiNodeKind::ScrollView,
            AuiRect::fixed_position(10.0, 10.0, 160.0, 100.0),
        )
        .with_parent("root")
        .with_children(["item_0", "item_1", "item_2"]);
        let item_0 = AuiNode::new(
            "item_0",
            AuiNodeKind::Button,
            AuiRect::fixed_position(0.0, 0.0, 140.0, 40.0),
        )
        .with_parent("list")
        .with_interactable(true);
        let item_1 = AuiNode::new(
            "item_1",
            AuiNodeKind::Button,
            AuiRect::fixed_position(0.0, 50.0, 140.0, 40.0),
        )
        .with_parent("list")
        .with_interactable(true);
        let item_2 = AuiNode::new(
            "item_2",
            AuiNodeKind::Button,
            AuiRect::fixed_position(0.0, 150.0, 140.0, 40.0),
        )
        .with_parent("list")
        .with_interactable(true);
        let document = AuiDocument::new(
            "navigation-scroll-doc",
            vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
            vec![root, list, item_0, item_1, item_2],
        );
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
            key: "ArrowDown".to_string(),
        }]);
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("item_1".to_string());
        state.focus.focus_reason = AuiFocusReason::Keyboard;

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(state.focus.focused_node.as_deref(), Some("item_2"));
        assert_eq!(result.keyboard_navigation_event_count, 1);
        assert_eq!(result.focus_visible_scroll_count, 1);
        assert_eq!(result.scroll_offset_change_count, 1);
        assert!(state
            .scroll_offsets
            .get("list")
            .is_some_and(|scroll| scroll.offset_y > 0.0));
        assert!(result.commands.iter().any(|command| {
            command.command_kind == AuiCommandKind::Focus && command.source_node == "item_2"
        }));
        assert!(result.commands.iter().any(|command| {
            command.command_kind == AuiCommandKind::Scroll && command.source_node == "list"
        }));
    }

    #[test]
    fn aui_rectclip_scrollbar_navigation_report_collects_gate_evidence() {
        let document = scroll_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let (_, render_report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        let metrics = layout
            .scrollbar_metrics
            .iter()
            .find(|metrics| metrics.scroll_node_id == "list" && metrics.visible)
            .expect("visible list scrollbar");
        let thumb_x = metrics.thumb_rect.x + metrics.thumb_rect.width * 0.5;
        let thumb_y = metrics.thumb_rect.y + metrics.thumb_rect.height * 0.5;
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: thumb_x,
                y: thumb_y,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerMove {
                x: thumb_x,
                y: thumb_y + 20.0,
            },
        ]);
        let mut state = AuiInteractionState::default();
        let mut interaction = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        let clipped_hit = AuiInteractionSystem::hit_test(&document, &layout, 20.0, 200.0);
        interaction.hit_test_clip_rejected_count += clipped_hit.clip_rejected_count;
        interaction.keyboard_navigation_event_count = 1;
        interaction.focus_change_count = 1;
        interaction.focus_visible_scroll_count = 1;

        let report = AuiRectClipScrollbarNavigationProductizationReport::from_parts(
            &layout.report,
            &render_report,
            &interaction,
            Some("item_1".to_string()),
            Some("item_2".to_string()),
        );

        assert_eq!(
            report.schema_version,
            AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.status, "passed");
        assert!(report.clip_root_count > 0);
        assert!(report.effective_clip_item_count > 0);
        assert!(report.culled_draw_item_count > 0);
        assert!(report.hit_test_clip_rejected_count > 0);
        assert!(report.scrollbar_visible_count > 0);
        assert!(report.scrollbar_thumb_drag_count > 0);
        assert!(report.scrollbar_offset_change_count > 0);
        assert_eq!(report.keyboard_navigation_event_count, 1);
        assert_eq!(report.focus_visible_scroll_count, 1);
        assert!(report.stencil_mask_deferred);
        assert!(report.full_gamepad_navigation_deferred);
    }

    #[test]
    fn aui_navigation_screenflow_textentry_schema_serializes() {
        let document = navigation_text_entry_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        }]);
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("play_button".to_string());

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        let filtered = frame.filter_consumed_events(&result.consumed_event_indices);
        let report = AuiRuntimeNavigationScreenFlowTextEntryProductizationReport::from_result(
            &document,
            frame.events.len(),
            filtered.events.len(),
            &result,
        );
        let json = serde_json::to_string(&report).expect("216 report should serialize");

        assert_eq!(
            report.schema_version,
            AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.status, "passed");
        assert_eq!(report.submit_count, 1);
        assert!(json.contains("aui-runtime-navigation-screenflow-textentry"));
        assert!(json.contains("imePlatformCoverage"));
    }

    #[test]
    fn aui_submit_cancel_prioritizes_text_edit_before_screen_pop() {
        let document = navigation_text_entry_document();
        let mut state = AuiInteractionState::default();
        AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
            .expect("pause screen should push");
        let input_node = document
            .nodes
            .iter()
            .find(|node| node.node_id == "name_input")
            .expect("input node should exist");
        state.input_field = Some(AuiInputFieldState::start(input_node));
        state.input_mode = AuiInputMode::TextEditing {
            node_id: "name_input".to_string(),
        };
        let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
        let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
            key: "Enter".to_string(),
        }]);

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(result.text_submitted_count, 1);
        assert_eq!(result.screen_stack_pop_count, 0);
        assert_eq!(state.screen_stack.active_stack.len(), 1);
        assert!(result.commands.iter().any(|command| {
            command.command_kind == AuiCommandKind::TextSubmitted
                && command.source_node == "name_input"
        }));
    }

    #[test]
    fn aui_effective_visibility_hidden_parent_keeps_layout_but_blocks_draw_and_hit() {
        let hidden_root = AuiNode::new("hidden-root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["visible-child"]);
        let child = AuiNode::new(
            "visible-child",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 30.0, 160.0, 48.0),
        )
        .with_parent("hidden-root")
        .with_interactable(true)
        .with_text("Hidden action");
        let mut document = AuiDocument::new(
            "effective-visibility",
            vec![AuiCanvas::screen_overlay(
                "main",
                320.0,
                180.0,
                "hidden-root",
            )],
            vec![hidden_root, child],
        );
        document
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "hidden-root")
            .expect("root")
            .visible = false;

        let layout = AuiLayoutEngine::layout(&document, 1);
        let root = layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "hidden-root")
            .expect("root layout");
        let child = layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "visible-child")
            .expect("child layout must remain available");
        assert!(!root.local_visible);
        assert!(!root.effective_visible);
        assert!(child.local_visible);
        assert!(!child.effective_visible);
        assert_eq!(
            child.rect,
            AuiComputedRect {
                x: 20.0,
                y: 30.0,
                width: 160.0,
                height: 48.0,
            }
        );
        assert_eq!(layout.report.node_count, 2);
        assert_eq!(layout.report.visible_node_count, 0);

        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        assert!(draw_list.commands.is_empty());
        let hit = AuiInteractionSystem::hit_test(&document, &layout, 40.0, 40.0);
        assert_eq!(hit.reason, AuiHitTestReason::OutsideUi);
        assert!(hit.hit_node.is_none());
    }

    #[test]
    fn aui_effective_visibility_reconciles_hidden_interaction_state_once() {
        let hidden_root = AuiNode::new("hidden-root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["input"]);
        let input = AuiNode::new(
            "input",
            AuiNodeKind::InputField,
            AuiRect::fixed_position(10.0, 10.0, 180.0, 40.0),
        )
        .with_parent("hidden-root")
        .with_interactable(true);
        let mut modal_canvas = AuiCanvas::screen_overlay("modal", 320.0, 180.0, "hidden-root");
        modal_canvas.composition_stage = AuiCompositionStage::Modal;
        let mut document = AuiDocument::new(
            "effective-visibility-state",
            vec![modal_canvas],
            vec![hidden_root, input],
        );
        document
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "hidden-root")
            .expect("root")
            .visible = false;
        let input_node = document
            .nodes
            .iter()
            .find(|node| node.node_id == "input")
            .expect("input");
        let mut input_field = AuiInputFieldState::start(input_node);
        input_field.composition = Some(AuiTextCompositionState {
            preedit_text: "draft".to_string(),
            cursor_start: 0,
            cursor_end: 5,
            active: true,
        });
        let mut state = AuiInteractionState {
            active_drag: Some(AuiActiveDrag {
                source_node: "input".to_string(),
                start_pointer: AuiPointer::new(12.0, 12.0),
                current_pointer: AuiPointer::new(20.0, 20.0),
                started: true,
                pointer_id: 0,
                device_kind: RuntimePointerDeviceKind::Mouse,
            }),
            primary_press: Some(AuiPrimaryPressCapture {
                node_id: "input".to_string(),
                pointer_id: 0,
                device_kind: RuntimePointerDeviceKind::Mouse,
                hover_capable: true,
                inside: true,
            }),
            focus: AuiFocusState {
                focused_node: Some("input".to_string()),
                focus_scope_root: Some("hidden-root".to_string()),
                focus_reason: AuiFocusReason::Pointer,
            },
            active_modal_root: Some("hidden-root".to_string()),
            input_mode: AuiInputMode::TextEditing {
                node_id: "input".to_string(),
            },
            input_field: Some(input_field),
            active_scroll_capture: Some(AuiActiveScrollCapture {
                node_id: "input".to_string(),
                captured_node_id: "input".to_string(),
                start_pointer: AuiPointer::new(12.0, 12.0),
                last_pointer: AuiPointer::new(20.0, 20.0),
                started: true,
                scroll_delta_per_pointer_delta_y: None,
                pointer_id: 0,
                device_kind: RuntimePointerDeviceKind::Mouse,
            }),
            ..Default::default()
        };
        let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
        let empty_frame = pointer_frame(Vec::new());

        let first = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &empty_frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert!(first.visibility_reconciliation_count >= 7);
        assert_eq!(first.ime_cancel_count, 1);
        assert!(first.actions.is_empty());
        assert!(first.commands.is_empty());
        assert!(state.focus.focused_node.is_none());
        assert!(state.focus.focus_scope_root.is_none());
        assert!(state.pressed_node().is_none());
        assert!(state.active_drag.is_none());
        assert!(state.active_scroll_capture.is_none());
        assert!(state.input_field.is_none());
        assert!(state.active_modal_root.is_none());
        assert_eq!(state.input_mode, AuiInputMode::Navigation);

        let second = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &empty_frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(second.visibility_reconciliation_count, 0);
        assert_eq!(second.ime_cancel_count, 0);
        assert!(second.actions.is_empty());
        assert!(second.commands.is_empty());
    }

    #[test]
    fn aui_screen_flow_push_pop_uses_canvas_visibility_and_focus_restore() {
        let document = navigation_text_entry_document();
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("play_button".to_string());
        let initial_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
        assert!(initial_layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "name_input")
            .is_none_or(|node| !node.effective_visible));

        AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
            .expect("pause screen should push");
        let pushed_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 2, &state);
        assert!(pushed_layout
            .computed_nodes
            .iter()
            .any(|node| node.node_id == "name_input" && node.effective_visible));
        assert_eq!(state.focus.focused_node.as_deref(), Some("name_input"));

        let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
            key: "Escape".to_string(),
        }]);
        let result = AuiInteractionSystem::process_with_state(
            &document,
            &pushed_layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        let popped_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 3, &state);

        assert_eq!(result.screen_stack_push_count, 1);
        assert_eq!(result.screen_stack_pop_count, 1);
        assert_eq!(result.focus_restore_count, 1);
        assert_eq!(state.focus.focused_node.as_deref(), Some("play_button"));
        assert!(popped_layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "name_input")
            .is_none_or(|node| !node.effective_visible));
    }

    #[test]
    fn aui_gamepad_navigation_moves_focus_and_submit() {
        let document = navigation_text_entry_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("play_button".to_string());
        let move_frame = pointer_frame(vec![RuntimeInputEvent::GamepadButtonDown {
            gamepad_id: 0,
            button: "DPadDown".to_string(),
        }]);
        let move_result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &move_frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(state.focus.focused_node.as_deref(), Some("settings_button"));
        assert_eq!(move_result.gamepad_intent_count, 1);
        assert_eq!(move_result.keyboard_navigation_event_count, 1);

        let submit_frame = pointer_frame(vec![RuntimeInputEvent::GamepadButtonDown {
            gamepad_id: 0,
            button: "South".to_string(),
        }]);
        let submit_result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &submit_frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(submit_result.submit_count, 1);
        assert_eq!(submit_result.gamepad_intent_count, 1);
        assert!(submit_result.actions.iter().any(|action| {
            action.event == AuiActionEvent::Submit && action.action_id == "ui.settings"
        }));
    }

    #[test]
    fn aui_input_field_text_edit_changes_submits_and_cancels() {
        let document = navigation_text_entry_document();
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 250.0,
                y: 70.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::TextInput {
                text: "B".to_string(),
            },
            RuntimeInputEvent::KeyDown {
                key: "Backspace".to_string(),
            },
            RuntimeInputEvent::KeyDown {
                key: "Escape".to_string(),
            },
        ]);
        let mut state = AuiInteractionState::default();
        AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
            .expect("pause screen should push");
        let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );

        assert_eq!(result.text_edit_session_count, 1);
        assert_eq!(result.text_changed_count, 2);
        assert_eq!(result.text_cancelled_count, 1);
        assert_eq!(state.input_mode, AuiInputMode::Navigation);
        assert!(result.actions.iter().any(|action| {
            action.event == AuiActionEvent::TextCancelled && action.action_id == "ui.name_cancelled"
        }));
    }

    #[test]
    fn aui_ime_preedit_commit_and_cancel_updates_text_edit_state() {
        let document = navigation_text_entry_document();
        let mut state = AuiInteractionState::default();
        AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
            .expect("pause screen should push");
        let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
        let frame = pointer_frame(vec![
            RuntimeInputEvent::PointerDown {
                x: 250.0,
                y: 70.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::ImePreedit {
                text: "ni".to_string(),
                cursor_start: 0,
                cursor_end: 2,
            },
            RuntimeInputEvent::ImeCancel,
            RuntimeInputEvent::ImeCommit {
                text: "hao".to_string(),
            },
            RuntimeInputEvent::KeyDown {
                key: "Enter".to_string(),
            },
        ]);

        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        let report = AuiRuntimeNavigationScreenFlowTextEntryProductizationReport::from_result(
            &document,
            frame.events.len(),
            frame
                .filter_consumed_events(&result.consumed_event_indices)
                .events
                .len(),
            &result,
        );

        assert_eq!(result.ime_preedit_count, 1);
        assert_eq!(result.ime_cancel_count, 1);
        assert_eq!(result.ime_commit_count, 1);
        assert_eq!(result.text_changed_count, 1);
        assert_eq!(result.text_submitted_count, 1);
        assert_eq!(
            report.ime_platform_coverage,
            "schema_headless_and_winit_cmin"
        );
        assert!(report.ime_candidate_window_deferred);
    }

    #[test]
    fn aui_feedback_draw_transforms_owned_subtree_without_mutating_layout_or_clip() {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["button"]);
        let button = AuiNode::new(
            "button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 30.0, 100.0, 60.0),
        )
        .with_parent("root")
        .with_children(["icon", "caption"])
        .with_interactable(true)
        .with_style(AuiStyle {
            color: Some("#808080".to_string()),
            text_color: Some("#ffffff".to_string()),
            font_size: Some(18.0),
            font: None,
        });
        let icon = AuiNode::new(
            "icon",
            AuiNodeKind::Image,
            AuiRect::fixed_position(10.0, 10.0, 20.0, 20.0),
        )
        .with_parent("button")
        .with_image("ui/icon")
        .with_style(AuiStyle {
            color: Some("#4080c0".to_string()),
            text_color: None,
            font_size: None,
            font: None,
        });
        let caption = AuiNode::new(
            "caption",
            AuiNodeKind::Text,
            AuiRect::fixed_position(40.0, 10.0, 50.0, 20.0),
        )
        .with_parent("button")
        .with_text("Play")
        .with_style(AuiStyle::text("#ffffff", 16.0));
        let document = AuiDocument::new(
            "feedback-draw",
            vec![AuiCanvas::screen_overlay("main", 200.0, 200.0, "root")],
            vec![root, button, icon, caption],
        );
        let layout = AuiLayoutEngine::layout(&document, 1);
        let layout_bytes = serde_json::to_vec(&layout).unwrap();
        let (plain, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        let mut overrides = AuiVisualOverrideSet::default();
        overrides.set(
            "button",
            AuiControlVisualOverride {
                scale: 0.9,
                translation: AuiVec2::new(2.0, 3.0),
                brightness_multiplier: 0.8,
                opacity_multiplier: 0.5,
            },
        );
        let (animated, _) = AuiLayoutEngine::extract_draw_list_with_visual_overrides(
            &document, &layout, &overrides,
        );

        assert_eq!(plain.commands.len(), animated.commands.len());
        assert_eq!(serde_json::to_vec(&layout).unwrap(), layout_bytes);
        for (before, after) in plain.commands.iter().zip(&animated.commands) {
            assert_eq!(
                draw_command_effective_clip_rect(before),
                draw_command_effective_clip_rect(after)
            );
            assert_eq!(
                std::mem::discriminant(before),
                std::mem::discriminant(after)
            );
        }
        let plain_icon = plain.commands.iter().find(|command| matches!(command, AuiDrawCommand::DrawImage { node_id, .. } if node_id == "icon")).unwrap();
        let animated_icon = animated.commands.iter().find(|command| matches!(command, AuiDrawCommand::DrawImage { node_id, .. } if node_id == "icon")).unwrap();
        let (plain_rect, animated_rect, animated_color) = match (plain_icon, animated_icon) {
            (
                AuiDrawCommand::DrawImage { rect: before, .. },
                AuiDrawCommand::DrawImage {
                    rect: after, color, ..
                },
            ) => (*before, *after, color.as_deref()),
            _ => unreachable!(),
        };
        assert!(animated_rect.width < plain_rect.width);
        assert!(animated_rect.x > plain_rect.x);
        assert_eq!(animated_color, Some("#33669a80"));
    }

    #[test]
    fn aui_feedback_draw_empty_override_is_byte_equivalent() {
        let document = interaction_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let (plain, plain_report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        let (overridden, overridden_report) =
            AuiLayoutEngine::extract_draw_list_with_visual_overrides(
                &document,
                &layout,
                &AuiVisualOverrideSet::default(),
            );
        assert_eq!(plain, overridden);
        assert_eq!(plain_report, overridden_report);
    }

    #[test]
    fn aui_interaction_productization_report_exposes_deferred_flags() {
        let document = drag_document();
        let layout = AuiLayoutEngine::layout(&document, 1);
        let frame = pointer_frame(vec![RuntimeInputEvent::PointerDown {
            x: 60.0,
            y: 60.0,
            button: RuntimePointerButton::Primary,
        }]);
        let result = AuiInteractionSystem::process(&document, &layout, &frame);
        let filtered = frame.filter_consumed_events(&result.consumed_event_indices);

        let report = AuiInteractionProductizationReport::from_result(
            &document,
            frame.events.len(),
            filtered.events.len(),
            &result,
            AuiInteractionConfig::default(),
            None,
        );

        assert_eq!(
            report.schema_version,
            AUI_INTERACTION_PRODUCTIZATION_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.drag_threshold_px, 4.0);
        assert_eq!(report.snapshot_frame_lag, 1);
        assert!(report.authoring_action_payload_deferred);
        assert!(!report.modal_input_blocking_deferred);
        assert!(!report.editor_hit_test_deferred_to_209);
        assert!(!report.control_style_deferred);
        assert!(report.slider_toggle_binding_target_deferred);
        assert_eq!(report.consumed_pointer_event_count, 1);
        assert_eq!(report.filtered_input_event_count, 0);
        assert_eq!(
            report.consumed_event_count_by_kind.get("PointerDown"),
            Some(&1)
        );
    }
}
