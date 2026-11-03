use crate::replay_input::{ReplayInputEvent, ReplayInputState};
use engine_runtime::project_observation::{ProjectObservationContract, ProjectObservationValue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PLAYTEST_SCENARIO_SCHEMA_VERSION: &str = "playtest-scenario.v1";
pub const PLAYTEST_SCENARIO_V2_SCHEMA_VERSION: &str = "playtest-scenario.v2";
// The 36,000-phase-tick streaming recipe needs 48,000 simulation ticks including
// bounded admission callbacks and up to 50,000 presentation frames. This is a tool
// execution bound; it does not change benchmark timing or memory qualification.
pub const MAX_PLAYTEST_FRAMES: u64 = 50_000;
// Full combined trajectories can take longer than their simulated duration.
// Callers still select and enforce a finite per-run deadline; defaults stay unchanged.
pub const MAX_PLAYTEST_TIMEOUT_MS: u64 = 14_400_000;
pub const MAX_PLAYTEST_INPUTS: usize = 4096;
pub const MAX_PLAYTEST_ASSERTIONS: usize = 64;
pub const MAX_PLAYTEST_CAPTURES: usize = 16;
pub const MAX_PLAYTEST_SCENARIO_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaytestTarget {
    WindowsHeadless,
    WindowsWindowed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaytestScenario {
    pub schema_version: String,
    pub scenario_id: String,
    pub initial_scene_id: String,
    pub target: PlaytestTarget,
    pub max_simulation_ticks: u64,
    pub max_presentation_frames: u64,
    pub timeout_ms: u64,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<PlaytestInput>,
    #[serde(default)]
    pub assertions: Vec<PlaytestAssertion>,
    #[serde(default)]
    pub captures: Vec<PlaytestCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_observation: Option<engine_runtime::render_geometry::SpatialObservationRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaytestInput {
    pub simulation_tick: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_down: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_up: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<ReplayInputEvent>>,
}

fn deserialize_inputs<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<PlaytestInput>, D::Error> {
    crate::replay_input::deserialize_points(deserializer, "inputs")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaytestAssertion {
    pub assertion_id: String,
    pub from_simulation_tick: u64,
    pub through_simulation_tick: u64,
    pub path: String,
    pub equals: ProjectObservationValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaytestCapture {
    pub capture_id: String,
    pub presentation_frame: u64,
    pub required: bool,
    pub subjective_review: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioDiagnostic {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

impl PlaytestScenario {
    pub fn summarize(
        &self,
        technical: OutcomeStatus,
        assertions: &BTreeMap<String, OutcomeStatus>,
        captures: &BTreeMap<String, OutcomeStatus>,
        delivery: OutcomeStatus,
    ) -> SemanticOutcome {
        SemanticOutcome {
            technical,
            gameplay: aggregate_outcomes(
                &self
                    .assertions
                    .iter()
                    .map(|assertion| {
                        (
                            true,
                            assertions
                                .get(&assertion.assertion_id)
                                .copied()
                                .unwrap_or(OutcomeStatus::NotProducedYet),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            visual: aggregate_outcomes(
                &self
                    .captures
                    .iter()
                    .map(|capture| {
                        (
                            capture.required,
                            capture.outcome(
                                captures
                                    .get(&capture.capture_id)
                                    .copied()
                                    .unwrap_or(OutcomeStatus::NotProducedYet),
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            delivery,
        }
    }

    pub fn validate(
        &self,
        contract: Option<&ProjectObservationContract>,
    ) -> Result<(), ScenarioDiagnostic> {
        if self.schema_version != PLAYTEST_SCENARIO_SCHEMA_VERSION
            && self.schema_version != PLAYTEST_SCENARIO_V2_SCHEMA_VERSION
        {
            return Err(diagnostic(
                "schema_unsupported",
                "/schemaVersion",
                "Unsupported scenario schema.",
            ));
        }
        if let Some(request) = &self.spatial_observation {
            let selected = request
                .entities
                .iter()
                .map(String::as_str)
                .chain(self.assertions.iter().filter_map(|a| {
                    engine_runtime::render_geometry::spatial_observation_path(&a.path)
                        .map(|(id, _, _)| id)
                }))
                .collect::<BTreeSet<_>>();
            let limit = match request.level {
                engine_runtime::render_contract::DiagnosticLevel::Off => 0,
                engine_runtime::render_contract::DiagnosticLevel::Summary => 64,
                engine_runtime::render_contract::DiagnosticLevel::Trace => 4096,
            };
            if !request.validate() || selected.len() > limit {
                return Err(diagnostic(
                    "spatial_request_invalid",
                    "/spatialObservation",
                    "Use spatial-observation-request.v1 with unique bounded entities; Off cannot satisfy engine assertions.",
                ));
            }
        }
        for (path, id) in [
            ("/scenarioId", &self.scenario_id),
            ("/initialSceneId", &self.initial_scene_id),
        ] {
            if !valid_id(id) {
                return Err(diagnostic(
                    "id_invalid",
                    path,
                    "Expected a stable ASCII id of 1-128 bytes.",
                ));
            }
        }
        for (path, value, limit) in [
            (
                "/maxSimulationTicks",
                self.max_simulation_ticks,
                MAX_PLAYTEST_FRAMES,
            ),
            (
                "/maxPresentationFrames",
                self.max_presentation_frames,
                MAX_PLAYTEST_FRAMES,
            ),
            ("/timeoutMs", self.timeout_ms, MAX_PLAYTEST_TIMEOUT_MS),
        ] {
            if value == 0 || value > limit {
                return Err(diagnostic(
                    "limit_invalid",
                    path,
                    format!("Expected 1..={limit}."),
                ));
            }
        }
        if self.seed.is_some() {
            return Err(diagnostic(
                "seed_unsupported",
                "/seed",
                "No project seed consumer is qualified.",
            ));
        }
        if self.inputs.len() > MAX_PLAYTEST_INPUTS
            || self.assertions.len() > MAX_PLAYTEST_ASSERTIONS
            || self.captures.len() > MAX_PLAYTEST_CAPTURES
        {
            return Err(diagnostic(
                "collection_limit",
                "/",
                "Scenario input/assertion/capture limit exceeded.",
            ));
        }
        let mut last_tick = 0;
        let mut held = BTreeSet::new();
        let mut ordered_state = ReplayInputState::default();
        for (index, input) in self.inputs.iter().enumerate() {
            let path = format!("/inputs/{index}");
            if input.simulation_tick <= last_tick
                || input.simulation_tick > self.max_simulation_ticks
            {
                return Err(diagnostic(
                    "input_tick_invalid",
                    &path,
                    "Input ticks must increase strictly within the simulation limit.",
                ));
            }
            last_tick = input.simulation_tick;
            if self.schema_version == PLAYTEST_SCENARIO_V2_SCHEMA_VERSION {
                let events = input.events.as_ref().ok_or_else(|| {
                    diagnostic(
                        "events_missing",
                        format!("{path}/events"),
                        "playtest-scenario.v2 requires events instead of keyDown/keyUp.",
                    )
                })?;
                if !input.key_down.is_empty() || !input.key_up.is_empty() {
                    return Err(diagnostic(
                        "input_version_mixed",
                        &path,
                        "playtest-scenario.v2 cannot contain legacy key fields.",
                    ));
                }
                ordered_state
                    .validate(events, &format!("{path}/events"))
                    .map_err(|(path, message)| {
                        diagnostic(
                            "events_invalid",
                            path,
                            format!("playtest-scenario.v2: {message}"),
                        )
                    })?;
                continue;
            }
            if input.events.is_some() {
                return Err(diagnostic(
                    "input_version_mixed",
                    format!("{path}/events"),
                    "playtest-scenario.v1 does not accept events; use playtest-scenario.v2.",
                ));
            }
            if input.key_down.len() + input.key_up.len() > 32
                || input.key_down.is_empty() && input.key_up.is_empty()
            {
                return Err(diagnostic(
                    "keys_invalid",
                    &path,
                    "Expected 1-32 keyboard transitions per tick.",
                ));
            }
            let mut changed = BTreeSet::new();
            for (keys, down) in [(&input.key_down, true), (&input.key_up, false)] {
                for key in keys {
                    if !supported_key(key)
                        || !changed.insert(key)
                        || if down {
                            !held.insert(key)
                        } else {
                            !held.remove(key)
                        }
                    {
                        return Err(diagnostic(
                            "keys_invalid",
                            &path,
                            "Unsupported key, duplicate transition, repeated press or release without press.",
                        ));
                    }
                }
            }
        }
        if !held.is_empty() {
            return Err(diagnostic(
                "keys_not_released",
                "/inputs",
                "Release all scripted keys before the scenario ends.",
            ));
        }
        if !ordered_state.released() {
            return Err(diagnostic(
                "events_not_released",
                "/inputs",
                "playtest-scenario.v2: release all scripted keys and mouse buttons before the timeline ends.",
            ));
        }
        if let Some(contract) = contract {
            if contract.validate().is_err() {
                return Err(diagnostic(
                    "observation_contract_invalid",
                    "/assertions",
                    "ObservationContract is invalid.",
                ));
            }
        }
        let mut ids = BTreeSet::new();
        let mut last_start = 0;
        for (index, assertion) in self.assertions.iter().enumerate() {
            let path = format!("/assertions/{index}");
            if !valid_id(&assertion.assertion_id) || !ids.insert(&assertion.assertion_id) {
                return Err(diagnostic(
                    "assertion_id_invalid",
                    &path,
                    "Assertion ids must be stable and unique.",
                ));
            }
            if assertion.from_simulation_tick == 0
                || assertion.from_simulation_tick < last_start
                || assertion.from_simulation_tick > assertion.through_simulation_tick
                || assertion.through_simulation_tick > self.max_simulation_ticks
            {
                return Err(diagnostic(
                    "assertion_window_invalid",
                    &path,
                    "Expected inclusive simulation intervals ordered by start, inside the simulation limit.",
                ));
            }
            last_start = assertion.from_simulation_tick;
            if assertion.path.starts_with("engine.") {
                let (_, _, ty) =
                    engine_runtime::render_geometry::spatial_observation_path(&assertion.path)
                        .ok_or_else(|| {
                            diagnostic(
                                "engine_observation_path_unknown",
                                format!("{path}/path"),
                                "Unknown reserved engine observation path.",
                            )
                        })?;
                if !assertion.equals.is_valid_scalar() || assertion.equals.value_type() != ty {
                    return Err(diagnostic(
                        "observation_type_mismatch",
                        format!("{path}/equals"),
                        "Engine observation requires its declared scalar type.",
                    ));
                }
                continue;
            }
            let entry = contract
                .and_then(|contract| {
                    contract
                        .observations
                        .iter()
                        .find(|entry| entry.path == assertion.path)
                })
                .ok_or_else(|| {
                    diagnostic(
                        "observation_path_unknown",
                        format!("{path}/path"),
                        "Path is not declared by the project's ObservationContract.",
                    )
                })?;
            if !assertion.equals.is_valid_scalar()
                || assertion.equals.value_type() != entry.value_type
            {
                return Err(diagnostic(
                    "observation_type_mismatch",
                    format!("{path}/equals"),
                    "Expected value must match the declared scalar type exactly.",
                ));
            }
            if entry
                .allowed_values
                .as_ref()
                .is_some_and(|values| !values.contains(&assertion.equals))
            {
                return Err(diagnostic(
                    "observation_value_not_allowed",
                    format!("{path}/equals"),
                    "Expected value is outside the declared allowedValues.",
                ));
            }
        }
        let mut ids = BTreeSet::new();
        let mut last_frame = 0;
        for (index, capture) in self.captures.iter().enumerate() {
            let path = format!("/captures/{index}");
            if !valid_id(&capture.capture_id)
                || !ids.insert(&capture.capture_id)
                || capture.presentation_frame <= last_frame
                || capture.presentation_frame > self.max_presentation_frames
            {
                return Err(diagnostic(
                    "capture_invalid",
                    path,
                    "Capture ids must be unique; presentation frames must strictly increase within the presentation limit.",
                ));
            }
            last_frame = capture.presentation_frame;
        }
        Ok(())
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub(crate) fn supported_key(key: &str) -> bool {
    matches!(
        key,
        "Space" | "Enter" | "Escape" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
    ) || key.len() == 1
        && (key.as_bytes()[0].is_ascii_uppercase() || key.as_bytes()[0].is_ascii_digit())
}

fn diagnostic(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ScenarioDiagnostic {
    ScenarioDiagnostic {
        code: format!("playtest.{code}"),
        field_path: path.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutcomeStatus {
    Passed,
    Failed,
    NotChecked,
    Unsupported,
    NotProducedYet,
    PendingReview,
}

/// The caller supplies one result per declared item, after validating evidence identity.
/// Required missing items fail closed; optional evidence cannot manufacture a pass.
pub fn aggregate_outcomes(items: &[(bool, OutcomeStatus)]) -> OutcomeStatus {
    use OutcomeStatus::*;
    let required: Vec<_> = items
        .iter()
        .filter(|(required, _)| *required)
        .map(|(_, status)| *status)
        .collect();
    for status in [
        Failed,
        Unsupported,
        NotProducedYet,
        NotChecked,
        PendingReview,
    ] {
        if required.contains(&status) {
            return status;
        }
    }
    if required.is_empty() {
        NotChecked
    } else {
        Passed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticOutcome {
    pub technical: OutcomeStatus,
    pub gameplay: OutcomeStatus,
    pub visual: OutcomeStatus,
    pub delivery: OutcomeStatus,
}

impl SemanticOutcome {
    pub fn overall(&self, scenario: &PlaytestScenario, require_delivery: bool) -> OutcomeStatus {
        aggregate_outcomes(&[
            (true, self.technical),
            (!scenario.assertions.is_empty(), self.gameplay),
            (
                scenario.captures.iter().any(|capture| capture.required),
                self.visual,
            ),
            (require_delivery, self.delivery),
        ])
    }
}

impl PlaytestCapture {
    pub fn outcome(&self, raw_evidence: OutcomeStatus) -> OutcomeStatus {
        if raw_evidence == OutcomeStatus::Passed && self.subjective_review {
            OutcomeStatus::PendingReview
        } else {
            raw_evidence
        }
    }
}

impl PlaytestAssertion {
    /// One committed sample only. The bounded executor owns interval completion, not this predicate.
    pub fn matches_sample(&self, simulation_tick: u64, actual: &ProjectObservationValue) -> bool {
        simulation_tick >= self.from_simulation_tick
            && simulation_tick <= self.through_simulation_tick
            && actual.is_valid_scalar()
            && actual.value_type() == self.equals.value_type()
            && actual == &self.equals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::project_observation::{
        ProjectObservationEntry, ProjectObservationType,
        PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION,
    };

    fn contract() -> ProjectObservationContract {
        ProjectObservationContract {
            schema_version: PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION.into(),
            contract_id: "game.observations".into(),
            observations: vec![ProjectObservationEntry {
                path: "game.ready".into(),
                value_type: ProjectObservationType::Bool,
                description: "Ready".into(),
                allowed_values: None,
            }],
        }
    }

    fn scenario() -> PlaytestScenario {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": PLAYTEST_SCENARIO_SCHEMA_VERSION,
            "scenarioId": "test.ready", "initialSceneId": "scene-main", "target": "windows-headless",
            "maxSimulationTicks": 3, "maxPresentationFrames": 6, "timeoutMs": 1000,
            "inputs": [{"simulationTick": 1, "keyDown": ["Space"]}, {"simulationTick": 2, "keyUp": ["Space"]}],
            "assertions": [{"assertionId": "ready", "fromSimulationTick": 2, "throughSimulationTick": 3, "path": "game.ready", "equals": true}]
        })).unwrap()
    }

    #[test]
    fn original_world_k2_admits_complete_route_callbacks_and_final_capture() {
        let input: PlaytestScenario = serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../samples/platform_3d_bm13_k2_project/Tests/bm13-k2.scenario.json"
        )))
        .unwrap();
        let observations: ProjectObservationContract = serde_json::from_slice(include_bytes!(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../samples/platform_3d_bm13_k2_project/Observations/project.observations.json"
            )
        ))
        .unwrap();
        assert_eq!(input.max_simulation_ticks, 48_000);
        assert_eq!(input.max_presentation_frames, 50_000);
        assert_eq!(input.timeout_ms, 14_400_000);
        assert_eq!(input.assertions.len(), 8);
        assert!(input.assertions.iter().all(|assertion| {
            assertion.from_simulation_tick == 48_000 && assertion.through_simulation_tick == 48_000
        }));
        assert_eq!(input.captures.len(), 7);
        assert_eq!(input.captures.last().unwrap().presentation_frame, 48_000);
        input.validate(Some(&observations)).unwrap();
    }

    #[test]
    fn semantic_full_world_k2_admits_explicit_four_hour_execution_budget() {
        let mut input = scenario();
        input.max_simulation_ticks = 48_000;
        input.max_presentation_frames = 50_000;
        input.timeout_ms = 14_400_000;
        input
            .validate(Some(&contract()))
            .expect("original full K2 needs bounded admission callbacks and wall time");
    }

    #[test]
    fn full_combined_cycle_allows_a_bounded_hour_without_changing_other_limits() {
        let mut input = scenario();
        input.max_simulation_ticks = 7201;
        input.max_presentation_frames = 9000;
        input.timeout_ms = 3_600_000;
        input
            .validate(Some(&contract()))
            .expect("full combined trajectory must admit its explicit bounded execution time");
        input.timeout_ms = MAX_PLAYTEST_TIMEOUT_MS + 1;
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.limit_invalid"
        );
        input.timeout_ms = 3_600_000;
        input.max_simulation_ticks = MAX_PLAYTEST_FRAMES + 1;
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.limit_invalid"
        );
    }

    #[test]
    fn semantic_world_route_has_bounded_room_for_sdk_callbacks() {
        let mut input = scenario();
        input.max_simulation_ticks = 40_000;
        input.max_presentation_frames = 40_000;
        input.timeout_ms = MAX_PLAYTEST_TIMEOUT_MS;
        input.validate(Some(&contract())).unwrap();
        for field in ["maxSimulationTicks", "maxPresentationFrames", "timeoutMs"] {
            let mut value = serde_json::to_value(&input).unwrap();
            value[field] = serde_json::json!(if field == "timeoutMs" {
                MAX_PLAYTEST_TIMEOUT_MS + 1
            } else {
                MAX_PLAYTEST_FRAMES + 1
            });
            let rejected: PlaytestScenario = serde_json::from_value(value).unwrap();
            assert_eq!(
                rejected.validate(Some(&contract())).unwrap_err().code,
                "playtest.limit_invalid"
            );
        }
    }

    #[test]
    fn ordered_input_v2_accepts_same_tick_mouse_and_key_edges() {
        let mut value = serde_json::to_value(scenario()).unwrap();
        value["schemaVersion"] = "playtest-scenario.v2".into();
        value["inputs"] = serde_json::json!([{"simulationTick":1,"events":[
            {"type":"pointerMove","position":[10,20]},
            {"type":"mouseDown","button":"Left"},
            {"type":"pointerMove","position":[40,50]},
            {"type":"mouseUp","button":"Left"},
            {"type":"keyDown","key":"ShiftLeft"},
            {"type":"keyUp","key":"ShiftLeft"}
        ]}]);
        let decoded: PlaytestScenario = serde_json::from_value(value.clone()).unwrap();
        decoded.validate(Some(&contract())).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["inputs"],
            value["inputs"]
        );
    }

    #[test]
    fn semantic_outcome_ordinary_3d_fixture_uses_current_schema() {
        let scenario: PlaytestScenario = serde_json::from_str(include_str!(
            "../../../../samples/three_d_lab_project/Tests/spatial-controls.scenario.json"
        ))
        .unwrap();
        scenario.validate(None).unwrap();
        assert_eq!(scenario.assertions.len(), 13);
        assert_eq!(scenario.captures.len(), 7);
    }

    #[test]
    fn semantic_outcome_spatial_requests_are_versioned_bounded_and_off_is_explicit() {
        use engine_runtime::render_contract::DiagnosticLevel;
        use engine_runtime::render_geometry::SpatialObservationRequest;
        let mut input = scenario();
        input.assertions.clear();
        input.spatial_observation = Some(SpatialObservationRequest {
            schema_version: "spatial-observation-request.v1".into(),
            level: DiagnosticLevel::Off,
            entities: vec![],
        });
        input.validate(None).unwrap();
        input
            .spatial_observation
            .as_mut()
            .unwrap()
            .entities
            .push("camera".into());
        assert!(input.validate(None).is_err());
        let request = input.spatial_observation.as_mut().unwrap();
        request.level = DiagnosticLevel::Summary;
        request.entities = (0..65).map(|i| format!("e-{i}")).collect();
        assert!(input.validate(None).is_err());
        input.spatial_observation.as_mut().unwrap().level = DiagnosticLevel::Trace;
        input.validate(None).unwrap();
        input.spatial_observation.as_mut().unwrap().schema_version = "future".into();
        assert!(input.validate(None).is_err());
    }

    #[test]
    fn semantic_outcome_engine_paths_are_typed_and_cannot_be_project_spoofed() {
        let mut input = scenario();
        input.assertions[0].path = "engine.spatial/cube-front/visible".into();
        input.validate(None).unwrap();
        input.assertions[0].equals = ProjectObservationValue::String("true".into());
        assert!(input.validate(None).is_err());
        input.assertions[0].path = "engine.spatial/cube-front/invented".into();
        assert!(input.validate(None).is_err());
        let mut project = contract();
        project.observations[0].path = "engine.spatial.visible".into();
        assert!(project.validate().is_err());
    }

    #[test]
    fn semantic_scenario_rejects_unknown_observation_path() {
        let mut input = scenario();
        input.assertions[0].path = "hud.text".into();
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.observation_path_unknown"
        );
    }

    #[test]
    fn semantic_scenario_roundtrip_and_typed_inclusive_samples() {
        let input = scenario();
        assert_eq!(
            serde_json::from_slice::<PlaytestScenario>(&serde_json::to_vec(&input).unwrap())
                .unwrap(),
            input
        );
        input.validate(Some(&contract())).unwrap();
        let assertion = &input.assertions[0];
        for tick in 0..=4 {
            assert_eq!(
                assertion.matches_sample(tick, &ProjectObservationValue::Bool(true)),
                (2..=3).contains(&tick)
            );
        }
        assert!(!assertion.matches_sample(2, &ProjectObservationValue::Bool(false)));
        assert!(!assertion.matches_sample(2, &ProjectObservationValue::Integer(1)));
        let mut checkpoint = assertion.clone();
        checkpoint.through_simulation_tick = 2;
        assert!(!checkpoint.matches_sample(3, &ProjectObservationValue::Bool(true)));
        for expected in [
            ProjectObservationValue::Integer(7),
            ProjectObservationValue::Number(7.0),
            ProjectObservationValue::String("ready".into()),
        ] {
            checkpoint.equals = expected.clone();
            assert!(checkpoint.matches_sample(2, &expected));
            let decoded: PlaytestAssertion =
                serde_json::from_slice(&serde_json::to_vec(&checkpoint).unwrap()).unwrap();
            assert_eq!(decoded.equals.value_type(), expected.value_type());
        }
        checkpoint.equals = ProjectObservationValue::Number(7.0);
        assert!(!checkpoint.matches_sample(2, &ProjectObservationValue::Integer(7)));
        assert!(!checkpoint.matches_sample(2, &ProjectObservationValue::Number(f64::INFINITY)));
    }

    #[test]
    fn semantic_scenario_rejects_type_missing_contract_and_allowed_value_errors() {
        let mut input = scenario();
        input.assertions[0].equals = ProjectObservationValue::Integer(1);
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.observation_type_mismatch"
        );
        input.assertions[0].equals = ProjectObservationValue::Number(f64::NAN);
        assert!(input.validate(Some(&contract())).is_err());
        input = scenario();
        assert!(input.validate(None).is_err());
        let mut declaration = contract();
        declaration.observations[0].allowed_values =
            Some(vec![ProjectObservationValue::Bool(false)]);
        assert_eq!(
            input.validate(Some(&declaration)).unwrap_err().code,
            "playtest.observation_value_not_allowed"
        );
        declaration.schema_version = "unknown".into();
        assert_eq!(
            input.validate(Some(&declaration)).unwrap_err().code,
            "playtest.observation_contract_invalid"
        );
    }

    #[test]
    fn semantic_scenario_rejects_limits_seed_unknown_fields_and_targets() {
        for (field, value) in [
            ("schemaVersion", serde_json::json!("unknown")),
            ("scenarioId", serde_json::json!("")),
            ("initialSceneId", serde_json::json!("../scene")),
            ("maxSimulationTicks", serde_json::json!(0)),
            (
                "maxSimulationTicks",
                serde_json::json!(MAX_PLAYTEST_FRAMES + 1),
            ),
            ("maxPresentationFrames", serde_json::json!(0)),
            (
                "maxPresentationFrames",
                serde_json::json!(MAX_PLAYTEST_FRAMES + 1),
            ),
            ("timeoutMs", serde_json::json!(0)),
            ("timeoutMs", serde_json::json!(MAX_PLAYTEST_TIMEOUT_MS + 1)),
            ("seed", serde_json::json!(0)),
        ] {
            let mut json = serde_json::to_value(scenario()).unwrap();
            json[field] = value;
            assert!(
                serde_json::from_value::<PlaytestScenario>(json)
                    .unwrap()
                    .validate(Some(&contract()))
                    .is_err(),
                "{field}"
            );
        }
        for (field, value) in [
            ("target", serde_json::json!("android")),
            ("script", serde_json::json!("arbitrary()")),
        ] {
            let mut json = serde_json::to_value(scenario()).unwrap();
            json[field] = value;
            assert!(serde_json::from_value::<PlaytestScenario>(json).is_err());
        }
        let mut input = scenario();
        input.inputs = vec![input.inputs[0].clone(); MAX_PLAYTEST_INPUTS + 1];
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.collection_limit"
        );
        input = scenario();
        input.assertions = vec![input.assertions[0].clone(); MAX_PLAYTEST_ASSERTIONS + 1];
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.collection_limit"
        );
    }

    #[test]
    fn semantic_scenario_rejects_input_order_bounds_and_invalid_key_transitions() {
        let invalid = [
            serde_json::json!([{"simulationTick": 0, "keyDown": ["Space"]}]),
            serde_json::json!([{"simulationTick": 4, "keyDown": ["Space"]}]),
            serde_json::json!([{"simulationTick": 2, "keyDown": ["Space"]}, {"simulationTick": 1, "keyUp": ["Space"]}]),
            serde_json::json!([{"simulationTick": 1, "keyDown": ["Space"]}, {"simulationTick": 1, "keyUp": ["Space"]}]),
            serde_json::json!([{"simulationTick": 1, "keyDown": ["MouseLeft"]}]),
            serde_json::json!([{"simulationTick": 1, "keyDown": ["Space", "Space"]}]),
            serde_json::json!([{"simulationTick": 1, "keyDown": ["Space"], "keyUp": ["Space"]}]),
            serde_json::json!([{"simulationTick": 1, "keyUp": ["Space"]}]),
            serde_json::json!([{"simulationTick": 1, "keyDown": ["Space"]}, {"simulationTick": 2, "keyDown": ["Space"]}]),
            serde_json::json!([{"simulationTick": 1, "keyDown": ["Space"]}]),
            serde_json::json!([{"simulationTick": 1}]),
        ];
        for json in invalid {
            let mut input = scenario();
            input.inputs = serde_json::from_value(json.clone()).unwrap();
            assert!(input.validate(Some(&contract())).is_err(), "{json}");
        }
        for key in ["A", "D", "0", "9", "ArrowLeft", "Enter"] {
            let mut input = scenario();
            input.inputs[0].key_down = vec![key.into()];
            input.inputs[1].key_up = vec![key.into()];
            input.validate(Some(&contract())).unwrap();
        }
        assert!(!supported_key("KeyA"));
        assert!(!supported_key("Digit1"));
    }

    #[test]
    fn semantic_scenario_rejects_duplicate_ids_and_invalid_assertion_windows() {
        for (from, through) in [(0, 1), (3, 2), (1, 4)] {
            let mut input = scenario();
            input.assertions[0].from_simulation_tick = from;
            input.assertions[0].through_simulation_tick = through;
            assert_eq!(
                input.validate(Some(&contract())).unwrap_err().code,
                "playtest.assertion_window_invalid"
            );
        }
        let mut input = scenario();
        input.assertions.push(input.assertions[0].clone());
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.assertion_id_invalid"
        );
        input.assertions[1].assertion_id = "earlier".into();
        input.assertions[1].from_simulation_tick = 1;
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.assertion_window_invalid"
        );
    }

    fn capture() -> PlaytestCapture {
        PlaytestCapture {
            capture_id: "last-frame".into(),
            presentation_frame: 6,
            required: true,
            subjective_review: false,
        }
    }

    #[test]
    fn semantic_scenario_capture_clock_is_not_simulation_clock() {
        let mut input = scenario();
        input.captures.push(capture());
        input.validate(Some(&contract())).unwrap();
        for frame in [0, 7] {
            input.captures[0].presentation_frame = frame;
            assert_eq!(
                input.validate(Some(&contract())).unwrap_err().code,
                "playtest.capture_invalid"
            );
        }
        input.captures = vec![capture(), capture()];
        assert!(input.validate(Some(&contract())).is_err());
        input.captures[1].capture_id = "other".into();
        assert!(input.validate(Some(&contract())).is_err());
        input.captures[1].presentation_frame = 5;
        assert!(input.validate(Some(&contract())).is_err());
        input.captures = vec![capture(); MAX_PLAYTEST_CAPTURES + 1];
        assert_eq!(
            input.validate(Some(&contract())).unwrap_err().code,
            "playtest.collection_limit"
        );
    }

    #[test]
    fn semantic_outcome_missing_required_capture_and_assertion_cannot_pass() {
        use OutcomeStatus::*;
        let mut input = scenario();
        input.captures.push(capture());
        let assertions = BTreeMap::from([("ready".into(), Passed)]);
        let mut captures = BTreeMap::new();
        let missing = input.summarize(Passed, &assertions, &captures, NotChecked);
        assert_eq!(missing.visual, NotProducedYet);
        assert_eq!(missing.overall(&input, false), NotProducedYet);
        captures.insert("last-frame".into(), Passed);
        let complete = input.summarize(Passed, &assertions, &captures, NotChecked);
        assert_eq!(complete.overall(&input, false), Passed);
        assert_eq!(complete.overall(&input, true), NotChecked);
        input.captures[0].subjective_review = true;
        assert_eq!(
            input
                .summarize(Passed, &assertions, &captures, NotChecked)
                .visual,
            PendingReview
        );
        assert_eq!(
            input
                .summarize(Passed, &BTreeMap::new(), &captures, NotChecked)
                .gameplay,
            NotProducedYet
        );
        assert_eq!(
            input
                .summarize(
                    Passed,
                    &BTreeMap::from([("ready".into(), Failed)]),
                    &captures,
                    NotChecked
                )
                .overall(&input, false),
            Failed
        );
        input.captures[0].required = false;
        assert_eq!(
            input
                .summarize(Passed, &assertions, &BTreeMap::new(), NotChecked)
                .visual,
            NotChecked
        );
    }

    #[test]
    fn semantic_outcome_nonpasses_remain_distinct_and_empty_is_not_passed() {
        use OutcomeStatus::*;
        assert_eq!(aggregate_outcomes(&[]), NotChecked);
        for status in [
            Failed,
            Unsupported,
            NotProducedYet,
            NotChecked,
            PendingReview,
        ] {
            assert_eq!(
                aggregate_outcomes(&[(true, Passed), (true, status)]),
                status
            );
            assert_eq!(capture().outcome(status), status);
        }
        let input = scenario();
        assert_eq!(
            input
                .summarize(Passed, &BTreeMap::new(), &BTreeMap::new(), NotChecked)
                .visual,
            NotChecked
        );
        let outcome = SemanticOutcome {
            technical: Passed,
            gameplay: Failed,
            visual: Unsupported,
            delivery: NotChecked,
        };
        assert_eq!(
            serde_json::from_slice::<SemanticOutcome>(&serde_json::to_vec(&outcome).unwrap())
                .unwrap(),
            outcome
        );
    }
}
