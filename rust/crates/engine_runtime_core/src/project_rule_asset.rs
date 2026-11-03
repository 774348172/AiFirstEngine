use crate::atomic_file_replace::atomic_file_replace;
use crate::rule_artifact::expected_rule_artifact_id;
use crate::rule_ir::{
    stable_ir_hash, ProjectRuleIr, RuleIrDiagnostic, RuleIrValidationReport, RuleIrValidationStatus,
};
use crate::runtime_package::{
    RuntimeRuleExecutor, RuntimeRuleManifest, RuntimeRuleManifestEntry, RuntimeRuleModuleEntry,
    RuntimeRuleModuleKind, RuntimeRulePhase, RUNTIME_RULE_MANIFEST_MODE,
    RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const PROJECT_RULE_ASSET_SCHEMA_VERSION: &str = "project-rule-asset.v1";
pub const RULE_ASSET_MANIFEST_SCHEMA_VERSION: &str = "rule-asset-manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRuleAssetSourceKind {
    AiDoc,
    UserAuthored,
    Template,
    Imported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuleSourceMap {
    #[serde(default)]
    pub feature_id: Option<String>,
    #[serde(default)]
    pub document_path: Option<String>,
    #[serde(default)]
    pub source_range: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuleAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub rule_id: String,
    pub display_name: String,
    pub source_kind: ProjectRuleAssetSourceKind,
    pub enabled: bool,
    pub canonical_ir: ProjectRuleIr,
    #[serde(default)]
    pub source_map: ProjectRuleSourceMap,
    #[serde(default)]
    pub validation: ProjectRuleAssetValidationCache,
}

impl ProjectRuleAsset {
    pub fn new(
        asset_id: impl Into<String>,
        display_name: impl Into<String>,
        source_kind: ProjectRuleAssetSourceKind,
        canonical_ir: ProjectRuleIr,
    ) -> Self {
        let rule_id = canonical_ir.rule_id.clone();
        Self {
            schema_version: PROJECT_RULE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: asset_id.into(),
            rule_id,
            display_name: display_name.into(),
            source_kind,
            enabled: true,
            canonical_ir,
            source_map: ProjectRuleSourceMap::default(),
            validation: ProjectRuleAssetValidationCache::default(),
        }
    }

    pub fn ir_hash(&self) -> String {
        stable_ir_hash(&self.canonical_ir)
    }

    pub fn validate(&self) -> ProjectRuleAssetValidationReport {
        validate_project_rule_asset(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectRuleAssetValidationStatus {
    Unknown,
    Success,
    Failed,
}

impl Default for ProjectRuleAssetValidationStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuleAssetValidationCache {
    pub status: ProjectRuleAssetValidationStatus,
    #[serde(default)]
    pub diagnostics: Vec<RuleIrDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuleAssetValidationReport {
    pub status: ProjectRuleAssetValidationStatus,
    #[serde(default)]
    pub diagnostics: Vec<RuleIrDiagnostic>,
}

pub fn validate_project_rule_asset(asset: &ProjectRuleAsset) -> ProjectRuleAssetValidationReport {
    let mut diagnostics = Vec::new();
    if asset.schema_version != PROJECT_RULE_ASSET_SCHEMA_VERSION {
        diagnostics.push(RuleIrDiagnostic {
            code: "InvalidProjectRuleAssetSchema".to_string(),
            message: format!(
                "schemaVersion must be {}",
                PROJECT_RULE_ASSET_SCHEMA_VERSION
            ),
            path: Some("schemaVersion".to_string()),
            suggestion: None,
        });
    }
    if asset.asset_id.trim().is_empty() {
        diagnostics.push(RuleIrDiagnostic {
            code: "MissingRuleAssetId".to_string(),
            message: "assetId is required".to_string(),
            path: Some("assetId".to_string()),
            suggestion: Some("Use a stable project asset id.".to_string()),
        });
    }
    if asset.rule_id != asset.canonical_ir.rule_id {
        diagnostics.push(RuleIrDiagnostic {
            code: "RuleAssetIdMismatch".to_string(),
            message: "ruleId must match canonicalIr.ruleId".to_string(),
            path: Some("ruleId".to_string()),
            suggestion: Some(
                "Keep ProjectRuleAsset.ruleId derived from Canonical Rule IR.".to_string(),
            ),
        });
    }
    let RuleIrValidationReport {
        status,
        diagnostics: ir_diagnostics,
    } = asset.canonical_ir.validate();
    diagnostics.extend(ir_diagnostics);
    ProjectRuleAssetValidationReport {
        status: if diagnostics.is_empty() && status == RuleIrValidationStatus::Success {
            ProjectRuleAssetValidationStatus::Success
        } else {
            ProjectRuleAssetValidationStatus::Failed
        },
        diagnostics,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleAssetManifest {
    pub schema_version: String,
    #[serde(default)]
    pub rules: Vec<RuleAssetManifestEntry>,
}

impl RuleAssetManifest {
    pub fn empty() -> Self {
        Self {
            schema_version: RULE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            rules: Vec::new(),
        }
    }

    pub fn from_assets<'a>(assets: impl IntoIterator<Item = &'a ProjectRuleAsset>) -> Self {
        Self {
            schema_version: RULE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            rules: assets
                .into_iter()
                .map(RuleAssetManifestEntry::from_asset)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleAssetManifestEntry {
    pub asset_id: String,
    pub rule_id: String,
    pub phase: String,
    pub enabled: bool,
    pub ir_hash: String,
    #[serde(default)]
    pub artifact_id: Option<String>,
    #[serde(default)]
    pub source_map_id: Option<String>,
}

impl RuleAssetManifestEntry {
    pub fn from_asset(asset: &ProjectRuleAsset) -> Self {
        Self {
            asset_id: asset.asset_id.clone(),
            rule_id: asset.rule_id.clone(),
            phase: asset.canonical_ir.phase.as_str().to_string(),
            enabled: asset.enabled && asset.canonical_ir.enabled,
            ir_hash: asset.ir_hash(),
            artifact_id: None,
            source_map_id: asset.source_map.feature_id.clone(),
        }
    }
}

pub fn runtime_rule_manifest_from_assets<'a>(
    assets: impl IntoIterator<Item = &'a ProjectRuleAsset>,
    module_artifact_id: impl Into<String>,
) -> RuntimeRuleManifest {
    let module_artifact_id = module_artifact_id.into();
    let rules = assets
        .into_iter()
        .map(runtime_rule_manifest_entry_from_asset)
        .collect::<Vec<_>>();
    let mut module_artifact_ids = rules
        .iter()
        .filter_map(|rule| rule.artifact_id.clone())
        .collect::<Vec<_>>();
    if module_artifact_ids.is_empty() && !module_artifact_id.is_empty() {
        module_artifact_ids.push(module_artifact_id);
    }
    RuntimeRuleManifest {
        schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
        mode: RUNTIME_RULE_MANIFEST_MODE.to_string(),
        rules,
        modules: module_artifact_ids
            .into_iter()
            .map(|artifact_id| RuntimeRuleModuleEntry {
                artifact_id,
                module_kind: RuntimeRuleModuleKind::StaticRegistry,
                path: None,
            })
            .collect(),
    }
}

pub fn runtime_rule_manifest_entry_from_asset(
    asset: &ProjectRuleAsset,
) -> RuntimeRuleManifestEntry {
    RuntimeRuleManifestEntry {
        rule_id: asset.rule_id.clone(),
        phase: runtime_rule_phase(asset.canonical_ir.phase),
        enabled: asset.enabled && asset.canonical_ir.enabled,
        executor: RuntimeRuleExecutor::RustAot,
        ir_source: Some(asset.asset_id.clone()),
        ir_hash: Some(asset.ir_hash()),
        artifact_id: Some(expected_rule_artifact_id(&asset.rule_id, &asset.ir_hash())),
        source_map: asset.source_map.feature_id.clone(),
    }
}

pub fn read_project_rule_asset_json(
    path: impl AsRef<Path>,
) -> Result<ProjectRuleAsset, ProjectRuleAssetIoError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|error| ProjectRuleAssetIoError {
        code: "read_rule_asset_failed",
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|error| ProjectRuleAssetIoError {
        code: "parse_rule_asset_failed",
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

pub fn write_project_rule_asset_json(
    path: impl AsRef<Path>,
    asset: &ProjectRuleAsset,
) -> Result<(), ProjectRuleAssetIoError> {
    let path = path.as_ref();
    let text = serde_json::to_string_pretty(asset).map_err(|error| ProjectRuleAssetIoError {
        code: "serialize_rule_asset_failed",
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    atomic_file_replace(path, text.as_bytes()).map_err(|error| ProjectRuleAssetIoError {
        code: "atomic_replace_rule_asset_failed",
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuleAssetIoError {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

fn runtime_rule_phase(phase: crate::rule_ir::ProjectRulePhase) -> RuntimeRulePhase {
    match phase {
        crate::rule_ir::ProjectRulePhase::FixedUpdate => RuntimeRulePhase::FixedUpdate,
        crate::rule_ir::ProjectRulePhase::Update => RuntimeRulePhase::Update,
        crate::rule_ir::ProjectRulePhase::PostPhysics => RuntimeRulePhase::PostPhysics,
        crate::rule_ir::ProjectRulePhase::EventHandler => RuntimeRulePhase::EventHandler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_ir::{ProjectRuleIr, ProjectRulePhase};

    #[test]
    fn project_rule_asset_roundtrips_json() {
        let ir = ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update);
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.fire",
            "Fire",
            ProjectRuleAssetSourceKind::AiDoc,
            ir,
        );
        asset.source_map.feature_id = Some("feature.fire".to_string());

        let json = serde_json::to_string_pretty(&asset).expect("asset should serialize");
        let decoded: ProjectRuleAsset =
            serde_json::from_str(&json).expect("asset should deserialize");

        assert_eq!(decoded.asset_id, "asset.rule.fire");
        assert_eq!(decoded.rule_id, "project.rule.fire");
        assert_eq!(
            decoded.source_map.feature_id.as_deref(),
            Some("feature.fire")
        );
    }

    #[test]
    fn project_rule_asset_validation_cache_does_not_change_ir_hash() {
        let ir = ProjectRuleIr::new("project.rule.same", ProjectRulePhase::Update);
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.same",
            "Same",
            ProjectRuleAssetSourceKind::UserAuthored,
            ir,
        );
        let before = asset.ir_hash();
        asset.validation = ProjectRuleAssetValidationCache {
            status: ProjectRuleAssetValidationStatus::Failed,
            diagnostics: vec![RuleIrDiagnostic {
                code: "CachedOnly".to_string(),
                message: "cache only".to_string(),
                path: None,
                suggestion: None,
            }],
        };

        assert_eq!(before, asset.ir_hash());
    }

    #[test]
    fn rule_asset_manifest_indexes_assets_without_business_logic() {
        let ir = ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update);
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.fire",
            "Fire",
            ProjectRuleAssetSourceKind::Template,
            ir,
        );
        asset.source_map.feature_id = Some("feature.fire".to_string());

        let manifest = RuleAssetManifest::from_assets([&asset]);

        assert_eq!(manifest.schema_version, RULE_ASSET_MANIFEST_SCHEMA_VERSION);
        assert_eq!(manifest.rules.len(), 1);
        assert_eq!(manifest.rules[0].asset_id, "asset.rule.fire");
        assert_eq!(
            manifest.rules[0].source_map_id.as_deref(),
            Some("feature.fire")
        );
    }

    #[test]
    fn project_rule_asset_builds_runtime_rule_manifest() {
        let ir = ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update);
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.fire",
            "Fire",
            ProjectRuleAssetSourceKind::AiDoc,
            ir,
        );
        asset.source_map.feature_id = Some("feature.fire".to_string());

        let manifest = runtime_rule_manifest_from_assets([&asset], "generated-rules");

        assert_eq!(
            manifest.schema_version,
            RUNTIME_RULE_MANIFEST_SCHEMA_VERSION
        );
        assert_eq!(manifest.mode, RUNTIME_RULE_MANIFEST_MODE);
        assert_eq!(
            manifest.modules[0].module_kind,
            RuntimeRuleModuleKind::StaticRegistry
        );
        assert_eq!(manifest.rules[0].rule_id, "project.rule.fire");
        assert_eq!(manifest.rules[0].executor, RuntimeRuleExecutor::RustAot);
        assert_eq!(
            manifest.rules[0].source_map.as_deref(),
            Some("feature.fire")
        );
    }

    #[test]
    fn project_rule_asset_reads_and_writes_json_file() {
        let dir = std::env::temp_dir().join(format!(
            "engine_runtime_rule_asset_test_{}",
            std::process::id()
        ));
        let path = dir.join("Rules").join("fire.rule.json");
        let ir = ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update);
        let asset = ProjectRuleAsset::new(
            "asset.rule.fire",
            "Fire",
            ProjectRuleAssetSourceKind::AiDoc,
            ir,
        );

        write_project_rule_asset_json(&path, &asset).expect("asset should write");
        let decoded = read_project_rule_asset_json(&path).expect("asset should read");

        assert_eq!(decoded.asset_id, "asset.rule.fire");
        let _ = std::fs::remove_dir_all(dir);
    }
}
