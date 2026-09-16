use crate::semantic_outcome::{OutcomeStatus, PlaytestScenario, PlaytestTarget, SemanticOutcome};
use crate::{
    NativePlayerInputScript, NativePlayerInputScriptFrame, NativeWindowHostDiagnostic,
    NativeWindowHostReport,
};
use engine_runtime::engine_host_loop::EngineFrameOutput;
use engine_runtime::project_observation::{
    ProjectObservationContract, ProjectObservationValue, ProjectRuntimeObservationState,
};
use engine_runtime::runtime_package::RuntimePackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaytestAssertionResult {
    pub assertion_id: String,
    pub path: String,
    pub expected: ProjectObservationValue,
    pub actual: Option<ProjectObservationValue>,
    pub simulation_tick: Option<u64>,
    pub presentation_frame: Option<u64>,
    pub status: OutcomeStatus,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPlaytestReport {
    pub scenario_id: String,
    pub session_id: String,
    pub contract_digest: Option<String>,
    pub simulation_ticks: u64,
    pub fixed_delta_seconds: f32,
    pub injected_transition_count: usize,
    pub assertions: Vec<PlaytestAssertionResult>,
    pub captures: BTreeMap<String, OutcomeStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capture_evidence: Vec<PlaytestCaptureEvidence>,
    pub outcome: SemanticOutcome,
    pub overall: OutcomeStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaytestCaptureEvidence {
    pub capture_id: String,
    pub session_id: String,
    pub simulation_tick: u64,
    pub presentation_frame: u64,
    pub path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
}

pub(crate) struct SemanticPlaytest {
    scenario: PlaytestScenario,
    report: SemanticPlaytestReport,
    started: Instant,
    input_tick: u64,
    terminal: bool,
}

impl SemanticPlaytest {
    pub(crate) fn validate(
        scenario: &PlaytestScenario,
        package: &RuntimePackage,
    ) -> Result<(), NativeWindowHostDiagnostic> {
        let contract = package
            .manifest
            .observation_contract
            .as_ref()
            .map(|contract| ProjectObservationContract {
                schema_version: contract.schema_version.clone(),
                contract_id: contract.contract_id.clone(),
                observations: contract.observations.clone(),
            });
        scenario.validate(contract.as_ref()).map_err(|error| {
            NativeWindowHostDiagnostic::error(error.code, "playtest", error.message)
                .with_path(error.field_path)
        })?;
        if scenario.initial_scene_id != package.active_scene.id {
            return Err(NativeWindowHostDiagnostic::error("playtest.initial_scene_mismatch", "playtest", "The package active scene must match initialSceneId; do not silently select another scene."));
        }
        Ok(())
    }

    pub(crate) fn new(
        scenario: PlaytestScenario,
        package: &RuntimePackage,
        session_id: &str,
        started: Instant,
    ) -> Result<Self, NativeWindowHostDiagnostic> {
        Self::validate(&scenario, package)?;
        let assertions = scenario
            .assertions
            .iter()
            .map(|assertion| PlaytestAssertionResult {
                assertion_id: assertion.assertion_id.clone(),
                path: assertion.path.clone(),
                expected: assertion.equals.clone(),
                actual: None,
                simulation_tick: None,
                presentation_frame: None,
                status: OutcomeStatus::NotProducedYet,
                diagnostic: None,
            })
            .collect();
        let captures = scenario
            .captures
            .iter()
            .map(|capture| {
                (
                    capture.capture_id.clone(),
                    if scenario.target == PlaytestTarget::WindowsHeadless {
                        OutcomeStatus::Unsupported
                    } else {
                        OutcomeStatus::NotProducedYet
                    },
                )
            })
            .collect();
        Ok(Self {
            report: SemanticPlaytestReport {
                scenario_id: scenario.scenario_id.clone(),
                session_id: session_id.into(),
                contract_digest: package
                    .manifest
                    .observation_contract
                    .as_ref()
                    .map(|contract| contract.contract_digest.clone()),
                simulation_ticks: 0,
                fixed_delta_seconds: engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
                injected_transition_count: 0,
                assertions,
                captures,
                capture_evidence: Vec::new(),
                outcome: SemanticOutcome {
                    technical: OutcomeStatus::Passed,
                    gameplay: OutcomeStatus::NotProducedYet,
                    visual: OutcomeStatus::NotChecked,
                    delivery: OutcomeStatus::NotChecked,
                },
                overall: OutcomeStatus::NotProducedYet,
            },
            scenario,
            started,
            input_tick: 0,
            terminal: false,
        })
    }

    pub(crate) fn input_script(scenario: &PlaytestScenario) -> NativePlayerInputScript {
        NativePlayerInputScript::new(
            &scenario.scenario_id,
            scenario
                .inputs
                .iter()
                .map(|input| {
                    NativePlayerInputScriptFrame::keys(
                        input.simulation_tick,
                        input.key_down.clone(),
                        input.key_up.clone(),
                    )
                })
                .collect(),
        )
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn capture_at(&self, frame: u64) -> Option<String> {
        self.scenario
            .captures
            .iter()
            .find(|capture| capture.presentation_frame == frame)
            .map(|capture| capture.capture_id.clone())
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn record_capture(
        &mut self,
        capture_id: String,
        frame: u64,
        report: &NativeWindowHostReport,
    ) {
        let screenshot = &report.screenshot;
        let evidence = screenshot.path.as_ref().and_then(|path| {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .ok()?
                .take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            if bytes.len() > 32 * 1024 * 1024
                || screenshot.status != crate::NativeWindowScreenshotStatus::Captured
            {
                return None;
            }
            Some(PlaytestCaptureEvidence {
                capture_id: capture_id.clone(),
                session_id: self.report.session_id.clone(),
                simulation_tick: self.report.simulation_ticks,
                presentation_frame: frame,
                path: path.clone(),
                sha256: engine_runtime::canonical_digest::sha256_prefixed(&bytes),
                width: screenshot.width,
                height: screenshot.height,
                byte_size: bytes.len() as u64,
            })
        });
        self.report.captures.insert(
            capture_id,
            if evidence.is_some() {
                OutcomeStatus::Passed
            } else {
                OutcomeStatus::Failed
            },
        );
        if let Some(evidence) = evidence {
            self.report.capture_evidence.push(evidence);
        }
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn presentation_failed(&mut self, report: &mut NativeWindowHostReport) {
        self.fail(report, "playtest.presentation_failed", "Presentation failed or the controlled window closed; do not replay a simulation tick as a retry.", OutcomeStatus::Failed);
    }

    pub(crate) fn next_input_tick(&mut self) -> Option<u64> {
        let tick = self.report.simulation_ticks + 1;
        if self.terminal || tick > self.scenario.max_simulation_ticks || tick == self.input_tick {
            return None;
        }
        self.input_tick = tick;
        self.report.injected_transition_count += self
            .scenario
            .inputs
            .iter()
            .find(|input| input.simulation_tick == tick)
            .map_or(0, |input| input.key_down.len() + input.key_up.len());
        Some(tick)
    }

    pub(crate) fn should_stop(&mut self, host_report: &mut NativeWindowHostReport) -> bool {
        if self.started.elapsed() >= Duration::from_millis(self.scenario.timeout_ms)
            && !self.terminal
        {
            self.fail(
                host_report,
                "playtest.timeout",
                "Wall-clock deadline exceeded.",
                OutcomeStatus::Failed,
            );
        }
        self.terminal || self.report.simulation_ticks >= self.scenario.max_simulation_ticks
    }

    fn fail(
        &mut self,
        host_report: &mut NativeWindowHostReport,
        code: &str,
        message: &str,
        technical: OutcomeStatus,
    ) {
        self.report.outcome.technical = technical;
        self.terminal = true;
        host_report
            .diagnostics
            .push(NativeWindowHostDiagnostic::error(code, "playtest", message));
    }

    pub(crate) fn sample(
        &mut self,
        output: &EngineFrameOutput,
        host_report: &mut NativeWindowHostReport,
    ) {
        if self.terminal {
            return;
        }
        if output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|report| report.terminal_fault)
            || !output.runtime_advanced
        {
            self.fail(
                host_report,
                "playtest.runtime_fault",
                &format!(
                    "Runtime did not commit: {:?}",
                    output.project_runtime_session_report
                ),
                OutcomeStatus::Failed,
            );
            return;
        }
        let Some(time) = output.time_trace_summary else {
            self.fail(
                host_report,
                "playtest.time_missing",
                "No committed simulation time was returned.",
                OutcomeStatus::Failed,
            );
            return;
        };
        match next_committed_tick(self.report.simulation_ticks, time.fixed_frame_count) {
            Ok(false) => return,
            Err(message) => {
                self.fail(
                    host_report,
                    "playtest.tick_sequence_unsupported",
                    message,
                    OutcomeStatus::Unsupported,
                );
                return;
            }
            Ok(true) => {}
        }
        self.report.simulation_ticks = time.fixed_frame_count;
        let tick = self.report.simulation_ticks;
        let state = output.project_observation_state.as_ref();
        if let Some(state) = state {
            if state.session_id() != self.report.session_id
                || Some(state.contract_digest()) != self.report.contract_digest.as_deref()
            {
                self.fail(
                    host_report,
                    "playtest.observation_identity_mismatch",
                    "Observation session or contract differs from the bound package session.",
                    OutcomeStatus::Failed,
                );
                return;
            }
            if let ProjectRuntimeObservationState::ContractViolated { diagnostics, .. } = state {
                for result in &mut self.report.assertions {
                    result.status = OutcomeStatus::Failed;
                    result.diagnostic = Some("playtest.observation_contract_violated".into());
                }
                self.fail(
                    host_report,
                    "playtest.observation_contract_violated",
                    &format!("{diagnostics:?}"),
                    OutcomeStatus::Failed,
                );
                return;
            }
        }
        // Host output may retain an earlier snapshot on a zero-step frame. Never relabel it as a new sample.
        let current_state = state.filter(|state| state.runtime_frame() == Some(output.frame_index));
        for (assertion, result) in self
            .scenario
            .assertions
            .iter()
            .zip(&mut self.report.assertions)
        {
            if result.status == OutcomeStatus::Passed
                || tick < assertion.from_simulation_tick
                || tick > assertion.through_simulation_tick
            {
                continue;
            }
            let actual = current_state.and_then(|state| state.actual_value(&assertion.path));
            result.actual = actual.cloned();
            result.simulation_tick = Some(tick);
            result.presentation_frame = Some(output.frame_index);
            result.status = match actual {
                Some(value) if assertion.matches_sample(tick, value) => OutcomeStatus::Passed,
                Some(_) if tick == assertion.through_simulation_tick => OutcomeStatus::Failed,
                _ => OutcomeStatus::NotProducedYet,
            };
            result.diagnostic = match result.status {
                OutcomeStatus::Passed => None,
                OutcomeStatus::Failed => Some("playtest.condition_not_met".into()),
                _ => Some(
                    if actual.is_none() {
                        "playtest.observation_not_produced"
                    } else {
                        "playtest.condition_pending"
                    }
                    .into(),
                ),
            };
        }
    }

    pub(crate) fn finish(mut self, host_report: &mut NativeWindowHostReport) {
        if host_report.exit_code != 0
            && self.report.outcome.technical == OutcomeStatus::Passed
            && !self.terminal
        {
            self.report.outcome.technical = OutcomeStatus::Failed;
        }
        let statuses = self
            .report
            .assertions
            .iter()
            .map(|result| (result.assertion_id.clone(), result.status))
            .collect();
        self.report.outcome = self.scenario.summarize(
            self.report.outcome.technical,
            &statuses,
            &self.report.captures,
            OutcomeStatus::NotChecked,
        );
        self.report.overall = self.report.outcome.overall(&self.scenario, false);
        if self.report.overall != OutcomeStatus::Passed {
            host_report.exit_code = 1;
        }
        host_report.semantic_playtest = Some(self.report);
    }
}

fn next_committed_tick(previous: u64, actual: u64) -> Result<bool, &'static str> {
    if actual == previous {
        Ok(false)
    } else if actual == previous + 1 {
        Ok(true)
    } else {
        Err("Controlled playtest requires one fixed tick per advance; skipped or reversed ticks cannot prove intermediate assertions.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use engine_runtime::aui::*;
    use engine_runtime::ids::EntityId;
    use engine_runtime::logic_executor::{ExecutorKind, LogicResult};
    use engine_runtime::project_observation::{
        ProjectObservationEntry, ProjectObservationType,
        PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION,
    };
    use engine_runtime::project_runtime_module::*;
    use engine_runtime::project_runtime_session::*;
    use project_game_sdk as sdk;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TestModule(ProjectRuntimeModuleDescriptor, u8);
    struct TestSession(u8);
    struct EmptyUi;
    impl ProjectRuntimeModule for TestModule {
        fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
            &self.0
        }
        fn install(
            &self,
            registration: &mut ProjectRuntimeRegistration,
        ) -> Result<(), ProjectRuntimeError> {
            let fault = self.1;
            registration.register_rust_aot_rule(
                "rule.press",
                "rule-artifact:rule.press:test.press",
                move |context| {
                    if context.frame_index == 1 && fault == 4 {
                        return LogicResult::failed(
                            "rule.press",
                            ExecutorKind::RustAot,
                            "test.rule.failure",
                            "intentional first-frame rule failure",
                        );
                    }
                    if context.frame_index == 1 && fault == 5 {
                        context.request_despawn_entity(EntityId::from("already-despawned"));
                        return LogicResult::applied("rule.press", ExecutorKind::RustAot);
                    }
                    if !context.action_pressed("action.fire") {
                        return LogicResult::skipped("rule.press", ExecutorKind::RustAot);
                    }
                    let id = EntityId::from("entity-main");
                    let mut position = context.read_transform_local_position(&id).unwrap();
                    position.x += 1.0;
                    let write = context
                        .write_transform_local_position(id, position)
                        .unwrap();
                    let mut result = LogicResult::applied("rule.press", ExecutorKind::RustAot);
                    result.writes.push(write);
                    result
                },
            )?;
            let invalid = self.1;
            registration
                .set_runtime_session_factory(move |_| Ok(Box::new(TestSession(invalid))))?;
            registration.set_ui_state_producer_factory(|| Box::new(EmptyUi))
        }
    }
    impl ProjectUiStateSnapshotProducer for EmptyUi {
        fn producer_id(&self) -> &str {
            "test.empty.ui"
        }
        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::EmptyDefaultSnapshot,
                ProjectUiStateSnapshot::new(context.frame_index),
            )
        }
    }
    struct ObservationSession {
        presses: i64,
        commits: i64,
    }
    impl sdk::ProjectGameSession for ObservationSession {
        fn session_id(&self) -> &str {
            "test.playtest.session"
        }
    }
    struct UnusedWorld;
    impl sdk::WorldRead for UnusedWorld {
        fn query(&self, _: &[String], _: &[String]) -> sdk::GameResult<Vec<sdk::EntityHandle>> {
            Ok(vec![])
        }
        fn read_component(&self, _: &sdk::EntityHandle, _: &str) -> sdk::GameResult<sdk::Value> {
            unreachable!("fixture observation uses already-read committed values")
        }
    }
    fn definition() -> sdk::ProjectGameDefinition<ObservationSession> {
        sdk::ProjectGameDefinition::new(|_| {
            Ok(ObservationSession {
                presses: 0,
                commits: 0,
            })
        })
        .observe(|session, _, _| {
            let mut writer = sdk::ObservationWriter::default();
            writer.publish("game.presses", sdk::Value::Integer(session.presses));
            writer.publish("game.commits", sdk::Value::Integer(session.commits));
            Ok(sdk::ObservationOutput {
                values: writer.into_inner(),
            })
        })
    }
    impl ProjectRuntimeSession for TestSession {
        fn session_id(&self) -> &str {
            "test.playtest.session"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn fixed_update(
            &mut self,
            context: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            assert_ne!(self.0, 2, "fixture fixed-update panic");
            let id = EntityId::from("entity-main");
            let mut transform = context.world.read_transform(&id).unwrap();
            transform.local_position.y += 1.0;
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            mutations.write_transform(id, transform);
            ProjectRuntimeSessionOutput::applied(mutations)
        }
        fn observe(
            &self,
            context: ProjectRuntimeObservationContext<'_>,
        ) -> ProjectRuntimeObservationOutput {
            assert_ne!(self.0, 3, "fixture observation panic");
            let transform = context
                .world
                .read_transform(&EntityId::from("entity-main"))
                .unwrap();
            let mut state = ObservationSession {
                presses: transform.local_position.x as i64,
                commits: transform.local_position.y as i64,
            };
            let registered = definition();
            let result = registered.observation_handler().unwrap()(
                &mut state,
                &mut sdk::GameCallContext::new(&UnusedWorld),
                &sdk::FrameRequest {
                    frame_index: context.frame_index,
                    time: sdk::GameTime::default(),
                },
            )
            .unwrap();
            let mut output = ProjectRuntimeObservationOutput::empty();
            for (path, value) in result.values {
                let sdk::Value::Integer(value) = value else {
                    unreachable!()
                };
                output.insert(path, ProjectObservationValue::Integer(value));
            }
            if self.0 == 1 {
                output.insert("game.presses", ProjectObservationValue::Bool(true));
            }
            output
        }
    }

    struct Fixture {
        root: PathBuf,
        package: PathBuf,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn fixture() -> Fixture {
        let root = crate::tests::temp_root("semantic-playtest");
        let package = crate::tests::write_minimal_runtime_package(&root, "runtime-package");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(package.join("manifest.json")).unwrap()).unwrap();
        manifest["project"]["runtimeModule"] = serde_json::json!({"moduleId":"test.playtest", "interfaceVersion":"project-runtime-module.v2", "aotContentDigest":"sha256:test-playtest"});
        manifest["rules"]["mode"] = "rust-aot".into();
        let contract = ProjectObservationContract {
            schema_version: PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION.into(),
            contract_id: "test.observations".into(),
            observations: ["game.presses", "game.commits"]
                .into_iter()
                .map(|path| ProjectObservationEntry {
                    path: path.into(),
                    value_type: ProjectObservationType::Integer,
                    description: path.into(),
                    allowed_values: None,
                })
                .collect(),
        };
        manifest["observationContract"] = serde_json::to_value(contract.cook().unwrap()).unwrap();
        fs::write(
            package.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(package.join("rules/rule-manifest.json"), serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"runtime-rule-manifest.v1", "mode":"rust-aot",
            "rules":[{"ruleId":"rule.press","phase":"Update","enabled":true,"executor":"rustAot","artifactId":"rule-artifact:rule.press:test.press","irHash":"test.press","irSource":"fixture"}],
            "modules":[{"artifactId":"rule-artifact:rule.press:test.press","moduleKind":"staticRegistry","path":"fixture"}]
        })).unwrap()).unwrap();
        Fixture { root, package }
    }
    fn scenario() -> PlaytestScenario {
        serde_json::from_value(serde_json::json!({
            "schemaVersion":"playtest-scenario.v1", "scenarioId":"test.press", "initialSceneId":"scene-main", "target":"windows-headless",
            "maxSimulationTicks":4, "maxPresentationFrames":8, "timeoutMs":5000,
            "inputs":[{"simulationTick":1,"keyDown":["Space"]},{"simulationTick":3,"keyUp":["Space"]}],
            "assertions":[
                {"assertionId":"one-press","fromSimulationTick":1,"throughSimulationTick":4,"path":"game.presses","equals":1},
                {"assertionId":"held-then-released","fromSimulationTick":4,"throughSimulationTick":4,"path":"game.presses","equals":2},
                {"assertionId":"post-commit","fromSimulationTick":4,"throughSimulationTick":4,"path":"game.commits","equals":4}
            ]
        })).unwrap()
    }
    fn run(fixture: &Fixture, scenario: PlaytestScenario, invalid: bool) -> NativeWindowHostReport {
        run_with_fault(fixture, scenario, u8::from(invalid))
    }
    fn run_with_fault(
        fixture: &Fixture,
        scenario: PlaytestScenario,
        fault: u8,
    ) -> NativeWindowHostReport {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule(
            ProjectRuntimeModuleDescriptor::new("test.playtest", "sha256:test-playtest"),
            fault,
        )))
        .unwrap();
        run_headless_semantic_playtest_with_linked_modules(
            NativePlayerWindowRunRequest::headless_surface_gate(&fixture.package),
            &linked,
            scenario,
        )
    }

    #[test]
    fn semantic_playtest_actual_input_post_commit_repeatability_and_negative_expectation() {
        let fixture = fixture();
        let first = run(&fixture, scenario(), false);
        assert_eq!(first.exit_code, 0, "{first:#?}");
        let semantic = first.semantic_playtest.as_ref().unwrap();
        assert_eq!(semantic.simulation_ticks, 4);
        assert_eq!(semantic.injected_transition_count, 2);
        assert_eq!(first.input.pressed_key_count, 0);
        assert_eq!(semantic.assertions[0].simulation_tick, Some(1));
        assert_eq!(
            semantic.assertions[2].actual,
            Some(ProjectObservationValue::Integer(4))
        );
        assert_eq!(
            run(&fixture, scenario(), false).semantic_playtest,
            first.semantic_playtest
        );
        let mut without_input = scenario();
        without_input.inputs.clear();
        let failed = run(&fixture, without_input, false);
        assert_ne!(failed.exit_code, 0);
        let outcome = failed.semantic_playtest.unwrap();
        assert_eq!(outcome.outcome.technical, OutcomeStatus::Passed);
        assert_eq!(outcome.outcome.gameplay, OutcomeStatus::Failed);
        assert_eq!(
            outcome.assertions[0].actual,
            Some(ProjectObservationValue::Integer(0))
        );
        let mut wrong = scenario();
        wrong.assertions[0].equals = ProjectObservationValue::Integer(3);
        assert_eq!(
            run(&fixture, wrong, false)
                .semantic_playtest
                .unwrap()
                .outcome
                .gameplay,
            OutcomeStatus::Failed
        );
        let mut interval = scenario();
        interval.assertions[0].path = "game.commits".into();
        interval.assertions[0].equals = ProjectObservationValue::Integer(3);
        assert_eq!(
            run(&fixture, interval, false)
                .semantic_playtest
                .unwrap()
                .assertions[0]
                .simulation_tick,
            Some(3)
        );
    }

    #[test]
    fn native_loop_preserves_rule_and_command_failures_through_later_successful_frames() {
        use engine_runtime::windowed_player::WindowedPlayerRuntimeReportLevel;
        let fixture = fixture();
        for (fault, code) in [(4, "test.rule.failure"), (5, "world.entity.missing")] {
            for level in [
                WindowedPlayerRuntimeReportLevel::Summary,
                WindowedPlayerRuntimeReportLevel::Off,
            ] {
                let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule(
                    ProjectRuntimeModuleDescriptor::new("test.playtest", "sha256:test-playtest"),
                    fault,
                )))
                .unwrap();
                let mut request =
                    NativePlayerWindowRunRequest::headless_surface_gate(&fixture.package)
                        .with_runtime_report_level(level);
                request.frame_limit = 3;
                let report = crate::run_headless_native_player_from_package_with_linked_modules(
                    request, &linked,
                );
                assert_eq!(report.frames_completed, 3);
                assert_eq!(report.logic_status, "error");
                assert_eq!(report.exit_code, 1);
                assert_eq!(
                    report.present_status,
                    crate::NativeWindowPresentStatus::Presented
                );
                if level == WindowedPlayerRuntimeReportLevel::Summary {
                    let summary = report.gameplay_trace_summary.as_ref().unwrap();
                    assert_eq!(summary.failed_record_count, 1, "{report:#?}");
                    assert_eq!(summary.failure_details[0].error_code.as_deref(), Some(code));
                    assert_eq!(summary.failure_details[0].frame_index, 1);
                } else {
                    assert!(report.gameplay_trace_summary.is_none());
                }
                assert!(report.gameplay_trace_records.is_empty());
            }
        }
    }

    #[test]
    fn semantic_playtest_contract_violation_limits_and_capture_never_fake_success() {
        let fixture = fixture();
        let invalid = run(&fixture, scenario(), true);
        assert_ne!(invalid.exit_code, 0);
        assert!(invalid
            .diagnostics
            .iter()
            .any(|d| d.code == "playtest.observation_contract_violated"
                && d.message.contains("value_type_mismatch")));
        let mut limited = scenario();
        limited.max_presentation_frames = 2;
        let report = run(&fixture, limited, false);
        assert_eq!(report.frames_completed, 2);
        assert_eq!(
            report.semantic_playtest.unwrap().outcome.gameplay,
            OutcomeStatus::NotProducedYet
        );
        let mut captured = scenario();
        captured
            .captures
            .push(crate::semantic_outcome::PlaytestCapture {
                capture_id: "view".into(),
                presentation_frame: 1,
                required: true,
                subjective_review: false,
            });
        let report = run(&fixture, captured, false);
        assert_eq!(
            report.semantic_playtest.unwrap().outcome.visual,
            OutcomeStatus::Unsupported
        );
        assert_ne!(report.exit_code, 0);
    }

    #[test]
    fn semantic_playtest_runtime_fault_and_observation_panic_preserve_failure() {
        let fixture = fixture();
        for (fault, expected) in [
            (2, "playtest.runtime_fault"),
            (3, "playtest.observation_contract_violated"),
        ] {
            let report = run_with_fault(&fixture, scenario(), fault);
            assert_ne!(report.exit_code, 0);
            assert!(
                report.diagnostics.iter().any(|d| d.code == expected),
                "{:?}",
                report.diagnostics
            );
            assert_eq!(
                report.semantic_playtest.unwrap().outcome.technical,
                OutcomeStatus::Failed
            );
        }
    }

    #[test]
    fn semantic_playtest_zero_multi_step_input_dedup_and_deadline() {
        let fixture = fixture();
        let package = load_runtime_package(&fixture.package).value.unwrap();
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule(
            ProjectRuntimeModuleDescriptor::new("test.playtest", "sha256:test-playtest"),
            0,
        )))
        .unwrap();
        let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
            .unwrap()
            .into_parts();
        let mut host = NativePlayerRuntimeComposition::from_bound("scene-main", parts).host;
        let request = NativePlayerWindowRunRequest::headless_surface_gate(&fixture.package);
        let mut report = NativeWindowHostReport::base(&request);
        let (mut world, mut hydrator) =
            hydrate_active_scene_for_player(&package, &mut report).unwrap();
        let mut playtest = SemanticPlaytest::new(
            scenario(),
            &package,
            "test.playtest.session",
            Instant::now(),
        )
        .unwrap();
        assert_eq!(playtest.next_input_tick(), Some(1));
        assert_eq!(playtest.next_input_tick(), None);
        let zero = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(0),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: hydrator.instance_loader_mut(),
            },
        );
        playtest.sample(&zero, &mut report);
        assert_eq!(playtest.report.simulation_ticks, 0);
        assert!(playtest
            .report
            .assertions
            .iter()
            .all(|r| r.actual.is_none()));
        let committed = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: hydrator.instance_loader_mut(),
            },
        );
        let mut checkpoint = scenario();
        checkpoint.assertions = vec![crate::semantic_outcome::PlaytestAssertion {
            assertion_id: "commit-one".into(),
            from_simulation_tick: 1,
            through_simulation_tick: 1,
            path: "game.commits".into(),
            equals: ProjectObservationValue::Integer(1),
        }];
        for variant in 0..5 {
            let mut output = committed.clone();
            if variant == 4 {
                output.project_observation_state = None;
            }
            if let Some(ProjectRuntimeObservationState::Published { snapshot }) =
                &mut output.project_observation_state
            {
                match variant {
                    1 => snapshot.runtime_frame -= 1,
                    2 => snapshot.session_id = "stale-session".into(),
                    3 => snapshot.contract_digest = "stale-contract".into(),
                    _ => {}
                }
            }
            let mut evaluator = SemanticPlaytest::new(
                checkpoint.clone(),
                &package,
                "test.playtest.session",
                Instant::now(),
            )
            .unwrap();
            evaluator.sample(&output, &mut report);
            assert_eq!(
                evaluator.report.assertions[0].status == OutcomeStatus::Passed,
                variant == 0
            );
            if variant == 2 || variant == 3 {
                assert_eq!(evaluator.report.outcome.technical, OutcomeStatus::Failed);
            }
            if variant == 1 || variant == 4 {
                assert_eq!(
                    evaluator.report.assertions[0].status,
                    OutcomeStatus::NotProducedYet
                );
            }
        }
        let multiple = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(2),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: hydrator.instance_loader_mut(),
            },
        );
        playtest.sample(&multiple, &mut report);
        assert_eq!(
            playtest.report.outcome.technical,
            OutcomeStatus::Unsupported
        );
        assert_eq!(playtest.next_input_tick(), None);
        let mut expired = SemanticPlaytest::new(
            scenario(),
            &package,
            "test.playtest.session",
            Instant::now() - Duration::from_secs(6),
        )
        .unwrap();
        assert!(expired.should_stop(&mut report));
        assert_eq!(expired.report.outcome.technical, OutcomeStatus::Failed);
        assert_eq!(expired.next_input_tick(), None);
    }

    #[test]
    fn semantic_playtest_rejects_repeated_or_skipped_tick_evidence() {
        assert!(
            !next_committed_tick(1, 1).unwrap(),
            "Zero-step frame must not repeat a committed sample"
        );
        assert!(
            next_committed_tick(1, 3).is_err(),
            "Last sample of a multi-step frame cannot prove tick 2"
        );
        assert!(next_committed_tick(2, 1).is_err());
        assert!(next_committed_tick(1, 2).unwrap());
    }

    #[cfg(all(feature = "real-window", target_os = "windows"))]
    #[test]
    #[ignore = "Local Windows GPU capture with linked project runtime; explicit construction authorization required"]
    fn semantic_outcome_window_capture() {
        let fixture = fixture();
        let scene_path = fixture.package.join("scenes/scene-main.json");
        let mut scene: serde_json::Value =
            serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
        scene["entities"][0]["spriteRenderer2D"] = serde_json::json!({"spriteRef":{"id":"tex-test","type":"texture"},"color":[0.9,0.15,0.1,1.0],"visible":true});
        let assets_path = fixture.package.join("assets/asset-manifest.json");
        let mut assets: serde_json::Value =
            serde_json::from_slice(&fs::read(&assets_path).unwrap()).unwrap();
        assets["assets"].as_array_mut().unwrap().push(serde_json::json!({"id":"tex-test","name":"Test sprite","type":"texture","source":"assets/test.texture.json","state":"available","bundleId":"startup"}));
        assets["runtimeAssetIndex"].as_array_mut().unwrap().push(serde_json::json!({"assetGuid":"tex-test","assetId":"tex-test","assetType":"texture","version":"1","cookedAssetId":"cooked-tex-test","bundleId":"startup","loaderKind":"texture","dependencies":[],"flags":["test"]}));
        assets["cookedAssetTable"].as_array_mut().unwrap().push(serde_json::json!({"cookedAssetId":"cooked-tex-test","bundleId":"startup","path":"assets/test.texture.json","compression":"none"}));
        fs::write(&assets_path, serde_json::to_vec(&assets).unwrap()).unwrap();
        let metadata = engine_runtime::runtime_package::CookedTextureAsset {
            schema_version: engine_runtime::runtime_package::COOKED_TEXTURE_SCHEMA_VERSION.into(),
            asset_id: "tex-test".into(),
            cooked_asset_id: "cooked-tex-test".into(),
            source_hash: "test-texture".into(),
            width: 1,
            height: 1,
            format: "rgba8UnormSrgb".into(),
            color_space: "srgb".into(),
            mip_count: 1,
            byte_length: 4,
            pixel_data_path: "assets/test.rgba8".into(),
            sampler: "linearClamp".into(),
        };
        fs::write(
            fixture.package.join("assets/test.texture.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        fs::write(
            fixture.package.join("assets/test.rgba8"),
            [255, 255, 255, 255],
        )
        .unwrap();
        fs::write(&scene_path, serde_json::to_vec(&scene).unwrap()).unwrap();
        let mut scenario = scenario();
        scenario.target = PlaytestTarget::WindowsWindowed;
        scenario.timeout_ms = 15_000;
        scenario.captures = [1, 4]
            .into_iter()
            .map(|frame| crate::semantic_outcome::PlaytestCapture {
                capture_id: format!("frame-{frame}"),
                presentation_frame: frame,
                required: true,
                subjective_review: false,
            })
            .collect();
        let output = std::env::var_os("AIFE_SEMANTIC_CAPTURE_EVIDENCE")
            .map(PathBuf::from)
            .unwrap_or_else(|| fixture.root.join("captures"));
        fs::create_dir_all(&output).unwrap();
        let linked = Arc::new(
            LinkedProjectRuntimeSet::singleton(Arc::new(TestModule(
                ProjectRuntimeModuleDescriptor::new("test.playtest", "sha256:test-playtest"),
                0,
            )))
            .unwrap(),
        );
        let mut request = NativePlayerWindowRunRequest::windowed(&fixture.package);
        request.config.width = 640;
        request.config.height = 480;
        request.performance_sample_frames = 4;
        let report = crate::run_windowed_semantic_playtest_with_linked_modules(
            request,
            linked,
            scenario,
            output.clone(),
        );
        fs::write(
            output.join("player.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.exit_code, 0, "{report:#?}");
        assert_eq!(
            report
                .frame_performance_summary
                .as_ref()
                .unwrap()
                .observed_sample_frames,
            4
        );
        let semantic = report.semantic_playtest.as_ref().unwrap();
        assert_eq!(semantic.capture_evidence.len(), 2);
        assert_eq!(semantic.outcome.gameplay, OutcomeStatus::Passed);
        assert_eq!(semantic.outcome.visual, OutcomeStatus::Passed);
        let mut images = Vec::new();
        let mut centers = Vec::new();
        for capture in &semantic.capture_evidence {
            let mut reader = png::Decoder::new(std::io::BufReader::new(
                fs::File::open(&capture.path).unwrap(),
            ))
            .read_info()
            .unwrap();
            let mut buffer = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut buffer).unwrap();
            assert_eq!((info.width, info.height), (capture.width, capture.height));
            assert!(info.width > 0 && info.height > 0);
            buffer.truncate(info.buffer_size());
            let red = buffer
                .chunks_exact(4)
                .filter(|pixel| pixel[0] > 150 && pixel[1] < 120 && pixel[2] < 120)
                .count();
            assert!(
                red > 20,
                "No visible project sprite in capture: {capture:?}"
            );
            let x_sum: usize = buffer
                .chunks_exact(4)
                .enumerate()
                .filter(|(_, pixel)| pixel[0] > 150 && pixel[1] < 120 && pixel[2] < 120)
                .map(|(index, _)| index % info.width as usize)
                .sum();
            centers.push(x_sum as f64 / red as f64);
            images.push(buffer);
        }
        assert_ne!(
            images[0], images[1],
            "Committed movement must visibly change the captured project scene"
        );
        assert!(
            centers[1] > centers[0] + 5.0,
            "Input-driven horizontal movement must be visible: {centers:?}"
        );
        println!("semantic captures retained at {}", output.display());
    }
}
