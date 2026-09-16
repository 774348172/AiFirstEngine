use editor_core::{
    PrefabRuntimeBakeInstanceEntry, PrefabRuntimeBakeReport, ProjectRuntimePackageAssembler,
    ProjectRuntimePackageAssemblyReport, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus, PREFAB_INSTANCE_COMPONENT_TYPE,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-prefab-runtime-bake-report.v1";
pub const COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_SCENARIO_ID: &str =
    "complex-shooter-prefab-runtime-bake-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterPrefabRuntimeBakeStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterPrefabRuntimeBakeMetrics {
    pub prefab_asset_count: usize,
    pub scene_prefab_instance_count: usize,
    pub baked_instance_count: usize,
    pub baked_entity_count: usize,
    pub runtime_scene_entity_count: usize,
    pub runtime_scene_prefab_instance_component_count: usize,
    pub enemy_scout_baked_instance_count: usize,
    pub enemy_a_baked: bool,
    pub enemy_b_baked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterPrefabRuntimeBakeReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterPrefabRuntimeBakeStatus,
    pub project_root: String,
    pub output_root: String,
    pub assembly_status: Option<ProjectRuntimePackageAssemblyStatus>,
    pub metrics: ComplexShooterPrefabRuntimeBakeMetrics,
    pub prefab_bake_report: Option<PrefabRuntimeBakeReport>,
    pub baked_instances: Vec<PrefabRuntimeBakeInstanceEntry>,
    pub assembly_report: Option<ProjectRuntimePackageAssemblyReport>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ComplexShooterPrefabRuntimeBakeReport {
    fn new(project_root: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_SCENARIO_ID.to_string(),
            status: ComplexShooterPrefabRuntimeBakeStatus::Failed,
            project_root: project_root.into(),
            output_root: output_root.into(),
            assembly_status: None,
            metrics: ComplexShooterPrefabRuntimeBakeMetrics::default(),
            prefab_bake_report: None,
            baked_instances: Vec::new(),
            assembly_report: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn recompute(&mut self) {
        self.next_actions.clear();
        if self.assembly_status != Some(ProjectRuntimePackageAssemblyStatus::Success) {
            self.next_actions
                .push("fix_runtime_package_assembly".to_string());
        }
        if self.metrics.baked_instance_count < 2 {
            self.next_actions
                .push("bake_enemy_scout_scene_prefab_instances".to_string());
        }
        if !self.metrics.enemy_a_baked {
            self.next_actions
                .push("bake_entity_enemy_a_from_prefab_enemy_scout".to_string());
        }
        if !self.metrics.enemy_b_baked {
            self.next_actions
                .push("bake_entity_enemy_b_from_prefab_enemy_scout".to_string());
        }
        if self.metrics.runtime_scene_prefab_instance_component_count > 0 {
            self.next_actions
                .push("strip_authoring_prefab_instance_from_runtime_scene".to_string());
        }
        self.next_actions.sort();
        self.next_actions.dedup();

        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("error:"))
        {
            self.status = ComplexShooterPrefabRuntimeBakeStatus::Failed;
        } else if self.next_actions.is_empty() {
            self.status = ComplexShooterPrefabRuntimeBakeStatus::Passed;
        } else {
            self.status = ComplexShooterPrefabRuntimeBakeStatus::Partial;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterPrefabRuntimeBakeRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterPrefabRuntimeBakeRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_prefab_runtime_bake_report(
    request: ComplexShooterPrefabRuntimeBakeRequest,
) -> ComplexShooterPrefabRuntimeBakeReport {
    let mut report = ComplexShooterPrefabRuntimeBakeReport::new(
        request.project_root.display().to_string(),
        request.output_root.display().to_string(),
    );

    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&request.project_root),
    );
    report.assembly_status = Some(assembly.status);
    report.assembly_report = Some(assembly.report.clone());

    if let Some(prefab_bake_report) = assembly.report.prefab_bake_report.clone() {
        report.metrics.prefab_asset_count = prefab_bake_report.prefab_asset_count;
        report.metrics.scene_prefab_instance_count = prefab_bake_report.scene_prefab_instance_count;
        report.metrics.baked_instance_count = prefab_bake_report.baked_instance_count;
        report.metrics.baked_entity_count = prefab_bake_report.baked_entity_count;
        report.metrics.enemy_scout_baked_instance_count = prefab_bake_report
            .instances
            .iter()
            .filter(|instance| instance.prefab_id == "prefab-enemy-scout")
            .count();
        report.metrics.enemy_a_baked = prefab_bake_report.instances.iter().any(|instance| {
            instance.scene_entity_id == "entity-enemy-a"
                && instance.prefab_id == "prefab-enemy-scout"
                && instance
                    .emitted_entity_ids
                    .contains(&"entity-enemy-a".to_string())
        });
        report.metrics.enemy_b_baked = prefab_bake_report.instances.iter().any(|instance| {
            instance.scene_entity_id == "entity-enemy-b"
                && instance.prefab_id == "prefab-enemy-scout"
                && instance
                    .emitted_entity_ids
                    .contains(&"entity-enemy-b".to_string())
        });
        report.baked_instances = prefab_bake_report.instances.clone();
        report.prefab_bake_report = Some(prefab_bake_report);
    } else {
        report
            .diagnostics
            .push("error:prefab_bake_report_missing".to_string());
    }

    if let Some(input) = assembly.build_input {
        if let Some(scene) = input.scenes.first() {
            report.metrics.runtime_scene_entity_count = scene.entities.len();
            report.metrics.runtime_scene_prefab_instance_component_count = scene
                .entities
                .iter()
                .flat_map(|entity| entity.components.iter())
                .filter(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE)
                .count();
        } else {
            report
                .diagnostics
                .push("error:runtime_scene_missing".to_string());
        }
    } else {
        report
            .diagnostics
            .push("error:runtime_package_build_input_missing".to_string());
    }

    if report.assembly_status != Some(ProjectRuntimePackageAssemblyStatus::Success) {
        report
            .diagnostics
            .push("error:runtime_package_assembly_failed".to_string());
    }

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-prefab-runtime-bake-report.json");
    report.recompute();
    if write_json(&artifact_path, &report).is_ok() {
        report.artifacts.push(artifact_path.display().to_string());
    } else {
        report.diagnostics.push(format!(
            "error:prefab_runtime_bake_report_write_failed:{}",
            artifact_path.display()
        ));
    }
    report.recompute();
    if write_json(&artifact_path, &report).is_err() {
        report.diagnostics.push(format!(
            "error:prefab_runtime_bake_report_rewrite_failed:{}",
            artifact_path.display()
        ));
        report.recompute();
    }
    report
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, json)
}
