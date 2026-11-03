use engine_runtime::aui::{
    AuiActionRef, AuiBindingRef, AuiBindingTarget, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind,
    AuiRect,
};
use project_authoring_execution::{
    inspect_aui_project_snapshot, inspect_aui_project_snapshot_with_request, AuiInspectionMode,
    AuiInspectionRequest, CheckProfile, GameProjectCompiler, ProjectAuthoringSession,
    TargetProfile, AUI_PROJECT_INSPECTION_SCHEMA_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn inspect_previews_v2_to_v3_without_writing_and_preserves_stable_identity() {
    let fixture = Fixture::new("migration-preview");
    let document = legacy_typed_document();
    fixture.write_aui("menu.aui.json", &document);
    let before = fs::read(fixture.root.join("AUI/menu.aui.json")).unwrap();
    let (mut session, paths) = fixture.session_and_paths();
    let lease = session.acquire_snapshot_lease("m5-inspect", paths).unwrap();

    let report = inspect_aui_project_snapshot(lease.snapshot());

    assert!(report.ok(), "{:#?}", report.diagnostics);
    assert_eq!(report.schema_version, AUI_PROJECT_INSPECTION_SCHEMA_VERSION);
    assert_eq!(report.document_count, 1);
    assert_eq!(report.migration_candidate_count, 1);
    let inspected = &report.documents[0];
    assert_eq!(inspected.source_path, "AUI/menu.aui.json");
    assert_eq!(inspected.source_schema_version, "aui-document.v2");
    assert_eq!(inspected.cooked_schema_version, "aui-document.v2");
    assert_eq!(inspected.migration.target_schema_version, "aui-document.v3");
    assert!(inspected.migration.rewrite_required);
    assert!(inspected.migration.document_id_preserved);
    assert!(inspected.migration.stable_node_ids_preserved);
    assert!(inspected.migration.binding_ids_preserved);
    assert!(inspected.migration.action_ids_preserved);
    assert!(inspected.capabilities.contains(&"binding".to_string()));
    assert!(inspected.capabilities.contains(&"action".to_string()));
    assert!(report.trace.is_empty());
    let traced = inspect_aui_project_snapshot_with_request(
        lease.snapshot(),
        &AuiInspectionRequest {
            mode: AuiInspectionMode::Trace,
            document_id: Some("menu".to_string()),
            node_id: Some("play".to_string()),
        },
    );
    assert_eq!(traced.trace.len(), 1);
    assert_eq!(traced.trace[0].ancestor_path, vec!["root", "play"]);
    assert!(traced.trace[0].final_rect.is_some());
    assert_eq!(traced.trace[0].style_source, "engine_default");
    assert_eq!(
        before,
        fs::read(fixture.root.join("AUI/menu.aui.json")).unwrap()
    );
}

#[test]
fn compiler_check_includes_shared_aui_inspection() {
    let fixture = Fixture::new("compiler-check");
    fixture.write_aui("menu.aui.json", &legacy_typed_document());
    let (mut session, paths) = fixture.session_and_paths();
    let lease = session.acquire_snapshot_lease("m5-check", paths).unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();

    let report = compiler
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev).with_process_approval(false),
        )
        .unwrap();

    assert!(report.aui_inspection.ok());
    assert_eq!(report.aui_inspection.document_count, 1);
    assert!(report
        .checked_scope
        .contains(&"aui_schema_capabilities_and_migration".to_string()));
    assert_eq!(report.rust_check, "not_applicable");
}

#[test]
fn inspect_and_check_reject_invalid_control_contract_with_source_path() {
    let fixture = Fixture::new("invalid-slider");
    let mut document = legacy_typed_document();
    document.schema_version = "aui-document.v3".to_string();
    let mut slider = AuiNode::new(
        "volume",
        AuiNodeKind::Slider,
        AuiRect::fixed_position(10.0, 80.0, 240.0, 32.0),
    )
    .with_parent("root");
    slider.slider_value = Some(4.0);
    slider.slider_min = Some(10.0);
    slider.slider_max = Some(1.0);
    slider.slider_step = Some(0.0);
    document.nodes[0].children.push("volume".to_string());
    document.nodes.push(slider);
    fixture.write_aui("invalid.aui.json", &document);
    let (mut session, paths) = fixture.session_and_paths();
    let lease = session.acquire_snapshot_lease("m5-invalid", paths).unwrap();

    let inspection = inspect_aui_project_snapshot(lease.snapshot());
    assert!(!inspection.ok());
    assert!(inspection.diagnostics.iter().any(|item| {
        item.source_path == "AUI/invalid.aui.json"
            && item.domain == "aui"
            && item.severity == "error"
            && !item.next_action.is_empty()
    }));

    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let failure = compiler
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev).with_process_approval(false),
        )
        .unwrap_err();
    assert_eq!(failure.code(), "game_project_compiler.aui_invalid");
    assert_eq!(
        failure.source_location().unwrap().source_path,
        "AUI/invalid.aui.json"
    );
}

fn legacy_typed_document() -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.children = vec!["label".to_string(), "play".to_string()];
    let mut label = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(20.0, 20.0, 300.0, 48.0),
    )
    .with_parent("root");
    label.text = Some("Play".to_string());
    label.binding_refs.push(AuiBindingRef::new(
        "menu.title",
        AuiBindingTarget::TextText,
        "menu.title",
        None,
    ));
    let mut play = AuiNode::new(
        "play",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 90.0, 180.0, 52.0),
    )
    .with_parent("root")
    .with_interactable(true);
    play.action_refs.push(AuiActionRef::click("menu.play"));
    let mut document = AuiDocument::new(
        "menu",
        vec![AuiCanvas::screen_overlay(
            "menu.canvas",
            1280.0,
            720.0,
            "root",
        )],
        vec![root, label, play],
    );
    document.schema_version = "aui-document.v2".to_string();
    document
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aife-aui-tooling-m5-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("AUI")).unwrap();
        fs::write(
            root.join("project.aife.json"),
            br#"{"schemaVersion":"aife-project.v2","projectId":"aui.tooling.fixture"}"#,
        )
        .unwrap();
        Self { root }
    }

    fn write_aui(&self, name: &str, document: &AuiDocument) {
        fs::write(
            self.root.join("AUI").join(name),
            serde_json::to_vec_pretty(document).unwrap(),
        )
        .unwrap();
    }

    fn session_and_paths(&self) -> (ProjectAuthoringSession, Vec<String>) {
        let session = ProjectAuthoringSession::open(&self.root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect();
        (session, paths)
    }
}

impl AsRef<Path> for Fixture {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
