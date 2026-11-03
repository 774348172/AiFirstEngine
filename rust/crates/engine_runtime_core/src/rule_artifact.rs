use crate::rule_compiler::{
    RuleCompileReport, RuleCompileStatus, ENGINE_RULE_ABI_VERSION, RULE_COMPILER_VERSION,
};
use crate::runtime_package::{
    RuntimeRuleManifest, RuntimeRuleModuleKind, RUNTIME_RULE_MANIFEST_MODE,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RULE_ARTIFACT_MANIFEST_SCHEMA_VERSION: &str = "rule-artifact-manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactManifest {
    pub schema_version: String,
    #[serde(default)]
    pub artifacts: Vec<RuleArtifactManifestEntry>,
}

impl RuleArtifactManifest {
    pub fn from_compile_reports(
        reports: &[RuleCompileReport],
        module_kind: RuntimeRuleModuleKind,
    ) -> Self {
        Self {
            schema_version: RULE_ARTIFACT_MANIFEST_SCHEMA_VERSION.to_string(),
            artifacts: reports
                .iter()
                .map(|report| RuleArtifactManifestEntry::from_compile_report(report, module_kind))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactManifestEntry {
    pub artifact_id: String,
    pub rule_id: String,
    pub ir_hash: String,
    pub abi_version: String,
    pub compiler_version: String,
    pub module_kind: RuntimeRuleModuleKind,
    #[serde(default)]
    pub generated_source_path: Option<String>,
    #[serde(default)]
    pub artifact_path: Option<String>,
    pub status: RuleArtifactStatus,
    #[serde(default)]
    pub diagnostics: Vec<RuleArtifactDiagnostic>,
}

impl RuleArtifactManifestEntry {
    pub fn from_compile_report(
        report: &RuleCompileReport,
        module_kind: RuntimeRuleModuleKind,
    ) -> Self {
        let artifact_id = report
            .artifact_id
            .clone()
            .unwrap_or_else(|| expected_rule_artifact_id(&report.rule_id, &report.ir_hash));
        Self {
            artifact_id,
            rule_id: report.rule_id.clone(),
            ir_hash: report.ir_hash.clone(),
            abi_version: ENGINE_RULE_ABI_VERSION.to_string(),
            compiler_version: RULE_COMPILER_VERSION.to_string(),
            module_kind,
            generated_source_path: report.generated_source_path.clone(),
            artifact_path: None,
            status: if report.status == RuleCompileStatus::Success {
                RuleArtifactStatus::Built
            } else {
                RuleArtifactStatus::Rejected
            },
            diagnostics: report
                .diagnostics
                .iter()
                .map(|diagnostic| RuleArtifactDiagnostic {
                    code: diagnostic.code.clone(),
                    message: diagnostic.message.clone(),
                    path: diagnostic.path.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuleArtifactStatus {
    Declared,
    Built,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuleModuleLifecycleState {
    Declared,
    Validated,
    Registered,
    Ready,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleArtifactValidationIssue {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleArtifactValidationReport {
    pub issues: Vec<RuleArtifactValidationIssue>,
}

impl RuleArtifactValidationReport {
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn error(
        &mut self,
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issues.push(RuleArtifactValidationIssue {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuleArtifactRegistry {
    artifacts: BTreeMap<String, RuleArtifactManifestEntry>,
}

impl RuleArtifactRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_manifest(
        manifest: &RuleArtifactManifest,
    ) -> Result<Self, RuleArtifactValidationReport> {
        let mut report = RuleArtifactValidationReport::default();
        let mut registry = Self::new();
        if manifest.schema_version != RULE_ARTIFACT_MANIFEST_SCHEMA_VERSION {
            report.error(
                "invalid_rule_artifact_manifest_schema",
                "ruleArtifacts.schemaVersion",
                format!(
                    "rule artifact manifest schemaVersion must be {}",
                    RULE_ARTIFACT_MANIFEST_SCHEMA_VERSION
                ),
            );
        }
        for (index, artifact) in manifest.artifacts.iter().enumerate() {
            let path = format!("ruleArtifacts.artifacts[{}]", index);
            validate_artifact_entry(artifact, &path, &mut report);
            if registry.artifacts.contains_key(&artifact.artifact_id) {
                report.error(
                    "duplicate_rule_artifact",
                    format!("{}.artifactId", path),
                    format!("duplicate rule artifact: {}", artifact.artifact_id),
                );
            } else {
                registry
                    .artifacts
                    .insert(artifact.artifact_id.clone(), artifact.clone());
            }
        }
        if report.is_ok() {
            Ok(registry)
        } else {
            Err(report)
        }
    }

    pub fn insert(&mut self, artifact: RuleArtifactManifestEntry) {
        self.artifacts
            .insert(artifact.artifact_id.clone(), artifact);
    }

    pub fn artifact(&self, artifact_id: &str) -> Option<&RuleArtifactManifestEntry> {
        self.artifacts.get(artifact_id)
    }

    pub fn validate_runtime_manifest(
        &self,
        manifest: &RuntimeRuleManifest,
    ) -> RuleArtifactValidationReport {
        validate_runtime_rule_manifest_artifacts(Some(self), manifest)
    }
}

pub fn expected_rule_artifact_id(rule_id: &str, ir_hash: &str) -> String {
    format!("rule-artifact:{}:{}", rule_id, ir_hash)
}

pub fn validate_runtime_rule_manifest_artifacts(
    registry: Option<&RuleArtifactRegistry>,
    manifest: &RuntimeRuleManifest,
) -> RuleArtifactValidationReport {
    let mut report = RuleArtifactValidationReport::default();
    if manifest.mode != RUNTIME_RULE_MANIFEST_MODE {
        return report;
    }

    let module_artifacts = manifest
        .modules
        .iter()
        .map(|module| module.artifact_id.as_str())
        .collect::<BTreeSet<_>>();

    for (index, module) in manifest.modules.iter().enumerate() {
        let path = format!("rules.modules[{}]", index);
        if module.artifact_id.trim().is_empty() {
            report.error(
                "missing_rule_module_artifact_id",
                format!("{}.artifactId", path),
                "rule module artifactId is required",
            );
        }
        match module.module_kind {
            RuntimeRuleModuleKind::StaticRegistry => {}
            RuntimeRuleModuleKind::DynamicValidationHost => {
                if module.path.as_deref().unwrap_or_default().trim().is_empty() {
                    report.error(
                        "dynamic_validation_host_requires_path",
                        format!("{}.path", path),
                        "dynamicValidationHost module requires path, but B-min does not execute it",
                    );
                }
            }
        }
        if let Some(registry) = registry {
            match registry.artifact(&module.artifact_id) {
                Some(artifact) => {
                    if artifact.module_kind != module.module_kind {
                        report.error(
                            "rule_module_kind_mismatch",
                            format!("{}.moduleKind", path),
                            format!(
                                "module kind {:?} differs from artifact manifest {:?}",
                                module.module_kind, artifact.module_kind
                            ),
                        );
                    }
                    if artifact.status == RuleArtifactStatus::Rejected {
                        report.error(
                            "rule_artifact_rejected",
                            format!("{}.artifactId", path),
                            format!("rule artifact {} is rejected", module.artifact_id),
                        );
                    }
                }
                None => report.error(
                    "missing_rule_artifact_manifest_entry",
                    format!("{}.artifactId", path),
                    format!(
                        "rule module artifact {} is missing from RuleArtifactRegistry",
                        module.artifact_id
                    ),
                ),
            }
        }
    }

    for (index, rule) in manifest.rules.iter().enumerate() {
        let path = format!("rules.rules[{}]", index);
        if !rule.enabled {
            continue;
        }
        let Some(ir_hash) = rule.ir_hash.as_deref() else {
            report.error(
                "missing_rule_ir_hash",
                format!("{}.irHash", path),
                "enabled rust-aot rule requires irHash",
            );
            continue;
        };
        let expected = expected_rule_artifact_id(&rule.rule_id, ir_hash);
        let Some(artifact_id) = rule.artifact_id.as_deref() else {
            report.error(
                "missing_rule_artifact_id",
                format!("{}.artifactId", path),
                "enabled rust-aot rule requires artifactId",
            );
            continue;
        };
        if artifact_id != expected {
            report.error(
                "rule_artifact_id_mismatch",
                format!("{}.artifactId", path),
                format!("artifactId must be {}", expected),
            );
        }
        if !module_artifacts.contains(artifact_id) {
            report.error(
                "missing_rule_module_for_artifact",
                format!("{}.artifactId", path),
                format!("no rule module declares artifact {}", artifact_id),
            );
        }
        if let Some(registry) = registry {
            match registry.artifact(artifact_id) {
                Some(artifact) => {
                    if artifact.rule_id != rule.rule_id {
                        report.error(
                            "rule_artifact_rule_id_mismatch",
                            format!("{}.artifactId", path),
                            format!(
                                "artifact belongs to {}, but rule entry is {}",
                                artifact.rule_id, rule.rule_id
                            ),
                        );
                    }
                    if artifact.ir_hash != ir_hash {
                        report.error(
                            "rule_artifact_ir_hash_mismatch",
                            format!("{}.irHash", path),
                            format!(
                                "artifact irHash {} differs from rule irHash {}",
                                artifact.ir_hash, ir_hash
                            ),
                        );
                    }
                    if artifact.abi_version != ENGINE_RULE_ABI_VERSION {
                        report.error(
                            "rule_artifact_abi_mismatch",
                            format!("{}.artifactId", path),
                            format!(
                                "artifact ABI {} differs from engine ABI {}",
                                artifact.abi_version, ENGINE_RULE_ABI_VERSION
                            ),
                        );
                    }
                }
                None => report.error(
                    "missing_rule_artifact_manifest_entry",
                    format!("{}.artifactId", path),
                    format!(
                        "rule artifact {} is missing from RuleArtifactRegistry",
                        artifact_id
                    ),
                ),
            }
        }
    }

    report
}

fn validate_artifact_entry(
    artifact: &RuleArtifactManifestEntry,
    path: &str,
    report: &mut RuleArtifactValidationReport,
) {
    if artifact.rule_id.trim().is_empty() {
        report.error(
            "missing_rule_artifact_rule_id",
            format!("{}.ruleId", path),
            "rule artifact ruleId is required",
        );
    }
    if artifact.ir_hash.trim().is_empty() {
        report.error(
            "missing_rule_artifact_ir_hash",
            format!("{}.irHash", path),
            "rule artifact irHash is required",
        );
    }
    let expected = expected_rule_artifact_id(&artifact.rule_id, &artifact.ir_hash);
    if artifact.artifact_id != expected {
        report.error(
            "rule_artifact_id_mismatch",
            format!("{}.artifactId", path),
            format!("artifactId must be {}", expected),
        );
    }
    if artifact.abi_version != ENGINE_RULE_ABI_VERSION {
        report.error(
            "rule_artifact_abi_mismatch",
            format!("{}.abiVersion", path),
            format!(
                "artifact ABI {} differs from engine ABI {}",
                artifact.abi_version, ENGINE_RULE_ABI_VERSION
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_compiler::{RuleCompileRequest, RuleCompileStatus, RuleCompiler};
    use crate::rule_ir::{ProjectRuleIr, ProjectRulePhase};
    use crate::runtime_package::{
        RuntimeRuleExecutor, RuntimeRuleManifestEntry, RuntimeRuleModuleEntry, RuntimeRulePhase,
        RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
    };

    fn manifest(rule_id: &str, ir_hash: &str) -> RuntimeRuleManifest {
        let artifact_id = expected_rule_artifact_id(rule_id, ir_hash);
        RuntimeRuleManifest {
            schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
            mode: RUNTIME_RULE_MANIFEST_MODE.to_string(),
            rules: vec![RuntimeRuleManifestEntry {
                rule_id: rule_id.to_string(),
                phase: RuntimeRulePhase::Update,
                enabled: true,
                executor: RuntimeRuleExecutor::RustAot,
                ir_source: Some(format!("Rules/{}.rule.ir.json", rule_id)),
                ir_hash: Some(ir_hash.to_string()),
                artifact_id: Some(artifact_id.clone()),
                source_map: None,
            }],
            modules: vec![RuntimeRuleModuleEntry {
                artifact_id,
                module_kind: RuntimeRuleModuleKind::StaticRegistry,
                path: None,
            }],
        }
    }

    #[test]
    fn rule_artifact_manifest_is_derived_from_compile_report() {
        let ir = ProjectRuleIr::new("project.rule.move", ProjectRulePhase::Update);
        let report = RuleCompiler::compile(
            &RuleCompileRequest::dev_desktop("target/generated-rules"),
            &ir,
            None,
        );

        assert_eq!(report.status, RuleCompileStatus::Success);
        let manifest = RuleArtifactManifest::from_compile_reports(
            &[report.clone()],
            RuntimeRuleModuleKind::StaticRegistry,
        );
        let artifact = &manifest.artifacts[0];

        assert_eq!(artifact.rule_id, report.rule_id);
        assert_eq!(artifact.ir_hash, report.ir_hash);
        assert_eq!(artifact.status, RuleArtifactStatus::Built);
        assert_eq!(
            artifact.artifact_id,
            expected_rule_artifact_id(&artifact.rule_id, &artifact.ir_hash)
        );
    }

    #[test]
    fn runtime_manifest_rejects_artifact_id_that_does_not_match_ir_hash() {
        let mut manifest = manifest("project.rule.move", "hash-a");
        manifest.rules[0].artifact_id = Some("rule-artifact:project.rule.move:hash-b".to_string());

        let report = validate_runtime_rule_manifest_artifacts(None, &manifest);

        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "rule_artifact_id_mismatch"));
    }

    #[test]
    fn runtime_manifest_rejects_rule_without_module_artifact() {
        let mut manifest = manifest("project.rule.move", "hash-a");
        manifest.modules.clear();

        let report = validate_runtime_rule_manifest_artifacts(None, &manifest);

        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "missing_rule_module_for_artifact"));
    }

    #[test]
    fn artifact_registry_validates_runtime_manifest() {
        let ir_hash = "hash-a";
        let artifact_id = expected_rule_artifact_id("project.rule.move", ir_hash);
        let artifact_manifest = RuleArtifactManifest {
            schema_version: RULE_ARTIFACT_MANIFEST_SCHEMA_VERSION.to_string(),
            artifacts: vec![RuleArtifactManifestEntry {
                artifact_id,
                rule_id: "project.rule.move".to_string(),
                ir_hash: ir_hash.to_string(),
                abi_version: ENGINE_RULE_ABI_VERSION.to_string(),
                compiler_version: RULE_COMPILER_VERSION.to_string(),
                module_kind: RuntimeRuleModuleKind::StaticRegistry,
                generated_source_path: None,
                artifact_path: None,
                status: RuleArtifactStatus::Built,
                diagnostics: Vec::new(),
            }],
        };
        let registry =
            RuleArtifactRegistry::from_manifest(&artifact_manifest).expect("registry should build");

        assert!(registry
            .validate_runtime_manifest(&manifest("project.rule.move", ir_hash))
            .is_ok());
    }
}
