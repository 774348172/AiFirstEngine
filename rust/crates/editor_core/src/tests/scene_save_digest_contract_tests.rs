use super::*;

struct TestProjectRoot(PathBuf);

impl Drop for TestProjectRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&source_path, &target_path);
        } else {
            fs::copy(source_path, target_path).unwrap();
        }
    }
}

fn open_legacy_switch_puzzle() -> (EditorSession, TestProjectRoot) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/switch_puzzle_project")
        .canonicalize()
        .unwrap();
    let project_root = std::env::temp_dir().join(format!(
        "scene-save-digest-contract-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    copy_tree(&source, &project_root);

    let mut session = fixtures::session_with_linked_project_runtime("sample.switch-puzzle.runtime");
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    }));
    assert_eq!(
        open.status,
        CommandStatus::Committed,
        "OpenProject diagnostics: {:?}",
        open.diagnostics
    );
    (session, TestProjectRoot(project_root))
}

fn save_active_scene(session: &mut EditorSession, path: Option<&Path>) -> crate::SceneSaveReport {
    let scope = session
        .active_project_session()
        .unwrap()
        .write_scope()
        .clone();
    SceneSavePipeline::save_in_scope(
        session.editor_scene_document.as_mut().unwrap(),
        &scope,
        path,
    )
}

#[test]
fn clean_same_path_save_preserves_legacy_scene_bytes_mtime_and_project_digest() {
    let (mut session, project_root) = open_legacy_switch_puzzle();
    let scene_path = project_root.0.join("Scenes/Main.scene.json");
    assert_eq!(session.scene_dirty(), Some(false));
    let before_bytes = fs::read(&scene_path).unwrap();
    let before_modified = fs::metadata(&scene_path).unwrap().modified().unwrap();
    let before_digest = ProjectCandidateEntry::inspect_project_binding(&session)
        .unwrap()
        .project_digest;

    let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    assert_eq!(save.status, CommandStatus::Committed);
    assert!(save
        .diagnostics
        .iter()
        .any(|entry| entry.code == "editor.scene_document.unchanged"));
    assert!(!save
        .diagnostics
        .iter()
        .any(|entry| entry.code == "editor.scene_document.saved"));

    let after_bytes = fs::read(&scene_path).unwrap();
    let after_modified = fs::metadata(&scene_path).unwrap().modified().unwrap();
    let after_digest = ProjectCandidateEntry::inspect_project_binding(&session)
        .unwrap()
        .project_digest;
    assert_eq!(before_bytes, after_bytes);
    assert_eq!(before_modified, after_modified);
    assert_eq!(before_digest, after_digest);
}

#[test]
fn dirty_scene_with_changed_bytes_is_saved_and_clears_dirty() {
    let (mut session, project_root) = open_legacy_switch_puzzle();
    let scene_path = project_root.0.join("Scenes/Main.scene.json");
    let before = fs::read(&scene_path).unwrap();
    let document = session.editor_scene_document.as_mut().unwrap();
    document.name = "Changed Switch Puzzle".to_string();
    document.mark_dirty("test-change");

    let report = save_active_scene(&mut session, None);

    assert_eq!(report.status, SceneSaveStatus::Saved);
    assert!(!report.dirty_after);
    assert_eq!(session.scene_dirty(), Some(false));
    assert_ne!(before, fs::read(scene_path).unwrap());
}

#[test]
fn dirty_scene_with_equal_serialized_bytes_is_unchanged_and_clears_dirty() {
    let (mut session, project_root) = open_legacy_switch_puzzle();
    let scene_path = project_root.0.join("Scenes/Main.scene.json");
    session
        .editor_scene_document
        .as_mut()
        .unwrap()
        .mark_dirty("canonicalize");
    assert_eq!(
        save_active_scene(&mut session, None).status,
        SceneSaveStatus::Saved
    );
    let canonical_bytes = fs::read(&scene_path).unwrap();
    let canonical_modified = fs::metadata(&scene_path).unwrap().modified().unwrap();
    session
        .editor_scene_document
        .as_mut()
        .unwrap()
        .mark_dirty("no-semantic-change");

    let report = save_active_scene(&mut session, None);

    assert_eq!(report.status, SceneSaveStatus::Unchanged);
    assert!(!report.dirty_after);
    assert_eq!(session.scene_dirty(), Some(false));
    assert_eq!(canonical_bytes, fs::read(&scene_path).unwrap());
    assert_eq!(
        canonical_modified,
        fs::metadata(scene_path).unwrap().modified().unwrap()
    );
}

#[test]
fn canonical_clean_second_save_is_unchanged() {
    let (mut session, project_root) = open_legacy_switch_puzzle();
    let scene_path = project_root.0.join("Scenes/Main.scene.json");
    session
        .editor_scene_document
        .as_mut()
        .unwrap()
        .mark_dirty("canonicalize");
    assert_eq!(
        save_active_scene(&mut session, None).status,
        SceneSaveStatus::Saved
    );
    let before = fs::read(&scene_path).unwrap();
    let before_modified = fs::metadata(&scene_path).unwrap().modified().unwrap();

    let report = save_active_scene(&mut session, None);

    assert_eq!(report.status, SceneSaveStatus::Unchanged);
    assert_eq!(before, fs::read(&scene_path).unwrap());
    assert_eq!(
        before_modified,
        fs::metadata(scene_path).unwrap().modified().unwrap()
    );
}

#[test]
fn clean_save_as_creates_a_different_target() {
    let (mut session, project_root) = open_legacy_switch_puzzle();
    let save_as_path = project_root.0.join("Scenes/Copy.scene.json");
    assert!(!save_as_path.exists());

    let report = save_active_scene(&mut session, Some(&save_as_path));

    assert_eq!(report.status, SceneSaveStatus::Saved);
    assert!(save_as_path.exists());
    assert_eq!(
        session
            .editor_scene_document()
            .unwrap()
            .scene_path
            .as_deref(),
        Some(save_as_path.as_path())
    );
}

#[test]
fn clean_save_recreates_a_missing_current_target() {
    let (mut session, project_root) = open_legacy_switch_puzzle();
    let scene_path = project_root.0.join("Scenes/Main.scene.json");
    fs::remove_file(&scene_path).unwrap();
    assert_eq!(session.scene_dirty(), Some(false));

    let report = save_active_scene(&mut session, None);

    assert_eq!(report.status, SceneSaveStatus::Saved);
    assert!(scene_path.exists());
    assert_eq!(session.scene_dirty(), Some(false));
}
