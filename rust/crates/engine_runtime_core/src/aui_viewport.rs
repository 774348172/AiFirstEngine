use crate::aui::{AuiComputedRect, AuiDocument, AuiScaleMode};
use crate::aui_layout_schema::LayoutVisibility;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const AUI_VIEWPORT_LAYOUT_SCHEMA_VERSION: &str = "aui-viewport-layout.v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiViewportInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl AuiViewportInsets {
    pub const fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    fn is_finite_nonnegative(self) -> bool {
        [self.left, self.right, self.top, self.bottom]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
    }

    fn logical(self, scale_factor: f32) -> Self {
        Self::new(
            self.left / scale_factor,
            self.right / scale_factor,
            self.top / scale_factor,
            self.bottom / scale_factor,
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiSafeAreaPolicy {
    #[default]
    Ignore,
    InsetRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuiViewportOrientation {
    Landscape,
    Portrait,
}

impl AuiViewportOrientation {
    fn from_size(width: f32, height: f32) -> Self {
        if height > width {
            Self::Portrait
        } else {
            Self::Landscape
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiViewportCondition {
    pub condition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<AuiViewportOrientation>,
}

impl AuiViewportCondition {
    pub fn new(condition_id: impl Into<String>) -> Self {
        Self {
            condition_id: condition_id.into(),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            orientation: None,
        }
    }

    pub fn with_min_width(mut self, value: f32) -> Self {
        self.min_width = Some(value);
        self
    }

    pub fn with_max_width(mut self, value: f32) -> Self {
        self.max_width = Some(value);
        self
    }

    pub fn with_min_height(mut self, value: f32) -> Self {
        self.min_height = Some(value);
        self
    }

    pub fn with_max_height(mut self, value: f32) -> Self {
        self.max_height = Some(value);
        self
    }

    pub fn with_orientation(mut self, value: AuiViewportOrientation) -> Self {
        self.orientation = Some(value);
        self
    }

    fn matches(&self, width: f32, height: f32, orientation: AuiViewportOrientation) -> bool {
        self.min_width.is_none_or(|value| width >= value)
            && self.max_width.is_none_or(|value| width <= value)
            && self.min_height.is_none_or(|value| height >= value)
            && self.max_height.is_none_or(|value| height <= value)
            && self.orientation.is_none_or(|value| value == orientation)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiViewportLayoutSpec {
    pub safe_area_policy: AuiSafeAreaPolicy,
    pub conditions: Vec<AuiViewportCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiViewportInput {
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
    #[serde(default)]
    pub physical_safe_insets: AuiViewportInsets,
}

impl AuiViewportInput {
    pub const fn new(physical_width: u32, physical_height: u32, scale_factor: f64) -> Self {
        Self {
            physical_width,
            physical_height,
            scale_factor,
            physical_safe_insets: AuiViewportInsets::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub const fn with_physical_safe_insets(mut self, insets: AuiViewportInsets) -> Self {
        self.physical_safe_insets = insets;
        self
    }

    pub fn compact_identity(&self) -> Result<String, AuiViewportError> {
        let normalized = normalize_input(self)?;
        Ok(format!(
            "{}x{}@{:016x}:{:08x}:{:08x}:{:08x}:{:08x}",
            self.physical_width,
            self.physical_height,
            self.scale_factor.to_bits(),
            normalized.logical_safe_insets.left.to_bits(),
            normalized.logical_safe_insets.right.to_bits(),
            normalized.logical_safe_insets.top.to_bits(),
            normalized.logical_safe_insets.bottom.to_bits(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedCanvasViewport {
    pub canvas_id: String,
    pub logical_viewport: AuiComputedRect,
    pub safe_rect: AuiComputedRect,
    pub logical_safe_insets: AuiViewportInsets,
    pub orientation: AuiViewportOrientation,
    pub matched_condition_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiViewportResolution {
    pub schema_version: String,
    pub input_identity: String,
    pub document: AuiDocument,
    pub canvases: Vec<AuiResolvedCanvasViewport>,
}

impl AuiViewportResolution {
    pub fn canvas(&self, canvas_id: &str) -> Option<&AuiResolvedCanvasViewport> {
        self.canvases
            .iter()
            .find(|canvas| canvas.canvas_id == canvas_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiViewportDiagnostic {
    pub code: String,
    pub canvas_id: Option<String>,
    pub node_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiViewportError {
    pub diagnostics: Vec<AuiViewportDiagnostic>,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedInput {
    logical_viewport: AuiComputedRect,
    safe_rect: AuiComputedRect,
    logical_safe_insets: AuiViewportInsets,
    orientation: AuiViewportOrientation,
}

pub struct AuiViewportResolver;

impl AuiViewportResolver {
    pub fn requires_viewport(document: &AuiDocument) -> bool {
        document
            .canvases
            .iter()
            .any(|canvas| canvas.scale_mode == AuiScaleMode::ViewportLayout)
            || document
                .nodes
                .iter()
                .any(|node| node.viewport_condition.is_some())
    }

    pub fn validate_contract(document: &AuiDocument) -> Vec<AuiViewportDiagnostic> {
        let mut diagnostics = Vec::new();
        let node_by_id = document
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.node_id.as_str(), index))
            .collect::<HashMap<_, _>>();
        let mut responsive_nodes = HashSet::new();
        let mut responsive_node_owners = HashMap::<usize, String>::new();

        for canvas in &document.canvases {
            let Some(spec) = &canvas.viewport_layout else {
                if canvas.scale_mode == AuiScaleMode::ViewportLayout {
                    diagnostics.push(diagnostic(
                        "aui.viewport.layout_spec_required",
                        Some(&canvas.canvas_id),
                        None,
                        "ViewportLayout canvas requires viewportLayout.",
                    ));
                }
                continue;
            };
            if canvas.scale_mode != AuiScaleMode::ViewportLayout {
                diagnostics.push(diagnostic(
                    "aui.viewport.layout_spec_without_mode",
                    Some(&canvas.canvas_id),
                    None,
                    "viewportLayout is only valid with AuiScaleMode::ViewportLayout.",
                ));
            }
            let mut condition_ids = HashSet::new();
            for condition in &spec.conditions {
                validate_condition(condition, &canvas.canvas_id, &mut diagnostics);
                if !condition_ids.insert(condition.condition_id.as_str()) {
                    diagnostics.push(diagnostic(
                        "aui.viewport.condition_duplicate",
                        Some(&canvas.canvas_id),
                        None,
                        format!(
                            "Viewport condition '{}' is duplicated.",
                            condition.condition_id
                        ),
                    ));
                }
            }
            let Some(&root_index) = node_by_id.get(canvas.root_node.as_str()) else {
                continue;
            };
            let mut pending = vec![root_index];
            let mut seen = HashSet::new();
            while let Some(index) = pending.pop() {
                if !seen.insert(index) {
                    continue;
                }
                let node = &document.nodes[index];
                if let Some(condition_id) = &node.viewport_condition {
                    responsive_nodes.insert(index);
                    if let Some(owner) = responsive_node_owners.get(&index) {
                        if owner != &canvas.canvas_id {
                            diagnostics.push(diagnostic(
                                "aui.viewport.condition_multiple_canvas_owners",
                                Some(&canvas.canvas_id),
                                Some(&node.node_id),
                                format!(
                                    "Responsive node is owned by both viewport canvases '{}' and '{}'.",
                                    owner, canvas.canvas_id
                                ),
                            ));
                        }
                    } else {
                        responsive_node_owners.insert(index, canvas.canvas_id.clone());
                    }
                    if !condition_ids.contains(condition_id.as_str()) {
                        diagnostics.push(diagnostic(
                            "aui.viewport.condition_missing",
                            Some(&canvas.canvas_id),
                            Some(&node.node_id),
                            format!("Node references missing viewport condition '{condition_id}'."),
                        ));
                    }
                }
                pending.extend(
                    node.children
                        .iter()
                        .filter_map(|child| node_by_id.get(child.as_str()).copied()),
                );
            }
        }

        for (index, node) in document.nodes.iter().enumerate() {
            if node.viewport_condition.is_some() && !responsive_nodes.contains(&index) {
                diagnostics.push(diagnostic(
                    "aui.viewport.condition_without_viewport_canvas",
                    None,
                    Some(&node.node_id),
                    "Responsive node is not owned by a ViewportLayout canvas.",
                ));
            }
        }
        diagnostics
    }

    pub fn resolve_document(
        document: &AuiDocument,
        input: &AuiViewportInput,
    ) -> Result<AuiViewportResolution, AuiViewportError> {
        let contract_diagnostics = Self::validate_contract(document);
        if !contract_diagnostics.is_empty() {
            return Err(AuiViewportError {
                diagnostics: contract_diagnostics,
            });
        }
        if !Self::requires_viewport(document) {
            return Ok(AuiViewportResolution {
                schema_version: AUI_VIEWPORT_LAYOUT_SCHEMA_VERSION.to_string(),
                input_identity: "reference-canvas".to_string(),
                document: document.clone(),
                canvases: Vec::new(),
            });
        }
        let normalized = normalize_input(input)?;
        let mut resolved = document.clone();
        let mut canvases = Vec::new();
        let node_by_id = resolved
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.node_id.clone(), index))
            .collect::<HashMap<_, _>>();

        for canvas_index in 0..resolved.canvases.len() {
            if resolved.canvases[canvas_index].scale_mode != AuiScaleMode::ViewportLayout {
                continue;
            }
            let canvas_id = resolved.canvases[canvas_index].canvas_id.clone();
            let root_node = resolved.canvases[canvas_index].root_node.clone();
            let spec = resolved.canvases[canvas_index]
                .viewport_layout
                .clone()
                .expect("validated viewport spec");
            let matched_condition_ids = spec
                .conditions
                .iter()
                .filter(|condition| {
                    condition.matches(
                        normalized.logical_viewport.width,
                        normalized.logical_viewport.height,
                        normalized.orientation,
                    )
                })
                .map(|condition| condition.condition_id.clone())
                .collect::<Vec<_>>();
            let matched = matched_condition_ids
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            resolved.canvases[canvas_index].reference_resolution.x =
                normalized.logical_viewport.width;
            resolved.canvases[canvas_index].reference_resolution.y =
                normalized.logical_viewport.height;
            resolved.canvases[canvas_index].resolved_viewport_root_rect =
                Some(if spec.safe_area_policy == AuiSafeAreaPolicy::InsetRoot {
                    normalized.safe_rect
                } else {
                    normalized.logical_viewport
                });

            let mut pending = vec![node_by_id[&root_node]];
            let mut seen = HashSet::new();
            while let Some(index) = pending.pop() {
                if !seen.insert(index) {
                    continue;
                }
                let children = resolved.nodes[index].children.clone();
                let node = &mut resolved.nodes[index];
                let original = node
                    .resolved_viewport_original_visibility
                    .unwrap_or(node.layout_visibility);
                node.resolved_viewport_original_visibility = Some(original);
                if node
                    .viewport_condition
                    .as_deref()
                    .is_some_and(|condition| !matched.contains(condition))
                {
                    node.layout_visibility = Some(LayoutVisibility::Collapsed);
                } else {
                    node.layout_visibility = original;
                }
                pending.extend(
                    children
                        .iter()
                        .filter_map(|child| node_by_id.get(child).copied()),
                );
            }
            canvases.push(AuiResolvedCanvasViewport {
                canvas_id,
                logical_viewport: normalized.logical_viewport,
                safe_rect: if spec.safe_area_policy == AuiSafeAreaPolicy::InsetRoot {
                    normalized.safe_rect
                } else {
                    normalized.logical_viewport
                },
                logical_safe_insets: normalized.logical_safe_insets,
                orientation: normalized.orientation,
                matched_condition_ids,
            });
        }

        Ok(AuiViewportResolution {
            schema_version: AUI_VIEWPORT_LAYOUT_SCHEMA_VERSION.to_string(),
            input_identity: input.compact_identity()?,
            document: resolved,
            canvases,
        })
    }
}

fn normalize_input(input: &AuiViewportInput) -> Result<NormalizedInput, AuiViewportError> {
    let mut diagnostics = Vec::new();
    if input.physical_width == 0 || input.physical_height == 0 {
        diagnostics.push(diagnostic(
            "aui.viewport.extent_invalid",
            None,
            None,
            "Viewport physical extent must be non-zero.",
        ));
    }
    if !input.scale_factor.is_finite()
        || input.scale_factor <= 0.0
        || input.scale_factor > f64::from(f32::MAX)
    {
        diagnostics.push(diagnostic(
            "aui.viewport.scale_factor_invalid",
            None,
            None,
            "Viewport scale factor must be finite and positive.",
        ));
    }
    if !input.physical_safe_insets.is_finite_nonnegative() {
        diagnostics.push(diagnostic(
            "aui.viewport.safe_area_invalid",
            None,
            None,
            "Viewport safe insets must be finite and non-negative.",
        ));
    }
    if !diagnostics.is_empty() {
        return Err(AuiViewportError { diagnostics });
    }

    let scale_factor = input.scale_factor as f32;
    let width = input.physical_width as f32 / scale_factor;
    let height = input.physical_height as f32 / scale_factor;
    let logical_safe_insets = input.physical_safe_insets.logical(scale_factor);
    let safe_width = width - logical_safe_insets.left - logical_safe_insets.right;
    let safe_height = height - logical_safe_insets.top - logical_safe_insets.bottom;
    if !width.is_finite()
        || !height.is_finite()
        || safe_width <= 0.0
        || safe_height <= 0.0
        || !safe_width.is_finite()
        || !safe_height.is_finite()
    {
        return Err(AuiViewportError {
            diagnostics: vec![diagnostic(
                "aui.viewport.safe_area_invalid",
                None,
                None,
                "Viewport safe insets must leave a finite positive logical area.",
            )],
        });
    }
    Ok(NormalizedInput {
        logical_viewport: AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
        safe_rect: AuiComputedRect {
            x: logical_safe_insets.left,
            y: logical_safe_insets.top,
            width: safe_width,
            height: safe_height,
        },
        logical_safe_insets,
        orientation: AuiViewportOrientation::from_size(width, height),
    })
}

fn validate_condition(
    condition: &AuiViewportCondition,
    canvas_id: &str,
    diagnostics: &mut Vec<AuiViewportDiagnostic>,
) {
    let values = [
        condition.min_width,
        condition.max_width,
        condition.min_height,
        condition.max_height,
    ];
    if condition.condition_id.trim().is_empty() {
        diagnostics.push(diagnostic(
            "aui.viewport.condition_id_required",
            Some(canvas_id),
            None,
            "Viewport condition id is required.",
        ));
    }
    if values
        .into_iter()
        .flatten()
        .any(|value| !value.is_finite() || value < 0.0)
        || matches!((condition.min_width, condition.max_width), (Some(min), Some(max)) if min > max)
        || matches!((condition.min_height, condition.max_height), (Some(min), Some(max)) if min > max)
    {
        diagnostics.push(diagnostic(
            "aui.viewport.condition_range_invalid",
            Some(canvas_id),
            None,
            format!(
                "Viewport condition '{}' has an invalid range.",
                condition.condition_id
            ),
        ));
    }
    let impossible = match condition.orientation {
        Some(AuiViewportOrientation::Landscape) => matches!(
            (condition.max_width, condition.min_height),
            (Some(max_width), Some(min_height)) if max_width < min_height
        ),
        Some(AuiViewportOrientation::Portrait) => matches!(
            (condition.max_height, condition.min_width),
            (Some(max_height), Some(min_width)) if max_height <= min_width
        ),
        None => false,
    };
    if impossible {
        diagnostics.push(diagnostic(
            "aui.viewport.condition_impossible",
            Some(canvas_id),
            None,
            format!(
                "Viewport condition '{}' cannot satisfy its orientation and ranges.",
                condition.condition_id
            ),
        ));
    }
}

fn diagnostic(
    code: impl Into<String>,
    canvas_id: Option<&str>,
    node_id: Option<&str>,
    message: impl Into<String>,
) -> AuiViewportDiagnostic {
    AuiViewportDiagnostic {
        code: code.into(),
        canvas_id: canvas_id.map(str::to_string),
        node_id: node_id.map(str::to_string),
        message: message.into(),
    }
}
