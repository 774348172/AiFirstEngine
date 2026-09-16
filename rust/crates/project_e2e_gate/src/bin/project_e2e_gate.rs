use editor_core::{ConsistencyReportLevel, SaveReloadRebuildStatus};
use project_e2e_gate::{
    run_author_save_child, run_c01_from_blank_creation_gate, run_c01_golden_gate,
    run_reopen_read_child, run_save_reload_rebuild_consistency, validate_existing_c01_project,
    C01FromBlankCreationRequest, C01FromBlankCreationStatus, C01GoldenGateRequest,
    C01GoldenGateStatus, SaveReloadRebuildConsistencyRequest,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or_else(|| {
        "mode is required: run, author-save-child, or reopen-read-child".to_string()
    })?;
    let mut flags = BTreeMap::new();
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        if !flag.starts_with("--") {
            return Err(format!("unsupported argument {flag}"));
        }
        flags.insert(flag, value);
    }
    match mode.as_str() {
        "run" => run_parent(flags),
        "c01-golden-gate" => run_c01(flags),
        "c01-validation-only" => run_c01_validation_only(flags),
        "c01-from-blank-creation" => run_c01_from_blank_creation(flags),
        "author-save-child" => run_child_mode(flags, run_author_save_child),
        "reopen-read-child" => run_child_mode(flags, run_reopen_read_child),
        other => Err(format!("unsupported child mode {other}")),
    }
}

fn run_c01_from_blank_creation(mut flags: BTreeMap<String, String>) -> Result<(), String> {
    let project = required_path(&mut flags, "--project")?;
    let project_name = required(&mut flags, "--project-name")?;
    let engine_sdk = required_path(&mut flags, "--engine-sdk")?;
    let candidate_store = required_path(&mut flags, "--candidate-store")?;
    let evidence = required_path(&mut flags, "--evidence")?;
    let frozen_assets = required_path(&mut flags, "--frozen-assets")?;
    let external_export = required_path(&mut flags, "--external-export")?;
    let approved_by = required(&mut flags, "--approved-by")?;
    let prior_attempt_report = flags.remove("--prior-attempt-report").map(PathBuf::from);
    reject_unused_flags(flags)?;
    let mut request = C01FromBlankCreationRequest::new(
        project,
        project_name,
        engine_sdk,
        candidate_store,
        evidence,
        frozen_assets,
        external_export,
        approved_by,
    );
    if let Some(path) = prior_attempt_report {
        request = request.with_prior_attempt_report(path);
    }
    let report = run_c01_from_blank_creation_gate(request);
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to serialize from-blank report: {error}"))?
    );
    if report.status == C01FromBlankCreationStatus::Passed {
        Ok(())
    } else {
        Err(format!(
            "C-01 from-blank creation Gate failed: {}",
            report.first_blocker.as_deref().unwrap_or("unknown blocker")
        ))
    }
}

fn run_c01_validation_only(flags: BTreeMap<String, String>) -> Result<(), String> {
    run_c01_entry(flags, validate_existing_c01_project)
}

fn run_c01(flags: BTreeMap<String, String>) -> Result<(), String> {
    run_c01_entry(flags, run_c01_golden_gate)
}

fn run_c01_entry(
    mut flags: BTreeMap<String, String>,
    entry: fn(C01GoldenGateRequest) -> project_e2e_gate::C01GoldenGateReport,
) -> Result<(), String> {
    let project = required_path(&mut flags, "--project")?;
    let engine_sdk = required_path(&mut flags, "--engine-sdk")?;
    let candidate_store = required_path(&mut flags, "--candidate-store")?;
    let evidence = required_path(&mut flags, "--evidence")?;
    let frozen_assets = required_path(&mut flags, "--frozen-assets")?;
    let external_export = required_path(&mut flags, "--external-export")?;
    reject_unused_flags(flags)?;
    let report = entry(C01GoldenGateRequest::new(
        project,
        engine_sdk,
        candidate_store,
        evidence,
        frozen_assets,
        external_export,
    ));
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to serialize C-01 report: {error}"))?
    );
    if report.status == C01GoldenGateStatus::Passed {
        Ok(())
    } else {
        Err(format!(
            "C-01 Golden Gate failed: {}",
            report.first_blocker.as_deref().unwrap_or("unknown blocker")
        ))
    }
}

fn run_parent(mut flags: BTreeMap<String, String>) -> Result<(), String> {
    let project = required_path(&mut flags, "--project")?;
    let temp_root = required_path(&mut flags, "--temp-root")?;
    let report_path = required_path(&mut flags, "--report")?;
    let report_level = match flags
        .remove("--report-level")
        .unwrap_or_else(|| "trace".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "off" => ConsistencyReportLevel::Off,
        "summary" => ConsistencyReportLevel::Summary,
        "trace" => ConsistencyReportLevel::Trace,
        value => return Err(format!("unsupported --report-level {value}")),
    };
    let child_executable = flags
        .remove("--child-executable")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)
        .map_err(|error| format!("failed to resolve child executable: {error}"))?;
    reject_unused_flags(flags)?;
    let mut request =
        SaveReloadRebuildConsistencyRequest::new(project, temp_root, report_path, child_executable);
    request.report_level = report_level;
    let report = run_save_reload_rebuild_consistency(request);
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to serialize final report: {error}"))?
    );
    if report.status == SaveReloadRebuildStatus::Passed {
        Ok(())
    } else {
        Err("save/reload/rebuild consistency gate failed".to_string())
    }
}

type ChildMode =
    fn(&std::path::Path, &std::path::Path, &std::path::Path, &str, &str) -> Result<(), String>;

fn run_child_mode(
    mut flags: BTreeMap<String, String>,
    child_mode: ChildMode,
) -> Result<(), String> {
    let project = required_path(&mut flags, "--project")?;
    let temp_root = required_path(&mut flags, "--temp-root")?;
    let checkpoint = required_path(&mut flags, "--checkpoint")?;
    let invocation_id = required(&mut flags, "--invocation-id")?;
    let parent_token = required(&mut flags, "--parent-token")?;
    reject_unused_flags(flags)?;
    child_mode(
        &project,
        &temp_root,
        &checkpoint,
        &invocation_id,
        &parent_token,
    )
}

fn required(flags: &mut BTreeMap<String, String>, name: &str) -> Result<String, String> {
    flags
        .remove(name)
        .ok_or_else(|| format!("{name} is required"))
}

fn required_path(flags: &mut BTreeMap<String, String>, name: &str) -> Result<PathBuf, String> {
    required(flags, name).map(PathBuf::from)
}

fn reject_unused_flags(flags: BTreeMap<String, String>) -> Result<(), String> {
    if flags.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "unsupported arguments: {}",
            flags.keys().cloned().collect::<Vec<_>>().join(", ")
        ))
    }
}
