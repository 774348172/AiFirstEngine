use editor_core::{
    AiCapabilityToolKernel, AiToolCatalogRequest, TOOL_ID_EVIDENCE_READ,
    TOOL_ID_PROJECT_BUILD_EXPORT, TOOL_ID_PROJECT_CREATE, TOOL_ID_PROJECT_DELIVERY_VERIFY,
    TOOL_ID_PROJECT_DIAGNOSTICS, TOOL_ID_PROJECT_INSPECT, TOOL_ID_PROJECT_MUTATE,
    TOOL_ID_PROJECT_PREVIEW, TOOL_ID_PROJECT_READ_OBJECT, TOOL_ID_PROJECT_REFERENCES,
    TOOL_ID_PROJECT_ROLLBACK, TOOL_ID_PROJECT_SEARCH, TOOL_ID_PROJECT_SOURCE_SYMBOLS,
    TOOL_ID_PROJECT_TRACE_UI_OWNER, TOOL_ID_RUNTIME_CAPTURE_ISSUE, TOOL_ID_UI_EXPLAIN_VISIBILITY,
    TOOL_ID_UI_LOCATE,
};
use std::path::{Path, PathBuf};

#[test]
fn core_catalog_retains_ai_authoring_tools() {
    let catalog = AiCapabilityToolKernel::new()
        .catalog(AiToolCatalogRequest::default())
        .expect("read Core tool catalog");
    let actual = catalog
        .tools
        .iter()
        .map(|tool| tool.tool_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        [
            TOOL_ID_PROJECT_CREATE,
            TOOL_ID_PROJECT_INSPECT,
            TOOL_ID_PROJECT_MUTATE,
            TOOL_ID_PROJECT_ROLLBACK,
            TOOL_ID_PROJECT_PREVIEW,
            TOOL_ID_PROJECT_SEARCH,
            TOOL_ID_PROJECT_READ_OBJECT,
            TOOL_ID_PROJECT_REFERENCES,
            TOOL_ID_PROJECT_SOURCE_SYMBOLS,
            TOOL_ID_PROJECT_DIAGNOSTICS,
            TOOL_ID_EVIDENCE_READ,
            TOOL_ID_RUNTIME_CAPTURE_ISSUE,
            TOOL_ID_UI_LOCATE,
            TOOL_ID_UI_EXPLAIN_VISIBILITY,
            TOOL_ID_PROJECT_TRACE_UI_OWNER,
            TOOL_ID_PROJECT_BUILD_EXPORT,
            TOOL_ID_PROJECT_DELIVERY_VERIFY,
        ]
    );
}

#[test]
fn active_surface_excludes_retired_production_lifecycle() {
    let gateway_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rust_crates = gateway_root.parent().expect("rust crates root");

    assert_absent(
        &gateway_root.join("src/lib.rs"),
        &[
            "production_candidate",
            "codex_outcome_evaluation",
            "codex_outcome_runtime",
            "real_session_observer",
            "activation_lease",
            "candidate_freezer",
            "gate_f_acceptance",
        ],
    );
    assert_absent(
        &gateway_root.join("Cargo.toml"),
        &["legacy-r1", "candidate_freezer", "gate_f_acceptance"],
    );
    assert_absent(
        &gateway_root.join("src/core.rs"),
        &[
            "GatewayAttemptObservation",
            "attempt_journals",
            "session_attempt_claims",
            "gateway.observation.attempt_terminal",
        ],
    );
    assert_absent(
        &gateway_root.join("src/editor_host.rs"),
        &[
            "production_evaluation_module",
            "CodexOutcomeEvaluationModule",
        ],
    );
    assert_absent(
        &rust_crates.join("editor_host/src/main.rs"),
        &[
            "--production-candidate-request",
            "--production-candidate-result",
            "--candidate-freeze-preflight",
        ],
    );
    assert_absent(
        &rust_crates.join("editor_window_winit/src/application.rs"),
        &[
            "production_evaluation_module",
            "prepare_candidate_freeze_direct_input",
            "create_candidate_freeze_direct_input",
        ],
    );
    assert_absent(
        &rust_crates.join("editor_window_winit/src/real_window.rs"),
        &["capture_game_view_plan_exact_shared_texture"],
    );
}

#[test]
fn retired_source_archive_is_read_only_and_has_no_active_dependency() {
    let gateway_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rust_root = gateway_root
        .parent()
        .and_then(Path::parent)
        .expect("Rust workspace root");
    let workspace_root = rust_root.parent().expect("repository root");
    let archive = workspace_root
        .join("legacy")
        .join("rust")
        .join("254-r2-lifecycle");
    assert!(archive.join("README.md").is_file());

    for path in files_below(&archive) {
        let name = path.file_name().and_then(|name| name.to_str());
        assert!(
            !matches!(name, Some("Cargo.toml" | "build.rs")),
            "retired source archive must not be buildable: {}",
            path.display()
        );
    }

    let archive_reference = ["legacy", "rust", "254-r2-lifecycle"].join("/");
    for path in files_below(&rust_root.join("crates")) {
        let is_active_source = path.extension().and_then(|extension| extension.to_str())
            == Some("rs")
            || path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml");
        if !is_active_source {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            !source.replace('\\', "/").contains(&archive_reference),
            "active source depends on retired archive: {}",
            path.display()
        );
    }
}

fn assert_absent(path: &Path, retired_tokens: &[&str]) {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    for token in retired_tokens {
        assert!(
            !source.contains(token),
            "retired token {token:?} remains active in {}",
            path.display()
        );
    }
}

fn files_below(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files
}
