use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::{ProjectBrowserEntry, ProjectBrowserEntryKind, ProjectBrowserModel};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct EditorAssetRef {
    #[serde(rename = "id")]
    pub asset_id: String,
    #[serde(rename = "type")]
    pub asset_type_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(rename = "subAsset", default, skip_serializing_if = "Option::is_none")]
    pub sub_asset_id: Option<String>,
}

impl EditorAssetRef {
    pub fn new(asset_id: impl Into<String>, asset_type_id: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
            asset_type_id: asset_type_id.into(),
            guid: None,
            sub_asset_id: None,
        }
    }

    pub fn legacy(asset_id: impl Into<String>) -> Self {
        Self::new(asset_id, "asset")
    }

    pub fn display_id(&self) -> &str {
        &self.asset_id
    }
}

impl<'de> Deserialize<'de> for EditorAssetRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input {
            Structured {
                #[serde(rename = "id", alias = "assetId", alias = "asset_id")]
                asset_id: String,
                #[serde(
                    rename = "type",
                    alias = "assetType",
                    alias = "assetTypeId",
                    alias = "asset_type_id"
                )]
                asset_type_id: String,
                #[serde(default, alias = "assetGuid")]
                guid: Option<String>,
                #[serde(
                    rename = "subAsset",
                    default,
                    alias = "subAssetId",
                    alias = "sub_asset_id"
                )]
                sub_asset_id: Option<String>,
            },
            Legacy(String),
        }

        match Input::deserialize(deserializer)? {
            Input::Structured {
                asset_id,
                asset_type_id,
                guid,
                sub_asset_id,
            } => Ok(Self {
                asset_id,
                asset_type_id,
                guid,
                sub_asset_id,
            }),
            Input::Legacy(asset_id) => Ok(Self::legacy(asset_id)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "camelCase")]
pub enum AssetEntryKey {
    AuthoringAsset {
        asset_id: String,
        asset_type_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guid: Option<String>,
    },
    SourceFile {
        canonical_project_relative_path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_hash: Option<String>,
    },
    Folder {
        canonical_project_relative_path: String,
    },
}

impl AssetEntryKey {
    pub fn authoring(asset_ref: &EditorAssetRef) -> Self {
        Self::AuthoringAsset {
            asset_id: asset_ref.asset_id.clone(),
            asset_type_id: asset_ref.asset_type_id.clone(),
            guid: asset_ref.guid.clone(),
        }
    }

    pub fn source_file(path: impl Into<String>) -> Self {
        Self::SourceFile {
            canonical_project_relative_path: path.into(),
            content_hash: None,
        }
    }

    pub fn folder(path: impl Into<String>) -> Self {
        Self::Folder {
            canonical_project_relative_path: path.into(),
        }
    }

    pub fn canonical_path(&self) -> Option<&str> {
        match self {
            Self::AuthoringAsset { .. } => None,
            Self::SourceFile {
                canonical_project_relative_path,
                ..
            }
            | Self::Folder {
                canonical_project_relative_path,
            } => Some(canonical_project_relative_path),
        }
    }

    pub fn stable_token(&self) -> String {
        let raw = serde_json::to_string(self).unwrap_or_else(|_| "asset-entry-invalid".to_string());
        raw.as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn content_hash(&self) -> Option<&str> {
        match self {
            Self::SourceFile { content_hash, .. } => content_hash.as_deref(),
            Self::AuthoringAsset { .. } | Self::Folder { .. } => None,
        }
    }
}

impl Default for AssetEntryKey {
    fn default() -> Self {
        Self::source_file(String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetEntryRole {
    AuthoringAsset,
    SourceFile,
    Folder,
}

impl Default for AssetEntryRole {
    fn default() -> Self {
        Self::SourceFile
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetIdentityStatus {
    StableGuid,
    StableTypeAndId,
    Missing,
    Invalid,
    NotApplicable,
}

impl Default for AssetIdentityStatus {
    fn default() -> Self {
        Self::NotApplicable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetSourceStatus {
    Linked,
    Standalone,
    Missing,
    Invalid,
    NotApplicable,
}

impl Default for AssetSourceStatus {
    fn default() -> Self {
        Self::NotApplicable
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetBrowserModel {
    pub project_root: Option<String>,
    pub index_status: AssetBrowserIndexStatus,
    pub index_progress: AssetBrowserIndexProgress,
    pub scan_generation: u64,
    pub view_mode: AssetBrowserViewMode,
    pub current_folder: Option<String>,
    pub scroll_offset: f32,
    pub thumbnail_size: u32,
    pub query: AssetQuery,
    pub selection: AssetSelection,
    pub folder_entries: Vec<AssetBrowserEntry>,
    pub entries: Vec<AssetBrowserEntry>,
    #[serde(default)]
    pub picker: Option<AssetPickerModel>,
    pub report: AssetBrowserReport,
    pub empty_message: String,
}

impl AssetBrowserModel {
    pub fn empty() -> Self {
        Self {
            project_root: None,
            index_status: AssetBrowserIndexStatus::NotBuilt,
            index_progress: AssetBrowserIndexProgress::default(),
            scan_generation: 0,
            view_mode: AssetBrowserViewMode::List,
            current_folder: None,
            scroll_offset: 0.0,
            thumbnail_size: 96,
            query: AssetQuery::default(),
            selection: AssetSelection::default(),
            folder_entries: Vec::new(),
            entries: Vec::new(),
            picker: None,
            report: AssetBrowserReport::default(),
            empty_message: "No project is open.".to_string(),
        }
    }

    pub fn to_project_browser_model(&self) -> ProjectBrowserModel {
        ProjectBrowserModel {
            project_root: self.project_root.clone(),
            selected_path: self.selection.primary_path.clone(),
            entries: self
                .entries
                .iter()
                .map(|entry| {
                    ProjectBrowserEntry::new(
                        entry.path.clone(),
                        entry.label.clone(),
                        entry.kind.to_project_browser_kind(),
                        entry.exists,
                        entry.selected,
                        entry.openable,
                    )
                })
                .collect(),
            empty_message: self.empty_message.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetBrowserIndexStatus {
    NotBuilt,
    Scanning,
    Ready,
    Stale,
    Failed,
}

impl Default for AssetBrowserIndexStatus {
    fn default() -> Self {
        Self::NotBuilt
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBrowserIndexProgress {
    pub completed_units: usize,
    pub total_units: Option<usize>,
    pub phase: String,
}

impl Default for AssetBrowserIndexProgress {
    fn default() -> Self {
        Self {
            completed_units: 0,
            total_units: None,
            phase: "idle".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetBrowserViewMode {
    List,
    Grid,
}

impl Default for AssetBrowserViewMode {
    fn default() -> Self {
        Self::List
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetBrowserToolbarAction {
    Back,
    Forward,
    Up,
    Refresh,
    ToggleView,
    CycleTypeFilter,
    ClearSearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBrowserEntry {
    #[serde(default)]
    pub entry_key: AssetEntryKey,
    #[serde(default)]
    pub role: AssetEntryRole,
    #[serde(default)]
    pub canonical_path: String,
    pub asset_id: Option<String>,
    #[serde(default)]
    pub asset_type_id: Option<String>,
    pub guid: Option<String>,
    pub path: String,
    pub label: String,
    pub kind: AssetKind,
    pub exists: bool,
    pub imported: bool,
    pub openable: bool,
    pub placeable: bool,
    pub selectable: bool,
    pub selected: bool,
    #[serde(default)]
    pub identity_status: AssetIdentityStatus,
    #[serde(default)]
    pub source_status: AssetSourceStatus,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_asset_key: Option<AssetEntryKey>,
    pub preview: AssetPreviewDescriptor,
}

impl AssetBrowserEntry {
    pub fn new(path: impl Into<String>, label: impl Into<String>, kind: AssetKind) -> Self {
        let path = path.into();
        let role = if kind == AssetKind::Folder {
            AssetEntryRole::Folder
        } else {
            AssetEntryRole::SourceFile
        };
        let entry_key = if role == AssetEntryRole::Folder {
            AssetEntryKey::folder(path.clone())
        } else {
            AssetEntryKey::source_file(path.clone())
        };
        Self {
            entry_key,
            role,
            canonical_path: path.clone(),
            asset_id: None,
            asset_type_id: None,
            guid: None,
            path,
            label: label.into(),
            kind,
            exists: true,
            imported: true,
            openable: false,
            placeable: false,
            selectable: true,
            selected: false,
            identity_status: if role == AssetEntryRole::Folder {
                AssetIdentityStatus::NotApplicable
            } else {
                AssetIdentityStatus::Missing
            },
            source_status: if role == AssetEntryRole::Folder {
                AssetSourceStatus::NotApplicable
            } else {
                AssetSourceStatus::Standalone
            },
            source_path: None,
            source_asset_key: None,
            preview: AssetPreviewDescriptor::for_kind(kind),
        }
    }

    pub fn authoring(
        path: impl Into<String>,
        label: impl Into<String>,
        kind: AssetKind,
        asset_ref: EditorAssetRef,
    ) -> Self {
        let path = path.into();
        let mut entry = Self::new(path.clone(), label, kind);
        entry.entry_key = AssetEntryKey::authoring(&asset_ref);
        entry.role = AssetEntryRole::AuthoringAsset;
        entry.canonical_path = path;
        entry.asset_id = Some(asset_ref.asset_id.clone());
        entry.asset_type_id = Some(asset_ref.asset_type_id.clone());
        entry.guid = asset_ref.guid.clone();
        entry.identity_status = if asset_ref.guid.is_some() {
            AssetIdentityStatus::StableGuid
        } else {
            AssetIdentityStatus::StableTypeAndId
        };
        entry.source_status = AssetSourceStatus::NotApplicable;
        entry.openable = kind.openable_by_default();
        entry.placeable = kind.placeable_by_default();
        entry
    }

    pub fn editor_asset_ref(&self) -> Option<EditorAssetRef> {
        if self.role != AssetEntryRole::AuthoringAsset {
            return None;
        }
        Some(EditorAssetRef {
            asset_id: self.asset_id.clone()?,
            asset_type_id: self.asset_type_id.clone()?,
            guid: self.guid.clone(),
            sub_asset_id: None,
        })
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum AssetKind {
    Folder,
    Scene,
    Prefab,
    Texture,
    Sprite,
    Material,
    Rule,
    Aui,
    InputMapping,
    Font,
    Audio,
    BuildProfile,
    ProjectSettings,
    Unknown,
}

impl AssetKind {
    pub fn to_project_browser_kind(self) -> ProjectBrowserEntryKind {
        match self {
            Self::Folder => ProjectBrowserEntryKind::Folder,
            Self::Scene => ProjectBrowserEntryKind::Scene,
            Self::Unknown => ProjectBrowserEntryKind::Unknown,
            _ => ProjectBrowserEntryKind::Asset,
        }
    }

    pub fn openable_by_default(self) -> bool {
        matches!(
            self,
            Self::Scene
                | Self::Prefab
                | Self::Rule
                | Self::Aui
                | Self::InputMapping
                | Self::BuildProfile
                | Self::ProjectSettings
        )
    }

    pub fn placeable_by_default(self) -> bool {
        matches!(
            self,
            Self::Texture | Self::Sprite | Self::Material | Self::Prefab
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetQuery {
    pub search_text: String,
    pub folder: Option<String>,
    pub kinds: Vec<AssetKind>,
    pub include_missing: bool,
    pub include_unimported: bool,
}

impl AssetQuery {
    pub fn matches(&self, entry: &AssetBrowserEntry) -> bool {
        if !self.include_missing && !entry.exists {
            return false;
        }
        if !self.include_unimported && !entry.imported {
            return false;
        }
        if let Some(folder) = &self.folder {
            if !entry.path.starts_with(folder) {
                return false;
            }
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&entry.kind) {
            return false;
        }
        let search = self.search_text.trim().to_ascii_lowercase();
        search.is_empty()
            || entry.label.to_ascii_lowercase().contains(&search)
            || entry.path.to_ascii_lowercase().contains(&search)
            || entry
                .asset_id
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains(&search)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSelection {
    #[serde(default)]
    pub selected_entry_keys: Vec<AssetEntryKey>,
    #[serde(default)]
    pub primary_entry_key: Option<AssetEntryKey>,
    pub selected_paths: Vec<String>,
    pub primary_path: Option<String>,
    pub primary_asset_id: Option<String>,
}

impl AssetSelection {
    pub fn single(path: impl Into<String>, asset_id: Option<String>) -> Self {
        let path = path.into();
        Self {
            selected_entry_keys: Vec::new(),
            primary_entry_key: None,
            selected_paths: vec![path.clone()],
            primary_path: Some(path),
            primary_asset_id: asset_id,
        }
    }

    pub fn contains_path(&self, path: &str) -> bool {
        self.selected_paths.iter().any(|selected| selected == path)
    }

    pub fn contains_entry(&self, entry: &AssetBrowserEntry) -> bool {
        self.selected_entry_keys
            .iter()
            .any(|selected| selected == &entry.entry_key)
            || self.contains_path(&entry.path)
    }

    pub fn single_entry(entry: &AssetBrowserEntry) -> Self {
        Self {
            selected_entry_keys: vec![entry.entry_key.clone()],
            primary_entry_key: Some(entry.entry_key.clone()),
            selected_paths: vec![entry.path.clone()],
            primary_path: Some(entry.path.clone()),
            primary_asset_id: entry.asset_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetPreviewDescriptor {
    pub preview_kind: AssetPreviewKind,
    pub text: Option<String>,
    pub thumbnail_asset_id: Option<String>,
    #[serde(default)]
    pub thumbnail_source_path: Option<String>,
    #[serde(default)]
    pub thumbnail_id: Option<String>,
    #[serde(default)]
    pub thumbnail_aspect_ratio: Option<AssetThumbnailAspectRatio>,
    pub status: AssetPreviewStatus,
}

impl AssetPreviewDescriptor {
    pub fn for_kind(kind: AssetKind) -> Self {
        Self {
            preview_kind: match kind {
                AssetKind::Texture | AssetKind::Sprite | AssetKind::Material => {
                    AssetPreviewKind::Thumbnail
                }
                AssetKind::Folder => AssetPreviewKind::Folder,
                AssetKind::Unknown => AssetPreviewKind::None,
                _ => AssetPreviewKind::TextSummary,
            },
            text: None,
            thumbnail_asset_id: None,
            thumbnail_source_path: None,
            thumbnail_id: None,
            thumbnail_aspect_ratio: None,
            status: if matches!(
                kind,
                AssetKind::Texture | AssetKind::Sprite | AssetKind::Material
            ) {
                AssetPreviewStatus::Pending
            } else {
                AssetPreviewStatus::Ready
            },
        }
    }
}

impl Default for AssetPreviewDescriptor {
    fn default() -> Self {
        Self {
            preview_kind: AssetPreviewKind::None,
            text: None,
            thumbnail_asset_id: None,
            thumbnail_source_path: None,
            thumbnail_id: None,
            thumbnail_aspect_ratio: None,
            status: AssetPreviewStatus::NotAvailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetThumbnailAspectRatio {
    pub width: u32,
    pub height: u32,
}

impl AssetThumbnailAspectRatio {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        (width > 0 && height > 0).then_some(Self { width, height })
    }

    pub fn as_f32(self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetPreviewKind {
    None,
    Folder,
    Thumbnail,
    TextSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetPreviewStatus {
    Ready,
    Pending,
    Failed,
    NotAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetPickRequest {
    pub request_id: String,
    pub allowed_kinds: Vec<AssetKind>,
    #[serde(default)]
    pub allowed_asset_types: Vec<String>,
    #[serde(default)]
    pub target_kind: Option<AssetPickTargetKind>,
    pub target_path: Option<String>,
    #[serde(default)]
    pub target_object_id: Option<String>,
    pub target_field_path: Option<String>,
    #[serde(default)]
    pub current_asset_ref: Option<EditorAssetRef>,
    #[serde(default)]
    pub expected_source_revision: Option<u64>,
    #[serde(default)]
    pub expected_source_hash: Option<String>,
}

impl AssetPickRequest {
    pub fn new(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            allowed_kinds: Vec::new(),
            allowed_asset_types: Vec::new(),
            target_kind: None,
            target_path: None,
            target_object_id: None,
            target_field_path: None,
            current_asset_ref: None,
            expected_source_revision: None,
            expected_source_hash: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetPickTargetKind {
    SceneComponentField,
    AuiNodeField,
    BuildProfileField,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetPickResult {
    pub request_id: String,
    #[serde(default)]
    pub selected_entry_key: Option<AssetEntryKey>,
    pub asset_ref: Option<EditorAssetRef>,
    pub accepted: bool,
    pub diagnostics: Vec<AssetBrowserDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetPickerModel {
    pub request: AssetPickRequest,
    pub candidate: Option<AssetPickResult>,
    pub can_confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetDragPayload {
    #[serde(default)]
    pub entry_keys: Vec<AssetEntryKey>,
    pub asset_refs: Vec<EditorAssetRef>,
    pub source_panel: String,
    pub allowed_drop_targets: Vec<AssetDropTargetKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetDropTargetKind {
    Scene,
    InspectorField,
    ProjectFolder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetBrowserCommand {
    Select {
        entry_key: AssetEntryKey,
        additive: bool,
    },
    Open {
        entry_key: AssetEntryKey,
    },
    Pick {
        request: AssetPickRequest,
        entry_key: AssetEntryKey,
    },
    PlaceIntoScene {
        entry_key: AssetEntryKey,
    },
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBrowserReport {
    pub schema_version: String,
    pub asset_count: usize,
    pub folder_count: usize,
    pub selected_count: usize,
    pub missing_count: usize,
    pub unimported_count: usize,
    pub filtered_count: usize,
    pub diagnostics: Vec<AssetBrowserDiagnostic>,
}

impl Default for AssetBrowserReport {
    fn default() -> Self {
        Self {
            schema_version: "asset-browser-report.v1".to_string(),
            asset_count: 0,
            folder_count: 0,
            selected_count: 0,
            missing_count: 0,
            unimported_count: 0,
            filtered_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBrowserDiagnostic {
    pub severity: AssetBrowserDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl AssetBrowserDiagnostic {
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: AssetBrowserDiagnosticSeverity::Warning,
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
            severity: AssetBrowserDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetBrowserDiagnosticSeverity {
    Info,
    Warning,
    Error,
}
