use engine_runtime::aui::{
    AuiBindingRef, AuiBindingTarget, AuiBindingValue, AuiCanvas, AuiDocument,
    AuiInteractionFeedbackProfile, AuiNode, AuiNodeKind, AuiRect, AuiStyle,
    AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID, AUI_DOCUMENT_SCHEMA_VERSION,
    LEGACY_AUI_DOCUMENT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub const AUI_DOCUMENT_COOK_REPORT_SCHEMA_VERSION: &str = "aui-document-cook-report.v1";

#[derive(Debug, Clone)]
pub struct AuiDocumentCookRequest {
    pub source_path: PathBuf,
    pub document: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiDocumentCookStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiDocumentCookDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiDocumentCookDiagnostic {
    pub severity: AuiDocumentCookDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl AuiDocumentCookDiagnostic {
    fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self {
            severity: AuiDocumentCookDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path,
            suggestion,
        }
    }

    fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self {
            severity: AuiDocumentCookDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
            suggestion,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiDocumentCookReport {
    pub schema_version: String,
    pub status: AuiDocumentCookStatus,
    pub source_path: String,
    pub package_path: Option<String>,
    pub document_id: Option<String>,
    pub source_shape: String,
    #[serde(default)]
    pub source_schema_version: Option<String>,
    #[serde(default)]
    pub normalized_schema_version: Option<String>,
    #[serde(default)]
    pub feedback_fallback_identity: Option<String>,
    pub canvas_count: usize,
    pub node_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    pub asset_refs: Vec<String>,
    pub diagnostics: Vec<AuiDocumentCookDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct AuiDocumentCookOutput {
    pub document: AuiDocument,
    pub package_path: String,
    pub report: AuiDocumentCookReport,
}

pub struct AuiDocumentCooker;

impl AuiDocumentCooker {
    pub fn cook(
        request: AuiDocumentCookRequest,
    ) -> Result<AuiDocumentCookOutput, AuiDocumentCookReport> {
        let mut diagnostics = Vec::new();
        let source_shape = detect_source_shape(&request.document).to_string();
        let source_schema_version = request
            .document
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let asset_refs = collect_asset_refs(&request.document)
            .into_iter()
            .collect::<Vec<_>>();

        let cook_result = if source_shape == "runtime_aui_document" {
            cook_runtime_document(&request.document, &mut diagnostics)
        } else {
            diagnostics.push(AuiDocumentCookDiagnostic::warning(
                "AuiLegacyAuthoringTreeNormalized",
                "Legacy AUI authoring tree was normalized to runtime AuiDocument shape.",
                Some(request.source_path.display().to_string()),
                Some("Save new AUI documents in canonical runtime-compatible shape.".to_string()),
            ));
            cook_legacy_document(&request.document, &mut diagnostics)
        };

        let source_path = request.source_path.display().to_string();
        match cook_result {
            Ok(document) => {
                let package_path = format!(
                    "aui/documents/{}.aui.json",
                    sanitize_package_id(&document.document_id)
                );
                let report = AuiDocumentCookReport {
                    schema_version: AUI_DOCUMENT_COOK_REPORT_SCHEMA_VERSION.to_string(),
                    status: AuiDocumentCookStatus::Success,
                    source_path,
                    package_path: Some(package_path.clone()),
                    document_id: Some(document.document_id.clone()),
                    source_shape,
                    source_schema_version,
                    normalized_schema_version: Some(document.schema_version.clone()),
                    feedback_fallback_identity: Some(feedback_fallback_identity(&document)),
                    canvas_count: document.canvases.len(),
                    node_count: document.nodes.len(),
                    binding_count: document
                        .nodes
                        .iter()
                        .map(|node| node.binding_refs.len())
                        .sum(),
                    action_count: document
                        .nodes
                        .iter()
                        .map(|node| node.action_refs.len())
                        .sum(),
                    asset_refs,
                    diagnostics,
                };
                Ok(AuiDocumentCookOutput {
                    document,
                    package_path,
                    report,
                })
            }
            Err(message) => {
                diagnostics.push(AuiDocumentCookDiagnostic::error(
                    "AuiDocumentCookFailed",
                    message,
                    Some(source_path.clone()),
                    Some("Fix the AUI document id/root/node tree before building.".to_string()),
                ));
                let report = AuiDocumentCookReport {
                    schema_version: AUI_DOCUMENT_COOK_REPORT_SCHEMA_VERSION.to_string(),
                    status: AuiDocumentCookStatus::Failed,
                    source_path,
                    package_path: None,
                    document_id: None,
                    source_shape,
                    source_schema_version,
                    normalized_schema_version: None,
                    feedback_fallback_identity: None,
                    canvas_count: 0,
                    node_count: 0,
                    binding_count: 0,
                    action_count: 0,
                    asset_refs,
                    diagnostics,
                };
                Err(report)
            }
        }
    }
}

fn feedback_fallback_identity(document: &AuiDocument) -> String {
    document
        .interaction_feedback
        .as_ref()
        .and_then(|registry| registry.default_button_profile.clone())
        .unwrap_or_else(|| AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID.to_string())
}

fn detect_source_shape(value: &serde_json::Value) -> &'static str {
    if value.get("canvases").is_some() && value.get("nodes").is_some() {
        "runtime_aui_document"
    } else {
        "legacy_authoring_tree"
    }
}

fn cook_runtime_document(
    value: &serde_json::Value,
    diagnostics: &mut Vec<AuiDocumentCookDiagnostic>,
) -> Result<AuiDocument, String> {
    let source_schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut document = serde_json::from_value::<AuiDocument>(value.clone()).map_err(|error| {
        let message = error.to_string();
        if message.contains("easing")
            || (message.contains("unknown variant") && message.contains("easeOutCubic"))
        {
            diagnostics.push(AuiDocumentCookDiagnostic::error(
                "AuiFeedbackEasingInvalid",
                format!("AUI feedback easing must use a supported finite easing: {message}"),
                Some("interaction_feedback.profiles".to_string()),
                Some("Use linear, easeOutCubic, or easeOutBack.".to_string()),
            ));
        }
        format!("Failed to parse runtime AuiDocument: {message}")
    })?;
    if document.schema_version.trim().is_empty() {
        document.schema_version = AUI_DOCUMENT_SCHEMA_VERSION.to_string();
    } else if source_schema == LEGACY_AUI_DOCUMENT_SCHEMA_VERSION {
        document.schema_version = AUI_DOCUMENT_SCHEMA_VERSION.to_string();
        diagnostics.push(AuiDocumentCookDiagnostic::warning(
            "AuiDocumentV1MigratedToV2",
            format!(
                "AUI document schema '{}' was normalized to '{}' with feedback=auto fallback.",
                LEGACY_AUI_DOCUMENT_SCHEMA_VERSION, AUI_DOCUMENT_SCHEMA_VERSION
            ),
            Some("schema_version".to_string()),
            Some("Save the cooked document to persist the normalized v2 schema.".to_string()),
        ));
    }
    if let Some(registry) = &mut document.interaction_feedback {
        registry
            .profiles
            .sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
    }
    validate_document(&document, diagnostics)?;
    Ok(document)
}

fn cook_legacy_document(
    value: &serde_json::Value,
    diagnostics: &mut Vec<AuiDocumentCookDiagnostic>,
) -> Result<AuiDocument, String> {
    let document_id = value
        .get("documentId")
        .or_else(|| value.get("document_id"))
        .or_else(|| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "AUI documentId is required.".to_string())?
        .to_string();
    let root = value
        .get("root")
        .ok_or_else(|| "Legacy AUI document requires root.".to_string())?;
    let root_node_id = string_field(root, "nodeId")
        .or_else(|| string_field(root, "node_id"))
        .unwrap_or_else(|| "aui-root".to_string());
    let canvas = AuiCanvas::screen_overlay("main", 1280.0, 720.0, root_node_id.clone());
    let mut nodes = Vec::new();
    cook_legacy_node(root, None, &mut nodes, diagnostics)?;
    let document = AuiDocument::new(document_id, vec![canvas], nodes);
    validate_document(&document, diagnostics)?;
    Ok(document)
}

fn cook_legacy_node(
    value: &serde_json::Value,
    parent: Option<String>,
    nodes: &mut Vec<AuiNode>,
    diagnostics: &mut Vec<AuiDocumentCookDiagnostic>,
) -> Result<String, String> {
    let node_id = string_field(value, "nodeId")
        .or_else(|| string_field(value, "node_id"))
        .ok_or_else(|| "Legacy AUI node requires nodeId.".to_string())?;
    let node_type = string_field(value, "nodeType")
        .or_else(|| string_field(value, "node_type"))
        .unwrap_or_else(|| "panel".to_string());
    let children_values = value
        .get("children")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    let kind = match node_type.as_str() {
        "canvas" | "panel" => AuiNodeKind::Panel,
        "text" => AuiNodeKind::Text,
        "image" | "image-row" => AuiNodeKind::Image,
        "button" => AuiNodeKind::Button,
        "progress-bar" | "progressBar" => AuiNodeKind::ProgressBar,
        other => {
            diagnostics.push(AuiDocumentCookDiagnostic::warning(
                "AuiLegacyUnknownNodeType",
                format!("Unknown legacy AUI nodeType '{other}' was normalized to Panel."),
                Some(format!("nodes.{node_id}.nodeType")),
                Some(
                    "Use canvas, panel, text, image, image-row, button, or progress-bar."
                        .to_string(),
                ),
            ));
            AuiNodeKind::Panel
        }
    };

    if node_type == "image-row" {
        diagnostics.push(AuiDocumentCookDiagnostic::warning(
            "AuiLegacyImageRowCollapsed",
            "Legacy image-row was normalized to one Image node for C-min package present.",
            Some(format!("nodes.{node_id}")),
            Some("Model repeated UI images as explicit Image nodes in canonical AUI.".to_string()),
        ));
    }

    let mut child_ids = Vec::new();
    for child in &children_values {
        let child_id = cook_legacy_node(child, Some(node_id.clone()), nodes, diagnostics)?;
        child_ids.push(child_id);
    }

    let mut node = AuiNode::new(node_id.clone(), kind, rect_from_legacy(value, &node_type));
    node.parent = parent;
    node.children = child_ids;
    node.name = string_field(value, "name").unwrap_or_else(|| node_id.clone());
    node.visible = value
        .get("visible")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    node.style = Some(style_from_legacy(value, kind));

    match kind {
        AuiNodeKind::Text => {
            node.text = string_field(value, "text").or_else(|| Some(String::new()));
            if node_id.contains("score") {
                node.binding_refs.push(AuiBindingRef::new(
                    format!("{node_id}.text"),
                    AuiBindingTarget::TextText,
                    "game.score_text",
                    Some(AuiBindingValue::String(
                        node.text
                            .clone()
                            .unwrap_or_else(|| "SCORE 000000".to_string()),
                    )),
                ));
            } else if node_id.contains("wave") {
                node.binding_refs.push(AuiBindingRef::new(
                    format!("{node_id}.text"),
                    AuiBindingTarget::TextText,
                    "game.wave_text",
                    Some(AuiBindingValue::String(
                        node.text.clone().unwrap_or_else(|| "WAVE 1".to_string()),
                    )),
                ));
            } else if node_id.contains("enemy") {
                node.binding_refs.push(AuiBindingRef::new(
                    format!("{node_id}.text"),
                    AuiBindingTarget::TextText,
                    "game.enemy_count_text",
                    Some(AuiBindingValue::String(
                        node.text.clone().unwrap_or_else(|| "0".to_string()),
                    )),
                ));
            }
        }
        AuiNodeKind::Image => {
            if let Some(asset_id) = value
                .get("imageRef")
                .or_else(|| value.get("image_ref"))
                .and_then(|image| image.get("id"))
                .and_then(serde_json::Value::as_str)
            {
                node.image = Some(engine_runtime::aui::AuiAssetRef::new(asset_id));
            }
            if node_id.contains("life") || node_id.contains("ship") || node_id.contains("player") {
                node.binding_refs.push(AuiBindingRef::new(
                    format!("{node_id}.image"),
                    AuiBindingTarget::ImageAssetRef,
                    "player.ship_icon",
                    node.image.clone().map(AuiBindingValue::AssetRef),
                ));
            }
        }
        AuiNodeKind::ProgressBar => {
            node.progress_value = value
                .get("value")
                .or_else(|| value.get("progressValue"))
                .and_then(serde_json::Value::as_f64)
                .map(|value| value as f32)
                .or(Some(1.0));
            node.binding_refs.push(AuiBindingRef::new(
                format!("{node_id}.value"),
                AuiBindingTarget::ProgressBarValue,
                "player.hp_ratio",
                Some(AuiBindingValue::Number(node.progress_value.unwrap_or(1.0))),
            ));
        }
        _ => {}
    }

    nodes.push(node);
    Ok(node_id)
}

fn rect_from_legacy(value: &serde_json::Value, node_type: &str) -> AuiRect {
    if let Some(rect) = value.get("rect") {
        let x = rect
            .get("x")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        let y = rect
            .get("y")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        let width = rect
            .get("width")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(240.0) as f32;
        let height = rect
            .get("height")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(48.0) as f32;
        return AuiRect::fixed_position(x, y, width, height);
    }

    match node_type {
        "canvas" => AuiRect::stretch_full(),
        "text" => match string_field(value, "anchor").as_deref() {
            Some("top-right") => AuiRect::fixed_position(980.0, 24.0, 260.0, 36.0),
            Some("bottom-left") => AuiRect::fixed_position(24.0, 650.0, 260.0, 36.0),
            Some("bottom-right") => AuiRect::fixed_position(980.0, 650.0, 260.0, 36.0),
            _ => AuiRect::fixed_position(24.0, 24.0, 280.0, 36.0),
        },
        "image" | "image-row" => match string_field(value, "anchor").as_deref() {
            Some("top-right") => AuiRect::fixed_position(1112.0, 24.0, 120.0, 32.0),
            Some("bottom-left") => AuiRect::fixed_position(24.0, 640.0, 120.0, 32.0),
            Some("bottom-right") => AuiRect::fixed_position(1112.0, 640.0, 120.0, 32.0),
            _ => AuiRect::fixed_position(24.0, 72.0, 120.0, 32.0),
        },
        "progress-bar" | "progressBar" => AuiRect::fixed_position(24.0, 66.0, 220.0, 18.0),
        _ => AuiRect::fixed_position(0.0, 0.0, 240.0, 48.0),
    }
}

fn style_from_legacy(value: &serde_json::Value, kind: AuiNodeKind) -> AuiStyle {
    let color = string_field(value, "color");
    let text_color = string_field(value, "textColor").or_else(|| string_field(value, "text_color"));
    let font_size = value
        .get("fontSize")
        .or_else(|| value.get("font_size"))
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32);
    match kind {
        AuiNodeKind::Text => AuiStyle {
            color: None,
            text_color: text_color.or_else(|| Some("#ffffff".to_string())),
            font_size: font_size.or(Some(24.0)),
            font: None,
        },
        AuiNodeKind::Panel | AuiNodeKind::Button | AuiNodeKind::ProgressBar => AuiStyle {
            color: color.or_else(|| Some("#101820cc".to_string())),
            text_color,
            font_size,
            font: None,
        },
        _ => AuiStyle {
            color,
            text_color,
            font_size,
            font: None,
        },
    }
}

fn validate_document(
    document: &AuiDocument,
    diagnostics: &mut Vec<AuiDocumentCookDiagnostic>,
) -> Result<(), String> {
    if document.schema_version != AUI_DOCUMENT_SCHEMA_VERSION {
        diagnostics.push(AuiDocumentCookDiagnostic::error(
            "AuiDocumentSchemaMismatch",
            format!(
                "AUI document schema_version '{}' is not normalized schema '{}'.",
                document.schema_version, AUI_DOCUMENT_SCHEMA_VERSION
            ),
            Some("schema_version".to_string()),
            Some("Migrate source v1 through AuiDocumentCooker before packaging.".to_string()),
        ));
        return Err("AUI document must use normalized aui-document.v2 schema.".to_string());
    }
    if document.document_id.trim().is_empty() {
        return Err("AUI document_id is required.".to_string());
    }
    if document.canvases.is_empty() {
        return Err("AUI document requires at least one canvas.".to_string());
    }
    let node_ids = document
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<BTreeSet<_>>();
    if node_ids.len() != document.nodes.len() {
        return Err("AUI document node_id values must be unique.".to_string());
    }
    for canvas in &document.canvases {
        if !node_ids.contains(canvas.root_node.as_str()) {
            return Err(format!(
                "AUI canvas '{}' references missing root node '{}'.",
                canvas.canvas_id, canvas.root_node
            ));
        }
    }
    for node in &document.nodes {
        if let Some(parent) = &node.parent {
            if !node_ids.contains(parent.as_str()) {
                return Err(format!(
                    "AUI node '{}' references missing parent '{}'.",
                    node.node_id, parent
                ));
            }
        }
        for child in &node.children {
            if !node_ids.contains(child.as_str()) {
                return Err(format!(
                    "AUI node '{}' references missing child '{}'.",
                    node.node_id, child
                ));
            }
        }
    }
    validate_interaction_feedback(document, diagnostics)?;
    Ok(())
}

fn validate_interaction_feedback(
    document: &AuiDocument,
    diagnostics: &mut Vec<AuiDocumentCookDiagnostic>,
) -> Result<(), String> {
    let mut invalid = false;
    let mut profile_ids = BTreeSet::new();

    if let Some(registry) = &document.interaction_feedback {
        if registry.motion_scale_permille > 2000 {
            invalid = true;
            diagnostics.push(AuiDocumentCookDiagnostic::error(
                "AuiFeedbackMotionScaleInvalid",
                format!(
                    "motion_scale_permille {} is outside 0..=2000.",
                    registry.motion_scale_permille
                ),
                Some("interaction_feedback.motion_scale_permille".to_string()),
                Some("Use a value between 0 and 2000.".to_string()),
            ));
        }
        for (index, profile) in registry.profiles.iter().enumerate() {
            let profile_path = format!("interaction_feedback.profiles[{index}]");
            let profile_id = profile.profile_id.trim();
            if profile_id.is_empty() || matches!(profile_id, "auto" | "none") {
                invalid = true;
                diagnostics.push(AuiDocumentCookDiagnostic::error(
                    "AuiFeedbackProfileIdInvalid",
                    format!(
                        "Feedback profile id '{}' is empty or reserved.",
                        profile.profile_id
                    ),
                    Some(format!("{profile_path}.profile_id")),
                    Some("Use a stable non-empty id other than auto or none.".to_string()),
                ));
            } else if !profile_ids.insert(profile_id.to_string()) {
                invalid = true;
                diagnostics.push(AuiDocumentCookDiagnostic::error(
                    "AuiFeedbackProfileDuplicate",
                    format!("Feedback profile id '{profile_id}' is duplicated."),
                    Some(format!("{profile_path}.profile_id")),
                    Some("Keep exactly one profile per profile_id.".to_string()),
                ));
            }
            invalid |= validate_feedback_profile(profile, &profile_path, diagnostics);
        }

        if let Some(default_profile) = registry.default_button_profile.as_deref() {
            if !profile_ids.contains(default_profile) {
                invalid = true;
                diagnostics.push(AuiDocumentCookDiagnostic::error(
                    "AuiFeedbackProfileMissing",
                    format!("Default Button feedback profile '{default_profile}' is missing."),
                    Some("interaction_feedback.default_button_profile".to_string()),
                    Some("Declare the profile or clear default_button_profile.".to_string()),
                ));
            }
        }
    }

    for node in &document.nodes {
        if let Some(profile_id) = node.feedback.profile_id() {
            if !profile_ids.contains(profile_id) {
                invalid = true;
                diagnostics.push(AuiDocumentCookDiagnostic::error(
                    "AuiFeedbackProfileMissing",
                    format!(
                        "AUI node '{}' references missing feedback profile '{}'.",
                        node.node_id, profile_id
                    ),
                    Some(format!("nodes.{}.feedback", node.node_id)),
                    Some("Declare the profile, use auto, or use none.".to_string()),
                ));
            }
        }
    }

    if invalid {
        Err("AUI interaction feedback schema validation failed.".to_string())
    } else {
        Ok(())
    }
}

fn validate_feedback_profile(
    profile: &AuiInteractionFeedbackProfile,
    path: &str,
    diagnostics: &mut Vec<AuiDocumentCookDiagnostic>,
) -> bool {
    let mut invalid = false;
    for (field, value) in [
        ("hover_scale_permille", profile.hover_scale_permille),
        ("pressed_scale_permille", profile.pressed_scale_permille),
        ("activated_scale_permille", profile.activated_scale_permille),
    ] {
        if !(500..=1500).contains(&value) {
            invalid = true;
            diagnostics.push(AuiDocumentCookDiagnostic::error(
                "AuiFeedbackProfileScaleInvalid",
                format!("{field} {value} is outside 500..=1500."),
                Some(format!("{path}.{field}")),
                Some("Use a scale between 500 and 1500 permille.".to_string()),
            ));
        }
    }
    for (field, value) in [
        ("hover_opacity_permille", profile.hover_opacity_permille),
        ("pressed_opacity_permille", profile.pressed_opacity_permille),
        (
            "activated_opacity_permille",
            profile.activated_opacity_permille,
        ),
        (
            "disabled_opacity_permille",
            profile.disabled_opacity_permille,
        ),
    ] {
        if value > 1000 {
            invalid = true;
            diagnostics.push(AuiDocumentCookDiagnostic::error(
                "AuiFeedbackProfileOpacityInvalid",
                format!("{field} {value} is outside 0..=1000."),
                Some(format!("{path}.{field}")),
                Some("Use an opacity between 0 and 1000 permille.".to_string()),
            ));
        }
    }
    for (field, value) in [
        (
            "hover_brightness_permille",
            profile.hover_brightness_permille,
        ),
        (
            "pressed_brightness_permille",
            profile.pressed_brightness_permille,
        ),
        (
            "activated_brightness_permille",
            profile.activated_brightness_permille,
        ),
    ] {
        if !(-1000..=1000).contains(&value) {
            invalid = true;
            diagnostics.push(AuiDocumentCookDiagnostic::error(
                "AuiFeedbackProfileBrightnessInvalid",
                format!("{field} {value} is outside -1000..=1000."),
                Some(format!("{path}.{field}")),
                Some("Use brightness between -1000 and 1000 permille.".to_string()),
            ));
        }
    }
    for (field, value) in [
        ("hover_in_ms", profile.hover_in_ms),
        ("hover_out_ms", profile.hover_out_ms),
        ("press_in_ms", profile.press_in_ms),
        ("release_ms", profile.release_ms),
        ("activated_ms", profile.activated_ms),
        ("cancel_ms", profile.cancel_ms),
    ] {
        if value > 5000 {
            invalid = true;
            diagnostics.push(AuiDocumentCookDiagnostic::error(
                "AuiFeedbackProfileDurationInvalid",
                format!("{field} {value}ms exceeds 5000ms."),
                Some(format!("{path}.{field}")),
                Some("Use a duration from 0 through 5000 milliseconds.".to_string()),
            ));
        }
    }
    if !profile.pressed_offset.x.is_finite()
        || !profile.pressed_offset.y.is_finite()
        || profile.pressed_offset.x.abs() > 2000.0
        || profile.pressed_offset.y.abs() > 2000.0
    {
        invalid = true;
        diagnostics.push(AuiDocumentCookDiagnostic::error(
            "AuiFeedbackProfileTranslationInvalid",
            "pressed_offset must be finite and within +/-2000 logical pixels.",
            Some(format!("{path}.pressed_offset")),
            Some("Use a small finite logical-pixel translation.".to_string()),
        ));
    }
    invalid
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn collect_asset_refs(value: &serde_json::Value) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    collect_asset_refs_recursive(value, &mut refs);
    refs
}

fn collect_asset_refs_recursive(value: &serde_json::Value, refs: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(id) = map.get("id").and_then(serde_json::Value::as_str) {
                if map
                    .get("type")
                    .or_else(|| map.get("assetType"))
                    .and_then(serde_json::Value::as_str)
                    .is_some()
                {
                    refs.insert(id.to_string());
                }
            }
            if let Some(id) = map.get("asset_id").and_then(serde_json::Value::as_str) {
                refs.insert(id.to_string());
            }
            for value in map.values() {
                collect_asset_refs_recursive(value, refs);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_asset_refs_recursive(value, refs);
            }
        }
        _ => {}
    }
}

fn sanitize_package_id(id: &str) -> String {
    let sanitized = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "aui-document".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod feedback_tests {
    use super::*;
    use engine_runtime::aui::{
        AuiCanvas, AuiFeedbackSelection, AuiInteractionFeedbackProfile,
        AuiInteractionFeedbackRegistry, AuiNode, AuiNodeKind, AuiRect,
        LEGACY_AUI_DOCUMENT_SCHEMA_VERSION,
    };

    fn feedback_document() -> AuiDocument {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["button"]);
        let button = AuiNode::new(
            "button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(10.0, 20.0, 160.0, 48.0),
        )
        .with_parent("root")
        .with_interactable(true);
        AuiDocument::new(
            "feedback-doc",
            vec![AuiCanvas::screen_overlay("canvas", 1080.0, 1920.0, "root")],
            vec![root, button],
        )
    }

    #[test]
    fn aui_document_feedback_v1_migrates_to_v2_auto() {
        let mut document = feedback_document();
        document.schema_version = LEGACY_AUI_DOCUMENT_SCHEMA_VERSION.to_string();
        let mut value = serde_json::to_value(document).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("interaction_feedback");
        for node in value
            .get_mut("nodes")
            .and_then(serde_json::Value::as_array_mut)
            .unwrap()
        {
            node.as_object_mut().unwrap().remove("feedback");
        }

        let output = AuiDocumentCooker::cook(AuiDocumentCookRequest {
            source_path: PathBuf::from("Project/AUI/feedback-v1.aui.json"),
            document: value,
        })
        .expect("v1 document should migrate");

        assert_eq!(output.document.schema_version, AUI_DOCUMENT_SCHEMA_VERSION);
        assert_eq!(
            output.report.source_schema_version.as_deref(),
            Some(LEGACY_AUI_DOCUMENT_SCHEMA_VERSION)
        );
        assert_eq!(
            output.report.normalized_schema_version.as_deref(),
            Some(AUI_DOCUMENT_SCHEMA_VERSION)
        );
        assert_eq!(
            output.report.feedback_fallback_identity.as_deref(),
            Some(AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID)
        );
        assert!(output
            .document
            .nodes
            .iter()
            .all(|node| node.feedback == AuiFeedbackSelection::auto()));
        assert!(output
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AuiDocumentV1MigratedToV2"));
    }

    #[test]
    fn aui_document_feedback_profiles_sort_and_round_trip() {
        let mut document = feedback_document();
        document.interaction_feedback = Some(AuiInteractionFeedbackRegistry {
            motion_scale_permille: 1000,
            default_button_profile: Some("ink.a".to_string()),
            profiles: vec![
                AuiInteractionFeedbackProfile::new("ink.z"),
                AuiInteractionFeedbackProfile::new("ink.a"),
            ],
        });
        document.nodes[1].feedback = AuiFeedbackSelection::profile("ink.a");

        let output = AuiDocumentCooker::cook(AuiDocumentCookRequest {
            source_path: PathBuf::from("Project/AUI/feedback-v2.aui.json"),
            document: serde_json::to_value(document).unwrap(),
        })
        .expect("valid v2 feedback document should cook");

        let profiles = &output
            .document
            .interaction_feedback
            .as_ref()
            .unwrap()
            .profiles;
        assert_eq!(profiles[0].profile_id, "ink.a");
        assert_eq!(profiles[1].profile_id, "ink.z");
        assert_eq!(
            output.report.feedback_fallback_identity.as_deref(),
            Some("ink.a")
        );
        let round_trip: AuiDocument =
            serde_json::from_slice(&serde_json::to_vec(&output.document).unwrap()).unwrap();
        assert_eq!(round_trip, output.document);
    }

    #[test]
    fn aui_document_feedback_rejects_duplicate_and_missing_profile() {
        let mut document = feedback_document();
        document.interaction_feedback = Some(AuiInteractionFeedbackRegistry {
            motion_scale_permille: 1000,
            default_button_profile: None,
            profiles: vec![
                AuiInteractionFeedbackProfile::new("ink.duplicate"),
                AuiInteractionFeedbackProfile::new("ink.duplicate"),
            ],
        });
        document.nodes[1].feedback = AuiFeedbackSelection::profile("ink.missing");

        let report = AuiDocumentCooker::cook(AuiDocumentCookRequest {
            source_path: PathBuf::from("Project/AUI/feedback-invalid.aui.json"),
            document: serde_json::to_value(document).unwrap(),
        })
        .expect_err("invalid feedback document must fail");

        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AuiFeedbackProfileDuplicate"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AuiFeedbackProfileMissing"));
    }

    #[test]
    fn aui_document_feedback_rejects_invalid_ranges_and_easing() {
        let mut document = feedback_document();
        let mut profile = AuiInteractionFeedbackProfile::new("ink.invalid");
        profile.hover_scale_permille = 1600;
        profile.pressed_opacity_permille = 1001;
        profile.activated_ms = 5001;
        document.interaction_feedback = Some(AuiInteractionFeedbackRegistry {
            motion_scale_permille: 2001,
            default_button_profile: Some("ink.invalid".to_string()),
            profiles: vec![profile],
        });

        let report = AuiDocumentCooker::cook(AuiDocumentCookRequest {
            source_path: PathBuf::from("Project/AUI/feedback-ranges.aui.json"),
            document: serde_json::to_value(document).unwrap(),
        })
        .expect_err("out-of-range feedback values must fail");

        for code in [
            "AuiFeedbackMotionScaleInvalid",
            "AuiFeedbackProfileScaleInvalid",
            "AuiFeedbackProfileOpacityInvalid",
            "AuiFeedbackProfileDurationInvalid",
        ] {
            assert!(report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code));
        }

        let mut value = serde_json::to_value(feedback_document()).unwrap();
        value["interaction_feedback"] = serde_json::json!({
            "profiles": [{
                "profile_id": "ink.bad-easing",
                "hover_easing": "springForever"
            }]
        });
        let report = AuiDocumentCooker::cook(AuiDocumentCookRequest {
            source_path: PathBuf::from("Project/AUI/feedback-easing.aui.json"),
            document: value,
        })
        .expect_err("unsupported easing must fail");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AuiFeedbackEasingInvalid"));
    }
}
