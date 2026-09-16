use editor_core::{
    command_for_test, AssetBrowserIndex, AssetBrowserReportLevel, AssetBrowserService,
    CommandStatus, EditorSession, ProjectRuntimePackageAssembler,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyStatus,
    ASSET_THUMBNAIL_MAX_CPU_BYTES, ASSET_THUMBNAIL_MAX_ITEMS, ASSET_THUMBNAIL_MAX_PENDING,
};
use editor_ui_model::{
    AssetEntryKey, AssetEntryRole, AssetKind, AssetPlacementMode, AssetQuery, EditorAssetRef,
    InspectorValue, UiCommandPayload,
};
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-asset-browser-native-productization-report.v1";
pub const COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_SCENARIO_ID: &str =
    "complex_shooter_asset_browser_native_productization";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexShooterAssetBrowserNativeStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBrowserThumbnailEvidence {
    pub thumbnail_id: Option<String>,
    pub request_count: usize,
    pub width: u32,
    pub height: u32,
    pub rgba_byte_count: usize,
    pub non_empty_alpha_pixel: bool,
    pub record_count: usize,
    pub pending_count: usize,
    pub ready_count: usize,
    pub failed_count: usize,
    pub cpu_bytes: usize,
    pub within_budget: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBrowserDragDropEvidence {
    pub source_entry_key: Option<AssetEntryKey>,
    pub asset_ref: Option<EditorAssetRef>,
    pub transaction_status: String,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBrowserPickerEvidence {
    pub target_document: String,
    pub target_entity: String,
    pub target_field: String,
    pub candidate_entry_key: Option<AssetEntryKey>,
    pub old_asset_ref: Option<EditorAssetRef>,
    pub new_asset_ref: Option<EditorAssetRef>,
    pub cancel_status: String,
    pub cancel_preserved_source_hash: bool,
    pub confirm_status: String,
    pub structured_reference_written: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBrowserSaveReloadEvidence {
    pub source_hash_before: Option<String>,
    pub source_hash_after_cancel: Option<String>,
    pub source_hash_after_save: Option<String>,
    pub save_status: String,
    pub reopen_status: String,
    pub reloaded_asset_ref: Option<EditorAssetRef>,
    pub reference_preserved: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBrowserRuntimePackageEvidence {
    pub assembly_status: String,
    pub build_status: String,
    pub package_loaded: bool,
    pub runtime_scene_asset_ref: Option<engine_runtime::runtime_package::RuntimeAssetRef>,
    pub runtime_asset_record_id: Option<String>,
    pub runtime_asset_record_type: Option<String>,
    pub runtime_asset_index_resolved: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBrowserPathSafetyEvidence {
    pub traversal_rejected: bool,
    pub traversal_code: Option<String>,
    pub root_escape_rejected: bool,
    pub root_escape_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAssetBrowserNativeReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAssetBrowserNativeStatus,
    pub project_root: String,
    pub workspace_root: String,
    pub output_root: String,
    pub runtime_package_dir: String,
    pub index_revision: u64,
    pub scan_generation_before_frames: u64,
    pub scan_generation_after_frames: u64,
    pub no_rescan_for_300_frames: bool,
    pub entry_count_by_role: BTreeMap<String, usize>,
    pub entry_count_by_kind: BTreeMap<String, usize>,
    pub excluded_generated_roots: bool,
    pub query_text: String,
    pub query_result_count: usize,
    pub selected_entry_key: Option<AssetEntryKey>,
    pub opened_asset_statuses: BTreeMap<String, String>,
    pub thumbnail: AssetBrowserThumbnailEvidence,
    pub drag_drop: AssetBrowserDragDropEvidence,
    pub picker: AssetBrowserPickerEvidence,
    pub save_reload: AssetBrowserSaveReloadEvidence,
    pub runtime_package: AssetBrowserRuntimePackageEvidence,
    pub path_safety: AssetBrowserPathSafetyEvidence,
    pub report_panel_provider_present: bool,
    pub report_panel_trace_evidence_count: usize,
    pub diagnostics: Vec<String>,
    pub artifacts: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAssetBrowserNativeRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAssetBrowserNativeRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_asset_browser_native_report(
    request: ComplexShooterAssetBrowserNativeRequest,
) -> ComplexShooterAssetBrowserNativeReport {
    let workspace_root = request.output_root.join("workspace");
    let package_dir = request.output_root.join("runtime_package");
    let mut report = empty_report(&request, &workspace_root, &package_dir);
    if let Err(error) = fs::create_dir_all(&request.output_root)
        .and_then(|_| copy_project(&request.project_root, &workspace_root))
    {
        report
            .diagnostics
            .push(format!("workspace_copy_failed:{error}"));
        return finalize_report(&request.output_root, report);
    }

    let scene_path = workspace_root.join("Scenes/Main.scene.json");
    report.save_reload.source_hash_before = file_hash(&scene_path).ok();
    let mut session = crate::complex_shooter_editor_session();
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: workspace_root.display().to_string(),
    }));
    if open.status != CommandStatus::Committed {
        report.diagnostics.push("open_project_failed".to_string());
        return finalize_report(&request.output_root, report);
    }

    let initial_state = session.asset_browser_state();
    report.index_revision = initial_state
        .index_snapshot
        .as_ref()
        .map_or(0, |snapshot| snapshot.revision);
    report.scan_generation_before_frames = initial_state
        .index_snapshot
        .as_ref()
        .map_or(0, |snapshot| snapshot.scan_generation);
    let full_query = AssetQuery {
        include_missing: true,
        include_unimported: true,
        ..AssetQuery::default()
    };
    let full_model = session.build_asset_browser_model(full_query.clone());
    collect_entry_counts(&full_model.entries, &mut report);
    report.excluded_generated_roots = full_model.entries.iter().all(|entry| {
        !["Build", "Reports", "dist", "exports", "release", "target"]
            .iter()
            .any(|root| entry.path == *root || entry.path.starts_with(&format!("{root}/")))
    });

    for frame in 0..300 {
        session.set_asset_browser_query(AssetQuery {
            search_text: if frame % 2 == 0 {
                "tex".to_string()
            } else {
                String::new()
            },
            kinds: if frame % 3 == 0 {
                vec![AssetKind::Texture]
            } else {
                Vec::new()
            },
            include_missing: true,
            include_unimported: true,
            ..AssetQuery::default()
        });
        let _ = session.build_ui_model();
    }
    report.scan_generation_after_frames = session
        .asset_browser_state()
        .index_snapshot
        .as_ref()
        .map_or(0, |snapshot| snapshot.scan_generation);
    report.no_rescan_for_300_frames =
        report.scan_generation_before_frames == report.scan_generation_after_frames;

    report.query_text = "tex-player-ship".to_string();
    session.set_asset_browser_query(AssetQuery {
        search_text: report.query_text.clone(),
        include_missing: true,
        include_unimported: true,
        ..AssetQuery::default()
    });
    let query_model =
        session.build_asset_browser_model(session.asset_browser_state().ui_state.query.clone());
    report.query_result_count = query_model.entries.len();
    let player_texture = query_model
        .entries
        .iter()
        .find(|entry| entry.asset_id.as_deref() == Some("tex-player-ship"))
        .cloned();
    if let Some(player_texture) = player_texture {
        report.selected_entry_key = Some(player_texture.entry_key.clone());
        let select = session.execute_command(command_for_test(
            UiCommandPayload::SelectAssetBrowserEntry {
                entry_key: player_texture.entry_key.clone(),
                additive: false,
                range: false,
            },
        ));
        if select.status != CommandStatus::Committed {
            report
                .diagnostics
                .push("query_selection_failed".to_string());
        }
        collect_thumbnail_evidence(&mut session, &player_texture.entry_key, &mut report);
    } else {
        report
            .diagnostics
            .push("player_texture_query_result_missing".to_string());
    }

    collect_open_evidence(&mut session, &full_model.entries, &mut report);
    collect_drag_evidence(&mut session, &full_model.entries, &mut report);
    collect_picker_evidence(&mut session, &full_model.entries, &scene_path, &mut report);
    collect_reload_evidence(&workspace_root, &mut report);
    collect_runtime_package_evidence(&workspace_root, &package_dir, &mut report);
    collect_path_safety_evidence(&workspace_root, &request.output_root, &mut report);

    session.set_asset_browser_report_level(AssetBrowserReportLevel::Trace);
    let panel = session.build_ui_model().report_panel;
    if let Some(asset_report) = panel
        .reports
        .iter()
        .find(|entry| entry.provider_id == "authoring.asset_browser")
    {
        report.report_panel_provider_present = true;
        report.report_panel_trace_evidence_count = asset_report.evidence.len();
    }

    validate_report(&mut report);
    if report.diagnostics.is_empty() {
        report.status = ComplexShooterAssetBrowserNativeStatus::Passed;
    } else {
        report
            .next_actions
            .push("inspect_complex_shooter_asset_browser_native_report".to_string());
    }
    finalize_report(&request.output_root, report)
}

fn empty_report(
    request: &ComplexShooterAssetBrowserNativeRequest,
    workspace_root: &Path,
    package_dir: &Path,
) -> ComplexShooterAssetBrowserNativeReport {
    ComplexShooterAssetBrowserNativeReport {
        schema_version: COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_SCENARIO_ID.to_string(),
        status: ComplexShooterAssetBrowserNativeStatus::Failed,
        project_root: request.project_root.display().to_string(),
        workspace_root: workspace_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        runtime_package_dir: package_dir.display().to_string(),
        index_revision: 0,
        scan_generation_before_frames: 0,
        scan_generation_after_frames: 0,
        no_rescan_for_300_frames: false,
        entry_count_by_role: BTreeMap::new(),
        entry_count_by_kind: BTreeMap::new(),
        excluded_generated_roots: false,
        query_text: String::new(),
        query_result_count: 0,
        selected_entry_key: None,
        opened_asset_statuses: BTreeMap::new(),
        thumbnail: AssetBrowserThumbnailEvidence::default(),
        drag_drop: AssetBrowserDragDropEvidence::default(),
        picker: AssetBrowserPickerEvidence {
            target_document: "Scenes/Main.scene.json".to_string(),
            target_entity: "entity-player".to_string(),
            target_field: "SpriteRenderer2D.spriteRef".to_string(),
            ..AssetBrowserPickerEvidence::default()
        },
        save_reload: AssetBrowserSaveReloadEvidence::default(),
        runtime_package: AssetBrowserRuntimePackageEvidence::default(),
        path_safety: AssetBrowserPathSafetyEvidence::default(),
        report_panel_provider_present: false,
        report_panel_trace_evidence_count: 0,
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
        next_actions: Vec::new(),
    }
}

fn collect_entry_counts(
    entries: &[editor_ui_model::AssetBrowserEntry],
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    for entry in entries {
        *report
            .entry_count_by_role
            .entry(format!("{:?}", entry.role))
            .or_insert(0) += 1;
        *report
            .entry_count_by_kind
            .entry(format!("{:?}", entry.kind))
            .or_insert(0) += 1;
    }
}

fn collect_thumbnail_evidence(
    session: &mut EditorSession,
    entry_key: &AssetEntryKey,
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    let model =
        session.build_asset_browser_model(session.asset_browser_state().ui_state.query.clone());
    let thumbnail_id = model
        .entries
        .iter()
        .find(|entry| &entry.entry_key == entry_key)
        .and_then(|entry| entry.preview.thumbnail_id.clone());
    report.thumbnail.thumbnail_id = thumbnail_id.clone();
    let Some(thumbnail_id) = thumbnail_id else {
        report.diagnostics.push("thumbnail_id_missing".to_string());
        return;
    };
    let ids = BTreeSet::from([thumbnail_id]);
    report.thumbnail.request_count = session.request_asset_thumbnail_ids(&ids);
    for _ in 0..200 {
        let _ = session.pump_asset_browser_refresh();
        if session.asset_thumbnail_summary().pending_count == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let payloads = session.asset_thumbnail_payloads_for_ids(&ids);
    if let Some(payload) = payloads.first() {
        report.thumbnail.width = payload.width;
        report.thumbnail.height = payload.height;
        report.thumbnail.rgba_byte_count = payload.rgba8.len();
        report.thumbnail.non_empty_alpha_pixel = payload
            .rgba8
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && pixel[..3] != [0, 0, 0]);
    }
    let summary = session.asset_thumbnail_summary();
    report.thumbnail.record_count = summary.record_count;
    report.thumbnail.pending_count = summary.pending_count;
    report.thumbnail.ready_count = summary.ready_count;
    report.thumbnail.failed_count = summary.failed_count;
    report.thumbnail.cpu_bytes = summary.cpu_bytes;
    report.thumbnail.within_budget = summary.record_count <= ASSET_THUMBNAIL_MAX_ITEMS
        && summary.pending_count <= ASSET_THUMBNAIL_MAX_PENDING
        && summary.cpu_bytes <= ASSET_THUMBNAIL_MAX_CPU_BYTES;
}

fn collect_open_evidence(
    session: &mut EditorSession,
    entries: &[editor_ui_model::AssetBrowserEntry],
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    for (label, kind) in [
        ("rule", AssetKind::Rule),
        ("aui", AssetKind::Aui),
        ("input", AssetKind::InputMapping),
    ] {
        let Some(entry) = entries
            .iter()
            .find(|entry| entry.kind == kind && entry.role == AssetEntryRole::AuthoringAsset)
        else {
            report
                .opened_asset_statuses
                .insert(label.to_string(), "Missing".to_string());
            continue;
        };
        let result =
            session.execute_command(command_for_test(UiCommandPayload::OpenAssetBrowserEntry {
                entry_key: entry.entry_key.clone(),
            }));
        report
            .opened_asset_statuses
            .insert(label.to_string(), format!("{:?}", result.status));
    }
}

fn collect_drag_evidence(
    session: &mut EditorSession,
    entries: &[editor_ui_model::AssetBrowserEntry],
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    let Some(prefab) = entries.iter().find(|entry| {
        entry.asset_id.as_deref() == Some("prefab-enemy-scout")
            && entry.role == AssetEntryRole::AuthoringAsset
    }) else {
        report
            .diagnostics
            .push("prefab_drag_source_missing".to_string());
        return;
    };
    report.drag_drop.source_entry_key = Some(prefab.entry_key.clone());
    let payload = AssetBrowserService::drag_payload(&[prefab.clone()]);
    report.drag_drop.asset_ref = payload.asset_refs.first().cloned();
    let Some(asset_ref) = payload.asset_refs.first() else {
        report
            .diagnostics
            .push("prefab_drag_asset_ref_missing".to_string());
        return;
    };
    let Ok(placement) = AssetBrowserService::placement_request_from_reference(
        asset_ref,
        None,
        None,
        AssetPlacementMode::WorldOrigin,
    ) else {
        report
            .diagnostics
            .push("prefab_drag_placement_rejected".to_string());
        return;
    };
    let result = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: placement.asset_id,
        asset_type: placement.asset_type,
        asset_guid: placement.asset_guid,
        target_parent_id: placement.target_parent_id,
        local_position: placement
            .local_position
            .map(|position| editor_ui_model::Vec3 {
                x: position.x,
                y: position.y,
                z: position.z,
            }),
        placement_mode: placement.placement_mode,
    }));
    report.drag_drop.transaction_status = format!("{:?}", result.status);
    report.drag_drop.changed_paths = result
        .state_changes
        .iter()
        .map(|change| change.path.clone())
        .collect();
}

fn collect_picker_evidence(
    session: &mut EditorSession,
    entries: &[editor_ui_model::AssetBrowserEntry],
    scene_path: &Path,
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    let Some(replacement) = entries
        .iter()
        .find(|entry| entry.asset_id.as_deref() == Some("tex-enemy-scout"))
    else {
        report
            .diagnostics
            .push("replacement_texture_missing".to_string());
        return;
    };
    report.picker.candidate_entry_key = Some(replacement.entry_key.clone());
    let select_player =
        session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
            entity_id: "entity-player".to_string(),
        }));
    if select_player.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("picker_target_entity_select_failed".to_string());
        return;
    }

    let begin_cancel =
        session.execute_command(command_for_test(UiCommandPayload::BeginAssetPick {
            field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
        }));
    let select_cancel = session.execute_command(command_for_test(
        UiCommandPayload::SelectAssetBrowserEntry {
            entry_key: replacement.entry_key.clone(),
            additive: false,
            range: false,
        },
    ));
    if begin_cancel.status != CommandStatus::Committed
        || select_cancel.status != CommandStatus::Committed
    {
        report
            .diagnostics
            .push("picker_cancel_preview_failed".to_string());
        return;
    }
    let cancel = session.execute_command(command_for_test(UiCommandPayload::CancelAssetPick));
    report.picker.cancel_status = format!("{:?}", cancel.status);
    report.save_reload.source_hash_after_cancel = file_hash(scene_path).ok();
    report.picker.cancel_preserved_source_hash =
        report.save_reload.source_hash_before == report.save_reload.source_hash_after_cancel;

    let begin = session.execute_command(command_for_test(UiCommandPayload::BeginAssetPick {
        field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
    }));
    let select = session.execute_command(command_for_test(
        UiCommandPayload::SelectAssetBrowserEntry {
            entry_key: replacement.entry_key.clone(),
            additive: false,
            range: false,
        },
    ));
    if begin.status != CommandStatus::Committed || select.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("picker_confirm_preview_failed".to_string());
        return;
    }
    let confirm = session.execute_command(command_for_test(UiCommandPayload::ConfirmAssetPick));
    report.picker.confirm_status = format!("{:?}", confirm.status);
    if let Some(plan) = &session.asset_browser_state().last_pick_commit_plan {
        report.picker.old_asset_ref = plan.old_asset_ref.clone();
        report.picker.new_asset_ref = Some(plan.new_asset_ref.clone());
    }
    report.picker.structured_reference_written =
        inspector_asset_ref(session).is_some_and(|reference| {
            reference.asset_id == "tex-enemy-scout" && reference.asset_type_id == "texture"
        });

    let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    report.save_reload.save_status = format!("{:?}", save.status);
    report.save_reload.source_hash_after_save = file_hash(scene_path).ok();
}

fn collect_reload_evidence(
    workspace_root: &Path,
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    let mut reopened = crate::complex_shooter_editor_session();
    let open = reopened.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: workspace_root.display().to_string(),
    }));
    report.save_reload.reopen_status = format!("{:?}", open.status);
    if open.status == CommandStatus::Committed {
        let _ = reopened.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
            entity_id: "entity-player".to_string(),
        }));
        report.save_reload.reloaded_asset_ref = inspector_asset_ref(&reopened);
    }
    report.save_reload.reference_preserved = report
        .save_reload
        .reloaded_asset_ref
        .as_ref()
        .is_some_and(|reference| {
            reference.asset_id == "tex-enemy-scout" && reference.asset_type_id == "texture"
        });
}

fn collect_runtime_package_evidence(
    workspace_root: &Path,
    package_dir: &Path,
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(workspace_root),
    );
    report.runtime_package.assembly_status = format!("{:?}", assembly.status);
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        return;
    }
    let active_scene_id = assembly
        .active_scene_id
        .clone()
        .unwrap_or_else(|| "scene-main".to_string());
    let Some(build_input) = assembly.build_input else {
        return;
    };
    let build = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(package_dir, active_scene_id),
        &build_input,
    );
    report.runtime_package.build_status = format!("{:?}", build.status);
    if build.status != RuntimePackageBuildStatus::Success {
        return;
    }
    report.artifacts.push(package_dir.display().to_string());
    let load = load_runtime_package(package_dir);
    let Some(package) = load.value else {
        return;
    };
    report.runtime_package.package_loaded = true;
    let runtime_ref = package
        .active_scene
        .entities
        .iter()
        .find(|entity| entity.id == "entity-player")
        .and_then(|entity| entity.sprite_renderer2d.as_ref())
        .and_then(|sprite| sprite.sprite_ref.clone());
    report.runtime_package.runtime_scene_asset_ref = runtime_ref.clone();
    if let Some(runtime_ref) = runtime_ref {
        if let Ok(record) = package.runtime_asset_index.resolve(&runtime_ref) {
            report.runtime_package.runtime_asset_record_id = Some(record.asset_id.clone());
            report.runtime_package.runtime_asset_record_type = Some(record.asset_type.clone());
            report.runtime_package.runtime_asset_index_resolved =
                record.asset_id == "tex-enemy-scout" && record.asset_type == "texture";
        }
    }
}

fn collect_path_safety_evidence(
    workspace_root: &Path,
    output_root: &Path,
    report: &mut ComplexShooterAssetBrowserNativeReport,
) {
    if let Err(diagnostic) =
        AssetBrowserIndex::validate_project_path(workspace_root, Path::new("../outside.asset"))
    {
        report.path_safety.traversal_rejected = diagnostic.code == "asset_path_traversal";
        report.path_safety.traversal_code = Some(diagnostic.code);
    }
    let outside = output_root.join("outside.asset");
    let _ = fs::write(&outside, b"outside");
    if let Err(diagnostic) = AssetBrowserIndex::validate_project_path(workspace_root, &outside) {
        report.path_safety.root_escape_rejected = diagnostic.code == "asset_path_root_escape";
        report.path_safety.root_escape_code = Some(diagnostic.code);
    }
}

fn inspector_asset_ref(session: &EditorSession) -> Option<EditorAssetRef> {
    session
        .build_ui_model()
        .inspector
        .sections
        .iter()
        .flat_map(|section| section.fields.iter())
        .find(|field| field.field_id == "components.SpriteRenderer2D.spriteRef")
        .and_then(|field| match &field.value {
            InspectorValue::AssetRef(reference) => Some(reference.clone()),
            _ => None,
        })
}

fn validate_report(report: &mut ComplexShooterAssetBrowserNativeReport) {
    let required_kinds = [
        "Scene",
        "Prefab",
        "Rule",
        "Aui",
        "InputMapping",
        "Texture",
        "Font",
        "BuildProfile",
    ];
    for kind in required_kinds {
        if report.entry_count_by_kind.get(kind).copied().unwrap_or(0) == 0 {
            report
                .diagnostics
                .push(format!("required_asset_kind_missing:{kind}"));
        }
    }
    if !report.no_rescan_for_300_frames {
        report
            .diagnostics
            .push("asset_browser_rescanned_during_frontend_frames".to_string());
    }
    if !report.excluded_generated_roots {
        report
            .diagnostics
            .push("generated_root_leaked_into_asset_index".to_string());
    }
    if report.query_result_count < 2 || report.selected_entry_key.is_none() {
        report
            .diagnostics
            .push("asset_query_or_stable_selection_incomplete".to_string());
    }
    if !report.thumbnail.non_empty_alpha_pixel || !report.thumbnail.within_budget {
        report
            .diagnostics
            .push("thumbnail_evidence_incomplete".to_string());
    }
    if report.drag_drop.transaction_status != "Committed" {
        report
            .diagnostics
            .push("prefab_drag_drop_transaction_failed".to_string());
    }
    if !report.picker.cancel_preserved_source_hash
        || report.picker.confirm_status != "Committed"
        || !report.picker.structured_reference_written
    {
        report
            .diagnostics
            .push("picker_cancel_or_confirm_evidence_incomplete".to_string());
    }
    if report.save_reload.source_hash_before == report.save_reload.source_hash_after_save
        || !report.save_reload.reference_preserved
    {
        report
            .diagnostics
            .push("save_reload_reference_evidence_incomplete".to_string());
    }
    if !report.runtime_package.package_loaded
        || !report.runtime_package.runtime_asset_index_resolved
    {
        report
            .diagnostics
            .push("runtime_package_asset_resolution_failed".to_string());
    }
    if !report.path_safety.traversal_rejected || !report.path_safety.root_escape_rejected {
        report
            .diagnostics
            .push("asset_path_safety_evidence_incomplete".to_string());
    }
    if !report.report_panel_provider_present || report.report_panel_trace_evidence_count <= 1 {
        report
            .diagnostics
            .push("asset_browser_report_panel_trace_missing".to_string());
    }
    for domain in ["rule", "aui", "input"] {
        if report
            .opened_asset_statuses
            .get(domain)
            .is_none_or(|status| status != "Committed")
        {
            report
                .diagnostics
                .push(format!("asset_open_failed:{domain}"));
        }
    }
}

fn file_hash(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("fnv1a64:{hash:016x}"))
}

fn copy_project(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if [
            "Build", "Reports", "dist", "exports", "release", "target", ".git",
        ]
        .iter()
        .any(|excluded| name == *excluded)
        {
            continue;
        }
        let target_path = target.join(name);
        if path.is_dir() {
            copy_project(&path, &target_path)?;
        } else {
            fs::copy(path, target_path)?;
        }
    }
    Ok(())
}

fn finalize_report(
    output_root: &Path,
    mut report: ComplexShooterAssetBrowserNativeReport,
) -> ComplexShooterAssetBrowserNativeReport {
    let report_path = output_root
        .join("reports")
        .join("complex-shooter-asset-browser-native-productization-report.json");
    if let Some(parent) = report_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        if fs::write(&report_path, json).is_ok() {
            report.artifacts.push(report_path.display().to_string());
        }
    }
    report
}
