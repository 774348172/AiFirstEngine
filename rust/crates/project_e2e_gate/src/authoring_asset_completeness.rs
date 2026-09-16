use editor_core::{
    scan_rule_asset_paths, EditorSceneDocument, EditorSceneEntity, PrefabAsset, PrefabInstance,
    PrefabWorkflowService, RuleAuthoringService,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_AUTHORING_ASSET_COMPLETENESS_REPORT_SCHEMA_VERSION: &str =
    "project-authoring-asset-completeness-report.v1";
pub const PROJECT_AUTHORING_ASSET_COMPLETENESS_SCENARIO_ID: &str =
    "project-authoring-asset-completeness-prefab-rule-assetization-gate-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectAuthoringAssetCompletenessStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAuthoringAssetCompletenessReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ProjectAuthoringAssetCompletenessStatus,
    pub project_root: String,
    pub output_root: String,
    pub scanned_domains: Vec<String>,
    pub prefab_summary: PrefabCompletenessSummary,
    pub rule_summary: RuleCompletenessSummary,
    pub candidates: Vec<AssetizationCandidate>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ProjectAuthoringAssetCompletenessReport {
    fn new(project_root: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_AUTHORING_ASSET_COMPLETENESS_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: PROJECT_AUTHORING_ASSET_COMPLETENESS_SCENARIO_ID.to_string(),
            status: ProjectAuthoringAssetCompletenessStatus::Failed,
            project_root: project_root.into(),
            output_root: output_root.into(),
            scanned_domains: vec!["prefab".to_string(), "rule".to_string()],
            prefab_summary: PrefabCompletenessSummary::default(),
            rule_summary: RuleCompletenessSummary::default(),
            candidates: Vec::new(),
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn recompute_status(&mut self) {
        self.next_actions.clear();
        if self.prefab_summary.prefab_asset_count == 0 {
            self.next_actions.push("create_prefab_assets".to_string());
        }
        if self.prefab_summary.missing_scene_instance_evidence {
            self.next_actions
                .push("convert_scene_entity_to_prefab_instance".to_string());
        }
        if !self.rule_summary.missing_authoring_rule_ids.is_empty() {
            self.next_actions
                .push("migrate_runtime_manifest_to_rule_authoring_assets".to_string());
        }
        if !self.rule_summary.stale_authoring_rule_ids.is_empty() {
            self.next_actions
                .push("review_stale_rule_authoring_assets".to_string());
        }
        self.next_actions.sort();
        self.next_actions.dedup();

        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("error:"))
        {
            self.status = ProjectAuthoringAssetCompletenessStatus::Failed;
        } else if self.next_actions.is_empty() {
            self.status = ProjectAuthoringAssetCompletenessStatus::Passed;
        } else {
            self.status = ProjectAuthoringAssetCompletenessStatus::Partial;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrefabCompletenessSummary {
    pub prefab_asset_count: usize,
    pub scene_prefab_instance_count: usize,
    pub runtime_spawn_reference_count: usize,
    pub unused_prefab_asset_ids: Vec<String>,
    pub missing_scene_instance_evidence: bool,
    pub explicit_waivers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuleCompletenessSummary {
    pub runtime_manifest_rule_count: usize,
    pub rule_authoring_asset_count: usize,
    pub missing_authoring_rule_ids: Vec<String>,
    pub stale_authoring_rule_ids: Vec<String>,
    pub migration_candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetizationCandidate {
    pub candidate_id: String,
    pub domain: AssetizationCandidateDomain,
    pub source_kind: String,
    pub source_path: String,
    pub target_path: Option<String>,
    pub scene_entity_id: Option<String>,
    pub prefab_asset_id: Option<String>,
    pub rule_id: Option<String>,
    pub confidence: AssetizationCandidateConfidence,
    pub status: AssetizationCandidateStatus,
    pub apply_route: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetizationCandidateDomain {
    Prefab,
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetizationCandidateConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetizationCandidateStatus {
    Ready,
    Blocked,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectAuthoringAssetCompletenessRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ProjectAuthoringAssetCompletenessRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct PrefabScan {
    assets: Vec<PrefabAssetRecord>,
    instances: Vec<PrefabInstanceRecord>,
    runtime_spawn_refs: Vec<RuntimePrefabReference>,
    plain_scene_entities: Vec<SceneEntityRecord>,
}

#[derive(Debug, Clone)]
struct PrefabAssetRecord {
    relative_path: String,
    asset: PrefabAsset,
}

#[derive(Debug, Clone)]
struct PrefabInstanceRecord {
    scene_path: String,
    entity_id: String,
    instance: PrefabInstance,
}

#[derive(Debug, Clone)]
struct RuntimePrefabReference {
    source_path: String,
    scene_entity_id: String,
    prefab_id: String,
}

#[derive(Debug, Clone)]
struct SceneEntityRecord {
    scene_path: String,
    entity: EditorSceneEntity,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRuleManifestView {
    #[serde(default)]
    rules: Vec<RuntimeRuleManifestRuleView>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRuleManifestRuleView {
    rule_id: String,
    phase: String,
    #[serde(default)]
    ir_source: Option<String>,
    #[serde(default)]
    artifact_id: Option<String>,
}

pub fn run_project_authoring_asset_completeness_report(
    request: ProjectAuthoringAssetCompletenessRequest,
) -> ProjectAuthoringAssetCompletenessReport {
    let mut report = ProjectAuthoringAssetCompletenessReport::new(
        request.project_root.display().to_string(),
        request.output_root.display().to_string(),
    );

    let prefab_scan = scan_prefab_domain(&request.project_root, &mut report);
    apply_prefab_scan_to_report(&prefab_scan, &mut report);

    let manifest_rules = scan_rule_domain(&request.project_root, &mut report);
    apply_rule_scan_to_report(&request.project_root, &manifest_rules, &mut report);

    report.recompute_status();
    let artifact_path = request
        .output_root
        .join("reports")
        .join("project-authoring-asset-completeness-report.json");
    if write_json(&artifact_path, &report).is_ok() {
        report.artifacts.push(artifact_path.display().to_string());
    } else {
        report.diagnostics.push(format!(
            "error:authoring_asset_completeness_report_write_failed:{}",
            artifact_path.display()
        ));
        report.recompute_status();
        return report;
    }
    report.recompute_status();
    if write_json(&artifact_path, &report).is_err() {
        report.diagnostics.push(format!(
            "error:authoring_asset_completeness_report_rewrite_failed:{}",
            artifact_path.display()
        ));
        report.recompute_status();
    }
    report
}

pub fn run_complex_shooter_authoring_asset_completeness_report(
    request: ProjectAuthoringAssetCompletenessRequest,
) -> ProjectAuthoringAssetCompletenessReport {
    run_project_authoring_asset_completeness_report(request)
}

fn scan_prefab_domain(
    project_root: &Path,
    report: &mut ProjectAuthoringAssetCompletenessReport,
) -> PrefabScan {
    let mut scan = PrefabScan {
        assets: Vec::new(),
        instances: Vec::new(),
        runtime_spawn_refs: Vec::new(),
        plain_scene_entities: Vec::new(),
    };

    for path in collect_files_with_suffix(&project_root.join("Prefabs"), ".prefab.json") {
        let relative_path = project_relative_path(project_root, &path);
        match PrefabWorkflowService::load_asset(project_root, &relative_path) {
            Ok(asset) => scan.assets.push(PrefabAssetRecord {
                relative_path,
                asset,
            }),
            Err(message) => report.diagnostics.push(format!(
                "error:prefab_asset_load_failed:{relative_path}:{message}"
            )),
        }
    }

    for path in collect_files_with_suffix(&project_root.join("Scenes"), ".scene.json") {
        let relative_path = project_relative_path(project_root, &path);
        let scene = match EditorSceneDocument::load_from_path(&path) {
            Ok(scene) => scene,
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    report.diagnostics.push(format!(
                        "error:scene_load_failed:{}:{}",
                        relative_path, diagnostic.code
                    ));
                }
                continue;
            }
        };
        for entity in scene.entities {
            collect_runtime_prefab_references(
                &relative_path,
                &entity,
                &mut scan.runtime_spawn_refs,
            );
            match PrefabInstance::from_scene_entity(&entity) {
                Ok(instance) => scan.instances.push(PrefabInstanceRecord {
                    scene_path: relative_path.clone(),
                    entity_id: entity.entity_id.clone(),
                    instance,
                }),
                Err(_) => scan.plain_scene_entities.push(SceneEntityRecord {
                    scene_path: relative_path.clone(),
                    entity,
                }),
            }
        }
    }

    scan
}

fn apply_prefab_scan_to_report(
    scan: &PrefabScan,
    report: &mut ProjectAuthoringAssetCompletenessReport,
) {
    let prefab_ids = scan
        .assets
        .iter()
        .map(|asset| asset.asset.prefab_id.clone())
        .collect::<BTreeSet<_>>();
    let scene_instance_prefab_ids = scan
        .instances
        .iter()
        .map(|instance| instance.instance.prefab_ref.id.clone())
        .collect::<BTreeSet<_>>();
    let runtime_spawn_prefab_ids = scan
        .runtime_spawn_refs
        .iter()
        .map(|reference| reference.prefab_id.clone())
        .collect::<BTreeSet<_>>();
    let used_prefab_ids = scene_instance_prefab_ids
        .union(&runtime_spawn_prefab_ids)
        .cloned()
        .collect::<BTreeSet<_>>();

    report.prefab_summary.prefab_asset_count = scan.assets.len();
    report.prefab_summary.scene_prefab_instance_count = scan.instances.len();
    report.prefab_summary.runtime_spawn_reference_count = scan.runtime_spawn_refs.len();
    report.prefab_summary.unused_prefab_asset_ids = prefab_ids
        .difference(&used_prefab_ids)
        .cloned()
        .collect::<Vec<_>>();
    report.prefab_summary.missing_scene_instance_evidence =
        !scan.assets.is_empty() && scan.instances.is_empty();

    for record in &scan.instances {
        if !prefab_ids.contains(&record.instance.prefab_ref.id) {
            report.diagnostics.push(format!(
                "error:scene_prefab_instance_missing_asset:{}:{}:{}",
                record.scene_path, record.entity_id, record.instance.prefab_ref.id
            ));
        }
    }

    for reference in &scan.runtime_spawn_refs {
        let status = if prefab_ids.contains(&reference.prefab_id) {
            AssetizationCandidateStatus::Ready
        } else {
            report.diagnostics.push(format!(
                "error:runtime_spawn_reference_missing_prefab:{}:{}",
                reference.source_path, reference.prefab_id
            ));
            AssetizationCandidateStatus::Blocked
        };
        report.candidates.push(AssetizationCandidate {
            candidate_id: format!(
                "prefab-runtime-spawn-reference:{}:{}",
                reference.scene_entity_id, reference.prefab_id
            ),
            domain: AssetizationCandidateDomain::Prefab,
            source_kind: "scene_component_prefab_id".to_string(),
            source_path: reference.source_path.clone(),
            target_path: None,
            scene_entity_id: Some(reference.scene_entity_id.clone()),
            prefab_asset_id: Some(reference.prefab_id.clone()),
            rule_id: None,
            confidence: AssetizationCandidateConfidence::High,
            status,
            apply_route: "existing_runtime_spawn_reference".to_string(),
            diagnostics: Vec::new(),
        });
    }

    for prefab_id in &report.prefab_summary.unused_prefab_asset_ids {
        let source_path = scan
            .assets
            .iter()
            .find(|asset| asset.asset.prefab_id == *prefab_id)
            .map(|asset| asset.relative_path.clone())
            .unwrap_or_else(|| "Prefabs".to_string());
        report.candidates.push(AssetizationCandidate {
            candidate_id: format!("prefab-unused:{prefab_id}"),
            domain: AssetizationCandidateDomain::Prefab,
            source_kind: "prefab_asset_without_usage_evidence".to_string(),
            source_path,
            target_path: None,
            scene_entity_id: None,
            prefab_asset_id: Some(prefab_id.clone()),
            rule_id: None,
            confidence: AssetizationCandidateConfidence::Medium,
            status: AssetizationCandidateStatus::Warning,
            apply_route: "review_or_add_explicit_waiver".to_string(),
            diagnostics: vec!["unused_prefab_asset_is_non_blocking_in_b_min".to_string()],
        });
    }

    for entity in &scan.plain_scene_entities {
        for asset in &scan.assets {
            if is_scene_entity_prefab_candidate(&entity.entity, &asset.asset) {
                report.candidates.push(AssetizationCandidate {
                    candidate_id: format!(
                        "prefab-convert:{}:{}",
                        entity.entity.entity_id, asset.asset.prefab_id
                    ),
                    domain: AssetizationCandidateDomain::Prefab,
                    source_kind: "scene_entity_matches_prefab_asset".to_string(),
                    source_path: entity.scene_path.clone(),
                    target_path: Some(asset.relative_path.clone()),
                    scene_entity_id: Some(entity.entity.entity_id.clone()),
                    prefab_asset_id: Some(asset.asset.prefab_id.clone()),
                    rule_id: None,
                    confidence: AssetizationCandidateConfidence::Medium,
                    status: AssetizationCandidateStatus::Ready,
                    apply_route: "convert_scene_entity_to_prefab_instance".to_string(),
                    diagnostics: vec![
                        "candidate_requires_user_or_ai_review_before_apply".to_string()
                    ],
                });
            }
        }
    }
}

fn scan_rule_domain(
    project_root: &Path,
    report: &mut ProjectAuthoringAssetCompletenessReport,
) -> Vec<RuntimeRuleManifestRuleView> {
    let manifest_path = project_root.join("Rules").join("rule-manifest.json");
    let text = match fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(error) => {
            report.diagnostics.push(format!(
                "error:runtime_rule_manifest_read_failed:{}:{error}",
                manifest_path.display()
            ));
            return Vec::new();
        }
    };
    match serde_json::from_str::<RuntimeRuleManifestView>(&text) {
        Ok(manifest) => manifest.rules,
        Err(error) => {
            report.diagnostics.push(format!(
                "error:runtime_rule_manifest_parse_failed:{}:{error}",
                manifest_path.display()
            ));
            Vec::new()
        }
    }
}

fn apply_rule_scan_to_report(
    project_root: &Path,
    manifest_rules: &[RuntimeRuleManifestRuleView],
    report: &mut ProjectAuthoringAssetCompletenessReport,
) {
    let mut assets_by_rule_id = BTreeMap::new();
    for path in scan_rule_asset_paths(project_root) {
        match RuleAuthoringService::load(project_root, &path) {
            Ok(asset) => {
                assets_by_rule_id.insert(asset.rule_id.clone(), path);
            }
            Err(message) => {
                report.diagnostics.push(format!(
                    "error:rule_authoring_asset_load_failed:{path}:{message}"
                ));
            }
        }
    }

    let manifest_rule_ids = manifest_rules
        .iter()
        .map(|rule| rule.rule_id.clone())
        .collect::<BTreeSet<_>>();
    let authoring_rule_ids = assets_by_rule_id.keys().cloned().collect::<BTreeSet<_>>();
    let mut missing = manifest_rule_ids
        .difference(&authoring_rule_ids)
        .cloned()
        .collect::<Vec<_>>();
    let mut stale = authoring_rule_ids
        .difference(&manifest_rule_ids)
        .cloned()
        .collect::<Vec<_>>();
    missing.sort();
    stale.sort();

    report.rule_summary.runtime_manifest_rule_count = manifest_rules.len();
    report.rule_summary.rule_authoring_asset_count = authoring_rule_ids.len();
    report.rule_summary.missing_authoring_rule_ids = missing.clone();
    report.rule_summary.stale_authoring_rule_ids = stale;

    for rule_id in missing {
        let manifest_rule = manifest_rules
            .iter()
            .find(|rule| rule.rule_id == rule_id)
            .expect("missing rule id came from manifest");
        let target_path = rule_asset_target_path(&rule_id);
        let mut diagnostics = vec![format!("runtime_manifest_phase={}", manifest_rule.phase)];
        if let Some(ir_source) = &manifest_rule.ir_source {
            diagnostics.push(format!("ir_source={ir_source}"));
        }
        if let Some(artifact_id) = &manifest_rule.artifact_id {
            diagnostics.push(format!("artifact_id={artifact_id}"));
        }
        report.candidates.push(AssetizationCandidate {
            candidate_id: format!("rule-migrate:{rule_id}"),
            domain: AssetizationCandidateDomain::Rule,
            source_kind: "runtime_rule_manifest_entry".to_string(),
            source_path: "Rules/rule-manifest.json".to_string(),
            target_path: Some(target_path),
            scene_entity_id: None,
            prefab_asset_id: None,
            rule_id: Some(rule_id),
            confidence: AssetizationCandidateConfidence::High,
            status: AssetizationCandidateStatus::Ready,
            apply_route: "migrate_runtime_manifest_entry_to_authoring_rule".to_string(),
            diagnostics,
        });
    }
    report.rule_summary.migration_candidate_count = report
        .candidates
        .iter()
        .filter(|candidate| candidate.domain == AssetizationCandidateDomain::Rule)
        .filter(|candidate| {
            candidate.apply_route == "migrate_runtime_manifest_entry_to_authoring_rule"
        })
        .count();
}

fn collect_runtime_prefab_references(
    scene_path: &str,
    entity: &EditorSceneEntity,
    output: &mut Vec<RuntimePrefabReference>,
) {
    for component in &entity.components {
        let source = format!(
            "{scene_path}#{}:{}",
            entity.entity_id, component.component_type
        );
        collect_runtime_prefab_references_from_value(
            &component.fields,
            &source,
            &entity.entity_id,
            output,
        );
    }
}

fn collect_runtime_prefab_references_from_value(
    value: &serde_json::Value,
    source_path: &str,
    scene_entity_id: &str,
    output: &mut Vec<RuntimePrefabReference>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "prefabId" | "prefab_id") {
                    if let Some(prefab_id) = value.as_str() {
                        output.push(RuntimePrefabReference {
                            source_path: format!("{source_path}.{key}"),
                            scene_entity_id: scene_entity_id.to_string(),
                            prefab_id: prefab_id.to_string(),
                        });
                    }
                }
                collect_runtime_prefab_references_from_value(
                    value,
                    source_path,
                    scene_entity_id,
                    output,
                );
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_runtime_prefab_references_from_value(
                    value,
                    source_path,
                    scene_entity_id,
                    output,
                );
            }
        }
        _ => {}
    }
}

fn is_scene_entity_prefab_candidate(entity: &EditorSceneEntity, prefab: &PrefabAsset) -> bool {
    let entity_name = normalized_key(&entity.name);
    let prefab_name = normalized_key(&prefab.name);
    if prefab_name.is_empty() || !entity_name.contains(&prefab_name) {
        return false;
    }
    let prefab_components = prefab
        .entities
        .iter()
        .flat_map(|entity| entity.components.iter())
        .map(|component| component.component_type.as_str())
        .collect::<BTreeSet<_>>();
    if prefab_components.is_empty() {
        return true;
    }
    entity
        .components
        .iter()
        .any(|component| prefab_components.contains(component.component_type.as_str()))
}

fn normalized_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn rule_asset_target_path(rule_id: &str) -> String {
    let short = rule_id.strip_prefix("rule.").unwrap_or(rule_id);
    let file_stem = short.replace(['.', '-'], "_");
    format!("Rules/{file_stem}.rule.json")
}

fn collect_files_with_suffix(root: &Path, suffix: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(suffix))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
