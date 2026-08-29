use crate::diagnostics::{DiagnosticSeverity, RuntimeDiagnostic};
use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use crate::runtime_package::load_runtime_package;
use crate::runtime_scene_hydration::hydrate_active_scene_into_world;
use crate::runtime_trace::RuntimeTrace;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const RUNTIME_RUN_REPORT_SCHEMA_VERSION: &str = "runtime-run-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeRunMode {
    Headless,
    Windowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunOptions {
    pub package_dir: PathBuf,
    pub cooked_assets_root: Option<PathBuf>,
    pub mode: RuntimeRunMode,
    pub frame_limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRunDiagnostic {
    pub severity: RuntimeRunDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl RuntimeRunDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RuntimeRunDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRunDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceSummary {
    pub event_count: usize,
    pub last_frame: Option<u64>,
    pub last_phase: Option<String>,
    pub last_system_id: Option<String>,
    pub last_message: Option<String>,
}

impl RuntimeTraceSummary {
    pub fn from_trace(trace: &RuntimeTrace) -> Self {
        let last = trace.events.last();
        Self {
            event_count: trace.events.len(),
            last_frame: last.map(|event| event.frame),
            last_phase: last.map(|event| event.phase.clone()),
            last_system_id: last.map(|event| event.system_id.clone()),
            last_message: last.map(|event| event.message.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFrameSummary {
    pub frame_index: u64,
    pub raw_command_count: usize,
    pub merged_command_count: usize,
    pub applied_command_count: usize,
    pub render_scene_proxy_count: usize,
    pub diagnostic_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRunReport {
    pub schema_version: String,
    pub run_id: String,
    pub package_path: String,
    pub cooked_assets_root: Option<String>,
    pub mode: RuntimeRunMode,
    pub frame_limit: u64,
    pub frames_executed: u64,
    pub exit_reason: String,
    pub exit_code: i32,
    pub diagnostics: Vec<RuntimeRunDiagnostic>,
    pub last_frame_hash: Option<String>,
    pub last_runtime_trace_summary: Option<RuntimeTraceSummary>,
    pub last_render_frame_summary: Option<RenderFrameSummary>,
}

impl RuntimeRunReport {
    pub fn failed(
        mode: RuntimeRunMode,
        frame_limit: u64,
        diagnostics: Vec<RuntimeRunDiagnostic>,
    ) -> Self {
        Self {
            schema_version: RUNTIME_RUN_REPORT_SCHEMA_VERSION.to_string(),
            run_id: "runtime-run-failed".to_string(),
            package_path: String::new(),
            cooked_assets_root: None,
            mode,
            frame_limit,
            frames_executed: 0,
            exit_reason: "failed".to_string(),
            exit_code: 1,
            diagnostics,
            last_frame_hash: None,
            last_runtime_trace_summary: None,
            last_render_frame_summary: None,
        }
    }
}

pub fn run_runtime_package_headless(options: RuntimeRunOptions) -> RuntimeRunReport {
    let mut diagnostics = Vec::new();
    if options.mode != RuntimeRunMode::Headless {
        return RuntimeRunReport {
            package_path: options.package_dir.display().to_string(),
            cooked_assets_root: options
                .cooked_assets_root
                .as_ref()
                .map(|path| path.display().to_string()),
            ..RuntimeRunReport::failed(
                options.mode,
                options.frame_limit,
                vec![RuntimeRunDiagnostic::error(
                    "unsupported_mode",
                    "runtime CLI v1 only supports headless mode",
                )],
            )
        };
    }
    let load = load_runtime_package(&options.package_dir);
    diagnostics.extend(convert_load_diagnostics(&load.diagnostics.issues));
    let Some(package) = load.value else {
        return RuntimeRunReport {
            package_path: options.package_dir.display().to_string(),
            cooked_assets_root: options
                .cooked_assets_root
                .as_ref()
                .map(|path| path.display().to_string()),
            ..RuntimeRunReport::failed(options.mode, options.frame_limit, diagnostics)
        };
    };

    let world_result = hydrate_active_scene_into_world(&package);
    diagnostics.extend(convert_load_diagnostics(&world_result.diagnostics.issues));
    let Some((mut world, _hydration_report)) = world_result.value else {
        return RuntimeRunReport {
            package_path: options.package_dir.display().to_string(),
            cooked_assets_root: options
                .cooked_assets_root
                .as_ref()
                .map(|path| path.display().to_string()),
            ..RuntimeRunReport::failed(options.mode, options.frame_limit, diagnostics)
        };
    };

    let mut host = EngineHostLoop::new(package.active_scene.id.clone());
    let mut frames_executed = 0;
    let mut last_frame_hash = None;
    let mut last_runtime_trace_summary = None;
    let mut last_render_frame_summary = None;
    for _ in 0..options.frame_limit {
        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );
        frames_executed += 1;
        last_frame_hash = output.frame_hash;
        last_runtime_trace_summary = Some(RuntimeTraceSummary::from_trace(&output.runtime_trace));
        last_render_frame_summary = output.render_frame_report.map(|report| RenderFrameSummary {
            frame_index: report.frame_index,
            raw_command_count: report.counters.raw_command_count,
            merged_command_count: output
                .minimal_renderer_frame
                .as_ref()
                .map(|_| 0)
                .unwrap_or(report.counters.merged_command_count),
            applied_command_count: report.counters.applied_command_count,
            render_scene_proxy_count: host.render_scene().proxies_len(),
            diagnostic_count: report.render_events.len(),
        });
    }

    RuntimeRunReport {
        schema_version: RUNTIME_RUN_REPORT_SCHEMA_VERSION.to_string(),
        run_id: format!(
            "runtime-run-{}",
            package.manifest.content_hash.unwrap_or_default()
        ),
        package_path: options.package_dir.display().to_string(),
        cooked_assets_root: options
            .cooked_assets_root
            .as_ref()
            .map(|path| path.display().to_string()),
        mode: options.mode,
        frame_limit: options.frame_limit,
        frames_executed,
        exit_reason: "completed".to_string(),
        exit_code: 0,
        diagnostics,
        last_frame_hash,
        last_runtime_trace_summary,
        last_render_frame_summary,
    }
}

fn convert_load_diagnostics(issues: &[RuntimeDiagnostic]) -> Vec<RuntimeRunDiagnostic> {
    issues
        .iter()
        .map(|issue| RuntimeRunDiagnostic {
            severity: match issue.severity {
                DiagnosticSeverity::Error => RuntimeRunDiagnosticSeverity::Error,
                DiagnosticSeverity::Warning => RuntimeRunDiagnosticSeverity::Warning,
            },
            code: match issue.severity {
                DiagnosticSeverity::Error => "runtime_load_error".to_string(),
                DiagnosticSeverity::Warning => "runtime_load_warning".to_string(),
            },
            message: issue.message.clone(),
            path: Some(issue.path.clone()),
        })
        .collect()
}

#[cfg(test)]
pub mod tests_support {
    use std::fs;
    use std::path::{Path, PathBuf};

    pub fn write_minimal_runtime_package(root: &Path, name: &str) -> PathBuf {
        let package_dir = root.join(name);
        fs::create_dir_all(package_dir.join("scenes")).unwrap();
        fs::create_dir_all(package_dir.join("assets")).unwrap();
        fs::create_dir_all(package_dir.join("rules")).unwrap();
        fs::create_dir_all(package_dir.join("input")).unwrap();
        fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-runtime-cli-test",
    "name": "Runtime CLI Test",
    "version": "0.0.3",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "none" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
  "contentHash": "testhash"
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("scenes").join("scene-main.json"),
            r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#101010",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    }
  }]
}"##,
        )
        .unwrap();
        fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [{
    "id": "scene-main",
    "name": "Main",
    "type": "scene",
    "source": "scenes/scene-main.json",
    "state": "available",
    "bundleId": "startup"
  }],
  "runtimeAssetIndex": [{
    "assetGuid": "scene-main",
    "assetId": "scene-main",
    "assetType": "scene",
    "subAssetId": null,
    "version": "1",
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "loaderKind": "scene",
    "dependencies": [],
    "hash": null,
    "size": null,
    "flags": ["test"]
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "path": "scenes/scene-main.json",
    "offset": null,
    "size": null,
    "compression": "none",
    "hash": null
  }],
  "dependencyTable": []
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "none",
  "rules": [],
  "modules": []
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input-manifest.json"),
            r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input.none.json"),
            serde_json::to_string_pretty(&engine_input::InputMappingAsset::explicit_empty(
                "input.none",
            ))
            .unwrap(),
        )
        .unwrap();
        package_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_run_report_serializes() {
        let report = RuntimeRunReport::failed(
            RuntimeRunMode::Headless,
            1,
            vec![RuntimeRunDiagnostic::error(
                "package_load_failure",
                "failed",
            )],
        );
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("runtime-run-report.v1"));
        assert!(json.contains("package_load_failure"));
    }

    #[test]
    fn runtime_run_report_records_successful_fixed_frame_run() {
        let root = temp_root("success");
        let package_dir = tests_support::write_minimal_runtime_package(&root, "runtime-package");
        let report = run_runtime_package_headless(RuntimeRunOptions {
            package_dir,
            cooked_assets_root: None,
            mode: RuntimeRunMode::Headless,
            frame_limit: 3,
        });

        assert_eq!(report.exit_code, 0);
        assert_eq!(report.frames_executed, 3);
        assert_eq!(report.exit_reason, "completed");
        assert!(report.last_frame_hash.is_some());
        assert_eq!(
            report
                .last_runtime_trace_summary
                .as_ref()
                .and_then(|summary| summary.last_frame),
            Some(3)
        );
    }

    #[test]
    fn runtime_run_report_records_package_load_failure() {
        let root = temp_root("missing-package");
        let report = run_runtime_package_headless(RuntimeRunOptions {
            package_dir: root.join("missing-package"),
            cooked_assets_root: None,
            mode: RuntimeRunMode::Headless,
            frame_limit: 1,
        });

        assert_eq!(report.exit_code, 1);
        assert_eq!(report.frames_executed, 0);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_load_error"));
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("engine-runtime-run-{name}-{stamp}"))
    }
}
