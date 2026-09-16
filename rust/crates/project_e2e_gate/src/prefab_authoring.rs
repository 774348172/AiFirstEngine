use editor_core::{
    command_for_test, CommandResult, CommandStatus, EditorSceneDocument, EditorSession,
    PrefabAsset, PrefabAuthoringReport, PrefabDiagnostic, PrefabInstance, PrefabWorkflowService,
};
use editor_ui_model::{PrefabStageSavePolicy, UiCommandPayload, Vec3};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const COMPLEX_SHOOTER_PREFAB_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-prefab-authoring-productization-report.v1";
pub const COMPLEX_SHOOTER_PREFAB_AUTHORING_SCENARIO_ID: &str =
    "complex-shooter-prefab-authoring-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterPrefabAuthoringStatus {
    Passed,
    Partial,
    Failed,
}

impl ComplexShooterPrefabAuthoringStatus {
    fn from_command_status(status: CommandStatus) -> Self {
        match status {
            CommandStatus::Committed => Self::Passed,
            CommandStatus::Rejected => Self::Partial,
            CommandStatus::Pending | CommandStatus::Validated | CommandStatus::Failed => {
                Self::Failed
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAuthoringCommandEvidence {
    pub command_id: String,
    pub status: ComplexShooterPrefabAuthoringStatus,
    pub diagnostic_codes: Vec<String>,
    pub state_change_kinds: Vec<String>,
}

impl PrefabAuthoringCommandEvidence {
    fn from_result(result: &CommandResult) -> Self {
        Self {
            command_id: result.command_id.clone(),
            status: ComplexShooterPrefabAuthoringStatus::from_command_status(result.status),
            diagnostic_codes: result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
            state_change_kinds: result
                .state_changes
                .iter()
                .map(|change| change.kind.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAuthoringAssetEvidence {
    pub source_path: String,
    pub prefab_id: Option<String>,
    pub root_entity_id: Option<String>,
    pub entity_count: usize,
    pub component_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAuthoringInstanceEvidence {
    pub scene_path: String,
    pub instance_id: String,
    pub prefab_ref: String,
    pub instance_entity_id: String,
    pub override_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterPrefabAuthoringMetrics {
    pub prefab_assets_count: usize,
    pub prefab_instances_count: usize,
    pub smoke_command_count: usize,
    pub smoke_committed_command_count: usize,
    pub smoke_prefab_assets_count: usize,
    pub smoke_prefab_instances_count: usize,
    pub smoke_applied_override_count: usize,
    pub smoke_reverted_override_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterPrefabAuthoringReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterPrefabAuthoringStatus,
    pub project_root: String,
    pub output_root: String,
    pub prefab_assets_count: usize,
    pub prefab_instances_count: usize,
    pub prefab_assets: Vec<PrefabAuthoringAssetEvidence>,
    pub prefab_instances: Vec<PrefabAuthoringInstanceEvidence>,
    pub smoke_project_root: Option<String>,
    pub smoke_commands: Vec<PrefabAuthoringCommandEvidence>,
    pub smoke_authoring_report: Option<PrefabAuthoringReport>,
    pub metrics: ComplexShooterPrefabAuthoringMetrics,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ComplexShooterPrefabAuthoringReport {
    fn new(project_root: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_SHOOTER_PREFAB_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: COMPLEX_SHOOTER_PREFAB_AUTHORING_SCENARIO_ID.to_string(),
            status: ComplexShooterPrefabAuthoringStatus::Failed,
            project_root: project_root.into(),
            output_root: output_root.into(),
            prefab_assets_count: 0,
            prefab_instances_count: 0,
            prefab_assets: Vec::new(),
            prefab_instances: Vec::new(),
            smoke_project_root: None,
            smoke_commands: Vec::new(),
            smoke_authoring_report: None,
            metrics: ComplexShooterPrefabAuthoringMetrics {
                prefab_assets_count: 0,
                prefab_instances_count: 0,
                smoke_command_count: 0,
                smoke_committed_command_count: 0,
                smoke_prefab_assets_count: 0,
                smoke_prefab_instances_count: 0,
                smoke_applied_override_count: 0,
                smoke_reverted_override_count: 0,
            },
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn recompute(&mut self) {
        self.prefab_assets_count = self.prefab_assets.len();
        self.prefab_instances_count = self.prefab_instances.len();
        self.metrics.prefab_assets_count = self.prefab_assets_count;
        self.metrics.prefab_instances_count = self.prefab_instances_count;
        self.metrics.smoke_command_count = self.smoke_commands.len();
        self.metrics.smoke_committed_command_count = self
            .smoke_commands
            .iter()
            .filter(|command| command.status == ComplexShooterPrefabAuthoringStatus::Passed)
            .count();
        if let Some(authoring) = &self.smoke_authoring_report {
            self.metrics.smoke_prefab_assets_count = authoring.prefab_assets_count;
            self.metrics.smoke_prefab_instances_count = authoring.prefab_instances_count;
            self.metrics.smoke_applied_override_count = authoring.applied_override_count;
            self.metrics.smoke_reverted_override_count = authoring.reverted_override_count;
        }

        if self.prefab_assets_count == 0 {
            self.next_actions
                .push("create_sample_prefab_assets".to_string());
        }
        if self.prefab_instances_count == 0 {
            self.next_actions.push(
                "sample_scene_has_no_engine_prefab_instance_yet_use_instantiate_prefab_in_scene"
                    .to_string(),
            );
        }
        if self.metrics.smoke_prefab_instances_count == 0 {
            self.next_actions
                .push("fix_prefab_instantiate_smoke".to_string());
        }
        if self.metrics.smoke_applied_override_count == 0 {
            self.next_actions
                .push("fix_prefab_apply_override_smoke".to_string());
        }
        self.next_actions.sort();
        self.next_actions.dedup();

        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("error:"))
            || self
                .smoke_commands
                .iter()
                .any(|command| command.status == ComplexShooterPrefabAuthoringStatus::Failed)
        {
            self.status = ComplexShooterPrefabAuthoringStatus::Failed;
        } else if self.prefab_instances_count == 0 || !self.next_actions.is_empty() {
            self.status = ComplexShooterPrefabAuthoringStatus::Partial;
        } else {
            self.status = ComplexShooterPrefabAuthoringStatus::Passed;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterPrefabAuthoringRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterPrefabAuthoringRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_prefab_authoring_report(
    request: ComplexShooterPrefabAuthoringRequest,
) -> ComplexShooterPrefabAuthoringReport {
    let mut report = ComplexShooterPrefabAuthoringReport::new(
        request.project_root.display().to_string(),
        request.output_root.display().to_string(),
    );

    collect_sample_project_prefab_assets(&request.project_root, &mut report);
    collect_sample_project_prefab_instances(&request.project_root, &mut report);
    run_prefab_authoring_smoke(&mut report);

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-prefab-authoring-productization-report.json");
    if write_json(&artifact_path, &report).is_ok() {
        report.artifacts.push(artifact_path.display().to_string());
    } else {
        report.diagnostics.push(format!(
            "error:prefab_authoring_report_write_failed:{}",
            artifact_path.display()
        ));
        report
            .next_actions
            .push("fix_prefab_authoring_report_output_path".to_string());
    }

    report.recompute();
    if write_json(&artifact_path, &report).is_err() {
        report.diagnostics.push(format!(
            "error:prefab_authoring_report_rewrite_failed:{}",
            artifact_path.display()
        ));
        report.recompute();
    }
    report
}

fn collect_sample_project_prefab_assets(
    project_root: &Path,
    report: &mut ComplexShooterPrefabAuthoringReport,
) {
    let prefab_root = project_root.join("Prefabs");
    let Ok(entries) = fs::read_dir(&prefab_root) else {
        report.diagnostics.push(format!(
            "error:prefab_directory_missing:{}",
            prefab_root.display()
        ));
        return;
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".prefab.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let relative_path = project_relative_path(project_root, &path);
        match PrefabWorkflowService::load_asset(project_root, &relative_path) {
            Ok(asset) => {
                report
                    .prefab_assets
                    .push(asset_evidence(&relative_path, &asset));
                for diagnostic in validate_asset(&relative_path, &asset) {
                    report.diagnostics.push(diagnostic);
                }
            }
            Err(message) => report.diagnostics.push(format!(
                "error:prefab_asset_load_failed:{relative_path}:{message}"
            )),
        }
    }
}

fn collect_sample_project_prefab_instances(
    project_root: &Path,
    report: &mut ComplexShooterPrefabAuthoringReport,
) {
    let scene_root = project_root.join("Scenes");
    let Ok(entries) = fs::read_dir(&scene_root) else {
        report.diagnostics.push(format!(
            "error:scene_directory_missing:{}",
            scene_root.display()
        ));
        return;
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".scene.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let relative_path = project_relative_path(project_root, &path);
        match EditorSceneDocument::load_from_path(&path) {
            Ok(scene) => {
                for entity in &scene.entities {
                    if let Ok(instance) = PrefabInstance::from_scene_entity(entity) {
                        report
                            .prefab_instances
                            .push(PrefabAuthoringInstanceEvidence {
                                scene_path: relative_path.clone(),
                                instance_id: instance.instance_id,
                                prefab_ref: instance.prefab_ref.id,
                                instance_entity_id: instance.instance_root_entity_id,
                                override_count: instance.overrides.len(),
                            });
                    }
                }
            }
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    report.diagnostics.push(format!(
                        "error:scene_load_failed:{}:{}",
                        relative_path, diagnostic.code
                    ));
                }
            }
        }
    }
}

fn run_prefab_authoring_smoke(report: &mut ComplexShooterPrefabAuthoringReport) {
    let smoke_root = std::env::temp_dir().join(format!(
        "project-e2e-gate-prefab-authoring-smoke-{}",
        unique_stamp()
    ));
    report.smoke_project_root = Some(smoke_root.display().to_string());
    let mut session = EditorSession::new();

    let commands = vec![
        UiCommandPayload::CreateProject {
            path: smoke_root.display().to_string(),
            name: "Prefab Authoring Smoke".to_string(),
        },
        UiCommandPayload::OpenProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
        UiCommandPayload::CreateSceneEntity {
            parent_id: None,
            name: "Smoke Source".to_string(),
        },
        UiCommandPayload::CreatePrefabFromSelection {
            scene_path: Some("Scenes/Main.scene.json".to_string()),
            root_entity_id: "entity-smoke-source".to_string(),
            prefab_id: "prefab-smoke-source".to_string(),
            name: "Smoke Source".to_string(),
            replace_selection_with_instance: true,
        },
        UiCommandPayload::OpenPrefabDocument {
            path: "Prefabs/prefab-smoke-source.prefab.json".to_string(),
        },
        UiCommandPayload::SetPrefabStageEntityField {
            source_entity_id: "entity-smoke-source".to_string(),
            component_type: Some("engine.transform".to_string()),
            field_path: "localPosition".to_string(),
            value: serde_json::json!({ "x": 1.0, "y": 2.0, "z": 0.0 }),
        },
        UiCommandPayload::SavePrefabDocument {
            path: "Prefabs/prefab-smoke-source.prefab.json".to_string(),
        },
        UiCommandPayload::ExitPrefabStage {
            save_policy: PrefabStageSavePolicy::Discard,
        },
        UiCommandPayload::InstantiatePrefabInScene {
            prefab_id: "prefab-smoke-source".to_string(),
            parent_entity_id: None,
            local_position: Some(Vec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            }),
        },
        UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-smoke-source".to_string(),
            component_type: "engine.transform".to_string(),
            field_path: "localPosition".to_string(),
            value: serde_json::json!({ "x": 3.0, "y": 0.0, "z": 0.0 }),
        },
        UiCommandPayload::RevertPrefabOverride {
            instance_entity_id: "entity-smoke-source".to_string(),
            target_source_entity_id: "entity-smoke-source".to_string(),
            component_type: "engine.transform".to_string(),
            field_path: "localPosition".to_string(),
        },
        UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-smoke-source".to_string(),
            component_type: "engine.transform".to_string(),
            field_path: "localPosition".to_string(),
            value: serde_json::json!({ "x": 4.0, "y": 0.0, "z": 0.0 }),
        },
        UiCommandPayload::ApplyPrefabOverrideToAsset {
            instance_entity_id: "entity-smoke-source".to_string(),
            target_source_entity_id: "entity-smoke-source".to_string(),
            component_type: "engine.transform".to_string(),
            field_path: "localPosition".to_string(),
        },
        UiCommandPayload::ValidatePrefabReferences { path: None },
    ];

    for payload in commands {
        let result = session.execute_command(command_for_test(payload));
        report
            .smoke_commands
            .push(PrefabAuthoringCommandEvidence::from_result(&result));
    }
    report.smoke_authoring_report = Some(session.prefab_authoring_report().clone());
}

fn asset_evidence(relative_path: &str, asset: &PrefabAsset) -> PrefabAuthoringAssetEvidence {
    PrefabAuthoringAssetEvidence {
        source_path: relative_path.to_string(),
        prefab_id: Some(asset.prefab_id.clone()),
        root_entity_id: Some(asset.root_entity_id.clone()),
        entity_count: asset.entities.len(),
        component_count: asset
            .entities
            .iter()
            .map(|entity| entity.components.len())
            .sum(),
    }
}

fn validate_asset(relative_path: &str, asset: &PrefabAsset) -> Vec<String> {
    editor_core::validate_prefab_asset(asset)
        .into_iter()
        .map(|diagnostic| format_prefab_diagnostic(relative_path, diagnostic))
        .collect()
}

fn format_prefab_diagnostic(relative_path: &str, diagnostic: PrefabDiagnostic) -> String {
    format!(
        "{:?}:prefab_asset_diagnostic:{}:{}",
        diagnostic.severity,
        relative_path,
        diagnostic.code.as_str()
    )
    .to_ascii_lowercase()
}

fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
