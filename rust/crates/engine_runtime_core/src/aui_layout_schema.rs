//! Versioned C2 authoring contract. No solver-specific types cross this boundary.
use crate::aui::{AuiDocument, AuiFontStyle, AuiValidationItem, AUI_DOCUMENT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use unicode_segmentation::UnicodeSegmentation;

pub const AUI_LAYOUT_SCHEMA_VERSION: &str = "aui-document.v3";
pub const MAX_LAYOUT_NODES: usize = 65_536;
pub const MAX_LAYOUT_DEPTH: usize = 128;

macro_rules! choice {
    ($name:ident, $default:ident $(, $variant:ident)* $(,)?) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name { #[default] $default, $($variant),* }
    };
}
choice!(LayoutMode, Anchor, Row, Column, Flex, Grid, Overlay);
choice!(FlexDirection, Row, RowReverse, Column, ColumnReverse);
choice!(FlexWrap, NoWrap, Wrap, WrapReverse);
choice!(
    JustifyContent,
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly
);
choice!(AlignItems, Stretch, Start, End, Center, Baseline);
choice!(
    AlignContent,
    Stretch,
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly
);
choice!(GridAutoFlow, Row, Column);
choice!(Participation, Flow, OutOfFlow);
choice!(LayoutVisibility, Visible, Hidden, Collapsed);
choice!(Overflow, Visible, Clip, Scroll);
choice!(TextWrap, None, Word, GraphemeFallback);
choice!(TextAlign, Start, Center, End, Justify);
choice!(VerticalAlign, Start, Center, End);
choice!(TextOverflow, Visible, Clip, Ellipsis);
choice!(TextDirection, Auto, Ltr, Rtl);

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Dimension {
    #[default]
    Content,
    Fixed {
        value: f32,
    },
    Fill,
    Percent {
        value: f32,
    },
}

impl<'de> Deserialize<'de> for Dimension {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
        enum Strict {
            Content {},
            Fixed { value: f32 },
            Fill {},
            Percent { value: f32 },
        }
        Ok(match Strict::deserialize(deserializer)? {
            Strict::Content {} => Self::Content,
            Strict::Fixed { value } => Self::Fixed { value },
            Strict::Fill {} => Self::Fill,
            Strict::Percent { value } => Self::Percent { value },
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum GridTrack {
    #[default]
    Content,
    Fixed {
        value: f32,
    },
    MinContent,
    MaxContent,
    Fr {
        value: f32,
    },
    Minmax {
        min: Box<GridTrack>,
        max: Box<GridTrack>,
    },
}

impl<'de> Deserialize<'de> for GridTrack {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
        enum Strict {
            Content {},
            Fixed {
                value: f32,
            },
            MinContent {},
            MaxContent {},
            Fr {
                value: f32,
            },
            Minmax {
                min: Box<GridTrack>,
                max: Box<GridTrack>,
            },
        }
        Ok(match Strict::deserialize(deserializer)? {
            Strict::Content {} => Self::Content,
            Strict::Fixed { value } => Self::Fixed { value },
            Strict::MinContent {} => Self::MinContent,
            Strict::MaxContent {} => Self::MaxContent,
            Strict::Fr { value } => Self::Fr { value },
            Strict::Minmax { min, max } => Self::Minmax { min, max },
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Gap {
    pub column: f32,
    pub row: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutSpec {
    pub mode: LayoutMode,
    pub padding: Insets,
    pub gap: Gap,
    pub direction: FlexDirection,
    pub wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub grid_columns: Vec<GridTrack>,
    pub grid_rows: Vec<GridTrack>,
    pub grid_auto_flow: GridAutoFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct GridPlacement {
    pub start: Option<u16>,
    pub span: u16,
}
impl Default for GridPlacement {
    fn default() -> Self {
        Self {
            start: None,
            span: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct SlotSpec {
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub margin: Insets,
    pub flex_basis: Dimension,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    #[serde(
        deserialize_with = "deserialize_align_self",
        skip_serializing_if = "Option::is_none"
    )]
    pub align_self: Option<AlignItems>,
    pub grid_column: GridPlacement,
    pub grid_row: GridPlacement,
    pub participation: Participation,
}
fn deserialize_align_self<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AlignItems>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_null() || value.as_str() == Some("inherit") {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}
impl Default for SlotSpec {
    fn default() -> Self {
        Self {
            width: Dimension::Content,
            height: Dimension::Content,
            min_width: 0.0,
            min_height: 0.0,
            max_width: None,
            max_height: None,
            margin: Insets::default(),
            flex_basis: Dimension::Content,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            align_self: None,
            grid_column: GridPlacement::default(),
            grid_row: GridPlacement::default(),
            participation: Participation::Flow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LineHeight {
    Multiple { value: f32 },
    Fixed { value: f32 },
}
impl Default for LineHeight {
    fn default() -> Self {
        Self::Multiple { value: 1.0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct TextLayoutSpec {
    pub wrap: TextWrap,
    pub align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub line_height: LineHeight,
    pub max_lines: Option<u32>,
    pub overflow: TextOverflow,
    pub direction: TextDirection,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiRichTextSpan {
    /// UTF-8 byte offsets into `AuiNode.text`.
    pub start: u32,
    pub end: u32,
    pub color: Option<String>,
    pub font_size: Option<f32>,
    pub font: Option<AuiFontStyle>,
}

pub fn uses_c2(document: &AuiDocument) -> bool {
    document.nodes.iter().any(|node| {
        node.layout.is_some()
            || node.slot.is_some()
            || node.text_layout.is_some()
            || !node.text_spans.is_empty()
            || node.layout_visibility.is_some()
            || node.overflow.is_some()
            || node.theme.is_some()
            || node.style_ref.is_some()
            || node.variant.is_some()
            || node.style_overrides.is_some()
            || node.clip_radius.is_some()
            || node.viewport_condition.is_some()
            || node.toggle_checked.is_some()
            || node.slider_value.is_some()
            || node.slider_min.is_some()
            || node.slider_max.is_some()
            || node.slider_step.is_some()
    }) || document.theme.is_some()
        || !document.virtual_lists.is_empty()
        || document
            .canvases
            .iter()
            .any(|canvas| canvas.viewport_layout.is_some())
}

pub fn supported_schema(schema: &str) -> bool {
    matches!(
        schema,
        AUI_DOCUMENT_SCHEMA_VERSION | AUI_LAYOUT_SCHEMA_VERSION
    )
}

/// Source normalization must never rewrite a future version or discard typed fields.
pub fn normalize_source_version(document: &mut AuiDocument) -> Result<(), String> {
    match document.schema_version.as_str() {
        "" | "aui-document.v1" | AUI_DOCUMENT_SCHEMA_VERSION => {
            document.schema_version = if uses_c2(document) {
                AUI_LAYOUT_SCHEMA_VERSION
            } else {
                AUI_DOCUMENT_SCHEMA_VERSION
            }
            .into();
            Ok(())
        }
        AUI_LAYOUT_SCHEMA_VERSION => Ok(()),
        other => Err(format!("Unsupported AUI schema '{other}'")),
    }
}

/// Legacy tree conversion has no typed C2 mapping. Fail instead of dropping fields.
pub fn validate_legacy_source(value: &serde_json::Value) -> Result<(), String> {
    if let Some(version) = value
        .get("schemaVersion")
        .or_else(|| value.get("schema_version"))
        .and_then(serde_json::Value::as_str)
    {
        if version.starts_with("aui-document.v")
            && version != "aui-document.v1"
            && version != AUI_DOCUMENT_SCHEMA_VERSION
        {
            return Err(format!(
                "AUI schema '{version}' requires canonical document shape"
            ));
        }
    }
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if [
                    "layout",
                    "slot",
                    "textLayout",
                    "textSpans",
                    "layoutVisibility",
                    "overflow",
                    "viewportLayout",
                    "viewportCondition",
                    "toggleChecked",
                    "toggle_checked",
                    "sliderValue",
                    "slider_value",
                    "sliderMin",
                    "slider_min",
                    "sliderMax",
                    "slider_max",
                    "sliderStep",
                    "slider_step",
                    "virtualLists",
                ]
                .iter()
                .any(|key| object.contains_key(*key))
                {
                    return Err("C2 fields require canonical AuiDocument shape; legacy conversion would lose layout semantics".into());
                }
                pending.extend(object.values());
            }
            serde_json::Value::Array(array) => pending.extend(array),
            _ => {}
        }
    }
    Ok(())
}

fn error(items: &mut Vec<AuiValidationItem>, node: Option<&str>, code: &str, path: &str) {
    items.push(AuiValidationItem::error(
        code,
        node.map(str::to_owned),
        None,
        format!("Invalid C2 layout at {path}"),
        "Use finite, nonnegative values and an acyclic, consistent layout tree.",
    ));
}
fn finite(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}
fn insets(value: Insets) -> bool {
    [value.left, value.right, value.top, value.bottom]
        .into_iter()
        .all(finite)
}
fn dimension(value: Dimension) -> bool {
    match value {
        Dimension::Fixed { value } | Dimension::Percent { value } => finite(value),
        _ => true,
    }
}
fn track(value: &GridTrack, nested: bool) -> bool {
    match value {
        GridTrack::Fixed { value } => finite(*value),
        GridTrack::Fr { value } => finite(*value) && *value > 0.0,
        GridTrack::Minmax { min, max } => {
            !nested
                && !matches!(**min, GridTrack::Fr { .. })
                && track(min, true)
                && track(max, true)
                && !matches!((&**min,&**max), (GridTrack::Fixed { value:a },GridTrack::Fixed { value:b }) if a>b)
        }
        _ => true,
    }
}

pub fn validate(document: &AuiDocument) -> Vec<AuiValidationItem> {
    let mut items = Vec::new();
    if !supported_schema(&document.schema_version) {
        error(&mut items, None, "aui_schema_unsupported", "schema_version");
    }
    if !uses_c2(document) && document.schema_version != AUI_LAYOUT_SCHEMA_VERSION {
        return items;
    }
    if document.schema_version != AUI_LAYOUT_SCHEMA_VERSION {
        error(&mut items, None, "aui_layout_requires_v3", "schema_version");
    }
    if document.nodes.len() > MAX_LAYOUT_NODES {
        error(&mut items, None, "aui_layout_node_budget", "nodes");
        return items;
    }
    if document.nodes.is_empty() || document.canvases.is_empty() {
        error(
            &mut items,
            None,
            "aui_layout_empty_document",
            "nodes/canvases",
        );
    }
    for (index, node) in document.nodes.iter().enumerate() {
        let id = Some(node.node_id.as_str());
        let path = format!("nodes[{index}]");
        if node.presentation.is_some_and(|p| p.validate().is_err()) {
            error(
                &mut items,
                id,
                "aui_presentation_invalid",
                &format!("{path}.presentation"),
            );
        }
        if let Some(layout) = &node.layout {
            if !insets(layout.padding)
                || !finite(layout.gap.row)
                || !finite(layout.gap.column)
                || !layout
                    .grid_columns
                    .iter()
                    .chain(&layout.grid_rows)
                    .all(|t| track(t, false))
            {
                error(
                    &mut items,
                    id,
                    "aui_layout_metrics_invalid",
                    &format!("{path}.layout"),
                );
            }
        }
        if let Some(slot) = &node.slot {
            let minmax =
                |min, max: Option<f32>| finite(min) && max.is_none_or(|v| finite(v) && v >= min);
            let placement = |p: GridPlacement| {
                p.span > 0
                    && p.start.is_none_or(|v| {
                        v > 0 && u32::from(v) + u32::from(p.span) <= u32::from(u16::MAX)
                    })
            };
            if !dimension(slot.width)
                || !dimension(slot.height)
                || !dimension(slot.flex_basis)
                || matches!(slot.flex_basis, Dimension::Fill)
                || !insets(slot.margin)
                || !minmax(slot.min_width, slot.max_width)
                || !minmax(slot.min_height, slot.max_height)
                || !finite(slot.flex_grow)
                || !finite(slot.flex_shrink)
                || !placement(slot.grid_column)
                || !placement(slot.grid_row)
            {
                error(&mut items, id, "aui_slot_invalid", &format!("{path}.slot"));
            }
        }
        if node.text.as_ref().is_some_and(|text| text.len() > 65_536) {
            error(
                &mut items,
                id,
                "aui_text_layout_invalid",
                &format!("{path}.text"),
            );
        }
        if !node.text_spans.is_empty() {
            let valid = node.text.as_ref().is_some_and(|text| {
                let mut boundaries = text
                    .grapheme_indices(true)
                    .map(|(offset, _)| offset)
                    .collect::<Vec<_>>();
                boundaries.push(text.len());
                let mut previous_end = 0usize;
                node.text_spans.iter().all(|span| {
                    let start = span.start as usize;
                    let end = span.end as usize;
                    let ordered = start >= previous_end;
                    previous_end = end;
                    ordered
                        && start < end
                        && end <= text.len()
                        && text.is_char_boundary(start)
                        && text.is_char_boundary(end)
                        && boundaries.binary_search(&start).is_ok()
                        && boundaries.binary_search(&end).is_ok()
                        && span
                            .font_size
                            .is_none_or(|size| size.is_finite() && size > 0.0)
                        && span.color.as_deref().is_none_or(valid_color)
                        && span
                            .font
                            .as_ref()
                            .is_none_or(|font| (1..=1000).contains(&font.weight))
                })
            });
            if !valid {
                error(
                    &mut items,
                    id,
                    "aui_rich_text_span_invalid",
                    &format!("{path}.textSpans"),
                );
            }
        }
        if let Some(text) = &node.text_layout {
            let value = match text.line_height {
                LineHeight::Multiple { value } | LineHeight::Fixed { value } => value,
            };
            if !value.is_finite()
                || value <= 0.0
                || text.max_lines.is_some_and(|v| v == 0 || v > 4096)
            {
                error(
                    &mut items,
                    id,
                    "aui_text_layout_invalid",
                    &format!("{path}.textLayout"),
                );
            }
        }
        let has_toggle_value = node.toggle_checked.is_some();
        let has_slider_value = node.slider_value.is_some()
            || node.slider_min.is_some()
            || node.slider_max.is_some()
            || node.slider_step.is_some();
        let slider_valid = if node.kind == crate::aui::AuiNodeKind::Slider {
            let min = node.slider_min.unwrap_or(0.0);
            let max = node.slider_max.unwrap_or(1.0);
            let step = node.slider_step.unwrap_or(0.1);
            let value = node.slider_value.unwrap_or(min);
            min.is_finite()
                && max.is_finite()
                && step.is_finite()
                && value.is_finite()
                && min < max
                && step > 0.0
                && value >= min
                && value <= max
        } else {
            !has_slider_value
        };
        if (has_toggle_value && node.kind != crate::aui::AuiNodeKind::Toggle) || !slider_valid {
            error(
                &mut items,
                id,
                "aui_control_value_invalid",
                &format!("{path}.controlValue"),
            );
        }
    }
    let nodes_by_id = document
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let mut list_nodes = HashSet::new();
    let mut data_paths = HashSet::new();
    for (index, declaration) in document.virtual_lists.iter().enumerate() {
        let path = format!("virtualLists[{index}]");
        let list_node = nodes_by_id.get(declaration.list_node_id.as_str()).copied();
        let template = nodes_by_id
            .get(declaration.item_template_node_id.as_str())
            .copied();
        let extent_valid = match declaration.extent_mode {
            crate::aui_virtual_list::AuiVirtualListExtentMode::Fixed { item_extent } => {
                item_extent.is_finite() && item_extent > 0.0
            }
            crate::aui_virtual_list::AuiVirtualListExtentMode::Variable {
                estimated_item_extent,
            } => estimated_item_extent.is_finite() && estimated_item_extent > 0.0,
        };
        let valid = !declaration.data_path.trim().is_empty()
            && list_node.is_some_and(|node| node.kind == crate::aui::AuiNodeKind::List)
            && template.is_some_and(|node| {
                node.parent.as_deref() == Some(declaration.list_node_id.as_str())
                    && node.layout_visibility == Some(LayoutVisibility::Collapsed)
            })
            && declaration.item_template_node_id != declaration.list_node_id
            && declaration.overscan.is_finite()
            && declaration.overscan >= 0.0
            && (1..=4096).contains(&declaration.max_live_instances)
            && extent_valid
            && list_nodes.insert(declaration.list_node_id.as_str())
            && data_paths.insert(declaration.data_path.as_str());
        if !valid {
            error(
                &mut items,
                Some(&declaration.list_node_id),
                "aui_virtual_list_declaration_invalid",
                &path,
            );
        }
    }
    items.extend(
        crate::aui_viewport::AuiViewportResolver::validate_contract(document)
            .into_iter()
            .map(|diagnostic| {
                AuiValidationItem::error(
                    diagnostic.code,
                    diagnostic.node_id,
                    diagnostic.canvas_id,
                    diagnostic.message,
                    "Use a valid root-viewport condition and a ViewportLayout canvas.",
                )
            }),
    );
    validate_tree(document, &mut items);
    items
}

fn valid_color(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(|hex| {
        matches!(hex.len(), 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn validate_tree(document: &AuiDocument, items: &mut Vec<AuiValidationItem>) {
    let ids: HashMap<_, _> = document
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.node_id.as_str(), i))
        .collect();
    if ids.len() != document.nodes.len() {
        error(items, None, "aui_layout_duplicate_id", "nodes");
        return;
    }
    let mut parents = vec![None; document.nodes.len()];
    let mut children = vec![Vec::new(); document.nodes.len()];
    for (i, node) in document.nodes.iter().enumerate() {
        for child in &node.children {
            let Some(&j) = ids.get(child.as_str()) else {
                error(
                    items,
                    Some(&node.node_id),
                    "aui_layout_missing_child",
                    "children",
                );
                continue;
            };
            if parents[j].replace(i).is_some() {
                error(
                    items,
                    Some(child),
                    "aui_layout_multiple_parents",
                    "children",
                );
            }
            if document.nodes[j].parent.as_deref() != Some(node.node_id.as_str()) {
                error(items, Some(child), "aui_layout_parent_mismatch", "parent");
            }
            children[i].push(j);
        }
    }
    for (i, node) in document.nodes.iter().enumerate() {
        if node.parent.as_deref() != parents[i].map(|p| document.nodes[p].node_id.as_str()) {
            error(
                items,
                Some(&node.node_id),
                "aui_layout_parent_mismatch",
                "parent",
            );
        }
    }
    let mut colors = vec![0u8; document.nodes.len()];
    let roots = (0..parents.len())
        .filter(|&i| parents[i].is_none())
        .chain((0..parents.len()).filter(|&i| parents[i].is_some()));
    for root in roots {
        if colors[root] != 0 {
            continue;
        }
        let mut stack = vec![(root, 0, false)];
        while let Some((i, depth, exit)) = stack.pop() {
            if exit {
                colors[i] = 2;
                continue;
            }
            if colors[i] == 1 {
                error(
                    items,
                    Some(&document.nodes[i].node_id),
                    "aui_layout_cycle",
                    "children",
                );
                return;
            }
            if colors[i] == 2 {
                continue;
            }
            if depth > MAX_LAYOUT_DEPTH {
                error(
                    items,
                    Some(&document.nodes[i].node_id),
                    "aui_layout_depth_budget",
                    "children",
                );
                return;
            }
            colors[i] = 1;
            stack.push((i, depth, true));
            for &j in children[i].iter().rev() {
                stack.push((j, depth + 1, false));
            }
        }
    }
    for canvas in &document.canvases {
        if !canvas.reference_resolution.x.is_finite()
            || canvas.reference_resolution.x <= 0.0
            || !canvas.reference_resolution.y.is_finite()
            || canvas.reference_resolution.y <= 0.0
        {
            error(
                items,
                None,
                "aui_layout_canvas_invalid",
                "canvases.reference_resolution",
            );
        }
        match ids.get(canvas.root_node.as_str()) {
            Some(&i) if parents[i].is_none() => {}
            _ => error(
                items,
                Some(&canvas.root_node),
                "aui_layout_canvas_root_invalid",
                "canvases.root_node",
            ),
        }
    }
}
