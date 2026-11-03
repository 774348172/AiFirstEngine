use crate::aui::{AuiControlInteractionSnapshot, AuiDocument, AuiNode, AuiNodeKind};
use crate::aui_layout_schema::Insets;
use crate::runtime_asset::{RuntimeAssetIndex, RuntimeAssetResolveError};
use crate::runtime_package::{RuntimeAssetManifest, RuntimeAssetRef};
use crate::runtime_package_source::RuntimePackageSource;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

pub const AUI_THEME_ASSET_SCHEMA_VERSION: &str = "aui-theme-asset.v1";
pub const BUILTIN_AUI_THEME_ID: &str = "builtin.default.v1";

type MergedTheme = (
    BTreeMap<String, AuiThemeToken>,
    BTreeMap<String, AuiThemeStyle>,
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AuiThemeToken {
    Color { value: String },
    Font { value: RuntimeAssetRef },
    Length { value: f32 },
    Number { value: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "camelCase", deny_unknown_fields)]
pub enum AuiThemeValue<T> {
    Value { value: T },
    Token { token: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiSliceInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AuiBrushSpec {
    #[default]
    None,
    Solid {
        color: AuiThemeValue<String>,
    },
    Image {
        asset: RuntimeAssetRef,
        #[serde(default)]
        tint: Option<AuiThemeValue<String>>,
    },
    NineSlice {
        asset: RuntimeAssetRef,
        #[serde(default)]
        tint: Option<AuiThemeValue<String>>,
        slice: AuiSliceInsets,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemeBorderSpec {
    pub color: Option<AuiThemeValue<String>>,
    pub width: Option<AuiThemeValue<f32>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemeShadowSpec {
    pub color: Option<AuiThemeValue<String>>,
    pub offset_x: Option<AuiThemeValue<f32>>,
    pub offset_y: Option<AuiThemeValue<f32>>,
    pub blur: Option<AuiThemeValue<f32>>,
    pub spread: Option<AuiThemeValue<f32>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemeMetricsSpec {
    pub font: Option<AuiThemeValue<RuntimeAssetRef>>,
    pub font_size: Option<AuiThemeValue<f32>>,
    pub line_height: Option<AuiThemeValue<f32>>,
    pub padding: Option<AuiThemeValue<f32>>,
    pub min_width: Option<AuiThemeValue<f32>>,
    pub min_height: Option<AuiThemeValue<f32>>,
    pub border_inset: Option<AuiThemeValue<f32>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemePaintSpec {
    pub background: Option<AuiBrushSpec>,
    pub foreground: Option<AuiThemeValue<String>>,
    pub border: Option<AuiThemeBorderSpec>,
    pub radius: Option<AuiThemeValue<f32>>,
    pub shadow: Option<AuiThemeShadowSpec>,
    pub opacity: Option<AuiThemeValue<f32>>,
}

pub type AuiThemeStyleOverride = AuiThemePaintSpec;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemeStates {
    pub hover: Option<AuiThemePaintSpec>,
    pub pressed: Option<AuiThemePaintSpec>,
    pub focus: Option<AuiThemePaintSpec>,
    pub disabled: Option<AuiThemePaintSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemeStyle {
    pub base_style: Option<String>,
    pub role: Option<String>,
    pub variant: Option<String>,
    pub metrics: AuiThemeMetricsSpec,
    pub paint: AuiThemePaintSpec,
    pub states: AuiThemeStates,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiThemeAsset {
    pub schema_version: String,
    pub asset_id: String,
    #[serde(default)]
    pub extends: Option<RuntimeAssetRef>,
    #[serde(default)]
    pub tokens: BTreeMap<String, AuiThemeToken>,
    #[serde(default)]
    pub styles: BTreeMap<String, AuiThemeStyle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiThemeDiagnostic {
    pub code: String,
    pub asset_id: Option<String>,
    pub style_id: Option<String>,
    pub message: String,
}

impl AuiThemeDiagnostic {
    fn new(
        code: impl Into<String>,
        asset_id: Option<&str>,
        style_id: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            asset_id: asset_id.map(str::to_string),
            style_id: style_id.map(str::to_string),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedMetrics {
    pub font: Option<RuntimeAssetRef>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub padding: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub border_inset: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AuiResolvedBrush {
    #[default]
    None,
    Solid {
        color: String,
    },
    Image {
        asset_id: String,
        tint: String,
    },
    NineSlice {
        asset_id: String,
        tint: String,
        slice: AuiSliceInsets,
    },
}

impl AuiResolvedBrush {
    pub fn solid(color: impl Into<String>) -> Self {
        Self::Solid {
            color: normalize_color(&color.into()).unwrap_or_else(|| "#ff00ffff".to_string()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedBorder {
    pub color: Option<String>,
    pub width: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedShadow {
    pub color: Option<String>,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
}

impl AuiResolvedShadow {
    pub fn visible(&self) -> bool {
        self.color.is_some()
            && (self.blur > 0.0
                || self.spread > 0.0
                || self.offset_x != 0.0
                || self.offset_y != 0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedPaint {
    pub background: AuiResolvedBrush,
    pub foreground: Option<String>,
    pub border: AuiResolvedBorder,
    pub radius: f32,
    pub shadow: AuiResolvedShadow,
    pub opacity: f32,
}

impl Default for AuiResolvedPaint {
    fn default() -> Self {
        Self {
            background: AuiResolvedBrush::None,
            foreground: None,
            border: AuiResolvedBorder::default(),
            radius: 0.0,
            shadow: AuiResolvedShadow::default(),
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiResolvedStyle {
    pub theme_id: String,
    pub style_id: String,
    pub metrics: AuiResolvedMetrics,
    pub paint: AuiResolvedPaint,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuiResolvedThemeDocument {
    pub document: AuiDocument,
    pub styles: BTreeMap<String, AuiResolvedStyle>,
}

#[derive(Debug, Clone)]
pub struct AuiThemeRegistry {
    assets: std::sync::Arc<BTreeMap<String, AuiThemeAsset>>,
    default_theme_id: String,
}

impl Default for AuiThemeRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl AuiThemeRegistry {
    pub fn builtin() -> Self {
        static BUILTIN: std::sync::OnceLock<AuiThemeRegistry> = std::sync::OnceLock::new();
        BUILTIN
            .get_or_init(|| {
                let asset = builtin_theme();
                Self {
                    default_theme_id: asset.asset_id.clone(),
                    assets: std::sync::Arc::new(
                        [(asset.asset_id.clone(), asset)].into_iter().collect(),
                    ),
                }
            })
            .clone()
    }

    pub(crate) fn shares_storage(&self, other: &Self) -> bool {
        self.default_theme_id == other.default_theme_id
            && std::sync::Arc::ptr_eq(&self.assets, &other.assets)
    }

    pub fn from_assets(assets: Vec<AuiThemeAsset>) -> Result<Self, Vec<AuiThemeDiagnostic>> {
        let mut registry = Self::builtin();
        let mut errors = Vec::new();
        for asset in assets {
            if asset.schema_version != AUI_THEME_ASSET_SCHEMA_VERSION {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.schema_unsupported",
                    Some(&asset.asset_id),
                    None,
                    format!("Expected {AUI_THEME_ASSET_SCHEMA_VERSION}."),
                ));
            }
            if registry.assets.contains_key(&asset.asset_id) {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.asset_duplicate",
                    Some(&asset.asset_id),
                    None,
                    "Theme asset id must be unique.",
                ));
            } else {
                std::sync::Arc::make_mut(&mut registry.assets)
                    .insert(asset.asset_id.clone(), asset);
            }
        }
        registry.validate(&mut errors);
        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(errors)
        }
    }

    pub fn from_runtime_package(
        package_dir: &Path,
        manifest: &RuntimeAssetManifest,
        index: &RuntimeAssetIndex,
    ) -> Result<Self, Vec<AuiThemeDiagnostic>> {
        Self::from_package_source(
            &RuntimePackageSource::directory(package_dir),
            package_dir,
            manifest,
            index,
        )
    }

    pub fn from_package_source(
        source: &RuntimePackageSource,
        _package_dir: &Path,
        manifest: &RuntimeAssetManifest,
        index: &RuntimeAssetIndex,
    ) -> Result<Self, Vec<AuiThemeDiagnostic>> {
        let mut assets = Vec::new();
        let mut errors = Vec::new();
        for record in manifest
            .runtime_asset_index
            .iter()
            .filter(|record| record.asset_type == "aui-theme")
        {
            let Some(cooked) = index.cooked_asset(&record.cooked_asset_id) else {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.cooked_asset_missing",
                    Some(&record.asset_id),
                    None,
                    "Theme has no cooked asset record.",
                ));
                continue;
            };
            let Some(relative) = cooked.path.as_deref() else {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.cooked_path_missing",
                    Some(&record.asset_id),
                    None,
                    "Theme cooked asset has no package path.",
                ));
                continue;
            };
            let path = match source.resolve(relative) {
                Ok(path) => path,
                Err(error) => {
                    errors.push(AuiThemeDiagnostic::new(
                        "aui_theme.cooked_path_invalid",
                        Some(&record.asset_id),
                        None,
                        error.to_string(),
                    ));
                    continue;
                }
            };
            let asset = source
                .read_path(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<AuiThemeAsset>(&bytes)
                        .map_err(|error| error.to_string())
                });
            match asset {
                Ok(asset) if asset.asset_id == record.asset_id => assets.push(asset),
                Ok(asset) => errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.asset_id_mismatch",
                    Some(&record.asset_id),
                    None,
                    format!("Cooked theme declares assetId '{}'.", asset.asset_id),
                )),
                Err(message) => errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.cooked_asset_invalid",
                    Some(&record.asset_id),
                    None,
                    message,
                )),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let registry = Self::from_assets(assets)?;
        registry.validate_runtime_asset_refs(index)?;
        Ok(registry)
    }

    pub fn texture_asset_ids(&self) -> BTreeSet<String> {
        self.assets
            .values()
            .flat_map(|asset| asset.styles.values())
            .flat_map(|style| {
                std::iter::once(&style.paint).chain(
                    [
                        style.states.hover.as_ref(),
                        style.states.pressed.as_ref(),
                        style.states.focus.as_ref(),
                        style.states.disabled.as_ref(),
                    ]
                    .into_iter()
                    .flatten(),
                )
            })
            .filter_map(|paint| match paint.background.as_ref() {
                Some(AuiBrushSpec::Image { asset, .. } | AuiBrushSpec::NineSlice { asset, .. }) => {
                    Some(asset.id.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn validate_document_refs(
        &self,
        document: &AuiDocument,
        index: &RuntimeAssetIndex,
    ) -> Result<(), Vec<AuiThemeDiagnostic>> {
        let mut errors = Vec::new();
        for reference in document
            .theme
            .iter()
            .chain(document.nodes.iter().filter_map(|node| node.theme.as_ref()))
        {
            if let Err(error) = validate_runtime_ref(index, reference) {
                errors.push(error);
            } else if !self.assets.contains_key(&reference.id) {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.asset_missing",
                    Some(&reference.id),
                    None,
                    "Referenced theme is not present in the runtime registry.",
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_runtime_asset_refs(
        &self,
        index: &RuntimeAssetIndex,
    ) -> Result<(), Vec<AuiThemeDiagnostic>> {
        let mut errors = Vec::new();
        for asset in self
            .assets
            .values()
            .filter(|asset| asset.asset_id != BUILTIN_AUI_THEME_ID)
        {
            if let Some(reference) = &asset.extends {
                if let Err(error) = validate_runtime_ref(index, reference) {
                    errors.push(error);
                }
            }
            for token in asset.tokens.values() {
                if let AuiThemeToken::Font { value } = token {
                    if let Err(error) = validate_runtime_ref(index, value) {
                        errors.push(error);
                    }
                }
            }
            for style in asset.styles.values() {
                if let Some(AuiThemeValue::Value { value }) = &style.metrics.font {
                    if let Err(error) = validate_runtime_ref(index, value) {
                        errors.push(error);
                    }
                }
                for paint in std::iter::once(&style.paint).chain(
                    [
                        style.states.hover.as_ref(),
                        style.states.pressed.as_ref(),
                        style.states.focus.as_ref(),
                        style.states.disabled.as_ref(),
                    ]
                    .into_iter()
                    .flatten(),
                ) {
                    if let Some(
                        AuiBrushSpec::Image { asset, .. } | AuiBrushSpec::NineSlice { asset, .. },
                    ) = &paint.background
                    {
                        if let Err(error) = validate_runtime_ref(index, asset) {
                            errors.push(error);
                        }
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn default_theme_id(&self) -> &str {
        &self.default_theme_id
    }

    pub fn assets(&self) -> impl Iterator<Item = &AuiThemeAsset> {
        self.assets.values()
    }

    pub fn get(&self, id: &str) -> Option<&AuiThemeAsset> {
        self.assets.get(id)
    }

    fn validate(&self, errors: &mut Vec<AuiThemeDiagnostic>) {
        for asset in self.assets.values() {
            if let Some(base) = &asset.extends {
                if base.asset_type != "aui-theme" || !self.assets.contains_key(&base.id) {
                    errors.push(AuiThemeDiagnostic::new(
                        "aui_theme.base_missing",
                        Some(&asset.asset_id),
                        None,
                        format!("Base theme '{}' is missing or has the wrong type.", base.id),
                    ));
                }
            }
            validate_style_cycles(asset, errors);
            for (style_id, style) in &asset.styles {
                validate_metrics(asset, style_id, &style.metrics, errors);
                validate_paint(asset, style_id, &style.paint, errors);
                for paint in [
                    style.states.hover.as_ref(),
                    style.states.pressed.as_ref(),
                    style.states.focus.as_ref(),
                    style.states.disabled.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    validate_paint(asset, style_id, paint, errors);
                }
            }
        }
        for id in self.assets.keys() {
            let mut visiting = HashSet::new();
            let mut current = id.as_str();
            while visiting.insert(current.to_string()) {
                let Some(next) = self
                    .assets
                    .get(current)
                    .and_then(|asset| asset.extends.as_ref())
                else {
                    break;
                };
                current = &next.id;
            }
            if self
                .assets
                .get(current)
                .and_then(|asset| asset.extends.as_ref())
                .is_some()
            {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.inheritance_cycle",
                    Some(id),
                    None,
                    "Theme inheritance must be acyclic.",
                ));
            }
        }
    }

    pub fn resolve_document(
        &self,
        document: &AuiDocument,
        interaction: &AuiControlInteractionSnapshot,
    ) -> Result<AuiResolvedThemeDocument, Vec<AuiThemeDiagnostic>> {
        let mut output = document.clone();
        let nodes = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let mut styles = BTreeMap::new();
        let mut merged_themes = HashMap::new();
        let mut errors = Vec::new();
        for (index, node) in document.nodes.iter().enumerate() {
            let theme_id =
                scoped_theme_id(document, node, &nodes).unwrap_or(&self.default_theme_id);
            match self.resolve_node(theme_id, node, interaction, &mut merged_themes) {
                Ok(style) => {
                    let target = &mut output.nodes[index];
                    apply_metrics_and_inline_style(target, &style);
                    target.resolved_style = Some(style.clone());
                    styles.insert(node.node_id.clone(), style);
                }
                Err(mut diagnostics) => errors.append(&mut diagnostics),
            }
        }
        if errors.is_empty() {
            Ok(AuiResolvedThemeDocument {
                document: output,
                styles,
            })
        } else {
            Err(errors)
        }
    }

    /// The prepared node already records its inherited theme; ordinary binding edits cannot
    /// change that scope. Resolve only this node through the same rules as a full document.
    pub(crate) fn resolve_prepared_node(
        &self,
        theme_id: &str,
        node: &mut AuiNode,
    ) -> Result<(), Vec<AuiThemeDiagnostic>> {
        self.resolve_prepared_node_with_interaction(
            theme_id,
            node,
            &AuiControlInteractionSnapshot::default(),
        )
    }

    pub(crate) fn resolve_prepared_node_with_interaction(
        &self,
        theme_id: &str,
        node: &mut AuiNode,
        interaction: &AuiControlInteractionSnapshot,
    ) -> Result<(), Vec<AuiThemeDiagnostic>> {
        let style = self.resolve_node(theme_id, node, interaction, &mut HashMap::new())?;
        apply_metrics_and_inline_style(node, &style);
        node.resolved_style = Some(style);
        Ok(())
    }

    fn resolve_node(
        &self,
        theme_id: &str,
        node: &AuiNode,
        interaction: &AuiControlInteractionSnapshot,
        merged_themes: &mut HashMap<String, MergedTheme>,
    ) -> Result<AuiResolvedStyle, Vec<AuiThemeDiagnostic>> {
        let Some(theme) = self.assets.get(theme_id) else {
            return Err(vec![AuiThemeDiagnostic::new(
                "aui_theme.asset_missing",
                Some(theme_id),
                None,
                "Selected theme is not loaded.",
            )]);
        };
        let (tokens, styles) = merged_themes
            .entry(theme_id.to_string())
            .or_insert_with(|| self.merged_theme(theme));
        let role = role_for_kind(node.kind);
        let style_id = node
            .style_ref
            .clone()
            .or_else(|| {
                styles
                    .iter()
                    .find(|(_, style)| {
                        style.role.as_deref() == Some(role) && style.variant == node.variant
                    })
                    .map(|(id, _)| id.clone())
            })
            .or_else(|| {
                styles
                    .iter()
                    .find(|(_, style)| {
                        style.role.as_deref() == Some(role) && style.variant.is_none()
                    })
                    .map(|(id, _)| id.clone())
            })
            .unwrap_or_else(|| "fallback".to_string());
        let mut chain = Vec::new();
        let mut current = style_id.as_str();
        let mut seen = HashSet::new();
        while let Some(style) = styles.get(current) {
            if !seen.insert(current.to_string()) {
                break;
            }
            chain.push((current.to_string(), style));
            let Some(base) = style.base_style.as_deref() else {
                break;
            };
            current = base;
        }
        chain.reverse();
        let mut resolved = AuiResolvedStyle {
            theme_id: theme_id.to_string(),
            style_id: style_id.clone(),
            ..Default::default()
        };
        for (id, style) in &chain {
            apply_metrics(tokens, &style.metrics, &mut resolved.metrics);
            apply_paint(tokens, &style.paint, &mut resolved.paint);
            resolved.sources.push(format!("{theme_id}:{id}"));
        }
        let selected = chain.last().map(|(_, style)| *style);
        let disabled = is_control(node.kind) && !node.interactable;
        if let Some(style) = selected {
            if disabled {
                if let Some(paint) = &style.states.disabled {
                    apply_paint(tokens, paint, &mut resolved.paint);
                }
            } else {
                if interaction.hovered_node.as_deref() == Some(&node.node_id) {
                    if let Some(paint) = &style.states.hover {
                        apply_paint(tokens, paint, &mut resolved.paint);
                    }
                }
                if interaction.pressed_node.as_deref() == Some(&node.node_id)
                    && interaction.pressed_inside
                {
                    if let Some(paint) = &style.states.pressed {
                        apply_paint(tokens, paint, &mut resolved.paint);
                    }
                }
                if interaction.focus_visible
                    && interaction.focused_node.as_deref() == Some(&node.node_id)
                {
                    if let Some(paint) = &style.states.focus {
                        apply_paint(tokens, paint, &mut resolved.paint);
                    }
                }
            }
        }
        if let Some(overrides) = &node.style_overrides {
            apply_paint(tokens, overrides, &mut resolved.paint);
        }
        apply_inline_style(node, &mut resolved);
        Ok(resolved)
    }

    fn merged_theme(&self, theme: &AuiThemeAsset) -> MergedTheme {
        let mut chain = Vec::new();
        let mut current = Some(theme);
        let mut seen = HashSet::new();
        while let Some(asset) = current {
            if !seen.insert(asset.asset_id.clone()) {
                break;
            }
            chain.push(asset);
            current = asset
                .extends
                .as_ref()
                .and_then(|base| self.assets.get(&base.id));
        }
        chain.reverse();
        let mut tokens = BTreeMap::new();
        let mut styles = BTreeMap::new();
        for asset in chain {
            tokens.extend(asset.tokens.clone());
            styles.extend(asset.styles.clone());
        }
        (tokens, styles)
    }
}

fn scoped_theme_id<'a>(
    document: &'a AuiDocument,
    node: &'a AuiNode,
    nodes: &HashMap<&str, &'a AuiNode>,
) -> Option<&'a str> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if let Some(theme) = &candidate.theme {
            return Some(&theme.id);
        }
        current = candidate
            .parent
            .as_deref()
            .and_then(|parent| nodes.get(parent).copied());
    }
    document.theme.as_ref().map(|theme| theme.id.as_str())
}

fn asset_ref_error(
    reference: &RuntimeAssetRef,
    error: RuntimeAssetResolveError,
) -> AuiThemeDiagnostic {
    let code = match error {
        RuntimeAssetResolveError::MissingAssetRef => "aui_theme.asset_ref_missing",
        RuntimeAssetResolveError::TypeMismatch { .. } => "aui_theme.asset_ref_type_mismatch",
        RuntimeAssetResolveError::SubAssetMissing => "aui_theme.asset_ref_sub_asset_missing",
    };
    AuiThemeDiagnostic::new(
        code,
        Some(&reference.id),
        None,
        format!("Runtime asset reference could not be resolved: {error:?}."),
    )
}

fn validate_runtime_ref(
    index: &RuntimeAssetIndex,
    reference: &RuntimeAssetRef,
) -> Result<(), AuiThemeDiagnostic> {
    let record = index
        .resolve(reference)
        .map_err(|error| asset_ref_error(reference, error))?;
    if record.asset_id != reference.id {
        return Err(AuiThemeDiagnostic::new(
            "aui_theme.asset_ref_identity_mismatch",
            Some(&reference.id),
            None,
            format!(
                "AssetRef id '{}' and guid '{}' resolved to asset id '{}'.",
                reference.id,
                reference.guid.as_deref().unwrap_or_default(),
                record.asset_id
            ),
        ));
    }
    Ok(())
}

fn role_for_kind(kind: AuiNodeKind) -> &'static str {
    match kind {
        AuiNodeKind::Panel => "panel",
        AuiNodeKind::Image => "image",
        AuiNodeKind::Text => "text",
        AuiNodeKind::Button => "button",
        AuiNodeKind::ProgressBar => "progress",
        AuiNodeKind::Toggle => "toggle",
        AuiNodeKind::Slider => "slider",
        AuiNodeKind::List => "list",
        AuiNodeKind::ScrollView => "scrollView",
        AuiNodeKind::InputField => "input",
        AuiNodeKind::Custom => "custom",
    }
}

fn is_control(kind: AuiNodeKind) -> bool {
    matches!(
        kind,
        AuiNodeKind::Button
            | AuiNodeKind::InputField
            | AuiNodeKind::Toggle
            | AuiNodeKind::Slider
            | AuiNodeKind::List
    )
}

fn validate_style_cycles(asset: &AuiThemeAsset, errors: &mut Vec<AuiThemeDiagnostic>) {
    for id in asset.styles.keys() {
        let mut seen = HashSet::new();
        let mut current = id.as_str();
        loop {
            if !seen.insert(current.to_string()) {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.style_cycle",
                    Some(&asset.asset_id),
                    Some(id),
                    "Style inheritance must be acyclic.",
                ));
                break;
            }
            let Some(base) = asset
                .styles
                .get(current)
                .and_then(|style| style.base_style.as_deref())
            else {
                break;
            };
            if !asset.styles.contains_key(base) {
                errors.push(AuiThemeDiagnostic::new(
                    "aui_theme.style_base_missing",
                    Some(&asset.asset_id),
                    Some(id),
                    format!("Base style '{base}' is missing."),
                ));
                break;
            }
            current = base;
        }
    }
}

#[derive(Clone, Copy)]
enum TokenKind {
    Color,
    Font,
    Length,
    Number,
}

fn token_kind(token: &AuiThemeToken) -> TokenKind {
    match token {
        AuiThemeToken::Color { .. } => TokenKind::Color,
        AuiThemeToken::Font { .. } => TokenKind::Font,
        AuiThemeToken::Length { .. } => TokenKind::Length,
        AuiThemeToken::Number { .. } => TokenKind::Number,
    }
}

fn validate_value<T>(
    asset: &AuiThemeAsset,
    style_id: &str,
    value: Option<&AuiThemeValue<T>>,
    expected: TokenKind,
    errors: &mut Vec<AuiThemeDiagnostic>,
) {
    let Some(AuiThemeValue::Token { token }) = value else {
        return;
    };
    match asset.tokens.get(token) {
        None => errors.push(AuiThemeDiagnostic::new(
            "aui_theme.token_missing",
            Some(&asset.asset_id),
            Some(style_id),
            format!("Token '{token}' is missing."),
        )),
        Some(actual)
            if std::mem::discriminant(&token_kind(actual)) != std::mem::discriminant(&expected) =>
        {
            errors.push(AuiThemeDiagnostic::new(
                "aui_theme.token_type_mismatch",
                Some(&asset.asset_id),
                Some(style_id),
                format!("Token '{token}' has the wrong type."),
            ))
        }
        _ => {}
    }
}

fn validate_metrics(
    asset: &AuiThemeAsset,
    id: &str,
    value: &AuiThemeMetricsSpec,
    errors: &mut Vec<AuiThemeDiagnostic>,
) {
    validate_value(asset, id, value.font.as_ref(), TokenKind::Font, errors);
    for length in [
        &value.font_size,
        &value.padding,
        &value.min_width,
        &value.min_height,
        &value.border_inset,
    ] {
        validate_value(asset, id, length.as_ref(), TokenKind::Length, errors);
    }
    validate_value(
        asset,
        id,
        value.line_height.as_ref(),
        TokenKind::Number,
        errors,
    );
}

fn validate_paint(
    asset: &AuiThemeAsset,
    id: &str,
    value: &AuiThemePaintSpec,
    errors: &mut Vec<AuiThemeDiagnostic>,
) {
    if let Some(AuiBrushSpec::Solid { color }) = &value.background {
        validate_value(asset, id, Some(color), TokenKind::Color, errors);
    }
    if let Some(AuiBrushSpec::Image { tint, .. } | AuiBrushSpec::NineSlice { tint, .. }) =
        &value.background
    {
        validate_value(asset, id, tint.as_ref(), TokenKind::Color, errors);
    }
    validate_value(
        asset,
        id,
        value.foreground.as_ref(),
        TokenKind::Color,
        errors,
    );
    validate_value(asset, id, value.radius.as_ref(), TokenKind::Length, errors);
    validate_value(asset, id, value.opacity.as_ref(), TokenKind::Number, errors);
    if let Some(border) = &value.border {
        validate_value(asset, id, border.color.as_ref(), TokenKind::Color, errors);
        validate_value(asset, id, border.width.as_ref(), TokenKind::Length, errors);
    }
    if let Some(shadow) = &value.shadow {
        validate_value(asset, id, shadow.color.as_ref(), TokenKind::Color, errors);
        for length in [
            &shadow.offset_x,
            &shadow.offset_y,
            &shadow.blur,
            &shadow.spread,
        ] {
            validate_value(asset, id, length.as_ref(), TokenKind::Length, errors);
        }
    }
}

fn resolve_color(
    tokens: &BTreeMap<String, AuiThemeToken>,
    value: &AuiThemeValue<String>,
) -> Option<String> {
    let raw = match value {
        AuiThemeValue::Value { value } => value,
        AuiThemeValue::Token { token } => match tokens.get(token)? {
            AuiThemeToken::Color { value } => value,
            _ => return None,
        },
    };
    normalize_color(raw)
}

fn resolve_length(
    tokens: &BTreeMap<String, AuiThemeToken>,
    value: &AuiThemeValue<f32>,
) -> Option<f32> {
    let number = match value {
        AuiThemeValue::Value { value } => *value,
        AuiThemeValue::Token { token } => match tokens.get(token)? {
            AuiThemeToken::Length { value } => *value,
            _ => return None,
        },
    };
    number.is_finite().then_some(number.max(0.0))
}

fn resolve_number(
    tokens: &BTreeMap<String, AuiThemeToken>,
    value: &AuiThemeValue<f32>,
) -> Option<f32> {
    let number = match value {
        AuiThemeValue::Value { value } => *value,
        AuiThemeValue::Token { token } => match tokens.get(token)? {
            AuiThemeToken::Number { value } => *value,
            _ => return None,
        },
    };
    number.is_finite().then_some(number)
}

fn resolve_font(
    tokens: &BTreeMap<String, AuiThemeToken>,
    value: &AuiThemeValue<RuntimeAssetRef>,
) -> Option<RuntimeAssetRef> {
    match value {
        AuiThemeValue::Value { value } => Some(value.clone()),
        AuiThemeValue::Token { token } => match tokens.get(token)? {
            AuiThemeToken::Font { value } => Some(value.clone()),
            _ => None,
        },
    }
}

fn apply_metrics(
    tokens: &BTreeMap<String, AuiThemeToken>,
    spec: &AuiThemeMetricsSpec,
    out: &mut AuiResolvedMetrics,
) {
    if let Some(value) = spec
        .font
        .as_ref()
        .and_then(|value| resolve_font(tokens, value))
    {
        out.font = Some(value);
    }
    if let Some(value) = spec
        .font_size
        .as_ref()
        .and_then(|value| resolve_length(tokens, value))
    {
        out.font_size = Some(value);
    }
    if let Some(value) = spec
        .line_height
        .as_ref()
        .and_then(|value| resolve_number(tokens, value))
    {
        out.line_height = Some(value);
    }
    if let Some(value) = spec
        .padding
        .as_ref()
        .and_then(|value| resolve_length(tokens, value))
    {
        out.padding = Some(value);
    }
    if let Some(value) = spec
        .min_width
        .as_ref()
        .and_then(|value| resolve_length(tokens, value))
    {
        out.min_width = Some(value);
    }
    if let Some(value) = spec
        .min_height
        .as_ref()
        .and_then(|value| resolve_length(tokens, value))
    {
        out.min_height = Some(value);
    }
    if let Some(value) = spec
        .border_inset
        .as_ref()
        .and_then(|value| resolve_length(tokens, value))
    {
        out.border_inset = Some(value);
    }
}

fn apply_paint(
    tokens: &BTreeMap<String, AuiThemeToken>,
    spec: &AuiThemePaintSpec,
    out: &mut AuiResolvedPaint,
) {
    if let Some(brush) = &spec.background {
        out.background = resolve_brush(tokens, brush);
    }
    if let Some(value) = spec
        .foreground
        .as_ref()
        .and_then(|value| resolve_color(tokens, value))
    {
        out.foreground = Some(value);
    }
    if let Some(border) = &spec.border {
        if let Some(value) = border
            .color
            .as_ref()
            .and_then(|value| resolve_color(tokens, value))
        {
            out.border.color = Some(value);
        }
        if let Some(value) = border
            .width
            .as_ref()
            .and_then(|value| resolve_length(tokens, value))
        {
            out.border.width = value;
        }
    }
    if let Some(value) = spec
        .radius
        .as_ref()
        .and_then(|value| resolve_length(tokens, value))
    {
        out.radius = value;
    }
    if let Some(shadow) = &spec.shadow {
        if let Some(value) = shadow
            .color
            .as_ref()
            .and_then(|value| resolve_color(tokens, value))
        {
            out.shadow.color = Some(value);
        }
        if let Some(value) = shadow
            .offset_x
            .as_ref()
            .and_then(|value| resolve_length_signed(tokens, value))
        {
            out.shadow.offset_x = value;
        }
        if let Some(value) = shadow
            .offset_y
            .as_ref()
            .and_then(|value| resolve_length_signed(tokens, value))
        {
            out.shadow.offset_y = value;
        }
        if let Some(value) = shadow
            .blur
            .as_ref()
            .and_then(|value| resolve_length(tokens, value))
        {
            out.shadow.blur = value;
        }
        if let Some(value) = shadow
            .spread
            .as_ref()
            .and_then(|value| resolve_length(tokens, value))
        {
            out.shadow.spread = value;
        }
    }
    if let Some(value) = spec
        .opacity
        .as_ref()
        .and_then(|value| resolve_number(tokens, value))
    {
        out.opacity = value.clamp(0.0, 1.0);
    }
}

fn resolve_length_signed(
    tokens: &BTreeMap<String, AuiThemeToken>,
    value: &AuiThemeValue<f32>,
) -> Option<f32> {
    let number = match value {
        AuiThemeValue::Value { value } => *value,
        AuiThemeValue::Token { token } => match tokens.get(token)? {
            AuiThemeToken::Length { value } => *value,
            _ => return None,
        },
    };
    number.is_finite().then_some(number)
}

fn resolve_brush(
    tokens: &BTreeMap<String, AuiThemeToken>,
    brush: &AuiBrushSpec,
) -> AuiResolvedBrush {
    match brush {
        AuiBrushSpec::None => AuiResolvedBrush::None,
        AuiBrushSpec::Solid { color } => resolve_color(tokens, color)
            .map(AuiResolvedBrush::solid)
            .unwrap_or_default(),
        AuiBrushSpec::Image { asset, tint } => AuiResolvedBrush::Image {
            asset_id: asset.id.clone(),
            tint: tint
                .as_ref()
                .and_then(|value| resolve_color(tokens, value))
                .unwrap_or_else(|| "#ffffffff".into()),
        },
        AuiBrushSpec::NineSlice { asset, tint, slice } => AuiResolvedBrush::NineSlice {
            asset_id: asset.id.clone(),
            tint: tint
                .as_ref()
                .and_then(|value| resolve_color(tokens, value))
                .unwrap_or_else(|| "#ffffffff".into()),
            slice: *slice,
        },
    }
}

fn apply_inline_style(node: &AuiNode, style: &mut AuiResolvedStyle) {
    let Some(inline) = &node.style else {
        return;
    };
    if let Some(color) = inline.color.as_deref().and_then(normalize_color) {
        style.paint.background = AuiResolvedBrush::solid(color);
    }
    if let Some(color) = inline.text_color.as_deref().and_then(normalize_color) {
        style.paint.foreground = Some(color);
    }
    if let Some(size) = inline
        .font_size
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        style.metrics.font_size = Some(size);
    }
}

fn apply_metrics_and_inline_style(node: &mut AuiNode, style: &AuiResolvedStyle) {
    let inline = node.style.clone();
    let current = node.style.get_or_insert(crate::aui::AuiStyle {
        color: None,
        text_color: None,
        font_size: None,
        font: None,
    });
    if inline.as_ref().and_then(|style| style.font_size).is_none() {
        current.font_size = style.metrics.font_size;
    }
    if inline
        .as_ref()
        .and_then(|style| style.font.as_ref())
        .is_none()
    {
        current.font = style
            .metrics
            .font
            .as_ref()
            .map(|font| crate::aui::AuiFontStyle {
                // FontAtlasProfile asset ids are the stable FontBundle ids produced by the font cooker.
                font_bundle_id: Some(font.id.clone()),
                font_family_id: None,
                style: crate::aui::AuiFontStyleKind::Normal,
                weight: 400,
                raster_policy: crate::aui::AuiFontRasterPolicy::AutoHybrid,
            });
    }
    if let Some(slot) = node.slot.as_mut() {
        if let Some(value) = style.metrics.min_width {
            slot.min_width = slot.min_width.max(value);
        }
        if let Some(value) = style.metrics.min_height {
            slot.min_height = slot.min_height.max(value);
        }
    }
    if let (Some(layout), Some(padding)) = (node.layout.as_mut(), style.metrics.padding) {
        layout.padding = Insets {
            left: padding,
            right: padding,
            top: padding,
            bottom: padding,
        };
    }
}

fn normalize_color(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#')?;
    if !(digits.len() == 6 || digits.len() == 8)
        || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some(format!(
        "#{}{}",
        digits.to_ascii_lowercase(),
        if digits.len() == 6 { "ff" } else { "" }
    ))
}

fn builtin_theme() -> AuiThemeAsset {
    let color = |value: &str| AuiThemeValue::Value {
        value: value.to_string(),
    };
    let length = |value| AuiThemeValue::Value { value };
    let mut styles = BTreeMap::new();
    let roles = [
        "panel",
        "image",
        "text",
        "button",
        "input",
        "toggle",
        "slider",
        "progress",
        "list",
        "scrollView",
        "custom",
    ];
    for role in roles {
        let interactive = matches!(role, "button" | "input" | "toggle" | "slider" | "list");
        let mut style = AuiThemeStyle {
            role: Some(role.to_string()),
            ..Default::default()
        };
        style.paint.foreground = Some(color(if role == "text" {
            "#f5f7faff"
        } else {
            "#eef2f7ff"
        }));
        if role != "text" && role != "image" && role != "custom" {
            style.paint.background = Some(AuiBrushSpec::Solid {
                color: color(if role == "button" {
                    "#2463aaff"
                } else {
                    "#202832ff"
                }),
            });
            style.paint.border = Some(AuiThemeBorderSpec {
                color: Some(color("#627080ff")),
                width: Some(length(1.0)),
            });
            style.paint.radius = Some(length(4.0));
        }
        if interactive {
            style.metrics.min_height = Some(length(32.0));
            style.states.hover = Some(AuiThemePaintSpec {
                opacity: Some(AuiThemeValue::Value { value: 0.92 }),
                ..Default::default()
            });
            style.states.pressed = Some(AuiThemePaintSpec {
                opacity: Some(AuiThemeValue::Value { value: 0.82 }),
                ..Default::default()
            });
            style.states.focus = Some(AuiThemePaintSpec {
                border: Some(AuiThemeBorderSpec {
                    color: Some(color("#66b3ffff")),
                    width: Some(length(2.0)),
                }),
                ..Default::default()
            });
            style.states.disabled = Some(AuiThemePaintSpec {
                opacity: Some(AuiThemeValue::Value { value: 0.48 }),
                ..Default::default()
            });
        }
        styles.insert(role.to_string(), style);
    }
    AuiThemeAsset {
        schema_version: AUI_THEME_ASSET_SCHEMA_VERSION.into(),
        asset_id: BUILTIN_AUI_THEME_ID.into(),
        extends: None,
        tokens: BTreeMap::new(),
        styles,
    }
}
