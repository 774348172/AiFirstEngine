use crate::report::{ComplexProjectE2eDiagnostic, ComplexProjectE2eStatus};
use editor_core::{EditorSceneDocument, ProjectManifest, PROJECT_MANIFEST_SCHEMA_VERSION};
use engine_runtime::rule_artifact::{
    expected_rule_artifact_id, validate_runtime_rule_manifest_artifacts,
};
use engine_runtime::runtime_package::{
    RuntimeRuleManifest, RuntimeRuleModuleKind, RUNTIME_RULE_MANIFEST_MODE,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_PROJECT_ASSEMBLY_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-project-assembly-report.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterProjectAssemblySpec {
    pub project_root: PathBuf,
    pub required_directories: Vec<&'static str>,
    pub required_prefabs: Vec<&'static str>,
    pub required_assets: Vec<&'static str>,
    pub required_actions: Vec<&'static str>,
    pub required_aui_documents: Vec<&'static str>,
    pub build_profile: &'static str,
}

impl ComplexShooterProjectAssemblySpec {
    pub fn for_project(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            required_directories: vec![
                "Settings",
                "Scenes",
                "Prefabs",
                "Assets",
                "Rules",
                "AUI",
                "Input",
                "BuildProfiles",
                "Reports",
            ],
            required_prefabs: vec![
                "Prefabs/player_bullet.prefab.json",
                "Prefabs/enemy_scout.prefab.json",
                "Prefabs/explosion_effect.prefab.json",
            ],
            required_assets: vec![
                "Assets/tex-player-ship.asset",
                "Assets/tex-bullet.asset",
                "Assets/tex-enemy-scout.asset",
                "Assets/tex-starfield.asset",
                "Assets/font-main.asset",
            ],
            required_actions: vec!["action.move", "action.fire", "action.pause"],
            required_aui_documents: vec!["AUI/hud.aui.json"],
            build_profile: "BuildProfiles/windows.dev.json",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssemblyDomainStatus {
    Passed,
    Partial,
    Failed,
    Skipped,
}

impl From<AssemblyDomainStatus> for ComplexProjectE2eStatus {
    fn from(value: AssemblyDomainStatus) -> Self {
        match value {
            AssemblyDomainStatus::Passed => Self::Passed,
            AssemblyDomainStatus::Partial => Self::Partial,
            AssemblyDomainStatus::Failed => Self::Failed,
            AssemblyDomainStatus::Skipped => Self::Skipped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterProjectAssemblyDomain {
    pub domain_id: String,
    pub status: AssemblyDomainStatus,
    pub summary: String,
    #[serde(default)]
    pub required_items: Vec<String>,
    #[serde(default)]
    pub checked_items: Vec<String>,
    #[serde(default)]
    pub missing_items: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
}

impl ComplexShooterProjectAssemblyDomain {
    fn passed(
        domain_id: impl Into<String>,
        summary: impl Into<String>,
        checked_items: Vec<String>,
    ) -> Self {
        Self {
            domain_id: domain_id.into(),
            status: AssemblyDomainStatus::Passed,
            summary: summary.into(),
            required_items: Vec::new(),
            checked_items,
            missing_items: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn failed(
        domain_id: impl Into<String>,
        summary: impl Into<String>,
        missing_items: Vec<String>,
        next_actions: Vec<String>,
    ) -> Self {
        Self {
            domain_id: domain_id.into(),
            status: AssemblyDomainStatus::Failed,
            summary: summary.into(),
            required_items: Vec::new(),
            checked_items: Vec::new(),
            missing_items,
            next_actions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterProjectAssemblyDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: Option<String>,
}

impl ComplexShooterProjectAssemblyDiagnostic {
    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            path: None,
            next_action: None,
        }
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn with_next_action(mut self, next_action: impl Into<String>) -> Self {
        self.next_action = Some(next_action.into());
        self
    }
}

impl From<&ComplexShooterProjectAssemblyDiagnostic> for ComplexProjectE2eDiagnostic {
    fn from(value: &ComplexShooterProjectAssemblyDiagnostic) -> Self {
        Self {
            severity: value.severity.clone(),
            code: value.code.clone(),
            message: value.message.clone(),
            path: value.path.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterProjectAssemblyMetrics {
    pub domain_count: usize,
    pub passed_domain_count: usize,
    pub partial_domain_count: usize,
    pub failed_domain_count: usize,
    pub scene_entity_count: usize,
    pub prefab_count: usize,
    pub asset_count: usize,
    pub rule_count: usize,
    pub input_action_count: usize,
    pub aui_document_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterProjectAssemblyReport {
    pub schema_version: String,
    pub project_path: String,
    pub status: AssemblyDomainStatus,
    pub domains: Vec<ComplexShooterProjectAssemblyDomain>,
    pub metrics: ComplexShooterProjectAssemblyMetrics,
    pub diagnostics: Vec<ComplexShooterProjectAssemblyDiagnostic>,
    pub next_actions: Vec<String>,
}

impl ComplexShooterProjectAssemblyReport {
    pub fn new(project_path: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_SHOOTER_PROJECT_ASSEMBLY_REPORT_SCHEMA_VERSION.to_string(),
            project_path: project_path.into(),
            status: AssemblyDomainStatus::Failed,
            domains: Vec::new(),
            metrics: ComplexShooterProjectAssemblyMetrics::default(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    pub fn recompute(&mut self) {
        self.metrics.domain_count = self.domains.len();
        self.metrics.passed_domain_count = self
            .domains
            .iter()
            .filter(|domain| domain.status == AssemblyDomainStatus::Passed)
            .count();
        self.metrics.partial_domain_count = self
            .domains
            .iter()
            .filter(|domain| domain.status == AssemblyDomainStatus::Partial)
            .count();
        self.metrics.failed_domain_count = self
            .domains
            .iter()
            .filter(|domain| domain.status == AssemblyDomainStatus::Failed)
            .count();
        self.next_actions = self
            .domains
            .iter()
            .flat_map(|domain| domain.next_actions.clone())
            .chain(
                self.diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic.next_action.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.status = if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
            || self
                .domains
                .iter()
                .any(|domain| domain.status == AssemblyDomainStatus::Failed)
        {
            AssemblyDomainStatus::Failed
        } else if self
            .domains
            .iter()
            .any(|domain| domain.status == AssemblyDomainStatus::Partial)
        {
            AssemblyDomainStatus::Partial
        } else {
            AssemblyDomainStatus::Passed
        };
    }
}

pub struct ComplexShooterProjectAssemblyValidator;

impl ComplexShooterProjectAssemblyValidator {
    pub fn validate(
        spec: &ComplexShooterProjectAssemblySpec,
    ) -> ComplexShooterProjectAssemblyReport {
        let mut report =
            ComplexShooterProjectAssemblyReport::new(spec.project_root.display().to_string());
        let project = validate_project(spec, &mut report);
        validate_directories(spec, &mut report);
        let scene = validate_scene(spec, project.as_ref(), &mut report);
        validate_prefabs(spec, &mut report);
        validate_assets(spec, scene.as_ref(), &mut report);
        validate_rules(spec, &mut report);
        validate_input(spec, &mut report);
        validate_aui(spec, &mut report);
        validate_build_profile(spec, &mut report);
        report.recompute();
        report
    }
}

fn validate_project(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) -> Option<ProjectManifest> {
    let path = spec.project_root.join("project.aife.json");
    let manifest = read_json::<ProjectManifest>(&path, report, "ProjectManifestParseFailed")?;
    let mut missing = Vec::new();
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
        missing.push(format!(
            "schemaVersion must be {}",
            PROJECT_MANIFEST_SCHEMA_VERSION
        ));
    }
    if manifest.project_id.trim().is_empty() {
        missing.push("projectId".to_string());
    }
    if manifest.project_name.trim().is_empty() {
        missing.push("projectName".to_string());
    }
    if manifest.default_scene.trim().is_empty() {
        missing.push("defaultScene".to_string());
    }
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "project",
                format!("project {} manifest is valid", manifest.project_name),
                vec![relative(&spec.project_root, &path)],
            ));
        Some(manifest)
    } else {
        push_error(
            report,
            "ProjectManifestInvalid",
            "project manifest is missing required fields",
            &path,
            "Fix project.aife.json before running assembly validation.",
        );
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "project",
                "project manifest is invalid",
                missing,
                vec!["Fix project.aife.json required fields.".to_string()],
            ));
        None
    }
}

fn validate_directories(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let mut checked = Vec::new();
    let mut missing = Vec::new();
    for directory in &spec.required_directories {
        let path = spec.project_root.join(directory);
        if path.is_dir() {
            checked.push((*directory).to_string());
        } else {
            missing.push((*directory).to_string());
            push_error(
                report,
                "AssemblyDirectoryMissing",
                format!("required sample project directory is missing: {directory}"),
                &path,
                "Create the missing sample project directory.",
            );
        }
    }
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "directories",
                "sample project directory layout is complete",
                checked,
            ));
    } else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "directories",
                "sample project directory layout is incomplete",
                missing,
                vec!["Create missing required directories.".to_string()],
            ));
    }
}

fn validate_scene(
    spec: &ComplexShooterProjectAssemblySpec,
    project: Option<&ProjectManifest>,
    report: &mut ComplexShooterProjectAssemblyReport,
) -> Option<EditorSceneDocument> {
    let Some(project) = project else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "scene",
                "scene validation skipped because project manifest failed",
                vec!["project manifest".to_string()],
                vec!["Fix project manifest first.".to_string()],
            ));
        return None;
    };
    let scene_path = spec.project_root.join(&project.default_scene);
    match EditorSceneDocument::load_from_path(&scene_path) {
        Ok(scene) => {
            report.metrics.scene_entity_count = scene.entities.len();
            if scene.entities.len() < 6 {
                push_error(
                    report,
                    "SceneEntityCountTooSmall",
                    "complex shooter sample scene must contain at least six entities",
                    &scene_path,
                    "Add representative player, enemy, background, camera, effect, and gameplay entities.",
                );
                report
                    .domains
                    .push(ComplexShooterProjectAssemblyDomain::failed(
                        "scene",
                        "scene entity coverage is too small",
                        vec!["at least 6 scene entities".to_string()],
                        vec!["Add representative entities to Main.scene.json.".to_string()],
                    ));
                None
            } else {
                report
                    .domains
                    .push(ComplexShooterProjectAssemblyDomain::passed(
                        "scene",
                        format!("default scene loads with {} entities", scene.entities.len()),
                        vec![project.default_scene.clone()],
                    ));
                Some(scene)
            }
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                report.diagnostics.push(
                    ComplexShooterProjectAssemblyDiagnostic::error(
                        diagnostic.code,
                        diagnostic.message,
                    )
                    .with_path(scene_path.display().to_string())
                    .with_next_action("Fix the default scene document."),
                );
            }
            report
                .domains
                .push(ComplexShooterProjectAssemblyDomain::failed(
                    "scene",
                    "default scene could not be loaded",
                    vec![project.default_scene.clone()],
                    vec!["Fix the default scene document.".to_string()],
                ));
            None
        }
    }
}

fn validate_prefabs(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let mut checked = Vec::new();
    let mut missing = Vec::new();
    for prefab in &spec.required_prefabs {
        let path = spec.project_root.join(prefab);
        match read_json_value(&path, report, "PrefabParseFailed") {
            Some(value) => {
                let has_root = value
                    .get("rootEntityId")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty());
                let has_entities = value
                    .get("entities")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|items| !items.is_empty());
                if has_root && has_entities {
                    checked.push((*prefab).to_string());
                } else {
                    missing.push(format!("{prefab}: rootEntityId/entities"));
                    push_error(
                        report,
                        "PrefabShapeInvalid",
                        format!("prefab {prefab} must have rootEntityId and entities"),
                        &path,
                        "Fix the authoring prefab document shape.",
                    );
                }
            }
            None => missing.push((*prefab).to_string()),
        }
    }
    report.metrics.prefab_count = checked.len();
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "prefab",
                format!("{} required prefabs are loadable", checked.len()),
                checked,
            ));
    } else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "prefab",
                "required prefabs are missing or invalid",
                missing,
                vec!["Fix required sample prefab files.".to_string()],
            ));
    }
}

fn validate_assets(
    spec: &ComplexShooterProjectAssemblySpec,
    scene: Option<&EditorSceneDocument>,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let mut checked = Vec::new();
    let mut missing = Vec::new();
    for asset in &spec.required_assets {
        let path = spec.project_root.join(asset);
        if path.is_file() {
            checked.push((*asset).to_string());
        } else {
            missing.push((*asset).to_string());
            push_error(
                report,
                "SampleAssetMissing",
                format!("required sample asset is missing: {asset}"),
                &path,
                "Add or regenerate the missing sample asset.",
            );
        }
    }
    if let Some(scene) = scene {
        for asset_id in sprite_asset_refs(scene) {
            let asset_path = spec
                .project_root
                .join("Assets")
                .join(format!("{asset_id}.asset"));
            if asset_path.is_file() {
                checked.push(format!("scene spriteRef {asset_id}"));
            } else {
                missing.push(format!("scene spriteRef {asset_id}"));
                push_error(
                    report,
                    "SceneSpriteAssetRefMissing",
                    format!("scene SpriteRenderer2D references missing asset {asset_id}"),
                    &asset_path,
                    "Add the referenced texture asset or fix the scene SpriteRenderer2D asset ref.",
                );
            }
        }
    }
    report.metrics.asset_count = checked.len();
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "asset",
                "required assets and scene SpriteRenderer2D refs are resolvable",
                checked,
            ));
    } else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "asset",
                "required assets or SpriteRenderer2D refs are missing",
                missing,
                vec!["Fix sample asset files and SpriteRenderer2D asset refs.".to_string()],
            ));
    }
}

fn validate_rules(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let path = spec.project_root.join("Rules").join("rule-manifest.json");
    let Some(manifest) = read_json::<RuntimeRuleManifest>(&path, report, "RuleManifestParseFailed")
    else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "rule",
                "rule manifest could not be parsed",
                vec!["Rules/rule-manifest.json".to_string()],
                vec!["Fix Rules/rule-manifest.json.".to_string()],
            ));
        return;
    };
    let mut missing = Vec::new();
    if manifest.mode != RUNTIME_RULE_MANIFEST_MODE {
        missing.push(format!("mode must be {RUNTIME_RULE_MANIFEST_MODE}"));
    }
    let artifact_report = validate_runtime_rule_manifest_artifacts(None, &manifest);
    for issue in artifact_report.issues {
        missing.push(format!("{}: {}", issue.path, issue.message));
        report.diagnostics.push(
            ComplexShooterProjectAssemblyDiagnostic::error(issue.code, issue.message)
                .with_path(issue.path)
                .with_next_action("Upgrade sample rule manifest to 187 artifact lifecycle rules."),
        );
    }
    for (index, rule) in manifest.rules.iter().enumerate() {
        let prefix = format!("rules[{index}] {}", rule.rule_id);
        if rule
            .ir_source
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            missing.push(format!("{prefix}: irSource"));
        }
        let Some(ir_hash) = rule.ir_hash.as_deref() else {
            missing.push(format!("{prefix}: irHash"));
            continue;
        };
        let expected = expected_rule_artifact_id(&rule.rule_id, ir_hash);
        if rule.artifact_id.as_deref() != Some(expected.as_str()) {
            missing.push(format!("{prefix}: artifactId must be {expected}"));
        }
    }
    for module in &manifest.modules {
        if module.module_kind != RuntimeRuleModuleKind::StaticRegistry {
            missing.push(format!(
                "module {} must use staticRegistry in C-min",
                module.artifact_id
            ));
        }
    }
    report.metrics.rule_count = manifest.rules.len();
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "rule",
                format!(
                    "{} project rules follow artifact lifecycle",
                    manifest.rules.len()
                ),
                manifest
                    .rules
                    .iter()
                    .map(|rule| rule.rule_id.clone())
                    .collect(),
            ));
    } else {
        push_error(
            report,
            "RuleAssemblyInvalid",
            "project rule manifest does not follow 187 artifact lifecycle rules",
            &path,
            "Upgrade rule artifactId and module declarations.",
        );
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "rule",
                "project rule manifest is not assembly-ready",
                missing,
                vec!["Upgrade Rules/rule-manifest.json to 187 format.".to_string()],
            ));
    }
}

fn validate_input(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let path = spec.project_root.join("Input").join("input.default.json");
    let Some(value) = read_json_value(&path, report, "InputMappingParseFailed") else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "input",
                "input mapping could not be parsed",
                vec!["Input/input.default.json".to_string()],
                vec!["Fix Input/input.default.json.".to_string()],
            ));
        return;
    };
    let action_ids = value
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(|action| action.get("id").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let missing = spec
        .required_actions
        .iter()
        .filter(|action| !action_ids.contains(**action))
        .map(|action| (*action).to_string())
        .collect::<Vec<_>>();
    report.metrics.input_action_count = action_ids.len();
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "input",
                format!("input mapping has {} actions", action_ids.len()),
                action_ids.into_iter().collect(),
            ));
    } else {
        push_error(
            report,
            "InputActionsMissing",
            "input mapping is missing required sample actions",
            &path,
            "Add required actions to Input/input.default.json.",
        );
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "input",
                "input mapping is missing required actions",
                missing,
                vec!["Add required input actions.".to_string()],
            ));
    }
}

fn validate_aui(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let mut checked = Vec::new();
    let mut missing = Vec::new();
    for document in &spec.required_aui_documents {
        let path = spec.project_root.join(document);
        let Some(value) = read_json_value(&path, report, "AuiDocumentParseFailed") else {
            missing.push((*document).to_string());
            continue;
        };
        let has_id = value
            .get("documentId")
            .or_else(|| value.get("document_id"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| !id.trim().is_empty());
        if has_id {
            checked.push((*document).to_string());
        } else {
            missing.push(format!("{document}: documentId"));
            push_error(
                report,
                "AuiDocumentShapeInvalid",
                format!("AUI document {document} must have documentId"),
                &path,
                "Fix the AUI HUD document.",
            );
        }
    }
    report.metrics.aui_document_count = checked.len();
    if missing.is_empty() {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "aui",
                "required AUI HUD documents are parseable",
                checked,
            ));
    } else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "aui",
                "required AUI HUD documents are missing or invalid",
                missing,
                vec!["Fix required AUI HUD documents.".to_string()],
            ));
    }
}

fn validate_build_profile(
    spec: &ComplexShooterProjectAssemblySpec,
    report: &mut ComplexShooterProjectAssemblyReport,
) {
    let path = spec.project_root.join(spec.build_profile);
    let Some(value) = read_json_value(&path, report, "BuildProfileParseFailed") else {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "build",
                "Windows dev build profile is missing or invalid",
                vec![spec.build_profile.to_string()],
                vec!["Create BuildProfiles/windows.dev.json.".to_string()],
            ));
        return;
    };
    let target = value
        .get("target")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let profile = value
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if target == "windows" && profile == "dev" {
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::passed(
                "build",
                "Windows dev build profile is present",
                vec![spec.build_profile.to_string()],
            ));
    } else {
        push_error(
            report,
            "BuildProfileInvalid",
            "BuildProfiles/windows.dev.json must target windows dev",
            &path,
            "Set target=windows and profile=dev.",
        );
        report
            .domains
            .push(ComplexShooterProjectAssemblyDomain::failed(
                "build",
                "Windows dev build profile is invalid",
                vec!["target=windows".to_string(), "profile=dev".to_string()],
                vec!["Fix BuildProfiles/windows.dev.json.".to_string()],
            ));
    }
}

fn sprite_asset_refs(scene: &EditorSceneDocument) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for entity in &scene.entities {
        for component in &entity.components {
            if component.component_type != "SpriteRenderer2D" {
                continue;
            }
            if let Some(id) = component
                .fields
                .get("spriteRef")
                .or_else(|| component.fields.get("sprite_ref"))
                .and_then(|value| value.get("id"))
                .and_then(serde_json::Value::as_str)
            {
                refs.insert(id.to_string());
            }
        }
    }
    refs
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    report: &mut ComplexShooterProjectAssemblyReport,
    code: &'static str,
) -> Option<T> {
    let value = read_json_value(path, report, code)?;
    serde_json::from_value(value)
        .map_err(|error| {
            report.diagnostics.push(
                ComplexShooterProjectAssemblyDiagnostic::error(
                    code,
                    format!("failed to decode JSON shape: {error}"),
                )
                .with_path(path.display().to_string())
                .with_next_action("Fix the JSON document shape."),
            );
        })
        .ok()
}

fn read_json_value(
    path: &Path,
    report: &mut ComplexShooterProjectAssemblyReport,
    code: &'static str,
) -> Option<serde_json::Value> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            report.diagnostics.push(
                ComplexShooterProjectAssemblyDiagnostic::error(
                    "AssemblyFileReadFailed",
                    format!("failed to read file: {error}"),
                )
                .with_path(path.display().to_string())
                .with_next_action("Create or restore the missing sample project file."),
            );
            return None;
        }
    };
    serde_json::from_str(&text)
        .map_err(|error| {
            report.diagnostics.push(
                ComplexShooterProjectAssemblyDiagnostic::error(
                    code,
                    format!("failed to parse JSON: {error}"),
                )
                .with_path(path.display().to_string())
                .with_next_action("Fix the JSON syntax."),
            );
        })
        .ok()
}

fn push_error(
    report: &mut ComplexShooterProjectAssemblyReport,
    code: impl Into<String>,
    message: impl Into<String>,
    path: &Path,
    next_action: impl Into<String>,
) {
    report.diagnostics.push(
        ComplexShooterProjectAssemblyDiagnostic::error(code, message)
            .with_path(path.display().to_string())
            .with_next_action(next_action),
    );
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[allow(dead_code)]
fn _domain_map(
    domains: &[ComplexShooterProjectAssemblyDomain],
) -> BTreeMap<String, AssemblyDomainStatus> {
    domains
        .iter()
        .map(|domain| (domain.domain_id.clone(), domain.status))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn assembly_validator_accepts_complex_shooter_sample_project() {
        let report = ComplexShooterProjectAssemblyValidator::validate(
            &ComplexShooterProjectAssemblySpec::for_project(sample_project_root()),
        );

        assert_eq!(report.status, AssemblyDomainStatus::Passed, "{report:#?}");
        assert!(report
            .domains
            .iter()
            .any(|domain| domain.domain_id == "rule"
                && domain.status == AssemblyDomainStatus::Passed));
        assert!(report.metrics.scene_entity_count >= 6);
    }

    #[test]
    fn assembly_validator_reports_missing_required_directory() {
        let root = copy_sample_project("missing-build-profile-dir");
        std::fs::remove_dir_all(root.join("BuildProfiles")).unwrap();

        let report = ComplexShooterProjectAssemblyValidator::validate(
            &ComplexShooterProjectAssemblySpec::for_project(&root),
        );

        assert_eq!(report.status, AssemblyDomainStatus::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AssemblyDirectoryMissing"));
    }

    #[test]
    fn assembly_validator_rejects_legacy_rule_module_artifact_id() {
        let root = copy_sample_project("legacy-rule-module");
        let path = root.join("Rules").join("rule-manifest.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["modules"][0]["artifactId"] =
            serde_json::Value::String("sample-project-rules".to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let report = ComplexShooterProjectAssemblyValidator::validate(
            &ComplexShooterProjectAssemblySpec::for_project(&root),
        );

        assert_eq!(report.status, AssemblyDomainStatus::Failed);
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "missing_rule_module_for_artifact"
            || diagnostic.code == "RuleAssemblyInvalid"));
    }

    fn sample_project_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("samples")
            .join("complex_shooter_project")
    }

    fn copy_sample_project(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("assembly-validator-{name}-{stamp}"));
        copy_dir(&sample_project_root(), &root).unwrap();
        root
    }

    fn copy_dir(source: &Path, destination: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(destination)?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir(&source_path, &destination_path)?;
            } else {
                std::fs::copy(&source_path, &destination_path)?;
            }
        }
        Ok(())
    }
}
