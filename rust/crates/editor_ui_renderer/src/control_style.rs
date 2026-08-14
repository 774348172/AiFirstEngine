use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::{UiColor, WidgetRole};

pub const EDITOR_STYLE_SHEET_SCHEMA_VERSION: &str = "editor-style-sheet.v1";
const DARK_NEUTRAL_STYLE_SHEET: &str =
    include_str!("../../../resources/editor/themes/dark-neutral/control-style.v1.json");
static DARK_NEUTRAL_MODULE: OnceLock<Mutex<EditorControlStyleModule>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlPseudoState {
    Hover,
    Active,
    Selected,
    Checked,
    Disabled,
    Focus,
    FocusVisible,
}

impl ControlPseudoState {
    const fn bit(self) -> u16 {
        match self {
            Self::Hover => 1 << 0,
            Self::Active => 1 << 1,
            Self::Selected => 1 << 2,
            Self::Checked => 1 << 3,
            Self::Disabled => 1 << 4,
            Self::Focus => 1 << 5,
            Self::FocusVisible => 1 << 6,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlPseudoStateSet(u16);

impl ControlPseudoStateSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn from_states(states: impl IntoIterator<Item = ControlPseudoState>) -> Self {
        let mut result = Self::empty();
        for state in states {
            result.insert(state);
        }
        result
    }

    pub fn insert(&mut self, state: ControlPseudoState) {
        self.0 |= state.bit();
    }

    pub fn with(mut self, state: ControlPseudoState, enabled: bool) -> Self {
        if enabled {
            self.insert(state);
        } else {
            self.remove(state);
        }
        self
    }

    pub fn remove(&mut self, state: ControlPseudoState) {
        self.0 &= !state.bit();
    }

    pub const fn contains(self, state: ControlPseudoState) -> bool {
        self.0 & state.bit() != 0
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlClassSet(Vec<String>);

impl ControlClassSet {
    pub fn new(classes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut classes = classes.into_iter().map(Into::into).collect::<Vec<_>>();
        classes.sort();
        classes.dedup();
        classes.truncate(8);
        Self(classes)
    }

    pub fn contains(&self, class: &str) -> bool {
        self.0
            .binary_search_by(|candidate| candidate.as_str().cmp(class))
            .is_ok()
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ControlContentOffset {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ControlSliceInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ControlBrush {
    None,
    Solid {
        color: UiColor,
    },
    Texture {
        texture_id: String,
        fallback_color: UiColor,
        tint: UiColor,
    },
    NineSlice {
        texture_id: String,
        fallback_color: UiColor,
        tint: UiColor,
        slice: ControlSliceInsets,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ControlStyleBorder {
    pub color: UiColor,
    pub width: f32,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedControlStyle {
    pub background: ControlBrush,
    pub border: ControlStyleBorder,
    pub foreground: UiColor,
    pub icon_tint: UiColor,
    pub opacity: f32,
    pub content_offset: ControlContentOffset,
}

impl Default for ResolvedControlStyle {
    fn default() -> Self {
        Self {
            background: ControlBrush::Solid {
                color: UiColor::FIELD,
            },
            border: ControlStyleBorder {
                color: UiColor::BORDER,
                width: 1.0,
                corner_radius: 3.0,
            },
            foreground: UiColor::TEXT,
            icon_tint: UiColor::TEXT,
            opacity: 1.0,
            content_offset: ControlContentOffset::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlStyleQuery {
    pub role: WidgetRole,
    pub classes: ControlClassSet,
    pub pseudo_states: ControlPseudoStateSet,
}

impl ControlStyleQuery {
    pub fn new(
        role: WidgetRole,
        classes: impl IntoIterator<Item = impl Into<String>>,
        pseudo_states: ControlPseudoStateSet,
    ) -> Self {
        Self {
            role,
            classes: ControlClassSet::new(classes),
            pseudo_states,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStyleDiagnostic {
    pub code: String,
    pub message: String,
    pub rule_index: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStyleSummary {
    pub sheet_id: String,
    pub generation: u64,
    pub rule_count: usize,
    pub cache_hit_count: u64,
    pub cache_miss_count: u64,
    pub fallback_count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStyleTrace {
    pub matched_rule_indices: Vec<usize>,
    pub winning_rule_by_property: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlStyleResolution {
    pub style: ResolvedControlStyle,
    pub trace: Option<ControlStyleTrace>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StyleSheetSpec {
    schema_version: String,
    sheet_id: String,
    generation: u64,
    tokens: BTreeMap<String, String>,
    rules: Vec<StyleRuleSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StyleRuleSpec {
    selector: StyleSelectorSpec,
    declarations: StyleDeclarationsSpec,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StyleSelectorSpec {
    role: Option<WidgetRole>,
    #[serde(default)]
    classes: Vec<String>,
    #[serde(default)]
    pseudo: Vec<ControlPseudoState>,
    #[serde(default)]
    pseudo_not: Vec<ControlPseudoState>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StyleDeclarationsSpec {
    background: Option<BrushSpec>,
    border_color_token: Option<String>,
    border_width: Option<f32>,
    corner_radius: Option<f32>,
    foreground_token: Option<String>,
    icon_tint_token: Option<String>,
    opacity: Option<f32>,
    content_offset: Option<ControlContentOffset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum BrushSpec {
    None,
    Solid {
        color_token: String,
    },
    Texture {
        texture_id: String,
        fallback_color_token: String,
        #[serde(default)]
        tint_token: Option<String>,
    },
    NineSlice {
        texture_id: String,
        fallback_color_token: String,
        #[serde(default)]
        tint_token: Option<String>,
        slice: ControlSliceInsets,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ControlStyleCacheKey {
    role: String,
    classes: ControlClassSet,
    pseudo_bits: u16,
}

pub struct EditorControlStyleModule {
    sheet: StyleSheetSpec,
    cache: BTreeMap<ControlStyleCacheKey, ResolvedControlStyle>,
    cache_hit_count: u64,
    cache_miss_count: u64,
    fallback_count: u64,
    diagnostics: Vec<ControlStyleDiagnostic>,
}

impl EditorControlStyleModule {
    pub fn from_json(source: &str) -> Result<Self, Vec<ControlStyleDiagnostic>> {
        let sheet: StyleSheetSpec = serde_json::from_str(source).map_err(|error| {
            vec![ControlStyleDiagnostic {
                code: "editor_style.schema_invalid".to_string(),
                message: error.to_string(),
                rule_index: None,
            }]
        })?;
        let mut diagnostics = Vec::new();
        if sheet.schema_version != EDITOR_STYLE_SHEET_SCHEMA_VERSION {
            diagnostics.push(ControlStyleDiagnostic {
                code: "editor_style.schema_unsupported".to_string(),
                message: format!(
                    "Expected {EDITOR_STYLE_SHEET_SCHEMA_VERSION}, found {}.",
                    sheet.schema_version
                ),
                rule_index: None,
            });
        }
        if sheet.rules.is_empty() {
            diagnostics.push(ControlStyleDiagnostic {
                code: "editor_style.no_rules".to_string(),
                message: "Style sheet must contain at least one rule.".to_string(),
                rule_index: None,
            });
        }
        for (index, rule) in sheet.rules.iter().enumerate() {
            validate_rule(&sheet, index, rule, &mut diagnostics);
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(Self {
            sheet,
            cache: BTreeMap::new(),
            cache_hit_count: 0,
            cache_miss_count: 0,
            fallback_count: 0,
            diagnostics: Vec::new(),
        })
    }

    pub fn dark_neutral() -> Self {
        Self::from_json(DARK_NEUTRAL_STYLE_SHEET)
            .expect("built-in DarkNeutral control style sheet must be valid")
    }

    pub fn resolve(&mut self, query: &ControlStyleQuery) -> ResolvedControlStyle {
        let key = cache_key(query);
        if let Some(style) = self.cache.get(&key) {
            self.cache_hit_count += 1;
            return style.clone();
        }
        self.cache_miss_count += 1;
        let resolution = self.resolve_uncached(query, false);
        self.cache.insert(key, resolution.style.clone());
        resolution.style
    }

    pub fn resolve_with_trace(&mut self, query: &ControlStyleQuery) -> ControlStyleResolution {
        self.resolve_uncached(query, true)
    }

    pub fn summary(&self) -> ControlStyleSummary {
        ControlStyleSummary {
            sheet_id: self.sheet.sheet_id.clone(),
            generation: self.sheet.generation,
            rule_count: self.sheet.rules.len(),
            cache_hit_count: self.cache_hit_count,
            cache_miss_count: self.cache_miss_count,
            fallback_count: self.fallback_count,
        }
    }

    pub fn diagnostics(&self) -> &[ControlStyleDiagnostic] {
        &self.diagnostics
    }

    fn resolve_uncached(
        &mut self,
        query: &ControlStyleQuery,
        include_trace: bool,
    ) -> ControlStyleResolution {
        let mut matches = self
            .sheet
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| selector_matches(&rule.selector, query))
            .map(|(index, rule)| (specificity(&rule.selector, index), index, rule))
            .collect::<Vec<_>>();
        matches.sort_by_key(|(specificity, _, _)| *specificity);

        let mut style = ResolvedControlStyle::default();
        let mut trace = include_trace.then(ControlStyleTrace::default);
        if matches.is_empty() {
            self.fallback_count += 1;
        }
        for (_, index, rule) in matches {
            if let Some(trace) = trace.as_mut() {
                trace.matched_rule_indices.push(index);
            }
            apply_declarations(
                &self.sheet.tokens,
                &rule.declarations,
                index,
                &mut style,
                trace.as_mut(),
            );
        }
        ControlStyleResolution { style, trace }
    }
}

pub fn dark_neutral_control_style(query: &ControlStyleQuery) -> ResolvedControlStyle {
    DARK_NEUTRAL_MODULE
        .get_or_init(|| Mutex::new(EditorControlStyleModule::dark_neutral()))
        .lock()
        .expect("DarkNeutral control style mutex poisoned")
        .resolve(query)
}

pub fn dark_neutral_control_style_summary() -> ControlStyleSummary {
    DARK_NEUTRAL_MODULE
        .get_or_init(|| Mutex::new(EditorControlStyleModule::dark_neutral()))
        .lock()
        .expect("DarkNeutral control style mutex poisoned")
        .summary()
}

fn validate_rule(
    sheet: &StyleSheetSpec,
    index: usize,
    rule: &StyleRuleSpec,
    diagnostics: &mut Vec<ControlStyleDiagnostic>,
) {
    if rule.selector.role.is_none() {
        diagnostics.push(ControlStyleDiagnostic {
            code: "editor_style.selector_invalid".to_string(),
            message: "v1 selectors must specify a role.".to_string(),
            rule_index: Some(index),
        });
    }
    if rule.selector.classes.len() > 8
        || rule
            .selector
            .classes
            .iter()
            .any(|class| !valid_class(class))
    {
        diagnostics.push(ControlStyleDiagnostic {
            code: "editor_style.selector_invalid".to_string(),
            message: "Control classes must be bounded semantic identifiers.".to_string(),
            rule_index: Some(index),
        });
    }
    let pseudo = rule
        .selector
        .pseudo
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let pseudo_not = rule
        .selector
        .pseudo_not
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if !pseudo.is_disjoint(&pseudo_not) {
        diagnostics.push(ControlStyleDiagnostic {
            code: "editor_style.selector_invalid".to_string(),
            message: "A pseudo state cannot be both required and forbidden.".to_string(),
            rule_index: Some(index),
        });
    }
    for token in declaration_tokens(&rule.declarations) {
        if !sheet.tokens.contains_key(token) {
            diagnostics.push(ControlStyleDiagnostic {
                code: "editor_style.token_missing".to_string(),
                message: format!("Unknown color token {token}."),
                rule_index: Some(index),
            });
        }
    }
    if rule
        .declarations
        .opacity
        .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || rule
            .declarations
            .border_width
            .is_some_and(|value| value < 0.0)
        || rule
            .declarations
            .corner_radius
            .is_some_and(|value| value < 0.0)
    {
        diagnostics.push(ControlStyleDiagnostic {
            code: "editor_style.declaration_invalid".to_string(),
            message: "Opacity, border width, or corner radius is out of range.".to_string(),
            rule_index: Some(index),
        });
    }
}

fn declaration_tokens(declarations: &StyleDeclarationsSpec) -> Vec<&str> {
    let mut tokens = Vec::new();
    if let Some(token) = declarations.border_color_token.as_deref() {
        tokens.push(token);
    }
    if let Some(token) = declarations.foreground_token.as_deref() {
        tokens.push(token);
    }
    if let Some(token) = declarations.icon_tint_token.as_deref() {
        tokens.push(token);
    }
    match declarations.background.as_ref() {
        Some(BrushSpec::Solid { color_token }) => tokens.push(color_token),
        Some(BrushSpec::Texture {
            fallback_color_token,
            tint_token,
            ..
        })
        | Some(BrushSpec::NineSlice {
            fallback_color_token,
            tint_token,
            ..
        }) => {
            tokens.push(fallback_color_token);
            if let Some(token) = tint_token.as_deref() {
                tokens.push(token);
            }
        }
        Some(BrushSpec::None) | None => {}
    }
    tokens
}

fn selector_matches(selector: &StyleSelectorSpec, query: &ControlStyleQuery) -> bool {
    selector.role.is_none_or(|role| role == query.role)
        && selector
            .classes
            .iter()
            .all(|class| query.classes.contains(class))
        && selector
            .pseudo
            .iter()
            .all(|state| query.pseudo_states.contains(*state))
        && selector
            .pseudo_not
            .iter()
            .all(|state| !query.pseudo_states.contains(*state))
}

fn specificity(selector: &StyleSelectorSpec, index: usize) -> (usize, usize, usize, usize) {
    (
        selector.pseudo.len() + selector.pseudo_not.len(),
        selector.classes.len(),
        usize::from(selector.role.is_some()),
        index,
    )
}

fn apply_declarations(
    tokens: &BTreeMap<String, String>,
    declarations: &StyleDeclarationsSpec,
    rule_index: usize,
    style: &mut ResolvedControlStyle,
    trace: Option<&mut ControlStyleTrace>,
) {
    let mut trace = trace;
    if let Some(background) = declarations.background.as_ref() {
        style.background = resolve_brush(tokens, background);
        record_winner(&mut trace, "background", rule_index);
    }
    if let Some(token) = declarations.border_color_token.as_deref() {
        style.border.color = resolve_color(tokens, token);
        record_winner(&mut trace, "border.color", rule_index);
    }
    if let Some(width) = declarations.border_width {
        style.border.width = width;
        record_winner(&mut trace, "border.width", rule_index);
    }
    if let Some(radius) = declarations.corner_radius {
        style.border.corner_radius = radius;
        record_winner(&mut trace, "border.cornerRadius", rule_index);
    }
    if let Some(token) = declarations.foreground_token.as_deref() {
        style.foreground = resolve_color(tokens, token);
        record_winner(&mut trace, "foreground", rule_index);
    }
    if let Some(token) = declarations.icon_tint_token.as_deref() {
        style.icon_tint = resolve_color(tokens, token);
        record_winner(&mut trace, "iconTint", rule_index);
    }
    if let Some(opacity) = declarations.opacity {
        style.opacity = opacity;
        record_winner(&mut trace, "opacity", rule_index);
    }
    if let Some(offset) = declarations.content_offset {
        style.content_offset = offset;
        record_winner(&mut trace, "contentOffset", rule_index);
    }
}

fn record_winner(trace: &mut Option<&mut ControlStyleTrace>, property: &str, rule_index: usize) {
    if let Some(trace) = trace.as_deref_mut() {
        trace
            .winning_rule_by_property
            .insert(property.to_string(), rule_index);
    }
}

fn resolve_brush(tokens: &BTreeMap<String, String>, spec: &BrushSpec) -> ControlBrush {
    match spec {
        BrushSpec::None => ControlBrush::None,
        BrushSpec::Solid { color_token } => ControlBrush::Solid {
            color: resolve_color(tokens, color_token),
        },
        BrushSpec::Texture {
            texture_id,
            fallback_color_token,
            tint_token,
        } => ControlBrush::Texture {
            texture_id: texture_id.clone(),
            fallback_color: resolve_color(tokens, fallback_color_token),
            tint: tint_token
                .as_deref()
                .map(|token| resolve_color(tokens, token))
                .unwrap_or(UiColor::rgba(255, 255, 255, 255)),
        },
        BrushSpec::NineSlice {
            texture_id,
            fallback_color_token,
            tint_token,
            slice,
        } => ControlBrush::NineSlice {
            texture_id: texture_id.clone(),
            fallback_color: resolve_color(tokens, fallback_color_token),
            tint: tint_token
                .as_deref()
                .map(|token| resolve_color(tokens, token))
                .unwrap_or(UiColor::rgba(255, 255, 255, 255)),
            slice: *slice,
        },
    }
}

fn resolve_color(tokens: &BTreeMap<String, String>, token: &str) -> UiColor {
    tokens
        .get(token)
        .and_then(|value| parse_hex_color(value))
        .unwrap_or(UiColor::ERROR)
}

fn parse_hex_color(value: &str) -> Option<UiColor> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 8 {
        return None;
    }
    Some(UiColor::rgba(
        u8::from_str_radix(&digits[0..2], 16).ok()?,
        u8::from_str_radix(&digits[2..4], 16).ok()?,
        u8::from_str_radix(&digits[4..6], 16).ok()?,
        u8::from_str_radix(&digits[6..8], 16).ok()?,
    ))
}

fn valid_class(class: &str) -> bool {
    !class.is_empty()
        && class.len() <= 64
        && class
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn cache_key(query: &ControlStyleQuery) -> ControlStyleCacheKey {
    ControlStyleCacheKey {
        role: format!("{:?}", query.role),
        classes: query.classes.clone(),
        pseudo_bits: query.pseudo_states.bits(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(role: WidgetRole, class: &str, states: &[ControlPseudoState]) -> ControlStyleQuery {
        ControlStyleQuery::new(
            role,
            [class],
            ControlPseudoStateSet::from_states(states.iter().copied()),
        )
    }

    #[test]
    fn control_style_resolves_composed_pseudo_states_and_trace() {
        let mut module = EditorControlStyleModule::dark_neutral();
        let result = module.resolve_with_trace(&query(
            WidgetRole::Tab,
            "workspace-tab",
            &[ControlPseudoState::Selected, ControlPseudoState::Hover],
        ));
        assert!(matches!(
            result.style.background,
            ControlBrush::NineSlice { ref texture_id, .. }
                if texture_id == "editor-control-tab-selected-hover"
        ));
        let trace = result.trace.expect("trace requested");
        assert!(trace.matched_rule_indices.len() >= 3);
        assert!(trace.winning_rule_by_property.contains_key("background"));
    }

    #[test]
    fn control_style_cache_key_is_independent_of_widget_identity_and_rect() {
        let mut module = EditorControlStyleModule::dark_neutral();
        let query = query(
            WidgetRole::Button,
            "toolbar-control",
            &[ControlPseudoState::Hover],
        );
        let first = module.resolve(&query);
        let second = module.resolve(&query);
        assert_eq!(first, second);
        let summary = module.summary();
        assert_eq!(summary.cache_miss_count, 1);
        assert_eq!(summary.cache_hit_count, 1);
    }

    #[test]
    fn control_style_schema_rejects_unknown_tokens_and_unbounded_selectors() {
        let source = r##"{
          "schemaVersion":"editor-style-sheet.v1",
          "sheetId":"bad",
          "generation":1,
          "tokens":{},
          "rules":[{
            "selector":{"role":"Button","classes":["Not_Semantic"]},
            "declarations":{"foregroundToken":"missing"}
          }]
        }"##;
        let diagnostics = match EditorControlStyleModule::from_json(source) {
            Ok(_) => panic!("invalid style sheet accepted"),
            Err(diagnostics) => diagnostics,
        };
        assert!(diagnostics
            .iter()
            .any(|item| item.code == "editor_style.selector_invalid"));
        assert!(diagnostics
            .iter()
            .any(|item| item.code == "editor_style.token_missing"));
    }

    #[test]
    fn control_style_disabled_overrides_hover_without_flattening_selected() {
        let mut module = EditorControlStyleModule::dark_neutral();
        let style = module.resolve(&query(
            WidgetRole::Button,
            "toolbar-control",
            &[ControlPseudoState::Hover, ControlPseudoState::Disabled],
        ));
        assert_eq!(style.foreground, UiColor::rgba(102, 102, 102, 255));
        assert_eq!(style.opacity, 0.72);
    }

    #[test]
    fn control_style_visual_matrix_preserves_geometry_and_selects_distinct_states() {
        let surfaces = [(1280.0_f32, 720.0_f32), (1600.0, 900.0)];
        let scales = [1.0_f32, 1.25, 1.5, 2.0];
        let tab_cases = [
            (vec![], None),
            (
                vec![ControlPseudoState::Hover],
                Some("editor-control-tab-hover"),
            ),
            (
                vec![ControlPseudoState::Active],
                Some("editor-control-tab-active"),
            ),
            (
                vec![ControlPseudoState::Selected],
                Some("editor-control-tab-selected"),
            ),
            (
                vec![ControlPseudoState::Selected, ControlPseudoState::Hover],
                Some("editor-control-tab-selected-hover"),
            ),
        ];
        let button_cases = [
            vec![],
            vec![ControlPseudoState::Hover],
            vec![ControlPseudoState::Active],
            vec![ControlPseudoState::Disabled],
        ];

        for (physical_width, physical_height) in surfaces {
            for scale in scales {
                let logical_width = physical_width / scale;
                let logical_height = physical_height / scale;
                let target = crate::UiRect {
                    x: 20.0,
                    y: 20.0,
                    width: (logical_width * 0.25).min(140.0),
                    height: logical_height.min(28.0),
                };

                for (states, expected_texture) in &tab_cases {
                    let mut module = EditorControlStyleModule::dark_neutral();
                    let style = module.resolve(&query(WidgetRole::Tab, "workspace-tab", states));
                    let output =
                        crate::paint_control_brush(target, &style.background, style.opacity);
                    assert_brush_covers_target(&output.commands, target);
                    match (&style.background, expected_texture) {
                        (ControlBrush::Solid { .. }, None) => {}
                        (ControlBrush::NineSlice { texture_id, .. }, Some(expected)) => {
                            assert_eq!(texture_id, expected);
                        }
                        pair => panic!("unexpected tab brush mapping: {pair:?}"),
                    }
                }

                let mut button_backgrounds = Vec::new();
                for states in &button_cases {
                    let mut module = EditorControlStyleModule::dark_neutral();
                    let style =
                        module.resolve(&query(WidgetRole::Button, "toolbar-control", states));
                    let output =
                        crate::paint_control_brush(target, &style.background, style.opacity);
                    assert_brush_covers_target(&output.commands, target);
                    button_backgrounds.push(style.background);
                }
                button_backgrounds.dedup();
                assert_eq!(button_backgrounds.len(), button_cases.len());
            }
        }
    }

    fn assert_brush_covers_target(commands: &[crate::DrawCommand], target: crate::UiRect) {
        assert!(!commands.is_empty());
        let rects = commands
            .iter()
            .map(|command| match command {
                crate::DrawCommand::Rect { rect, .. }
                | crate::DrawCommand::ImageTextureSlot { rect, .. } => *rect,
                other => panic!("unexpected control brush command: {other:?}"),
            })
            .collect::<Vec<_>>();
        let left = rects
            .iter()
            .map(|rect| rect.x)
            .fold(f32::INFINITY, f32::min);
        let top = rects
            .iter()
            .map(|rect| rect.y)
            .fold(f32::INFINITY, f32::min);
        let right = rects
            .iter()
            .map(|rect| rect.x + rect.width)
            .fold(f32::NEG_INFINITY, f32::max);
        let bottom = rects
            .iter()
            .map(|rect| rect.y + rect.height)
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(
            (left, top, right, bottom),
            (
                target.x,
                target.y,
                target.x + target.width,
                target.y + target.height
            )
        );
        assert!(rects
            .iter()
            .all(|rect| rect.width >= 0.0 && rect.height >= 0.0));
    }
}
