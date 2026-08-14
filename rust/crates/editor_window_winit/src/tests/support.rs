use super::*;

struct TestLinkedProjectRuntime {
    descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor,
}

impl engine_runtime::project_runtime_module::ProjectRuntimeModule for TestLinkedProjectRuntime {
    fn descriptor(
        &self,
    ) -> &engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        _registration: &mut engine_runtime::project_runtime_module::ProjectRuntimeRegistration,
    ) -> Result<(), engine_runtime::project_runtime_module::ProjectRuntimeError> {
        Ok(())
    }
}

pub(super) fn session_with_linked_project_runtime(module_id: &str) -> EditorSession {
    let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
        std::sync::Arc::new(TestLinkedProjectRuntime {
            descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                module_id,
                "sha256:test-linked-runtime",
            ),
        }),
    )
    .expect("test linked project runtime must form a singleton composition");
    EditorSession::with_linked_project_runtimes(std::sync::Arc::new(linked))
}

pub(super) fn pump_editor_play_until_terminal(
    app: &mut NativeEditorApplication,
) -> NativeEditorApplicationReport {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let report = app.frame(1280.0, 720.0);
        if report.last_command_status != Some(CommandStatus::Pending) {
            return report;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Editor Play preparation did not reach a terminal status: {:?}",
            report.last_feedback
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

pub(super) fn fixture_model() -> EditorUiModel {
    EditorUiModel {
        revision: 1,
        frame: 1,
        mode: EditorUiMode::AuthoringWorkspace,
        project_launcher: editor_ui_model::ProjectLauncherModel::empty(),
        project_intent: editor_ui_model::ProjectIntentWorkspaceModel::empty(),
        project_browser: editor_ui_model::ProjectBrowserModel::empty(),
        asset_browser: editor_ui_model::AssetBrowserModel::empty(),
        build_export: BuildExportModel::empty(),
        report_panel: editor_ui_model::ReportPanelModel::empty(),
        input_mapping_authoring: editor_ui_model::InputMappingAuthoringModel::empty(),
        rule_authoring: editor_ui_model::RuleAuthoringModel::empty(),
        animator2d_authoring: editor_ui_model::Animator2DAuthoringModel::default(),
        project_authoring_workspace: editor_ui_model::ProjectAuthoringWorkspaceModel::empty(),
        authoring_workflow: editor_ui_model::AuthoringWorkflowModel::empty(),
        workspace_view_mode: editor_ui_model::WorkspaceViewMode::SceneView,
        active_runtime_package: None,
        panels: PanelLayoutModel::fixed_mvp(),
        toolbar: ToolbarModel {
            commands: vec![ToolbarCommand {
                command_id: "tick_one_frame".to_string(),
                label: "Tick".to_string(),
                enabled: true,
                reason_disabled: None,
            }],
            runtime_state: RuntimeRunState::Paused,
            game_view_layout: editor_ui_model::GameViewLayoutState::default(),
        },
        hierarchy: HierarchyModel {
            scene_id: Some("scene-main".to_string()),
            roots: Vec::new(),
            selected_entity_id: None,
            authoring_view: editor_ui_model::HierarchyAuthoringView::EntityTree,
            visual_order: None,
            source_domain: editor_ui_model::HierarchySourceDomain::AuthoringScene,
            status: "authoring_scene".to_string(),
        },
        inspector: InspectorModel {
            selected_entity_id: None,
            title: "No Selection".to_string(),
            sections: Vec::new(),
            readonly: true,
            persistence: editor_ui_model::InspectorPersistence::ReadOnly,
        },
        viewport: ViewportModel {
            scene_id: Some("scene-main".to_string()),
            frame: 1,
            frame_hash: None,
            texture_id: None,
            target_id: None,
            renderable_count: 0,
            selected_entity: None,
            renderables: Vec::new(),
            collider_overlay: editor_ui_model::ColliderOverlayModel::default(),
        },
        console: ConsoleModel {
            entries: Vec::new(),
            unread_error_count: 0,
            unread_warning_count: 0,
        },
        runtime_trace: RuntimeTraceModel {
            frame: 0,
            entries: Vec::new(),
            selected_entry_id: None,
        },
        ai_panel: editor_ui_model::AiPanelModel {
            prompt_placeholder: "Describe an editor change...".to_string(),
            prompt_draft: String::new(),
            messages: Vec::new(),
            gateway_access: Default::default(),
            proposed_commands: Vec::new(),
            allowed_command_ids: Vec::new(),
            busy: false,
            stage: editor_ui_model::AiPanelStage::Idle,
            status_summary: None,
        },
        project_runtime_trust_prompt: None,
        interaction_feedback: None,
        diagnostics: Vec::new(),
    }
}

pub(super) fn rect(x: f32, y: f32, width: f32, height: f32) -> UiRect {
    UiRect {
        x,
        y,
        width,
        height,
    }
}

fn fire_moves_player_rule(context: &mut LogicContext<'_>) -> LogicResult {
    const RULE_ID: &str = "project.fire_move";
    if !context.action_pressed("action.fire") {
        return LogicResult::skipped(RULE_ID, ExecutorKind::RustAot);
    }
    let entity_id = EntityId::from("entity-player");
    let mut position = context
        .read_transform_local_position(&entity_id)
        .expect("player transform should exist");
    position.x += 1.0;
    let write = context
        .write_transform_local_position(entity_id, position)
        .expect("write should succeed");
    let mut result = LogicResult::applied(RULE_ID, ExecutorKind::RustAot);
    result.writes.push(write);
    result
}

pub(super) fn fire_move_runner() -> ProjectLogicRunner {
    let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
        fixed_update: Vec::new(),
        frame_update: vec![RuleCall::rust_aot("project.fire_move")],
        post_physics: Vec::new(),
        event_handler: Vec::new(),
    });
    runner.register_rust_aot_rule("project.fire_move", fire_moves_player_rule);
    runner
}

pub(super) fn write_editor_project_fixture_for_shell() -> std::path::PathBuf {
    use std::fs;

    let root = unique_project_launcher_temp_dir();
    let mut session = EditorSession::new();
    let create = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "ShellFixture".to_string(),
        },
    ));
    assert_eq!(create.status, CommandStatus::Committed);
    let scene_path = root.join("Scenes").join("Main.scene.json");
    fs::write(
        &scene_path,
        r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
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
      "visible": true,
      "layer": "default"
    }
  }]
}"##,
    )
    .expect("fixture scene");
    root
}

pub(super) fn write_runtime_package_fixture_for_shell(
    root: &std::path::Path,
    name: &str,
) -> std::path::PathBuf {
    use std::fs;

    let package_dir = root.join(name);
    fs::create_dir_all(package_dir.join("scenes")).unwrap();
    fs::create_dir_all(package_dir.join("assets")).unwrap();
    fs::create_dir_all(package_dir.join("rules")).unwrap();
    fs::create_dir_all(package_dir.join("input")).unwrap();
    fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "runtime-shell-test",
    "name": "Runtime Shell Test",
    "version": "0.0.1",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "rust-aot" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
  "contentHash": null
}"#,
        )
        .unwrap();
    fs::write(
        package_dir.join("scenes").join("scene-main.json"),
        r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#101010",
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
    }
  }]
}"##,
    )
    .unwrap();
    fs::write(
        package_dir.join("assets").join("asset-manifest.json"),
        r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [{
    "id": "scene-main",
    "name": "Main",
    "type": "scene",
    "source": "scenes/scene-main.json",
    "state": "available",
    "bundleId": "startup"
  }],
  "runtimeAssetIndex": [{
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
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "path": "scenes/scene-main.json",
    "offset": null,
    "size": null,
    "compression": "none",
    "hash": null
  }],
  "dependencyTable": []
}"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("rules").join("rule-manifest.json"),
        r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "rust-aot",
  "rules": [],
  "modules": []
}"#,
    )
    .unwrap();
    write_empty_input_fixture(&package_dir);
    package_dir
}

fn write_empty_input_fixture(package_dir: &std::path::Path) {
    std::fs::write(
        package_dir.join("input").join("input-manifest.json"),
        r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
    )
    .unwrap();
    std::fs::write(
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
    .unwrap();
}

pub(super) fn complex_shooter_project_fixture_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/complex_shooter_project")
}

pub(super) fn copy_complex_shooter_project_fixture() -> std::path::PathBuf {
    let source = complex_shooter_project_fixture_root();
    let destination = unique_project_launcher_temp_dir();
    copy_project_source_tree(&source, &destination);
    destination
}

fn copy_project_source_tree(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        if entry.file_type().unwrap().is_dir()
            && matches!(entry.file_name().to_str(), Some("Build" | "Library"))
        {
            continue;
        }
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_project_source_tree(&source_path, &destination_path);
        } else {
            std::fs::copy(source_path, destination_path).unwrap();
        }
    }
}

pub(super) fn opened_editor_project_session(project_root: &std::path::Path) -> EditorSession {
    let mut session = EditorSession::new();
    let result = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        },
    ));
    assert_eq!(result.status, CommandStatus::Committed);
    session
}

pub(super) fn unique_project_launcher_temp_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be monotonic")
        .as_nanos();
    let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "native-editor-project-launcher-{}-{stamp}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn project_launcher_temp_dirs_are_unique_under_parallel_creation() {
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};

    let barrier = Arc::new(Barrier::new(64));
    let workers = (0..64)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                unique_project_launcher_temp_dir()
            })
        })
        .collect::<Vec<_>>();
    let paths = workers
        .into_iter()
        .map(|worker| worker.join().expect("temp path worker"))
        .collect::<HashSet<_>>();

    assert_eq!(paths.len(), 64);
}
