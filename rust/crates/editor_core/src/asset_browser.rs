use crate::{
    import_generated_image_formally, AiImageGenerationRequest, AssetPlacementRequest,
    AuiAuthoringService, CommandResult, CommandStatus, CommandTransaction, EditorSession,
    ImageGenerationProvider, ImageKind, MockImageGenerationProvider, SceneEditCommand,
};
use editor_ui_model::{
    AssetBrowserDiagnostic, AssetBrowserEntry, AssetBrowserIndexProgress, AssetBrowserIndexStatus,
    AssetBrowserModel, AssetBrowserReport, AssetBrowserToolbarAction, AssetBrowserViewMode,
    AssetDragPayload, AssetDropTargetKind, AssetEntryKey, AssetEntryRole, AssetIdentityStatus,
    AssetKind, AssetPickRequest, AssetPickResult, AssetPickTargetKind, AssetPickerModel,
    AssetPlacementMode, AssetPreviewDescriptor, AssetQuery, AssetSelection, AssetSourceStatus,
    EditorAssetRef, UiCommandPayload, WorkspaceSelectionTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::asset_thumbnail::{
    AssetThumbnailCpuPayload, AssetThumbnailService, AssetThumbnailServiceSummary,
};

pub const ASSET_BROWSER_REPORT_SCHEMA_VERSION: &str = "asset-browser-report.v1";
pub const ASSET_BROWSER_NATIVE_PRODUCTIZATION_REPORT_SCHEMA_VERSION: &str =
    "asset-browser-native-productization-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetBrowserReportLevel {
    Off,
    Summary,
    Trace,
}

impl Default for AssetBrowserReportLevel {
    fn default() -> Self {
        Self::Summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBrowserBuildRequest {
    pub project_root: PathBuf,
    pub query: AssetQuery,
    pub selection: AssetSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBrowserIndexSnapshot {
    pub project_root: PathBuf,
    pub revision: u64,
    pub entries: Vec<AssetBrowserEntry>,
    pub source_fingerprint: String,
    pub scan_generation: u64,
    pub refreshed_at_epoch_ms: u128,
    pub dirty_reasons: Vec<String>,
    pub report: AssetBrowserReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetBrowserUiState {
    pub current_folder: Option<String>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub query: AssetQuery,
    pub view_mode: AssetBrowserViewMode,
    pub thumbnail_size: u32,
    pub scroll_offset: f32,
    pub selection: AssetSelection,
}

impl Default for AssetBrowserUiState {
    fn default() -> Self {
        Self {
            current_folder: None,
            history: Vec::new(),
            history_index: None,
            query: AssetQuery::default(),
            view_mode: AssetBrowserViewMode::List,
            thumbnail_size: 96,
            scroll_offset: 0.0,
            selection: AssetSelection::default(),
        }
    }
}

pub struct AssetBrowserSessionState {
    pub index_snapshot: Option<AssetBrowserIndexSnapshot>,
    pub index_status: AssetBrowserIndexStatus,
    pub index_progress: AssetBrowserIndexProgress,
    pub pending_refresh: bool,
    pub dirty_reasons: Vec<String>,
    pub ui_state: AssetBrowserUiState,
    pub scan_started_count: u64,
    pub scan_committed_count: u64,
    pub report_level: AssetBrowserReportLevel,
    pub active_picker: Option<AssetPickerSessionState>,
    pub last_pick_commit_plan: Option<AssetPickCommitPlan>,
    thumbnail_service: AssetThumbnailService,
    active_scan_reasons: Vec<String>,
    scan_receiver: Option<Receiver<Result<AssetBrowserIndexSnapshot, AssetBrowserDiagnostic>>>,
}

impl Default for AssetBrowserSessionState {
    fn default() -> Self {
        Self {
            index_snapshot: None,
            index_status: AssetBrowserIndexStatus::NotBuilt,
            index_progress: AssetBrowserIndexProgress::default(),
            pending_refresh: false,
            dirty_reasons: Vec::new(),
            ui_state: AssetBrowserUiState::default(),
            scan_started_count: 0,
            scan_committed_count: 0,
            report_level: AssetBrowserReportLevel::Summary,
            active_picker: None,
            last_pick_commit_plan: None,
            thumbnail_service: AssetThumbnailService::new(),
            active_scan_reasons: Vec::new(),
            scan_receiver: None,
        }
    }
}

impl AssetBrowserSessionState {
    pub fn initialize(&mut self, project_root: &Path) {
        self.index_snapshot = None;
        self.scan_receiver = None;
        self.thumbnail_service.clear();
        self.active_picker = None;
        self.last_pick_commit_plan = None;
        self.ui_state = AssetBrowserUiState {
            history: vec![String::new()],
            history_index: Some(0),
            ..AssetBrowserUiState::default()
        };
        self.pending_refresh = false;
        self.dirty_reasons = vec!["project_open".to_string()];
        self.active_scan_reasons = std::mem::take(&mut self.dirty_reasons);
        self.index_status = AssetBrowserIndexStatus::Scanning;
        self.index_progress = scanning_progress();
        self.scan_started_count = self.scan_started_count.saturating_add(1);
        match AssetBrowserIndex::scan(project_root) {
            Ok(snapshot) => self.commit_snapshot(snapshot),
            Err(diagnostic) => self.fail_scan(diagnostic),
        }
    }

    pub fn mark_dirty(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        if !self.dirty_reasons.contains(&reason) {
            self.dirty_reasons.push(reason);
        }
        self.pending_refresh = true;
        self.index_status = if self.index_snapshot.is_some() {
            AssetBrowserIndexStatus::Stale
        } else {
            AssetBrowserIndexStatus::NotBuilt
        };
    }

    pub fn request_refresh(&mut self, project_root: PathBuf) {
        if self.scan_receiver.is_some() {
            return;
        }
        self.pending_refresh = false;
        self.active_scan_reasons = std::mem::take(&mut self.dirty_reasons);
        self.index_status = AssetBrowserIndexStatus::Scanning;
        self.index_progress = scanning_progress();
        self.scan_started_count = self.scan_started_count.saturating_add(1);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(AssetBrowserIndex::scan(&project_root));
        });
        self.scan_receiver = Some(receiver);
    }

    pub fn refresh_now(&mut self, project_root: &Path, reason: impl Into<String>) {
        self.mark_dirty(reason);
        self.pending_refresh = false;
        self.scan_receiver = None;
        self.active_scan_reasons = std::mem::take(&mut self.dirty_reasons);
        self.index_status = AssetBrowserIndexStatus::Scanning;
        self.index_progress = scanning_progress();
        self.scan_started_count = self.scan_started_count.saturating_add(1);
        match AssetBrowserIndex::scan(project_root) {
            Ok(snapshot) => self.commit_snapshot(snapshot),
            Err(diagnostic) => self.fail_scan(diagnostic),
        }
    }

    pub fn pump(&mut self, project_root: Option<PathBuf>) -> bool {
        let Some(receiver) = self.scan_receiver.as_ref() else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(AssetBrowserDiagnostic::error(
                "asset_scan_worker_disconnected",
                "Asset Browser scan worker disconnected before producing a result.",
                None,
            )),
        };
        self.scan_receiver = None;
        match result {
            Ok(snapshot) => self.commit_snapshot(snapshot),
            Err(diagnostic) => self.fail_scan(diagnostic),
        }
        if self.pending_refresh {
            if let Some(project_root) = project_root {
                self.request_refresh(project_root);
            }
        }
        true
    }

    pub fn model(&self, query: AssetQuery, selection: AssetSelection) -> AssetBrowserModel {
        let Some(snapshot) = &self.index_snapshot else {
            let mut model = AssetBrowserModel::empty();
            model.index_status = self.index_status;
            model.index_progress = self.index_progress.clone();
            model.view_mode = self.ui_state.view_mode;
            model.current_folder = self.ui_state.current_folder.clone();
            model.thumbnail_size = self.ui_state.thumbnail_size;
            return model;
        };
        let mut model = AssetBrowserIndex::query(snapshot, query, selection);
        model.index_status = self.index_status;
        model.index_progress = self.index_progress.clone();
        model.view_mode = self.ui_state.view_mode;
        model.current_folder = self.ui_state.current_folder.clone();
        model.thumbnail_size = self.ui_state.thumbnail_size;
        self.thumbnail_service.decorate_model(
            &snapshot.project_root,
            &snapshot.entries,
            &mut model,
        );
        model.picker = self.active_picker.as_ref().map(|picker| AssetPickerModel {
            request: picker.request.clone(),
            candidate: picker.candidate.clone(),
            can_confirm: picker
                .candidate
                .as_ref()
                .is_some_and(|candidate| candidate.accepted),
        });
        model
    }

    pub fn pump_thumbnails(&mut self) -> bool {
        self.thumbnail_service.pump()
    }

    pub fn request_thumbnail_ids(&mut self, thumbnail_ids: &BTreeSet<String>) -> usize {
        let Some(snapshot) = self.index_snapshot.as_ref() else {
            return 0;
        };
        self.thumbnail_service.request_ids(
            &snapshot.project_root,
            &snapshot.entries,
            thumbnail_ids,
            self.ui_state.thumbnail_size,
        )
    }

    pub fn thumbnail_payloads_for_ids(
        &mut self,
        thumbnail_ids: &BTreeSet<String>,
    ) -> Vec<AssetThumbnailCpuPayload> {
        self.thumbnail_service.payloads_for_ids(thumbnail_ids)
    }

    pub fn thumbnail_summary(&self) -> AssetThumbnailServiceSummary {
        self.thumbnail_service.summary()
    }

    fn commit_snapshot(&mut self, mut snapshot: AssetBrowserIndexSnapshot) {
        let next_generation = self
            .index_snapshot
            .as_ref()
            .map_or(1, |current| current.scan_generation.saturating_add(1));
        snapshot.revision = next_generation;
        snapshot.scan_generation = next_generation;
        snapshot.dirty_reasons = std::mem::take(&mut self.active_scan_reasons);
        self.index_progress = AssetBrowserIndexProgress {
            completed_units: snapshot.entries.len(),
            total_units: Some(snapshot.entries.len()),
            phase: "ready".to_string(),
        };
        self.index_snapshot = Some(snapshot);
        self.index_status = AssetBrowserIndexStatus::Ready;
        self.scan_committed_count = self.scan_committed_count.saturating_add(1);
    }

    fn fail_scan(&mut self, diagnostic: AssetBrowserDiagnostic) {
        for reason in std::mem::take(&mut self.active_scan_reasons) {
            if !self.dirty_reasons.contains(&reason) {
                self.dirty_reasons.push(reason);
            }
        }
        if let Some(snapshot) = self.index_snapshot.as_mut() {
            snapshot.report.diagnostics.push(diagnostic);
            self.index_status = AssetBrowserIndexStatus::Stale;
        } else {
            self.index_status = AssetBrowserIndexStatus::Failed;
        }
        self.index_progress.phase = "failed".to_string();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetPickerSessionState {
    pub request: AssetPickRequest,
    pub candidate: Option<AssetPickResult>,
    pub previous_query: AssetQuery,
    pub previous_selection: AssetSelection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetPickCommitPlan {
    pub request_id: String,
    pub target_kind: AssetPickTargetKind,
    pub target_document_path: String,
    pub target_object_id: String,
    pub target_field_path: String,
    pub old_asset_ref: Option<EditorAssetRef>,
    pub new_asset_ref: EditorAssetRef,
    pub expected_source_revision: Option<u64>,
    pub expected_source_hash: Option<String>,
    pub lowered_domain_command: UiCommandPayload,
}

fn scanning_progress() -> AssetBrowserIndexProgress {
    AssetBrowserIndexProgress {
        completed_units: 0,
        total_units: None,
        phase: "scanning".to_string(),
    }
}

pub struct AssetBrowserIndex;

impl AssetBrowserIndex {
    pub fn validate_project_path(
        project_root: &Path,
        path: &Path,
    ) -> Result<String, AssetBrowserDiagnostic> {
        canonical_project_relative_path(project_root, path)
    }

    pub fn scan(project_root: &Path) -> Result<AssetBrowserIndexSnapshot, AssetBrowserDiagnostic> {
        if !project_root.is_dir() {
            return Err(AssetBrowserDiagnostic::error(
                "asset_project_root_missing",
                "Asset Browser cannot scan a missing project root.",
                Some(project_root.display().to_string()),
            ));
        }
        let model = Self::build(AssetBrowserBuildRequest {
            project_root: project_root.to_path_buf(),
            query: AssetQuery {
                include_missing: true,
                include_unimported: true,
                ..AssetQuery::default()
            },
            selection: AssetSelection::default(),
        });
        let source_fingerprint = fingerprint_entries(&model.entries);
        Ok(AssetBrowserIndexSnapshot {
            project_root: project_root.to_path_buf(),
            revision: 0,
            entries: model.entries,
            source_fingerprint,
            scan_generation: 0,
            refreshed_at_epoch_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            dirty_reasons: Vec::new(),
            report: model.report,
        })
    }

    pub fn query(
        snapshot: &AssetBrowserIndexSnapshot,
        query: AssetQuery,
        selection: AssetSelection,
    ) -> AssetBrowserModel {
        let total_count = snapshot.entries.len();
        let folder_entries = snapshot
            .entries
            .iter()
            .filter(|entry| entry.role == AssetEntryRole::Folder)
            .cloned()
            .collect::<Vec<_>>();
        let mut entries = snapshot
            .entries
            .iter()
            .filter(|entry| query.matches(entry))
            .cloned()
            .map(|mut entry| {
                entry.selected = selection.contains_entry(&entry);
                entry
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            a.path
                .to_ascii_lowercase()
                .cmp(&b.path.to_ascii_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        let mut report = snapshot.report.clone();
        report.selected_count = if selection.selected_entry_keys.is_empty() {
            selection.selected_paths.len()
        } else {
            selection.selected_entry_keys.len()
        };
        report.filtered_count = total_count.saturating_sub(entries.len());
        AssetBrowserModel {
            project_root: Some(snapshot.project_root.display().to_string()),
            index_status: AssetBrowserIndexStatus::Ready,
            index_progress: AssetBrowserIndexProgress {
                completed_units: total_count,
                total_units: Some(total_count),
                phase: "ready".to_string(),
            },
            scan_generation: snapshot.scan_generation,
            view_mode: AssetBrowserViewMode::List,
            current_folder: query.folder.clone(),
            scroll_offset: 0.0,
            thumbnail_size: 96,
            query,
            selection,
            folder_entries,
            entries,
            picker: None,
            report,
            empty_message: "No assets match the current query.".to_string(),
        }
    }

    pub fn build(request: AssetBrowserBuildRequest) -> AssetBrowserModel {
        let mut all_entries = base_entries(&request.project_root);
        let mut diagnostics = Vec::new();
        scan_known_roots(&request.project_root, &mut all_entries, &mut diagnostics);
        link_source_relations(&request.project_root, &mut all_entries, &mut diagnostics);
        let mut report = report_for_entries(&all_entries, &request.selection, diagnostics);
        let total_count = all_entries.len();
        let folder_entries = all_entries
            .iter()
            .filter(|entry| entry.role == AssetEntryRole::Folder)
            .cloned()
            .collect::<Vec<_>>();
        let mut entries = all_entries
            .into_iter()
            .filter(|entry| request.query.matches(entry))
            .map(|mut entry| {
                entry.selected = request.selection.contains_entry(&entry);
                entry
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            a.path
                .to_ascii_lowercase()
                .cmp(&b.path.to_ascii_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        report.filtered_count = total_count.saturating_sub(entries.len());
        AssetBrowserModel {
            project_root: Some(request.project_root.display().to_string()),
            index_status: AssetBrowserIndexStatus::Ready,
            index_progress: AssetBrowserIndexProgress {
                completed_units: total_count,
                total_units: Some(total_count),
                phase: "ready".to_string(),
            },
            scan_generation: 1,
            view_mode: AssetBrowserViewMode::List,
            current_folder: request.query.folder.clone(),
            scroll_offset: 0.0,
            thumbnail_size: 96,
            query: request.query,
            selection: request.selection,
            folder_entries,
            entries,
            picker: None,
            report,
            empty_message: "No assets match the current query.".to_string(),
        }
    }
}

pub struct AssetBrowserService;

impl AssetBrowserService {
    pub fn pick(
        model: &AssetBrowserModel,
        request: AssetPickRequest,
        path: &str,
    ) -> AssetPickResult {
        let Some(entry) = model.entries.iter().find(|entry| entry.path == path) else {
            return AssetPickResult {
                request_id: request.request_id,
                selected_entry_key: None,
                asset_ref: None,
                accepted: false,
                diagnostics: vec![AssetBrowserDiagnostic::error(
                    "asset_missing",
                    format!("Asset is missing from browser model: {path}"),
                    Some(path.to_string()),
                )],
            };
        };
        Self::pick_entry(model, request, &entry.entry_key)
    }

    pub fn pick_entry(
        model: &AssetBrowserModel,
        request: AssetPickRequest,
        entry_key: &AssetEntryKey,
    ) -> AssetPickResult {
        let Some(entry) = model
            .entries
            .iter()
            .find(|entry| &entry.entry_key == entry_key)
        else {
            return AssetPickResult {
                request_id: request.request_id,
                selected_entry_key: Some(entry_key.clone()),
                asset_ref: None,
                accepted: false,
                diagnostics: vec![AssetBrowserDiagnostic::error(
                    "asset_missing",
                    "Asset is missing from the current cached browser model.",
                    None,
                )],
            };
        };
        if !request.allowed_kinds.is_empty() && !request.allowed_kinds.contains(&entry.kind) {
            return AssetPickResult {
                request_id: request.request_id,
                selected_entry_key: Some(entry.entry_key.clone()),
                asset_ref: None,
                accepted: false,
                diagnostics: vec![AssetBrowserDiagnostic::error(
                    "asset_type_mismatch",
                    format!(
                        "Asset kind {:?} is not allowed for this picker.",
                        entry.kind
                    ),
                    Some(entry.path.clone()),
                )],
            };
        }
        let Some(asset_ref) = entry.editor_asset_ref() else {
            return AssetPickResult {
                request_id: request.request_id,
                selected_entry_key: Some(entry.entry_key.clone()),
                asset_ref: None,
                accepted: false,
                diagnostics: vec![AssetBrowserDiagnostic::error(
                    "asset_identity_required",
                    "Source files and folders cannot be picked as authoring AssetRef values.",
                    Some(entry.canonical_path.clone()),
                )],
            };
        };
        if !request.allowed_asset_types.is_empty()
            && !request
                .allowed_asset_types
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&asset_ref.asset_type_id))
        {
            return AssetPickResult {
                request_id: request.request_id,
                selected_entry_key: Some(entry.entry_key.clone()),
                asset_ref: None,
                accepted: false,
                diagnostics: vec![AssetBrowserDiagnostic::error(
                    "asset_type_id_mismatch",
                    format!(
                        "Asset type '{}' is not allowed for this picker.",
                        asset_ref.asset_type_id
                    ),
                    Some(entry.path.clone()),
                )],
            };
        }
        AssetPickResult {
            request_id: request.request_id,
            selected_entry_key: Some(entry.entry_key.clone()),
            asset_ref: Some(asset_ref),
            accepted: true,
            diagnostics: Vec::new(),
        }
    }

    pub fn drag_payload(entries: &[AssetBrowserEntry]) -> AssetDragPayload {
        AssetDragPayload {
            entry_keys: entries
                .iter()
                .map(|entry| entry.entry_key.clone())
                .collect(),
            asset_refs: entries
                .iter()
                .filter_map(AssetBrowserEntry::editor_asset_ref)
                .collect(),
            source_panel: "asset_browser".to_string(),
            allowed_drop_targets: vec![
                AssetDropTargetKind::Scene,
                AssetDropTargetKind::InspectorField,
                AssetDropTargetKind::ProjectFolder,
            ],
        }
    }

    pub fn placement_request_from_reference(
        reference: &EditorAssetRef,
        target_parent_id: Option<String>,
        local_position: Option<crate::EditorVec3>,
        placement_mode: AssetPlacementMode,
    ) -> Result<AssetPlacementRequest, AssetBrowserDiagnostic> {
        let kind = kind_for_asset_type(&reference.asset_type_id);
        if !kind.placeable_by_default() {
            return Err(AssetBrowserDiagnostic::error(
                "asset_place_unsupported",
                format!(
                    "Asset kind {:?} cannot be placed into Scene.",
                    reference.asset_type_id
                ),
                Some(reference.asset_id.clone()),
            ));
        }
        Ok(AssetPlacementRequest {
            asset_id: reference.asset_id.clone(),
            asset_type: reference.asset_type_id.clone(),
            asset_guid: reference.guid.clone(),
            target_parent_id,
            local_position,
            placement_mode,
        })
    }
}

const KNOWN_ASSET_ROOTS: &[&str] = &[
    "Assets",
    "Scenes",
    "Prefabs",
    "Rules",
    "AUI",
    "Input",
    "BuildProfiles",
    "Settings",
    "UI",
];

const GENERATED_ROOTS: &[&str] = &["Build", "Reports", "dist", "exports", "release", "target"];

fn base_entries(project_root: &Path) -> Vec<AssetBrowserEntry> {
    KNOWN_ASSET_ROOTS
        .iter()
        .map(|path| {
            let mut entry = AssetBrowserEntry::new(*path, *path, AssetKind::Folder);
            entry.exists = project_root.join(path).is_dir();
            entry.imported = true;
            entry.identity_status = AssetIdentityStatus::NotApplicable;
            entry.source_status = AssetSourceStatus::NotApplicable;
            entry
        })
        .collect()
}

fn scan_known_roots(
    project_root: &Path,
    entries: &mut Vec<AssetBrowserEntry>,
    diagnostics: &mut Vec<AssetBrowserDiagnostic>,
) {
    let mut visited = BTreeSet::new();
    for root in KNOWN_ASSET_ROOTS {
        let absolute = project_root.join(root);
        if !absolute.is_dir() {
            continue;
        }
        scan_directory(project_root, &absolute, entries, diagnostics, &mut visited);
    }
}

fn scan_directory(
    project_root: &Path,
    directory: &Path,
    entries: &mut Vec<AssetBrowserEntry>,
    diagnostics: &mut Vec<AssetBrowserDiagnostic>,
    visited: &mut BTreeSet<PathBuf>,
) {
    let canonical_directory = match canonical_inside_project(project_root, directory) {
        Ok(path) => path,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    if !visited.insert(canonical_directory) {
        diagnostics.push(AssetBrowserDiagnostic::warning(
            "asset_path_cycle",
            "Skipped an already visited asset directory.",
            canonical_project_relative_path(project_root, directory).ok(),
        ));
        return;
    }

    let read_dir = match fs::read_dir(directory) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            diagnostics.push(AssetBrowserDiagnostic::error(
                "asset_directory_read_failed",
                format!("Failed to read asset directory: {error}"),
                canonical_project_relative_path(project_root, directory).ok(),
            ));
            return;
        }
    };
    let mut items = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    items.sort_by_key(|item| item.path());
    for item in items {
        let path = item.path();
        let relative = match canonical_project_relative_path(project_root, &path) {
            Ok(relative) => relative,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        if is_generated_path(&relative) {
            continue;
        }
        if relative.to_ascii_lowercase().ends_with(".meta.json") {
            continue;
        }
        if path.is_dir() {
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(relative.as_str());
            entries.push(AssetBrowserEntry::new(
                relative.clone(),
                label,
                AssetKind::Folder,
            ));
            scan_directory(project_root, &path, entries, diagnostics, visited);
        } else {
            match file_entry(project_root, &path, &relative) {
                Ok(entry) => entries.push(entry),
                Err(diagnostic) => {
                    let mut entry = AssetBrowserEntry::new(
                        relative.clone(),
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(relative.as_str()),
                        AssetKind::Unknown,
                    );
                    entry.imported = false;
                    entry.identity_status = AssetIdentityStatus::Invalid;
                    entries.push(entry);
                    diagnostics.push(diagnostic);
                }
            }
        }
    }
}

fn file_entry(
    project_root: &Path,
    path: &Path,
    relative: &str,
) -> Result<AssetBrowserEntry, AssetBrowserDiagnostic> {
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(relative)
        .to_string();
    if let Some(kind) = source_kind_for_path(relative) {
        let bytes = fs::read(path).map_err(|error| {
            AssetBrowserDiagnostic::error(
                "asset_source_read_failed",
                format!("Failed to read source asset: {error}"),
                Some(relative.to_string()),
            )
        })?;
        let mut entry = AssetBrowserEntry::new(relative, label, kind);
        entry.entry_key = AssetEntryKey::SourceFile {
            canonical_project_relative_path: relative.to_string(),
            content_hash: Some(stable_content_hash(&bytes)),
        };
        entry.imported = true;
        entry.identity_status = AssetIdentityStatus::NotApplicable;
        entry.source_status = AssetSourceStatus::Standalone;
        entry.preview = preview_for_entry(&entry);
        entry.preview.thumbnail_source_path = Some(relative.to_string());
        return Ok(entry);
    }

    let document = read_typed_asset_document(path, relative)?;
    let Some((kind, asset_type_id, asset_id)) = typed_identity(&document) else {
        let mut entry = AssetBrowserEntry::new(relative, label, AssetKind::Unknown);
        entry.imported = false;
        entry.identity_status = AssetIdentityStatus::Invalid;
        return Ok(entry);
    };
    let asset_ref = EditorAssetRef {
        asset_id,
        asset_type_id,
        guid: document.guid.or(document.asset_guid),
        sub_asset_id: None,
    };
    let mut entry = AssetBrowserEntry::authoring(relative, label, kind, asset_ref);
    entry.imported = true;
    if let Some(source_image) = document.source_image {
        let source_path = normalize_document_source_path(project_root, &source_image)?;
        entry.source_path = Some(source_path.clone());
        entry.source_status = AssetSourceStatus::Missing;
        entry.preview.thumbnail_source_path = Some(source_path);
    }
    entry.preview = preview_for_entry(&entry);
    if let Some(source_path) = entry.source_path.clone() {
        entry.preview.thumbnail_source_path = Some(source_path);
    }
    Ok(entry)
}

#[derive(Debug, Default, Deserialize)]
struct TypedAssetDocument {
    #[serde(rename = "schemaVersion", alias = "schema_version", default)]
    schema_version: String,
    #[serde(rename = "assetId", alias = "asset_id", default)]
    asset_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "prefabId", alias = "prefab_id", default)]
    prefab_id: Option<String>,
    #[serde(rename = "documentId", alias = "document_id", default)]
    document_id: Option<String>,
    #[serde(default)]
    guid: Option<String>,
    #[serde(rename = "assetGuid", alias = "asset_guid", default)]
    asset_guid: Option<String>,
    #[serde(rename = "sourceImage", alias = "source_image", default)]
    source_image: Option<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    target: Option<String>,
}

fn read_typed_asset_document(
    path: &Path,
    relative: &str,
) -> Result<TypedAssetDocument, AssetBrowserDiagnostic> {
    let bytes = fs::read(path).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_document_read_failed",
            format!("Failed to read asset document: {error}"),
            Some(relative.to_string()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        AssetBrowserDiagnostic::warning(
            "asset_document_unrecognized",
            format!("File is not a recognized typed asset document: {error}"),
            Some(relative.to_string()),
        )
    })
}

fn typed_identity(document: &TypedAssetDocument) -> Option<(AssetKind, String, String)> {
    let schema = document.schema_version.as_str();
    let (kind, asset_type_id, asset_id) = if schema == "editor-scene-document.v1" {
        (AssetKind::Scene, "scene", document.id.clone())
    } else if schema == "authoring-prefab-asset.v1" {
        (AssetKind::Prefab, "prefab", document.prefab_id.clone())
    } else if schema == "project-rule-asset.v1" {
        (AssetKind::Rule, "rule", document.asset_id.clone())
    } else if schema == "aui-document.v1" {
        (AssetKind::Aui, "aui", document.document_id.clone())
    } else if schema.starts_with("input-mapping.v") {
        (
            AssetKind::InputMapping,
            "input_mapping",
            document.asset_id.clone(),
        )
    } else if schema.starts_with("texture-asset.v") {
        (AssetKind::Texture, "texture", document.asset_id.clone())
    } else if schema.starts_with("sprite-asset.v") {
        (AssetKind::Sprite, "sprite", document.asset_id.clone())
    } else if schema.starts_with("material-asset.v") {
        (AssetKind::Material, "material", document.asset_id.clone())
    } else if schema.starts_with("font-asset.v") {
        (AssetKind::Font, "font", document.asset_id.clone())
    } else if schema.starts_with("audio-asset.v") {
        (AssetKind::Audio, "audio", document.asset_id.clone())
    } else if schema == "build-profile.v1" {
        let id = match (&document.target, &document.profile) {
            (Some(target), Some(profile)) => Some(format!("{target}.{profile}")),
            _ => None,
        };
        (AssetKind::BuildProfile, "build_profile", id)
    } else if schema == "aife-project-settings.v1" {
        (
            AssetKind::ProjectSettings,
            "project_settings",
            Some("project.settings".to_string()),
        )
    } else {
        return None;
    };
    let asset_id = asset_id?.trim().to_string();
    if asset_id.is_empty() {
        return None;
    }
    Some((kind, asset_type_id.to_string(), asset_id))
}

fn source_kind_for_path(path: &str) -> Option<AssetKind> {
    let lower = path.to_ascii_lowercase();
    if [".png", ".jpg", ".jpeg", ".webp"]
        .iter()
        .any(|extension| lower.ends_with(extension))
    {
        Some(AssetKind::Texture)
    } else if [".wav", ".ogg", ".mp3", ".flac"]
        .iter()
        .any(|extension| lower.ends_with(extension))
    {
        Some(AssetKind::Audio)
    } else {
        None
    }
}

fn report_for_entries(
    entries: &[AssetBrowserEntry],
    selection: &AssetSelection,
    diagnostics: Vec<AssetBrowserDiagnostic>,
) -> AssetBrowserReport {
    let mut report = AssetBrowserReport {
        schema_version: ASSET_BROWSER_REPORT_SCHEMA_VERSION.to_string(),
        asset_count: entries
            .iter()
            .filter(|entry| entry.kind != AssetKind::Folder)
            .count(),
        folder_count: entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Folder)
            .count(),
        selected_count: if selection.selected_entry_keys.is_empty() {
            selection.selected_paths.len()
        } else {
            selection.selected_entry_keys.len()
        },
        missing_count: entries.iter().filter(|entry| !entry.exists).count(),
        unimported_count: entries.iter().filter(|entry| !entry.imported).count(),
        filtered_count: 0,
        diagnostics,
    };
    for entry in entries {
        if !entry.exists {
            report.diagnostics.push(AssetBrowserDiagnostic::warning(
                "asset_missing",
                format!("Asset path is missing: {}", entry.path),
                Some(entry.path.clone()),
            ));
        } else if !entry.imported {
            report.diagnostics.push(AssetBrowserDiagnostic::warning(
                "asset_unimported",
                format!("Asset is not imported yet: {}", entry.path),
                Some(entry.path.clone()),
            ));
        }
        if entry.role == AssetEntryRole::AuthoringAsset && entry.guid.is_none() {
            report.diagnostics.push(AssetBrowserDiagnostic::warning(
                "asset_identity_guid_missing",
                format!(
                    "Asset '{}' uses stable (type,id) identity because no guid is present.",
                    entry.asset_id.as_deref().unwrap_or("<missing>")
                ),
                Some(entry.canonical_path.clone()),
            ));
        }
    }
    report
}

fn preview_for_entry(entry: &AssetBrowserEntry) -> AssetPreviewDescriptor {
    let mut preview = AssetPreviewDescriptor::for_kind(entry.kind);
    preview.text = Some(format!("{:?}: {}", entry.kind, entry.path));
    if entry.role == AssetEntryRole::AuthoringAsset
        && matches!(entry.kind, AssetKind::Texture | AssetKind::Sprite)
    {
        preview.thumbnail_asset_id = entry.asset_id.clone();
    }
    preview
}

fn kind_for_asset_type(asset_type_id: &str) -> AssetKind {
    match asset_type_id.trim().to_ascii_lowercase().as_str() {
        "scene" => AssetKind::Scene,
        "prefab" => AssetKind::Prefab,
        "rule" => AssetKind::Rule,
        "aui" => AssetKind::Aui,
        "input_mapping" => AssetKind::InputMapping,
        "texture" => AssetKind::Texture,
        "sprite" => AssetKind::Sprite,
        "material" => AssetKind::Material,
        "font" => AssetKind::Font,
        "audio" => AssetKind::Audio,
        "build_profile" => AssetKind::BuildProfile,
        "project_settings" => AssetKind::ProjectSettings,
        _ => AssetKind::Unknown,
    }
}

fn link_source_relations(
    project_root: &Path,
    entries: &mut [AssetBrowserEntry],
    diagnostics: &mut Vec<AssetBrowserDiagnostic>,
) {
    let by_path = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.canonical_path.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let links = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            entry
                .source_path
                .clone()
                .map(|source_path| (index, source_path, entry.entry_key.clone()))
        })
        .collect::<Vec<_>>();

    for (authoring_index, source_path, authoring_key) in links {
        let normalized = match normalize_document_source_path(project_root, &source_path) {
            Ok(path) => path,
            Err(diagnostic) => {
                entries[authoring_index].source_status = AssetSourceStatus::Invalid;
                diagnostics.push(diagnostic);
                continue;
            }
        };
        entries[authoring_index].source_path = Some(normalized.clone());
        let Some(source_index) = by_path.get(&normalized).copied() else {
            entries[authoring_index].source_status = AssetSourceStatus::Missing;
            diagnostics.push(AssetBrowserDiagnostic::warning(
                "asset_source_missing",
                format!("Linked source file does not exist: {normalized}"),
                Some(entries[authoring_index].canonical_path.clone()),
            ));
            continue;
        };
        if entries[source_index].role != AssetEntryRole::SourceFile {
            entries[authoring_index].source_status = AssetSourceStatus::Invalid;
            diagnostics.push(AssetBrowserDiagnostic::error(
                "asset_source_role_invalid",
                "An authoring asset source relation must target a SourceFile entry.",
                Some(normalized),
            ));
            continue;
        }
        entries[authoring_index].source_status = AssetSourceStatus::Linked;
        entries[source_index].source_status = AssetSourceStatus::Linked;
        entries[source_index].source_asset_key = Some(authoring_key);
    }
}

fn is_generated_path(relative: &str) -> bool {
    relative.split('/').next().is_some_and(|root| {
        GENERATED_ROOTS
            .iter()
            .any(|item| item.eq_ignore_ascii_case(root))
    })
}

fn canonical_inside_project(
    project_root: &Path,
    path: &Path,
) -> Result<PathBuf, AssetBrowserDiagnostic> {
    let canonical_root = fs::canonicalize(project_root).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_project_root_invalid",
            format!("Failed to resolve project root: {error}"),
            Some(project_root.display().to_string()),
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        if path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        }) {
            return Err(AssetBrowserDiagnostic::error(
                "asset_path_traversal",
                "Asset path contains traversal or an absolute-root component.",
                Some(path.display().to_string()),
            ));
        }
        project_root.join(path)
    };
    let canonical_candidate = fs::canonicalize(&candidate).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_path_resolve_failed",
            format!("Failed to resolve asset path: {error}"),
            Some(candidate.display().to_string()),
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(AssetBrowserDiagnostic::error(
            "asset_path_root_escape",
            "Asset path resolves outside the project root.",
            Some(candidate.display().to_string()),
        ));
    }
    Ok(canonical_candidate)
}

fn canonical_project_relative_path(
    project_root: &Path,
    path: &Path,
) -> Result<String, AssetBrowserDiagnostic> {
    let canonical_root = fs::canonicalize(project_root).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_project_root_invalid",
            format!("Failed to resolve project root: {error}"),
            Some(project_root.display().to_string()),
        )
    })?;
    let canonical_path = canonical_inside_project(project_root, path)?;
    let relative = canonical_path.strip_prefix(&canonical_root).map_err(|_| {
        AssetBrowserDiagnostic::error(
            "asset_path_root_escape",
            "Asset path cannot be represented relative to the project root.",
            Some(path.display().to_string()),
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_document_source_path(
    project_root: &Path,
    source_path: &str,
) -> Result<String, AssetBrowserDiagnostic> {
    let source = Path::new(source_path);
    if source.is_absolute()
        || source.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(AssetBrowserDiagnostic::error(
            "asset_source_path_traversal",
            "Asset source path must be project-relative and cannot contain traversal.",
            Some(source_path.to_string()),
        ));
    }
    let normalized = source
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() || is_generated_path(&normalized) {
        return Err(AssetBrowserDiagnostic::error(
            "asset_source_path_invalid",
            "Asset source path is empty or points at a generated root.",
            Some(source_path.to_string()),
        ));
    }
    let absolute = project_root.join(&normalized);
    if absolute.exists() {
        canonical_project_relative_path(project_root, &absolute)
    } else {
        Ok(normalized)
    }
}

pub(crate) fn stable_content_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn source_file_hash(path: &Path) -> Result<String, AssetBrowserDiagnostic> {
    let bytes = fs::read(path).map_err(|error| {
        AssetBrowserDiagnostic::error(
            "asset_picker_source_read_failed",
            format!("Failed to read picker target source: {error}"),
            Some(path.display().to_string()),
        )
    })?;
    Ok(stable_content_hash(&bytes))
}

fn fingerprint_entries(entries: &[AssetBrowserEntry]) -> String {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend_from_slice(entry.entry_key.stable_token().as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(entry.canonical_path.as_bytes());
        bytes.push(0xff);
    }
    stable_content_hash(&bytes)
}

fn relative_project_path(project_root: &Path, path: &Path) -> String {
    canonical_project_relative_path(project_root, path)
        .unwrap_or_else(|_| "<invalid-asset-path>".to_string())
}

impl EditorSession {
    pub fn asset_browser_state(&self) -> &AssetBrowserSessionState {
        &self.asset_browser_state
    }

    pub fn set_asset_browser_report_level(&mut self, level: AssetBrowserReportLevel) {
        self.asset_browser_state.report_level = level;
    }

    pub fn pump_asset_browser_refresh(&mut self) -> bool {
        let project_root = self
            .active_project_session
            .as_ref()
            .map(|session| session.project_root.clone());
        let scan_changed = self.asset_browser_state.pump(project_root);
        let thumbnail_changed = self.asset_browser_state.pump_thumbnails();
        scan_changed || thumbnail_changed
    }

    pub fn request_asset_thumbnail_ids(&mut self, thumbnail_ids: &BTreeSet<String>) -> usize {
        self.asset_browser_state
            .request_thumbnail_ids(thumbnail_ids)
    }

    pub fn asset_thumbnail_payloads_for_ids(
        &mut self,
        thumbnail_ids: &BTreeSet<String>,
    ) -> Vec<AssetThumbnailCpuPayload> {
        self.asset_browser_state
            .thumbnail_payloads_for_ids(thumbnail_ids)
    }

    pub fn asset_thumbnail_summary(&self) -> AssetThumbnailServiceSummary {
        self.asset_browser_state.thumbnail_summary()
    }

    pub fn request_asset_browser_refresh(&mut self, reason: impl Into<String>) {
        let Some(project_root) = self
            .active_project_session
            .as_ref()
            .map(|session| session.project_root.clone())
        else {
            return;
        };
        self.asset_browser_state.mark_dirty(reason);
        self.asset_browser_state.request_refresh(project_root);
    }

    pub fn refresh_asset_browser_now(&mut self, reason: impl Into<String>) {
        let Some(project_root) = self
            .active_project_session
            .as_ref()
            .map(|session| session.project_root.clone())
        else {
            return;
        };
        self.asset_browser_state.refresh_now(&project_root, reason);
    }

    pub fn set_asset_browser_query(&mut self, query: AssetQuery) {
        self.asset_browser_state.ui_state.current_folder = query.folder.clone();
        self.asset_browser_state.ui_state.query = query;
    }

    pub fn set_asset_browser_view_mode(&mut self, view_mode: AssetBrowserViewMode) {
        self.asset_browser_state.ui_state.view_mode = view_mode;
    }

    pub(crate) fn initialize_asset_browser(&mut self) {
        let Some(project_root) = self
            .active_project_session
            .as_ref()
            .map(|session| session.project_root.clone())
        else {
            self.asset_browser_state = AssetBrowserSessionState::default();
            return;
        };
        self.asset_browser_state.initialize(&project_root);
    }

    pub(crate) fn select_asset_browser_entry(
        &mut self,
        transaction: &mut CommandTransaction,
        entry_key: AssetEntryKey,
        additive: bool,
        range: bool,
    ) -> CommandResult {
        let query = self.asset_browser_state.ui_state.query.clone();
        let current_selection = self.asset_browser_state.ui_state.selection.clone();
        let model = self
            .asset_browser_state
            .model(query, current_selection.clone());
        let Some(entry) = model
            .entries
            .iter()
            .find(|entry| entry.entry_key == entry_key)
            .cloned()
        else {
            self.push_error(
                transaction,
                "editor.asset_browser.entry_missing",
                "Cannot select an entry that is not present in the current Asset Browser view.",
                Some("Refresh Assets or clear the current filter."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };

        let mut selection = current_selection;
        if range {
            let anchor = selection
                .primary_entry_key
                .as_ref()
                .and_then(|key| model.entries.iter().position(|item| &item.entry_key == key));
            let target = model
                .entries
                .iter()
                .position(|item| item.entry_key == entry.entry_key);
            if let (Some(anchor), Some(target)) = (anchor, target) {
                let (start, end) = if anchor <= target {
                    (anchor, target)
                } else {
                    (target, anchor)
                };
                let selected = &model.entries[start..=end];
                selection.selected_entry_keys =
                    selected.iter().map(|item| item.entry_key.clone()).collect();
                selection.selected_paths = selected.iter().map(|item| item.path.clone()).collect();
            } else {
                selection = AssetSelection::single_entry(&entry);
            }
        } else if additive {
            if let Some(index) = selection
                .selected_entry_keys
                .iter()
                .position(|key| key == &entry.entry_key)
            {
                selection.selected_entry_keys.remove(index);
                selection.selected_paths.retain(|path| path != &entry.path);
            } else {
                selection.selected_entry_keys.push(entry.entry_key.clone());
                selection.selected_paths.push(entry.path.clone());
            }
        } else {
            selection = AssetSelection::single_entry(&entry);
        }
        selection.primary_entry_key = Some(entry.entry_key.clone());
        selection.primary_path = Some(entry.path.clone());
        selection.primary_asset_id = entry.asset_id.clone();
        self.asset_browser_state.ui_state.selection = selection;
        if let Some(request) = self
            .asset_browser_state
            .active_picker
            .as_ref()
            .map(|picker| picker.request.clone())
        {
            let picker_model = self.asset_browser_state.model(
                self.asset_browser_state.ui_state.query.clone(),
                self.asset_browser_state.ui_state.selection.clone(),
            );
            let candidate =
                AssetBrowserService::pick_entry(&picker_model, request, &entry.entry_key);
            if let Some(picker) = self.asset_browser_state.active_picker.as_mut() {
                picker.candidate = Some(candidate);
            }
        } else {
            self.selected_project_browser_path = Some(entry.path.clone());
        }
        transaction
            .write_set
            .push("asset_browser.selection".to_string());
        self.push_info(
            transaction,
            "editor.asset_browser.selected",
            format!("Selected Asset Browser entry {}.", entry.path),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn begin_asset_pick(
        &mut self,
        transaction: &mut CommandTransaction,
        field_id: String,
    ) -> CommandResult {
        let request = match self.asset_pick_request_for_field(&field_id, &transaction.request_id) {
            Ok(request) => request,
            Err(diagnostic) => {
                self.push_error(
                    transaction,
                    &diagnostic.code,
                    diagnostic.message,
                    Some("Select an editable AssetRef field with an AssetFilter."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        if self.asset_browser_state.index_snapshot.is_none() {
            self.push_error(
                transaction,
                "editor.asset_picker.index_unavailable",
                "Cannot open Asset Picker before the Asset Browser index is ready.",
                Some("Refresh Assets and retry."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }

        let previous_query = self.asset_browser_state.ui_state.query.clone();
        let previous_selection = self.asset_browser_state.ui_state.selection.clone();
        let mut picker_query = previous_query.clone();
        picker_query.search_text.clear();
        picker_query.folder = None;
        picker_query.kinds = request.allowed_kinds.clone();
        self.asset_browser_state.ui_state.query = picker_query;
        self.asset_browser_state.ui_state.current_folder = None;
        self.asset_browser_state.ui_state.selection = request
            .current_asset_ref
            .as_ref()
            .and_then(|current| {
                self.asset_browser_state
                    .index_snapshot
                    .as_ref()?
                    .entries
                    .iter()
                    .find(|entry| {
                        entry.asset_id.as_deref() == Some(current.asset_id.as_str())
                            && entry.asset_type_id.as_deref()
                                == Some(current.asset_type_id.as_str())
                    })
                    .map(AssetSelection::single_entry)
            })
            .unwrap_or_default();
        self.asset_browser_state.active_picker = Some(AssetPickerSessionState {
            request,
            candidate: None,
            previous_query,
            previous_selection,
        });
        transaction
            .write_set
            .push("asset_browser.picker_state".to_string());
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn cancel_asset_pick(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        let Some(picker) = self.asset_browser_state.active_picker.take() else {
            self.push_error(
                transaction,
                "editor.asset_picker.not_active",
                "There is no active Asset Picker to cancel.",
                None,
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        self.restore_asset_picker_ui(picker);
        transaction
            .write_set
            .push("asset_browser.picker_state".to_string());
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn confirm_asset_pick(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        let Some(picker) = self.asset_browser_state.active_picker.clone() else {
            self.push_error(
                transaction,
                "editor.asset_picker.not_active",
                "There is no active Asset Picker to confirm.",
                None,
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let Some(candidate) = picker.candidate else {
            self.push_error(
                transaction,
                "editor.asset_picker.candidate_required",
                "Select a compatible asset before confirming the picker.",
                None,
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        self.commit_asset_pick_candidate(transaction, picker.request, candidate, true)
    }

    pub(crate) fn drop_asset_on_inspector_field(
        &mut self,
        transaction: &mut CommandTransaction,
        entry_key: AssetEntryKey,
        field_id: String,
    ) -> CommandResult {
        let request = match self.asset_pick_request_for_field(&field_id, &transaction.request_id) {
            Ok(request) => request,
            Err(diagnostic) => {
                self.push_error(
                    transaction,
                    &diagnostic.code,
                    diagnostic.message,
                    Some("Drop onto a compatible AssetRef picker field."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        let mut query = AssetQuery {
            kinds: request.allowed_kinds.clone(),
            ..AssetQuery::default()
        };
        query.include_missing = true;
        query.include_unimported = true;
        let model = self
            .asset_browser_state
            .model(query, AssetSelection::default());
        let candidate = AssetBrowserService::pick_entry(&model, request.clone(), &entry_key);
        self.commit_asset_pick_candidate(transaction, request, candidate, false)
    }

    fn commit_asset_pick_candidate(
        &mut self,
        transaction: &mut CommandTransaction,
        request: AssetPickRequest,
        candidate: AssetPickResult,
        close_active_picker: bool,
    ) -> CommandResult {
        if !candidate.accepted {
            let diagnostic = candidate.diagnostics.first().cloned().unwrap_or_else(|| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_candidate_rejected",
                    "Asset Picker candidate was rejected.",
                    None,
                )
            });
            self.push_error(
                transaction,
                &diagnostic.code,
                diagnostic.message,
                Some("Choose an existing authoring asset allowed by the field filter."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let selected_key = candidate
            .selected_entry_key
            .clone()
            .expect("accepted picker candidate must have a key");
        let current_model = self.asset_browser_state.model(
            AssetQuery {
                kinds: request.allowed_kinds.clone(),
                include_missing: true,
                include_unimported: true,
                ..AssetQuery::default()
            },
            AssetSelection::default(),
        );
        let current_candidate =
            AssetBrowserService::pick_entry(&current_model, request.clone(), &selected_key);
        if !current_candidate.accepted || current_candidate.asset_ref != candidate.asset_ref {
            self.push_error(
                transaction,
                "editor.asset_picker.candidate_stale",
                "The selected asset changed after preview and can no longer be committed.",
                Some("Refresh Assets and select the replacement again."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        if let Err(diagnostic) = self.validate_asset_pick_source(&request) {
            self.push_error(
                transaction,
                &diagnostic.code,
                diagnostic.message,
                Some("Reload the document and reopen Asset Picker."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let new_asset_ref = current_candidate
            .asset_ref
            .expect("accepted picker candidate must have an AssetRef");
        let plan = match self.build_asset_pick_commit_plan(&request, new_asset_ref) {
            Ok(plan) => plan,
            Err(diagnostic) => {
                self.push_error(
                    transaction,
                    &diagnostic.code,
                    diagnostic.message,
                    Some("Use a supported Scene SpriteRenderer2D or AUI image field."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        self.asset_browser_state.last_pick_commit_plan = Some(plan.clone());
        transaction
            .read_set
            .push(format!("asset_picker.request.{}", plan.request_id));

        let result = match plan.lowered_domain_command.clone() {
            UiCommandPayload::SetSceneComponentField {
                entity_id,
                component_type,
                field_path,
                value,
            } => {
                if let Some(result) = self.set_prefab_instance_override_field(
                    transaction,
                    entity_id.clone(),
                    component_type.clone(),
                    field_path.clone(),
                    value.clone(),
                ) {
                    result
                } else {
                    self.execute_scene_edit(
                        transaction,
                        SceneEditCommand::SetComponentField {
                            entity_id,
                            component_type,
                            field_path,
                            value,
                        },
                    )
                }
            }
            UiCommandPayload::SetAuiNodeField {
                path,
                node_id,
                schema_path,
                value,
            } => self.set_aui_node_field(transaction, path, node_id, schema_path, value),
            UiCommandPayload::SetReleaseProfileIcon { asset_ref } => {
                self.set_release_profile_icon(transaction, asset_ref)
            }
            _ => unreachable!("AssetPickCommitPlan produced an unsupported lowered command"),
        };
        if close_active_picker && result.status == CommandStatus::Committed {
            if let Some(picker) = self.asset_browser_state.active_picker.take() {
                self.restore_asset_picker_ui(picker);
            }
        }
        result
    }

    fn asset_pick_request_for_field(
        &self,
        field_id: &str,
        request_id: &str,
    ) -> Result<AssetPickRequest, AssetBrowserDiagnostic> {
        let project = self.active_project_session.as_ref().ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_picker_project_required",
                "Asset Picker requires an open project.",
                None,
            )
        })?;
        if field_id == "components.SpriteRenderer2D.spriteRef" {
            let document = self.editor_scene_document.as_ref().ok_or_else(|| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_scene_required",
                    "SpriteRenderer2D Asset Picker requires an open authoring Scene.",
                    None,
                )
            })?;
            let entity_id = self
                .scene_selection
                .primary_entity_id
                .clone()
                .ok_or_else(|| {
                    AssetBrowserDiagnostic::error(
                        "asset_picker_scene_entity_required",
                        "SpriteRenderer2D Asset Picker requires a selected Scene entity.",
                        None,
                    )
                })?;
            let entity = document.entity(&entity_id).ok_or_else(|| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_scene_entity_missing",
                    "Selected Scene entity is missing.",
                    None,
                )
            })?;
            let component = entity
                .components
                .iter()
                .find(|component| component.component_type == "SpriteRenderer2D")
                .ok_or_else(|| {
                    AssetBrowserDiagnostic::error(
                        "asset_picker_component_missing",
                        "Selected entity has no SpriteRenderer2D component.",
                        None,
                    )
                })?;
            let current_asset_ref = component
                .fields
                .get("spriteRef")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok());
            let scene_path = self.scene_path.as_ref().ok_or_else(|| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_scene_path_missing",
                    "Open Scene has no source path.",
                    None,
                )
            })?;
            let relative_path = if scene_path.is_absolute() {
                relative_project_path(&project.project_root, scene_path)
            } else {
                scene_path.to_string_lossy().replace('\\', "/")
            };
            return Ok(AssetPickRequest {
                request_id: request_id.to_string(),
                allowed_kinds: vec![AssetKind::Texture, AssetKind::Sprite],
                allowed_asset_types: vec!["texture".to_string(), "sprite".to_string()],
                target_kind: Some(AssetPickTargetKind::SceneComponentField),
                target_path: Some(relative_path.clone()),
                target_object_id: Some(entity_id),
                target_field_path: Some("SpriteRenderer2D.spriteRef".to_string()),
                current_asset_ref,
                expected_source_revision: Some(document.revision),
                expected_source_hash: Some(source_file_hash(
                    &project.project_root.join(relative_path),
                )?),
            });
        }
        if field_id == "build.release.application.icon" {
            let relative_path = "BuildProfiles/windows.release.json".to_string();
            let source_path = project.project_root.join(&relative_path);
            let profile = self.release_profile_cache.as_ref().ok_or_else(|| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_build_profile_required",
                    "Release icon Asset Picker requires a valid cached windows.release profile.",
                    Some(relative_path.clone()),
                )
            })?;
            let current_asset_ref = profile.application.as_ref().map(|application| {
                EditorAssetRef::new(application.icon.asset_id.clone(), "texture")
            });
            return Ok(AssetPickRequest {
                request_id: request_id.to_string(),
                allowed_kinds: vec![AssetKind::Texture, AssetKind::Sprite],
                allowed_asset_types: vec!["texture".to_string(), "sprite".to_string()],
                target_kind: Some(AssetPickTargetKind::BuildProfileField),
                target_path: Some(relative_path),
                target_object_id: Some("windows-release".to_string()),
                target_field_path: Some("application.icon".to_string()),
                current_asset_ref,
                expected_source_revision: None,
                expected_source_hash: Some(source_file_hash(&source_path)?),
            });
        }
        if field_id == "aui.image" {
            let WorkspaceSelectionTarget::AuiNode {
                document_path,
                node_id,
                ..
            } = self.selected_aui_node.as_ref().ok_or_else(|| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_aui_node_required",
                    "AUI image Asset Picker requires a selected AUI node.",
                    None,
                )
            })?
            else {
                return Err(AssetBrowserDiagnostic::error(
                    "asset_picker_aui_node_required",
                    "AUI image Asset Picker requires a selected AUI node.",
                    None,
                ));
            };
            let source_path = project.project_root.join(document_path);
            let service = AuiAuthoringService::open(&source_path).map_err(|error| {
                AssetBrowserDiagnostic::error(
                    "asset_picker_aui_document_read_failed",
                    format!("Failed to read AUI document for picker: {error}"),
                    Some(document_path.clone()),
                )
            })?;
            let current_asset_ref = service
                .document()
                .nodes
                .iter()
                .find(|node| node.node_id == *node_id)
                .and_then(|node| node.image.as_ref())
                .map(|image| EditorAssetRef::new(image.asset_id.clone(), "texture"));
            return Ok(AssetPickRequest {
                request_id: request_id.to_string(),
                allowed_kinds: vec![AssetKind::Texture, AssetKind::Sprite],
                allowed_asset_types: vec!["texture".to_string(), "sprite".to_string()],
                target_kind: Some(AssetPickTargetKind::AuiNodeField),
                target_path: Some(document_path.clone()),
                target_object_id: Some(node_id.clone()),
                target_field_path: Some("image.assetId".to_string()),
                current_asset_ref,
                expected_source_revision: None,
                expected_source_hash: Some(source_file_hash(&source_path)?),
            });
        }
        Err(AssetBrowserDiagnostic::error(
            "asset_picker_filter_missing",
            format!("Inspector field '{field_id}' has no AssetRef + AssetFilter contract."),
            Some(field_id.to_string()),
        ))
    }

    fn validate_asset_pick_source(
        &self,
        request: &AssetPickRequest,
    ) -> Result<(), AssetBrowserDiagnostic> {
        let project = self.active_project_session.as_ref().ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_picker_project_required",
                "Asset Picker requires an open project.",
                None,
            )
        })?;
        if let Some(expected_revision) = request.expected_source_revision {
            let actual_revision = self.editor_scene_document.as_ref().map(|doc| doc.revision);
            if actual_revision != Some(expected_revision) {
                return Err(AssetBrowserDiagnostic::error(
                    "asset_picker_source_revision_changed",
                    format!(
                        "Target document revision changed: expected {expected_revision}, actual {:?}.",
                        actual_revision
                    ),
                    request.target_path.clone(),
                ));
            }
        }
        if let (Some(path), Some(expected_hash)) =
            (&request.target_path, &request.expected_source_hash)
        {
            let actual_hash = source_file_hash(&project.project_root.join(path))?;
            if &actual_hash != expected_hash {
                return Err(AssetBrowserDiagnostic::error(
                    "asset_picker_source_hash_changed",
                    "Target document changed outside the picker preview transaction.",
                    Some(path.clone()),
                ));
            }
        }
        Ok(())
    }

    fn build_asset_pick_commit_plan(
        &self,
        request: &AssetPickRequest,
        new_asset_ref: EditorAssetRef,
    ) -> Result<AssetPickCommitPlan, AssetBrowserDiagnostic> {
        let target_kind = request.target_kind.ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_picker_target_kind_missing",
                "Asset Picker request has no target kind.",
                request.target_path.clone(),
            )
        })?;
        let target_document_path = request.target_path.clone().ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_picker_target_document_missing",
                "Asset Picker request has no target document path.",
                None,
            )
        })?;
        let target_object_id = request.target_object_id.clone().ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_picker_target_object_missing",
                "Asset Picker request has no target object id.",
                Some(target_document_path.clone()),
            )
        })?;
        let target_field_path = request.target_field_path.clone().ok_or_else(|| {
            AssetBrowserDiagnostic::error(
                "asset_picker_target_field_missing",
                "Asset Picker request has no target field path.",
                Some(target_document_path.clone()),
            )
        })?;
        let lowered_domain_command = match target_kind {
            AssetPickTargetKind::SceneComponentField => UiCommandPayload::SetSceneComponentField {
                entity_id: target_object_id.clone(),
                component_type: "SpriteRenderer2D".to_string(),
                field_path: "spriteRef".to_string(),
                value: serde_json::to_value(&new_asset_ref).map_err(|error| {
                    AssetBrowserDiagnostic::error(
                        "asset_picker_reference_serialize_failed",
                        format!("Failed to serialize structured AssetRef: {error}"),
                        Some(target_document_path.clone()),
                    )
                })?,
            },
            AssetPickTargetKind::AuiNodeField => UiCommandPayload::SetAuiNodeField {
                path: target_document_path.clone(),
                node_id: target_object_id.clone(),
                schema_path: "image.assetId".to_string(),
                value: serde_json::Value::String(new_asset_ref.asset_id.clone()),
            },
            AssetPickTargetKind::BuildProfileField => UiCommandPayload::SetReleaseProfileIcon {
                asset_ref: new_asset_ref.clone(),
            },
        };
        Ok(AssetPickCommitPlan {
            request_id: request.request_id.clone(),
            target_kind,
            target_document_path,
            target_object_id,
            target_field_path,
            old_asset_ref: request.current_asset_ref.clone(),
            new_asset_ref,
            expected_source_revision: request.expected_source_revision,
            expected_source_hash: request.expected_source_hash.clone(),
            lowered_domain_command,
        })
    }

    fn restore_asset_picker_ui(&mut self, picker: AssetPickerSessionState) {
        self.asset_browser_state.ui_state.query = picker.previous_query.clone();
        self.asset_browser_state.ui_state.current_folder = picker.previous_query.folder;
        self.asset_browser_state.ui_state.selection = picker.previous_selection;
    }

    pub(crate) fn open_asset_browser_entry(
        &mut self,
        transaction: &mut CommandTransaction,
        entry_key: AssetEntryKey,
    ) -> CommandResult {
        let Some((path, kind, role, openable)) = self
            .asset_browser_state
            .index_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .entries
                    .iter()
                    .find(|entry| entry.entry_key == entry_key)
            })
            .map(|entry| (entry.path.clone(), entry.kind, entry.role, entry.openable))
        else {
            self.push_error(
                transaction,
                "editor.asset_browser.entry_missing",
                "Cannot open an entry that is not present in the Asset Browser snapshot.",
                Some("Refresh Assets and retry."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        if role == AssetEntryRole::Folder {
            return self.set_asset_browser_folder(transaction, Some(path));
        }
        if !openable {
            self.push_error(
                transaction,
                "editor.asset_browser.entry_not_openable",
                format!("Asset Browser entry {path} is not openable."),
                Some("Select an authoring document such as Scene, Prefab, Rule, AUI, or Input."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        match kind {
            AssetKind::Scene => self.open_project_browser_entry(transaction, path),
            AssetKind::Prefab => self.open_prefab_document(transaction, path),
            AssetKind::Rule => self.open_rule_asset(transaction, path),
            AssetKind::Aui => self.open_aui_document(transaction, path),
            AssetKind::InputMapping => self.open_input_mapping(transaction, path),
            _ => self.open_project_browser_entry(transaction, path),
        }
    }

    pub(crate) fn set_asset_browser_folder(
        &mut self,
        transaction: &mut CommandTransaction,
        folder: Option<String>,
    ) -> CommandResult {
        if let Some(folder_path) = &folder {
            let exists = self
                .asset_browser_state
                .index_snapshot
                .as_ref()
                .is_some_and(|snapshot| {
                    snapshot.entries.iter().any(|entry| {
                        entry.role == AssetEntryRole::Folder
                            && entry.canonical_path == *folder_path
                            && entry.exists
                    })
                });
            if !exists {
                self.push_error(
                    transaction,
                    "editor.asset_browser.folder_missing",
                    format!("Asset Browser folder does not exist: {folder_path}"),
                    Some("Refresh Assets or select an existing project folder."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        }
        self.navigate_asset_browser_folder(folder, true);
        transaction
            .write_set
            .push("asset_browser.current_folder".to_string());
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_asset_browser_search(
        &mut self,
        transaction: &mut CommandTransaction,
        search_text: String,
    ) -> CommandResult {
        self.asset_browser_state.ui_state.query.search_text = search_text;
        transaction
            .write_set
            .push("asset_browser.query".to_string());
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_asset_browser_kind_filter(
        &mut self,
        transaction: &mut CommandTransaction,
        kinds: Vec<AssetKind>,
    ) -> CommandResult {
        self.asset_browser_state.ui_state.query.kinds = kinds;
        transaction
            .write_set
            .push("asset_browser.query".to_string());
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn asset_browser_toolbar_action(
        &mut self,
        transaction: &mut CommandTransaction,
        action: AssetBrowserToolbarAction,
    ) -> CommandResult {
        match action {
            AssetBrowserToolbarAction::Back => self.asset_browser_history_step(-1),
            AssetBrowserToolbarAction::Forward => self.asset_browser_history_step(1),
            AssetBrowserToolbarAction::Up => {
                let parent = self
                    .asset_browser_state
                    .ui_state
                    .current_folder
                    .as_deref()
                    .and_then(|folder| {
                        folder
                            .rsplit_once('/')
                            .map(|(parent, _)| parent.to_string())
                    });
                self.navigate_asset_browser_folder(parent, true);
            }
            AssetBrowserToolbarAction::Refresh => {
                self.request_asset_browser_refresh("user_refresh");
            }
            AssetBrowserToolbarAction::ToggleView => {
                self.asset_browser_state.ui_state.view_mode =
                    match self.asset_browser_state.ui_state.view_mode {
                        AssetBrowserViewMode::List => AssetBrowserViewMode::Grid,
                        AssetBrowserViewMode::Grid => AssetBrowserViewMode::List,
                    };
            }
            AssetBrowserToolbarAction::CycleTypeFilter => {
                let kinds = &mut self.asset_browser_state.ui_state.query.kinds;
                *kinds = match kinds.as_slice() {
                    [] => vec![AssetKind::Texture, AssetKind::Sprite],
                    [AssetKind::Texture, AssetKind::Sprite] => vec![AssetKind::Prefab],
                    [AssetKind::Prefab] => vec![AssetKind::Aui],
                    _ => Vec::new(),
                };
            }
            AssetBrowserToolbarAction::ClearSearch => {
                self.asset_browser_state.ui_state.query.search_text.clear();
            }
        }
        transaction
            .write_set
            .push("asset_browser.ui_state".to_string());
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn scroll_asset_browser(
        &mut self,
        transaction: &mut CommandTransaction,
        _delta: f32,
    ) -> CommandResult {
        // Compatibility command only. Production scrolling is widget-local editor state.
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    fn navigate_asset_browser_folder(&mut self, folder: Option<String>, record_history: bool) {
        let token = folder.clone().unwrap_or_default();
        if record_history {
            let state = &mut self.asset_browser_state.ui_state;
            if let Some(index) = state.history_index {
                state.history.truncate(index.saturating_add(1));
            }
            if state.history.last() != Some(&token) {
                state.history.push(token.clone());
            }
            state.history_index = state.history.len().checked_sub(1);
        }
        self.asset_browser_state.ui_state.current_folder = folder.clone();
        self.asset_browser_state.ui_state.query.folder = folder;
    }

    fn asset_browser_history_step(&mut self, delta: isize) {
        let state = &mut self.asset_browser_state.ui_state;
        let Some(index) = state.history_index else {
            return;
        };
        let next = (index as isize + delta).clamp(0, state.history.len().saturating_sub(1) as isize)
            as usize;
        state.history_index = Some(next);
        let token = state.history.get(next).cloned().unwrap_or_default();
        state.current_folder = (!token.is_empty()).then_some(token.clone());
        state.query.folder = (!token.is_empty()).then_some(token);
    }

    pub(crate) fn register_existing_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        expected_kind: Option<AssetKind>,
    ) -> CommandResult {
        let Some(project_session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.asset.no_project",
                "Cannot register an asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("asset.path={path}"));
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: project_session.project_root.clone(),
            query: AssetQuery {
                include_unimported: true,
                kinds: expected_kind.into_iter().collect(),
                ..AssetQuery::default()
            },
            selection: AssetSelection::default(),
        });
        let mut request = AssetPickRequest::new(transaction.request_id.clone());
        request.allowed_kinds = expected_kind.into_iter().collect();
        request.target_path = Some(path.clone());
        let result = AssetBrowserService::pick(&model, request, &path);
        if !result.accepted {
            for diagnostic in result.diagnostics {
                self.push_error(
                    transaction,
                    "editor.asset.register_failed",
                    diagnostic.message,
                    Some("Check the asset path and expected kind."),
                );
            }
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        self.selected_project_browser_path = Some(path.clone());
        self.push_info(
            transaction,
            "editor.asset.registered",
            format!("Registered existing project asset {path}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn generate_mock_image_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        prompt: String,
        target_folder: String,
        asset_name: String,
        image_kind: String,
        width: u32,
        height: u32,
        transparent_background: bool,
    ) -> CommandResult {
        let Some(project_session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.asset.no_project",
                "Cannot generate an image asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(parsed_kind) = parse_image_kind(&image_kind) else {
            self.push_error(
                transaction,
                "editor.asset.image_kind_invalid",
                format!("Unsupported image kind: {image_kind}"),
                Some("Use texture, sprite, uiImage, or referenceImage."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let mut request = AiImageGenerationRequest::new(
            format!("asset-command-{}", transaction.request_id),
            prompt,
            PathBuf::from(target_folder.clone()),
            asset_name,
            parsed_kind,
        );
        request.width = width;
        request.height = height;
        request.transparent_background = transparent_background;
        transaction
            .write_set
            .push(format!("asset.generated.folder={target_folder}"));
        let provider = MockImageGenerationProvider;
        let result = provider.generate_image(&project_session.project_root, &request);
        if !result.diagnostics.is_empty() {
            for diagnostic in result.diagnostics {
                self.push_error(
                    transaction,
                    diagnostic.code.as_str(),
                    diagnostic.message,
                    Some("Fix the image generation request."),
                );
            }
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let Some(source) = result.generated_images.first() else {
            self.push_error(
                transaction,
                "editor.asset.generated_source_missing",
                "Mock image provider succeeded without returning a generated image.",
                Some("Retry image generation."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        match import_generated_image_formally(
            &project_session.project_root,
            source,
            &request.target_folder,
            "editor-generate-image-command",
        ) {
            Ok(imported) => {
                self.selected_project_browser_path = Some(imported.record.descriptor_path.clone());
                self.asset_browser_state.mark_dirty("formal_asset_import");
                self.push_info(
                    transaction,
                    "editor.asset.generated",
                    format!(
                        "Generated mock image asset {}.",
                        imported.record.descriptor_path
                    ),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(diagnostic) => {
                self.push_error(
                    transaction,
                    diagnostic.code.as_str(),
                    diagnostic.message,
                    Some("Fix the generated asset import diagnostics."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn validate_asset_browser_index(
        &mut self,
        transaction: &mut CommandTransaction,
        query_kind: Option<AssetKind>,
    ) -> CommandResult {
        let Some(project_session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.asset.no_project",
                "Cannot validate the Asset Browser index before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push("asset_browser.index".to_string());
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: project_session.project_root.clone(),
            query: AssetQuery {
                include_unimported: true,
                kinds: query_kind.into_iter().collect(),
                ..AssetQuery::default()
            },
            selection: AssetSelection::default(),
        });
        self.push_info(
            transaction,
            "editor.asset.index_validated",
            format!(
                "Asset Browser index validated: entries={}, diagnostics={}.",
                model.entries.len(),
                model.report.diagnostics.len()
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}

fn parse_image_kind(value: &str) -> Option<ImageKind> {
    match value {
        "texture" | "Texture" => Some(ImageKind::Texture),
        "sprite" | "Sprite" => Some(ImageKind::Sprite),
        "uiImage" | "ui_image" | "UiImage" => Some(ImageKind::UiImage),
        "referenceImage" | "reference_image" | "ReferenceImage" => Some(ImageKind::ReferenceImage),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn asset_browser_index_scans_project_folders_and_files() {
        let root = fixture_project();
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: root.clone(),
            query: AssetQuery::default(),
            selection: AssetSelection::default(),
        });

        assert!(model.entries.iter().any(|entry| entry.path == "Assets"));
        assert!(model.entries.iter().any(|entry| {
            entry.path == "Assets/icon.png"
                && entry.kind == AssetKind::Texture
                && entry.role == AssetEntryRole::SourceFile
        }));
        assert!(model.entries.iter().any(|entry| {
            entry.path == "Scenes/Main.scene.json"
                && entry.kind == AssetKind::Scene
                && entry.asset_id.as_deref() == Some("scene-main")
        }));
        let texture = model
            .entries
            .iter()
            .find(|entry| entry.path == "Assets/icon.asset")
            .expect("typed texture asset should be indexed");
        assert_eq!(texture.asset_id.as_deref(), Some("texture-icon"));
        assert_eq!(texture.source_status, AssetSourceStatus::Linked);
        assert_eq!(
            texture.preview.thumbnail_source_path.as_deref(),
            Some("Assets/icon.png")
        );
    }

    #[test]
    fn asset_browser_query_filters_by_text_and_kind() {
        let root = fixture_project();
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: root,
            query: AssetQuery {
                search_text: "texture-icon".to_string(),
                kinds: vec![AssetKind::Texture],
                ..AssetQuery::default()
            },
            selection: AssetSelection::default(),
        });

        assert_eq!(model.entries.len(), 1);
        assert_eq!(model.entries[0].kind, AssetKind::Texture);
        assert!(model.report.filtered_count > 0);
    }

    #[test]
    fn asset_browser_pick_rejects_type_mismatch() {
        let root = fixture_project();
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: root,
            query: AssetQuery::default(),
            selection: AssetSelection::default(),
        });

        let mut request = AssetPickRequest::new("pick-1");
        request.allowed_kinds = vec![AssetKind::Scene];
        let result = AssetBrowserService::pick(&model, request, "Assets/icon.asset");

        assert!(!result.accepted);
        assert_eq!(result.diagnostics[0].code, "asset_type_mismatch");
    }

    #[test]
    fn asset_browser_drag_payload_converts_placeable_asset_to_placement_request() {
        let entry = AssetBrowserEntry::authoring(
            "Assets/icon.asset",
            "icon.asset",
            AssetKind::Texture,
            EditorAssetRef::new("texture-icon", "texture"),
        );
        let payload = AssetBrowserService::drag_payload(&[entry]);
        let request = AssetBrowserService::placement_request_from_reference(
            &payload.asset_refs[0],
            None,
            None,
            AssetPlacementMode::WorldOrigin,
        )
        .unwrap();

        assert_eq!(request.asset_type, "texture");
        assert_eq!(request.asset_id, "texture-icon");
    }

    #[test]
    fn asset_browser_report_counts_unimported_unknown_assets() {
        let root = fixture_project();
        std::fs::write(root.join("Assets").join("unknown.bin"), b"unknown").unwrap();
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: root,
            query: AssetQuery {
                include_unimported: true,
                ..AssetQuery::default()
            },
            selection: AssetSelection::default(),
        });

        assert!(model.report.unimported_count >= 1);
        assert!(model
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "asset_unimported"));
    }

    #[test]
    fn asset_browser_source_file_cannot_be_picked_as_asset_ref() {
        let root = fixture_project();
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: root,
            query: AssetQuery::default(),
            selection: AssetSelection::default(),
        });

        let mut request = AssetPickRequest::new("pick-source");
        request.allowed_kinds = vec![AssetKind::Texture];
        let result = AssetBrowserService::pick(&model, request, "Assets/icon.png");

        assert!(!result.accepted);
        assert_eq!(result.diagnostics[0].code, "asset_identity_required");
    }

    #[test]
    fn asset_browser_picker_cancel_restores_query_without_scene_write() {
        let (_root, mut session, new_texture_key, _) = picker_scene_session();
        let original_revision = session
            .editor_scene_document
            .as_ref()
            .expect("open scene")
            .revision;

        assert_eq!(
            session
                .execute_command(crate::command_for_test(
                    UiCommandPayload::SetAssetBrowserSearch {
                        search_text: "before-picker".to_string(),
                    },
                ))
                .status,
            CommandStatus::Committed
        );
        assert_eq!(
            session
                .execute_command(crate::command_for_test(UiCommandPayload::BeginAssetPick {
                    field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
                }))
                .status,
            CommandStatus::Committed
        );
        assert!(session.build_ui_model().asset_browser.picker.is_some());
        assert_eq!(
            session
                .execute_command(crate::command_for_test(
                    UiCommandPayload::SelectAssetBrowserEntry {
                        entry_key: new_texture_key,
                        additive: false,
                        range: false,
                    },
                ))
                .status,
            CommandStatus::Committed
        );
        assert!(session
            .build_ui_model()
            .asset_browser
            .picker
            .as_ref()
            .is_some_and(|picker| picker.can_confirm));

        let cancel =
            session.execute_command(crate::command_for_test(UiCommandPayload::CancelAssetPick));

        assert_eq!(cancel.status, CommandStatus::Committed);
        let model = session.build_ui_model().asset_browser;
        assert!(model.picker.is_none());
        assert_eq!(model.query.search_text, "before-picker");
        assert_eq!(
            session
                .editor_scene_document
                .as_ref()
                .expect("open scene")
                .revision,
            original_revision
        );
        assert_eq!(scene_sprite_ref(&session).asset_id, "texture-old");
        assert!(session.asset_browser_state.last_pick_commit_plan.is_none());
    }

    #[test]
    fn asset_browser_picker_confirm_writes_structured_scene_asset_ref() {
        let (root, mut session, new_texture_key, _) = picker_scene_session();
        let original_revision = session
            .editor_scene_document
            .as_ref()
            .expect("open scene")
            .revision;

        begin_and_select_scene_asset(&mut session, new_texture_key);
        let confirm =
            session.execute_command(crate::command_for_test(UiCommandPayload::ConfirmAssetPick));

        assert_eq!(confirm.status, CommandStatus::Committed, "{confirm:?}");
        assert!(session.build_ui_model().asset_browser.picker.is_none());
        let reference = scene_sprite_ref(&session);
        assert_eq!(reference.asset_id, "texture-new");
        assert_eq!(reference.asset_type_id, "texture");
        assert_eq!(reference.guid.as_deref(), Some("guid-texture-new"));
        assert!(reference.sub_asset_id.is_none());
        assert!(
            session
                .editor_scene_document
                .as_ref()
                .expect("open scene")
                .revision
                > original_revision
        );
        let plan = session
            .asset_browser_state
            .last_pick_commit_plan
            .as_ref()
            .expect("committed picker plan");
        assert_eq!(plan.new_asset_ref, reference);
        assert!(matches!(
            plan.lowered_domain_command,
            UiCommandPayload::SetSceneComponentField { .. }
        ));

        let save = session.execute_command(crate::command_for_test(
            UiCommandPayload::SaveSceneDocument { path: None },
        ));
        assert_eq!(save.status, CommandStatus::Committed, "{save:?}");
        let saved: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("Scenes/Main.scene.json")).unwrap(),
        )
        .unwrap();
        let saved_ref = &saved["entities"][0]["components"][0]["fields"]["spriteRef"];
        assert!(saved_ref.is_object());
        assert_eq!(saved_ref["id"], "texture-new");
        assert_eq!(saved_ref["type"], "texture");
        assert_eq!(saved_ref["guid"], "guid-texture-new");
    }

    #[test]
    fn asset_browser_picker_rejects_external_source_hash_change() {
        let (root, mut session, new_texture_key, _) = picker_scene_session();
        begin_and_select_scene_asset(&mut session, new_texture_key);
        let scene_path = root.join("Scenes/Main.scene.json");
        let mut source = std::fs::read_to_string(&scene_path).unwrap();
        source.push('\n');
        std::fs::write(&scene_path, source).unwrap();

        let confirm =
            session.execute_command(crate::command_for_test(UiCommandPayload::ConfirmAssetPick));

        assert_eq!(confirm.status, CommandStatus::Rejected);
        assert!(confirm
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "asset_picker_source_hash_changed"));
        assert_eq!(scene_sprite_ref(&session).asset_id, "texture-old");
        assert!(session.build_ui_model().asset_browser.picker.is_some());
        assert!(session.asset_browser_state.last_pick_commit_plan.is_none());
    }

    #[test]
    fn asset_browser_drop_rejects_source_file_and_field_without_asset_filter() {
        let (_root, mut session, new_texture_key, source_texture_key) = picker_scene_session();

        let source_drop = session.execute_command(crate::command_for_test(
            UiCommandPayload::DropAssetOnInspectorField {
                entry_key: source_texture_key,
                field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
            },
        ));
        assert_eq!(source_drop.status, CommandStatus::Rejected);
        assert!(source_drop
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "asset_identity_required"));

        let unfiltered_drop = session.execute_command(crate::command_for_test(
            UiCommandPayload::DropAssetOnInspectorField {
                entry_key: new_texture_key,
                field_id: "components.game.health.hp".to_string(),
            },
        ));
        assert_eq!(unfiltered_drop.status, CommandStatus::Rejected);
        assert!(unfiltered_drop
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "asset_picker_filter_missing"));
        assert_eq!(scene_sprite_ref(&session).asset_id, "texture-old");
    }

    #[test]
    fn asset_browser_picker_confirm_writes_aui_image_asset() {
        let (root, mut session, new_texture_key, _) = picker_scene_session();
        for payload in [
            UiCommandPayload::CreateAuiDocument {
                path: "AUI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            },
            UiCommandPayload::AddAuiNode {
                path: "AUI/hud.aui.json".to_string(),
                parent_node_id: "root".to_string(),
                node_id: "equipment_icon".to_string(),
                kind: "image".to_string(),
                name: "Equipment Icon".to_string(),
                rect: serde_json::json!({
                    "x": 16.0,
                    "y": 16.0,
                    "width": 64.0,
                    "height": 64.0
                }),
            },
            UiCommandPayload::SelectAuiNode {
                document_path: "AUI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                node_id: "equipment_icon".to_string(),
            },
            UiCommandPayload::BeginAssetPick {
                field_id: "aui.image".to_string(),
            },
            UiCommandPayload::SelectAssetBrowserEntry {
                entry_key: new_texture_key,
                additive: false,
                range: false,
            },
            UiCommandPayload::ConfirmAssetPick,
        ] {
            let result = session.execute_command(crate::command_for_test(payload));
            assert_eq!(result.status, CommandStatus::Committed, "{result:?}");
        }

        let document: engine_runtime::aui::AuiDocument =
            serde_json::from_str(&std::fs::read_to_string(root.join("AUI/hud.aui.json")).unwrap())
                .unwrap();
        let image = document
            .nodes
            .iter()
            .find(|node| node.node_id == "equipment_icon")
            .and_then(|node| node.image.as_ref())
            .expect("AUI image reference");
        assert_eq!(image.asset_id, "texture-new");
        assert!(matches!(
            session
                .asset_browser_state
                .last_pick_commit_plan
                .as_ref()
                .expect("AUI picker plan")
                .lowered_domain_command,
            UiCommandPayload::SetAuiNodeField { .. }
        ));
    }

    #[test]
    fn asset_browser_rejects_traversal_and_root_escape() {
        let root = fixture_project();
        let traversal = canonical_inside_project(&root, Path::new("../outside.asset"))
            .expect_err("traversal must be rejected");
        assert_eq!(traversal.code, "asset_path_traversal");

        let outside = root
            .parent()
            .expect("fixture parent")
            .join("asset-browser-outside.asset");
        std::fs::write(&outside, b"outside").unwrap();
        let escape = canonical_inside_project(&root, &outside)
            .expect_err("absolute root escape must be rejected");
        assert_eq!(escape.code, "asset_path_root_escape");
    }

    #[test]
    fn complex_shooter_catalog_covers_required_typed_assets_and_excludes_generated_roots() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/complex_shooter_project");
        let model = AssetBrowserIndex::build(AssetBrowserBuildRequest {
            project_root: root,
            query: AssetQuery {
                include_unimported: true,
                ..AssetQuery::default()
            },
            selection: AssetSelection::default(),
        });

        for (path, kind) in [
            ("AUI/hud.aui.json", AssetKind::Aui),
            ("Assets/font-main.asset", AssetKind::Font),
            ("BuildProfiles/windows.dev.json", AssetKind::BuildProfile),
            ("Settings/project_settings.json", AssetKind::ProjectSettings),
            ("Assets/tex-player-ship.asset", AssetKind::Texture),
        ] {
            assert!(
                model.entries.iter().any(|entry| {
                    entry.path == path
                        && entry.kind == kind
                        && entry.role == AssetEntryRole::AuthoringAsset
                }),
                "missing typed catalog entry: {path}"
            );
        }
        assert!(model.entries.iter().all(|entry| {
            !GENERATED_ROOTS
                .iter()
                .any(|root| entry.path == *root || entry.path.starts_with(&format!("{root}/")))
        }));
    }

    #[test]
    fn asset_browser_cached_state_does_not_rescan_for_frontend_changes_or_300_models() {
        let root = fixture_project();
        let mut state = AssetBrowserSessionState::default();
        state.initialize(&root);

        assert_eq!(state.index_status, AssetBrowserIndexStatus::Ready);
        assert_eq!(
            state
                .index_snapshot
                .as_ref()
                .map(|snapshot| snapshot.scan_generation),
            Some(1)
        );
        assert_eq!(state.scan_started_count, 1);

        for frame in 0..300 {
            let query = AssetQuery {
                search_text: if frame % 2 == 0 {
                    "texture".to_string()
                } else {
                    String::new()
                },
                kinds: if frame % 3 == 0 {
                    vec![AssetKind::Texture]
                } else {
                    Vec::new()
                },
                ..AssetQuery::default()
            };
            let _ = state.model(query, AssetSelection::default());
            state.ui_state.view_mode = if frame % 2 == 0 {
                AssetBrowserViewMode::Grid
            } else {
                AssetBrowserViewMode::List
            };
        }

        assert_eq!(state.scan_started_count, 1);
        assert_eq!(state.scan_committed_count, 1);
        assert_eq!(
            state
                .index_snapshot
                .as_ref()
                .map(|snapshot| snapshot.scan_generation),
            Some(1)
        );
    }

    #[test]
    fn asset_browser_explicit_dirty_refresh_commits_exactly_one_generation() {
        let root = fixture_project();
        let mut state = AssetBrowserSessionState::default();
        state.initialize(&root);

        state.mark_dirty("external_asset_change");
        state.refresh_now(&root, "external_asset_change");

        let snapshot = state.index_snapshot.as_ref().expect("ready snapshot");
        assert_eq!(snapshot.scan_generation, 2);
        assert_eq!(state.scan_started_count, 2);
        assert_eq!(state.scan_committed_count, 2);
        assert_eq!(state.index_status, AssetBrowserIndexStatus::Ready);
    }

    #[test]
    fn asset_browser_failed_refresh_preserves_last_ready_snapshot_as_stale() {
        let root = fixture_project();
        let mut state = AssetBrowserSessionState::default();
        state.initialize(&root);
        let fingerprint = state
            .index_snapshot
            .as_ref()
            .expect("initial snapshot")
            .source_fingerprint
            .clone();

        state.refresh_now(&root.join("missing-project"), "invalid_test_root");

        assert_eq!(state.index_status, AssetBrowserIndexStatus::Stale);
        assert_eq!(
            state
                .index_snapshot
                .as_ref()
                .expect("last snapshot remains")
                .source_fingerprint,
            fingerprint
        );
    }

    fn fixture_project() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("asset-browser-fixture-{stamp}"));
        std::fs::create_dir_all(root.join("Assets")).unwrap();
        std::fs::create_dir_all(root.join("Scenes")).unwrap();
        std::fs::create_dir_all(root.join("Prefabs")).unwrap();
        std::fs::create_dir_all(root.join("AUI")).unwrap();
        std::fs::create_dir_all(root.join("Rules")).unwrap();
        std::fs::create_dir_all(root.join("Input")).unwrap();
        std::fs::create_dir_all(root.join("BuildProfiles")).unwrap();
        std::fs::create_dir_all(root.join("Settings")).unwrap();
        std::fs::create_dir_all(root.join("Build")).unwrap();
        std::fs::write(root.join("Assets").join("icon.png"), b"png").unwrap();
        std::fs::write(
            root.join("Assets").join("icon.asset"),
            br#"{
                "schemaVersion":"texture-asset.v1",
                "assetId":"texture-icon",
                "sourceImage":"Assets/icon.png"
            }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Scenes").join("Main.scene.json"),
            br#"{"schemaVersion":"editor-scene-document.v1","id":"scene-main"}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Prefabs").join("Widget.prefab.json"),
            br#"{"schemaVersion":"authoring-prefab-asset.v1","prefabId":"prefab-widget"}"#,
        )
        .unwrap();
        std::fs::write(root.join("Build").join("generated.asset"), b"generated").unwrap();
        root
    }

    fn picker_scene_session() -> (PathBuf, EditorSession, AssetEntryKey, AssetEntryKey) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_root = canonical_test_temp_dir();
        let root = temp_root.join(format!("asset-picker-session-{stamp}"));
        let mut session = EditorSession::new();
        let create =
            session.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
                path: root.display().to_string(),
                name: "Asset Picker Test".to_string(),
            }));
        assert_eq!(create.status, CommandStatus::Committed, "{create:?}");

        std::fs::write(root.join("Assets/old.png"), b"old-png").unwrap();
        std::fs::write(root.join("Assets/new.png"), b"new-png").unwrap();
        write_texture_asset(
            &root.join("Assets/old.asset"),
            "texture-old",
            "guid-texture-old",
            "Assets/old.png",
        );
        write_texture_asset(
            &root.join("Assets/new.asset"),
            "texture-new",
            "guid-texture-new",
            "Assets/new.png",
        );
        std::fs::write(
            root.join("Scenes/Main.scene.json"),
            r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{
    "schemaVersion": "editor-scene-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },
    "mesh": null,
    "components": [{
      "componentType": "SpriteRenderer2D",
      "fields": {
        "spriteRef": {
          "id": "texture-old",
          "type": "texture",
          "guid": "guid-texture-old"
        }
      }
    }]
  }]
}"##,
        )
        .unwrap();
        let open = session.execute_command(crate::command_for_test(
            UiCommandPayload::OpenSceneDocument {
                path: root.join("Scenes/Main.scene.json").display().to_string(),
            },
        ));
        assert_eq!(open.status, CommandStatus::Committed, "{open:?}");
        let select = session.execute_command(crate::command_for_test(
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity-player".to_string(),
            },
        ));
        assert_eq!(select.status, CommandStatus::Committed, "{select:?}");
        session.refresh_asset_browser_now("asset_picker_fixture_ready");
        let browser = session.build_ui_model().asset_browser;
        let new_texture_key = browser
            .entries
            .iter()
            .find(|entry| entry.path == "Assets/new.asset")
            .expect("new texture authoring asset")
            .entry_key
            .clone();
        let source_texture_key = browser
            .entries
            .iter()
            .find(|entry| entry.path == "Assets/new.png")
            .expect("new texture source file")
            .entry_key
            .clone();
        (root, session, new_texture_key, source_texture_key)
    }

    fn canonical_test_temp_dir() -> PathBuf {
        let canonical =
            std::fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
        #[cfg(windows)]
        {
            let display = canonical.to_string_lossy();
            if let Some(unc) = display.strip_prefix(r"\\?\UNC\") {
                return PathBuf::from(format!(r"\\{unc}"));
            }
            if let Some(local) = display.strip_prefix(r"\\?\") {
                return PathBuf::from(local);
            }
        }
        canonical
    }

    fn begin_and_select_scene_asset(session: &mut EditorSession, entry_key: AssetEntryKey) {
        for payload in [
            UiCommandPayload::BeginAssetPick {
                field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
            },
            UiCommandPayload::SelectAssetBrowserEntry {
                entry_key,
                additive: false,
                range: false,
            },
        ] {
            let result = session.execute_command(crate::command_for_test(payload));
            assert_eq!(result.status, CommandStatus::Committed, "{result:?}");
        }
    }

    fn scene_sprite_ref(session: &EditorSession) -> EditorAssetRef {
        session
            .editor_scene_document
            .as_ref()
            .expect("open scene")
            .entity("entity-player")
            .expect("player entity")
            .components
            .iter()
            .find(|component| component.component_type == "SpriteRenderer2D")
            .and_then(|component| component.fields.get("spriteRef"))
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .expect("structured SpriteRenderer2D AssetRef")
    }

    fn write_texture_asset(path: &Path, asset_id: &str, guid: &str, source_image: &str) {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "texture-asset.v1",
                "assetId": asset_id,
                "assetGuid": guid,
                "sourceImage": source_image
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
