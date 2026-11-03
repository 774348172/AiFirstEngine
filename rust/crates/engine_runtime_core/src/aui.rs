#[path = "aui/interaction.rs"]
mod interaction;
#[path = "aui/layout.rs"]
mod layout;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

#[path = "aui_drawing_cache.rs"]
mod drawing_cache;
pub(crate) use drawing_cache::AuiDrawingCache;
pub use drawing_cache::AuiDrawingStats;

pub use crate::aui_hit_test_index::AuiHitTestIndexStats;
use crate::aui_hit_test_index::{
    AuiInteractionIndexJournal, AuiInteractionIndexRuntime, AuiNodeLookup,
};
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
use crate::text_shaping::{layout_line, shape_stack};
use crate::world::World;

pub const AUI_DOCUMENT_SCHEMA_VERSION: &str = "aui-document.v2";
pub const LEGACY_AUI_DOCUMENT_SCHEMA_VERSION: &str = "aui-document.v1";
pub const AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID: &str = "button.default.v1";
pub const AUI_ASSET_MANIFEST_SCHEMA_VERSION: &str = "aui-asset-manifest.v1";
pub const AUI_IME_PLATFORM_COVERAGE: &str = "winit_candidate_rect_adapter_v1";

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
    ViewportLayout,
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
    #[serde(
        default,
        rename = "viewportLayout",
        skip_serializing_if = "Option::is_none"
    )]
    pub viewport_layout: Option<crate::aui_viewport::AuiViewportLayoutSpec>,
    #[serde(skip)]
    pub(crate) resolved_viewport_root_rect: Option<AuiComputedRect>,
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
            viewport_layout: None,
            resolved_viewport_root_rect: None,
            root_node: root_node.into(),
            screen_id: None,
            default_focus_node_id: None,
            cancel_action_id: None,
            submit_action_id: None,
        }
    }

    pub fn with_viewport_layout(
        mut self,
        viewport_layout: crate::aui_viewport::AuiViewportLayoutSpec,
    ) -> Self {
        self.scale_mode = AuiScaleMode::ViewportLayout;
        self.viewport_layout = Some(viewport_layout);
        self
    }

    pub(crate) fn resolved_layout_root_rect(&self) -> AuiComputedRect {
        self.resolved_viewport_root_rect.unwrap_or(AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: self.reference_resolution.x,
            height: self.reference_resolution.y,
        })
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
    Rounded,
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
    /// sRGB hexadecimal RGB with optional straight, linear alpha.
    pub color: Option<String>,
    /// sRGB text/foreground color; ProgressBar uses this for its fill.
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
    RectOffsetX,
    RectOffsetY,
    TextColor,
    TextText,
    InputFieldText,
    ProgressBarValue,
    ToggleChecked,
    SliderValue,
    PanelVisible,
    ImageVisible,
    ImageAssetRef,
    Interactable,
    Translation,
    Rotation,
    Scale,
    Pivot,
    Opacity,
    Tint,
    Resource,
    LocalStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AuiBindingValue {
    Bool(bool),
    Number(f32),
    String(String),
    Color(String),
    Vector([f32; 2]),
    Rgba([f32; 4]),
    Resource(project_runtime_sdk::aui::UiResourceRef),
    LocalStyle(project_runtime_sdk::aui::UiLocalStyle),
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
            AuiBindingValue::Vector(_) => "vector",
            AuiBindingValue::Rgba(_) => "rgba",
            AuiBindingValue::Resource(_) => "resource",
            AuiBindingValue::LocalStyle(_) => "local_style",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectUiStateSnapshot {
    pub frame_index: u64,
    pub values: HashMap<String, AuiBindingValue>,
    #[serde(
        default,
        rename = "virtualLists",
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub virtual_lists: HashMap<String, crate::aui_virtual_list::AuiVirtualListSnapshot>,
}

impl ProjectUiStateSnapshot {
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            values: HashMap::new(),
            virtual_lists: HashMap::new(),
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
    pub(crate) fn equivalent_except_frame(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.status == other.status
            && self.producer_id == other.producer_id
            && self.snapshot_source == other.snapshot_source
            && self.value_count == other.value_count
            && self.active_binding_paths == other.active_binding_paths
            && self.produced_paths == other.produced_paths
            && self.declared_binding_paths == other.declared_binding_paths
            && self.missing_paths == other.missing_paths
            && self.type_mismatch_paths == other.type_mismatch_paths
            && self.dirty_domains == other.dirty_domains
            && self.cache_status == other.cache_status
            && self.cache_hit_paths == other.cache_hit_paths
            && self.cache_miss_paths == other.cache_miss_paths
            && self.source_paths == other.source_paths
            && self.diagnostics == other.diagnostics
    }

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

    pub(crate) fn checkpoint(&self) -> (Option<ProjectUiStateIdentity>, bool) {
        (self.previous_identity.clone(), self.binding_set_registered)
    }

    pub(crate) fn restore(&mut self, checkpoint: (Option<ProjectUiStateIdentity>, bool)) {
        self.previous_identity = checkpoint.0;
        self.binding_set_registered = checkpoint.1;
    }

    pub fn set_active_binding_paths(&mut self, paths: impl IntoIterator<Item = String>) {
        let paths = sorted_strings(paths);
        if self.active_binding_paths == paths {
            return;
        }
        self.binding_set_identity = ProjectUiBindingSetIdentity::from_paths(&paths);
        self.active_binding_paths = paths;
        self.binding_set_registered = false;
        self.previous_identity = None;
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
    /// A business activation shared by pointer Click and focused Submit.
    pub fn activate(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::Activate,
            action_id: action_id.into(),
        }
    }

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

    pub fn value_changed(action_id: impl Into<String>) -> Self {
        Self {
            event: AuiActionEvent::ValueChanged,
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiActionEvent {
    Activate,
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
    ValueChanged,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<project_runtime_sdk::aui_presentation::NodePresentation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<crate::aui_extension::AuiCustomNodeSpec>,
    #[serde(skip)]
    pub custom_runtime: Option<crate::aui_extension::AuiCustomRuntimeOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<crate::aui_layout_schema::LayoutSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<crate::aui_layout_schema::SlotSpec>,
    #[serde(
        default,
        rename = "textLayout",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_layout: Option<crate::aui_layout_schema::TextLayoutSpec>,
    #[serde(default, rename = "textSpans", skip_serializing_if = "Vec::is_empty")]
    pub text_spans: Vec<crate::aui_layout_schema::AuiRichTextSpan>,
    #[serde(
        default,
        rename = "layoutVisibility",
        skip_serializing_if = "Option::is_none"
    )]
    pub layout_visibility: Option<crate::aui_layout_schema::LayoutVisibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow: Option<crate::aui_layout_schema::Overflow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<crate::runtime_package::RuntimeAssetRef>,
    #[serde(default, rename = "styleRef", skip_serializing_if = "Option::is_none")]
    pub style_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_overrides: Option<crate::aui_theme::AuiThemeStyleOverride>,
    #[serde(
        default,
        rename = "clipRadius",
        skip_serializing_if = "Option::is_none"
    )]
    pub clip_radius: Option<f32>,
    #[serde(skip)]
    pub resolved_style: Option<crate::aui_theme::AuiResolvedStyle>,
    #[serde(
        default,
        rename = "viewportCondition",
        skip_serializing_if = "Option::is_none"
    )]
    pub viewport_condition: Option<String>,
    #[serde(skip)]
    pub(crate) resolved_viewport_original_visibility:
        Option<Option<crate::aui_layout_schema::LayoutVisibility>>,
    #[serde(skip)]
    pub virtual_list_item: Option<crate::aui_virtual_list::AuiVirtualListItemContext>,
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
    #[serde(
        default,
        rename = "toggleChecked",
        alias = "toggle_checked",
        skip_serializing_if = "Option::is_none"
    )]
    pub toggle_checked: Option<bool>,
    #[serde(
        default,
        rename = "sliderValue",
        alias = "slider_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub slider_value: Option<f32>,
    #[serde(
        default,
        rename = "sliderMin",
        alias = "slider_min",
        skip_serializing_if = "Option::is_none"
    )]
    pub slider_min: Option<f32>,
    #[serde(
        default,
        rename = "sliderMax",
        alias = "slider_max",
        skip_serializing_if = "Option::is_none"
    )]
    pub slider_max: Option<f32>,
    #[serde(
        default,
        rename = "sliderStep",
        alias = "slider_step",
        skip_serializing_if = "Option::is_none"
    )]
    pub slider_step: Option<f32>,
    pub binding_refs: Vec<AuiBindingRef>,
    pub action_refs: Vec<AuiActionRef>,
}

impl AuiNode {
    pub fn new(node_id: impl Into<String>, kind: AuiNodeKind, rect: AuiRect) -> Self {
        let node_id = node_id.into();
        Self {
            custom: None,
            presentation: None,
            custom_runtime: None,
            name: node_id.clone(),
            layout: None,
            slot: None,
            text_layout: None,
            text_spans: Vec::new(),
            layout_visibility: None,
            overflow: None,
            theme: None,
            style_ref: None,
            variant: None,
            style_overrides: None,
            clip_radius: None,
            resolved_style: None,
            viewport_condition: None,
            resolved_viewport_original_visibility: None,
            virtual_list_item: None,
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
            toggle_checked: None,
            slider_value: None,
            slider_min: None,
            slider_max: None,
            slider_step: None,
            binding_refs: Vec::new(),
            action_refs: Vec::new(),
        }
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_viewport_condition(mut self, condition_id: impl Into<String>) -> Self {
        self.viewport_condition = Some(condition_id.into());
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

    pub fn with_toggle_checked(mut self, checked: bool) -> Self {
        self.toggle_checked = Some(checked);
        self
    }

    pub fn with_slider(mut self, value: f32, min: f32, max: f32, step: f32) -> Self {
        self.slider_value = Some(value);
        self.slider_min = Some(min);
        self.slider_max = Some(max);
        self.slider_step = Some(step);
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
    /// Package-owned immutable data, never serialized into the authoring document.
    #[serde(skip)]
    pub graphics_assets: crate::aui_graphics_asset::GraphicsAssetRegistry,
    #[serde(skip)]
    pub resource_identities: Arc<BTreeMap<String, (String, String)>>,
    pub schema_version: String,
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<crate::runtime_package::RuntimeAssetRef>,
    #[serde(default)]
    pub interaction_feedback: Option<AuiInteractionFeedbackRegistry>,
    #[serde(
        default,
        rename = "virtualLists",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub virtual_lists: Vec<crate::aui_virtual_list::AuiVirtualListDeclaration>,
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
            graphics_assets: Default::default(),
            resource_identities: Default::default(),
            document_id: document_id.into(),
            theme: None,
            interaction_feedback: None,
            virtual_lists: Vec::new(),
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

                match validate_binding_resource(
                    node,
                    &binding,
                    &value,
                    &document.resource_identities,
                    &document.graphics_assets,
                )
                .and_then(|()| {
                    apply_binding_value_for_schema(
                        node,
                        &binding,
                        value,
                        document.schema_version
                            == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION,
                    )
                }) {
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

fn local_binding_target_valid(
    node: &AuiNode,
    target: AuiBindingTarget,
    value: &AuiBindingValue,
) -> bool {
    if let Some((property, value)) = presentation_binding_value(target, value) {
        return project_runtime_sdk::aui_wire::validate_value(
            project_runtime_sdk::aui::UiNodeHandle {
                instance: 1,
                slot: 0,
                generation: 1,
            },
            property,
            &value,
        )
        .is_ok();
    }
    match (target, value) {
        (
            AuiBindingTarget::TextText | AuiBindingTarget::InputFieldText,
            AuiBindingValue::String(_),
        )
        | (
            AuiBindingTarget::PanelVisible
            | AuiBindingTarget::ImageVisible
            | AuiBindingTarget::Interactable,
            AuiBindingValue::Bool(_),
        )
        | (AuiBindingTarget::ImageAssetRef, AuiBindingValue::AssetRef(_)) => true,
        (AuiBindingTarget::TextColor, AuiBindingValue::Rgba(value)) => {
            project_runtime_sdk::aui_presentation::validate_rgba(*value).is_ok()
        }
        (
            AuiBindingTarget::ProgressBarValue
            | AuiBindingTarget::RectOffsetX
            | AuiBindingTarget::RectOffsetY,
            AuiBindingValue::Number(value),
        ) => value.is_finite(),
        (AuiBindingTarget::ToggleChecked, AuiBindingValue::Bool(_)) => {
            node.kind == AuiNodeKind::Toggle
        }
        (AuiBindingTarget::SliderValue, AuiBindingValue::Number(value)) => {
            node.kind == AuiNodeKind::Slider && value.is_finite()
        }
        (
            AuiBindingTarget::TextColor,
            AuiBindingValue::String(value) | AuiBindingValue::Color(value),
        ) => {
            value.starts_with('#')
                && matches!(value.len(), 7 | 9)
                && value[1..].bytes().all(|c| c.is_ascii_hexdigit())
        }
        _ => false,
    }
}

impl AuiBindingTarget {
    pub(crate) fn presentation_property(self) -> Option<project_runtime_sdk::aui::UiProperty> {
        use project_runtime_sdk::aui::UiProperty as P;
        Some(match self {
            Self::Translation => P::Translation,
            Self::Rotation => P::Rotation,
            Self::Scale => P::Scale,
            Self::Pivot => P::Pivot,
            Self::Opacity => P::Opacity,
            Self::Tint => P::Tint,
            Self::Resource => P::Resource,
            Self::LocalStyle => P::LocalStyle,
            _ => return None,
        })
    }
}

fn presentation_binding_value(
    target: AuiBindingTarget,
    value: &AuiBindingValue,
) -> Option<(
    project_runtime_sdk::aui::UiProperty,
    project_runtime_sdk::aui::UiValue,
)> {
    use project_runtime_sdk::aui::{UiProperty as P, UiValue as V};
    let property = target.presentation_property()?;
    let typed = match (property, value) {
        (P::Translation, AuiBindingValue::Vector(v)) => V::Translation(*v),
        (P::Scale | P::Pivot, AuiBindingValue::Vector(v)) => V::Vector(*v),
        (P::Rotation | P::Opacity, AuiBindingValue::Number(v)) => V::Number(*v),
        (P::Tint, AuiBindingValue::Rgba(v)) => V::Color(*v),
        (P::Resource, AuiBindingValue::Resource(v)) => V::Resource(Some(v.clone())),
        (P::LocalStyle, AuiBindingValue::LocalStyle(v)) => V::LocalStyle(v.clone()),
        _ => return None,
    };
    Some((property, typed))
}

fn apply_binding_value(
    node: &mut AuiNode,
    binding: &AuiBindingRef,
    value: AuiBindingValue,
) -> Result<(), String> {
    apply_binding_value_for_schema(node, binding, value, true)
}

fn apply_binding_value_for_schema(
    node: &mut AuiNode,
    binding: &AuiBindingRef,
    value: AuiBindingValue,
    uses_c2: bool,
) -> Result<(), String> {
    if binding.target_field == AuiBindingTarget::TextColor {
        if let AuiBindingValue::Rgba(value) = value {
            project_runtime_sdk::aui_presentation::validate_rgba(value)
                .map_err(|e| format!("{e}"))?;
            return apply_binding_value(
                node,
                binding,
                AuiBindingValue::Color(extension_color(value)),
            );
        }
    }
    if let Some((property, typed)) = presentation_binding_value(binding.target_field, &value) {
        project_runtime_sdk::aui_wire::validate_value(
            project_runtime_sdk::aui::UiNodeHandle {
                instance: 1,
                slot: 0,
                generation: 1,
            },
            property,
            &typed,
        )
        .map_err(|e| format!("{e:?}"))?;
        apply_retained_value(node, property, &typed, uses_c2);
        return Ok(());
    }
    match (binding.target_field, value) {
        (AuiBindingTarget::RectOffsetX, AuiBindingValue::Number(value)) if value.is_finite() => {
            node.rect.offset_min.x = value;
            Ok(())
        }
        (AuiBindingTarget::RectOffsetY, AuiBindingValue::Number(value)) if value.is_finite() => {
            node.rect.offset_min.y = value;
            Ok(())
        }
        (AuiBindingTarget::TextColor, AuiBindingValue::String(value))
        | (AuiBindingTarget::TextColor, AuiBindingValue::Color(value))
            if value.starts_with('#')
                && matches!(value.len(), 7 | 9)
                && value[1..].bytes().all(|c| c.is_ascii_hexdigit()) =>
        {
            node.style
                .get_or_insert(AuiStyle {
                    color: None,
                    text_color: None,
                    font_size: None,
                    font: None,
                })
                .text_color = Some(value);
            Ok(())
        }
        (AuiBindingTarget::TextText, AuiBindingValue::String(value))
        | (AuiBindingTarget::InputFieldText, AuiBindingValue::String(value)) => {
            node.text = Some(value);
            Ok(())
        }
        (AuiBindingTarget::ProgressBarValue, AuiBindingValue::Number(value)) => {
            node.progress_value = Some(value.clamp(0.0, 1.0));
            Ok(())
        }
        (AuiBindingTarget::ToggleChecked, AuiBindingValue::Bool(value))
            if node.kind == AuiNodeKind::Toggle =>
        {
            node.toggle_checked = Some(value);
            Ok(())
        }
        (AuiBindingTarget::SliderValue, AuiBindingValue::Number(value))
            if node.kind == AuiNodeKind::Slider && value.is_finite() =>
        {
            node.slider_value = Some(normalize_slider_value(node, value));
            Ok(())
        }
        (AuiBindingTarget::PanelVisible, AuiBindingValue::Bool(value))
        | (AuiBindingTarget::ImageVisible, AuiBindingValue::Bool(value)) => {
            node.visible = value;
            Ok(())
        }
        (AuiBindingTarget::Interactable, AuiBindingValue::Bool(value)) => {
            node.interactable = value;
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

fn validate_binding_resource(
    node: &AuiNode,
    binding: &AuiBindingRef,
    value: &AuiBindingValue,
    identities: &BTreeMap<String, (String, String)>,
    graphics: &crate::aui_graphics_asset::GraphicsAssetRegistry,
) -> Result<(), String> {
    if binding.target_field != AuiBindingTarget::Resource {
        return Ok(());
    }
    let AuiBindingValue::Resource(resource) = value else {
        return Err("Resource binding requires assetId and guid.".into());
    };
    let valid = match node.kind {
        AuiNodeKind::Image => identities
            .get(&resource.asset_id)
            .is_some_and(|(guid, kind)| guid == &resource.guid && kind == "texture"),
        AuiNodeKind::Custom => graphics.resolve(&resource.asset_id, &resource.guid).is_ok(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("Resource identity or node kind does not match mounted RuntimePackage assets.".into())
    }
}

fn slider_range(node: &AuiNode) -> (f32, f32, f32) {
    (
        node.slider_min.unwrap_or(0.0),
        node.slider_max.unwrap_or(1.0),
        node.slider_step.unwrap_or(0.1),
    )
}

pub(crate) fn normalize_slider_value(node: &AuiNode, value: f32) -> f32 {
    let (min, max, step) = slider_range(node);
    if !value.is_finite()
        || !min.is_finite()
        || !max.is_finite()
        || !step.is_finite()
        || min >= max
        || step <= 0.0
    {
        return value;
    }
    let steps = ((value.clamp(min, max) - min) / step).round();
    let snapped = (min as f64) + (steps as f64) * (step as f64);
    let decimal_places = ((-(step.abs() as f64).log10()).ceil() as i32 + 2).clamp(0, 9);
    let decimal_scale = 10_f64.powi(decimal_places);
    (((snapped * decimal_scale).round() / decimal_scale) as f32).clamp(min, max)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiComputedNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<AuiPresentationGeometry>,
    pub canvas_id: String,
    pub composition_stage: AuiCompositionStage,
    pub node_id: String,
    pub kind: AuiNodeKind,
    pub rect: AuiComputedRect,
    pub effective_clip_rect: Option<AuiComputedRect>,
    #[serde(default)]
    pub effective_clips: Vec<AuiResolvedClip>,
    pub clipped_by_node: Option<String>,
    pub tree_order: usize,
    pub local_visible: bool,
    pub effective_visible: bool,
}

/// One published geometry fact. Rect remains layout occupation plus legacy translation;
/// bounds is only the broad phase, never a replacement for transformed local geometry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AuiPresentationGeometry {
    pub local_to_canvas: [f32; 6],
    pub vertex_transform: [f32; 6],
    pub bounds: AuiComputedRect,
    pub modulation: [f32; 4],
}

impl AuiComputedNode {
    pub fn presentation_bounds(&self) -> AuiComputedRect {
        self.presentation.map_or(self.rect, |p| p.bounds)
    }
    pub fn contains_presented_point(&self, point: AuiPointer) -> bool {
        self.presented_point_to_local(point).is_some_and(|p| {
            p[0] >= 0.0 && p[1] >= 0.0 && p[0] <= self.rect.width && p[1] <= self.rect.height
        })
    }

    pub fn presented_point_to_local(&self, point: AuiPointer) -> Option<[f32; 2]> {
        if let Some(p) = self.presentation {
            project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(p.local_to_canvas)
                .and_then(|m| m.try_inverse())
                .and_then(|m| m.transform_point([point.x, point.y]))
                .ok()
        } else {
            Some([point.x - self.rect.x, point.y - self.rect.y])
        }
    }
}

impl AuiResolvedClip {
    pub fn contains(&self, point: AuiPointer) -> bool {
        let point = match self.transform {
            Some(m) => project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(m)
                .and_then(|m| m.try_inverse())
                .and_then(|m| m.transform_point([point.x, point.y]))
                .ok(),
            None => Some([point.x, point.y]),
        };
        let Some([x, y]) = point else {
            return false;
        };
        if !self.rect.contains(x, y) {
            return false;
        }
        let r = self
            .radius
            .max(0.0)
            .min(self.rect.width.min(self.rect.height) * 0.5);
        let cx = x.clamp(self.rect.x + r, self.rect.x + self.rect.width - r);
        let cy = y.clamp(self.rect.y + r, self.rect.y + self.rect.height - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r + 0.0001
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedClip {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f32; 6]>,
    pub node_id: String,
    pub rect: AuiComputedRect,
    pub radius: f32,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<AuiValidationItem>,
    pub computed_nodes: Vec<AuiComputedNode>,
    pub scrollbar_metrics: Vec<AuiScrollbarMetrics>,
    pub report: AuiLayoutReport,
    #[serde(skip)]
    pub(crate) interaction_index_update: crate::aui_hit_test_index::AuiInteractionIndexUpdate,
}

impl AuiLayoutResult {
    pub(crate) fn changed_slots_since(&self, previous: &Self) -> Option<Vec<usize>> {
        let old = &previous.interaction_index_update;
        AuiDrawingCache::changed_slots_since(self, (old.generation, old.revision, old.node_count))
    }
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
    ValueChanged,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct AuiToggleValueState {
    source_value: bool,
    value: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct AuiSliderValueState {
    source_value: f32,
    value: f32,
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
    #[serde(default)]
    toggle_values: BTreeMap<String, AuiToggleValueState>,
    #[serde(default)]
    slider_values: BTreeMap<String, AuiSliderValueState>,
    active_scroll_capture: Option<AuiActiveScrollCapture>,
    #[serde(skip)]
    interaction_index: AuiInteractionIndexRuntime,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiRecycledNodeCleanup {
    pub cleared_hover_count: usize,
    pub cleared_pointer_capture_count: usize,
    pub cleared_focus_count: usize,
    pub cleared_text_edit_count: usize,
    pub cleared_modal_count: usize,
}

impl AuiInteractionState {
    pub fn hit_test_index_stats(&self) -> AuiHitTestIndexStats {
        self.interaction_index.stats()
    }
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

    pub fn toggle_value(&self, node_id: &str) -> Option<bool> {
        self.toggle_values.get(node_id).map(|state| state.value)
    }

    pub fn slider_value(&self, node_id: &str) -> Option<f32> {
        self.slider_values.get(node_id).map(|state| state.value)
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

    pub(crate) fn reconcile_recycled_nodes(
        &mut self,
        node_ids: &HashSet<String>,
    ) -> AuiRecycledNodeCleanup {
        let mut report = AuiRecycledNodeCleanup::default();
        if self
            .hovered_node
            .as_ref()
            .is_some_and(|node| node_ids.contains(node))
        {
            self.hovered_node = None;
            report.cleared_hover_count += 1;
        }
        if self
            .primary_press
            .as_ref()
            .is_some_and(|capture| node_ids.contains(&capture.node_id))
        {
            self.primary_press = None;
            report.cleared_pointer_capture_count += 1;
        }
        if self
            .active_drag
            .as_ref()
            .is_some_and(|drag| node_ids.contains(&drag.source_node))
        {
            self.active_drag = None;
            report.cleared_pointer_capture_count += 1;
        }
        if self.active_scroll_capture.as_ref().is_some_and(|capture| {
            node_ids.contains(&capture.node_id) || node_ids.contains(&capture.captured_node_id)
        }) {
            self.active_scroll_capture = None;
            report.cleared_pointer_capture_count += 1;
        }
        if self
            .focus
            .focused_node
            .as_ref()
            .is_some_and(|node| node_ids.contains(node))
        {
            self.focus.focused_node = None;
            self.focus.focus_reason = AuiFocusReason::Cleared;
            report.cleared_focus_count += 1;
        }
        if self
            .focus
            .focus_scope_root
            .as_ref()
            .is_some_and(|node| node_ids.contains(node))
        {
            self.focus.focus_scope_root = None;
            report.cleared_focus_count += 1;
        }
        if self
            .input_field
            .as_ref()
            .is_some_and(|field| node_ids.contains(&field.node_id))
        {
            self.input_field = None;
            self.input_mode = AuiInputMode::Navigation;
            report.cleared_text_edit_count += 1;
        } else if matches!(
            &self.input_mode,
            AuiInputMode::TextEditing { node_id } if node_ids.contains(node_id)
        ) {
            self.input_mode = AuiInputMode::Navigation;
            report.cleared_text_edit_count += 1;
        }
        if self
            .active_modal_root
            .as_ref()
            .is_some_and(|node| node_ids.contains(node))
        {
            self.active_modal_root = None;
            report.cleared_modal_count += 1;
        }
        self.toggle_values
            .retain(|node_id, _| !node_ids.contains(node_id));
        self.slider_values
            .retain(|node_id, _| !node_ids.contains(node_id));
        self.pending_control_reconciliation_count += report.cleared_hover_count
            + report.cleared_pointer_capture_count
            + report.cleared_focus_count
            + report.cleared_text_edit_count
            + report.cleared_modal_count;
        report
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiImePlatformRequest {
    pub enabled: bool,
    pub node_id: Option<String>,
    pub canvas_id: Option<String>,
    pub cursor_rect: Option<AuiComputedRect>,
}

impl AuiInputFieldState {
    fn start(node: &AuiNode) -> Self {
        Self::from_node(node)
    }

    pub fn from_node(node: &AuiNode) -> Self {
        let text = node.text.clone().unwrap_or_default();
        let caret_index = text.len();
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
        let start = grapheme_boundary_at_or_before(&self.draft_text, start);
        let end = grapheme_boundary_at_or_after(&self.draft_text, end.max(start));
        let mut next = self.draft_text.clone();
        next.replace_range(start..end, insert);
        if let Some(max_length) = max_length {
            if let Some((truncate_at, _)) = next.grapheme_indices(true).nth(max_length) {
                next.truncate(truncate_at);
            }
        }
        self.draft_text = next;
        self.caret_index = grapheme_boundary_at_or_before(
            &self.draft_text,
            start
                .saturating_add(insert.len())
                .min(self.draft_text.len()),
        );
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
            let previous = previous_grapheme_boundary(&self.draft_text, self.caret_index);
            self.replace_range(previous, self.caret_index, "", None);
        }
        previous != self.draft_text
    }

    fn delete(&mut self) -> bool {
        let previous = self.draft_text.clone();
        let len = self.draft_text.len();
        if let Some((start, end)) = self.selected_range() {
            self.replace_range(start, end, "", None);
        } else if self.caret_index < len {
            let next = next_grapheme_boundary(&self.draft_text, self.caret_index);
            self.replace_range(self.caret_index, next, "", None);
        }
        previous != self.draft_text
    }

    fn move_caret(&mut self, command: AuiTextEditCommand, extend_selection: bool) -> bool {
        let previous = (
            self.caret_index,
            self.selection_anchor,
            self.selection_focus,
        );
        let len = self.draft_text.len();
        match command {
            AuiTextEditCommand::MoveCaretLeft | AuiTextEditCommand::SelectLeft => {
                self.caret_index = previous_grapheme_boundary(&self.draft_text, self.caret_index);
            }
            AuiTextEditCommand::MoveCaretRight | AuiTextEditCommand::SelectRight => {
                self.caret_index = next_grapheme_boundary(&self.draft_text, self.caret_index);
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

    pub fn move_grapheme_left(&mut self, extend_selection: bool) -> bool {
        self.move_caret(AuiTextEditCommand::MoveCaretLeft, extend_selection)
    }

    pub fn move_grapheme_right(&mut self, extend_selection: bool) -> bool {
        self.move_caret(AuiTextEditCommand::MoveCaretRight, extend_selection)
    }

    pub fn backspace_grapheme(&mut self) -> bool {
        self.backspace()
    }

    pub fn delete_grapheme(&mut self) -> bool {
        self.delete()
    }

    pub fn set_composition(
        &mut self,
        text: impl Into<String>,
        cursor_start: usize,
        cursor_end: usize,
    ) {
        self.composition = Some(AuiTextCompositionState {
            preedit_text: text.into(),
            cursor_start,
            cursor_end,
            active: true,
        });
    }

    pub fn cancel_composition(&mut self) {
        self.composition = None;
    }

    pub fn commit_composition(&mut self, text: &str, max_length: Option<usize>) -> bool {
        self.composition = None;
        self.insert_text(text, max_length)
    }
}

fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index < offset.min(text.len()))
        .last()
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|index| *index > offset.min(text.len()))
        .unwrap_or(text.len())
}

fn grapheme_boundary_at_or_before(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return offset;
    }
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}

fn grapheme_boundary_at_or_after(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return offset;
    }
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|index| *index >= offset)
        .unwrap_or(text.len())
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

    fn apply_delta(&mut self, delta_y: f32) -> f32 {
        self.last_delta_y = delta_y;
        let previous = self.offset_y;
        self.offset_y = (self.offset_y + delta_y).clamp(0.0, self.max_offset_y.max(0.0));
        self.offset_y - previous
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

pub const AUI_CONTROL_VALUE_PAYLOAD_SCHEMA_VERSION: &str = "aui-control-value-payload.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "valueType", content = "value", rename_all = "kebab-case")]
pub enum AuiControlValue {
    Bool(bool),
    Number(f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiControlValuePayload {
    pub schema_version: String,
    pub node_id: String,
    pub value: AuiControlValue,
}

impl AuiControlValuePayload {
    fn new(node_id: impl Into<String>, value: AuiControlValue) -> Self {
        Self {
            schema_version: AUI_CONTROL_VALUE_PAYLOAD_SCHEMA_VERSION.to_string(),
            node_id: node_id.into(),
            value,
        }
    }

    fn to_payload_string(&self) -> String {
        serde_json::to_string(self).expect("AUI control value payload must serialize")
    }
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
            slider_toggle_binding_target_deferred: false,
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
            rich_text_deferred: false,
            ime_candidate_window_deferred: false,
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
    DrawMesh {
        node_id: String,
        rect: AuiComputedRect,
        effective_clip_rect: Option<AuiComputedRect>,
        mesh: crate::aui_retained::AuiMeshPayload,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiDrawList {
    pub commands: Vec<Arc<AuiDrawCommand>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiRenderReport {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphics_diagnostics: Vec<String>,
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
    Mesh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuiOverlaySortKey {
    pub canvas_layer: i32,
    pub canvas_sorting_order: i32,
    pub tree_order: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiOverlayDrawItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<AuiPresentationGeometry>,
    pub item_id: String,
    pub canvas_id: String,
    pub composition_stage: AuiCompositionStage,
    pub node_id: String,
    pub item_kind: AuiOverlayItemKind,
    pub rect: AuiComputedRect,
    pub effective_clip_rect: Option<AuiComputedRect>,
    #[serde(default)]
    pub effective_clips: Vec<AuiResolvedClip>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<crate::aui_theme::AuiResolvedPaint>,
    pub color: Option<String>,
    pub asset_id: Option<String>,
    pub text: Option<String>,
    pub font_size: Option<f32>,
    pub font: Option<AuiFontStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<crate::aui_retained::AuiMeshPayload>,
    pub sort_key: AuiOverlaySortKey,
}

pub const AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION: &str =
    "aui-rectclip-scrollbar-navigation-productization-report.v2";

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
            nested_scroll_deferred: false,
            inertia_elastic_deferred: true,
            virtualized_list_deferred: false,
            input_field_ime_deferred: false,
            full_gamepad_navigation_deferred: true,
            diagnostics: Vec::new(),
            next_actions: vec!["stencil_mask_deferred".to_string()],
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_bundle: Option<(String, u64)>,
    pub rect: AuiComputedRect,
    pub uv_rect: [f32; 4],
    pub page_index: u32,
    pub render_mode: FontBundleRenderMode,
    pub clipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
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
    pub quads: Vec<Arc<AuiTextGlyphQuad>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiOverlayFrame {
    pub frame_index: u64,
    pub draw_items: Arc<Vec<Arc<AuiOverlayDrawItem>>>,
    pub report: AuiRenderReport,
    pub glyph_plan: Option<std::sync::Arc<AuiTextGlyphPlan>>,
}

pub const AUI_COMPOSITION_REPORT_SCHEMA_VERSION: &str = "aui-composition-report.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiCompositionStageFrame {
    pub stage: AuiCompositionStage,
    pub draw_items: std::sync::Arc<Vec<Arc<AuiOverlayDrawItem>>>,
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
            draw_items: Default::default(),
            item_count: 0,
            text_count: 0,
            image_count: 0,
            glyph_count: 0,
            canvas_count: 0,
            layer_group_count: 0,
            debug_label: stage.debug_label().to_string(),
        }
    }

    fn from_items(
        stage: AuiCompositionStage,
        draw_items: Arc<Vec<Arc<AuiOverlayDrawItem>>>,
    ) -> Self {
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AuiStageProjectionDelta {
    pub previous_items: Arc<Vec<Arc<AuiOverlayDrawItem>>>,
    pub current_items: Arc<Vec<Arc<AuiOverlayDrawItem>>>,
    pub changed_slots: Arc<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AuiGlyphProjectionDelta {
    pub previous_plan: Arc<AuiTextGlyphPlan>,
    pub current_plan: Arc<AuiTextGlyphPlan>,
    pub changed_groups: Arc<BTreeMap<String, Arc<Vec<Arc<AuiTextGlyphQuad>>>>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AuiProjectionDelta {
    pub stages: BTreeMap<AuiCompositionStage, AuiStageProjectionDelta>,
    pub glyphs: Option<AuiGlyphProjectionDelta>,
}

/// Process-local retained projection metadata. It is deliberately absent from
/// the serialized frame contract and from semantic frame equality.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct AuiProjectionHint(Option<Arc<AuiProjectionDelta>>);

impl PartialEq for AuiProjectionHint {
    fn eq(&self, _other: &Self) -> bool {
        true
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
    pub glyph_plan: Option<std::sync::Arc<AuiTextGlyphPlan>>,
    #[serde(default)]
    pub canvas_references: Vec<CanvasReferenceFact>,
    #[serde(skip)]
    #[doc(hidden)]
    pub projection_hint: AuiProjectionHint,
}

impl AuiCompositionFrame {
    pub(crate) fn projection_delta(&self) -> Option<&AuiProjectionDelta> {
        self.projection_hint.0.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn install_projection_hint_from_previous_for_test(&mut self, previous: &Self) {
        let mut delta = AuiProjectionDelta::default();
        if self.stages.len() == previous.stages.len() {
            for (current, old) in self.stages.iter().zip(&previous.stages) {
                if current.stage != old.stage || current.draw_items.len() != old.draw_items.len() {
                    continue;
                }
                let changed_slots = current
                    .draw_items
                    .iter()
                    .zip(old.draw_items.iter())
                    .enumerate()
                    .filter_map(|(slot, (next, old))| (!Arc::ptr_eq(next, old)).then_some(slot))
                    .collect();
                delta.stages.insert(
                    current.stage,
                    AuiStageProjectionDelta {
                        previous_items: old.draw_items.clone(),
                        current_items: current.draw_items.clone(),
                        changed_slots: Arc::new(changed_slots),
                    },
                );
            }
        }
        if let (Some(current), Some(old)) = (&self.glyph_plan, &previous.glyph_plan) {
            let current_groups = glyph_quads_by_item(current);
            let old_groups = glyph_quads_by_item(old);
            let metadata_changed = !glyph_plan_projection_metadata_equal(current, old);
            let mut changed_groups = BTreeMap::new();
            for item_id in current_groups.keys().chain(old_groups.keys()) {
                if changed_groups.contains_key(item_id) {
                    continue;
                }
                let next = current_groups.get(item_id).cloned().unwrap_or_default();
                let previous_group = old_groups.get(item_id).cloned().unwrap_or_default();
                if metadata_changed
                    || next.len() != previous_group.len()
                    || next
                        .iter()
                        .zip(previous_group.iter())
                        .any(|(next, old)| !Arc::ptr_eq(next, old))
                {
                    changed_groups.insert(item_id.clone(), Arc::new(next));
                }
            }
            delta.glyphs = Some(AuiGlyphProjectionDelta {
                previous_plan: old.clone(),
                current_plan: current.clone(),
                changed_groups: Arc::new(changed_groups),
            });
        }
        self.projection_hint = AuiProjectionHint(Some(Arc::new(delta)));
    }

    /// Compare retained drawing identity without scanning UI items or glyphs.
    pub fn shares_drawing(&self, other: &Self) -> bool {
        let glyphs = match (&self.glyph_plan, &other.glyph_plan) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        glyphs
            && self.canvas_references == other.canvas_references
            && self.stages.len() == other.stages.len()
            && self
                .stages
                .iter()
                .zip(&other.stages)
                .all(|(a, b)| a.stage == b.stage && Arc::ptr_eq(&a.draw_items, &b.draw_items))
    }

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
        let draw_items = match self.stages.as_slice() {
            [stage]
                if stage
                    .draw_items
                    .windows(2)
                    .all(|pair| pair[0].sort_key <= pair[1].sort_key) =>
            {
                stage.draw_items.clone()
            }
            stages => {
                let mut items = stages
                    .iter()
                    .flat_map(|stage| stage.draw_items.iter().cloned())
                    .collect::<Vec<_>>();
                items.sort_by_key(|item| item.sort_key);
                Arc::new(items)
            }
        };
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
                graphics_diagnostics: Vec::new(),
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
        glyph_plan: Option<std::sync::Arc<AuiTextGlyphPlan>>,
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
            projection_hint: AuiProjectionHint::default(),
        }
    }
}

#[cfg(test)]
fn glyph_plan_projection_metadata_equal(a: &AuiTextGlyphPlan, b: &AuiTextGlyphPlan) -> bool {
    a.font_atlas_id == b.font_atlas_id
        && a.font_source_kind == b.font_source_kind
        && a.atlas_generation == b.atlas_generation
}

#[cfg(test)]
fn glyph_quads_by_item(plan: &AuiTextGlyphPlan) -> BTreeMap<String, Vec<Arc<AuiTextGlyphQuad>>> {
    let mut groups = BTreeMap::<String, Vec<Arc<AuiTextGlyphQuad>>>::new();
    for quad in &plan.quads {
        groups
            .entry(quad.item_id.clone())
            .or_default()
            .push(quad.clone());
    }
    groups
}

impl AuiDrawingCache {
    fn publish_projection_hint(
        &mut self,
        composition: &mut AuiCompositionFrame,
        current_glyph_plan: Option<Arc<AuiTextGlyphPlan>>,
    ) {
        let previous_glyph_plan =
            std::mem::replace(&mut self.previous_glyph_plan, current_glyph_plan.clone());
        let mut stages = std::mem::take(&mut self.pending_projection_stages);
        let glyphs = match (previous_glyph_plan, current_glyph_plan) {
            (Some(previous_plan), Some(current_plan))
                if Arc::ptr_eq(&previous_plan, &current_plan) =>
            {
                None
            }
            (Some(previous_plan), Some(current_plan)) => {
                if let Some(changed_groups) = self.pending_glyph_groups.take() {
                    Some(AuiGlyphProjectionDelta {
                        previous_plan,
                        current_plan,
                        changed_groups: Arc::new(changed_groups),
                    })
                } else {
                    stages.clear();
                    None
                }
            }
            (None, None) => None,
            _ => {
                stages.clear();
                None
            }
        };
        self.pending_glyph_groups = None;
        composition.projection_hint = if stages.is_empty() {
            AuiProjectionHint::default()
        } else {
            AuiProjectionHint(Some(Arc::new(AuiProjectionDelta { stages, glyphs })))
        };
    }
}

pub struct AuiRendererBridge;

impl AuiRendererBridge {
    pub fn build_overlay_frame(frame_index: u64, draw_list: &AuiDrawList) -> AuiOverlayFrame {
        let mut draw_items = Vec::new();
        let mut text_count = 0;
        let mut image_count = 0;

        for (tree_order, command) in draw_list.commands.iter().enumerate() {
            match command.as_ref() {
                AuiDrawCommand::DrawRect {
                    node_id,
                    rect,
                    effective_clip_rect,
                    color,
                } => draw_items.push(Arc::new(AuiOverlayDrawItem {
                    presentation: None,
                    item_id: format!("aui-item-{}", tree_order + 1),
                    canvas_id: "legacy-overlay".to_string(),
                    composition_stage: AuiCompositionStage::ScreenOverlay,
                    node_id: node_id.clone(),
                    item_kind: overlay_rect_item_kind(node_id),
                    rect: *rect,
                    effective_clip_rect: *effective_clip_rect,
                    effective_clips: Vec::new(),
                    paint: None,
                    color: color.clone(),
                    asset_id: None,
                    text: None,
                    font_size: None,
                    font: None,
                    mesh: None,
                    sort_key: AuiOverlaySortKey {
                        canvas_layer: 0,
                        canvas_sorting_order: 0,
                        tree_order,
                    },
                })),
                AuiDrawCommand::DrawImage {
                    node_id,
                    rect,
                    effective_clip_rect,
                    asset_id,
                    color,
                } => {
                    image_count += 1;
                    draw_items.push(Arc::new(AuiOverlayDrawItem {
                        presentation: None,
                        item_id: format!("aui-item-{}", tree_order + 1),
                        canvas_id: "legacy-overlay".to_string(),
                        composition_stage: AuiCompositionStage::ScreenOverlay,
                        node_id: node_id.clone(),
                        item_kind: AuiOverlayItemKind::Image,
                        rect: *rect,
                        effective_clip_rect: *effective_clip_rect,
                        effective_clips: Vec::new(),
                        paint: None,
                        color: color.clone(),
                        asset_id: Some(asset_id.clone()),
                        text: None,
                        font_size: None,
                        font: None,
                        mesh: None,
                        sort_key: AuiOverlaySortKey {
                            canvas_layer: 0,
                            canvas_sorting_order: 0,
                            tree_order,
                        },
                    }));
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
                    draw_items.push(Arc::new(AuiOverlayDrawItem {
                        presentation: None,
                        item_id: format!("aui-item-{}", tree_order + 1),
                        canvas_id: "legacy-overlay".to_string(),
                        composition_stage: AuiCompositionStage::ScreenOverlay,
                        node_id: node_id.clone(),
                        item_kind: AuiOverlayItemKind::Text,
                        rect: *rect,
                        effective_clip_rect: *effective_clip_rect,
                        effective_clips: Vec::new(),
                        paint: None,
                        color: color.clone(),
                        asset_id: None,
                        text: Some(text.clone()),
                        font_size: *font_size,
                        font: font.clone(),
                        mesh: None,
                        sort_key: AuiOverlaySortKey {
                            canvas_layer: 0,
                            canvas_sorting_order: 0,
                            tree_order,
                        },
                    }));
                }
                AuiDrawCommand::DrawMesh {
                    node_id,
                    rect,
                    effective_clip_rect,
                    mesh,
                } => draw_items.push(Arc::new(AuiOverlayDrawItem {
                    presentation: None,
                    item_id: format!("aui-item-{}", tree_order + 1),
                    canvas_id: "legacy-overlay".to_string(),
                    composition_stage: AuiCompositionStage::ScreenOverlay,
                    node_id: node_id.clone(),
                    item_kind: AuiOverlayItemKind::Mesh,
                    rect: *rect,
                    effective_clip_rect: *effective_clip_rect,
                    effective_clips: Vec::new(),
                    paint: None,
                    color: None,
                    asset_id: mesh.asset_id.clone(),
                    text: None,
                    font_size: None,
                    font: None,
                    mesh: Some(mesh.clone()),
                    sort_key: AuiOverlaySortKey {
                        canvas_layer: 0,
                        canvas_sorting_order: 0,
                        tree_order,
                    },
                })),
            }
        }

        AuiOverlayFrame {
            frame_index,
            report: AuiRenderReport {
                draw_command_count: draw_items.len(),
                text_count,
                image_count,
                graphics_diagnostics: Vec::new(),
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
            draw_items: draw_items.into(),
            glyph_plan: None,
        }
    }

    pub fn build_composition_frame(
        frame_index: u64,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        draw_list: &AuiDrawList,
    ) -> AuiCompositionFrame {
        let mut indices = drawing_cache::DrawingIndices::default();
        indices.refresh(document, layout);
        Self::build_composition_indexed(frame_index, document, layout, draw_list, &indices, None)
    }

    fn build_composition_frame_cached(
        frame_index: u64,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        draw_list: &AuiDrawList,
        cache: &mut AuiDrawingCache,
    ) -> AuiCompositionFrame {
        let mut indices = std::mem::take(&mut cache.indices);
        if !cache.has_recorded_layout(layout) {
            let (rebuilds, entries) = indices.refresh(document, layout);
            cache.stats.index_rebuilds += rebuilds;
            cache.stats.index_entries_built += entries;
        }
        let frame = Self::build_composition_indexed(
            frame_index,
            document,
            layout,
            draw_list,
            &indices,
            Some(cache),
        );
        indices.trim();
        cache.indices = indices;
        frame
    }

    // Only the immediate extraction owner can attest to command-slot provenance.
    // The cached/public-value path above keeps its conservative comparisons.
    fn build_composition_frame_retained(
        frame_index: u64,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        draw_list: &AuiDrawList,
        cache: &mut AuiDrawingCache,
    ) -> AuiCompositionFrame {
        if let Some(frame) =
            Self::update_composition_slots(frame_index, document, layout, draw_list, cache)
        {
            return frame;
        }
        let frame =
            Self::build_composition_frame_cached(frame_index, document, layout, draw_list, cache);
        cache.stage_frames = frame.stages.clone();
        frame
    }

    fn update_composition_slots(
        frame_index: u64,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        draw_list: &AuiDrawList,
        cache: &mut AuiDrawingCache,
    ) -> Option<AuiCompositionFrame> {
        let slots = cache.pending_command_slots.take()?;
        if cache.items.len() != draw_list.commands.len()
            || cache.item_stage_slots.len() != cache.items.len()
            || cache.stage_frames.is_empty()
            || document
                .canvases
                .iter()
                .any(|canvas| canvas.mode != AuiCanvasMode::ScreenOverlay)
        {
            return None;
        }
        // Validate the entire changed batch before publishing any fragment or hint.
        let mut updates = Vec::with_capacity(slots.len());
        for slot in slots {
            cache.stats.items_visited += 1;
            let command = draw_list.commands.get(slot)?;
            let command_id = draw_command_node_id(command);
            let source_id = if cache.indices.computed.get(command_id).is_some() {
                command_id
            } else {
                draw_command_source_node_id(command_id)
            };
            let computed = &layout.computed_nodes[cache.indices.computed.get(source_id)?];
            let canvas = &document.canvases[cache.indices.canvases.get(&computed.canvas_id)?];
            let old = &cache.items[slot].item;
            let sort_key = AuiOverlaySortKey {
                canvas_layer: canvas.layer,
                canvas_sorting_order: canvas.sorting_order,
                tree_order: computed.tree_order.saturating_mul(1000) + slot,
            };
            if old.composition_stage != computed.composition_stage || old.sort_key != sort_key {
                return None;
            }
            let mut item = overlay_item_from_draw_command(
                command,
                slot,
                &canvas.canvas_id,
                computed.composition_stage,
                sort_key,
            );
            item.effective_clips = computed.effective_clips.clone();
            item.presentation = computed.presentation;
            if command_id == source_id
                && matches!(command.as_ref(), AuiDrawCommand::DrawRect { .. })
            {
                item.paint = cache
                    .indices
                    .nodes
                    .get(source_id)
                    .and_then(|node| {
                        cache
                            .draw_node_overrides
                            .get(&node)
                            .unwrap_or(&document.nodes[node])
                            .resolved_style
                            .as_ref()
                    })
                    .map(|style| style.paint.clone());
            }
            if old.item_kind != item.item_kind || old.canvas_id != item.canvas_id {
                return None;
            }
            updates.push((slot, command.clone(), Arc::new(item)));
        }
        cache.pending_projection_stages.clear();
        let mut per_stage = BTreeMap::<AuiCompositionStage, Vec<usize>>::new();
        for (slot, command, item) in updates {
            if Arc::ptr_eq(&cache.items[slot].command, &command) && cache.items[slot].item == item {
                cache.stats.item_payloads_reused += 1;
                continue;
            }
            let (stage, stage_slot) = cache.item_stage_slots[slot];
            per_stage.entry(stage).or_default().push(stage_slot);
            cache.items[slot] = drawing_cache::OverlayFragment { command, item };
            cache.stats.item_payloads_built += 1;
        }
        for stage in &mut cache.stage_frames {
            let previous_items = stage.draw_items.clone();
            let changed = per_stage.remove(&stage.stage).unwrap_or_default();
            if !changed.is_empty() {
                cache.stats.item_refs_copied += stage.draw_items.len() as u64;
                let items = Arc::make_mut(&mut stage.draw_items);
                for slot in &changed {
                    // Reverse lookup is populated at structural publication, never per-frame.
                    let command_slot = cache.stage_command_slots[&stage.stage][*slot].1;
                    items[*slot] = cache.items[command_slot].item.clone();
                }
            }
            cache
                .projection_stages
                .insert(stage.stage, stage.draw_items.clone());
            cache.pending_projection_stages.insert(
                stage.stage,
                AuiStageProjectionDelta {
                    previous_items,
                    current_items: stage.draw_items.clone(),
                    changed_slots: Arc::new(changed),
                },
            );
        }
        let mut frame =
            AuiCompositionFrame::from_stage_frames(frame_index, cache.stage_frames.clone(), None);
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
        Some(frame)
    }

    fn build_composition_indexed(
        frame_index: u64,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        draw_list: &AuiDrawList,
        indices: &drawing_cache::DrawingIndices,
        mut cache: Option<&mut AuiDrawingCache>,
    ) -> AuiCompositionFrame {
        let mut stage_items = AuiCompositionStage::ordered()
            .into_iter()
            .map(|stage| (stage, Vec::<Arc<AuiOverlayDrawItem>>::new()))
            .collect::<HashMap<_, _>>();
        let mut previous = cache
            .as_mut()
            .map(|cache| std::mem::take(&mut cache.items))
            .unwrap_or_default()
            .into_iter();
        let retain = cache.is_some() && draw_list.commands.len() <= drawing_cache::MAX_DRAW_NODES;
        let mut cacheable = retain;
        let mut fragments = Vec::with_capacity(draw_list.commands.len());
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
            if let Some(cache) = cache.as_mut() {
                cache.stats.items_visited += 1;
            }
            let old = previous.next();
            let command_node_id = draw_command_node_id(command);
            let source_node_id = if indices.computed.get(command_node_id).is_some() {
                command_node_id
            } else {
                draw_command_source_node_id(command_node_id)
            };
            let Some(computed) = indices
                .computed
                .get(source_node_id)
                .map(|slot| &layout.computed_nodes[slot])
            else {
                cacheable = false;
                diagnostics.push(format!(
                    "aui_draw_command_missing_computed_node:{}",
                    command_node_id
                ));
                continue;
            };
            let Some(canvas) = indices
                .canvases
                .get(computed.canvas_id.as_str())
                .map(|slot| &document.canvases[slot])
            else {
                cacheable = false;
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
            let paint = if command_node_id == source_node_id
                && matches!(command.as_ref(), AuiDrawCommand::DrawRect { .. })
            {
                indices
                    .nodes
                    .get(source_node_id)
                    .and_then(|slot| {
                        cache
                            .as_ref()
                            .and_then(|cache| cache.draw_node_overrides.get(&slot))
                            .unwrap_or(&document.nodes[slot])
                            .resolved_style
                            .as_ref()
                    })
                    .map(|style| style.paint.clone())
            } else {
                None
            };
            let item = if cacheable
                && old.as_ref().is_some_and(|old| {
                    Arc::ptr_eq(&old.command, command)
                        && old.item.canvas_id == canvas.canvas_id
                        && old.item.composition_stage == stage
                        && old.item.sort_key == sort_key
                        && old.item.effective_clips == computed.effective_clips
                        && old.item.presentation == computed.presentation
                        && old.item.paint == paint
                }) {
                if let Some(cache) = cache.as_mut() {
                    cache.stats.item_payloads_reused += 1;
                }
                old.expect("checked cached overlay fragment").item
            } else {
                let mut item = overlay_item_from_draw_command(
                    command,
                    command_index,
                    canvas.canvas_id.as_str(),
                    stage,
                    sort_key,
                );
                item.effective_clips = computed.effective_clips.clone();
                item.presentation = computed.presentation;
                item.paint = paint;
                if let Some(cache) = cache.as_mut() {
                    cache.stats.item_payloads_built += 1;
                }
                Arc::new(item)
            };
            if retain {
                fragments.push(drawing_cache::OverlayFragment {
                    command: command.clone(),
                    item: item.clone(),
                });
            }
            if let Some(items) = stage_items.get_mut(&stage) {
                items.push(item);
            }
        }

        if let Some(cache) = cache.as_mut() {
            cache.items = if cacheable { fragments } else { Vec::new() };
        }

        let mut previous_projection_stages = cache
            .as_mut()
            .map(|cache| std::mem::take(&mut cache.projection_stages))
            .unwrap_or_default();
        if let Some(cache) = cache.as_mut() {
            cache.pending_projection_stages.clear();
        }
        let mut stages = Vec::new();
        for stage in AuiCompositionStage::ordered() {
            let mut draw_items = stage_items.remove(&stage).unwrap_or_default();
            draw_items.sort_by_key(|item| item.sort_key);
            if !draw_items.is_empty() {
                let mut draw_items = Arc::new(draw_items);
                if retain && cacheable {
                    if let Some(previous_items) = previous_projection_stages.remove(&stage) {
                        if previous_items.len() == draw_items.len() {
                            let changed_slots = previous_items
                                .iter()
                                .zip(draw_items.iter())
                                .enumerate()
                                .filter_map(|(slot, (old, next))| {
                                    (!Arc::ptr_eq(old, next)).then_some(slot)
                                })
                                .collect::<Vec<_>>();
                            if changed_slots.is_empty() {
                                draw_items = previous_items.clone();
                            }
                            if let Some(cache) = cache.as_mut() {
                                cache.pending_projection_stages.insert(
                                    stage,
                                    AuiStageProjectionDelta {
                                        previous_items,
                                        current_items: draw_items.clone(),
                                        changed_slots: Arc::new(changed_slots),
                                    },
                                );
                            }
                        }
                    }
                    if let Some(cache) = cache.as_mut() {
                        cache.projection_stages.insert(stage, draw_items.clone());
                    }
                }
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
        if let Some(cache) = cache.as_mut() {
            cache.item_stage_slots.clear();
            cache.stage_command_slots.clear();
            if cacheable {
                let mut positions = HashMap::new();
                for stage in &frame.stages {
                    for (slot, item) in stage.draw_items.iter().enumerate() {
                        positions.insert(item.item_id.as_str(), (stage.stage, slot));
                    }
                }
                for (command_slot, fragment) in cache.items.iter().enumerate() {
                    if let Some(&(stage, slot)) = positions.get(fragment.item.item_id.as_str()) {
                        cache.item_stage_slots.push((stage, slot));
                        cache
                            .stage_command_slots
                            .entry(stage)
                            .or_default()
                            .push((slot, command_slot));
                    }
                }
                for commands in cache.stage_command_slots.values_mut() {
                    commands.sort_unstable_by_key(|pair| pair.0);
                }
            }
        }
        frame
    }
}

fn draw_command_node_id(command: &AuiDrawCommand) -> &str {
    match command {
        AuiDrawCommand::DrawRect { node_id, .. }
        | AuiDrawCommand::DrawImage { node_id, .. }
        | AuiDrawCommand::DrawText { node_id, .. }
        | AuiDrawCommand::DrawMesh { node_id, .. } => node_id.as_str(),
    }
}

fn draw_command_source_node_id(node_id: &str) -> &str {
    node_id.rsplit_once(':').map_or(node_id, |(base, _)| base)
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
        }
        | AuiDrawCommand::DrawMesh {
            effective_clip_rect,
            ..
        } => *effective_clip_rect,
    }
}

fn draw_command_count_for_node(node: &AuiNode) -> usize {
    match node.kind {
        AuiNodeKind::Panel
        | AuiNodeKind::InputField
        | AuiNodeKind::Toggle
        | AuiNodeKind::Slider
        | AuiNodeKind::List
        | AuiNodeKind::ScrollView => 1,
        AuiNodeKind::Button => 1 + usize::from(node.text.is_some()),
        AuiNodeKind::ProgressBar => 2,
        AuiNodeKind::Image => usize::from(node.image.is_some()),
        AuiNodeKind::Text => usize::from(node.text.is_some()),
        AuiNodeKind::Custom => usize::from(
            node.custom_runtime
                .as_ref()
                .and_then(|runtime| runtime.paint.as_ref())
                .is_some(),
        ),
    }
}

fn aui_visual_override_owner<'d, 'v>(
    document: &'d AuiDocument,
    indices: &drawing_cache::DrawingIndices,
    node_id: &'d str,
    overrides: &'v crate::aui_control_feedback::AuiVisualOverrideSet,
) -> Option<(
    &'d str,
    &'v crate::aui_control_feedback::AuiControlVisualOverride,
)> {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        if let Some(visual) = overrides.get(current_id) {
            return Some((current_id, visual));
        }
        current = indices
            .nodes
            .get(current_id)
            .and_then(|slot| document.nodes[slot].parent.as_deref());
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

fn custom_primitive_rect(owner: AuiComputedRect, local: [f32; 4]) -> AuiComputedRect {
    AuiComputedRect {
        x: owner.x + local[0],
        y: owner.y + local[1],
        width: local[2],
        height: local[3],
    }
}

fn extension_color(color: [f32; 4]) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgb = |value: f32| {
        channel(
            project_runtime_sdk::aui_presentation::linear_channel_to_srgb(value.clamp(0.0, 1.0)),
        )
    };
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        rgb(color[0]),
        rgb(color[1]),
        rgb(color[2]),
        channel(color[3])
    )
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
    let brighten = |component: u8| {
        use project_runtime_sdk::aui_presentation::{
            linear_channel_to_srgb, srgb_channel_to_linear,
        };
        let linear = srgb_channel_to_linear(f32::from(component) / 255.0);
        (linear_channel_to_srgb((linear * visual.brightness_multiplier).clamp(0.0, 1.0)) * 255.0)
            .round() as u8
    };
    let red = brighten(parse(0)?);
    let green = brighten(parse(2)?);
    let blue = brighten(parse(4)?);
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
            presentation: None,
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
            effective_clips: Vec::new(),
            paint: None,
            color: color.clone(),
            asset_id: None,
            text: None,
            font_size: None,
            font: None,
            mesh: None,
            sort_key,
        },
        AuiDrawCommand::DrawImage {
            node_id,
            rect,
            effective_clip_rect,
            asset_id,
            color,
        } => AuiOverlayDrawItem {
            presentation: None,
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
            effective_clips: Vec::new(),
            paint: None,
            color: color.clone(),
            asset_id: Some(asset_id.clone()),
            text: None,
            font_size: None,
            font: None,
            mesh: None,
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
            presentation: None,
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
            effective_clips: Vec::new(),
            paint: None,
            color: color.clone(),
            asset_id: None,
            text: Some(text.clone()),
            font_size: *font_size,
            font: font.clone(),
            mesh: None,
            sort_key,
        },
        AuiDrawCommand::DrawMesh {
            node_id,
            rect,
            effective_clip_rect,
            mesh,
        } => AuiOverlayDrawItem {
            presentation: None,
            item_id: format!(
                "aui-{}-item-{}",
                composition_stage.pass_id_suffix(),
                command_index + 1
            ),
            canvas_id: canvas_id.to_string(),
            composition_stage,
            node_id: node_id.clone(),
            item_kind: AuiOverlayItemKind::Mesh,
            rect: *rect,
            effective_clip_rect: *effective_clip_rect,
            effective_clips: Vec::new(),
            paint: None,
            color: None,
            asset_id: mesh.asset_id.clone(),
            text: None,
            font_size: None,
            font: None,
            mesh: Some(mesh.clone()),
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
    pub viewport_status: String,
    pub viewport_input_identity: Option<String>,
    pub viewport_canvas_count: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiRuntimePresentOutput {
    pub overlay: AuiOverlayFrame,
    pub composition: AuiCompositionFrame,
    pub resolved_document: Arc<AuiDocument>,
    pub layout: Arc<AuiLayoutResult>,
    #[serde(
        default,
        rename = "virtualLists",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub virtual_lists: Vec<crate::aui_virtual_list::AuiVirtualListWindow>,
    #[serde(skip)]
    pub paragraph_layouts: Arc<HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>>,
    pub report: AuiRuntimePresentReport,
    #[serde(skip)]
    base_draw_report: Option<AuiRetainedDrawReport>,
}

// Equality describes the published output, not the presence of an internal cache.
impl PartialEq for AuiRuntimePresentOutput {
    fn eq(&self, other: &Self) -> bool {
        self.overlay == other.overlay
            && self.composition == other.composition
            && self.resolved_document == other.resolved_document
            && self.layout == other.layout
            && self.virtual_lists == other.virtual_lists
            && self.paragraph_layouts == other.paragraph_layouts
            && self.report == other.report
    }
}

#[derive(Debug, Clone)]
struct AuiRetainedDrawReport {
    document: Arc<AuiDocument>,
    layout: Arc<AuiLayoutResult>,
    report: AuiRenderReport,
    composition: AuiCompositionFrame,
    overlay: AuiOverlayFrame,
    atlases: RuntimeAuiFontAtlasRegistry,
    bundles: RuntimeFontBundleRegistry,
    themes: crate::aui_theme::AuiThemeRegistry,
    presentation: Option<ResolvedGameViewPresentation>,
}

impl AuiRuntimePresentOutput {
    /// Base document statistics, before interaction feedback is applied.
    /// Retaining source identities also makes external COW edits invalidate the result.
    pub fn base_draw_report(&self) -> std::borrow::Cow<'_, AuiRenderReport> {
        if let Some(cached) = &self.base_draw_report {
            if Arc::ptr_eq(&cached.document, &self.resolved_document)
                && Arc::ptr_eq(&cached.layout, &self.layout)
            {
                return std::borrow::Cow::Borrowed(&cached.report);
            }
        }
        std::borrow::Cow::Owned(
            AuiLayoutEngine::extract_draw_list(&self.resolved_document, &self.layout).1,
        )
    }
}

pub struct AuiRuntimePresenter;

/// Cumulative nanoseconds captured only during an explicit performance run.
/// Production/exchange includes the producer's wire encoding, native transfer and decode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiUpdateTimings {
    pub production_exchange_diff_ns: u64,
    pub prepare_style_preflight_ns: u64,
    pub measure_arrange_ns: u64,
    pub transform_clip_finalize_ns: u64,
    pub draw_projection_glyph_ns: u64,
}

impl AuiUpdateTimings {
    pub fn since(self, previous: Self) -> Self {
        Self {
            production_exchange_diff_ns: self
                .production_exchange_diff_ns
                .saturating_sub(previous.production_exchange_diff_ns),
            prepare_style_preflight_ns: self
                .prepare_style_preflight_ns
                .saturating_sub(previous.prepare_style_preflight_ns),
            measure_arrange_ns: self
                .measure_arrange_ns
                .saturating_sub(previous.measure_arrange_ns),
            transform_clip_finalize_ns: self
                .transform_clip_finalize_ns
                .saturating_sub(previous.transform_clip_finalize_ns),
            draw_projection_glyph_ns: self
                .draw_projection_glyph_ns
                .saturating_sub(previous.draw_projection_glyph_ns),
        }
    }

    pub fn total_ns(self) -> u64 {
        self.production_exchange_diff_ns
            .saturating_add(self.prepare_style_preflight_ns)
            .saturating_add(self.measure_arrange_ns)
            .saturating_add(self.transform_clip_finalize_ns)
            .saturating_add(self.draw_projection_glyph_ns)
    }
}

#[doc(hidden)]
pub fn aui_elapsed_ns(start: Option<crate::platform_time::Instant>) -> u64 {
    start.map_or(0, |start| {
        start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    })
}

/// Cumulative owner work, not allocator calls or process-wide heap bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiUpdateWork {
    pub active_binding_paths_compared: u64,
    pub paragraph_preflight_nodes_visited: u64,
    pub full_prepare_count: u64,
    pub full_prepare_node_count: u64,
    pub prepared_nodes_rebuilt: u64,
    pub prepared_document_nodes_copied: u64,
    pub layout_source_nodes_copied: u64,
    pub layout_nodes_visited: u64,
    pub layout_measure_calls: u64,
    pub layout_source_nodes_loaded: u64,
    pub layout_geometry_nodes: u64,
    pub geometry_nodes_updated: u64,
    pub geometry_source_nodes_compared: u64,
    pub geometry_snapshot_nodes_copied: u64,
    pub translation_entries_compared: u64,
}

impl AuiUpdateWork {
    pub fn since(self, old: Self) -> Self {
        Self {
            active_binding_paths_compared: self
                .active_binding_paths_compared
                .saturating_sub(old.active_binding_paths_compared),
            paragraph_preflight_nodes_visited: self
                .paragraph_preflight_nodes_visited
                .saturating_sub(old.paragraph_preflight_nodes_visited),
            full_prepare_count: self
                .full_prepare_count
                .saturating_sub(old.full_prepare_count),
            full_prepare_node_count: self
                .full_prepare_node_count
                .saturating_sub(old.full_prepare_node_count),
            prepared_nodes_rebuilt: self
                .prepared_nodes_rebuilt
                .saturating_sub(old.prepared_nodes_rebuilt),
            prepared_document_nodes_copied: self
                .prepared_document_nodes_copied
                .saturating_sub(old.prepared_document_nodes_copied),
            layout_source_nodes_copied: self
                .layout_source_nodes_copied
                .saturating_sub(old.layout_source_nodes_copied),
            layout_nodes_visited: self
                .layout_nodes_visited
                .saturating_sub(old.layout_nodes_visited),
            layout_measure_calls: self
                .layout_measure_calls
                .saturating_sub(old.layout_measure_calls),
            layout_source_nodes_loaded: self
                .layout_source_nodes_loaded
                .saturating_sub(old.layout_source_nodes_loaded),
            layout_geometry_nodes: self
                .layout_geometry_nodes
                .saturating_sub(old.layout_geometry_nodes),
            geometry_nodes_updated: self
                .geometry_nodes_updated
                .saturating_sub(old.geometry_nodes_updated),
            geometry_source_nodes_compared: self
                .geometry_source_nodes_compared
                .saturating_sub(old.geometry_source_nodes_compared),
            geometry_snapshot_nodes_copied: self
                .geometry_snapshot_nodes_copied
                .saturating_sub(old.geometry_snapshot_nodes_copied),
            translation_entries_compared: self
                .translation_entries_compared
                .saturating_sub(old.translation_entries_compared),
        }
    }
}

pub use crate::aui_geometry_cache::AuiGeometryUpdateStats;

#[derive(Clone)]
struct AuiPreparedInput {
    document: Arc<AuiDocument>,
    node_slots: Arc<HashMap<String, usize>>,
    binding_slots: Arc<HashMap<String, Vec<(usize, AuiBindingTarget)>>>,
    binding_report: AuiBindingReport,
    viewport_diagnostics: Vec<crate::aui_viewport::AuiViewportDiagnostic>,
    viewport_input_identity: Option<String>,
    viewport_canvas_count: usize,
}

#[derive(Default)]
pub struct AuiC2Runtime {
    pub(crate) measurement_enabled: bool,
    pub(crate) timings: AuiUpdateTimings,
    pub(crate) work: AuiUpdateWork,
    drawing_cache: AuiDrawingCache,
    prepared_input: Option<AuiPreparedInput>,
    pub(crate) reuse_prepared_input_once: bool,
    pub(crate) changed_binding_paths_once: Vec<String>,
    pub(crate) apply_retained_changes_to_prepared_input_once: bool,
    prepared_input_reused: bool,
    prepared_document_nodes_copied: usize,
    prepared_binding_nodes_rebuilt: usize,
    solver_build_count: u64,
    layout_source: Option<crate::aui_layout_runtime::AuiLayoutSource>,
    layout_source_nodes_copied: usize,
    solvers: HashMap<String, crate::aui_layout_runtime::AuiLayoutSolver>,
    paragraph_cache: crate::aui_paragraph::AuiParagraphCache,
    source: Option<Arc<AuiDocument>>,
    source_change_hint: Option<Vec<usize>>,
    retained_key: Option<(u64, u64)>,
    retained_document_id: Option<String>,
    retained_properties:
        BTreeMap<(String, project_runtime_sdk::aui::UiProperty), project_runtime_sdk::aui::UiValue>,
    retained_custom_outputs: Vec<(String, crate::aui_extension::AuiCustomRuntimeOutput)>,
    retained_changed_nodes: HashSet<String>,
    retained_value_removed: bool,
    retained_full_prepare_required: bool,
    retained_properties_read: usize,
    translations: HashMap<String, [f32; 2]>,
    presentations: HashMap<
        String,
        BTreeMap<project_runtime_sdk::aui::UiProperty, project_runtime_sdk::aui::UiValue>,
    >,
    retained_translation_changes: HashSet<String>,
    retained_translation_changes_known: bool,
    geometry_cache: Option<crate::aui_geometry_cache::AuiGeometryCache>,
    geometry_stats: AuiGeometryUpdateStats,
    cached_paragraphs: Arc<HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>>,
    custom_measure_inputs:
        HashMap<String, project_runtime_sdk::aui_extension::UiExtensionConstraints>,
    finalized_paragraphs: std::collections::HashSet<String>,
    layout_fonts: Option<RuntimeFontBundleRegistry>,
    interaction_index_journal: AuiInteractionIndexJournal,
    published_layout: Option<Arc<AuiLayoutResult>>,
}

impl AuiC2Runtime {
    pub fn update_work(&self) -> AuiUpdateWork {
        self.work
    }
    pub fn drawing_stats(&self) -> AuiDrawingStats {
        self.drawing_cache.stats
    }

    /// Read only existing geometry counters during explicit performance capture.
    pub fn graphics_cache_measurement(&self) -> (u64, u64, usize) {
        let cache = &self.drawing_cache.graphics;
        (cache.builds, cache.hits, cache.resident_bytes())
    }
    pub fn graphics_cache_stats(&self) -> (u64, u64, usize) {
        let cache = &self.drawing_cache.graphics;
        (cache.builds, cache.hits, cache.resident_bytes())
    }
    pub fn prepared_input_reused(&self) -> bool {
        self.prepared_input_reused
    }

    pub fn prepared_document_nodes_copied(&self) -> usize {
        self.prepared_document_nodes_copied
    }

    /// Source node objects cloned to reapply local bindings, excluding COW document copies.
    pub fn prepared_binding_nodes_rebuilt(&self) -> usize {
        self.prepared_binding_nodes_rebuilt
    }

    pub fn solver_build_count(&self) -> u64 {
        self.solver_build_count
    }

    pub fn layout_source_nodes_copied(&self) -> usize {
        self.layout_source_nodes_copied
    }

    pub fn solver_node_count(&self, canvas: &str) -> Option<usize> {
        self.solvers
            .get(canvas)
            .map(crate::aui_layout_runtime::AuiLayoutSolver::resident_node_count)
    }

    pub fn current_layout(&self) -> Option<&Arc<AuiLayoutResult>> {
        self.published_layout.as_ref()
    }
    pub fn retained_properties_read(&self) -> usize {
        self.retained_properties_read
    }
    pub(crate) fn retained_change_pending(
        &self,
        instance: Option<&crate::aui_instance::AuiInstance>,
    ) -> bool {
        instance.and_then(crate::aui_instance::AuiInstance::presentation_key) != self.retained_key
    }
    pub(crate) fn retained_full_prepare_required(&self) -> bool {
        self.retained_full_prepare_required
    }
    /// Observe accepted instance edits without consuming its dirty queue. Clean frames are O(1).
    /// Package resources are immutable for this cache's lifetime; package reload creates a new cache.
    pub fn observe_retained(
        &mut self,
        instance: Option<&crate::aui_instance::AuiInstance>,
    ) -> bool {
        let key = instance.and_then(crate::aui_instance::AuiInstance::presentation_key);
        if key == self.retained_key {
            self.retained_properties_read = 0;
            return false;
        }
        let previous_key = self.retained_key;
        self.retained_changed_nodes.clear();
        self.retained_value_removed = false;
        self.retained_full_prepare_required = false;
        self.retained_properties_read = 0;
        if let (Some((old_instance, old_revision)), Some((new_instance, _)), Some(instance)) =
            (previous_key, key, instance)
        {
            if old_instance == new_instance {
                if let Some(changes) = instance.presentation_changes_since(old_revision) {
                    if changes
                        .iter()
                        .all(|(slot, _)| instance.presentation_node_id_at(*slot).is_some())
                    {
                        for (slot, change) in changes {
                            let node_id = instance
                                .presentation_node_id_at(slot)
                                .expect("checked live slot");
                            let node_id = node_id.to_owned();
                            self.retained_properties_read += 1;
                            let crate::aui_instance::AuiPresentationChange::Property(property) =
                                change
                            else {
                                let output = instance.source().nodes[slot as usize]
                                    .custom_runtime
                                    .as_ref();
                                let old = self
                                    .retained_custom_outputs
                                    .iter()
                                    .position(|(id, _)| id == &node_id);
                                match (old, output) {
                                    (Some(index), Some(output)) => {
                                        if &self.retained_custom_outputs[index].1 != output {
                                            self.retained_changed_nodes.insert(node_id);
                                            self.retained_custom_outputs[index].1 = output.clone();
                                        }
                                    }
                                    (None, Some(output)) => {
                                        self.retained_changed_nodes.insert(node_id.clone());
                                        self.retained_custom_outputs
                                            .push((node_id, output.clone()));
                                    }
                                    (Some(index), None) => {
                                        self.retained_custom_outputs.remove(index);
                                        self.retained_changed_nodes.insert(node_id);
                                        self.retained_value_removed = true;
                                    }
                                    (None, None) => {}
                                }
                                continue;
                            };
                            if property.is_presentation() {
                                self.retained_translation_changes.insert(node_id.clone());
                                if let Some((_, value)) =
                                    instance.presentation_property_at(slot, property)
                                {
                                    if let project_runtime_sdk::aui::UiValue::Translation(offset) =
                                        &value
                                    {
                                        self.translations.insert(node_id.clone(), *offset);
                                    }
                                    self.presentations
                                        .entry(node_id)
                                        .or_default()
                                        .insert(property, value);
                                } else {
                                    if property == project_runtime_sdk::aui::UiProperty::Translation
                                    {
                                        self.translations.remove(&node_id);
                                    }
                                    if let Some(overrides) = self.presentations.get_mut(&node_id) {
                                        overrides.remove(&property);
                                        if overrides.is_empty() {
                                            self.presentations.remove(&node_id);
                                        }
                                    }
                                }
                            } else {
                                let old = self
                                    .retained_properties
                                    .remove(&(node_id.clone(), property));
                                if let Some((_, value)) =
                                    instance.presentation_property_at(slot, property)
                                {
                                    if old.as_ref().is_none_or(|old_value| old_value != &value) {
                                        self.retained_changed_nodes.insert(node_id.clone());
                                    }
                                    self.retained_properties.insert((node_id, property), value);
                                } else if old.is_some() {
                                    self.retained_changed_nodes.insert(node_id);
                                    self.retained_value_removed = true;
                                }
                            }
                        }
                        self.retained_key = key;
                        return true;
                    }
                }
                self.retained_full_prepare_required = true;
            }
        }
        let previous_properties = std::mem::take(&mut self.retained_properties);
        self.retained_translation_changes_known = false;
        self.retained_translation_changes.clear();
        let previous_custom_outputs = std::mem::take(&mut self.retained_custom_outputs);
        self.retained_key = key;
        self.translations.clear();
        self.presentations.clear();
        self.retained_document_id = None;
        if let Some(instance) = instance.filter(|_| key.is_some()) {
            self.retained_document_id = Some(instance.source().document_id.clone());
            for (_, node, property, value) in instance.presentation_properties_with_slots() {
                self.retained_properties_read += 1;
                if property.is_presentation() {
                    if let project_runtime_sdk::aui::UiValue::Translation(offset) = &value {
                        self.translations.insert(node.to_owned(), *offset);
                    }
                    self.presentations
                        .entry(node.to_owned())
                        .or_default()
                        .insert(property, value);
                } else {
                    self.retained_properties
                        .insert((node.to_owned(), property), value);
                }
            }
            self.retained_custom_outputs.extend(
                instance
                    .custom_runtime_outputs()
                    .map(|(node, output)| (node.to_owned(), output.clone())),
            );
        }
        if previous_key
            .zip(key)
            .is_some_and(|(old, new)| old.0 == new.0)
        {
            for ((node_id, property), value) in &self.retained_properties {
                if previous_properties.get(&(node_id.clone(), *property)) != Some(value) {
                    self.retained_changed_nodes.insert(node_id.clone());
                }
            }
            for ((node_id, property), value) in &previous_properties {
                if self.retained_properties.get(&(node_id.clone(), *property)) != Some(value) {
                    self.retained_changed_nodes.insert(node_id.clone());
                }
                if !self
                    .retained_properties
                    .contains_key(&(node_id.clone(), *property))
                {
                    self.retained_value_removed = true;
                }
            }
            for (node_id, output) in &self.retained_custom_outputs {
                if !previous_custom_outputs
                    .iter()
                    .any(|(old_id, old)| old_id == node_id && old == output)
                {
                    self.retained_changed_nodes.insert(node_id.clone());
                }
            }
            for (node_id, output) in &previous_custom_outputs {
                if !self
                    .retained_custom_outputs
                    .iter()
                    .any(|(new_id, new)| new_id == node_id && new == output)
                {
                    self.retained_changed_nodes.insert(node_id.clone());
                }
                if !self
                    .retained_custom_outputs
                    .iter()
                    .any(|(new_id, _)| new_id == node_id)
                {
                    self.retained_value_removed = true;
                }
            }
        }
        true
    }

    pub(crate) fn retained_value_removed(&self) -> bool {
        self.retained_value_removed
    }

    fn apply_retained_properties(&self, document: &mut AuiDocument) {
        if self.retained_document_id.as_deref() != Some(document.document_id.as_str())
            || (self.retained_properties.is_empty() && self.retained_custom_outputs.is_empty())
        {
            return;
        }
        let slots: HashMap<_, _> = document
            .nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| (node.node_id.clone(), slot))
            .collect();
        let uses_c2 =
            document.schema_version == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION;
        for ((id, property), value) in &self.retained_properties {
            let Some(&slot) = slots.get(id) else {
                continue;
            };
            apply_retained_value(&mut document.nodes[slot], *property, value, uses_c2);
        }
        for (id, output) in &self.retained_custom_outputs {
            if let Some(&slot) = slots.get(id) {
                document.nodes[slot].custom_runtime = Some(output.clone());
                if let Some(resource) = self
                    .retained_properties
                    .get(&(id.clone(), project_runtime_sdk::aui::UiProperty::Resource))
                {
                    apply_retained_value(
                        &mut document.nodes[slot],
                        project_runtime_sdk::aui::UiProperty::Resource,
                        resource,
                        uses_c2,
                    );
                }
            }
        }
    }

    fn apply_retained_changes_to_prepared_input(&mut self, prepared: &mut AuiPreparedInput) {
        let source_was_prepared = self
            .source
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(source, &prepared.document));
        if source_was_prepared {
            self.source = None;
        }
        self.prepared_document_nodes_copied = if Arc::strong_count(&prepared.document) > 1 {
            prepared.document.nodes.len()
        } else {
            0
        };
        let document = Arc::make_mut(&mut prepared.document);
        let uses_c2 =
            document.schema_version == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION;
        let mut slots = self
            .retained_changed_nodes
            .iter()
            .filter_map(|node_id| prepared.node_slots.get(node_id).copied())
            .collect::<Vec<_>>();
        slots.sort_unstable();
        slots.dedup();
        for &slot in &slots {
            let node_id = document.nodes[slot].node_id.clone();
            for ((_, property), value) in self.retained_properties.range(
                (node_id.clone(), project_runtime_sdk::aui::UiProperty::Text)
                    ..=(
                        node_id.clone(),
                        project_runtime_sdk::aui::UiProperty::LocalStyle,
                    ),
            ) {
                apply_retained_value(&mut document.nodes[slot], *property, value, uses_c2);
            }
            for (id, output) in &self.retained_custom_outputs {
                if id == &node_id {
                    document.nodes[slot].custom_runtime = Some(output.clone());
                    if let Some(resource) = self
                        .retained_properties
                        .get(&(id.clone(), project_runtime_sdk::aui::UiProperty::Resource))
                    {
                        apply_retained_value(
                            &mut document.nodes[slot],
                            project_runtime_sdk::aui::UiProperty::Resource,
                            resource,
                            uses_c2,
                        );
                    }
                }
            }
        }
        self.source_change_hint = Some(slots);
        if source_was_prepared {
            self.source = Some(Arc::clone(&prepared.document));
        }
    }

    fn apply_changed_bindings_to_prepared_input(
        &mut self,
        prepared: &mut AuiPreparedInput,
        source: &AuiDocument,
        snapshot: &ProjectUiStateSnapshot,
        paths: &[String],
        themes: &crate::aui_theme::AuiThemeRegistry,
        retained_content_changed: bool,
    ) -> bool {
        if (paths.is_empty() && !retained_content_changed)
            || !prepared.binding_report.ok()
            || prepared.binding_report.fallback_count != 0
            || source.nodes.len() != prepared.document.nodes.len()
        {
            return false;
        }
        let mut slots = Vec::new();
        let mut style_slots = HashSet::new();
        for path in paths {
            let Some(value) = snapshot.values.get(path) else {
                return false;
            };
            let Some(bound_slots) = prepared.binding_slots.get(path) else {
                return false;
            };
            for &(slot, target) in bound_slots {
                if !local_binding_target_valid(&source.nodes[slot], target, value) {
                    return false;
                }
                if matches!(
                    target,
                    AuiBindingTarget::TextColor
                        | AuiBindingTarget::Interactable
                        | AuiBindingTarget::LocalStyle
                ) {
                    style_slots.insert(slot);
                }
                slots.push(slot);
            }
        }
        // Binding Replace and controller content edits can arrive in the same frame.
        // Reapply both sources to their union, retaining binding order and ownership.
        if retained_content_changed {
            slots.extend(
                self.retained_changed_nodes
                    .iter()
                    .filter_map(|id| prepared.node_slots.get(id).copied()),
            );
        }
        slots.sort_unstable();
        slots.dedup();
        let uses_c2 = source.schema_version == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION;
        let mut updated = Vec::with_capacity(slots.len());
        for &slot in &slots {
            self.prepared_binding_nodes_rebuilt += 1;
            let mut node = source.nodes[slot].clone();
            for binding in &node.binding_refs.clone() {
                let Some(value) = snapshot.values.get(&binding.path) else {
                    return false;
                };
                if validate_binding_resource(
                    &node,
                    binding,
                    value,
                    &source.resource_identities,
                    &source.graphics_assets,
                )
                .and_then(|()| {
                    apply_binding_value_for_schema(&mut node, binding, value.clone(), uses_c2)
                })
                .is_err()
                {
                    return false;
                }
            }
            for ((_, property), value) in self.retained_properties.range(
                (
                    node.node_id.clone(),
                    project_runtime_sdk::aui::UiProperty::Text,
                )
                    ..=(
                        node.node_id.clone(),
                        project_runtime_sdk::aui::UiProperty::LocalStyle,
                    ),
            ) {
                apply_retained_value(&mut node, *property, value, uses_c2);
                if *property == project_runtime_sdk::aui::UiProperty::LocalStyle {
                    style_slots.insert(slot);
                }
            }
            let style_changed = style_slots.contains(&slot)
                || prepared.document.nodes[slot].style_overrides != node.style_overrides;
            if style_changed && uses_c2 {
                let Some(style) = &prepared.document.nodes[slot].resolved_style else {
                    return false;
                };
                if themes
                    .resolve_prepared_node(&style.theme_id, &mut node)
                    .is_err()
                {
                    return false;
                }
            }
            let old = &prepared.document.nodes[slot];
            // Source nodes do not contain resolved theme products. A content-only update
            // must preserve them rather than replacing cached paint with None.
            if !style_changed && node.resolved_style.is_none() {
                node.resolved_style = old.resolved_style.clone();
                node.style = old.style.clone();
            }
            if old.rect != node.rect
                || old.text != node.text
                || old.visible != node.visible
                || old.layout_visibility != node.layout_visibility
                || old.interactable != node.interactable
                || old.image != node.image
                || old.progress_value != node.progress_value
                || old.toggle_checked != node.toggle_checked
                || old.slider_value != node.slider_value
                || old.presentation != node.presentation
                || old.custom_runtime != node.custom_runtime
                || old.style_overrides != node.style_overrides
                || old.style != node.style
                || old.resolved_style != node.resolved_style
                || (style_changed
                    && (old.style != node.style || old.resolved_style != node.resolved_style))
            {
                updated.push((slot, node, style_changed));
            }
        }
        let source_was_prepared = self
            .source
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &prepared.document));
        if source_was_prepared {
            self.source = None;
        }
        self.prepared_document_nodes_copied = if Arc::strong_count(&prepared.document) > 1 {
            prepared.document.nodes.len()
        } else {
            0
        };
        let document = Arc::make_mut(&mut prepared.document);
        let mut changed_slots = Vec::with_capacity(updated.len());
        for (slot, node, _style_changed) in updated {
            let target = &mut document.nodes[slot];
            target.rect = node.rect;
            target.text = node.text;
            target.visible = node.visible;
            target.layout_visibility = node.layout_visibility;
            target.interactable = node.interactable;
            target.image = node.image;
            target.progress_value = node.progress_value;
            target.toggle_checked = node.toggle_checked;
            target.slider_value = node.slider_value;
            target.presentation = node.presentation;
            target.custom_runtime = node.custom_runtime;
            target.style_overrides = node.style_overrides;
            target.style = node.style;
            target.resolved_style = node.resolved_style;
            changed_slots.push(slot);
        }
        self.source_change_hint = Some(changed_slots);
        if source_was_prepared {
            self.source = Some(Arc::clone(&prepared.document));
        }
        true
    }

    pub(crate) fn can_apply_changed_bindings(
        &mut self,
        source: &AuiDocument,
        snapshot: &ProjectUiStateSnapshot,
        paths: &[String],
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> bool {
        let Some(prepared) = &self.prepared_input else {
            return false;
        };
        if paths.is_empty()
            || !prepared.binding_report.ok()
            || prepared.binding_report.fallback_count != 0
            || source.nodes.len() != prepared.document.nodes.len()
        {
            return false;
        }
        let valid = paths.iter().all(|path| {
            let Some(value) = snapshot.values.get(path) else {
                return false;
            };
            let Some(slots) = prepared.binding_slots.get(path) else {
                return false;
            };
            slots.iter().all(|&(slot, target)| {
                slot < source.nodes.len()
                    // A root change can eliminate every draw item and fail presentation.
                    // Preserve the successful output through the ordinary preparation path.
                    && !source.canvases.iter().any(|canvas| canvas.root_node == source.nodes[slot].node_id)
                    && local_binding_target_valid(&source.nodes[slot], target, value)
                    && (!matches!(
                        target,
                        AuiBindingTarget::TextColor | AuiBindingTarget::Interactable
                    ) || source.schema_version
                        != crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
                        || prepared.document.nodes[slot].resolved_style.is_some())
            })
        });
        if !valid {
            return false;
        }
        // Validate text before the scheduler releases the successful output's Arc. Resource
        // failures must take the ordinary path, which retains that output for rollback.
        let document = Arc::clone(&prepared.document);
        let node_slots = Arc::clone(&prepared.node_slots);
        let binding_slots = Arc::clone(&prepared.binding_slots);
        for path in paths {
            let value = &snapshot.values[path];
            for &(slot, target) in &binding_slots[path] {
                let node = &document.nodes[slot];
                let text = match (target, value) {
                    (
                        AuiBindingTarget::TextText | AuiBindingTarget::InputFieldText,
                        AuiBindingValue::String(text),
                    ) => Some(text.as_str()),
                    _ => None,
                };
                if !self.local_paragraph_is_safe(node, text, font_bundles) {
                    return false;
                }
                if matches!(
                    target,
                    AuiBindingTarget::PanelVisible
                        | AuiBindingTarget::ImageVisible
                        | AuiBindingTarget::RectOffsetX
                        | AuiBindingTarget::RectOffsetY
                ) && !self.local_subtree_paragraphs_are_safe(
                    &document,
                    &node_slots,
                    slot,
                    font_bundles,
                ) {
                    return false;
                }
            }
        }
        true
    }

    fn local_paragraph_is_safe(
        &mut self,
        node: &AuiNode,
        text: Option<&str>,
        fonts: &RuntimeFontBundleRegistry,
    ) -> bool {
        if self.measurement_enabled {
            self.work.paragraph_preflight_nodes_visited += 1;
        }
        // Legacy UI uses its existing glyph path, not C2 paragraph measurements.
        if self.prepared_input.as_ref().is_some_and(|prepared| {
            prepared.document.schema_version != crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
        }) {
            return true;
        }
        if !matches!(
            node.kind,
            AuiNodeKind::Text | AuiNodeKind::InputField | AuiNodeKind::Button
        ) || (text.is_none() && node.text.is_none() && node.placeholder.is_none())
            || (text.is_none() && self.finalized_paragraphs.contains(&node.node_id))
        {
            return true;
        }
        let computed = self.published_layout.as_ref().and_then(|layout| {
            self.prepared_input
                .as_ref()
                .and_then(|input| input.node_slots.get(&node.node_id))
                .and_then(|&slot| layout.computed_nodes.get(slot))
        });
        let Some(computed) = computed else {
            return false;
        };
        if computed.node_id != node.node_id {
            return false;
        }
        crate::aui_paragraph::layout_paragraph(
            &mut self.paragraph_cache,
            crate::aui_paragraph::ParagraphRequest {
                text: text
                    .or(node.text.as_deref())
                    .or(node.placeholder.as_deref())
                    .unwrap_or(""),
                font_size: node
                    .style
                    .as_ref()
                    .and_then(|style| style.font_size)
                    .unwrap_or(12.0),
                font: node.style.as_ref().and_then(|style| style.font.as_ref()),
                spans: &node.text_spans,
                text_layout: node.text_layout.as_ref().unwrap_or(&Default::default()),
                constraint_width: computed.rect.width,
                box_height: Some(computed.rect.height),
                font_bundles: fonts,
            },
        )
        .is_ok()
    }

    fn local_subtree_paragraphs_are_safe(
        &mut self,
        document: &AuiDocument,
        node_slots: &HashMap<String, usize>,
        slot: usize,
        fonts: &RuntimeFontBundleRegistry,
    ) -> bool {
        let mut pending = vec![slot];
        while let Some(slot) = pending.pop() {
            let node = &document.nodes[slot];
            if !self.local_paragraph_is_safe(node, None, fonts) {
                return false;
            }
            for id in &node.children {
                let Some(&child) = node_slots.get(id) else {
                    return false;
                };
                pending.push(child);
            }
        }
        true
    }

    pub(crate) fn local_retained_edits_are_safe(
        &mut self,
        fonts: &RuntimeFontBundleRegistry,
    ) -> bool {
        let Some(prepared) = &self.prepared_input else {
            return false;
        };
        let document = Arc::clone(&prepared.document);
        let node_slots = Arc::clone(&prepared.node_slots);
        let slots: Vec<_> = self
            .retained_changed_nodes
            .iter()
            .filter_map(|id| node_slots.get(id).copied())
            .collect();
        for slot in slots {
            let node = &document.nodes[slot];
            if document
                .canvases
                .iter()
                .any(|canvas| canvas.root_node == node.node_id)
            {
                return false;
            }
            let text = match self.retained_properties.get(&(
                node.node_id.clone(),
                project_runtime_sdk::aui::UiProperty::Text,
            )) {
                Some(project_runtime_sdk::aui::UiValue::Text(text))
                    if node.text.as_ref() != Some(text) =>
                {
                    Some(text.clone())
                }
                _ => None,
            };
            if !self.local_paragraph_is_safe(node, text.as_deref(), fonts)
                || !self.local_subtree_paragraphs_are_safe(&document, &node_slots, slot, fonts)
            {
                return false;
            }
        }
        true
    }

    pub fn layout_stats(&self, canvas: &str) -> Option<crate::aui_layout_runtime::AuiLayoutStats> {
        self.solvers.get(canvas).map(|solver| {
            if self.geometry_stats.base_reused {
                crate::aui_layout_runtime::AuiLayoutStats {
                    clean_snapshot_reused: true,
                    ..Default::default()
                }
            } else {
                solver.last_stats()
            }
        })
    }

    pub fn geometry_update_stats(&self) -> AuiGeometryUpdateStats {
        self.geometry_stats
    }

    pub fn paragraph_cache_stats(&self) -> crate::aui_paragraph::AuiParagraphCacheStats {
        self.paragraph_cache.stats()
    }

    pub fn paragraph_cache_resident_bytes(&self) -> usize {
        self.paragraph_cache.resident_bytes()
    }

    pub fn custom_measure_inputs(
        &self,
    ) -> &HashMap<String, project_runtime_sdk::aui_extension::UiExtensionConstraints> {
        &self.custom_measure_inputs
    }

    pub fn paragraph_cache_len(&self) -> usize {
        self.paragraph_cache.len()
    }

    pub fn layout(
        &mut self,
        document: &AuiDocument,
        frame: u64,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> (
        AuiLayoutResult,
        HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>,
    ) {
        let (layout, paragraphs) =
            self.layout_shared(Arc::new(document.clone()), frame, font_bundles);
        (
            Arc::unwrap_or_clone(layout),
            Arc::unwrap_or_clone(paragraphs),
        )
    }

    fn layout_shared(
        &mut self,
        document: Arc<AuiDocument>,
        frame: u64,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> (
        Arc<AuiLayoutResult>,
        Arc<HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>>,
    ) {
        let result = self.layout_shared_inner(document, frame, font_bundles);
        if self.measurement_enabled {
            self.work.layout_source_nodes_copied += self.layout_source_nodes_copied as u64;
            self.work.geometry_nodes_updated += self.geometry_stats.nodes_updated as u64;
            self.work.geometry_source_nodes_compared +=
                self.geometry_stats.source_nodes_compared as u64;
            self.work.geometry_snapshot_nodes_copied +=
                self.geometry_stats.snapshot_nodes_copied as u64;
            self.work.translation_entries_compared +=
                self.geometry_stats.translation_entries_compared as u64;
        }
        result
    }

    fn layout_shared_inner(
        &mut self,
        document: Arc<AuiDocument>,
        frame: u64,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> (
        Arc<AuiLayoutResult>,
        Arc<HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>>,
    ) {
        let layout_started = self
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        self.published_layout = None;
        self.layout_source_nodes_copied = 0;
        let hinted_slots = self.source_change_hint.take().filter(|slots| {
            self.source.as_ref().is_some_and(|old| {
                old.nodes.len() == document.nodes.len()
                    && slots.iter().all(|&slot| slot < document.nodes.len())
            })
        });
        let same_source = self
            .source
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &document));
        let (changed, source_nodes_compared) = if let Some(slots) = &hinted_slots {
            (!slots.is_empty(), slots.len())
        } else if same_source {
            (false, 0)
        } else if let Some(old) = &self.source {
            let (equal, compared) = aui_documents_equal_counted(old, &document);
            (!equal, compared)
        } else {
            (true, 0)
        };
        let fonts_changed = !self.layout_fonts.as_ref().is_some_and(|old| {
            old.default_ui_font_bundle_id == font_bundles.default_ui_font_bundle_id
                && Arc::ptr_eq(&old.bundles_by_id, &font_bundles.bundles_by_id)
        });
        let retained_layout_is_compatible = !changed
            || hinted_slots.is_some()
            || self
                .source
                .as_ref()
                .is_some_and(|old| retained_layout_compatible(old, &document));
        let retained_final_compatible = changed
            && !fonts_changed
            && self.geometry_cache.is_some()
            && retained_layout_is_compatible;
        let changed_document_nodes = if retained_final_compatible {
            if let Some(slots) = &hinted_slots {
                // Local COW preparation may have released and mutated the old source in place.
                // Its accepted slot journal is authoritative; comparing that source to itself
                // would lose visibility, paint-only and fixed-text changes.
                slots
                    .iter()
                    .map(|&slot| document.nodes[slot].node_id.clone())
                    .collect::<HashSet<_>>()
            } else {
                self.source
                    .as_ref()
                    .into_iter()
                    .flat_map(|old| old.nodes.iter().zip(&document.nodes))
                    .filter(|(old, new)| retained_final_inputs_changed(old, new))
                    .map(|(_, node)| node.node_id.clone())
                    .collect::<HashSet<_>>()
            }
        } else {
            HashSet::new()
        };
        self.geometry_stats = AuiGeometryUpdateStats {
            source_nodes_compared,
            ..Default::default()
        };
        if !changed && !fonts_changed && self.geometry_cache.is_some() {
            self.timings.measure_arrange_ns += aui_elapsed_ns(layout_started);
            let transform_started = self
                .measurement_enabled
                .then(crate::platform_time::Instant::now);
            let result = self.update_translation_geometry(&document, frame, font_bundles);
            self.timings.transform_clip_finalize_ns += aui_elapsed_ns(transform_started);
            return result;
        }
        if !retained_final_compatible {
            self.geometry_cache = None;
            self.cached_paragraphs = Arc::new(HashMap::new());
            self.finalized_paragraphs.clear();
        }
        if fonts_changed {
            // Intrinsic measurements belong to the actual font resource, not only document style.
            for solver in self.solvers.values_mut() {
                solver.invalidate_instance_measurements();
            }
            self.paragraph_cache = crate::aui_paragraph::AuiParagraphCache::default();
        }
        self.layout_fonts = Some(font_bundles.clone());
        let mut diagnostics = if changed && hinted_slots.is_none() {
            crate::aui_layout_schema::validate(&document)
        } else {
            Vec::new()
        };
        let slots_changed = self.layout_source.is_none()
            || (hinted_slots.is_none()
                && self.source.as_ref().is_none_or(|old| {
                    old.nodes.len() != document.nodes.len()
                        || old
                            .nodes
                            .iter()
                            .zip(&document.nodes)
                            .any(|(a, b)| a.node_id != b.node_id)
                }));
        if diagnostics.is_empty() && slots_changed {
            self.layout_source = Some(crate::aui_layout_runtime::AuiLayoutSource::new(&document));
            self.layout_source_nodes_copied = document.nodes.len();
        }
        if changed && diagnostics.is_empty() {
            let mut failed_canvases = HashSet::new();
            let source = self.layout_source.as_ref().expect("source indexed");
            if hinted_slots.is_none() {
                self.solvers.retain(|_, solver| {
                    solver.reconcile_source_slots(&document, &source.node_slots)
                });
            }
            let slots = hinted_slots.clone().unwrap_or_else(|| {
                if slots_changed {
                    (0..document.nodes.len()).collect()
                } else {
                    self.source.as_ref().map_or_else(
                        || (0..document.nodes.len()).collect(),
                        |old| {
                            old.nodes
                                .iter()
                                .zip(&document.nodes)
                                .enumerate()
                                .filter_map(|(slot, (a, b))| (a != b).then_some(slot))
                                .collect()
                        },
                    )
                }
            });
            for slot in slots {
                let node = if self.layout_source_nodes_copied > 0 {
                    source.node(slot)
                } else {
                    Arc::new(document.nodes[slot].clone())
                };
                for solver in self.solvers.values_mut() {
                    solver.update_instance_source_node(slot, Arc::clone(&node));
                }
            }
            for canvas in &document.canvases {
                if let Some(solver) = self.solvers.get_mut(&canvas.canvas_id) {
                    solver.update_instance_root_rect(canvas.resolved_layout_root_rect());
                }
            }
            for (canvas, solver) in &mut self.solvers {
                if let Err(items) = solver.configure_instance_inputs() {
                    failed_canvases.insert(canvas.clone());
                    diagnostics.extend(items);
                }
            }
            if !failed_canvases.is_empty() {
                self.solvers
                    .retain(|canvas, _| !failed_canvases.contains(canvas));
                // Configuring a subtree can fail after changing its constraints. Drop that
                // solver and force synchronization on retry, even when input reverts to the
                // last successful Document. Independent roots retain their algorithm caches.
                self.source = None;
                self.geometry_cache = None;
            }
        }
        let mut successful_canvas_slots = Vec::new();
        let mut paragraph_layouts = HashMap::new();
        let mut custom_measure_inputs = HashMap::new();
        let mut changed_geometry_nodes = HashSet::new();
        if !diagnostics.is_empty() {
            let layout = Arc::new(AuiLayoutResult {
                diagnostics,
                report: AuiLayoutReport {
                    frame,
                    ..Default::default()
                },
                ..Default::default()
            });
            self.published_layout = Some(layout.clone());
            self.timings.measure_arrange_ns += aui_elapsed_ns(layout_started);
            return (layout, Arc::new(paragraph_layouts));
        }

        for (canvas_slot, canvas) in document.canvases.iter().enumerate() {
            if !self.solvers.contains_key(&canvas.canvas_id) {
                // Late/rebuilt Canvas roots must see current properties, not the first-frame
                // source. Refresh once, then share that source across all missing roots.
                if self.layout_source_nodes_copied == 0 {
                    self.layout_source =
                        Some(crate::aui_layout_runtime::AuiLayoutSource::new(&document));
                    self.layout_source_nodes_copied = document.nodes.len();
                    for solver in self.solvers.values_mut() {
                        solver.rebase_source(self.layout_source.as_ref().unwrap());
                    }
                }
                match crate::aui_layout_runtime::AuiLayoutSolver::for_shared_instance(
                    &document,
                    &canvas.canvas_id,
                    self.layout_source.as_ref().expect("shared mounted source"),
                ) {
                    Ok(solver) => {
                        self.solver_build_count = self.solver_build_count.saturating_add(1);
                        self.solvers.insert(canvas.canvas_id.clone(), solver);
                    }
                    Err(items) => {
                        diagnostics.extend(items);
                        continue;
                    }
                }
            }
            let solver = self
                .solvers
                .get_mut(&canvas.canvas_id)
                .expect("mounted canvas");
            let mut paragraphs = crate::aui_paragraph::AuiParagraphContext::with_node_slots(
                &document,
                &self
                    .layout_source
                    .as_ref()
                    .expect("mounted index")
                    .node_slots,
                font_bundles,
                &mut self.paragraph_cache,
            );
            match solver.solve_instance(&mut paragraphs) {
                Ok(_) => successful_canvas_slots.push(canvas_slot),
                Err(items) => diagnostics.extend(items),
            }
            if self.measurement_enabled {
                let stats = solver.last_stats();
                self.work.layout_nodes_visited += stats.visited_nodes as u64;
                self.work.layout_measure_calls += stats.measure_calls as u64;
                self.work.layout_source_nodes_loaded += stats.source_nodes_loaded as u64;
                self.work.layout_geometry_nodes += stats.geometry_nodes as u64;
            }
            custom_measure_inputs.extend(
                paragraphs
                    .custom_measure_inputs()
                    .iter()
                    .map(|(node_id, constraints)| (node_id.clone(), *constraints)),
            );
            changed_geometry_nodes.extend(solver.last_geometry_node_ids().map(str::to_owned));
            for (node_id, layout) in paragraphs.frame_layouts() {
                paragraph_layouts.insert(node_id.clone(), Arc::clone(layout));
            }
            for (node_id, error) in paragraphs.errors() {
                diagnostics.push(AuiValidationItem::error(
                    "aui_paragraph_layout_failed",
                    Some(node_id.clone()),
                    None,
                    format!("Paragraph layout failed: {error:?}"),
                    "Provide a valid RuntimeFontBundle with source font bytes.",
                ));
            }
            if !paragraphs.errors().is_empty() {
                // Keep partial geometry available to layout-only callers, but never retain
                // failed font measurements as a successful clean snapshot.
                self.solvers.remove(&canvas.canvas_id);
            }
        }

        self.custom_measure_inputs = custom_measure_inputs;

        let snapshots = successful_canvas_slots
            .iter()
            .filter_map(|&canvas_slot| {
                self.solvers
                    .get(&document.canvases[canvas_slot].canvas_id)
                    .and_then(crate::aui_layout_runtime::AuiLayoutSolver::instance_snapshot)
            })
            .collect::<Vec<_>>();

        self.timings.measure_arrange_ns += aui_elapsed_ns(layout_started);
        let transform_started = self
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        if retained_final_compatible && diagnostics.is_empty() {
            changed_geometry_nodes.extend(changed_document_nodes);
            let empty = HashMap::new();
            let translations =
                if self.retained_document_id.as_deref() == Some(document.document_id.as_str()) {
                    &self.translations
                } else {
                    &empty
                };
            let (affected_nodes, affected_slots) = {
                let geometry = self
                    .geometry_cache
                    .as_mut()
                    .expect("compatible final geometry");
                geometry.refresh_base(&document, &snapshots, &changed_geometry_nodes, frame);
                let affected = geometry.update_presentations(
                    translations,
                    &self.presentations,
                    self.retained_translation_changes_known
                        .then_some(&self.retained_translation_changes),
                    frame,
                );
                let nodes = affected
                    .iter()
                    .map(|&slot| geometry.result.computed_nodes[slot].clone())
                    .collect::<Vec<_>>();
                (nodes, affected)
            };
            let affected_ids = affected_nodes
                .iter()
                .map(|node| node.node_id.clone())
                .collect::<HashSet<_>>();
            let cached_paragraphs = Arc::make_mut(&mut self.cached_paragraphs);
            for node_id in &affected_ids {
                cached_paragraphs.remove(node_id);
                self.finalized_paragraphs.remove(node_id);
            }
            let mut paragraphs_updated = 0;
            let mut paragraphs = crate::aui_paragraph::AuiParagraphContext::with_node_slots(
                &document,
                &self
                    .layout_source
                    .as_ref()
                    .expect("mounted index")
                    .node_slots,
                font_bundles,
                &mut self.paragraph_cache,
            );
            for computed in affected_nodes.iter().filter(|computed| {
                computed.effective_visible
                    && matches!(
                        computed.kind,
                        AuiNodeKind::Text | AuiNodeKind::InputField | AuiNodeKind::Button
                    )
            }) {
                if paragraphs.has_text(&computed.node_id) {
                    if let Some(layout) = paragraphs.layout_node(
                        &computed.node_id,
                        computed.rect.width,
                        Some(computed.rect.height),
                    ) {
                        cached_paragraphs.insert(computed.node_id.clone(), layout);
                    }
                }
                self.finalized_paragraphs.insert(computed.node_id.clone());
                paragraphs_updated += 1;
            }
            let paragraph_diagnostics = paragraphs
                .errors()
                .iter()
                .map(|(node_id, error)| {
                    AuiValidationItem::error(
                        "aui_paragraph_layout_failed",
                        Some(node_id.clone()),
                        None,
                        format!("Paragraph layout failed: {error:?}"),
                        "Provide a valid RuntimeFontBundle with source font bytes.",
                    )
                })
                .collect::<Vec<_>>();
            self.geometry_stats = AuiGeometryUpdateStats {
                final_base_reused: true,
                nodes_updated: affected_nodes.len(),
                paragraphs_updated,
                source_nodes_compared,
                translation_entries_compared: self
                    .geometry_cache
                    .as_ref()
                    .unwrap()
                    .translation_entries_compared,
                ..Default::default()
            };
            let update = self.interaction_index_journal.record(
                self.geometry_cache
                    .as_ref()
                    .expect("compatible final geometry")
                    .result
                    .computed_nodes
                    .len(),
                &affected_slots,
            );
            self.geometry_cache
                .as_mut()
                .expect("compatible final geometry")
                .set_interaction_index_update(update);
            let mut result = self
                .geometry_cache
                .as_ref()
                .expect("compatible final geometry")
                .result
                .clone();
            if !paragraph_diagnostics.is_empty() {
                Arc::make_mut(&mut result)
                    .diagnostics
                    .extend(paragraph_diagnostics);
            }
            if !result.diagnostics.is_empty() {
                self.geometry_cache = None;
            } else {
                self.source = Some(Arc::clone(&document));
            }
            self.published_layout = Some(result.clone());
            self.retained_translation_changes.clear();
            self.retained_translation_changes_known = true;
            self.timings.transform_clip_finalize_ns += aui_elapsed_ns(transform_started);
            return (result, Arc::clone(&self.cached_paragraphs));
        }

        let base = c2_layout_result(&document, frame, &snapshots, diagnostics);
        self.geometry_stats.nodes_updated = base.computed_nodes.len();
        let mut geometry =
            crate::aui_geometry_cache::AuiGeometryCache::new(&document, &snapshots, base);
        let empty = HashMap::new();
        geometry.update_presentations(
            if self.retained_document_id.as_deref() == Some(document.document_id.as_str()) {
                &self.translations
            } else {
                &empty
            },
            &self.presentations,
            None,
            frame,
        );
        self.geometry_stats.translation_entries_compared = geometry.translation_entries_compared;
        self.retained_translation_changes.clear();
        self.retained_translation_changes_known = true;
        let update = self
            .interaction_index_journal
            .reset(geometry.result.computed_nodes.len());
        geometry.set_interaction_index_update(update);
        let mut result = geometry.result.clone();
        if result.diagnostics.is_empty() {
            let mut paragraphs = crate::aui_paragraph::AuiParagraphContext::with_node_slots(
                &document,
                &self
                    .layout_source
                    .as_ref()
                    .expect("mounted index")
                    .node_slots,
                font_bundles,
                &mut self.paragraph_cache,
            );
            for computed in result.computed_nodes.iter().filter(|computed| {
                computed.effective_visible
                    && matches!(
                        computed.kind,
                        AuiNodeKind::Text | AuiNodeKind::InputField | AuiNodeKind::Button
                    )
            }) {
                if paragraphs.has_text(&computed.node_id) {
                    if let Some(layout) = paragraphs.layout_node(
                        &computed.node_id,
                        computed.rect.width,
                        Some(computed.rect.height),
                    ) {
                        paragraph_layouts.insert(computed.node_id.clone(), layout);
                    }
                }
                // A control with no text also has a complete (empty) final paragraph state.
                self.finalized_paragraphs.insert(computed.node_id.clone());
            }
            for (node_id, error) in paragraphs.errors() {
                Arc::make_mut(&mut result)
                    .diagnostics
                    .push(AuiValidationItem::error(
                        "aui_paragraph_layout_failed",
                        Some(node_id.clone()),
                        None,
                        format!("Paragraph layout failed: {error:?}"),
                        "Provide a valid RuntimeFontBundle with source font bytes.",
                    ));
            }
        }
        if result.diagnostics.is_empty() {
            self.cached_paragraphs = Arc::new(paragraph_layouts);
            self.geometry_cache = Some(geometry);
            self.source = Some(Arc::clone(&document));
        }
        self.published_layout = Some(result.clone());
        self.timings.transform_clip_finalize_ns += aui_elapsed_ns(transform_started);
        (result, Arc::clone(&self.cached_paragraphs))
    }

    // Classic rect documents have no C2 solver, but their post-layout animation
    // must retain the same geometry owner and dirty-slot contract as C2.
    fn layout_legacy_shared(
        &mut self,
        document: Arc<AuiDocument>,
        frame: u64,
    ) -> Arc<AuiLayoutResult> {
        self.published_layout = None;
        let hint = self.source_change_hint.take();
        // Preparation can mutate a uniquely owned source in place. Pointer identity
        // alone is not proof of unchanged layout inputs when content slots changed.
        let reuse = hint.as_ref().is_none_or(Vec::is_empty)
            && self
                .source
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(old, &document))
            && self.geometry_cache.is_some();
        let layout_started = self
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        if !reuse {
            let mut base =
                AuiLayoutEngine::layout_raw(&document, frame, &BTreeMap::new(), &BTreeMap::new());
            if self.measurement_enabled {
                self.work.layout_nodes_visited += base.computed_nodes.len() as u64;
            }
            // Classic scrolling establishes implicit clip roots, unlike the C2
            // geometry contract. Preserve its existing path and clip semantics.
            if document
                .nodes
                .iter()
                .any(|node| matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List))
            {
                self.timings.measure_arrange_ns += aui_elapsed_ns(layout_started);
                let transform_started = self
                    .measurement_enabled
                    .then(crate::platform_time::Instant::now);
                apply_retained_translations(
                    &document,
                    &mut base,
                    &self.translations,
                    &self.presentations,
                );
                self.geometry_stats = AuiGeometryUpdateStats {
                    nodes_updated: base.computed_nodes.len(),
                    ..Default::default()
                };
                self.geometry_cache = None;
                self.source = Some(document);
                let result = Arc::new(base);
                self.published_layout = Some(result.clone());
                self.timings.transform_clip_finalize_ns += aui_elapsed_ns(transform_started);
                return result;
            }
            self.geometry_cache = Some(crate::aui_geometry_cache::AuiGeometryCache::new(
                &document,
                &[],
                base,
            ));
        }
        self.timings.measure_arrange_ns += aui_elapsed_ns(layout_started);
        let transform_started = self
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        let geometry = self.geometry_cache.as_mut().expect("classic base geometry");
        let empty = HashMap::new();
        let affected = geometry.update_presentations(
            if self.retained_document_id.as_deref() == Some(document.document_id.as_str()) {
                &self.translations
            } else {
                &empty
            },
            &self.presentations,
            (reuse && self.retained_translation_changes_known)
                .then_some(&self.retained_translation_changes),
            frame,
        );
        self.geometry_stats = AuiGeometryUpdateStats {
            base_reused: reuse,
            nodes_updated: affected.len(),
            translation_entries_compared: geometry.translation_entries_compared,
            ..Default::default()
        };
        let count = geometry.result.computed_nodes.len();
        let update = if reuse {
            self.interaction_index_journal.record(count, &affected)
        } else {
            self.interaction_index_journal.reset(count)
        };
        geometry.set_interaction_index_update(update);
        let result = geometry.result.clone();
        if reuse {
            // This owner has proved the exact change set, including an empty one.
            // Public extraction still uses the conservative fallback for edited layouts.
            self.drawing_cache.draw_changes_once = Some(affected);
        }
        self.retained_translation_changes.clear();
        self.retained_translation_changes_known = true;
        self.source = Some(document);
        if !result.diagnostics.is_empty() {
            self.geometry_cache = None;
        }
        self.published_layout = Some(result.clone());
        if self.measurement_enabled {
            self.work.geometry_nodes_updated += self.geometry_stats.nodes_updated as u64;
            self.work.translation_entries_compared +=
                self.geometry_stats.translation_entries_compared as u64;
        }
        self.timings.transform_clip_finalize_ns += aui_elapsed_ns(transform_started);
        result
    }

    fn update_translation_geometry(
        &mut self,
        document: &AuiDocument,
        frame: u64,
        font_bundles: &RuntimeFontBundleRegistry,
    ) -> (
        Arc<AuiLayoutResult>,
        Arc<HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>>,
    ) {
        let source_nodes_compared = self.geometry_stats.source_nodes_compared;
        let cache = self
            .geometry_cache
            .as_mut()
            .expect("successful base geometry");
        let empty = HashMap::new();
        let affected = cache.update_presentations(
            if self.retained_document_id.as_deref() == Some(document.document_id.as_str()) {
                &self.translations
            } else {
                &empty
            },
            &self.presentations,
            self.retained_translation_changes_known
                .then_some(&self.retained_translation_changes),
            frame,
        );
        self.geometry_stats = AuiGeometryUpdateStats {
            base_reused: true,
            nodes_updated: affected.len(),
            source_nodes_compared,
            translation_entries_compared: cache.translation_entries_compared,
            ..Default::default()
        };
        // A translated, previously clipped node can become visible for the first time.
        // Existing final paragraphs keep their width/height constraints and shared identity.
        let missing: Vec<_> = affected
            .iter()
            .filter_map(|&slot| {
                let node = &cache.result.computed_nodes[slot];
                (node.effective_visible
                    && matches!(
                        node.kind,
                        AuiNodeKind::Text | AuiNodeKind::InputField | AuiNodeKind::Button
                    )
                    && !self.finalized_paragraphs.contains(&node.node_id))
                .then_some(node)
            })
            .collect();
        let mut diagnostics = Vec::new();
        if !missing.is_empty() {
            let cached_paragraphs = Arc::make_mut(&mut self.cached_paragraphs);
            let mut paragraphs = crate::aui_paragraph::AuiParagraphContext::with_node_slots(
                document,
                &self
                    .layout_source
                    .as_ref()
                    .expect("mounted index")
                    .node_slots,
                font_bundles,
                &mut self.paragraph_cache,
            );
            for node in missing {
                if paragraphs.has_text(&node.node_id) {
                    if let Some(layout) = paragraphs.layout_node(
                        &node.node_id,
                        node.rect.width,
                        Some(node.rect.height),
                    ) {
                        cached_paragraphs.insert(node.node_id.clone(), layout);
                    }
                }
                self.finalized_paragraphs.insert(node.node_id.clone());
            }
            for (node_id, error) in paragraphs.errors() {
                diagnostics.push(AuiValidationItem::error(
                    "aui_paragraph_layout_failed",
                    Some(node_id.clone()),
                    None,
                    format!("Paragraph layout failed: {error:?}"),
                    "Provide a valid RuntimeFontBundle with source font bytes.",
                ));
            }
        }
        let update = self
            .interaction_index_journal
            .record(cache.result.computed_nodes.len(), &affected);
        cache.set_interaction_index_update(update);
        let mut result = cache.result.clone();
        if !diagnostics.is_empty() {
            Arc::make_mut(&mut result).diagnostics.extend(diagnostics);
        }
        if !result.diagnostics.is_empty() {
            self.geometry_cache = None;
        }
        self.published_layout = Some(result.clone());
        self.retained_translation_changes.clear();
        self.retained_translation_changes_known = true;
        (result, Arc::clone(&self.cached_paragraphs))
    }
}

fn apply_retained_value(
    node: &mut AuiNode,
    property: project_runtime_sdk::aui::UiProperty,
    value: &project_runtime_sdk::aui::UiValue,
    uses_c2: bool,
) {
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiVisibility};
    match (property, value) {
        (property, value) if property.is_presentation() => node
            .presentation
            .get_or_insert_with(Default::default)
            .apply(property, value),
        (UiProperty::Text, UiValue::Text(text)) => node.text = Some(text.clone()),
        (UiProperty::Interactable, UiValue::Bool(value)) => node.interactable = *value,
        (UiProperty::Visibility, UiValue::Visibility(value)) => {
            node.visible = *value == UiVisibility::Visible;
            if uses_c2 {
                node.layout_visibility = Some(match value {
                    UiVisibility::Visible => crate::aui_layout_schema::LayoutVisibility::Visible,
                    UiVisibility::Hidden => crate::aui_layout_schema::LayoutVisibility::Hidden,
                    UiVisibility::Collapsed => {
                        crate::aui_layout_schema::LayoutVisibility::Collapsed
                    }
                });
            }
        }
        (UiProperty::Active, UiValue::Bool(false)) => {
            node.visible = false;
            if uses_c2 {
                node.layout_visibility =
                    Some(crate::aui_layout_schema::LayoutVisibility::Collapsed);
            }
        }
        (UiProperty::Active, UiValue::Bool(true)) => {}
        (UiProperty::Value, UiValue::Number(value)) => {
            node.slider_value = Some(normalize_slider_value(node, *value));
        }
        (UiProperty::Resource, UiValue::Resource(resource)) => {
            if node.kind == AuiNodeKind::Image {
                node.image = resource
                    .as_ref()
                    .map(|r| AuiAssetRef::new(r.asset_id.clone()));
            } else if node.kind == AuiNodeKind::Custom {
                node.custom_runtime
                    .get_or_insert_with(Default::default)
                    .paint = resource.as_ref().map(|r| {
                    project_runtime_sdk::aui_extension::UiExtensionPaintOutput::from_graphics_asset(
                        r.asset_id.clone(),
                        r.guid.clone(),
                    )
                });
            }
        }
        (UiProperty::LocalStyle, UiValue::LocalStyle(style)) => {
            use crate::aui_theme::{AuiBrushSpec, AuiThemeValue};
            if uses_c2 {
                let overrides = node.style_overrides.get_or_insert_with(Default::default);
                if let Some(color) = style.background {
                    overrides.background = Some(AuiBrushSpec::Solid {
                        color: AuiThemeValue::Value {
                            value: extension_color(color),
                        },
                    });
                }
                if let Some(color) = style.foreground {
                    overrides.foreground = Some(AuiThemeValue::Value {
                        value: extension_color(color),
                    });
                }
                if let Some(color) = style.border {
                    overrides.border.get_or_insert_with(Default::default).color =
                        Some(AuiThemeValue::Value {
                            value: extension_color(color),
                        });
                }
                if let Some(radius) = style.radius {
                    overrides.radius = Some(AuiThemeValue::Value { value: radius });
                }
            }
            let authored = node.style.get_or_insert_with(|| AuiStyle {
                color: None,
                text_color: None,
                font_size: None,
                font: None,
            });
            if let Some(color) = style.background {
                authored.color = Some(extension_color(color));
            }
            if let Some(color) = style.foreground {
                authored.text_color = Some(extension_color(color));
            }
            let fallback_background = authored.color.clone();
            let resolved = node.resolved_style.get_or_insert_with(|| {
                let mut resolved = crate::aui_theme::AuiResolvedStyle::default();
                if let Some(color) = fallback_background {
                    resolved.paint.background = crate::aui_theme::AuiResolvedBrush::solid(color);
                }
                resolved
            });
            {
                if let Some(color) = style.background {
                    resolved.paint.background =
                        crate::aui_theme::AuiResolvedBrush::solid(extension_color(color));
                }
                if let Some(color) = style.foreground {
                    resolved.paint.foreground = Some(extension_color(color));
                }
                if let Some(color) = style.border {
                    resolved.paint.border.color = Some(extension_color(color));
                }
                if let Some(radius) = style.radius {
                    resolved.paint.radius = radius;
                }
            }
        }
        (UiProperty::Translation, UiValue::Translation(_)) => {
            unreachable!("stored separately")
        }
        _ => unreachable!("validated retained property/value pair"),
    }
}

// Legacy documents consume the same final-geometry owner as C2.
fn apply_retained_translations(
    document: &AuiDocument,
    layout: &mut AuiLayoutResult,
    translations: &HashMap<String, [f32; 2]>,
    presentations: &HashMap<
        String,
        BTreeMap<project_runtime_sdk::aui::UiProperty, project_runtime_sdk::aui::UiValue>,
    >,
) {
    if translations.is_empty()
        && presentations.is_empty()
        && document.nodes.iter().all(|n| n.presentation.is_none())
    {
        return;
    }
    let frame = layout.report.frame;
    let mut cache =
        crate::aui_geometry_cache::AuiGeometryCache::new(document, &[], std::mem::take(layout));
    cache.update_presentations(translations, presentations, None, frame);
    *layout = (*cache.result).clone();
}

fn aui_documents_equal_counted(old: &AuiDocument, new: &AuiDocument) -> (bool, usize) {
    if old.schema_version != new.schema_version
        || old.document_id != new.document_id
        || old.theme != new.theme
        || old.interaction_feedback != new.interaction_feedback
        || old.virtual_lists != new.virtual_lists
        || old.canvases != new.canvases
        || old.nodes.len() != new.nodes.len()
    {
        return (false, 0);
    }
    let mut compared = 0;
    for (old_node, new_node) in old.nodes.iter().zip(&new.nodes) {
        compared += 1;
        if old_node != new_node {
            return (false, compared);
        }
    }
    (true, compared)
}

// Final geometry indices remain valid while topology and Canvas composition stay stable.
// Changed constraints and measurement inputs update the mounted solver independently.
fn retained_layout_compatible(old: &AuiDocument, new: &AuiDocument) -> bool {
    old.document_id == new.document_id
        && old.canvases.len() == new.canvases.len()
        && old.canvases.iter().zip(&new.canvases).all(|(a, b)| {
            let mut constraints_updated = a.clone();
            constraints_updated.reference_resolution = b.reference_resolution;
            constraints_updated.resolved_viewport_root_rect = b.resolved_viewport_root_rect;
            &constraints_updated == b
        })
        && old.nodes.len() == new.nodes.len()
        && old.nodes.iter().zip(&new.nodes).all(|(a, b)| {
            a.node_id == b.node_id
                && a.kind == b.kind
                && a.parent == b.parent
                && a.children == b.children
        })
}

fn retained_final_inputs_changed(old: &AuiNode, new: &AuiNode) -> bool {
    old.text != new.text
        || old.presentation != new.presentation
        || old.visible != new.visible
        || old.layout_visibility != new.layout_visibility
        || old.clip_policy != new.clip_policy
        || old.clip_radius != new.clip_radius
        || old.style != new.style
        || old.text_layout != new.text_layout
        || old.text_spans != new.text_spans
        || old.placeholder != new.placeholder
        || old.custom_runtime != new.custom_runtime
        || old.resolved_style.as_ref().map(|style| style.paint.radius)
            != new.resolved_style.as_ref().map(|style| style.paint.radius)
}

fn c2_layout_result(
    document: &AuiDocument,
    frame: u64,
    snapshots: &[&crate::aui_layout_runtime::AuiLayoutSnapshot],
    diagnostics: Vec<AuiValidationItem>,
) -> AuiLayoutResult {
    let nodes_by_id = document
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let canvases_by_id = document
        .canvases
        .iter()
        .map(|canvas| (canvas.canvas_id.as_str(), canvas))
        .collect::<HashMap<_, _>>();
    let mut computed_nodes = Vec::new();
    let mut effective_visibility = HashMap::<String, bool>::new();
    let mut child_clips = HashMap::<String, AuiActiveClip>::new();
    let mut tree_order = 0;
    let overflow_count = snapshots
        .iter()
        .map(|snapshot| snapshot.overflow_node_ids.len())
        .sum();

    for snapshot in snapshots {
        let Some(canvas) = canvases_by_id.get(snapshot.canvas_id.as_str()) else {
            continue;
        };
        for geometry in &snapshot.nodes {
            let Some(node) = nodes_by_id.get(geometry.node_id.as_str()) else {
                continue;
            };
            let parent_visible = node
                .parent
                .as_ref()
                .and_then(|parent| effective_visibility.get(parent))
                .copied()
                .unwrap_or(true);
            let local_visible = node.visible
                && geometry.visibility == crate::aui_layout_schema::LayoutVisibility::Visible;
            let mut visible = parent_visible && local_visible;
            effective_visibility.insert(node.node_id.clone(), visible);
            let active_clip = node
                .parent
                .as_ref()
                .and_then(|parent| child_clips.get(parent))
                .cloned();
            let (effective_clip_rect, clipped_by_node) = match active_clip.as_ref() {
                Some(clip) => match clip.rect.and_then(|rect| rect.intersection(geometry.rect)) {
                    Some(rect) => (Some(rect), Some(clip.node_id.clone())),
                    None => {
                        visible = false;
                        (None, Some(clip.node_id.clone()))
                    }
                },
                None => (None, None),
            };
            computed_nodes.push(AuiComputedNode {
                presentation: None,
                canvas_id: canvas.canvas_id.clone(),
                composition_stage: canvas.composition_stage,
                node_id: node.node_id.clone(),
                kind: node.kind,
                rect: geometry.rect,
                effective_clip_rect,
                effective_clips: active_clip
                    .as_ref()
                    .map(|clip| clip.shapes.clone())
                    .unwrap_or_default(),
                clipped_by_node,
                tree_order,
                local_visible,
                effective_visible: visible,
            });
            tree_order += 1;

            let establishes_clip = matches!(
                node.overflow,
                Some(
                    crate::aui_layout_schema::Overflow::Clip
                        | crate::aui_layout_schema::Overflow::Scroll
                )
            ) || matches!(
                node.clip_policy,
                AuiClipPolicy::Rect | AuiClipPolicy::Rounded
            );
            let child_clip = if establishes_clip {
                let rect = match active_clip.as_ref() {
                    Some(clip) => clip
                        .rect
                        .and_then(|parent| parent.intersection(geometry.rect)),
                    None => Some(geometry.rect),
                };
                let mut shapes = active_clip
                    .as_ref()
                    .map(|clip| clip.shapes.clone())
                    .unwrap_or_default();
                shapes.push(AuiResolvedClip {
                    transform: None,
                    node_id: node.node_id.clone(),
                    rect: geometry.rect,
                    radius: if node.clip_policy == AuiClipPolicy::Rounded {
                        node.clip_radius
                            .or_else(|| {
                                node.resolved_style.as_ref().map(|style| style.paint.radius)
                            })
                            .unwrap_or_default()
                    } else {
                        0.0
                    },
                });
                AuiActiveClip {
                    node_id: node.node_id.clone(),
                    rect,
                    shapes,
                }
            } else {
                active_clip.unwrap_or(AuiActiveClip {
                    node_id: String::new(),
                    rect: None,
                    shapes: Vec::new(),
                })
            };
            if establishes_clip || !child_clip.node_id.is_empty() {
                child_clips.insert(node.node_id.clone(), child_clip);
            }
        }
    }

    let visible_node_count = computed_nodes
        .iter()
        .filter(|node| node.effective_visible)
        .count();
    let clipped_node_count = computed_nodes
        .iter()
        .filter(|node| node.clipped_by_node.is_some() && node.effective_clip_rect.is_none())
        .count();
    let effective_clip_node_count = computed_nodes
        .iter()
        .filter(|node| node.effective_clip_rect.is_some())
        .count();
    AuiLayoutResult {
        diagnostics,
        report: AuiLayoutReport {
            frame,
            canvas_count: snapshots.len(),
            node_count: computed_nodes.len(),
            visible_node_count,
            clipped_node_count,
            clip_root_count: document
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.overflow,
                        Some(
                            crate::aui_layout_schema::Overflow::Clip
                                | crate::aui_layout_schema::Overflow::Scroll
                        )
                    ) || matches!(
                        node.clip_policy,
                        AuiClipPolicy::Rect | AuiClipPolicy::Rounded
                    )
                })
                .count(),
            effective_clip_node_count,
            overflow_count,
            ..Default::default()
        },
        computed_nodes,
        scrollbar_metrics: Vec::new(),
        interaction_index_update: Default::default(),
    }
}

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

    pub fn present_project_snapshot_with_fonts_for_presentation_cached(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
        c2_runtime: &mut AuiC2Runtime,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts_for_presentation_cached(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            presentation,
            c2_runtime,
        )
    }

    pub fn present_project_snapshot_with_fonts_for_presentation_cached_with_themes(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        themes: &crate::aui_theme::AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
        c2_runtime: &mut AuiC2Runtime,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts_for_presentation_cached_and_themes(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            themes,
            presentation,
            c2_runtime,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present_project_snapshot_with_fonts_for_presentation_cached_with_themes_and_viewport(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        themes: &crate::aui_theme::AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
        viewport: &crate::aui_viewport::AuiViewportInput,
        c2_runtime: &mut AuiC2Runtime,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts_for_presentation_cached_and_themes(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            themes,
            presentation,
            c2_runtime,
            Some(viewport),
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
        let mut c2_runtime = AuiC2Runtime::default();
        Self::present_with_snapshot_output_and_fonts_for_presentation_cached(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            presentation,
            &mut c2_runtime,
        )
    }

    fn present_with_snapshot_output_and_fonts_for_presentation_cached(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
        c2_runtime: &mut AuiC2Runtime,
    ) -> AuiRuntimePresentOutput {
        Self::present_with_snapshot_output_and_fonts_for_presentation_cached_and_themes(
            document,
            snapshot_output,
            font_atlases,
            font_bundles,
            &crate::aui_theme::AuiThemeRegistry::builtin(),
            presentation,
            c2_runtime,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn present_with_snapshot_output_and_fonts_for_presentation_cached_and_themes(
        document: &AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        themes: &crate::aui_theme::AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
        c2_runtime: &mut AuiC2Runtime,
        viewport: Option<&crate::aui_viewport::AuiViewportInput>,
    ) -> AuiRuntimePresentOutput {
        let prepare_started = c2_runtime
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        let snapshot_source = snapshot_output.report.snapshot_source;
        let snapshot = snapshot_output.snapshot;
        let reuse = std::mem::take(&mut c2_runtime.reuse_prepared_input_once);
        let changed_binding_paths = std::mem::take(&mut c2_runtime.changed_binding_paths_once);
        let apply_retained_changes =
            std::mem::take(&mut c2_runtime.apply_retained_changes_to_prepared_input_once);
        c2_runtime.prepared_document_nodes_copied = 0;
        c2_runtime.prepared_binding_nodes_rebuilt = 0;
        let local_style_changed = apply_retained_changes
            && c2_runtime.retained_changed_nodes.iter().any(|id| {
                c2_runtime
                    .retained_properties
                    .contains_key(&(id.clone(), project_runtime_sdk::aui::UiProperty::LocalStyle))
                    || c2_runtime.prepared_input.as_ref().is_some_and(|p| {
                        p.node_slots.get(id).is_some_and(|slot| {
                            p.document.nodes[*slot].style_overrides
                                != document.nodes[*slot].style_overrides
                        })
                    })
            });
        let partial_bindings = if !changed_binding_paths.is_empty() || local_style_changed {
            c2_runtime.prepared_input.take().and_then(|mut prepared| {
                c2_runtime
                    .apply_changed_bindings_to_prepared_input(
                        &mut prepared,
                        document,
                        &snapshot,
                        &changed_binding_paths,
                        themes,
                        apply_retained_changes,
                    )
                    .then_some(prepared)
            })
        } else {
            None
        };
        c2_runtime.prepared_input_reused = partial_bindings.is_some()
            || ((reuse || apply_retained_changes) && c2_runtime.prepared_input.is_some());
        let prepared = if let Some(prepared) = partial_bindings {
            c2_runtime.prepared_input = Some(prepared.clone());
            prepared
        } else if c2_runtime.prepared_input_reused {
            let mut prepared = c2_runtime
                .prepared_input
                .take()
                .expect("checked prepared input");
            if apply_retained_changes {
                c2_runtime.apply_retained_changes_to_prepared_input(&mut prepared);
            }
            c2_runtime.prepared_input = Some(prepared.clone());
            prepared
        } else {
            c2_runtime.source_change_hint = None;
            c2_runtime.prepared_document_nodes_copied = document.nodes.len();
            let (mut binding_document, binding_report) =
                AuiRuntimeResolver::resolve_bindings(document, &snapshot);
            c2_runtime.apply_retained_properties(&mut binding_document);
            let themed_document = if binding_document.schema_version
                == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
            {
                themes
                    .resolve_document(&binding_document, &AuiControlInteractionSnapshot::default())
                    .map(|resolved| resolved.document)
                    .unwrap_or(binding_document)
            } else {
                binding_document
            };
            let mut viewport_diagnostics = Vec::new();
            let mut viewport_input_identity = None;
            let mut viewport_canvas_count = 0;
            let resolved_document = if crate::aui_viewport::AuiViewportResolver::requires_viewport(
                &themed_document,
            ) {
                match viewport {
                    Some(viewport) => {
                        match crate::aui_viewport::AuiViewportResolver::resolve_document(
                            &themed_document,
                            viewport,
                        ) {
                            Ok(resolution) => {
                                viewport_input_identity = Some(resolution.input_identity);
                                viewport_canvas_count = resolution.canvases.len();
                                resolution.document
                            }
                            Err(error) => {
                                viewport_diagnostics.extend(error.diagnostics);
                                themed_document
                            }
                        }
                    }
                    None => {
                        viewport_diagnostics.push(crate::aui_viewport::AuiViewportDiagnostic {
                        code: "aui.viewport.input_required".to_string(),
                        canvas_id: None,
                        node_id: None,
                        message: "ViewportLayout requires a platform or deterministic headless viewport input."
                            .to_string(),
                    });
                        themed_document
                    }
                }
            } else {
                themed_document
            };
            let node_slots = resolved_document
                .nodes
                .iter()
                .enumerate()
                .map(|(slot, node)| (node.node_id.clone(), slot))
                .collect();
            let mut binding_slots = HashMap::<String, Vec<(usize, AuiBindingTarget)>>::new();
            for (slot, node) in document.nodes.iter().enumerate() {
                for binding in &node.binding_refs {
                    binding_slots
                        .entry(binding.path.clone())
                        .or_default()
                        .push((slot, binding.target_field));
                }
            }
            let prepared = AuiPreparedInput {
                document: Arc::new(resolved_document),
                node_slots: Arc::new(node_slots),
                binding_slots: Arc::new(binding_slots),
                binding_report,
                viewport_diagnostics,
                viewport_input_identity,
                viewport_canvas_count,
            };
            c2_runtime.prepared_input = Some(prepared.clone());
            prepared
        };
        let AuiPreparedInput {
            document: resolved_document,
            node_slots: _,
            binding_slots: _,
            mut binding_report,
            viewport_diagnostics,
            viewport_input_identity,
            viewport_canvas_count,
        } = prepared;
        binding_report.frame_index = snapshot.frame_index;
        let snapshot_report = snapshot_output
            .report
            .with_binding_report(document, &binding_report);
        c2_runtime.timings.prepare_style_preflight_ns += aui_elapsed_ns(prepare_started);
        if c2_runtime.measurement_enabled {
            if !c2_runtime.prepared_input_reused {
                c2_runtime.work.full_prepare_count += 1;
                c2_runtime.work.full_prepare_node_count += resolved_document.nodes.len() as u64;
            }
            c2_runtime.work.prepared_document_nodes_copied +=
                c2_runtime.prepared_document_nodes_copied as u64;
            c2_runtime.work.prepared_nodes_rebuilt += if c2_runtime.prepared_input_reused {
                c2_runtime.source_change_hint.as_ref().map_or(0, Vec::len) as u64
            } else {
                resolved_document.nodes.len() as u64
            };
        }
        let (layout, paragraph_layouts) = if resolved_document.schema_version
            == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
        {
            c2_runtime.layout_shared(
                Arc::clone(&resolved_document),
                snapshot.frame_index,
                font_bundles,
            )
        } else {
            (
                c2_runtime
                    .layout_legacy_shared(Arc::clone(&resolved_document), snapshot.frame_index),
                Arc::new(HashMap::new()),
            )
        };
        let drawing_started = c2_runtime
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        c2_runtime
            .drawing_cache
            .prepare_graphics(&resolved_document, presentation);
        let (draw_list, base_draw_report) = AuiLayoutEngine::extract_draw_list_cached(
            &resolved_document,
            &layout,
            None,
            &mut c2_runtime.drawing_cache,
        );
        let mut composition = AuiRendererBridge::build_composition_frame_retained(
            snapshot.frame_index,
            &resolved_document,
            &layout,
            &draw_list,
            &mut c2_runtime.drawing_cache,
        );
        let mut overlay = composition.to_overlay_frame();
        c2_runtime.drawing_cache.pending_glyph_overlay = Some(overlay.draw_items.clone());
        let glyph_plan = if resolved_document.schema_version
            == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
            && font_bundles.default_bundle().is_some()
        {
            build_c2_text_glyph_plan_cached(
                &overlay,
                font_bundles,
                presentation,
                &paragraph_layouts,
                Some(&mut c2_runtime.drawing_cache),
            )
        } else if font_bundles.default_bundle().is_some() {
            build_text_glyph_plan_from_bundles_for_presentation(
                &overlay,
                font_bundles,
                presentation,
            )
        } else {
            build_text_glyph_plan(&overlay, font_atlases)
        };
        let glyph_plan = glyph_plan.map(std::sync::Arc::new);
        overlay.glyph_plan = glyph_plan.clone();
        composition.glyph_plan = glyph_plan.clone();
        c2_runtime
            .drawing_cache
            .publish_projection_hint(&mut composition, glyph_plan.clone());
        c2_runtime.timings.draw_projection_glyph_ns += aui_elapsed_ns(drawing_started);
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
        diagnostics.extend(viewport_diagnostics.iter().map(|diagnostic| {
            AuiRuntimePresentDiagnostic::error(
                &diagnostic.code,
                &diagnostic.message,
                diagnostic
                    .node_id
                    .clone()
                    .or_else(|| diagnostic.canvas_id.clone()),
            )
        }));
        diagnostics.extend(layout.diagnostics.iter().map(|item| {
            AuiRuntimePresentDiagnostic::error(&item.code, &item.message, item.node_id.clone())
        }));
        diagnostics.extend(base_draw_report.graphics_diagnostics.iter().map(|message| {
            AuiRuntimePresentDiagnostic::error(
                "aui_graphics.draw_failed",
                message,
                Some(document.document_id.clone()),
            )
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
                        "{} requested glyphs are missing from the font/atlas or use the legacy replacement glyph.",
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

        let base_draw_report = Some(AuiRetainedDrawReport {
            document: resolved_document.clone(),
            layout: layout.clone(),
            report: base_draw_report,
            composition: composition.clone(),
            overlay: overlay.clone(),
            atlases: font_atlases.clone(),
            bundles: font_bundles.clone(),
            themes: themes.clone(),
            presentation: presentation.cloned(),
        });
        AuiRuntimePresentOutput {
            overlay,
            composition,
            resolved_document,
            layout,
            base_draw_report,
            virtual_lists: Vec::new(),
            paragraph_layouts,
            report: AuiRuntimePresentReport {
                schema_version: "aui-runtime-present-report.v2".to_string(),
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
                viewport_status: if viewport_diagnostics.is_empty() {
                    if viewport_canvas_count > 0 {
                        "resolved"
                    } else {
                        "reference_canvas"
                    }
                } else {
                    "error"
                }
                .to_string(),
                viewport_input_identity,
                viewport_canvas_count,
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

    pub fn apply_control_feedback_with_fonts_and_themes(
        output: &mut AuiRuntimePresentOutput,
        interaction: &AuiInteractionResult,
        state: &mut crate::aui_control_feedback::AuiControlFeedbackState,
        presentation_delta_us: u64,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        themes: &crate::aui_theme::AuiThemeRegistry,
    ) -> crate::aui_control_feedback::AuiControlFeedbackFrame {
        Self::apply_control_feedback_with_fonts_for_presentation_and_themes(
            output,
            interaction,
            state,
            presentation_delta_us,
            font_atlases,
            font_bundles,
            themes,
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
        Self::apply_control_feedback_with_fonts_for_presentation_and_themes(
            output,
            interaction,
            state,
            presentation_delta_us,
            font_atlases,
            font_bundles,
            &crate::aui_theme::AuiThemeRegistry::builtin(),
            presentation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_control_feedback_with_fonts_for_presentation_and_themes(
        output: &mut AuiRuntimePresentOutput,
        interaction: &AuiInteractionResult,
        state: &mut crate::aui_control_feedback::AuiControlFeedbackState,
        presentation_delta_us: u64,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        themes: &crate::aui_theme::AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> crate::aui_control_feedback::AuiControlFeedbackFrame {
        let started = state
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        let frame = Self::apply_control_feedback_measured_inner(
            output,
            interaction,
            state,
            presentation_delta_us,
            font_atlases,
            font_bundles,
            themes,
            presentation,
        );
        state.cpu_ns += aui_elapsed_ns(started);
        state.source_nodes_visited += frame.report.document_nodes_scanned as u64;
        frame
    }

    fn apply_control_feedback_measured_inner(
        output: &mut AuiRuntimePresentOutput,
        interaction: &AuiInteractionResult,
        state: &mut crate::aui_control_feedback::AuiControlFeedbackState,
        presentation_delta_us: u64,
        font_atlases: &RuntimeAuiFontAtlasRegistry,
        font_bundles: &RuntimeFontBundleRegistry,
        themes: &crate::aui_theme::AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> crate::aui_control_feedback::AuiControlFeedbackFrame {
        let has_feedback_command = interaction.commands.iter().any(|c| {
            matches!(
                c.command_kind,
                AuiCommandKind::Click
                    | AuiCommandKind::Submit
                    | AuiCommandKind::PointerCancel
                    | AuiCommandKind::Cancel
                    | AuiCommandKind::DragCancel
            )
        });
        if !state.is_animating()
            && !has_feedback_command
            && state
                .last_frame_id
                .is_none_or(|last| interaction.control_snapshot.frame_id >= last)
        {
            if let Some(index) = state.presentation_variants.iter().position(|cached| {
                cached.matches(
                    output,
                    &interaction.control_snapshot,
                    font_atlases,
                    font_bundles,
                    themes,
                    presentation,
                )
            }) {
                let cached = state
                    .presentation_variants
                    .remove(index)
                    .expect("matched feedback presentation variant");
                let mut frame = cached.frame.clone();
                frame.report = crate::aui_control_feedback::AuiControlFeedbackReport {
                    frame_id: interaction.control_snapshot.frame_id,
                    override_count: frame.overrides.len(),
                    resolved_profile_ids: frame.report.resolved_profile_ids,
                    presentation_cache_hit: true,
                    large_delta_clamp_count: usize::from(
                        presentation_delta_us
                            > crate::aui_control_feedback::MAX_PRESENTATION_DELTA_US,
                    ),
                    ..Default::default()
                };
                state.last_frame_id = Some(interaction.control_snapshot.frame_id);
                state.presentation_variants.push_front(cached);
                return frame;
            }
        }
        let mut previous_variants = std::mem::take(&mut state.presentation_variants);
        // Resume from the base publication. Feedback never accumulates on last frame's transform.
        if let Some(base) = &output.base_draw_report {
            output.layout = base.layout.clone();
        }
        let source_slots = state
            .source_cursor
            .and_then(|cursor| AuiDrawingCache::changed_slots_since(&output.layout, cursor));
        let changed_ids = source_slots.as_ref().map(|slots| {
            slots
                .iter()
                .map(|&slot| output.layout.computed_nodes[slot].node_id.as_str())
                .collect::<Vec<_>>()
        });
        let mut feedback =
            crate::aui_control_feedback::AuiControlFeedbackModule::advance_retained_changes(
                state,
                &output.resolved_document,
                &interaction.control_snapshot,
                &interaction.commands,
                presentation_delta_us,
                changed_ids.as_deref(),
            );
        let update = &output.layout.interaction_index_update;
        state.source_cursor = Some((update.generation, update.revision, update.node_count));
        previous_variants.retain(|cached| {
            cached.matches_feedback_state(&interaction.control_snapshot, &feedback.overrides)
        });
        if let Some(index) = previous_variants.iter().position(|cached| {
            cached.matches(
                output,
                &interaction.control_snapshot,
                font_atlases,
                font_bundles,
                themes,
                presentation,
            )
        }) {
            let mut cached = previous_variants
                .remove(index)
                .expect("matched retained feedback presentation variant");
            feedback.report.presentation_cache_hit = true;
            cached.frame = feedback.clone();
            previous_variants.push_front(cached);
            previous_variants.truncate(crate::aui_feedback_cache::MAX_FEEDBACK_VARIANTS);
            state.presentation_variants = previous_variants;
            return feedback;
        }
        state.presentation_variants = previous_variants;
        // Default-state styles were already resolved by the base presenter. Reuse that
        // owned product, including when a preceding frame applied hover/press feedback.
        // External edits invalidate the base identities and take the conservative path.
        let neutral_style = interaction.control_snapshot.hovered_node.is_none()
            && interaction.control_snapshot.pressed_node.is_none()
            && (!interaction.control_snapshot.focus_visible
                || interaction.control_snapshot.focused_node.is_none());
        if neutral_style && feedback.overrides.is_empty() {
            if let Some(base) = output.base_draw_report.as_ref().filter(|base| {
                Arc::ptr_eq(&base.document, &output.resolved_document)
                    && Arc::ptr_eq(&base.layout, &output.layout)
                    && (Arc::ptr_eq(&base.atlases.atlases_by_id, &font_atlases.atlases_by_id)
                        || (base.atlases.is_empty() && font_atlases.is_empty()))
                    && base.atlases.default_ui_font_atlas_id
                        == font_atlases.default_ui_font_atlas_id
                    && (Arc::ptr_eq(&base.bundles.bundles_by_id, &font_bundles.bundles_by_id)
                        || (base.bundles.bundles_by_id.is_empty()
                            && font_bundles.bundles_by_id.is_empty()))
                    && base.bundles.default_ui_font_bundle_id
                        == font_bundles.default_ui_font_bundle_id
                    && base.themes.shares_storage(themes)
                    && match (&base.presentation, presentation) {
                        (Some(old), Some(next)) => old.same_ui_mapping(next),
                        (None, None) => true,
                        _ => false,
                    }
            }) {
                output.composition = base.composition.clone();
                output.overlay = base.overlay.clone();
                state.presentation_variants.clear();
                state.style_variants.clear();
                feedback.report.presentation_cache_hit = true;
                return feedback;
            }
        }
        state
            .drawing_cache
            .prepare_graphics(&output.resolved_document, presentation);
        let local_feedback = prepare_feedback_drawing(
            output,
            &interaction.control_snapshot,
            &feedback.overrides,
            themes,
            &mut state.drawing_cache,
        );
        let state_document = if local_feedback {
            output.resolved_document.clone()
        } else if let Some(index) = state.style_variants.iter().position(|cached| {
            cached.matches(
                &output.resolved_document,
                themes,
                &interaction.control_snapshot,
            )
        }) {
            let cached = state
                .style_variants
                .remove(index)
                .expect("matched feedback style variant");
            let document = cached.document.clone();
            state.style_variants.push_front(cached);
            document
        } else {
            let document = if output.resolved_document.schema_version
                == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
            {
                state.style_rebuilds += 1;
                themes
                    .resolve_document(&output.resolved_document, &interaction.control_snapshot)
                    .map(|resolved| Arc::new(resolved.document))
                    .unwrap_or_else(|_| output.resolved_document.clone())
            } else {
                output.resolved_document.clone()
            };
            if crate::aui_feedback_cache::feedback_variant_is_bounded(
                output.resolved_document.nodes.len(),
            ) {
                state.style_variants.push_front(
                    crate::aui_feedback_cache::CachedFeedbackStyles::new(
                        output.resolved_document.clone(),
                        themes.clone(),
                        interaction.control_snapshot.clone(),
                        document.clone(),
                    ),
                );
                state
                    .style_variants
                    .truncate(crate::aui_feedback_cache::MAX_FEEDBACK_VARIANTS);
            }
            document
        };
        let base_layout = output.layout.clone();
        let feedback_layout =
            compose_feedback_geometry(&state_document, &base_layout, &feedback.overrides, state);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list_cached(
            &state_document,
            &feedback_layout,
            Some(&feedback.overrides),
            &mut state.drawing_cache,
        );
        state.drawing_cache.feedback_style_targets =
            feedback_style_targets(&interaction.control_snapshot);
        state.drawing_cache.feedback_overrides = feedback.overrides.clone();
        state.drawing_cache.feedback_themes =
            Some(drawing_cache::DrawingThemeIdentity(themes.clone()));
        let mut composition = AuiRendererBridge::build_composition_frame_retained(
            output.report.frame_index,
            &state_document,
            &feedback_layout,
            &draw_list,
            &mut state.drawing_cache,
        );
        let mut overlay = composition.to_overlay_frame();
        state.drawing_cache.pending_glyph_overlay = Some(overlay.draw_items.clone());
        let glyph_plan = if output.resolved_document.schema_version
            == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
            && font_bundles.default_bundle().is_some()
        {
            build_c2_text_glyph_plan_cached(
                &overlay,
                font_bundles,
                presentation,
                &output.paragraph_layouts,
                Some(&mut state.drawing_cache),
            )
        } else if font_bundles.default_bundle().is_some() {
            build_text_glyph_plan_from_bundles_for_presentation(
                &overlay,
                font_bundles,
                presentation,
            )
        } else {
            build_text_glyph_plan(&overlay, font_atlases)
        };
        // Feedback currently reconstructs its output. Preserve immutable products
        // when the result is unchanged so Renderer and RHI do not redo that work.
        let glyph_plan = match (glyph_plan, output.composition.glyph_plan.as_ref()) {
            (Some(next), Some(previous)) if !local_feedback && &next == previous.as_ref() => {
                Some(previous.clone())
            }
            (next, _) => next.map(std::sync::Arc::new),
        };
        for stage in &mut composition.stages {
            if let Some(previous) = output.composition.stage(stage.stage) {
                if !local_feedback && stage.draw_items == previous.draw_items {
                    stage.draw_items = previous.draw_items.clone();
                    state
                        .drawing_cache
                        .projection_stages
                        .insert(stage.stage, stage.draw_items.clone());
                    if let Some(delta) = state
                        .drawing_cache
                        .pending_projection_stages
                        .get_mut(&stage.stage)
                    {
                        delta.current_items = stage.draw_items.clone();
                    }
                }
            }
        }
        // The bridge emits sorted stages. Preserve single-stage sharing after
        // restoring an equal previous stage's drawing identity above.
        if let [stage] = composition.stages.as_slice() {
            overlay.draw_items = stage.draw_items.clone();
        }
        overlay.glyph_plan = glyph_plan.clone();
        composition.glyph_plan = glyph_plan.clone();
        state
            .drawing_cache
            .publish_projection_hint(&mut composition, glyph_plan.clone());
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
        output.layout = feedback_layout;
        state.drawing_cache.record_layout(&base_layout);
        feedback.report.presentation_rebuilt = true;
        if crate::aui_feedback_cache::feedback_variant_is_bounded(
            output.resolved_document.nodes.len(),
        ) {
            state.presentation_variants.push_front(Box::new(
                crate::aui_feedback_cache::CachedFeedbackPresentation::new(
                    output,
                    &interaction.control_snapshot,
                    font_atlases,
                    font_bundles,
                    themes,
                    presentation,
                    feedback.clone(),
                ),
            ));
            state
                .presentation_variants
                .truncate(crate::aui_feedback_cache::MAX_FEEDBACK_VARIANTS);
        }
        feedback
    }
}

fn feedback_style_targets(interaction: &AuiControlInteractionSnapshot) -> BTreeSet<String> {
    [
        interaction.hovered_node.as_ref(),
        interaction.pressed_node.as_ref(),
        interaction
            .focus_visible
            .then_some(interaction.focused_node.as_ref())
            .flatten(),
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect()
}

fn compose_feedback_geometry(
    document: &AuiDocument,
    base: &Arc<AuiLayoutResult>,
    overrides: &crate::aui_control_feedback::AuiVisualOverrideSet,
    state: &mut crate::aui_control_feedback::AuiControlFeedbackState,
) -> Arc<AuiLayoutResult> {
    use project_runtime_sdk::aui_presentation::{Affine2D, Transform2D};
    if overrides.is_empty() || base.computed_nodes.iter().all(|n| n.presentation.is_none()) {
        return base.clone();
    }
    let cache = &mut state.drawing_cache;
    let (rebuilds, entries) = cache.indices.refresh(document, base);
    cache.stats.index_rebuilds += rebuilds;
    cache.stats.index_entries_built += entries;
    let mut pending = overrides
        .iter()
        .map(|(id, _)| id.to_owned())
        .collect::<Vec<_>>();
    let mut affected = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if let Some(slot) = cache.indices.computed.get(&id) {
            if !affected.insert(slot) {
                continue;
            }
            if let Some(node) = cache.indices.nodes.get(&id) {
                pending.extend(document.nodes[node].children.iter().cloned());
            }
        }
    }
    let mut layout = (**base).clone();
    let mut corrections = HashMap::<usize, Affine2D>::new();
    let mut feedback_colors = HashMap::<usize, [f32; 4]>::new();
    for &slot in &affected {
        let node = &base.computed_nodes[slot];
        let Some(mut geometry) = node.presentation else {
            continue;
        };
        let source = cache
            .indices
            .nodes
            .get(&node.node_id)
            .map(|i| &document.nodes[i]);
        let parent = source
            .and_then(|n| n.parent.as_ref())
            .and_then(|id| cache.indices.computed.get(id));
        let parent_correction = parent
            .and_then(|p| corrections.get(&p))
            .copied()
            .unwrap_or(Affine2D::IDENTITY);
        let mut feedback_color = parent
            .and_then(|p| feedback_colors.get(&p))
            .copied()
            .unwrap_or([1.0; 4]);
        let base_matrix =
            Affine2D::from_coefficients(geometry.local_to_canvas).expect("published matrix");
        let own = overrides.get(&node.node_id).copied();
        let transform = own.map_or(Transform2D::default(), |v| Transform2D {
            translation: [v.translation.x, v.translation.y],
            scale: [v.scale; 2],
            ..Default::default()
        });
        if let Some(v) = own {
            for channel in &mut feedback_color[..3] {
                *channel *= v.brightness_multiplier;
            }
            feedback_color[3] *= v.opacity_multiplier;
        }
        let matrix = parent_correction.compose(base_matrix).and_then(|m| {
            m.compose(transform.local_matrix([0.0; 2], [node.rect.width, node.rect.height])?)
        });
        let Ok(matrix) = matrix else {
            layout.computed_nodes[slot].effective_visible = false;
            continue;
        };
        corrections.insert(
            slot,
            base_matrix
                .try_inverse()
                .and_then(|inverse| matrix.compose(inverse))
                .unwrap_or(parent_correction),
        );
        feedback_colors.insert(slot, feedback_color);
        geometry.local_to_canvas = matrix.coefficients();
        let origin = Transform2D {
            translation: [-node.rect.x, -node.rect.y],
            ..Default::default()
        }
        .local_matrix([0.0; 2], [0.0; 2])
        .expect("layout origin");
        let Ok(vertex) = matrix.compose(origin) else {
            layout.computed_nodes[slot].effective_visible = false;
            continue;
        };
        geometry.vertex_transform = vertex.coefficients();
        geometry.bounds = crate::aui_geometry_cache::transformed_bounds(
            node.rect.width,
            node.rect.height,
            matrix,
        );
        for (color, feedback) in geometry.modulation.iter_mut().zip(feedback_color) {
            *color *= feedback;
        }
        geometry.modulation[3] = geometry.modulation[3].clamp(0., 1.);
        let mut clips = node.effective_clips.clone();
        let mut clip_rect = Some(geometry.bounds);
        for clip in &mut clips {
            if let Some(owner) = cache.indices.computed.get(&clip.node_id) {
                if let Some(owner_geometry) = layout.computed_nodes[owner].presentation {
                    clip.transform = Some(owner_geometry.vertex_transform);
                    clip_rect = clip_rect.and_then(|r| r.intersection(owner_geometry.bounds));
                }
            }
        }
        let parent_visible = parent.is_none_or(|p| layout.computed_nodes[p].effective_visible);
        let target = &mut layout.computed_nodes[slot];
        target.presentation = Some(geometry);
        target.effective_clips = clips;
        target.effective_clip_rect = if target.effective_clips.is_empty() {
            None
        } else {
            clip_rect
        };
        target.effective_visible = node.local_visible
            && parent_visible
            && (target.effective_clips.is_empty() || clip_rect.is_some());
    }
    let key = (
        base.interaction_index_update.generation,
        base.computed_nodes.len(),
    );
    if state.geometry_base_key != Some(key) {
        state.geometry_journal.reset(key.1);
        state.geometry_base_key = Some(key);
    }
    let mut slots = affected.into_iter().collect::<Vec<_>>();
    if let Some(changes) = &cache.draw_changes_once {
        slots.extend(changes);
        slots.sort_unstable();
        slots.dedup();
    } else {
        slots.extend(0..key.1);
        slots.sort_unstable();
        slots.dedup();
    }
    layout.interaction_index_update = state.geometry_journal.record(key.1, &slots);
    Arc::new(layout)
}

fn prepare_feedback_drawing(
    output: &AuiRuntimePresentOutput,
    interaction: &AuiControlInteractionSnapshot,
    overrides: &crate::aui_control_feedback::AuiVisualOverrideSet,
    themes: &crate::aui_theme::AuiThemeRegistry,
    cache: &mut AuiDrawingCache,
) -> bool {
    cache.draw_changes_once = None;
    cache.draw_node_overrides.clear();
    if output.resolved_document.schema_version
        != crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION
        || !cache
            .feedback_themes
            .as_ref()
            .is_some_and(|old| old.0.shares_storage(themes))
        || !output.base_draw_report.as_ref().is_some_and(|base| {
            Arc::ptr_eq(&base.document, &output.resolved_document)
                && Arc::ptr_eq(&base.layout, &output.layout)
                && base.themes.shares_storage(themes)
        })
        || cache.nodes.len() != output.layout.computed_nodes.len()
        || output.layout.computed_nodes.len() > drawing_cache::MAX_DRAW_NODES
    {
        return false;
    }
    let Some(mut slots) = cache
        .changed_layout_slots(&output.layout)
        .or_else(|| cache.has_recorded_layout(&output.layout).then(Vec::new))
    else {
        return false;
    };
    let targets = feedback_style_targets(interaction);
    let document = &output.resolved_document;
    let mut paint_nodes = HashMap::new();
    for target in &targets {
        let Some(slot) = cache.indices.nodes.get(target) else {
            continue;
        };
        let mut node = document.nodes[slot].clone();
        let Some(theme_id) = node
            .resolved_style
            .as_ref()
            .map(|style| style.theme_id.clone())
        else {
            return false;
        };
        if themes
            .resolve_prepared_node_with_interaction(&theme_id, &mut node, interaction)
            .is_err()
        {
            return false;
        }
        paint_nodes.insert(slot, node);
    }
    for target in targets.iter().chain(&cache.feedback_style_targets) {
        if let Some(slot) = cache.indices.computed.get(target) {
            slots.push(slot);
        }
    }
    let mut subtree_roots = BTreeSet::new();
    for (id, _) in overrides.iter().chain(cache.feedback_overrides.iter()) {
        if overrides.get(id) != cache.feedback_overrides.get(id)
            || cache
                .indices
                .computed
                .get(id)
                .is_some_and(|slot| slots.contains(&slot))
        {
            subtree_roots.insert(id);
        }
    }
    let mut pending = subtree_roots.into_iter().collect::<Vec<_>>();
    let mut visited = HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        if let Some(slot) = cache.indices.computed.get(id) {
            slots.push(slot);
        }
        if let Some(slot) = cache.indices.nodes.get(id) {
            pending.extend(document.nodes[slot].children.iter().map(String::as_str));
        }
    }
    slots.sort_unstable();
    slots.dedup();
    cache.draw_changes_once = Some(slots);
    cache.draw_node_overrides = paint_nodes;
    true
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
            quads.push(Arc::new(AuiTextGlyphQuad {
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint: ch as u32,
                font_bundle: None,
                glyph_id: glyph.glyph_id.clone(),
                rect,
                uv_rect: glyph.uv_rect,
                page_index: glyph.page_index,
                render_mode: FontBundleRenderMode::BitmapR8,
                clipped,
                color: None,
            }));
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

fn resolve_auto_hybrid_glyph<T>(
    physical_size: f32,
    bitmap: Option<T>,
    msdf: Option<T>,
    bitmap_pixel_size: impl FnOnce(&T) -> u16,
) -> Option<T> {
    let bitmap_is_close = bitmap.as_ref().is_some_and(|glyph| {
        (0.875..=1.125).contains(&(physical_size / f32::from(bitmap_pixel_size(glyph).max(1))))
    });
    if physical_size <= 32.0 && bitmap_is_close {
        bitmap
    } else {
        msdf.or(bitmap)
    }
}

#[cfg(test)]
fn build_c2_text_glyph_plan(
    overlay: &AuiOverlayFrame,
    font_bundles: &RuntimeFontBundleRegistry,
    presentation: Option<&ResolvedGameViewPresentation>,
    paragraphs: &HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>,
) -> Option<AuiTextGlyphPlan> {
    build_c2_text_glyph_plan_cached(overlay, font_bundles, presentation, paragraphs, None)
}

fn resolve_c2_raster_glyph<'a>(
    bundle: &'a crate::font_bundle::RuntimeLoadedFontBundle,
    face_id: &str,
    glyph_id: u32,
    mode: FontBundleRenderMode,
    pixel_size: u16,
    index: Option<&drawing_cache::C2RasterGlyphIndex>,
) -> (Option<&'a crate::font_bundle::CookedFontBundleGlyph>, usize) {
    if let Some(slots) = index.and_then(|index| index.candidates(bundle, face_id, glyph_id, mode)) {
        return (
            slots
                .iter()
                .map(|&slot| &bundle.metadata.glyphs[slot])
                .min_by_key(|glyph| glyph.pixel_size.abs_diff(pixel_size)),
            slots.len(),
        );
    }
    (
        bundle
            .metadata
            .glyphs
            .iter()
            .filter(|glyph| {
                glyph.font_face_id == face_id
                    && u32::from(glyph.glyph_id) == glyph_id
                    && glyph.render_mode == mode
            })
            .min_by_key(|glyph| glyph.pixel_size.abs_diff(pixel_size)),
        bundle.metadata.glyphs.len(),
    )
}

fn build_c2_text_glyph_plan_cached(
    overlay: &AuiOverlayFrame,
    font_bundles: &RuntimeFontBundleRegistry,
    presentation: Option<&ResolvedGameViewPresentation>,
    paragraphs: &HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>,
    mut cache: Option<&mut AuiDrawingCache>,
) -> Option<AuiTextGlyphPlan> {
    let index = cache
        .as_mut()
        .and_then(|cache| cache.raster_index(font_bundles));
    build_c2_text_glyph_plan_indexed(
        overlay,
        font_bundles,
        presentation,
        paragraphs,
        cache,
        index.as_deref(),
    )
}

fn build_c2_text_glyph_plan_indexed(
    overlay: &AuiOverlayFrame,
    font_bundles: &RuntimeFontBundleRegistry,
    presentation: Option<&ResolvedGameViewPresentation>,
    paragraphs: &HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>,
    mut cache: Option<&mut AuiDrawingCache>,
    index: Option<&drawing_cache::C2RasterGlyphIndex>,
) -> Option<AuiTextGlyphPlan> {
    if let Some(cache) = cache.as_mut() {
        if cache
            .pending_glyph_overlay
            .take()
            .is_some_and(|items| Arc::ptr_eq(&items, &overlay.draw_items))
        {
            if let Some(plan) = update_c2_glyph_slots(
                overlay,
                font_bundles,
                presentation,
                paragraphs,
                cache,
                index,
            ) {
                return Some(plan);
            }
        }
    }
    let previous_fragments = cache
        .as_mut()
        .map(|c| std::mem::take(&mut c.glyphs))
        .unwrap_or_default();
    // Match the source command identity, not its previous visible ordinal.
    // A queue also handles multiple text commands with the same node identity.
    let mut previous =
        HashMap::<String, std::collections::VecDeque<drawing_cache::GlyphFragment>>::new();
    for fragment in previous_fragments {
        previous
            .entry(fragment.item.node_id.clone())
            .or_default()
            .push_back(fragment);
    }
    let current_ids: HashSet<_> = overlay
        .draw_items
        .iter()
        .filter(|item| item.item_kind == AuiOverlayItemKind::Text)
        .map(|item| item.item_id.as_str())
        .collect();
    let same_fonts = cache
        .as_ref()
        .and_then(|c| c.fonts.as_ref())
        .is_some_and(|old| {
            Arc::ptr_eq(&old.bundles_by_id, &font_bundles.bundles_by_id)
                && old.default_ui_font_bundle_id == font_bundles.default_ui_font_bundle_id
        });
    if let Some(cache) = cache.as_mut() {
        cache.fonts = None;
        cache.pending_glyph_groups = None;
        cache.glyph_slots.clear();
        cache.glyph_presentation = None;
    }
    if overlay.report.text_count == 0 {
        return None;
    }
    let default_bundle = font_bundles.default_bundle()?;
    let mut quads = Vec::new();
    let mut requested_glyph_count = 0;
    let mut unsupported_glyph_count = 0;
    let mut clipped_glyph_count = 0;
    let mut fallback_used = false;

    let mut fragments = Vec::new();
    let mut changed_groups = BTreeMap::<String, Arc<Vec<Arc<AuiTextGlyphQuad>>>>::new();
    let retain = cache.is_some() && overlay.draw_items.len() <= drawing_cache::MAX_DRAW_NODES;
    for item in overlay
        .draw_items
        .iter()
        .filter(|item| item.item_kind == AuiOverlayItemKind::Text)
    {
        if let Some(cache) = cache.as_mut() {
            cache.stats.text_items_visited += 1;
        }
        let paragraph = paragraphs
            .get(&item.node_id)
            .or_else(|| paragraphs.get(draw_command_source_node_id(&item.node_id)));
        let target_scale = presentation
            .and_then(|value| value.canvas_reference_to_target_scale(&item.canvas_id))
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(1.0);
        let old = previous
            .get_mut(&item.node_id)
            .and_then(|fragments| fragments.pop_front());
        if let Some(cache) = cache.as_mut() {
            cache.stats.text_items_compared += 1;
            if old
                .as_ref()
                .is_some_and(|old| same_fonts && old.matches(item, paragraph, target_scale))
            {
                let mut old = old.expect("checked reusable raster fragment");
                let reindexed = old.item.item_id != item.item_id;
                old.item = item.clone();
                if reindexed {
                    // Published quads carry the new ordinal. Rebind only that
                    // metadata; glyph selection, geometry and UVs stay unchanged.
                    old.quads = old
                        .quads
                        .iter()
                        .map(|quad| {
                            let mut next = quad.as_ref().clone();
                            next.item_id.clone_from(&item.item_id);
                            Arc::new(next)
                        })
                        .collect();
                    old.hash = stable_glyph_fragment_hash(&old.quads);
                    changed_groups.insert(item.item_id.clone(), Arc::new(old.quads.clone()));
                    cache.stats.quad_payloads_built += old.quads.len() as u64;
                    cache.stats.glyph_hash_leaves_built += u64::from(!old.quads.is_empty());
                } else {
                    cache.stats.quad_payloads_reused += old.quads.len() as u64;
                    cache.stats.glyph_hash_leaves_reused += u64::from(!old.quads.is_empty());
                }
                cache.stats.text_items_reused += 1;
                quads.extend_from_slice(&old.quads);
                requested_glyph_count += old.requested;
                unsupported_glyph_count += old.unsupported;
                clipped_glyph_count += old.clipped;
                fallback_used |= old.fallback;
                if retain {
                    fragments.push(old);
                }
                continue;
            }
            cache.stats.text_items_built += 1;
        }
        if let Some(old) = old
            .as_ref()
            .filter(|old| !current_ids.contains(old.item.item_id.as_str()))
        {
            changed_groups.insert(old.item.item_id.clone(), Arc::new(Vec::new()));
        }
        let Some(paragraph) = paragraph else {
            unsupported_glyph_count += item
                .text
                .as_deref()
                .map(str::chars)
                .map(Iterator::count)
                .unwrap_or(0);
            changed_groups.insert(item.item_id.clone(), Arc::new(Vec::new()));
            if retain {
                fragments.push(drawing_cache::GlyphFragment {
                    item: item.clone(),
                    paragraph: None,
                    target_scale,
                    quads: Vec::new(),
                    requested: 0,
                    unsupported: item
                        .text
                        .as_deref()
                        .map(str::chars)
                        .map(Iterator::count)
                        .unwrap_or(0),
                    clipped: 0,
                    fallback: false,
                    hash: 0,
                });
            }
            continue;
        };
        let start = (
            quads.len(),
            requested_glyph_count,
            unsupported_glyph_count,
            clipped_glyph_count,
        );
        let mut item_fallback = false;
        let text = item.text.as_deref().unwrap_or("");
        for shaped in &paragraph.glyphs {
            let font_size = shaped.font_size.max(1.0);
            let physical_size = font_size * target_scale;
            let pixel_size = physical_size.round().clamp(1.0, f32::from(u16::MAX)) as u16;
            let bundle = font_bundles
                .bundles_by_id
                .get(&shaped.font_bundle_id)
                .unwrap_or(default_bundle);
            let policy = shaped.raster_policy;
            requested_glyph_count += 1;
            fallback_used |= shaped.fallback_used;
            item_fallback |= shaped.fallback_used;
            if shaped.glyph_id == 0 {
                unsupported_glyph_count += 1;
                continue;
            }
            if !shaped.has_outline {
                continue;
            }
            let resolve = |mode| {
                resolve_c2_raster_glyph(
                    bundle,
                    &shaped.face_id,
                    shaped.glyph_id,
                    mode,
                    pixel_size,
                    index,
                )
                .0
            };
            let glyph = match policy {
                AuiFontRasterPolicy::Bitmap => resolve(FontBundleRenderMode::BitmapR8),
                AuiFontRasterPolicy::Msdf => resolve(FontBundleRenderMode::MsdfRgba8),
                AuiFontRasterPolicy::AutoHybrid => resolve_auto_hybrid_glyph(
                    physical_size,
                    resolve(FontBundleRenderMode::BitmapR8),
                    resolve(FontBundleRenderMode::MsdfRgba8),
                    |glyph| glyph.pixel_size,
                ),
            };
            let Some(glyph) = glyph else {
                unsupported_glyph_count += 1;
                continue;
            };
            let Some(page) = bundle.metadata.pages.get(glyph.page_index as usize) else {
                unsupported_glyph_count += 1;
                continue;
            };
            let raster_scale = font_size / f32::from(glyph.pixel_size.max(1));
            let (left, top, width, height) =
                if let Some(bounds) = glyph.plane_bounds_per_em_millionths {
                    let [x, y, width, height] =
                        bounds.map(|value| value as f32 * font_size / 1_000_000.0);
                    (x, y, width, height)
                } else {
                    (
                        glyph.bearing_x as f32 * raster_scale,
                        glyph.bearing_y as f32 * raster_scale,
                        glyph.pixel_rect[2] as f32 * raster_scale,
                        glyph.pixel_rect[3] as f32 * raster_scale,
                    )
                };
            let rect = AuiComputedRect {
                x: item.rect.x + shaped.x + left,
                y: item.rect.y + shaped.y - top,
                width,
                height,
            };
            let ink = shaped
                .ink_bounds
                .map_or(rect, |[x0, y0, x1, y1]| AuiComputedRect {
                    x: item.rect.x + shaped.x + x0,
                    y: item.rect.y + shaped.y - y1,
                    width: x1 - x0,
                    height: y1 - y0,
                });
            let own_clip = (paragraph.overflow != crate::aui_layout_schema::TextOverflow::Visible)
                .then_some(item.rect);
            let clip = match (item.effective_clip_rect, own_clip) {
                (Some(parent), Some(own)) => parent.intersection(own),
                (Some(parent), None) => Some(parent),
                (None, Some(own)) => Some(own),
                (None, None) => None,
            };
            let clipped = clip.is_some_and(|clip| {
                ink.x < clip.x
                    || ink.y < clip.y
                    || ink.x + ink.width > clip.x + clip.width
                    || ink.y + ink.height > clip.y + clip.height
            });
            clipped_glyph_count += usize::from(clipped);
            let codepoint = text
                .get(shaped.cluster_start as usize..shaped.cluster_end as usize)
                .and_then(|cluster| cluster.chars().next())
                .map(u32::from)
                .unwrap_or(u32::from('…'));
            let [x, y, width, height] = glyph.pixel_rect;
            quads.push(Arc::new(AuiTextGlyphQuad {
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint,
                glyph_id: format!("{}:{}", shaped.face_id, shaped.glyph_id),
                font_bundle: Some((
                    bundle.metadata.font_bundle_id.clone(),
                    bundle.metadata.generation,
                )),
                rect,
                uv_rect: [
                    (x as f32 + 0.5) / page.width as f32,
                    (y as f32 + 0.5) / page.height as f32,
                    (x as f32 + width as f32 - 0.5) / page.width as f32,
                    (y as f32 + height as f32 - 0.5) / page.height as f32,
                ],
                page_index: glyph.page_index,
                render_mode: glyph.render_mode,
                clipped,
                color: shaped.color.clone(),
            }));
        }
        if let Some(cache) = cache.as_mut() {
            cache.stats.quad_payloads_built += (quads.len() - start.0) as u64;
            cache.stats.glyph_hash_leaves_built += u64::from(quads.len() > start.0);
        }
        changed_groups.insert(item.item_id.clone(), Arc::new(quads[start.0..].to_vec()));
        if retain {
            let fragment_quads = quads[start.0..].to_vec();
            fragments.push(drawing_cache::GlyphFragment {
                item: item.clone(),
                paragraph: Some(paragraph.clone()),
                target_scale,
                hash: stable_glyph_fragment_hash(&fragment_quads),
                quads: fragment_quads,
                requested: requested_glyph_count - start.1,
                unsupported: unsupported_glyph_count - start.2,
                clipped: clipped_glyph_count - start.3,
                fallback: item_fallback,
            });
        }
    }
    for old in previous.into_values().flatten() {
        if !current_ids.contains(old.item.item_id.as_str()) {
            changed_groups.insert(old.item.item_id.clone(), Arc::new(Vec::new()));
        }
    }
    let first_page = default_bundle.metadata.pages.first()?;
    let retain_glyphs = retain && quads.len() <= drawing_cache::MAX_GLYPH_QUADS;
    let glyph_plan_hash = if retain_glyphs {
        let cache = cache
            .as_mut()
            .expect("retain_glyphs requires drawing cache");
        cache.stats.glyph_hash_fragments_visited += fragments.len() as u64;
        let (root, hash_rebuilt) = cache.glyph_hash.update(&fragments, &changed_groups);
        cache.stats.glyph_hash_structure_rebuilds += u64::from(hash_rebuilt);
        stable_glyph_plan_hash_from_root(
            &default_bundle.metadata.font_bundle_id,
            root,
            fragments
                .iter()
                .filter(|fragment| !fragment.quads.is_empty())
                .count(),
        )
    } else {
        stable_glyph_plan_hash(&default_bundle.metadata.font_bundle_id, &quads)
    };
    if retain_glyphs {
        if let Some(cache) = cache.as_mut() {
            cache.glyphs = fragments;
            cache.glyph_slots = cache
                .glyphs
                .iter()
                .enumerate()
                .map(|(slot, fragment)| (fragment.item.item_id.clone(), slot))
                .collect();
            cache.glyph_presentation = presentation.cloned();
            cache.fonts = Some(font_bundles.clone());
            cache.pending_glyph_groups = Some(changed_groups);
        }
    }
    if let Some(cache) = cache.as_mut() {
        cache.stats.quad_refs_submitted += quads.len() as u64;
    }
    Some(AuiTextGlyphPlan {
        font_atlas_id: default_bundle.metadata.font_bundle_id.clone(),
        font_source_kind: "project_font_bundle_v2_paragraph".into(),
        font_asset_id: default_bundle.metadata.font_stack_id.clone(),
        font_asset_status: if unsupported_glyph_count == 0 {
            "qualified"
        } else {
            "incomplete"
        }
        .into(),
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

fn update_c2_glyph_slots(
    overlay: &AuiOverlayFrame,
    font_bundles: &RuntimeFontBundleRegistry,
    presentation: Option<&ResolvedGameViewPresentation>,
    paragraphs: &HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>,
    cache: &mut AuiDrawingCache,
    index: Option<&drawing_cache::C2RasterGlyphIndex>,
) -> Option<AuiTextGlyphPlan> {
    let previous = cache.previous_glyph_plan.as_ref()?;
    if cache.pending_projection_stages.is_empty()
        || cache.glyphs.len() != overlay.report.text_count
        || !cache.fonts.as_ref().is_some_and(|old| {
            Arc::ptr_eq(&old.bundles_by_id, &font_bundles.bundles_by_id)
                && old.default_ui_font_bundle_id == font_bundles.default_ui_font_bundle_id
        })
        || !match (&cache.glyph_presentation, presentation) {
            (Some(old), Some(next)) => old.same_ui_mapping(next),
            (None, None) => true,
            _ => false,
        }
    {
        return None;
    }
    let mut changes = Vec::new();
    for delta in cache.pending_projection_stages.values() {
        for &slot in delta.changed_slots.iter() {
            let item = delta.current_items.get(slot)?;
            if item.item_kind != AuiOverlayItemKind::Text {
                continue;
            }
            let glyph_slot = *cache.glyph_slots.get(&item.item_id)?;
            changes.push((glyph_slot, item.clone()));
        }
    }
    // No mutations until every changed item has a valid replacement.
    let mut replacements = Vec::with_capacity(changes.len());
    for (slot, item) in changes {
        let single = AuiOverlayFrame {
            frame_index: overlay.frame_index,
            draw_items: Arc::new(vec![item.clone()]),
            report: AuiRenderReport {
                text_count: 1,
                ..Default::default()
            },
            glyph_plan: None,
        };
        let plan = build_c2_text_glyph_plan_indexed(
            &single,
            font_bundles,
            presentation,
            paragraphs,
            None,
            index,
        )?;
        let paragraph = paragraphs
            .get(&item.node_id)
            .or_else(|| paragraphs.get(draw_command_source_node_id(&item.node_id)))
            .cloned();
        let target_scale = presentation
            .and_then(|value| value.canvas_reference_to_target_scale(&item.canvas_id))
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(1.0);
        replacements.push((
            slot,
            drawing_cache::GlyphFragment {
                item,
                paragraph,
                target_scale,
                hash: stable_glyph_fragment_hash(&plan.quads),
                quads: plan.quads,
                requested: plan.requested_glyph_count,
                unsupported: plan.unsupported_glyph_count,
                clipped: plan.clipped_glyph_count,
                fallback: plan.fallback_used,
            },
        ));
    }
    let quad_count = replacements
        .iter()
        .fold(previous.quads.len(), |count, (slot, fragment)| {
            count - cache.glyphs[*slot].quads.len() + fragment.quads.len()
        });
    if quad_count > drawing_cache::MAX_GLYPH_QUADS {
        return None;
    }
    let mut changed_groups = BTreeMap::new();
    let mut slots = Vec::with_capacity(replacements.len());
    for (slot, fragment) in replacements {
        cache.stats.text_items_visited += 1;
        cache.stats.text_items_compared += 1;
        cache.stats.text_items_built += 1;
        cache.stats.quad_payloads_built += fragment.quads.len() as u64;
        cache.stats.glyph_hash_leaves_built += u64::from(!fragment.quads.is_empty());
        changed_groups.insert(
            fragment.item.item_id.clone(),
            Arc::new(fragment.quads.clone()),
        );
        cache.glyphs[slot] = fragment;
        slots.push(slot);
    }
    let mut plan = AuiTextGlyphPlan {
        font_atlas_id: previous.font_atlas_id.clone(),
        font_source_kind: previous.font_source_kind.clone(),
        font_asset_id: previous.font_asset_id.clone(),
        font_asset_status: String::new(),
        fallback_used: false,
        requested_glyph_count: 0,
        rendered_glyph_count: 0,
        unsupported_glyph_count: 0,
        clipped_glyph_count: 0,
        atlas_width: previous.atlas_width,
        atlas_height: previous.atlas_height,
        atlas_generation: previous.atlas_generation,
        glyph_plan_hash: String::new(),
        quads: Vec::with_capacity(quad_count),
    };
    for fragment in &cache.glyphs {
        plan.quads.extend_from_slice(&fragment.quads);
        plan.requested_glyph_count += fragment.requested;
        plan.unsupported_glyph_count += fragment.unsupported;
        plan.clipped_glyph_count += fragment.clipped;
        plan.fallback_used |= fragment.fallback;
    }
    cache.stats.quad_refs_submitted += plan.quads.len() as u64;
    cache.stats.text_items_reused += (cache.glyphs.len() - slots.len()) as u64;
    plan.rendered_glyph_count = plan.quads.len();
    plan.font_asset_status = if plan.unsupported_glyph_count == 0 {
        "qualified"
    } else {
        "incomplete"
    }
    .into();
    let (root, hash_visited, hash_rebuilt) = cache.glyph_hash.update_slots(&cache.glyphs, &slots);
    cache.stats.glyph_hash_fragments_visited += hash_visited as u64;
    cache.stats.glyph_hash_structure_rebuilds += u64::from(hash_rebuilt);
    plan.glyph_plan_hash =
        stable_glyph_plan_hash_from_root(&plan.font_atlas_id, root, cache.glyph_hash.leaf_count());
    cache.pending_glyph_groups = Some(changed_groups);
    Some(plan)
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
        let selected_bundle = bundle_id
            .as_deref()
            .and_then(|id| font_bundles.bundles_by_id.get(id))
            .unwrap_or(default_bundle);
        if !selected_bundle.font_face_sources.is_empty() {
            let shaped_plan = build_shaped_item_quads(
                item,
                text,
                selected_bundle,
                physical_font_size,
                requested_pixel_size,
                raster_policy,
            );
            requested_glyph_count += shaped_plan.requested;
            unsupported_glyph_count += shaped_plan.unsupported;
            clipped_glyph_count += shaped_plan.quads.iter().filter(|quad| quad.clipped).count();
            fallback_used |= shaped_plan.fallback;
            quads.extend(shaped_plan.quads.into_iter().map(Arc::new));
            continue;
        }
        // Explicit adapter for old bundles without shaping input.
        let mut cursor_x = item.rect.x;
        // Text rectangles use a top-left origin, while cooked glyph metrics are
        // baseline-relative. Keep one shared baseline and apply each glyph's
        // bearings so punctuation (，。、“”) sits on the same line as Hanzi.
        let baseline_y = item.rect.y + font_size * 0.80;
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
                AuiFontRasterPolicy::AutoHybrid => resolve_auto_hybrid_glyph(
                    physical_font_size,
                    resolve(FontBundleRenderMode::BitmapR8),
                    resolve(FontBundleRenderMode::MsdfRgba8),
                    |resolved| resolved.glyph.pixel_size,
                ),
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
            let metric_scale = match resolved.glyph.render_mode {
                FontBundleRenderMode::BitmapR8 => scale,
                // MSDF bearings are stored in font units (1000/em).
                FontBundleRenderMode::MsdfRgba8 => font_size / 1000.0,
            };
            let rect = AuiComputedRect {
                x: cursor_x + resolved.glyph.bearing_x as f32 * metric_scale,
                y: baseline_y - resolved.glyph.bearing_y as f32 * metric_scale,
                width: resolved.glyph.pixel_rect[2] as f32 * scale,
                height: resolved.glyph.pixel_rect[3] as f32 * scale,
            };
            let clipped = rect.x + rect.width > item.rect.x + item.rect.width
                || rect.y + rect.height > item.rect.y + item.rect.height;
            if clipped {
                clipped_glyph_count += 1;
            }
            let [x, y, width, height] = resolved.glyph.pixel_rect;
            // Keep linear filtering inside the packed glyph cell. The cooker
            // leaves one pixel of padding, but sampling exactly on the cell
            // edge can still pull a neighbouring punctuation stroke into the
            // quad at large UI sizes.
            let inset = 0.5_f32;
            let u0 = (x as f32 + inset) / page.width as f32;
            let v0 = (y as f32 + inset) / page.height as f32;
            let u1 = (x as f32 + width as f32 - inset) / page.width as f32;
            let v1 = (y as f32 + height as f32 - inset) / page.height as f32;
            quads.push(Arc::new(AuiTextGlyphQuad {
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint: u32::from(ch),
                font_bundle: Some((
                    resolved.font_bundle_id.clone(),
                    font_bundles.bundles_by_id[&resolved.font_bundle_id]
                        .metadata
                        .generation,
                )),
                glyph_id: format!(
                    "{}:{}",
                    resolved.glyph.font_face_id, resolved.glyph.glyph_id
                ),
                rect,
                uv_rect: [u0, v0, u1, v1],
                page_index: resolved.glyph.page_index,
                render_mode: resolved.glyph.render_mode,
                clipped,
                color: None,
            }));
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
        font_asset_status: if unsupported_glyph_count == 0 {
            "qualified"
        } else {
            "incomplete"
        }
        .to_string(),
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

#[derive(Default)]
struct ShapedItemQuads {
    quads: Vec<AuiTextGlyphQuad>,
    requested: usize,
    unsupported: usize,
    fallback: bool,
}

fn build_shaped_item_quads(
    item: &AuiOverlayDrawItem,
    text: &str,
    bundle: &crate::font_bundle::RuntimeLoadedFontBundle,
    physical_size: f32,
    pixel_size: u16,
    policy: AuiFontRasterPolicy,
) -> ShapedItemQuads {
    let mut result = ShapedItemQuads::default();
    let font_size = item.font_size.unwrap_or(12.).max(1.);
    let family = item
        .font
        .as_ref()
        .and_then(|font| font.font_family_id.as_deref());
    let weight = item.font.as_ref().map(|font| font.weight).unwrap_or(400);
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
    let mut sources: Vec<_> = bundle.font_face_sources.iter().collect();
    sources.sort_by_key(|source| {
        bundle
            .metadata
            .glyphs
            .iter()
            .filter(|g| g.font_face_id == source.font_face_id)
            .map(|g| {
                (
                    family.is_some_and(|f| f != g.font_family_id),
                    g.style != style,
                    g.weight.abs_diff(weight),
                )
            })
            .min()
            .unwrap_or((true, true, u16::MAX))
    });
    let Some(runs) = shape_stack(&sources, text) else {
        result.requested = text.chars().count();
        result.unsupported = result.requested;
        return result;
    };
    let line = layout_line(
        &runs,
        font_size,
        item.rect.height,
        item.node_id.ends_with(":label"),
    );
    result.requested = line.glyphs.len();
    for placed in &line.glyphs {
        let run = &runs[placed.run_index];
        let shaped = &run.items[placed.item_index];
        result.fallback |= run.fallback_used;
        let cluster = text
            .get(shaped.cluster_start as usize..shaped.cluster_end as usize)
            .unwrap_or("");
        let ch = cluster.chars().next().unwrap_or('\u{fffd}');
        if shaped.glyph_id == 0 {
            result.unsupported += 1;
            continue;
        }
        // Whitespace advances without requiring a visible atlas cell.
        if !shaped.has_outline {
            continue;
        }
        let resolve = |mode| {
            bundle
                .metadata
                .glyphs
                .iter()
                .filter(|g| {
                    g.font_face_id == run.face_id
                        && u32::from(g.glyph_id) == shaped.glyph_id
                        && g.render_mode == mode
                })
                .min_by_key(|g| g.pixel_size.abs_diff(pixel_size))
        };
        let glyph = match policy {
            AuiFontRasterPolicy::Bitmap => resolve(FontBundleRenderMode::BitmapR8),
            AuiFontRasterPolicy::Msdf => resolve(FontBundleRenderMode::MsdfRgba8),
            AuiFontRasterPolicy::AutoHybrid => resolve_auto_hybrid_glyph(
                physical_size,
                resolve(FontBundleRenderMode::BitmapR8),
                resolve(FontBundleRenderMode::MsdfRgba8),
                |glyph| glyph.pixel_size,
            ),
        };
        let Some(glyph) = glyph else {
            result.unsupported += 1;
            continue;
        };
        let Some(page) = bundle.metadata.pages.get(glyph.page_index as usize) else {
            result.unsupported += 1;
            continue;
        };
        let scale = font_size / f32::from(glyph.pixel_size.max(1));
        let (left, top, width, height) = if let Some(bounds) = glyph.plane_bounds_per_em_millionths
        {
            let [x, y, w, h] = bounds.map(|v| v as f32 * font_size / 1_000_000.);
            (x, y, w, h)
        } else {
            (
                glyph.bearing_x as f32 * scale,
                glyph.bearing_y as f32 * scale,
                glyph.pixel_rect[2] as f32 * scale,
                glyph.pixel_rect[3] as f32 * scale,
            )
        };
        let rect = AuiComputedRect {
            x: item.rect.x + placed.x + left,
            y: item.rect.y + line.baseline + placed.y - top,
            width,
            height,
        };
        let clip = item.effective_clip_rect.unwrap_or(item.rect);
        // Distance-field padding is transparent support, not text ink. It may
        // extend beyond the line box without clipping a visible glyph.
        let ink = if glyph.render_mode == FontBundleRenderMode::MsdfRgba8 {
            let [x0, y0, x1, y1] = shaped.ink_bounds.expect("visible shaped glyph");
            let units = font_size / f32::from(run.units_per_em);
            AuiComputedRect {
                x: item.rect.x + placed.x + f32::from(x0) * units,
                y: item.rect.y + line.baseline + placed.y - f32::from(y1) * units,
                width: (f32::from(x1) - f32::from(x0)) * units,
                height: (f32::from(y1) - f32::from(y0)) * units,
            }
        } else {
            rect
        };
        let clipped = ink.x < clip.x
            || ink.y < clip.y
            || ink.x + ink.width > clip.x + clip.width
            || ink.y + ink.height > clip.y + clip.height;
        let [x, y, w, h] = glyph.pixel_rect;
        result.quads.push(AuiTextGlyphQuad {
            item_id: item.item_id.clone(),
            node_id: item.node_id.clone(),
            codepoint: u32::from(ch),
            font_bundle: Some((
                bundle.metadata.font_bundle_id.clone(),
                bundle.metadata.generation,
            )),
            glyph_id: format!("{}:{}", run.face_id, shaped.glyph_id),
            rect,
            uv_rect: [
                (x as f32 + 0.5) / page.width as f32,
                (y as f32 + 0.5) / page.height as f32,
                (x as f32 + w as f32 - 0.5) / page.width as f32,
                (y as f32 + h as f32 - 0.5) / page.height as f32,
            ],
            page_index: glyph.page_index,
            render_mode: glyph.render_mode,
            clipped,
            color: None,
        });
    }
    result
}

fn stable_glyph_plan_hash(font_atlas_id: &str, quads: &[Arc<AuiTextGlyphQuad>]) -> String {
    let mut fragment_hashes = Vec::new();
    let mut start = 0;
    while start < quads.len() {
        let item_id = quads[start].item_id.as_str();
        let mut end = start + 1;
        while end < quads.len() && quads[end].item_id == item_id {
            end += 1;
        }
        fragment_hashes.push(stable_glyph_fragment_hash(&quads[start..end]));
        start = end;
    }
    let root = stable_glyph_hash_root(&fragment_hashes);
    stable_glyph_plan_hash_from_root(font_atlas_id, root, fragment_hashes.len())
}

fn stable_glyph_fragment_hash(quads: &[Arc<AuiTextGlyphQuad>]) -> u64 {
    use std::fmt::Write as _;

    let mut hash = StableFnv1a::default();
    hash.write_u64(quads.len() as u64);
    for quad in quads {
        let _ = write!(
            hash,
            "|{:?}:{}|{}:{}:{}:{:.3}:{:.3}:{:.3}:{:.3}:{:.6}:{:.6}:{:.6}:{:.6}:{}:{:?}:{}:{:?}",
            quad.font_bundle,
            quad.glyph_id,
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
            quad.clipped,
            quad.color
        );
    }
    hash.0
}

pub(super) fn stable_glyph_hash_pair(left: u64, right: u64) -> u64 {
    let mut hash = StableFnv1a::default();
    hash.write_bytes(b"glyph-pair-v1");
    hash.write_u64(left);
    hash.write_u64(right);
    hash.0
}

fn stable_glyph_hash_root(leaves: &[u64]) -> u64 {
    if leaves.is_empty() {
        return 0;
    }
    let base = leaves.len().next_power_of_two();
    let mut tree = vec![0; base * 2];
    tree[base..base + leaves.len()].copy_from_slice(leaves);
    for node in (1..base).rev() {
        tree[node] = stable_glyph_hash_pair(tree[node * 2], tree[node * 2 + 1]);
    }
    tree[1]
}

fn stable_glyph_plan_hash_from_root(font_atlas_id: &str, root: u64, item_count: usize) -> String {
    let mut hash = StableFnv1a::default();
    hash.write_bytes(b"glyph-plan-v2");
    hash.write_bytes(font_atlas_id.as_bytes());
    hash.write_u64(item_count as u64);
    hash.write_u64(root);
    format!("{:016x}", hash.0)
}

struct StableFnv1a(u64);

impl Default for StableFnv1a {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl StableFnv1a {
    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }
}

impl std::fmt::Write for StableFnv1a {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.write_bytes(value.as_bytes());
        Ok(())
    }
}

pub struct AuiInteractionSystem;

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
                .filter(|action_ref| {
                    if action_ref.event == action_event {
                        return true;
                    }
                    action_ref.event == AuiActionEvent::Activate
                        && matches!(action_event, AuiActionEvent::Click | AuiActionEvent::Submit)
                        // During migration an explicit binding wins for this action
                        // and command only. Never deduplicate separate user operations.
                        && !node.action_refs.iter().any(|explicit| {
                            explicit.event == action_event && explicit.action_id == action_ref.action_id
                        })
                })
            {
                actions.push(AuiAction {
                    action_id: action_ref.action_id.clone(),
                    node_id: node.node_id.clone(),
                    event: action_ref.event,
                    payload: command.payload.clone().or_else(|| {
                        node.virtual_list_item.as_ref().map(|item| {
                            serde_json::to_string(&
                                crate::aui_virtual_list::AuiVirtualListActionPayload::from(item)
                            )
                            .expect("virtual list action payload is serializable")
                        })
                    }),
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
        AuiCommandKind::ValueChanged => Some(AuiActionEvent::ValueChanged),
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
    shapes: Vec<AuiResolvedClip>,
}

#[cfg(test)]
#[path = "aui/tests.rs"]
mod tests;
