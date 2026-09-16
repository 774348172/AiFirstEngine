use editor_core::{
    command_for_test, summarize_patch_history, AssetPatchOperation, AuiPatchOperation,
    BuildPatchOperation, CommandStatus, PatchOperation, PatchReviewModel, PatchSource,
    PatchValidator, PrefabPatchOperation, ProjectPatchDocument,
    ProjectPatchImportProductizationReport, ProjectPatchImportRequest, ProjectPatchImportService,
    ProjectPatchProductizationReport, RulePatchOperation, ThinLlmPatchSource,
};
use editor_ui_model::UiCommandPayload;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_PROJECT_PATCH_SCENARIO_ID: &str =
    "complex-shooter-project-patch-productization-v1";
pub const COMPLEX_SHOOTER_IMPORTED_PROJECT_PATCH_SCENARIO_ID: &str =
    "complex-shooter-imported-project-patch-productization-v2";
pub const COMPLEX_SHOOTER_LLM_PATCH_SOURCE_SCENARIO_ID: &str =
    "complex-shooter-llm-patch-source-productization-v3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterProjectPatchRequest {
    pub project_path: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterProjectPatchRequest {
    pub fn new(project_path: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_project_patch_smoke(
    request: ComplexShooterProjectPatchRequest,
) -> ProjectPatchProductizationReport {
    let mut session = crate::complex_shooter_editor_session();
    let open_project = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_path.display().to_string(),
    }));
    let scene_path = request.project_path.join("Scenes").join("Main.scene.json");
    let open_scene =
        session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
            path: scene_path.display().to_string(),
        }));

    let patch = ProjectPatchDocument::new(
        "complex-shooter-all-domain-project-patch-smoke",
        "All-domain ProjectPatch smoke",
        PatchSource::Test,
        all_domain_smoke_operations(),
    );
    let validation = PatchValidator::validate(&session, &patch);
    let review = PatchReviewModel::from_patch(&patch, validation.clone());
    let apply_report = (open_project.status == CommandStatus::Committed
        && open_scene.status == CommandStatus::Committed
        && validation.accepted)
        .then(|| session.execute_patch_as_transaction(patch.clone()));
    let history_summary = summarize_patch_history(&session.patch_history().entries);
    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-all-domain-project-patch-report.json");
    let mut report = ProjectPatchProductizationReport::from_parts(
        COMPLEX_SHOOTER_PROJECT_PATCH_SCENARIO_ID,
        &patch,
        validation,
        review,
        apply_report,
        history_summary,
        vec![artifact_path.display().to_string()],
    );

    if open_project.status != CommandStatus::Committed {
        report
            .next_actions
            .push("fix_sample_project_open_for_project_patch_smoke".to_string());
    }
    if open_scene.status != CommandStatus::Committed {
        report
            .next_actions
            .push("fix_sample_scene_open_for_project_patch_smoke".to_string());
    }
    if let Err(error) = write_json(&artifact_path, &report) {
        report
            .next_actions
            .push(format!("fix_project_patch_report_write_failed:{error}"));
    }

    report
}

pub fn run_complex_shooter_imported_project_patch_smoke(
    request: ComplexShooterProjectPatchRequest,
) -> ProjectPatchImportProductizationReport {
    let mut session = crate::complex_shooter_editor_session();
    let open_project = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_path.display().to_string(),
    }));
    let scene_path = request.project_path.join("Scenes").join("Main.scene.json");
    let open_scene =
        session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
            path: scene_path.display().to_string(),
        }));

    let patch = ProjectPatchDocument::new(
        "complex-shooter-imported-all-domain-project-patch-smoke",
        "Imported all-domain ProjectPatch smoke",
        PatchSource::ImportedPatch,
        all_domain_smoke_operations(),
    );
    let fixture_path = request
        .output_root
        .join("fixtures")
        .join("imported-project-patch-smoke.json");
    let fixture_artifact = fixture_path.display().to_string();
    let mut fixture_write_error = None;
    if let Err(error) = write_json(&fixture_path, &patch) {
        fixture_write_error = Some(error.to_string());
    }
    let import_request = ProjectPatchImportRequest::file_path(
        "complex-shooter-imported-patch-fixture",
        fixture_path.display().to_string(),
    )
    .with_expected_patch_id("complex-shooter-imported-all-domain-project-patch-smoke");
    let import_result = ProjectPatchImportService::from_file(&session, import_request);
    let apply_report = (open_project.status == CommandStatus::Committed
        && open_scene.status == CommandStatus::Committed)
        .then(|| import_result.parsed_patch.as_ref().cloned())
        .flatten()
        .filter(|_| {
            import_result
                .validation
                .as_ref()
                .is_some_and(|validation| validation.accepted)
        })
        .map(|patch| session.execute_patch_as_transaction(patch));
    let history_summary = summarize_patch_history(&session.patch_history().entries);
    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-imported-project-patch-productization-report.json");
    let mut report = ProjectPatchImportProductizationReport::from_parts(
        COMPLEX_SHOOTER_IMPORTED_PROJECT_PATCH_SCENARIO_ID,
        import_result,
        apply_report,
        history_summary,
        vec![
            artifact_path.display().to_string(),
            fixture_artifact.clone(),
        ],
    );

    if open_project.status != CommandStatus::Committed {
        report
            .next_actions
            .push("fix_sample_project_open_for_imported_project_patch_smoke".to_string());
    }
    if open_scene.status != CommandStatus::Committed {
        report
            .next_actions
            .push("fix_sample_scene_open_for_imported_project_patch_smoke".to_string());
    }
    if let Some(error) = fixture_write_error {
        report.next_actions.push(format!(
            "fix_imported_project_patch_fixture_write_failed:{error}"
        ));
    }
    if let Err(error) = write_json(&artifact_path, &report) {
        report.next_actions.push(format!(
            "fix_imported_project_patch_report_write_failed:{error}"
        ));
    }

    report
}

pub fn run_complex_shooter_llm_patch_source_smoke(
    request: ComplexShooterProjectPatchRequest,
) -> ProjectPatchImportProductizationReport {
    let mut session = crate::complex_shooter_editor_session();
    let open_project = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_path.display().to_string(),
    }));
    let scene_path = request.project_path.join("Scenes").join("Main.scene.json");
    let open_scene =
        session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
            path: scene_path.display().to_string(),
        }));

    let config = editor_core::LlmPatchSourceConfig::deterministic_mock();
    let source_result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "create all_domain ProjectPatch Smoke",
        "complex shooter e2e mock source",
    );
    let import_result = if let Some(raw_json) = source_result.raw_json {
        let import_request = ProjectPatchImportRequest::ai_structured_output(
            "complex-shooter-llm-patch-source-mock",
            raw_json,
        );
        ProjectPatchImportService::from_json_string(&session, import_request)
    } else {
        ProjectPatchImportService::from_json_string(
            &session,
            ProjectPatchImportRequest::ai_structured_output(
                "complex-shooter-llm-patch-source-mock",
                "{not-generated",
            ),
        )
    };
    let apply_report = (open_project.status == CommandStatus::Committed
        && open_scene.status == CommandStatus::Committed)
        .then(|| import_result.parsed_patch.as_ref().cloned())
        .flatten()
        .filter(|_| {
            import_result
                .validation
                .as_ref()
                .is_some_and(|validation| validation.accepted)
        })
        .map(|patch| session.execute_patch_as_transaction(patch));
    let history_summary = summarize_patch_history(&session.patch_history().entries);
    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-llm-patch-source-productization-report.json");
    let mut report = ProjectPatchImportProductizationReport::from_parts(
        COMPLEX_SHOOTER_LLM_PATCH_SOURCE_SCENARIO_ID,
        import_result,
        apply_report,
        history_summary,
        vec![artifact_path.display().to_string()],
    );

    if open_project.status != CommandStatus::Committed {
        report
            .next_actions
            .push("fix_sample_project_open_for_llm_patch_source_smoke".to_string());
    }
    if open_scene.status != CommandStatus::Committed {
        report
            .next_actions
            .push("fix_sample_scene_open_for_llm_patch_source_smoke".to_string());
    }
    if let Some(code) = source_result.error_code {
        report
            .next_actions
            .push(format!("fix_llm_patch_source_error:{code}"));
    }
    if let Err(error) = write_json(&artifact_path, &report) {
        report
            .next_actions
            .push(format!("fix_llm_patch_source_report_write_failed:{error}"));
    }

    report
}

fn all_domain_smoke_operations() -> Vec<PatchOperation> {
    vec![
        PatchOperation::Asset(AssetPatchOperation::GenerateMockImageAsset {
            operation_id: "op-generate-smoke-asset".to_string(),
            depends_on: Vec::new(),
            prompt: "complex shooter generic smoke sprite".to_string(),
            target_folder: "Assets/Generated".to_string(),
            asset_name: "project-patch-smoke-sprite".to_string(),
            image_kind: "sprite".to_string(),
            width: 16,
            height: 16,
            transparent_background: true,
        }),
        PatchOperation::Prefab(PrefabPatchOperation::ValidateReferences {
            operation_id: "op-validate-smoke-prefabs".to_string(),
            depends_on: Vec::new(),
            path: None,
        }),
        PatchOperation::Aui(AuiPatchOperation::CreateDocument {
            operation_id: "op-create-smoke-aui".to_string(),
            depends_on: Vec::new(),
            path: "UI/project-patch-smoke.aui.json".to_string(),
            document_id: "project-patch-smoke-hud".to_string(),
            width: 1280.0,
            height: 720.0,
        }),
        PatchOperation::Rule(RulePatchOperation::CreateAsset {
            operation_id: "op-create-smoke-rule".to_string(),
            depends_on: Vec::new(),
            path: "Rules/project-patch-smoke.rule.json".to_string(),
            rule_id: "project.rule.project_patch_smoke".to_string(),
            display_name: "ProjectPatch Smoke".to_string(),
            phase: None,
        }),
        PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
            operation_id: "op-export-smoke-build".to_string(),
            depends_on: vec!["op-create-smoke-rule".to_string()],
            profile_id: Some("windows-dev".to_string()),
        }),
    ]
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
