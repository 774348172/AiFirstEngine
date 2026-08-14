use super::*;
use editor_ui_renderer::{
    DockNode, EditorWorkspaceDockingModule, LayoutNodeId, PanelId, WorkspaceIntent,
    WorkspaceWindowId, WorkspaceWindowPlacement, WorkspaceWindowRoot,
};

#[test]
fn workspace_layout_store_roundtrips_and_atomically_replaces_canonical_file() {
    let root = unique_workspace_store_root("roundtrip");
    let path = root.join("editor-workspace-layout.v2.json");
    let store = WorkspaceLayoutStore::new(path.clone());
    let missing = store.load();
    assert!(missing.topology.is_none());
    assert_eq!(missing.diagnostics[0].code, "workspace_layout_missing");

    let mut module = EditorWorkspaceDockingModule::standard_editor();
    module.update(WorkspaceIntent::ClosePanel {
        panel_id: editor_ui_renderer::PanelId::new("ai_panel").unwrap(),
    });
    let first = store.save(&module.topology());
    assert!(first.written, "{:?}", first.diagnostics);
    std::fs::write(
        root.join("editor-workspace-layout.v2.json.tmp-interrupted"),
        b"{partial",
    )
    .unwrap();
    let loaded = store.load();
    assert_eq!(loaded.topology.as_ref(), Some(&module.topology()));
    std::fs::remove_file(root.join("editor-workspace-layout.v2.json.tmp-interrupted")).unwrap();

    module.update(WorkspaceIntent::ShowPanel {
        panel_id: editor_ui_renderer::PanelId::new("ai_panel").unwrap(),
    });
    let second = store.save(&module.topology());
    assert!(second.written, "{:?}", second.diagnostics);
    assert_eq!(store.load().topology.as_ref(), Some(&module.topology()));
    assert!(path.exists());
    assert_eq!(
        std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count(),
        0
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_layout_store_reports_malformed_write_failure_and_ignores_partial_temp() {
    let root = unique_workspace_store_root("negative");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("editor-workspace-layout.v2.json");
    std::fs::write(&path, b"{not-json").unwrap();
    std::fs::write(
        root.join("editor-workspace-layout.v1.json.tmp-interrupted"),
        b"{",
    )
    .unwrap();
    let store = WorkspaceLayoutStore::new(path);
    let malformed = store.load();
    assert!(malformed.topology.is_none());
    assert_eq!(malformed.diagnostics[0].code, "workspace_layout_malformed");

    let blocked_parent = root.join("blocked-parent");
    std::fs::write(&blocked_parent, b"not-a-directory").unwrap();
    let blocked_store = WorkspaceLayoutStore::new(blocked_parent.join("layout.json"));
    let save = blocked_store.save(&EditorWorkspaceDockingModule::standard_editor().topology());
    assert!(!save.written);
    assert_eq!(save.diagnostics[0].code, "workspace_layout_write_failed");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_application_restores_persists_and_resets_without_project_mutation() {
    let root = unique_workspace_store_root("application");
    let path = root.join("editor-workspace-layout.v2.json");
    let store = WorkspaceLayoutStore::new(path);
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session)
            .with_workspace_layout_store(store.clone());
    let project_revision = app.latest_model().revision;
    assert!(
        app.close_workspace_panel("ai_panel").changed,
        "closable panel"
    );
    assert!(store
        .load()
        .topology
        .unwrap()
        .main_root
        .closed_panels
        .contains(&editor_ui_renderer::PanelId::new("ai_panel").unwrap()));
    assert_eq!(app.latest_model().revision, project_revision);

    let session = opened_editor_project_session(&project_root);
    let mut reopened =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session)
            .with_workspace_layout_store(store.clone());
    assert!(reopened
        .workspace_docking()
        .layout()
        .closed_panels
        .contains(&editor_ui_renderer::PanelId::new("ai_panel").unwrap()));
    assert!(reopened.show_workspace_panel("ai_panel").changed);
    assert!(reopened.reset_workspace_layout().changed);
    assert!(store
        .load()
        .topology
        .unwrap()
        .main_root
        .closed_panels
        .is_empty());
    assert_eq!(reopened.latest_model().revision, project_revision);

    drop(reopened);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn workspace_application_corrupt_restore_and_write_failure_are_non_blocking() {
    let root = unique_workspace_store_root("application-negative");
    std::fs::create_dir_all(&root).unwrap();
    let corrupt_path = root.join("corrupt.json");
    std::fs::write(&corrupt_path, b"{broken").unwrap();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_workspace_layout_store(WorkspaceLayoutStore::new(corrupt_path));
    assert!(app
        .workspace_persistence_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "workspace_layout_malformed"));
    assert!(app.workspace_docking().layout().closed_panels.is_empty());
    assert!(app.reset_workspace_layout().changed);

    let blocked_parent = root.join("blocked");
    std::fs::write(&blocked_parent, b"file").unwrap();
    let mut blocked = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_workspace_layout_store(WorkspaceLayoutStore::new(
            blocked_parent.join("layout.json"),
        ));
    assert!(blocked.close_workspace_panel("ai_panel").changed);
    assert!(blocked
        .workspace_persistence_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "workspace_layout_write_failed"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_persistence_migrates_v1_and_discards_only_invalid_floating_root() {
    let migration_root = unique_workspace_store_root("migration");
    std::fs::create_dir_all(&migration_root).unwrap();
    let store = WorkspaceLayoutStore::new(migration_root.join("editor-workspace-layout.v2.json"));
    let mut legacy = EditorWorkspaceDockingModule::standard_editor();
    legacy.update(WorkspaceIntent::ClosePanel {
        panel_id: PanelId::new("ai_panel").unwrap(),
    });
    std::fs::write(
        migration_root.join("editor-workspace-layout.v1.json"),
        serde_json::to_vec_pretty(legacy.layout()).unwrap(),
    )
    .unwrap();
    let migrated = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_workspace_layout_store(store.clone());
    assert!(migrated
        .workspace_docking()
        .layout()
        .closed_panels
        .contains(&PanelId::new("ai_panel").unwrap()));
    assert!(migrated
        .workspace_persistence_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "workspace_layout_v1_migration_required"));
    assert!(store.path().exists(), "migration writes canonical v2");
    drop(migrated);

    let partial_root = unique_workspace_store_root("partial-floating");
    std::fs::create_dir_all(&partial_root).unwrap();
    let partial_store =
        WorkspaceLayoutStore::new(partial_root.join("editor-workspace-layout.v2.json"));
    let mut module = EditorWorkspaceDockingModule::standard_editor();
    module.update(WorkspaceIntent::FloatPanel {
        panel_id: PanelId::new("ai_panel").unwrap(),
        window_id: WorkspaceWindowId::new("floating-valid").unwrap(),
        placement: WorkspaceWindowPlacement::default(),
    });
    let mut topology = module.topology();
    topology.floating_roots.push(WorkspaceWindowRoot {
        window_id: WorkspaceWindowId::new("floating-invalid").unwrap(),
        root: DockNode::Stack {
            node_id: LayoutNodeId::new("workspace/floating-invalid/root").unwrap(),
            tabs: vec![PanelId::new("unknown-panel").unwrap()],
            active_panel_id: PanelId::new("unknown-panel").unwrap(),
        },
        placement: WorkspaceWindowPlacement::default(),
    });
    std::fs::write(
        partial_store.path(),
        serde_json::to_vec_pretty(&topology).unwrap(),
    )
    .unwrap();
    let restored = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_workspace_layout_store(partial_store);
    let restored_topology = restored.workspace_docking().topology();
    assert_eq!(restored_topology.floating_roots.len(), 1);
    assert_eq!(
        restored_topology.floating_roots[0].window_id.as_str(),
        "floating-valid"
    );
    assert!(restored
        .workspace_persistence_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "discarded_invalid_floating_root"));

    drop(restored);
    std::fs::remove_dir_all(migration_root).unwrap();
    std::fs::remove_dir_all(partial_root).unwrap();
}

fn unique_workspace_store_root(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ai-first-editor-workspace-{label}-{}-{nonce}",
        std::process::id()
    ))
}
