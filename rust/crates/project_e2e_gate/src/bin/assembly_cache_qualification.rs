use editor_core::{
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyReport,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyStatus,
};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssemblyCacheQualificationReport {
    schema_version: &'static str,
    project_root: String,
    cache_root: String,
    first_duration_ms: u64,
    second_duration_ms: u64,
    first: ProjectRuntimePackageAssemblyReport,
    second: ProjectRuntimePackageAssemblyReport,
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let project_root = PathBuf::from(args.next().ok_or("project root is required")?);
    let cache_root = PathBuf::from(args.next().ok_or("cache root is required")?);
    let report_path = PathBuf::from(args.next().ok_or("report path is required")?);
    let allow_existing = match args.next().as_deref() {
        None => false,
        Some("--allow-existing") => true,
        Some(value) => return Err(format!("unexpected extra argument {value}")),
    };
    if args.next().is_some() {
        return Err("unexpected extra argument".to_string());
    }
    if cache_root.exists() && !allow_existing {
        return Err(format!(
            "qualification cache root must be fresh: {}",
            cache_root.display()
        ));
    }

    let first_started = Instant::now();
    let first = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&project_root)
            .with_artifact_cache_root(&cache_root),
    );
    let first_duration_ms = elapsed_ms(first_started);
    if first.status != ProjectRuntimePackageAssemblyStatus::Success {
        return Err(format!(
            "first assembly failed: {:?}",
            first.report.diagnostics
        ));
    }

    let second_started = Instant::now();
    let second = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&project_root)
            .with_artifact_cache_root(&cache_root),
    );
    let second_duration_ms = elapsed_ms(second_started);
    if second.status != ProjectRuntimePackageAssemblyStatus::Success {
        return Err(format!(
            "second assembly failed: {:?}",
            second.report.diagnostics
        ));
    }

    let report = AssemblyCacheQualificationReport {
        schema_version: "assembly-cache-qualification-report.v1",
        project_root: project_root.display().to_string(),
        cache_root: cache_root.display().to_string(),
        first_duration_ms,
        second_duration_ms,
        first: first.report,
        second: second.report,
    };
    let parent = report_path
        .parent()
        .ok_or_else(|| "report path has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create report directory: {error}"))?;
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("cannot encode report: {error}"))?,
    )
    .map_err(|error| format!("cannot write report: {error}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("cannot print report: {error}"))?
    );
    Ok(())
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}
