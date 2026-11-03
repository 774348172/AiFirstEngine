//! Internal byte transport for the Engine DLL; not an Agent tool contract.

use crate::semantic_outcome::PlaytestScenario;
use crate::NativePlayerWindowRunRequest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const ENGINE_RUNTIME_EXECUTION_MAX_BYTES: usize = 4 * 1024 * 1024;
pub const ENGINE_RUNTIME_EXECUTION_CAPABILITY: u64 = 1 << 5;
pub const ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR: u32 = 2;

/// The caller owns serialized bytes until the synchronous DLL call returns.
/// Rust values and allocations are constructed and dropped inside the DLL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineRuntimeExecutionRequest {
    pub request: NativePlayerWindowRunRequest,
    pub scenario: Option<PlaytestScenario>,
    pub report_path: PathBuf,
    pub capture_directory: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NativePlayerInputScript, NativePlayerInputScriptFrame};
    use engine_runtime::windowed_player::WindowedPlayerRuntimeReportLevel;

    #[test]
    fn engine_dll_execution_roundtrip_preserves_window_input_and_evidence_settings() {
        let mut request = NativePlayerWindowRunRequest::windowed("package")
            .with_screenshot("capture.png")
            .with_input_script(NativePlayerInputScript::new(
                "pointer-and-fire",
                vec![NativePlayerInputScriptFrame {
                    frame_index: 2,
                    pointer_position: Some([120, 240]),
                    key_down: vec!["Space".into()],
                    key_up: vec![],
                    events: None,
                }],
            ))
            .with_runtime_report_level(WindowedPlayerRuntimeReportLevel::Trace)
            .with_frame_performance_sample(10, 60);
        request.frame_limit = 90;
        request.project_storage = Some(crate::NativePlayerProjectStorage {
            base: PathBuf::from("D:/isolated-player-data"),
            profile: "test-profile".into(),
        });
        request.config.title = "Engine DLL request".into();
        request.game_view_target.extent.width = 800;
        request.config.width = 800;
        request.screenshot.frame_index = Some(40);
        let request = EngineRuntimeExecutionRequest {
            request,
            scenario: None,
            report_path: "reports/player.json".into(),
            capture_directory: None,
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: EngineRuntimeExecutionRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);
        assert!(bytes.len() < ENGINE_RUNTIME_EXECUTION_MAX_BYTES);
    }

    #[test]
    fn engine_dll_execution_rejects_unknown_envelope_fields() {
        let request = EngineRuntimeExecutionRequest {
            request: NativePlayerWindowRunRequest::headless_surface_gate("package"),
            scenario: None,
            report_path: "reports/player.json".into(),
            capture_directory: None,
        };
        let mut value = serde_json::to_value(request).unwrap();
        value["operation"] = serde_json::json!("arbitrary-route");
        assert!(serde_json::from_value::<EngineRuntimeExecutionRequest>(value).is_err());
    }

    #[test]
    fn ordered_input_engine_dll_roundtrip_retains_versions_and_event_order() {
        let scenario: PlaytestScenario = serde_json::from_value(serde_json::json!({
            "schemaVersion":"playtest-scenario.v2","scenarioId":"ordered","initialSceneId":"scene-main",
            "target":"windows-headless","maxSimulationTicks":2,"maxPresentationFrames":4,"timeoutMs":1000,
            "inputs":[{"simulationTick":1,"events":[{"type":"pointerMove","position":[12,24]},{"type":"mouseDown","button":"Left"},{"type":"pointerMove","position":[36,48]},{"type":"mouseUp","button":"Left"}]}]
        })).unwrap();
        scenario.validate(None).unwrap();
        let script = crate::semantic_playtest::SemanticPlaytest::input_script(&scenario);
        script.validate().unwrap();
        let request = EngineRuntimeExecutionRequest {
            request: NativePlayerWindowRunRequest::headless_surface_gate("package")
                .with_input_script(script),
            scenario: Some(scenario),
            report_path: "report.json".into(),
            capture_directory: None,
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("keyDown"));
        let decoded: EngineRuntimeExecutionRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);
        decoded.scenario.as_ref().unwrap().validate(None).unwrap();
        decoded
            .request
            .input_script
            .as_ref()
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(crate::scripted_input_events(&decoded.request, 1).len(), 4);
    }
}
