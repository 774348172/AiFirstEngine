use crate::{
    command_for_test, CommandStatus, EditorSession, PlaySessionState,
    ASSET_AUTHORING_LOOP_REPORT_SCHEMA_VERSION,
};
use editor_ui_model::{AssetPlacementMode, UiCommandPayload, Vec3};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const EDITABLE_PROJECT_LOOP_REPORT_SCHEMA_VERSION: &str = "editable-project-loop-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditableProjectLoopDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableProjectLoopDiagnostic {
    pub severity: EditableProjectLoopDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

impl EditableProjectLoopDiagnostic {
    pub fn error(
        code: impl Into<String>,
        source_stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: EditableProjectLoopDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            source_stage: source_stage.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableProjectLoopReport {
    pub schema_version: String,
    pub project_id: String,
    pub scene_id: Option<String>,
    pub opened_scene: bool,
    pub selected_entity_id: Option<String>,
    pub inspector_entity_id: Option<String>,
    pub dirty_before_save: Option<bool>,
    pub dirty_after_save: Option<bool>,
    pub transform_edit_applied: bool,
    pub undo_applied: bool,
    pub redo_applied: bool,
    pub play_started: bool,
    pub play_finished: bool,
    pub console_entry_count: usize,
    pub runtime_trace_entry_count: usize,
    pub diagnostics: Vec<EditableProjectLoopDiagnostic>,
}

impl EditableProjectLoopReport {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: EDITABLE_PROJECT_LOOP_REPORT_SCHEMA_VERSION.to_string(),
            project_id: project_id.into(),
            scene_id: None,
            opened_scene: false,
            selected_entity_id: None,
            inspector_entity_id: None,
            dirty_before_save: None,
            dirty_after_save: None,
            transform_edit_applied: false,
            undo_applied: false,
            redo_applied: false,
            play_started: false,
            play_finished: false,
            console_entry_count: 0,
            runtime_trace_entry_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultEditableProjectFixture {
    pub project_id: String,
    pub root_dir: PathBuf,
    pub scene_path: PathBuf,
    pub runtime_package_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetAuthoringLoopDiagnostic {
    pub severity: EditableProjectLoopDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

impl AssetAuthoringLoopDiagnostic {
    pub fn error(
        code: impl Into<String>,
        source_stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: EditableProjectLoopDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            source_stage: source_stage.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetAuthoringLoopReport {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_type: String,
    pub opened_scene: bool,
    pub created_entity_id: Option<String>,
    pub created_component_types: Vec<String>,
    pub dirty_before_save: Option<bool>,
    pub dirty_after_save: Option<bool>,
    pub undo_applied: bool,
    pub redo_applied: bool,
    pub play_finished: bool,
    pub console_entry_count: usize,
    pub runtime_trace_entry_count: usize,
    pub diagnostics: Vec<AssetAuthoringLoopDiagnostic>,
}

impl AssetAuthoringLoopReport {
    pub fn new(asset_id: impl Into<String>, asset_type: impl Into<String>) -> Self {
        Self {
            schema_version: ASSET_AUTHORING_LOOP_REPORT_SCHEMA_VERSION.to_string(),
            asset_id: asset_id.into(),
            asset_type: asset_type.into(),
            opened_scene: false,
            created_entity_id: None,
            created_component_types: Vec::new(),
            dirty_before_save: None,
            dirty_after_save: None,
            undo_applied: false,
            redo_applied: false,
            play_finished: false,
            console_entry_count: 0,
            runtime_trace_entry_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

pub fn create_default_editable_project_fixture() -> DefaultEditableProjectFixture {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let root_dir = std::env::temp_dir().join(format!("editable-project-loop-{stamp}"));
    let scene_dir = root_dir.join("scenes");
    let runtime_package_dir = root_dir.join("runtime-package");
    crate::ProjectLauncherState::new("0.0.3")
        .create_project(&root_dir, "Editable Project Fixture")
        .expect("fixture producer should own the new project target");
    fs::create_dir_all(&scene_dir).expect("fixture scene dir should be created");
    fs::create_dir_all(runtime_package_dir.join("scenes"))
        .expect("fixture runtime scene dir should be created");
    fs::create_dir_all(runtime_package_dir.join("assets"))
        .expect("fixture runtime asset dir should be created");
    fs::create_dir_all(runtime_package_dir.join("rules"))
        .expect("fixture runtime rule dir should be created");
    fs::create_dir_all(runtime_package_dir.join("input"))
        .expect("fixture runtime input dir should be created");
    fs::create_dir_all(runtime_package_dir.join("cooked"))
        .expect("fixture cooked asset dir should be created");

    let scene_path = scene_dir.join("main.scene.json");
    fs::write(&scene_path, editor_scene_json()).expect("editable scene fixture should be written");
    fs::write(
        runtime_package_dir.join("manifest.json"),
        runtime_manifest_json(),
    )
    .expect("runtime manifest fixture should be written");
    fs::write(
        runtime_package_dir.join("scenes").join("scene-main.json"),
        runtime_scene_json(),
    )
    .expect("runtime scene fixture should be written");
    fs::write(
        runtime_package_dir
            .join("assets")
            .join("asset-manifest.json"),
        runtime_asset_manifest_json(),
    )
    .expect("runtime asset manifest fixture should be written");
    fs::write(
        runtime_package_dir.join("cooked").join("player.glb.bin"),
        [],
    )
    .expect("fixture model cooked asset should be written");
    fs::write(
        runtime_package_dir.join("cooked").join("player.mat.json"),
        b"{}",
    )
    .expect("fixture material cooked asset should be written");
    fs::write(
        runtime_package_dir.join("rules").join("rule-manifest.json"),
        r#"{ "schemaVersion": "runtime-rule-manifest.v1", "mode": "rust-aot", "rules": [], "modules": [] }"#,
    )
    .expect("runtime rule manifest fixture should be written");
    write_empty_input_fixture(&runtime_package_dir);

    DefaultEditableProjectFixture {
        project_id: "editable-project-loop-fixture".to_string(),
        root_dir,
        scene_path,
        runtime_package_dir,
    }
}

pub fn open_default_editable_scene_for_test() -> (EditorSession, DefaultEditableProjectFixture) {
    let fixture = create_default_editable_project_fixture();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: fixture.scene_path.display().to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    (session, fixture)
}

pub fn run_editable_project_loop_headless() -> EditableProjectLoopReport {
    let fixture = create_default_editable_project_fixture();
    let mut session = EditorSession::new();
    let mut report = EditableProjectLoopReport::new(fixture.project_id.clone());

    let open = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: fixture.scene_path.display().to_string(),
    }));
    report.opened_scene = open.status == CommandStatus::Committed;
    if open.status != CommandStatus::Committed {
        report
            .diagnostics
            .push(EditableProjectLoopDiagnostic::error(
                "editable_project_loop.open_scene_failed",
                "open_scene",
                "Default editable scene did not open.",
            ));
        return finalize_report(report, &session);
    }

    let model = session.build_ui_model();
    report.scene_id = model.hierarchy.scene_id.clone();

    let select = session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));
    if select.status != CommandStatus::Committed {
        report
            .diagnostics
            .push(EditableProjectLoopDiagnostic::error(
                "editable_project_loop.select_entity_failed",
                "select_entity",
                "Default player entity was not selected.",
            ));
        return finalize_report(report, &session);
    }
    let model = session.build_ui_model();
    report.selected_entity_id = model.hierarchy.selected_entity_id.clone();
    report.inspector_entity_id = model.inspector.selected_entity_id.clone();

    let edit = session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(Vec3 {
            x: 5.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));
    report.transform_edit_applied = edit.status == CommandStatus::Committed;
    report.dirty_before_save = session.scene_dirty();

    let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    if save.status != CommandStatus::Committed {
        report
            .diagnostics
            .push(EditableProjectLoopDiagnostic::error(
                "editable_project_loop.save_failed",
                "save_scene",
                "Default editable scene did not save.",
            ));
    }
    report.dirty_after_save = session.scene_dirty();

    let undo = session.execute_command(command_for_test(UiCommandPayload::UndoSceneEdit));
    report.undo_applied = undo.status == CommandStatus::Committed;

    let redo = session.execute_command(command_for_test(UiCommandPayload::RedoSceneEdit));
    report.redo_applied = redo.status == CommandStatus::Committed;

    let runtime_open =
        session.execute_command(command_for_test(UiCommandPayload::OpenRuntimePackage {
            path: fixture.runtime_package_dir.display().to_string(),
        }));
    if runtime_open.status != CommandStatus::Committed {
        report
            .diagnostics
            .push(EditableProjectLoopDiagnostic::error(
                "editable_project_loop.open_runtime_package_failed",
                "open_runtime_package",
                "Runtime package fixture did not open.",
            ));
        return finalize_report(report, &session);
    }

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    report.play_started = play
        .console_entries
        .iter()
        .any(|entry| entry.message.contains("Play session started"));
    report.play_finished = session
        .last_play_session_report()
        .is_some_and(|play_report| play_report.state == PlaySessionState::Completed);
    if play.status != CommandStatus::Committed {
        report
            .diagnostics
            .push(EditableProjectLoopDiagnostic::error(
                "editable_project_loop.play_failed",
                "play_session",
                "Play current scene did not complete.",
            ));
    }

    finalize_report(report, &session)
}

pub fn run_asset_authoring_loop_headless() -> AssetAuthoringLoopReport {
    let fixture = create_default_editable_project_fixture();
    let asset_id = "model-enemy".to_string();
    let asset_type = "model".to_string();
    let mut session = EditorSession::new();
    let mut report = AssetAuthoringLoopReport::new(asset_id.clone(), asset_type.clone());

    let open = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: fixture.scene_path.display().to_string(),
    }));
    report.opened_scene = open.status == CommandStatus::Committed;
    if open.status != CommandStatus::Committed {
        report.diagnostics.push(AssetAuthoringLoopDiagnostic::error(
            "asset_authoring_loop.open_scene_failed",
            "open_scene",
            "Default editable scene did not open.",
        ));
        return finalize_asset_authoring_report(report, &session);
    }

    let place = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: asset_id.clone(),
        asset_type: asset_type.clone(),
        asset_guid: Some("guid-model-enemy".to_string()),
        target_parent_id: None,
        local_position: Some(Vec3 {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        }),
        placement_mode: AssetPlacementMode::WorldOrigin,
    }));
    if place.status != CommandStatus::Committed {
        report.diagnostics.push(AssetAuthoringLoopDiagnostic::error(
            "asset_authoring_loop.place_asset_failed",
            "place_asset",
            "Asset placement did not commit.",
        ));
        return finalize_asset_authoring_report(report, &session);
    }
    let model = session.build_ui_model();
    report.created_entity_id = model.hierarchy.selected_entity_id.clone();
    report.dirty_before_save = session.scene_dirty();

    let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    if save.status != CommandStatus::Committed {
        report.diagnostics.push(AssetAuthoringLoopDiagnostic::error(
            "asset_authoring_loop.save_failed",
            "save_scene",
            "Scene did not save after asset placement.",
        ));
    }
    report.dirty_after_save = session.scene_dirty();

    let undo = session.execute_command(command_for_test(UiCommandPayload::UndoSceneEdit));
    report.undo_applied = undo.status == CommandStatus::Committed;

    let redo = session.execute_command(command_for_test(UiCommandPayload::RedoSceneEdit));
    report.redo_applied = redo.status == CommandStatus::Committed;

    let runtime_open =
        session.execute_command(command_for_test(UiCommandPayload::OpenRuntimePackage {
            path: fixture.runtime_package_dir.display().to_string(),
        }));
    if runtime_open.status != CommandStatus::Committed {
        report.diagnostics.push(AssetAuthoringLoopDiagnostic::error(
            "asset_authoring_loop.open_runtime_package_failed",
            "open_runtime_package",
            "Runtime package fixture did not open.",
        ));
        return finalize_asset_authoring_report(report, &session);
    }

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    report.play_finished = play.status == CommandStatus::Committed
        && session
            .last_play_session_report()
            .is_some_and(|play_report| play_report.state == PlaySessionState::Completed);
    if !report.play_finished {
        report.diagnostics.push(AssetAuthoringLoopDiagnostic::error(
            "asset_authoring_loop.play_failed",
            "play_session",
            "Play current scene did not complete.",
        ));
    }

    finalize_asset_authoring_report(report, &session)
}

fn finalize_report(
    mut report: EditableProjectLoopReport,
    session: &EditorSession,
) -> EditableProjectLoopReport {
    let model = session.build_ui_model();
    report.console_entry_count = model.console.entries.len();
    report.runtime_trace_entry_count = model.runtime_trace.entries.len();
    report
}

fn finalize_asset_authoring_report(
    mut report: AssetAuthoringLoopReport,
    session: &EditorSession,
) -> AssetAuthoringLoopReport {
    let model = session.build_ui_model();
    report.console_entry_count = model.console.entries.len();
    report.runtime_trace_entry_count = model.runtime_trace.entries.len();
    report
}

fn editor_scene_json() -> &'static str {
    r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
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
    "mesh": {
      "primitive": "model",
      "assetRef": { "id": "model-player", "type": "model" },
      "materialRef": { "id": "mat-player", "type": "material" },
      "visible": true,
      "layer": "default"
    },
    "components": [{
      "componentType": "game.health",
      "fields": { "hp": 10, "maxHp": 10 }
    }]
  }]
}"##
}

fn runtime_manifest_json() -> &'static str {
    r##"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "editable-project-loop-fixture",
    "name": "Editable Project Loop Fixture",
    "version": "0.0.3",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 3 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "rust-aot" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
  "contentHash": null
}"##
}

fn write_empty_input_fixture(package_dir: &std::path::Path) {
    fs::write(
        package_dir.join("input").join("input-manifest.json"),
        r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
    )
    .expect("runtime input manifest fixture should be written");
    fs::write(
        package_dir.join("input").join("input.none.json"),
        r#"{
  "schema_version": "input-mapping.v2",
  "asset_id": "input.none",
  "actions": [],
  "contexts": [],
  "bindings": [],
  "platform_overrides": []
}"#,
    )
    .expect("runtime input mapping fixture should be written");
}

fn runtime_scene_json() -> &'static str {
    r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
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
    "mesh": {
      "primitive": "model",
      "assetRef": { "id": "model-player", "type": "model" },
      "materialRef": { "id": "mat-player", "type": "material" },
      "visible": true,
      "layer": "default"
    }
  }]
}"##
}

fn runtime_asset_manifest_json() -> &'static str {
    r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [
    { "id": "scene-main", "name": "Main", "type": "scene", "source": "scenes/scene-main.json", "state": "available", "bundleId": "startup" },
    { "id": "model-player", "name": "Player", "type": "model", "source": "player.glb", "state": "available", "bundleId": "startup" },
    { "id": "mat-player", "name": "Player Material", "type": "material", "source": "player.mat", "state": "available", "bundleId": "startup" }
  ],
  "runtimeAssetIndex": [
    {
      "assetGuid": "scene-main",
      "assetId": "scene-main",
      "assetType": "scene",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-scene-main",
      "bundleId": "startup",
      "loaderKind": "scene",
      "dependencies": [],
      "hash": null,
      "size": null,
      "flags": ["test"]
    },
    {
      "assetGuid": "model-player",
      "assetId": "model-player",
      "assetType": "model",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-model-player",
      "bundleId": "startup",
      "loaderKind": "model",
      "dependencies": [],
      "hash": null,
      "size": 0,
      "flags": ["test"]
    },
    {
      "assetGuid": "mat-player",
      "assetId": "mat-player",
      "assetType": "material",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-mat-player",
      "bundleId": "startup",
      "loaderKind": "material",
      "dependencies": [],
      "hash": null,
      "size": 2,
      "flags": ["test"]
    }
  ],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [
    {
      "cookedAssetId": "cooked-scene-main",
      "bundleId": "startup",
      "path": "scenes/scene-main.json",
      "offset": null,
      "size": null,
      "compression": "none",
      "hash": null
    },
    {
      "cookedAssetId": "cooked-model-player",
      "bundleId": "startup",
      "path": "cooked/player.glb.bin",
      "offset": null,
      "size": 0,
      "compression": "none",
      "hash": null
    },
    {
      "cookedAssetId": "cooked-mat-player",
      "bundleId": "startup",
      "path": "cooked/player.mat.json",
      "offset": null,
      "size": 2,
      "compression": "none",
      "hash": null
    }
  ],
  "dependencyTable": []
}"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_project_loop_report_serializes() {
        let mut report = EditableProjectLoopReport::new("project-main");
        report.scene_id = Some("scene-main".to_string());
        report.opened_scene = true;

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(EDITABLE_PROJECT_LOOP_REPORT_SCHEMA_VERSION));
        assert!(json.contains("scene-main"));
    }

    #[test]
    fn editable_project_loop_report_records_diagnostics() {
        let mut report = EditableProjectLoopReport::new("project-main");
        report
            .diagnostics
            .push(EditableProjectLoopDiagnostic::error(
                "editable_project_loop.failed",
                "test",
                "failure",
            ));

        assert_eq!(report.diagnostics[0].code, "editable_project_loop.failed");
    }

    #[test]
    fn editable_project_loop_opens_default_scene() {
        let (session, _fixture) = open_default_editable_scene_for_test();

        assert_eq!(
            session.build_ui_model().hierarchy.scene_id.as_deref(),
            Some("scene-main")
        );
    }

    #[test]
    fn editable_project_loop_default_scene_has_player_entity() {
        let (session, _fixture) = open_default_editable_scene_for_test();

        assert!(session
            .build_ui_model()
            .hierarchy
            .roots
            .iter()
            .any(|node| node.entity_id == "entity-player"));
    }

    #[test]
    fn editable_project_loop_end_to_end_headless() {
        let report = run_editable_project_loop_headless();

        assert!(report.opened_scene);
        assert_eq!(report.scene_id.as_deref(), Some("scene-main"));
        assert_eq!(report.selected_entity_id.as_deref(), Some("entity-player"));
        assert_eq!(report.inspector_entity_id.as_deref(), Some("entity-player"));
        assert_eq!(report.dirty_before_save, Some(true));
        assert_eq!(report.dirty_after_save, Some(false));
        assert!(report.transform_edit_applied);
        assert!(report.undo_applied);
        assert!(report.redo_applied);
        assert!(report.play_started);
        assert!(report.play_finished);
        assert!(report.console_entry_count > 0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn editable_project_loop_report_is_ai_readable() {
        let report = run_editable_project_loop_headless();
        let json = serde_json::to_string_pretty(&report).expect("report should serialize");

        assert!(json.contains("editable-project-loop-report.v1"));
        assert!(json.contains("transformEditApplied"));
        assert!(json.contains("playFinished"));
    }

    #[test]
    fn asset_authoring_loop_report_serializes() {
        let report = AssetAuthoringLoopReport::new("model-enemy", "model");
        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains("asset-authoring-loop-report.v1"));
        assert!(json.contains("model-enemy"));
    }

    #[test]
    fn asset_authoring_loop_headless_places_saves_undoes_redoes_and_plays() {
        let report = run_asset_authoring_loop_headless();

        assert!(report.opened_scene);
        assert_eq!(
            report.created_entity_id.as_deref(),
            Some("entity-model-enemy")
        );
        assert_eq!(report.dirty_before_save, Some(true));
        assert_eq!(report.dirty_after_save, Some(false));
        assert!(report.undo_applied);
        assert!(report.redo_applied);
        assert!(report.play_finished);
        assert!(report.console_entry_count > 0);
        assert!(report.diagnostics.is_empty());
    }
}
