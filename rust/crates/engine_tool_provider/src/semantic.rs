use super::*;
use project_authoring_execution::{
    PreparedPlaytestScenario, ProjectArtifactLineage, ProjectPlaytestReport,
};

#[derive(Debug, Clone)]
pub(super) struct RetainedDelivery {
    pub delivery: DeliveryRef,
    pub compiler: GameProjectCompiler,
    pub scenario: Option<PreparedPlaytestScenario>,
}

#[derive(Debug)]
pub(super) struct RetainedPlaytest {
    compiler: GameProjectCompiler,
    report: ProjectPlaytestReport,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaytestInput {
    delivery_ref: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObserveInput {
    run_ref: String,
}

pub(super) fn playtest_schema() -> Value {
    json!({"type":"object","additionalProperties":false,
        "properties":{"deliveryRef":{"type":"string","minLength":1,"maxLength":256}},"required":[]})
}

pub(super) fn observe_schema() -> Value {
    json!({"type":"object","additionalProperties":false,
        "properties":{"runRef":{"type":"string","minLength":1,"maxLength":256}},"required":["runRef"]})
}

fn valid_reference(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}

pub(super) fn validate_playtest_input(arguments: &Value) -> Result<bool, serde_json::Error> {
    serde_json::from_value::<PlaytestInput>(arguments.clone()).map(|input| {
        input.delivery_ref.as_deref().is_none_or(valid_reference)
            && arguments.get("deliveryRef").is_none_or(Value::is_string)
    })
}

pub(super) fn validate_observe_input(arguments: &Value) -> Result<bool, serde_json::Error> {
    serde_json::from_value::<ObserveInput>(arguments.clone())
        .map(|input| valid_reference(&input.run_ref))
}

impl EngineToolProvider {
    fn retained_delivery(&self, reference: &str) -> Result<&RetainedDelivery, ToolDiagnostic> {
        self.deliveries.get(reference).ok_or_else(|| {
            diagnostic(
                "engine_provider.delivery_ref_unknown",
                "Delivery is not retained by this Provider project session.",
                "Use a deliveryRef returned by this session; no live-source fallback is performed.",
            )
        })
    }

    fn retained_playtest(&self, reference: &str) -> Result<&RetainedPlaytest, ToolDiagnostic> {
        self.playtests.get(reference).ok_or_else(|| diagnostic(
            "engine_provider.run_ref_unknown", "Run is not retained by this Provider project session.",
            "Use the runRef from the required playtest; do not substitute paths or a latest run."))
    }

    // These calls authorize retained artifacts, not the possibly changed live project.
    pub(super) fn retained_playtest_lineage(
        &self,
        call: &HostToolCall,
    ) -> Result<Option<&ProjectArtifactLineage>, ToolDiagnostic> {
        match call.tool_name.as_str() {
            "engine_runtime_observe" => {
                let input: ObserveInput = serde_json::from_value(call.arguments.clone())
                    .map_err(|e| execution_input_diagnostic("observe", e))?;
                Ok(Some(
                    self.retained_playtest(&input.run_ref)?
                        .report
                        .delivery()
                        .lineage(),
                ))
            }
            "engine_runtime_playtest" => {
                let input: PlaytestInput = serde_json::from_value(call.arguments.clone())
                    .map_err(|e| execution_input_diagnostic("playtest", e))?;
                input
                    .delivery_ref
                    .as_deref()
                    .map(|reference| {
                        self.retained_delivery(reference)
                            .map(|retained| retained.delivery.lineage())
                    })
                    .transpose()
            }
            _ => Ok(None),
        }
    }

    pub(super) fn invoke_playtest(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let input: PlaytestInput = serde_json::from_value(call.arguments.clone())
            .map_err(|e| execution_input_diagnostic("playtest", e))?;
        let (compiler, report) = if let Some(reference) = input.delivery_ref {
            let retained = self.retained_delivery(&reference)?;
            let scenario = retained.scenario.as_ref().ok_or_else(|| diagnostic(
                "engine_provider.delivery_scenario_unavailable", "This delivery has no valid frozen playtest scenario.",
                "Correct the project scenario and create a new delivery; an old delivery cannot adopt live test input."))?;
            let output = self.playtest_output(operation_id)?;
            let report = retained
                .compiler
                .playtest_delivery(&retained.delivery, scenario, output)
                .map_err(compiler_diagnostic)?;
            (retained.compiler.clone(), report)
        } else {
            let refresh = required_refresh(refresh)?;
            let output = self.playtest_output(operation_id)?;
            let session = self.project_mut()?;
            let root = session.project_root().to_path_buf();
            let (lease, compiler) = compiler_for_operation(session, operation_id, refresh)?;
            let prepared = compiler
                .prepare_with_artifact_cache(
                    &lease,
                    TargetProfile::WindowsDev,
                    &root.join("Library/CompilerCache"),
                )
                .map_err(compiler_diagnostic)?;
            let report = compiler
                .playtest(
                    &prepared,
                    BuildRequest::for_project(
                        TargetProfile::WindowsDev,
                        &root,
                        engine_tool_output(operation_id, "Playtest")?,
                        default_project_runtime_player_build_root(),
                    )
                    // The requested semantic scenario verifies this exact delivery.
                    // A second headless preflight cannot execute GPU-only effects.
                    .with_player_verification(false),
                    output,
                )
                .map_err(compiler_diagnostic)?;
            (compiler, report)
        };
        let reference = format!(
            "engine-run:{}",
            sha256_prefixed(report.process().run_id.as_bytes())
        );
        let delivery_ref = opaque_delivery_ref(report.delivery());
        self.deliveries.insert(
            delivery_ref,
            RetainedDelivery {
                delivery: report.delivery().clone(),
                compiler: compiler.clone(),
                scenario: Some(report.scenario().clone()),
            },
        );
        let output = playtest_output(&reference, &report);
        let result = InvocationSuccess {
            revision: Some(report.delivery().lineage().revision_id().to_string()),
            output,
            receipt_ref: None,
            evidence_refs: vec![reference.clone()],
        };
        self.playtests
            .insert(reference, RetainedPlaytest { compiler, report });
        Ok(result)
    }

    fn playtest_output(&self, operation_id: &str) -> Result<PathBuf, ToolDiagnostic> {
        let root = self
            .project
            .as_ref()
            .ok_or_else(|| {
                diagnostic(
                    "engine_provider.project_binding_required",
                    "A project is required.",
                    "Bind the project.",
                )
            })?
            .project_root();
        let relative = engine_tool_output(operation_id, "SemanticEvidence")?;
        let output = root.join(relative.as_str());
        let parent = relative
            .as_path()
            .parent()
            .expect("generated output has parent");
        project_authoring_execution::ProjectWriteScope::open(root)
            .and_then(|scope| scope.ensure_directory(parent))
            .map_err(|e| {
                diagnostic(
                    "engine_provider.playtest_output_unavailable",
                    e.to_string(),
                    "Restore the project's writable Library directory.",
                )
            })?;
        Ok(output)
    }

    pub(super) fn invoke_playtest_observe(
        &self,
        call: &HostToolCall,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let input: ObserveInput = serde_json::from_value(call.arguments.clone())
            .map_err(|e| execution_input_diagnostic("observe", e))?;
        let retained = self.retained_playtest(&input.run_ref)?;
        // Re-read and validate the sealed evidence; never return cached success for a missing file.
        retained
            .compiler
            .observe_playtest(&retained.report)
            .map_err(compiler_diagnostic)?;
        Ok(InvocationSuccess {
            revision: Some(
                retained
                    .report
                    .delivery()
                    .lineage()
                    .revision_id()
                    .to_string(),
            ),
            output: playtest_output(&input.run_ref, &retained.report),
            receipt_ref: None,
            evidence_refs: vec![input.run_ref],
        })
    }
}

fn playtest_output(reference: &str, report: &ProjectPlaytestReport) -> Value {
    let process = report.process();
    json!({
        "runRef": reference, "runId": process.run_id,
        "projectIdentity": report.delivery().lineage().project_identity(),
        "deliveryRef": opaque_delivery_ref(report.delivery()),
        "deliveryIdentity": report.delivery().delivery_identity(),
        "artifactIdentity": report.delivery().artifact_identity(),
        "runtimePackageDigest": process.package_digest,
        "preparationIdentity": report.scenario().preparation_identity(),
        "scenarioId": report.scenario().scenario().scenario_id,
        "scenarioSourcePath": report.scenario().source_path(),
        "scenarioSourceDigest": report.scenario().source_digest(),
        "scenarioDigest": process.scenario_digest,
        "outcome": process.outcome, "overall": process.overall,
        "processId": process.process.process_id, "processExitCode": process.process.exit_code,
        "processExitReason": process.process.exit_reason, "ownership": process.process.ownership,
        "semantic": process.player.as_ref().and_then(|player| player.semantic_playtest.as_ref()),
        "diagnostics": process.diagnostics,
        "reportDigest": report.evidence().report_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_inputs_are_bounded_references_not_paths_or_runtime_commands() {
        assert!(validate_playtest_input(&json!({})).unwrap());
        assert!(validate_playtest_input(&json!({"deliveryRef":"engine-delivery:test"})).unwrap());
        for input in [
            json!({"deliveryRef":""}),
            json!({"deliveryRef":null}),
            json!({"deliveryRef":"x".repeat(257)}),
        ] {
            assert!(!validate_playtest_input(&input).unwrap());
        }
        for input in [
            json!({"path":"C:/secret"}),
            json!({"scenario":{}}),
            json!({"timeoutMs":0}),
        ] {
            assert!(validate_playtest_input(&input).is_err());
            assert!(validate_observe_input(&input).is_err());
        }
        assert!(validate_observe_input(&json!({"runRef":"engine-run:test"})).unwrap());
        assert!(!validate_observe_input(&json!({"runRef":" "})).unwrap());
        assert!(validate_observe_input(&json!({"runRef":"test","path":"C:/secret"})).is_err());
        let definitions = tool_definitions();
        let playtest = definitions
            .iter()
            .find(|d| d.name == "engine_runtime_playtest")
            .unwrap();
        let observe = definitions
            .iter()
            .find(|d| d.name == "engine_runtime_observe")
            .unwrap();
        assert_eq!(playtest.side_effect, ToolSideEffect::ProcessSpawn);
        assert_eq!(observe.required_capabilities, [ToolCapability::ReadProject]);
        assert!(!observe.supports_cancellation);
        assert!(!playtest.supports_cancellation);
        let mut source_error = diagnostic(
            "game_project_compiler.playtest.observation_path_unknown",
            "Unknown project path",
            "Correct the scenario",
        );
        source_error.source_location = Some(project_authoring_execution::SourceLocation {
            source_path: "Tests/default.json".into(),
            field_path: Some("/assertions/0/path".into()),
            line: None,
            column: None,
            generated: false,
        });
        let flow = derive_local_flow(
            "engine_runtime_playtest",
            CanonicalToolStatus::RejectedByEngine,
            &Value::Null,
            &[source_error],
            None,
        );
        assert_eq!(flow.retryability, ToolRetryability::RetryAfterCorrection);
        assert_eq!(
            flow.transitions[0].tool_name.as_deref(),
            Some("engine_runtime_playtest")
        );
        let invalid_evidence = diagnostic(
            "game_project_compiler.playtest_evidence_invalid",
            "Changed bytes",
            "Choose valid evidence",
        );
        assert_eq!(
            derive_local_flow(
                "engine_runtime_observe",
                CanonicalToolStatus::RejectedByEngine,
                &Value::Null,
                &[invalid_evidence],
                None
            )
            .retryability,
            ToolRetryability::NotRetryable
        );
    }
}
