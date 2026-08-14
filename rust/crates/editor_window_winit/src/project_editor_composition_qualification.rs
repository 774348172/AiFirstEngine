use editor_core::{
    CommandStatus, EditorSession, ProjectEditorCompositionArtifact,
    ProjectEditorCompositionIdentity, ProjectEditorCompositionQualificationSeal,
    PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
};
use editor_ui_model::{ui_command_id_for_payload, UiCommand, UiCommandPayload, UiCommandSource};
use engine_input::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
#[cfg(feature = "real-window")]
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use engine_runtime::project_runtime_session::{
    ProjectRuntimeSessionStage, ProjectRuntimeSessionStageReport,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(feature = "real-window")]
use std::path::PathBuf;
use std::sync::Arc;

pub const PROJECT_EDITOR_COMPOSITION_QUALIFICATION_REPORT_SCHEMA_VERSION: &str =
    "project-editor-composition-qualification-report.v1";

pub fn qualify_and_seal_project_editor_composition_headless(
    project_root: &Path,
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    identity: ProjectEditorCompositionIdentity,
) -> ProjectEditorCompositionQualificationReport {
    let mut report = qualify_project_editor_composition_headless(
        project_root,
        linked_project_runtimes,
        identity.clone(),
    );
    if report.status == "passed" {
        let result = current_project_editor_composition_artifact(&identity).and_then(|artifact| {
            let evidence_root = artifact
                .descriptor_path
                .parent()
                .ok_or_else(|| {
                    "project_editor_composition.qualification_seal_artifact_root_missing"
                        .to_string()
                })?
                .join("qualification");
            persist_passed_qualification_and_seal(
                &artifact,
                &report,
                &report.status,
                &report.composition_identity_digest,
                &evidence_root.join("qualification-report.json"),
                &evidence_root.join("qualification-seal.json"),
            )
            .map(|_| ())
        });
        if let Err(error) = result {
            report.status = "failed".to_string();
            report.diagnostics.push(error);
        }
    }
    report
}

#[cfg(feature = "real-window")]
pub fn qualify_and_seal_project_editor_composition_real_window(
    request_path: &Path,
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    identity: ProjectEditorCompositionIdentity,
) -> ProjectEditorCompositionRealWindowQualificationReport {
    let mut report = qualify_project_editor_composition_real_window(
        request_path,
        linked_project_runtimes,
        identity.clone(),
    );
    if report.status == "passed" {
        let result = (|| {
            let request: ProjectEditorCompositionRealWindowQualificationRequest =
                serde_json::from_slice(
                    &std::fs::read(request_path).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
            let artifact = current_project_editor_composition_artifact(&identity)?;
            persist_passed_qualification_and_seal(
                &artifact,
                &report,
                &report.status,
                &report.composition_identity_digest,
                &request.evidence_root.join("gate-h-real-window-report.json"),
                &request.evidence_root.join("qualification-seal.json"),
            )
            .map(|_| ())
        })();
        if let Err(error) = result {
            report.status = "failed".to_string();
            report.diagnostics.push(error);
        }
    }
    report
}

fn persist_passed_qualification_and_seal<T: Serialize>(
    artifact: &ProjectEditorCompositionArtifact,
    report: &T,
    status: &str,
    composition_identity_digest: &str,
    report_path: &Path,
    seal_path: &Path,
) -> Result<ProjectEditorCompositionQualificationSeal, String> {
    if status != "passed" {
        return Err(
            "project_editor_composition.qualification_seal_requires_passed_report".to_string(),
        );
    }
    if artifact.descriptor.identity_digest != composition_identity_digest {
        return Err("project_editor_composition.qualification_seal_identity_mismatch".to_string());
    }
    let parent = report_path.parent().ok_or_else(|| {
        "project_editor_composition.qualification_seal_evidence_root_missing".to_string()
    })?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(
        report_path,
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ProjectEditorCompositionArtifact::seal_qualification(artifact, report_path, seal_path)
}

fn current_project_editor_composition_artifact(
    identity: &ProjectEditorCompositionIdentity,
) -> Result<ProjectEditorCompositionArtifact, String> {
    let executable_path = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|error| error.to_string())?;
    let artifact_root = executable_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            "project_editor_composition.qualification_seal_artifact_root_missing".to_string()
        })?
        .to_path_buf();
    let descriptor_path = artifact_root.join("composition-descriptor.json");
    let descriptor: editor_core::ProjectEditorCompositionDescriptor = serde_json::from_slice(
        &std::fs::read(&descriptor_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if &descriptor.identity != identity {
        return Err("project_editor_composition.qualification_seal_identity_mismatch".to_string());
    }
    Ok(ProjectEditorCompositionArtifact {
        schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
        executable_path,
        descriptor_path,
        build_report_path: artifact_root.join("build-report.json"),
        descriptor,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEditorCompositionQualificationStep {
    pub command_id: String,
    pub status: String,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEditorCompositionQualificationReport {
    pub schema_version: String,
    pub status: String,
    pub project_id: String,
    pub module_id: String,
    pub composition_identity_digest: String,
    pub linked_aot_content_digest: String,
    pub initial_frame_count: Option<u64>,
    pub paused_frame_count: Option<u64>,
    pub stepped_frame_count: Option<u64>,
    pub step_count: Option<u64>,
    pub resumed_frame_count: Option<u64>,
    pub stopped: bool,
    pub steps: Vec<ProjectEditorCompositionQualificationStep>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEditorCompositionRealLifecycleEvidence {
    pub status: String,
    pub initial_present: Option<editor_core::GameViewPresentReport>,
    pub aui_action_present: Option<editor_core::GameViewPresentReport>,
    pub next_frame_present: Option<editor_core::GameViewPresentReport>,
    pub paused_present: Option<editor_core::GameViewPresentReport>,
    pub stepped_present: Option<editor_core::GameViewPresentReport>,
    pub resumed_present: Option<editor_core::GameViewPresentReport>,
    pub stopped_present: Option<editor_core::GameViewPresentReport>,
    pub aui_action_id: String,
    pub aui_action_handled: bool,
    pub next_frame_snapshot_changed: bool,
    pub stopped: bool,
    pub diagnostics: Vec<String>,
}

#[cfg(feature = "real-window")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionRealWindowQualificationRequest {
    pub schema_version: String,
    pub project_root: PathBuf,
    pub evidence_root: PathBuf,
    pub physical_width: u32,
    pub physical_height: u32,
}

#[cfg(feature = "real-window")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEditorCompositionRealWindowQualificationReport {
    pub schema_version: String,
    pub status: String,
    pub project_id: String,
    pub module_id: String,
    pub composition_identity_digest: String,
    pub window_report: crate::RealNativeEditorWindowReport,
    pub native_window_id: Option<String>,
    pub scale_factor: f64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub screenshot_path: Option<String>,
    pub screenshot_hash: Option<String>,
    pub screenshot_nontransparent_pixels: u64,
    pub game_view_capture_path: Option<String>,
    pub game_view_capture_hash: Option<String>,
    pub game_view_distinct_color_count: usize,
    pub game_view_non_dominant_pixel_count: u64,
    pub input_replay: Option<crate::EditorInputReplayEvidence>,
    pub game_view_present_report: Option<editor_core::GameViewPresentReport>,
    pub active_runtime_after_play: bool,
    pub active_runtime_package_visible: bool,
    pub runtime_inspector_temporary: bool,
    pub project_lifecycle: Option<ProjectEditorCompositionRealLifecycleEvidence>,
    pub diagnostics: Vec<String>,
}

#[cfg(feature = "real-window")]
pub fn qualify_project_editor_composition_real_window(
    request_path: &Path,
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    identity: ProjectEditorCompositionIdentity,
) -> ProjectEditorCompositionRealWindowQualificationReport {
    let request: ProjectEditorCompositionRealWindowQualificationRequest = serde_json::from_slice(
        &std::fs::read(request_path).expect("real-window qualification request must be readable"),
    )
    .expect("real-window qualification request must be valid");
    assert_eq!(
        request.schema_version,
        "project-editor-composition-real-window-request.v1"
    );
    std::fs::create_dir_all(&request.evidence_root).unwrap();
    let outcome = crate::run_real_project_editor_composition_authority(
        crate::RealProjectEditorCompositionAuthorityOptions {
            authority: crate::RealNativeEditorAuthorityOptions {
                physical_width: request.physical_width,
                physical_height: request.physical_height,
                report_level: crate::EditorReachabilityReportLevel::Trace,
                project_root: Some(request.project_root.clone()),
                workspace_layout_store_root: Some(request.evidence_root.join("workspace-state")),
                click_widget_id: Some("editor/shell/toolbar/play".to_string()),
                wheel_delta: None,
                drag_target_widget_id: None,
                drag_delta: None,
                scenario_path: None,
            },
            linked_project_runtimes,
            identity: identity.clone(),
        },
    );
    let mut diagnostics = Vec::new();
    let (screenshot_path, screenshot_hash, screenshot_nontransparent_pixels) =
        if let Some(capture) = outcome.capture.as_ref() {
            let path = request.evidence_root.join("tower-defense-real-window.png");
            match write_rgba_png(&path, capture.width, capture.height, &capture.rgba8) {
                Ok(()) => (
                    Some(path.display().to_string()),
                    Some(sha256_prefixed(&capture.rgba8)),
                    capture
                        .rgba8
                        .chunks_exact(4)
                        .filter(|pixel| pixel[3] != 0)
                        .count() as u64,
                ),
                Err(error) => {
                    diagnostics.push(format!(
                        "project_editor_composition.real_window_screenshot_write_failed:{error}"
                    ));
                    (None, None, 0)
                }
            }
        } else {
            diagnostics.push("project_editor_composition.real_window_capture_missing".to_string());
            (None, None, 0)
        };
    let input_passed = outcome.input_replay.as_ref().is_some_and(|input| {
        input.route_status == crate::EditorReachabilityStatus::Passed
            && input.foreground_verified
            && input.pointer_down_observed
            && input.pointer_up_observed
            && input.after_command_id.as_deref() == Some("play")
    });
    let (
        game_view_capture_path,
        game_view_capture_hash,
        game_view_distinct_color_count,
        game_view_non_dominant_pixel_count,
    ) = if let Some(capture) = outcome.game_view_capture.as_ref() {
        let path = request.evidence_root.join("tower-defense-game-view.png");
        let colors = capture
            .rgba8
            .chunks_exact(4)
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
            .fold(std::collections::BTreeMap::new(), |mut colors, pixel| {
                *colors.entry(pixel).or_insert(0u64) += 1;
                colors
            });
        let dominant = colors.values().copied().max().unwrap_or(0);
        let pixel_count = capture.rgba8.len() as u64 / 4;
        match write_rgba_png(&path, capture.width, capture.height, &capture.rgba8) {
            Ok(()) => (
                Some(path.display().to_string()),
                Some(sha256_prefixed(&capture.rgba8)),
                colors.len(),
                pixel_count.saturating_sub(dominant),
            ),
            Err(error) => {
                diagnostics.push(format!(
                    "project_editor_composition.game_view_capture_write_failed:{error}"
                ));
                (
                    None,
                    None,
                    colors.len(),
                    pixel_count.saturating_sub(dominant),
                )
            }
        }
    } else {
        diagnostics.push("project_editor_composition.game_view_capture_missing".to_string());
        (None, None, 0, 0)
    };
    let game_view_pixels_passed =
        game_view_distinct_color_count >= 4 && game_view_non_dominant_pixel_count >= 1_000;
    let gpu_passed = outcome
        .game_view_present_report
        .as_ref()
        .is_some_and(|report| {
            report.gpu_present_status == "presented"
                && report.texture_descriptor_status != "not_started"
        });
    let lifecycle_passed = outcome
        .project_lifecycle
        .as_ref()
        .is_some_and(|lifecycle| lifecycle.status == "passed");
    let passed = outcome.window_report.present_status == "presented"
        && outcome.window_report.window_created
        && outcome.window_report.surface_created
        && outcome.window_report.surface_configured
        && outcome.window_report.device_created
        && outcome.window_report.viewport_texture_registry_count > 0
        && screenshot_nontransparent_pixels > 0
        && game_view_pixels_passed
        && input_passed
        && gpu_passed
        && outcome.active_runtime_after_play
        && outcome.active_runtime_package_visible
        && outcome.runtime_inspector_temporary
        && lifecycle_passed;
    if !input_passed {
        diagnostics.push("project_editor_composition.real_window_input_failed".to_string());
    }
    if !gpu_passed {
        diagnostics.push("project_editor_composition.real_window_gpu_present_failed".to_string());
    }
    if !game_view_pixels_passed {
        diagnostics.push(
            "project_editor_composition.real_window_game_view_pixels_insufficient".to_string(),
        );
    }
    if outcome.window_report.viewport_texture_registry_count == 0 {
        diagnostics.push(
            "project_editor_composition.real_window_viewport_texture_registry_empty".to_string(),
        );
    }
    if !outcome.active_runtime_package_visible || !outcome.runtime_inspector_temporary {
        diagnostics
            .push("project_editor_composition.real_window_runtime_inspector_failed".to_string());
    }
    if !lifecycle_passed {
        diagnostics.push("project_editor_composition.real_window_lifecycle_failed".to_string());
    }
    let composition_identity_digest = identity.digest().unwrap_or_default();
    let report = ProjectEditorCompositionRealWindowQualificationReport {
        schema_version: "project-editor-composition-real-window-report.v1".to_string(),
        status: if passed { "passed" } else { "failed" }.to_string(),
        project_id: identity.project_id,
        module_id: identity.module_id,
        composition_identity_digest,
        window_report: outcome.window_report,
        native_window_id: outcome.native_window_id,
        scale_factor: outcome.scale_factor,
        physical_width: outcome.physical_width,
        physical_height: outcome.physical_height,
        screenshot_path,
        screenshot_hash,
        screenshot_nontransparent_pixels,
        game_view_capture_path,
        game_view_capture_hash,
        game_view_distinct_color_count,
        game_view_non_dominant_pixel_count,
        input_replay: outcome.input_replay,
        game_view_present_report: outcome.game_view_present_report,
        active_runtime_after_play: outcome.active_runtime_after_play,
        active_runtime_package_visible: outcome.active_runtime_package_visible,
        runtime_inspector_temporary: outcome.runtime_inspector_temporary,
        project_lifecycle: outcome.project_lifecycle,
        diagnostics,
    };
    std::fs::write(
        request.evidence_root.join("gate-h-real-window-report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    report
}

#[cfg(feature = "real-window")]
pub fn project_editor_composition_real_window_report_json(
    report: &ProjectEditorCompositionRealWindowQualificationReport,
) -> Result<String, String> {
    serde_json::to_string(report).map_err(|error| error.to_string())
}

#[cfg(feature = "real-window")]
fn write_rgba_png(path: &Path, width: u32, height: u32, rgba8: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba8))
        .map_err(|error| error.to_string())
}

pub(crate) fn qualify_active_project_editor_composition_lifecycle(
    session: &mut EditorSession,
) -> ProjectEditorCompositionRealLifecycleEvidence {
    let initial_present = session.last_game_view_present_report().cloned();
    let initial_frame = initial_present
        .as_ref()
        .map(|report| report.frame_count)
        .unwrap_or(0);
    let mut runtime_input =
        RuntimeInputFrame::new(initial_frame.saturating_add(1), "EditorGameView");
    runtime_input.events.push(RuntimeInputEvent::PointerDown {
        x: 100.0,
        y: 520.0,
        button: RuntimePointerButton::Primary,
    });
    runtime_input.events.push(RuntimeInputEvent::PointerUp {
        x: 100.0,
        y: 520.0,
        button: RuntimePointerButton::Primary,
    });
    let aui_action_present =
        session.tick_active_game_view_runtime_descriptor_frame_with_input(runtime_input);
    let aui_action_id = "td.recruit".to_string();
    let aui_action_handled = aui_action_present
        .as_ref()
        .filter(|report| report.aui_consumed_event_count > 0)
        .and_then(|report| report.project_runtime_session_report.as_ref())
        .is_some_and(|report| {
            report
                .stages
                .iter()
                .any(summary_proves_single_aui_action_handled)
        });
    let next_frame_present = session.tick_active_game_view_runtime_descriptor_frame();
    let next_frame_snapshot_changed = aui_action_present
        .as_ref()
        .and_then(|report| report.last_frame_hash.as_ref())
        .zip(
            next_frame_present
                .as_ref()
                .and_then(|report| report.last_frame_hash.as_ref()),
        )
        .is_some_and(|(before, after)| before != after);

    let mut command_report = ProjectEditorCompositionQualificationReport {
        schema_version: PROJECT_EDITOR_COMPOSITION_QUALIFICATION_REPORT_SCHEMA_VERSION.to_string(),
        status: "running".to_string(),
        project_id: String::new(),
        module_id: String::new(),
        composition_identity_digest: String::new(),
        linked_aot_content_digest: String::new(),
        initial_frame_count: None,
        paused_frame_count: None,
        stepped_frame_count: None,
        step_count: None,
        resumed_frame_count: None,
        stopped: false,
        steps: Vec::new(),
        diagnostics: Vec::new(),
    };
    let pause_committed = execute(session, UiCommandPayload::Pause, &mut command_report);
    let paused_present = session.last_game_view_present_report().cloned();
    let step_committed = execute(session, UiCommandPayload::StepFrame, &mut command_report);
    let stepped_present = session.last_game_view_present_report().cloned();
    let resume_committed = execute(session, UiCommandPayload::Play, &mut command_report);
    let resumed_present = session.last_game_view_present_report().cloned();
    let stop_committed = execute(
        session,
        UiCommandPayload::StopPlaySession,
        &mut command_report,
    );
    let stopped_present = session.last_game_view_present_report().cloned();
    let stopped = !session.has_active_editor_runtime_play_instance();
    let lifecycle_valid = paused_present.as_ref().map(|report| report.frame_count)
        == next_frame_present.as_ref().map(|report| report.frame_count)
        && stepped_present.as_ref().map(|report| report.frame_count)
            == next_frame_present
                .as_ref()
                .map(|report| report.frame_count.saturating_add(1))
        && stepped_present.as_ref().map(|report| report.step_count) == Some(1)
        && resumed_present.as_ref().map(|report| report.frame_count)
            == stepped_present.as_ref().map(|report| report.frame_count)
        && pause_committed
        && step_committed
        && resume_committed
        && stop_committed
        && stopped;
    let passed = initial_present.is_some()
        && aui_action_handled
        && next_frame_snapshot_changed
        && lifecycle_valid;
    let mut diagnostics = command_report.diagnostics;
    if !aui_action_handled {
        diagnostics
            .push("project_editor_composition.real_window_aui_action_not_handled".to_string());
    }
    if !next_frame_snapshot_changed {
        diagnostics
            .push("project_editor_composition.real_window_next_snapshot_unchanged".to_string());
    }
    if !lifecycle_valid {
        diagnostics.push("project_editor_composition.real_window_lifecycle_mismatch".to_string());
    }
    ProjectEditorCompositionRealLifecycleEvidence {
        status: if passed { "passed" } else { "failed" }.to_string(),
        initial_present,
        aui_action_present,
        next_frame_present,
        paused_present,
        stepped_present,
        resumed_present,
        stopped_present,
        aui_action_id,
        aui_action_handled,
        next_frame_snapshot_changed,
        stopped,
        diagnostics,
    }
}

fn summary_proves_single_aui_action_handled(stage: &ProjectRuntimeSessionStageReport) -> bool {
    stage.stage == ProjectRuntimeSessionStage::AuiActionDispatch
        && stage.action_count == 1
        && stage.handled_action_count == 1
        && stage.unhandled_action_count == 0
        && stage.rejected_action_count == 0
        && stage.staged_mutation_count > 0
        && stage.committed_mutation_count == stage.staged_mutation_count
        && stage.rejected_mutation_count == 0
        && !stage.terminal_fault
}

pub(crate) fn qualify_active_project_editor_composition_runtime_inspector(
    session: &mut EditorSession,
) -> bool {
    let payload = UiCommandPayload::SelectRuntimeEntity {
        entity_id: "entity-tower-match-config".to_string(),
    };
    let result = session.execute_command(UiCommand {
        command_id: ui_command_id_for_payload(&payload).to_string(),
        source: UiCommandSource::Test,
        request_id: "gate-h-select-runtime-inspector".to_string(),
        payload,
    });
    result.status == CommandStatus::Committed
}

pub fn qualify_project_editor_composition_headless(
    project_root: &Path,
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    identity: ProjectEditorCompositionIdentity,
) -> ProjectEditorCompositionQualificationReport {
    let identity_digest = identity.digest().unwrap_or_default();
    let linked_aot_content_digest = linked_project_runtimes
        .only_descriptor()
        .map(|descriptor| descriptor.aot_content_digest.clone())
        .unwrap_or_default();
    let mut report = ProjectEditorCompositionQualificationReport {
        schema_version: PROJECT_EDITOR_COMPOSITION_QUALIFICATION_REPORT_SCHEMA_VERSION.to_string(),
        status: "failed".to_string(),
        project_id: identity.project_id.clone(),
        module_id: identity.module_id.clone(),
        composition_identity_digest: identity_digest,
        linked_aot_content_digest,
        initial_frame_count: None,
        paused_frame_count: None,
        stepped_frame_count: None,
        step_count: None,
        resumed_frame_count: None,
        stopped: false,
        steps: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut session =
        match EditorSession::with_project_editor_composition(linked_project_runtimes, identity) {
            Ok(session) => session,
            Err(error) => {
                report.diagnostics.push(error.to_string());
                return report;
            }
        };

    if !execute(
        &mut session,
        UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        },
        &mut report,
    ) || !execute(&mut session, UiCommandPayload::Play, &mut report)
    {
        return report;
    }
    report.initial_frame_count = session
        .last_game_view_present_report()
        .map(|value| value.frame_count);
    if !execute(&mut session, UiCommandPayload::Pause, &mut report) {
        return report;
    }
    report.paused_frame_count = session
        .last_game_view_present_report()
        .map(|value| value.frame_count);
    if !execute(&mut session, UiCommandPayload::StepFrame, &mut report) {
        return report;
    }
    if let Some(value) = session.last_game_view_present_report() {
        report.stepped_frame_count = Some(value.frame_count);
        report.step_count = Some(value.step_count);
    }
    if !execute(&mut session, UiCommandPayload::Play, &mut report) {
        return report;
    }
    report.resumed_frame_count = session
        .last_game_view_present_report()
        .map(|value| value.frame_count);
    if !execute(&mut session, UiCommandPayload::StopPlaySession, &mut report) {
        return report;
    }
    report.stopped = !session.has_active_editor_runtime_play_instance();
    let lifecycle_valid = report.initial_frame_count.is_some()
        && report.paused_frame_count == report.initial_frame_count
        && report.stepped_frame_count
            == report
                .initial_frame_count
                .map(|frame| frame.saturating_add(1))
        && report.step_count == Some(1)
        && report.resumed_frame_count == report.stepped_frame_count
        && report.stopped;
    if lifecycle_valid {
        report.status = "passed".to_string();
    } else {
        report
            .diagnostics
            .push("project_editor_composition.qualification_lifecycle_mismatch".to_string());
    }
    report
}

pub fn project_editor_composition_qualification_report_json(
    report: &ProjectEditorCompositionQualificationReport,
) -> Result<String, String> {
    serde_json::to_string(report).map_err(|error| error.to_string())
}

fn execute(
    session: &mut EditorSession,
    payload: UiCommandPayload,
    report: &mut ProjectEditorCompositionQualificationReport,
) -> bool {
    let command_id = ui_command_id_for_payload(&payload).to_string();
    eprintln!("project_editor_composition.qualification.begin:{command_id}");
    let result = session.execute_command(UiCommand {
        command_id: command_id.clone(),
        source: UiCommandSource::Test,
        request_id: format!("composition-qualification-{command_id}"),
        payload,
    });
    let committed = result.status == CommandStatus::Committed;
    let diagnostic_codes = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();
    if !committed {
        report.diagnostics.extend(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message)),
        );
    }
    report
        .steps
        .push(ProjectEditorCompositionQualificationStep {
            command_id: command_id.clone(),
            status: format!("{:?}", result.status).to_ascii_lowercase(),
            diagnostic_codes,
        });
    eprintln!(
        "project_editor_composition.qualification.end:{command_id}:{}",
        if committed { "committed" } else { "failed" }
    );
    committed
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_core::{
        GeneratedCompositionLockLineage, ProjectEditorCompositionBuildDeadlinePolicy,
        ProjectEditorCompositionBuildQosPolicy, ProjectEditorCompositionBuildReport,
        ProjectEditorCompositionBuildRequest, ProjectEditorCompositionBuildSourceKind,
        ProjectEditorCompositionBuildStatus, ProjectEditorCompositionCachePolicy,
        ProjectEditorCompositionCacheStatus, ProjectEditorCompositionCompilationCacheAffinity,
        ProjectEditorCompositionDescriptor, ProjectEditorCompositionPreparationControl,
        ProjectEditorCompositionProcessPriority, ProjectEditorCompositionPromotionRequest,
        ProjectEditorCompositionPromotionStatus, ProjectEditorCompositionResolvedIdentity,
        ProjectRuntimeTrustInspection, GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION,
    };
    use engine_runtime::canonical_digest::sha256_prefixed;
    use engine_runtime::project_runtime_session::ProjectRuntimeSessionStatus;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static QUALIFICATION_FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn project_editor_composition_282_gate_h_source_request(
        project_root: PathBuf,
        engine_sdk_root: PathBuf,
        build_root: PathBuf,
        expected_identity: ProjectEditorCompositionIdentity,
    ) -> ProjectEditorCompositionBuildRequest {
        ProjectEditorCompositionBuildRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root,
            engine_sdk_root,
            build_root,
            expected_identity,
            cache_policy: ProjectEditorCompositionCachePolicy::default(),
            qos_policy: ProjectEditorCompositionBuildQosPolicy::default(),
            deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy::default(),
            cargo_executable: None,
            cargo_identity: "cargo 282-gate-h-candidate".to_string(),
            capture_limit_bytes: 256 * 1024,
        }
    }

    fn project_editor_composition_normal_hit_request(
        source: &ProjectEditorCompositionBuildRequest,
        build_root: PathBuf,
        cargo_executable: PathBuf,
    ) -> ProjectEditorCompositionBuildRequest {
        let mut request = source.clone();
        request.build_root = build_root;
        request.cargo_executable = Some(cargo_executable);
        request
    }

    fn resolved_identity(
        identity: &ProjectEditorCompositionIdentity,
    ) -> ProjectEditorCompositionResolvedIdentity {
        ProjectEditorCompositionResolvedIdentity::new(
            identity.digest().unwrap(),
            &GeneratedCompositionLockLineage {
                schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
                lock_input_digest: format!("sha256:{}", "1".repeat(64)),
                raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
                resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
            },
        )
        .unwrap()
    }

    fn aui_dispatch_stage() -> ProjectRuntimeSessionStageReport {
        ProjectRuntimeSessionStageReport {
            stage: ProjectRuntimeSessionStage::AuiActionDispatch,
            status: ProjectRuntimeSessionStatus::Applied,
            action_count: 1,
            handled_action_count: 1,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            staged_mutation_count: 1,
            committed_mutation_count: 1,
            rejected_mutation_count: 0,
            diagnostics: Vec::new(),
            action_trace: Vec::new(),
            terminal_fault: false,
        }
    }

    #[test]
    fn qualification_accepts_summary_level_handled_aui_dispatch_without_trace() {
        assert!(summary_proves_single_aui_action_handled(
            &aui_dispatch_stage()
        ));
    }

    #[test]
    fn project_editor_composition_normal_hit_request_preserves_cargo_identity() {
        let source = project_editor_composition_282_gate_h_source_request(
            PathBuf::from("project"),
            PathBuf::from("sdk"),
            PathBuf::from("source-build"),
            ProjectEditorCompositionIdentity {
                schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
                project_id: "fixture.project".to_string(),
                module_id: "fixture.runtime".to_string(),
                interface_version: "project-runtime-module.v2".to_string(),
                aot_content_digest: format!("sha256:{}", "a".repeat(64)),
                editor_build_identity: format!("sha256:{}", "b".repeat(64)),
                engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
                toolchain_identity: "rustc-fixture".to_string(),
                target_triple: "x86_64-pc-windows-msvc".to_string(),
                profile: "release".to_string(),
                normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
                normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
                dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
            },
        );
        let hit = project_editor_composition_normal_hit_request(
            &source,
            PathBuf::from("destination-cache"),
            PathBuf::from("must-not-start-cargo.exe"),
        );

        assert_eq!(hit.cargo_identity, source.cargo_identity);
        assert_eq!(hit.expected_identity, source.expected_identity);
        assert_eq!(hit.project_root, source.project_root);
        assert_eq!(hit.engine_sdk_root, source.engine_sdk_root);
        assert_eq!(hit.schema_version, source.schema_version);
        assert_eq!(hit.cache_policy, source.cache_policy);
        assert_eq!(hit.qos_policy, source.qos_policy);
        assert_eq!(hit.deadline_policy, source.deadline_policy);
        assert_eq!(hit.capture_limit_bytes, source.capture_limit_bytes);
        assert_eq!(hit.build_root, PathBuf::from("destination-cache"));
        assert_eq!(
            hit.cargo_executable,
            Some(PathBuf::from("must-not-start-cargo.exe"))
        );
    }

    #[test]
    fn qualification_rejects_aui_dispatch_with_rejected_mutation() {
        let mut stage = aui_dispatch_stage();
        stage.committed_mutation_count = 0;
        stage.rejected_mutation_count = 1;

        assert!(!summary_proves_single_aui_action_handled(&stage));
    }

    #[test]
    fn project_editor_composition_qualification_seal_passed_report_binds_candidate() {
        let (root, artifact, report) = qualification_seal_fixture("passed");
        let report_path = root.join("evidence").join("qualification-report.json");
        let seal_path = root.join("evidence").join("qualification-seal.json");

        let seal = persist_passed_qualification_and_seal(
            &artifact,
            &report,
            &report.status,
            &report.composition_identity_digest,
            &report_path,
            &seal_path,
        )
        .unwrap();

        assert_eq!(
            seal.schema_version,
            PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION
        );
        assert_eq!(
            seal.executable_hash,
            sha256_prefixed(b"qualified-generated-editor")
        );
        assert!(report_path.is_file());
        assert!(seal_path.is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn project_editor_composition_qualification_seal_failed_report_emits_nothing() {
        let (root, artifact, mut report) = qualification_seal_fixture("failed");
        report.status = "failed".to_string();
        let report_path = root.join("evidence").join("qualification-report.json");
        let seal_path = root.join("evidence").join("qualification-seal.json");

        let error = persist_passed_qualification_and_seal(
            &artifact,
            &report,
            &report.status,
            &report.composition_identity_digest,
            &report_path,
            &seal_path,
        )
        .unwrap_err();

        assert!(error.contains("qualification_seal_requires_passed_report"));
        assert!(!report_path.exists());
        assert!(!seal_path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "runs the 282 C2 fresh controlled build, qualification, promotion, and cache-hit integration"]
    fn project_editor_composition_282_c2_fresh_gate_g() {
        let run_root = absolute_environment_path("AIFE_282_C2_RUN_ROOT");
        let project_root = absolute_environment_path("AIFE_282_C2_PROJECT_ROOT");
        assert!(project_root.starts_with(&run_root));
        let source_build_root = run_root.join("source-qualified");
        let destination_build_root = run_root.join("destination-cache");
        let backup_root = run_root.join("backup");
        let evidence_root = run_root.join("evidence");
        for path in [
            &source_build_root,
            &destination_build_root,
            &backup_root,
            &evidence_root,
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let editor_build_identity = crate::current_editor_build_identity().unwrap();
        let inspection = ProjectRuntimeTrustInspection::inspect(
            &project_root,
            &sdk_root,
            editor_build_identity.clone(),
        )
        .unwrap();
        let identity = crate::composition_identity(
            &project_root,
            &sdk_root,
            &inspection,
            &editor_build_identity,
        )
        .unwrap();
        let source_request = ProjectEditorCompositionBuildRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root: project_root.clone(),
            engine_sdk_root: sdk_root.clone(),
            build_root: source_build_root.clone(),
            expected_identity: identity.clone(),
            cache_policy: ProjectEditorCompositionCachePolicy::default(),
            qos_policy: ProjectEditorCompositionBuildQosPolicy::default(),
            deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy::default(),
            cargo_executable: None,
            cargo_identity: "cargo fresh-integration".to_string(),
            capture_limit_bytes: 256 * 1024,
        };
        let source_report = ProjectEditorCompositionArtifact::prepare(
            source_request,
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(
            &evidence_root.join("source-build-report.json"),
            &source_report,
        );
        assert_eq!(
            source_report.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        let artifact = source_report.artifact.clone().unwrap();
        let qualification = std::process::Command::new(&artifact.executable_path)
            .arg("--qualify-project-runtime")
            .arg(&project_root)
            .output()
            .unwrap();
        std::fs::write(
            evidence_root.join("qualification.stdout.log"),
            &qualification.stdout,
        )
        .unwrap();
        std::fs::write(
            evidence_root.join("qualification.stderr.log"),
            &qualification.stderr,
        )
        .unwrap();
        assert!(qualification.status.success());
        let source_artifact_root = artifact.descriptor_path.parent().unwrap().to_path_buf();
        let seal_path = source_artifact_root
            .join("qualification")
            .join("qualification-seal.json");
        assert!(seal_path.is_file());
        let promotion_request = ProjectEditorCompositionPromotionRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION.to_string(),
            authority_operation_id: "282-c2-fresh-exact".to_string(),
            authorized_run_root: run_root.clone(),
            source_artifact_root,
            destination_cache_root: destination_build_root.clone(),
            backup_root,
            qualification_seal_path: seal_path,
            expected_identity: identity.clone(),
            expected_resolved_identity: artifact.descriptor.resolved_identity.clone(),
        };
        write_json(
            &evidence_root.join("promotion-request.json"),
            &promotion_request,
        );
        let promoted = ProjectEditorCompositionArtifact::promote_exact(promotion_request.clone());
        write_json(&evidence_root.join("promotion-report.json"), &promoted);
        assert_eq!(
            promoted.status,
            ProjectEditorCompositionPromotionStatus::Promoted
        );
        let no_op = ProjectEditorCompositionArtifact::promote_exact(promotion_request);
        write_json(&evidence_root.join("promotion-noop-report.json"), &no_op);
        assert_eq!(
            no_op.status,
            ProjectEditorCompositionPromotionStatus::ExactCacheHit
        );
        let cache_hit = ProjectEditorCompositionArtifact::prepare(
            ProjectEditorCompositionBuildRequest {
                schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
                project_root,
                engine_sdk_root: sdk_root,
                build_root: destination_build_root,
                expected_identity: identity,
                cache_policy: ProjectEditorCompositionCachePolicy::default(),
                qos_policy: ProjectEditorCompositionBuildQosPolicy::default(),
                deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy::default(),
                cargo_executable: Some(run_root.join("must-not-start-cargo.exe")),
                cargo_identity: "cargo fresh-integration".to_string(),
                capture_limit_bytes: 256 * 1024,
            },
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(
            &evidence_root.join("normal-prepare-cache-hit-report.json"),
            &cache_hit,
        );
        assert_eq!(
            cache_hit.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        assert_eq!(
            cache_hit.source_kind,
            ProjectEditorCompositionBuildSourceKind::ExactCache
        );
        assert!(cache_hit.steps.is_empty());
    }

    #[test]
    #[ignore = "runs the 282-R1 fresh A/B lineage, path-affinity, qualification, and promotion matrix"]
    fn project_editor_composition_282_r1_fresh_gate_r_e_through_r_g() {
        let run_root = absolute_environment_path("AIFE_282_R1_RUN_ROOT");
        let project_root = absolute_environment_path("AIFE_282_R1_PROJECT_ROOT");
        assert!(project_root.starts_with(&run_root));
        let root_a = run_root.join("root-a");
        let root_b = run_root.join("root-b");
        let copied_ct_negative = run_root.join("copied-ct-negative");
        let destination = run_root.join("destination-cache");
        let backup = run_root.join("backup");
        let evidence = run_root.join("evidence");
        for path in [
            &root_a,
            &root_b,
            &copied_ct_negative,
            &destination,
            &backup,
            &evidence,
        ] {
            std::fs::create_dir_all(path).unwrap();
        }

        let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let editor_build_identity = crate::current_editor_build_identity().unwrap();
        let inspection = ProjectRuntimeTrustInspection::inspect(
            &project_root,
            &sdk_root,
            editor_build_identity.clone(),
        )
        .unwrap();
        let identity = crate::composition_identity(
            &project_root,
            &sdk_root,
            &inspection,
            &editor_build_identity,
        )
        .unwrap();
        let request = |build_root: PathBuf| ProjectEditorCompositionBuildRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root: project_root.clone(),
            engine_sdk_root: sdk_root.clone(),
            build_root,
            expected_identity: identity.clone(),
            cache_policy: ProjectEditorCompositionCachePolicy::default(),
            qos_policy: ProjectEditorCompositionBuildQosPolicy::default(),
            deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy::default(),
            cargo_executable: None,
            cargo_identity: "cargo 282-r1-fresh-integration".to_string(),
            capture_limit_bytes: 256 * 1024,
        };

        let report_a = ProjectEditorCompositionArtifact::prepare(
            request(root_a.clone()),
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(&evidence.join("root-a-build-report.json"), &report_a);
        assert_eq!(
            report_a.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        assert_eq!(
            report_a.compilation_cache_affinity,
            ProjectEditorCompositionCompilationCacheAffinity::Cold
        );

        let cache_a = root_a.join("project-editor-compositions");
        let cache_b = root_b.join("project-editor-compositions");
        copy_directory_tree(&cache_a.join("locks"), &cache_b.join("locks"));
        let report_b = ProjectEditorCompositionArtifact::prepare(
            request(root_b.clone()),
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(&evidence.join("root-b-build-report.json"), &report_b);
        assert_eq!(
            report_b.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        assert_eq!(
            report_b.compilation_cache_affinity,
            ProjectEditorCompositionCompilationCacheAffinity::Cold
        );
        assert!(report_b
            .steps
            .iter()
            .all(|step| step.stage != "generate_composition_lock"));
        assert_eq!(
            report_a
                .resolved_identity
                .as_ref()
                .unwrap()
                .resolved_graph_digest,
            report_b
                .resolved_identity
                .as_ref()
                .unwrap()
                .resolved_graph_digest
        );
        assert_eq!(report_a.resolved_identity, report_b.resolved_identity);

        let cache_negative = copied_ct_negative.join("project-editor-compositions");
        copy_directory_tree(&cache_a.join("locks"), &cache_negative.join("locks"));
        copy_directory_tree(&cache_a.join("ct"), &cache_negative.join("ct"));
        let report_negative = ProjectEditorCompositionArtifact::prepare(
            request(copied_ct_negative.clone()),
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(
            &evidence.join("copied-ct-negative-build-report.json"),
            &report_negative,
        );
        assert_eq!(
            report_negative.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        assert_eq!(
            report_negative.compilation_cache_affinity,
            ProjectEditorCompositionCompilationCacheAffinity::PathAffineMiss
        );
        assert_ne!(
            report_negative.canonical_target_anchor_digest,
            report_a.canonical_target_anchor_digest
        );
        assert!(report_negative
            .steps
            .iter()
            .all(|step| step.stage != "generate_composition_lock"));

        let artifact_b = report_b.artifact.clone().unwrap();
        let qualification = std::process::Command::new(&artifact_b.executable_path)
            .arg("--qualify-project-runtime")
            .arg(&project_root)
            .output()
            .unwrap();
        std::fs::write(
            evidence.join("qualification.stdout.log"),
            &qualification.stdout,
        )
        .unwrap();
        std::fs::write(
            evidence.join("qualification.stderr.log"),
            &qualification.stderr,
        )
        .unwrap();
        assert!(qualification.status.success());
        let source_artifact_root = artifact_b.descriptor_path.parent().unwrap().to_path_buf();
        let seal_path = source_artifact_root
            .join("qualification")
            .join("qualification-seal.json");
        assert!(seal_path.is_file());
        let promotion_request = ProjectEditorCompositionPromotionRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION.to_string(),
            authority_operation_id: "282-r1-fresh-exact".to_string(),
            authorized_run_root: run_root.clone(),
            source_artifact_root,
            destination_cache_root: destination.clone(),
            backup_root: backup,
            qualification_seal_path: seal_path,
            expected_identity: identity.clone(),
            expected_resolved_identity: artifact_b.descriptor.resolved_identity.clone(),
        };
        write_json(&evidence.join("promotion-request.json"), &promotion_request);
        let promoted = ProjectEditorCompositionArtifact::promote_exact(promotion_request.clone());
        write_json(&evidence.join("promotion-report.json"), &promoted);
        assert_eq!(
            promoted.status,
            ProjectEditorCompositionPromotionStatus::Promoted
        );
        let no_op = ProjectEditorCompositionArtifact::promote_exact(promotion_request);
        write_json(&evidence.join("promotion-noop-report.json"), &no_op);
        assert_eq!(
            no_op.status,
            ProjectEditorCompositionPromotionStatus::ExactCacheHit
        );

        let mut hit_request = request(destination);
        hit_request.cargo_executable = Some(run_root.join("must-not-start-cargo.exe"));
        let cache_hit = ProjectEditorCompositionArtifact::prepare(
            hit_request,
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(
            &evidence.join("normal-prepare-cache-hit-report.json"),
            &cache_hit,
        );
        assert_eq!(
            cache_hit.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        assert_eq!(
            cache_hit.source_kind,
            ProjectEditorCompositionBuildSourceKind::ExactCache
        );
        assert!(cache_hit.steps.is_empty());
        assert_eq!(
            cache_hit.resolved_identity,
            Some(artifact_b.descriptor.resolved_identity)
        );
    }

    #[test]
    #[ignore = "continues 282-R1 Gate R-F from an already qualified fresh root-b artifact"]
    fn project_editor_composition_282_r1_fresh_gate_r_f_continuation() {
        let run_root = absolute_environment_path("AIFE_282_R1_RUN_ROOT");
        let project_root = absolute_environment_path("AIFE_282_R1_PROJECT_ROOT");
        assert!(project_root.starts_with(&run_root));
        let root_b = run_root.join("root-b");
        let destination = run_root.join("destination-cache");
        let backup = run_root.join("backup");
        let evidence = run_root.join("evidence");
        let report_b: ProjectEditorCompositionBuildReport = serde_json::from_slice(
            &std::fs::read(evidence.join("root-b-build-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            report_b.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        let artifact_b = report_b.artifact.clone().unwrap();

        let qualification = std::process::Command::new(&artifact_b.executable_path)
            .arg("--qualify-project-runtime")
            .arg(&project_root)
            .output()
            .unwrap();
        std::fs::write(
            evidence.join("qualification-r-f.stdout.log"),
            &qualification.stdout,
        )
        .unwrap();
        std::fs::write(
            evidence.join("qualification-r-f.stderr.log"),
            &qualification.stderr,
        )
        .unwrap();
        assert!(qualification.status.success());

        let source_artifact_root = artifact_b.descriptor_path.parent().unwrap().to_path_buf();
        let seal_path = source_artifact_root
            .join("qualification")
            .join("qualification-seal.json");
        assert!(seal_path.is_file());
        let promotion_request = ProjectEditorCompositionPromotionRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION.to_string(),
            authority_operation_id: "282-r1-fresh-exact-r-f".to_string(),
            authorized_run_root: run_root.clone(),
            source_artifact_root,
            destination_cache_root: destination.clone(),
            backup_root: backup,
            qualification_seal_path: seal_path,
            expected_identity: artifact_b.descriptor.identity.clone(),
            expected_resolved_identity: artifact_b.descriptor.resolved_identity.clone(),
        };
        write_json(
            &evidence.join("promotion-r-f-request.json"),
            &promotion_request,
        );
        let promoted = ProjectEditorCompositionArtifact::promote_exact(promotion_request.clone());
        write_json(&evidence.join("promotion-r-f-report.json"), &promoted);
        assert_eq!(
            promoted.status,
            ProjectEditorCompositionPromotionStatus::Promoted
        );
        let no_op = ProjectEditorCompositionArtifact::promote_exact(promotion_request);
        write_json(&evidence.join("promotion-r-f-noop-report.json"), &no_op);
        assert_eq!(
            no_op.status,
            ProjectEditorCompositionPromotionStatus::ExactCacheHit
        );

        copy_directory_tree(
            &root_b.join("project-editor-compositions").join("locks"),
            &destination
                .join("project-editor-compositions")
                .join("locks"),
        );
        let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let mut hit_request = ProjectEditorCompositionBuildRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root,
            engine_sdk_root: sdk_root,
            build_root: destination,
            expected_identity: artifact_b.descriptor.identity.clone(),
            cache_policy: ProjectEditorCompositionCachePolicy::default(),
            qos_policy: ProjectEditorCompositionBuildQosPolicy::default(),
            deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy::default(),
            cargo_executable: None,
            cargo_identity: "cargo 282-r1-fresh-integration".to_string(),
            capture_limit_bytes: 256 * 1024,
        };
        hit_request.cargo_executable = Some(run_root.join("must-not-start-cargo.exe"));
        let cache_hit = ProjectEditorCompositionArtifact::prepare(
            hit_request,
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(
            &evidence.join("normal-prepare-r-f-cache-hit-report.json"),
            &cache_hit,
        );
        assert_eq!(
            cache_hit.status,
            ProjectEditorCompositionBuildStatus::Success
        );
        assert_eq!(
            cache_hit.source_kind,
            ProjectEditorCompositionBuildSourceKind::ExactCache
        );
        assert!(cache_hit.steps.is_empty());
        assert_eq!(
            cache_hit.resolved_identity,
            Some(artifact_b.descriptor.resolved_identity)
        );
    }

    #[test]
    #[ignore = "runs the separately authorized 282 Gate H candidate-bound source qualification"]
    fn project_editor_composition_282_gate_h_candidate_source() {
        let run_root = absolute_environment_path("AIFE_282_GATE_H_RUN_ROOT");
        let project_root = absolute_environment_path("AIFE_282_GATE_H_PROJECT_ROOT");
        let candidate = absolute_environment_path("AIFE_282_GATE_H_CANDIDATE");
        assert!(project_root.starts_with(&run_root));
        assert!(candidate.starts_with(&run_root));
        // Keep the Cargo target below the legacy Win32 MAX_PATH boundary.
        let source_build_root = run_root.join("c");
        let evidence = run_root.join("evidence");
        std::fs::create_dir_all(&source_build_root).unwrap();
        std::fs::create_dir_all(&evidence).unwrap();

        let candidate_bytes = std::fs::read(&candidate).unwrap();
        let editor_build_identity =
            engine_runtime::canonical_digest::sha256_prefixed(&candidate_bytes);
        let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let inspection = ProjectRuntimeTrustInspection::inspect(
            &project_root,
            &sdk_root,
            editor_build_identity.clone(),
        )
        .unwrap();
        let identity = crate::composition_identity(
            &project_root,
            &sdk_root,
            &inspection,
            &editor_build_identity,
        )
        .unwrap();
        let report = ProjectEditorCompositionArtifact::prepare(
            project_editor_composition_282_gate_h_source_request(
                project_root.clone(),
                sdk_root,
                source_build_root,
                identity,
            ),
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(
            &evidence.join("candidate-source-build-report.json"),
            &report,
        );
        assert_eq!(report.status, ProjectEditorCompositionBuildStatus::Success);
        let artifact = report.artifact.unwrap();
        assert_eq!(
            artifact.descriptor.identity.editor_build_identity,
            editor_build_identity
        );

        let qualification = std::process::Command::new(&artifact.executable_path)
            .arg("--qualify-project-runtime")
            .arg(&project_root)
            .output()
            .unwrap();
        std::fs::write(
            evidence.join("candidate-source-qualification.stdout.log"),
            &qualification.stdout,
        )
        .unwrap();
        std::fs::write(
            evidence.join("candidate-source-qualification.stderr.log"),
            &qualification.stderr,
        )
        .unwrap();
        assert!(qualification.status.success());
        let seal = artifact
            .descriptor_path
            .parent()
            .unwrap()
            .join("qualification/qualification-seal.json");
        assert!(seal.is_file());
    }

    #[test]
    #[ignore = "promotes the separately authorized 282 Gate H exact artifact into production cache"]
    fn project_editor_composition_282_gate_h_promote_and_normal_hit() {
        let run_root = absolute_environment_path("AIFE_282_GATE_H_RUN_ROOT");
        let project_root = absolute_environment_path("AIFE_282_GATE_H_PROJECT_ROOT");
        let destination = absolute_environment_path("AIFE_282_GATE_H_DESTINATION");
        let evidence = run_root.join("evidence");
        let report: ProjectEditorCompositionBuildReport = serde_json::from_slice(
            &std::fs::read(evidence.join("candidate-source-build-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report.status, ProjectEditorCompositionBuildStatus::Success);
        let artifact = report.artifact.unwrap();
        let source_artifact_root = artifact.descriptor_path.parent().unwrap().to_path_buf();
        let seal_path = source_artifact_root.join("qualification/qualification-seal.json");
        let request = ProjectEditorCompositionPromotionRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION.to_string(),
            authority_operation_id: "282-r1-gate-h-production-exact".to_string(),
            authorized_run_root: run_root.clone(),
            source_artifact_root,
            destination_cache_root: destination.clone(),
            backup_root: run_root.join("backups/cache"),
            qualification_seal_path: seal_path,
            expected_identity: artifact.descriptor.identity.clone(),
            expected_resolved_identity: artifact.descriptor.resolved_identity.clone(),
        };
        write_json(
            &evidence.join("production-promotion-request.json"),
            &request,
        );
        let promotion = ProjectEditorCompositionArtifact::promote_exact(request);
        write_json(
            &evidence.join("production-promotion-report.json"),
            &promotion,
        );
        assert_eq!(
            promotion.status,
            ProjectEditorCompositionPromotionStatus::Promoted
        );

        let source_request = project_editor_composition_282_gate_h_source_request(
            project_root,
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            run_root.join("c"),
            artifact.descriptor.identity,
        );
        let hit = ProjectEditorCompositionArtifact::prepare(
            project_editor_composition_normal_hit_request(
                &source_request,
                destination,
                run_root.join("must-not-start-cargo.exe"),
            ),
            ProjectEditorCompositionPreparationControl::default(),
        );
        write_json(&evidence.join("production-normal-hit-report.json"), &hit);
        assert_eq!(hit.status, ProjectEditorCompositionBuildStatus::Success);
        assert_eq!(
            hit.source_kind,
            ProjectEditorCompositionBuildSourceKind::ExactCache
        );
        assert!(hit.steps.is_empty());
        assert_eq!(
            hit.resolved_identity,
            Some(artifact.descriptor.resolved_identity)
        );
    }

    fn copy_directory_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory_tree(&source_path, &destination_path);
            } else {
                std::fs::copy(source_path, destination_path).unwrap();
            }
        }
    }

    fn absolute_environment_path(name: &str) -> PathBuf {
        let path = PathBuf::from(
            std::env::var_os(name)
                .unwrap_or_else(|| panic!("{name} must be set for the 282 C2 fresh integration")),
        );
        assert!(path.is_absolute());
        path
    }

    fn write_json<T: Serialize>(path: &Path, value: &T) {
        std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }

    fn qualification_seal_fixture(
        label: &str,
    ) -> (
        PathBuf,
        ProjectEditorCompositionArtifact,
        ProjectEditorCompositionQualificationReport,
    ) {
        let root = std::env::temp_dir().join(format!(
            "aife-282-qualification-seal-{label}-{}-{}",
            std::process::id(),
            QUALIFICATION_FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let artifact_root = root.join("source-qualified").join("artifact");
        std::fs::create_dir_all(artifact_root.join("bin")).unwrap();
        let executable_path = artifact_root.join("bin").join("candidate.exe");
        std::fs::write(&executable_path, b"qualified-generated-editor").unwrap();
        let identity = ProjectEditorCompositionIdentity {
            schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: "fixture.project".to_string(),
            module_id: "fixture.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            editor_build_identity: format!("sha256:{}", "b".repeat(64)),
            engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
            toolchain_identity: "rustc-fixture".to_string(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
            normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
            dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
        };
        let identity_digest = identity.digest().unwrap();
        let resolved_identity = resolved_identity(&identity);
        let descriptor = ProjectEditorCompositionDescriptor {
            schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: identity.clone(),
            identity_digest: identity_digest.clone(),
            resolved_identity: resolved_identity.clone(),
            executable_hash: sha256_prefixed(b"qualified-generated-editor"),
            created_at: 1,
        };
        let descriptor_path = artifact_root.join("composition-descriptor.json");
        std::fs::write(
            &descriptor_path,
            serde_json::to_vec_pretty(&descriptor).unwrap(),
        )
        .unwrap();
        let build_report_path = artifact_root.join("build-report.json");
        let build_report = ProjectEditorCompositionBuildReport {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionBuildStatus::Success,
            identity: Some(identity.clone()),
            identity_digest: Some(identity_digest.clone()),
            resolved_identity: Some(resolved_identity),
            artifact: None,
            source_kind: ProjectEditorCompositionBuildSourceKind::ControlledBuild,
            cache_status: ProjectEditorCompositionCacheStatus::Rebuilt,
            cleanup_status: "staging_published".to_string(),
            artifact_size_bytes: Some(1),
            steps: Vec::new(),
            deadline_policy: None,
            qos_policy: None,
            system_facts: None,
            qos_decision: None,
            requested_priority: ProjectEditorCompositionProcessPriority::BelowNormal,
            effective_priority: Some(ProjectEditorCompositionProcessPriority::BelowNormal),
            priority_applied: true,
            cancellation_requested: false,
            process_tree_terminated: false,
            output_readers_joined: true,
            root_wait_completed: true,
            process_group_released: true,
            owned_process_cleanup_confirmed: true,
            release_build_soft_budget_exceeded: false,
            release_build_soft_budget_exceeded_at_ms: None,
            compilation_cache_compatibility_digest: Some(format!("sha256:{}", "4".repeat(64))),
            compilation_cache_affinity: ProjectEditorCompositionCompilationCacheAffinity::Cold,
            canonical_target_anchor_digest: Some(format!("sha256:{}", "5".repeat(64))),
            canonical_target_root_digest: Some(format!("sha256:{}", "6".repeat(64))),
            cross_root_portable: false,
            worker_joined: false,
            redraw_policy_hz: Some(10),
            stage_durations_ms: BTreeMap::new(),
            diagnostics: Vec::new(),
        };
        std::fs::write(
            &build_report_path,
            serde_json::to_vec_pretty(&build_report).unwrap(),
        )
        .unwrap();
        let artifact = ProjectEditorCompositionArtifact {
            schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
            executable_path,
            descriptor_path,
            build_report_path,
            descriptor,
        };
        let report = ProjectEditorCompositionQualificationReport {
            schema_version: PROJECT_EDITOR_COMPOSITION_QUALIFICATION_REPORT_SCHEMA_VERSION
                .to_string(),
            status: "passed".to_string(),
            project_id: identity.project_id,
            module_id: identity.module_id,
            composition_identity_digest: identity_digest,
            linked_aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            initial_frame_count: Some(1),
            paused_frame_count: Some(1),
            stepped_frame_count: Some(2),
            step_count: Some(1),
            resumed_frame_count: Some(2),
            stopped: true,
            steps: Vec::new(),
            diagnostics: Vec::new(),
        };
        (root, artifact, report)
    }
}
