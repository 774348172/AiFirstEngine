use crate::logic_executor::RustAotRule;
use crate::project_logic::{ProjectLogicRunner, RuleCall, RuleExecutionPlan};
use crate::rule_artifact::validate_runtime_rule_manifest_artifacts;
use crate::runtime_package::{
    RuntimeRuleExecutor, RuntimeRuleManifest, RuntimeRuleManifestEntry, RuntimeRuleModuleKind,
    RUNTIME_RULE_MANIFEST_MODE,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct RegisteredRule {
    artifact_id: Option<String>,
    rule: RustAotRule,
}

#[derive(Clone, Default, Debug)]
pub struct RuleModuleRegistry {
    rules: BTreeMap<String, RegisteredRule>,
}

impl RuleModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_generated_rule(
        &mut self,
        rule_id: impl Into<String>,
        rule: RustAotRule,
    ) -> &mut Self {
        self.rules.insert(
            rule_id.into(),
            RegisteredRule {
                artifact_id: None,
                rule,
            },
        );
        self
    }

    pub fn register_generated_rule_artifact(
        &mut self,
        rule_id: impl Into<String>,
        artifact_id: impl Into<String>,
        rule: RustAotRule,
    ) -> &mut Self {
        self.rules.insert(
            rule_id.into(),
            RegisteredRule {
                artifact_id: Some(artifact_id.into()),
                rule,
            },
        );
        self
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn contains_rule(&self, rule_id: &str) -> bool {
        self.rules.contains_key(rule_id)
    }

    pub fn rule(&self, rule_id: &str) -> Option<RustAotRule> {
        self.rules.get(rule_id).map(|registered| registered.rule)
    }

    pub fn build_runner(
        &self,
        manifest: &RuntimeRuleManifest,
    ) -> Result<ProjectLogicRunner, RuleRegistryError> {
        self.build_runner_internal(manifest, false)
    }

    pub fn build_runner_strict(
        &self,
        manifest: &RuntimeRuleManifest,
    ) -> Result<ProjectLogicRunner, RuleRegistryError> {
        self.build_runner_internal(manifest, true)
    }

    fn build_runner_internal(
        &self,
        manifest: &RuntimeRuleManifest,
        require_artifact_identity: bool,
    ) -> Result<ProjectLogicRunner, RuleRegistryError> {
        let artifact_report = validate_runtime_rule_manifest_artifacts(None, manifest);
        if let Some(issue) = artifact_report.issues.first() {
            return Err(RuleRegistryError {
                code: issue.code,
                rule_id: String::new(),
                artifact_id: None,
                message: issue.message.clone(),
            });
        }
        for module in &manifest.modules {
            if module.module_kind != RuntimeRuleModuleKind::StaticRegistry {
                return Err(RuleRegistryError {
                    code: "unsupported_rule_module_kind",
                    rule_id: String::new(),
                    artifact_id: None,
                    message: "B-min only executes staticRegistry rule modules".to_string(),
                });
            }
        }
        let mut plan = RuleExecutionPlan::empty();
        for entry in &manifest.rules {
            if entry.executor != RuntimeRuleExecutor::RustAot {
                return Err(RuleRegistryError {
                    code: "unsupported_rule_executor",
                    rule_id: entry.rule_id.clone(),
                    artifact_id: entry.artifact_id.clone(),
                    message: "M2 v1 only supports rustAot rule execution".to_string(),
                });
            }
            if entry.enabled && !self.contains_rule(&entry.rule_id) {
                return Err(RuleRegistryError {
                    code: "missing_registered_rule",
                    rule_id: entry.rule_id.clone(),
                    artifact_id: entry.artifact_id.clone(),
                    message: "rule manifest references a rule that is not registered".to_string(),
                });
            }
            if entry.enabled && require_artifact_identity {
                let expected_artifact =
                    entry
                        .artifact_id
                        .as_deref()
                        .ok_or_else(|| RuleRegistryError {
                            code: "missing_rule_artifact_identity",
                            rule_id: entry.rule_id.clone(),
                            artifact_id: None,
                            message: "enabled rule manifest entry has no artifactId".to_string(),
                        })?;
                let registered_artifact = self
                    .rules
                    .get(&entry.rule_id)
                    .and_then(|registered| registered.artifact_id.as_deref())
                    .ok_or_else(|| RuleRegistryError {
                        code: "missing_registered_rule_artifact",
                        rule_id: entry.rule_id.clone(),
                        artifact_id: Some(expected_artifact.to_string()),
                        message: "linked rule registration has no artifact identity".to_string(),
                    })?;
                if registered_artifact != expected_artifact {
                    return Err(RuleRegistryError {
                        code: "registered_rule_artifact_mismatch",
                        rule_id: entry.rule_id.clone(),
                        artifact_id: Some(expected_artifact.to_string()),
                        message: format!(
                            "rule manifest artifact '{}' does not match linked artifact '{}'",
                            expected_artifact, registered_artifact
                        ),
                    });
                }
            }
            push_rule_call(&mut plan, entry);
        }
        let mut runner = ProjectLogicRunner::new(plan);
        for (rule_id, registered) in &self.rules {
            runner.register_rust_aot_rule(rule_id.clone(), registered.rule);
        }
        Ok(runner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRegistryError {
    pub code: &'static str,
    pub rule_id: String,
    pub artifact_id: Option<String>,
    pub message: String,
}

pub fn empty_rule_manifest() -> RuntimeRuleManifest {
    RuntimeRuleManifest {
        schema_version: crate::runtime_package::RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
        mode: RUNTIME_RULE_MANIFEST_MODE.to_string(),
        rules: Vec::new(),
        modules: Vec::new(),
    }
}

fn push_rule_call(plan: &mut RuleExecutionPlan, entry: &RuntimeRuleManifestEntry) {
    let mut call = RuleCall::rust_aot(entry.rule_id.clone());
    call.enabled = entry.enabled;
    match entry.phase {
        crate::runtime_package::RuntimeRulePhase::FixedUpdate => plan.fixed_update.push(call),
        crate::runtime_package::RuntimeRulePhase::Update => plan.frame_update.push(call),
        crate::runtime_package::RuntimeRulePhase::PostPhysics => plan.post_physics.push(call),
        crate::runtime_package::RuntimeRulePhase::EventHandler => plan.event_handler.push(call),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
    use crate::runtime_package::{RuntimeRuleExecutor, RuntimeRulePhase};

    fn noop_rule(_context: &mut LogicContext<'_>) -> LogicResult {
        LogicResult::applied("project.rule.noop", ExecutorKind::RustAot)
    }

    fn manifest_with_rule(rule_id: &str) -> RuntimeRuleManifest {
        let artifact_id = format!("rule-artifact:{}:hash", rule_id);
        RuntimeRuleManifest {
            schema_version: crate::runtime_package::RUNTIME_RULE_MANIFEST_SCHEMA_VERSION
                .to_string(),
            mode: RUNTIME_RULE_MANIFEST_MODE.to_string(),
            rules: vec![RuntimeRuleManifestEntry {
                rule_id: rule_id.to_string(),
                phase: RuntimeRulePhase::Update,
                enabled: true,
                executor: RuntimeRuleExecutor::RustAot,
                ir_source: Some("Rules/noop.rule.ir.json".to_string()),
                ir_hash: Some("hash".to_string()),
                artifact_id: Some(artifact_id.clone()),
                source_map: None,
            }],
            modules: vec![crate::runtime_package::RuntimeRuleModuleEntry {
                artifact_id,
                module_kind: RuntimeRuleModuleKind::StaticRegistry,
                path: None,
            }],
        }
    }

    #[test]
    fn rule_registry_builds_runner_from_manifest() {
        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule("project.rule.noop", noop_rule);

        let runner = registry
            .build_runner(&manifest_with_rule("project.rule.noop"))
            .expect("runner should build");

        assert_eq!(runner.plan().frame_update.len(), 1);
        assert_eq!(runner.plan().frame_update[0].rule_id, "project.rule.noop");
    }

    #[test]
    fn rule_registry_rejects_missing_rule() {
        let registry = RuleModuleRegistry::new();

        let error = registry
            .build_runner(&manifest_with_rule("project.rule.missing"))
            .expect_err("missing rule should fail");

        assert_eq!(error.code, "missing_registered_rule");
    }

    #[test]
    fn strict_rule_registry_requires_linked_artifact_identity() {
        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule("project.rule.noop", noop_rule);

        let error = registry
            .build_runner_strict(&manifest_with_rule("project.rule.noop"))
            .expect_err("strict binding must reject an untyped linked rule");

        assert_eq!(error.code, "missing_registered_rule_artifact");
        assert_eq!(error.rule_id, "project.rule.noop");
    }

    #[test]
    fn strict_rule_registry_rejects_artifact_mismatch() {
        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule_artifact(
            "project.rule.noop",
            "rule-artifact:project.rule.noop:other",
            noop_rule,
        );

        let error = registry
            .build_runner_strict(&manifest_with_rule("project.rule.noop"))
            .expect_err("strict binding must reject a mismatched linked artifact");

        assert_eq!(error.code, "registered_rule_artifact_mismatch");
        assert_eq!(error.rule_id, "project.rule.noop");
    }

    #[test]
    fn strict_rule_registry_accepts_exact_artifact_identity() {
        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule_artifact(
            "project.rule.noop",
            "rule-artifact:project.rule.noop:hash",
            noop_rule,
        );

        let runner = registry
            .build_runner_strict(&manifest_with_rule("project.rule.noop"))
            .expect("exact linked artifact should bind");

        assert_eq!(runner.plan().frame_update.len(), 1);
    }

    #[test]
    fn rule_registry_rejects_dynamic_validation_host_execution() {
        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule("project.rule.noop", noop_rule);
        let mut manifest = manifest_with_rule("project.rule.noop");
        manifest.modules[0].module_kind = RuntimeRuleModuleKind::DynamicValidationHost;
        manifest.modules[0].path = Some("rules/generated.dll".to_string());

        let error = registry
            .build_runner(&manifest)
            .expect_err("dynamic validation host should not execute in B-min");

        assert_eq!(error.code, "unsupported_rule_module_kind");
    }
}
