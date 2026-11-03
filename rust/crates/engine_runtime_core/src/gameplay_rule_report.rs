use crate::runtime_package::RuntimeRuleManifest;
use crate::runtime_trace::RuntimeTrace;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const GAMEPLAY_RULE_RUNTIME_EXECUTION_REPORT_SCHEMA_VERSION: &str =
    "gameplay-rule-runtime-execution-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayRuleRuntimeExecutionReport {
    pub schema_version: String,
    pub status: GameplayRuleRuntimeExecutionStatus,
    pub frames_simulated: u64,
    pub manifest_rule_count: usize,
    pub enabled_rule_count: usize,
    pub disabled_rule_count: usize,
    pub observed_rule_ids: Vec<String>,
    pub rule_event_count: usize,
    pub write_count: usize,
    pub command_enqueue_count: usize,
    pub command_apply_ok_count: usize,
    pub command_apply_failed_count: usize,
    pub command_apply_by_source: BTreeMap<String, usize>,
    pub collision_pair_count: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameplayRuleRuntimeExecutionStatus {
    Passed,
    Failed,
}

impl GameplayRuleRuntimeExecutionReport {
    pub fn from_traces(
        manifest: &RuntimeRuleManifest,
        frames_simulated: u64,
        traces: &[RuntimeTrace],
        diagnostics: Vec<String>,
    ) -> Self {
        let mut observed_rule_ids = BTreeSet::new();
        let mut rule_event_count = 0;
        let mut write_count = 0;
        let mut command_enqueue_count = 0;
        let mut command_apply_ok_count = 0;
        let mut command_apply_failed_count = 0;
        let mut command_apply_by_source = BTreeMap::<String, usize>::new();
        let mut collision_pair_count = 0;

        for trace in traces {
            for event in &trace.events {
                if let Some(rule_id) = event.system_id.strip_prefix("project.rule.") {
                    observed_rule_ids.insert(rule_id.to_string());
                    rule_event_count += 1;
                }
            }
            for record in &trace.gameplay_records {
                if record.rule_id != "engine.command_buffer" {
                    observed_rule_ids.insert(record.rule_id.clone());
                }
                match record.operation.as_str() {
                    "write" => write_count += 1,
                    "command_enqueue" => command_enqueue_count += 1,
                    "command_apply" if record.result == "ok" => {
                        command_apply_ok_count += 1;
                        if let Some(source) = &record.source {
                            *command_apply_by_source.entry(source.clone()).or_default() += 1;
                        }
                    }
                    "command_apply" => command_apply_failed_count += 1,
                    _ => {}
                }
            }
            collision_pair_count += trace
                .physics2d_records
                .iter()
                .filter(|record| record.operation == "build_collision_pairs")
                .filter_map(|record| record.pair_count)
                .sum::<usize>();
        }

        let status = if diagnostics.is_empty() && command_apply_failed_count == 0 {
            GameplayRuleRuntimeExecutionStatus::Passed
        } else {
            GameplayRuleRuntimeExecutionStatus::Failed
        };

        Self {
            schema_version: GAMEPLAY_RULE_RUNTIME_EXECUTION_REPORT_SCHEMA_VERSION.to_string(),
            status,
            frames_simulated,
            manifest_rule_count: manifest.rules.len(),
            enabled_rule_count: manifest.rules.iter().filter(|rule| rule.enabled).count(),
            disabled_rule_count: manifest.rules.iter().filter(|rule| !rule.enabled).count(),
            observed_rule_ids: observed_rule_ids.into_iter().collect(),
            rule_event_count,
            write_count,
            command_enqueue_count,
            command_apply_ok_count,
            command_apply_failed_count,
            command_apply_by_source,
            collision_pair_count,
            diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_command::GameplayCommandId;
    use crate::gameplay_trace::GameplayTraceRecord;
    use crate::ids::EntityId;
    use crate::runtime_package::{
        RuntimeRuleExecutor, RuntimeRuleManifestEntry, RuntimeRulePhase,
        RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
    };

    #[test]
    fn gameplay_rule_runtime_execution_report_summarizes_trace() {
        let manifest = RuntimeRuleManifest {
            schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
            mode: "rust-aot".to_string(),
            rules: vec![RuntimeRuleManifestEntry {
                rule_id: "rule.sample".to_string(),
                phase: RuntimeRulePhase::Update,
                enabled: true,
                executor: RuntimeRuleExecutor::RustAot,
                ir_source: None,
                ir_hash: None,
                artifact_id: None,
                source_map: None,
            }],
            modules: Vec::new(),
        };
        let mut trace = RuntimeTrace::new();
        trace.record(1, "project.rule.rule.sample", "Update", "applied", None);
        trace.gameplay_records.push(GameplayTraceRecord {
            frame_index: 1,
            phase: "Update".to_string(),
            rule_id: "engine.command_buffer".to_string(),
            operation: "command_apply".to_string(),
            entity_id: Some(EntityId::from("entity-created")),
            component_type: None,
            field_path: None,
            before: None,
            after: None,
            command_id: Some(GameplayCommandId(1)),
            source: Some("prefab-sample".to_string()),
            result: "ok".to_string(),
            error_code: None,
        });

        let report =
            GameplayRuleRuntimeExecutionReport::from_traces(&manifest, 1, &[trace], vec![]);

        assert_eq!(report.status, GameplayRuleRuntimeExecutionStatus::Passed);
        assert_eq!(report.manifest_rule_count, 1);
        assert_eq!(report.command_apply_ok_count, 1);
        assert_eq!(
            report.command_apply_by_source.get("prefab-sample"),
            Some(&1)
        );
    }
}
