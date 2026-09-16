use editor_core::{
    command_for_test, CommandStatus, ProjectRuntimePackageAssembler,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyStatus,
};
use editor_ui_model::{InputMappingPreviewStatus, UiCommandPayload};
use engine_input::{InputMappingAsset, InputResolver, RuntimeInputEvent, RuntimeInputFrame};
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-input-mapping-visual-authoring-report.v1";
pub const COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_SCENARIO_ID: &str =
    "complex_shooter_input_mapping_visual_authoring";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexShooterInputMappingVisualAuthoringStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterInputMappingVisualAuthoringMetrics {
    pub context_count: usize,
    pub action_count: usize,
    pub binding_count: usize,
    pub stable_binding_id_count: usize,
    pub preview_action_count: usize,
    pub runtime_action_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterInputMappingVisualAuthoringReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterInputMappingVisualAuthoringStatus,
    pub project_root: String,
    pub workspace_root: String,
    pub output_root: String,
    pub runtime_package_dir: Option<String>,
    pub source_schema_version: Option<String>,
    pub source_hash_before: Option<String>,
    pub source_hash_after: Option<String>,
    pub unsaved_draft_excluded_from_package: bool,
    pub saved_mapping_included_in_package: bool,
    pub preview_resolved_pause: bool,
    pub runtime_resolved_pause: bool,
    pub changed_paths: Vec<String>,
    pub metrics: ComplexShooterInputMappingVisualAuthoringMetrics,
    pub diagnostics: Vec<String>,
    pub artifacts: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterInputMappingVisualAuthoringRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterInputMappingVisualAuthoringRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_input_mapping_visual_authoring_report(
    request: ComplexShooterInputMappingVisualAuthoringRequest,
) -> ComplexShooterInputMappingVisualAuthoringReport {
    let workspace_root = request.output_root.join("workspace");
    let package_dir = request.output_root.join("runtime_package");
    let mut report = ComplexShooterInputMappingVisualAuthoringReport {
        schema_version: COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION
            .to_string(),
        scenario_id: COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_SCENARIO_ID.to_string(),
        status: ComplexShooterInputMappingVisualAuthoringStatus::Failed,
        project_root: request.project_root.display().to_string(),
        workspace_root: workspace_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        runtime_package_dir: None,
        source_schema_version: None,
        source_hash_before: None,
        source_hash_after: None,
        unsaved_draft_excluded_from_package: false,
        saved_mapping_included_in_package: false,
        preview_resolved_pause: false,
        runtime_resolved_pause: false,
        changed_paths: Vec::new(),
        metrics: ComplexShooterInputMappingVisualAuthoringMetrics::default(),
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
        next_actions: Vec::new(),
    };
    if let Err(error) = fs::create_dir_all(&request.output_root)
        .and_then(|_| copy_project(&request.project_root, &workspace_root))
    {
        report
            .diagnostics
            .push(format!("workspace_copy_failed:{error}"));
        return finalize_report(&request.output_root, report);
    }

    let mapping_path = "Input/input.default.json";
    let mut session = crate::complex_shooter_editor_session();
    if session
        .execute_command(command_for_test(UiCommandPayload::OpenProject {
            path: workspace_root.display().to_string(),
        }))
        .status
        != CommandStatus::Committed
    {
        report.diagnostics.push("open_project_failed".to_string());
        return finalize_report(&request.output_root, report);
    }
    if session
        .execute_command(command_for_test(UiCommandPayload::OpenInputMapping {
            path: mapping_path.to_string(),
        }))
        .status
        != CommandStatus::Committed
    {
        report
            .diagnostics
            .push("open_input_mapping_failed".to_string());
        return finalize_report(&request.output_root, report);
    }

    let opened = session.build_input_mapping_authoring_model();
    report.source_schema_version = Some(engine_input::INPUT_MAPPING_SCHEMA_VERSION.to_string());
    report.source_hash_before = opened.source_hash.clone();
    report.metrics.context_count = opened.contexts.len();
    report.metrics.action_count = opened.actions.len();
    report.metrics.binding_count = opened.bindings.len();
    report.metrics.stable_binding_id_count = opened
        .bindings
        .iter()
        .filter(|binding| !binding.binding_id.is_empty())
        .count();
    let Some(pause_binding_id) = opened
        .bindings
        .iter()
        .find(|binding| binding.action_id == "action.pause")
        .map(|binding| binding.binding_id.clone())
    else {
        report
            .diagnostics
            .push("action_pause_binding_missing".to_string());
        return finalize_report(&request.output_root, report);
    };

    let edit = session.execute_command(command_for_test(
        UiCommandPayload::SetInputBindingDevicePathById {
            path: mapping_path.to_string(),
            binding_id: pause_binding_id,
            device_path: "keyboard/P".to_string(),
        },
    ));
    if edit.status != CommandStatus::Committed {
        report.diagnostics.push("draft_edit_failed".to_string());
        return finalize_report(&request.output_root, report);
    }
    report.changed_paths = edit
        .state_changes
        .iter()
        .map(|change| change.path.clone())
        .collect();

    let preview =
        session.execute_command(command_for_test(UiCommandPayload::PreviewInputMapping {
            path: mapping_path.to_string(),
            device_path: Some("keyboard/P".to_string()),
        }));
    if preview.status == CommandStatus::Committed {
        let model = session.build_input_mapping_authoring_model();
        report.preview_resolved_pause = model.preview.as_ref().is_some_and(|preview| {
            preview.status == InputMappingPreviewStatus::Resolved
                && preview
                    .actions
                    .iter()
                    .any(|action| action.action_id == "action.pause")
        });
        report.metrics.preview_action_count = model
            .preview
            .as_ref()
            .map(|preview| preview.actions.len())
            .unwrap_or_default();
    }

    let unsaved_assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&workspace_root),
    );
    report.unsaved_draft_excluded_from_package = unsaved_assembly
        .build_input
        .as_ref()
        .and_then(|input| assembled_mapping(input))
        .is_some_and(|mapping| pause_device_path(&mapping) == Some("keyboard/Escape"));

    let save = session.execute_command(command_for_test(UiCommandPayload::SaveInputMapping {
        path: mapping_path.to_string(),
    }));
    if save.status != CommandStatus::Committed {
        report.diagnostics.push("save_mapping_failed".to_string());
        return finalize_report(&request.output_root, report);
    }
    report.source_hash_after = session
        .build_input_mapping_authoring_model()
        .source_hash
        .clone();

    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&workspace_root),
    );
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        report
            .diagnostics
            .push("saved_project_assembly_failed".to_string());
        return finalize_report(&request.output_root, report);
    }
    report.saved_mapping_included_in_package = assembly
        .build_input
        .as_ref()
        .and_then(|input| assembled_mapping(input))
        .is_some_and(|mapping| pause_device_path(&mapping) == Some("keyboard/P"));
    let input = assembly
        .build_input
        .expect("successful assembly must contain build input");
    let active_scene_id = assembly
        .active_scene_id
        .unwrap_or_else(|| "scene-main".to_string());
    let build = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&package_dir, active_scene_id),
        &input,
    );
    if build.status != RuntimePackageBuildStatus::Success {
        report
            .diagnostics
            .push("runtime_package_build_failed".to_string());
        return finalize_report(&request.output_root, report);
    }
    report.runtime_package_dir = Some(package_dir.display().to_string());
    report.artifacts.push(package_dir.display().to_string());

    let load = load_runtime_package(&package_dir);
    let Some(package) = load.value else {
        report
            .diagnostics
            .push("runtime_package_load_failed".to_string());
        return finalize_report(&request.output_root, report);
    };
    let Some(runtime_mapping) = package.default_input_mapping.as_ref() else {
        report
            .diagnostics
            .push("runtime_default_mapping_missing".to_string());
        return finalize_report(&request.output_root, report);
    };
    let mut frame = RuntimeInputFrame::new(1, "game-view");
    frame.events.push(RuntimeInputEvent::KeyDown {
        key: "P".to_string(),
    });
    let resolved = InputResolver::resolve(&frame, runtime_mapping);
    report.metrics.runtime_action_count = resolved.action_snapshot.actions.len();
    report.runtime_resolved_pause = resolved.action_snapshot.button_pressed("action.pause");

    if !report.unsaved_draft_excluded_from_package {
        report
            .diagnostics
            .push("unsaved_draft_leaked_into_assembly".to_string());
    }
    if !report.saved_mapping_included_in_package {
        report
            .diagnostics
            .push("saved_mapping_missing_from_assembly".to_string());
    }
    if !report.preview_resolved_pause {
        report
            .diagnostics
            .push("preview_did_not_resolve_pause".to_string());
    }
    if !report.runtime_resolved_pause {
        report
            .diagnostics
            .push("runtime_did_not_resolve_pause".to_string());
    }
    if report.metrics.stable_binding_id_count != report.metrics.binding_count {
        report
            .diagnostics
            .push("stable_binding_identity_incomplete".to_string());
    }
    if report.diagnostics.is_empty() {
        report.status = ComplexShooterInputMappingVisualAuthoringStatus::Passed;
    } else {
        report
            .next_actions
            .push("inspect_input_mapping_visual_authoring_report".to_string());
    }
    finalize_report(&request.output_root, report)
}

fn assembled_mapping(
    input: &engine_runtime::runtime_package_builder::RuntimePackageBuildInput,
) -> Option<InputMappingAsset> {
    input.input_mappings.iter().find_map(|mapping| {
        serde_json::from_value::<InputMappingAsset>(mapping.document.clone()).ok()
    })
}

fn pause_device_path(mapping: &InputMappingAsset) -> Option<&str> {
    mapping
        .bindings
        .iter()
        .find(|binding| binding.action_id == "action.pause")
        .map(|binding| binding.device_path.as_str())
}

fn copy_project(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "Build" || name == ".git" {
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
    mut report: ComplexShooterInputMappingVisualAuthoringReport,
) -> ComplexShooterInputMappingVisualAuthoringReport {
    let report_path = output_root
        .join("reports")
        .join("complex-shooter-input-mapping-visual-authoring-report.json");
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
