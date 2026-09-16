use editor_core::{ConsistencyReportLevel, SaveReloadRebuildStatus};
use project_e2e_gate::{
    run_process_isolated_authoring, run_save_reload_rebuild_consistency,
    SaveReloadRebuildConsistencyRequest,
};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn process_isolated_authoring() {
    let request = request("process-isolated");
    let report = run_process_isolated_authoring(request);
    assert_eq!(
        report.status,
        SaveReloadRebuildStatus::Passed,
        "{report:#?}"
    );
    assert_eq!(report.processes.len(), 2);
    assert_ne!(
        report.processes[0].invocation_id,
        report.processes[1].invocation_id
    );
    assert_eq!(report.reopen_mode, "process_isolated");
    assert!(report
        .comparisons
        .iter()
        .filter(|comparison| comparison.comparison_id.starts_with("saved_reopened_"))
        .all(|comparison| comparison.equal));
}

#[test]
fn save_reload_rebuild_consistency() {
    let request = request("full-consistency");
    let report = run_save_reload_rebuild_consistency(request);
    assert_eq!(
        report.status,
        SaveReloadRebuildStatus::Passed,
        "{report:#?}"
    );
    for comparison in [
        "build_recipe",
        "assembly_input",
        "runtime_content_hash",
        "payload_tree_digest",
        "runtime_file_inventory",
    ] {
        assert!(report
            .comparisons
            .iter()
            .any(|entry| entry.comparison_id == comparison && entry.equal));
    }
    assert!(report.source_runtime_witnesses.len() >= 8);
    assert!(report
        .source_runtime_witnesses
        .iter()
        .all(|witness| witness.field_path.is_some()));
    assert!(report.source_runtime_witnesses.iter().any(|witness| {
        witness.domain == "asset"
            && witness.source_path.starts_with("Assets/")
            && witness.runtime_path.contains("/textures/")
    }));
    assert!(report.mutations.iter().all(|mutation| mutation.observed));
}

#[test]
fn run_mode_writes_passing_report_and_returns_success() {
    let request = request("cli-run");
    let output = Command::new(&request.child_executable)
        .arg("run")
        .arg("--project")
        .arg(&request.source_project)
        .arg("--temp-root")
        .arg(&request.temp_root)
        .arg("--report")
        .arg(&request.report_path)
        .arg("--report-level")
        .arg("trace")
        .output()
        .expect("run mode should launch");
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = editor_core::read_consistency_report(&request.report_path).unwrap();
    assert_eq!(report.status, SaveReloadRebuildStatus::Passed);
    assert_eq!(report.reopen_mode, "process_isolated");
}

fn request(label: &str) -> SaveReloadRebuildConsistencyRequest {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!(
        "aife-save-reload-rebuild-{label}-{}-{stamp}",
        std::process::id()
    ));
    let source_project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("samples")
        .join("complex_shooter_project");
    let report_path = temp_root.join("reports").join("latest.json");
    let mut request = SaveReloadRebuildConsistencyRequest::new(
        source_project,
        &temp_root,
        report_path,
        env!("CARGO_BIN_EXE_project_e2e_gate"),
    );
    request.report_level = ConsistencyReportLevel::Trace;
    request
}
