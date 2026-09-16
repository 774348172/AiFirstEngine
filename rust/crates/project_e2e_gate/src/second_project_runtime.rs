use editor_core::{
    EditorRuntimePlayInstance, EditorRuntimePlayRequest, GameViewPresentStatus,
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus,
};
use engine_input::{PointerPosition, RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use engine_runtime::archetype::ComponentValue;
use engine_runtime::aui::{
    AuiBindingValue, AuiInteractionConfig, AuiInteractionState, AuiInteractionSystem,
    AuiRuntimePresenter, ProjectUiStateProducerContext,
};
use engine_runtime::component_value::RuntimeValue;
use engine_runtime::components::ComponentTypeId;
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
use engine_runtime::ids::EntityId;
use engine_runtime::project_observation::{
    ProjectObservationValue, ProjectRuntimeObservationState,
};
use engine_runtime::project_runtime_module::{ProjectRuntimeBindReceipt, ProjectRuntimeBootstrap};
use engine_runtime::project_runtime_session::{
    ProjectRuntimeSessionFrameReport, ProjectRuntimeSessionReportLevel, ProjectRuntimeSessionStage,
};
use engine_runtime::runtime_package::{load_runtime_package, RuntimePackage};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
use engine_runtime::world::World;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const SECOND_PROJECT_RUNTIME_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-session-e2e-report.v1";
const TOGGLE_SWITCH_AUI_ACTION_ID: &str = "ui.toggle-switch";
const SWITCH_PUZZLE_MODULE_ID: &str = "sample.switch-puzzle.runtime";
#[cfg(test)]
const COMPLEX_SHOOTER_MODULE_ID: &str = "sample.complex-shooter.runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecondProjectRuntimeStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondProjectRuntimeEvidence {
    pub aui_click_resolved: bool,
    pub session_action_observed: bool,
    pub session_fixed_update_observed: bool,
    pub switch_changed: bool,
    pub moves_changed: bool,
    pub solved_changed: bool,
    pub moves_text: Option<String>,
    pub solved_binding: Option<bool>,
    pub aui_resolved: bool,
    pub editor_gameview_succeeded: bool,
    pub observation_published: bool,
    pub editor_observation_projected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondProjectRuntimeReport {
    pub schema_version: String,
    pub status: SecondProjectRuntimeStatus,
    pub project_root: String,
    pub runtime_package_dir: String,
    pub bind_receipt: Option<ProjectRuntimeBindReceipt>,
    pub project_id: String,
    pub module_id: String,
    pub session_id: String,
    pub consumer_kind: String,
    pub input_action_order: Vec<String>,
    pub session_frame_report: Option<ProjectRuntimeSessionFrameReport>,
    pub evidence: SecondProjectRuntimeEvidence,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

pub fn run_second_project_runtime_report(
    project_root: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
) -> SecondProjectRuntimeReport {
    let project_root = project_root.as_ref();
    let output_root = output_root.as_ref();
    let package_dir = output_root.join("runtime-package");
    let mut report = SecondProjectRuntimeReport {
        schema_version: SECOND_PROJECT_RUNTIME_REPORT_SCHEMA_VERSION.to_string(),
        status: SecondProjectRuntimeStatus::Failed,
        project_root: project_root.display().to_string(),
        runtime_package_dir: package_dir.display().to_string(),
        bind_receipt: None,
        project_id: String::new(),
        module_id: String::new(),
        session_id: String::new(),
        consumer_kind: "headless-e2e".to_string(),
        input_action_order: Vec::new(),
        session_frame_report: None,
        evidence: SecondProjectRuntimeEvidence::default(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    let package = match assemble_runtime_package(project_root, &package_dir) {
        Ok(package) => package,
        Err(error) => {
            report.diagnostics.push(error);
            return finish_report(output_root, report);
        }
    };
    report.project_id = package.manifest.project.project_id.clone();
    report.module_id = package.manifest.project.runtime_module.module_id.clone();
    let linked = crate::switch_puzzle_linked_set();
    let bound = match ProjectRuntimeBootstrap::bind(&package, &linked) {
        Ok(bound) => bound,
        Err(error) => {
            report.diagnostics.push(error.to_string());
            return finish_report(output_root, report);
        }
    };
    report.bind_receipt = Some(bound.receipt().clone());
    report.session_id = bound.receipt().session_id.clone();
    let parts = bound.into_parts();
    let mut producer = parts.ui_state_producer;

    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let hydration = hydrator.hydrate_active_scene(&package, &mut world);
    if hydration.has_errors() {
        report
            .diagnostics
            .push("runtime scene hydration failed".to_string());
        return finish_report(output_root, report);
    }

    let Some(document) = package.aui_documents.get("puzzle-hud") else {
        report
            .diagnostics
            .push("puzzle-hud AUI document is missing".to_string());
        return finish_report(output_root, report);
    };
    let initial_snapshot = producer.produce(
        ProjectUiStateProducerContext::new(1, &package, &world).with_active_binding_paths([
            "puzzle.moves_text".to_string(),
            "puzzle.solved".to_string(),
        ]),
    );
    let initial_present = AuiRuntimePresenter::present_project_snapshot(document, initial_snapshot);
    let toggle_rect = initial_present
        .layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "toggle-button")
        .map(|node| node.rect)
        .expect("Switch Puzzle toggle button must participate in AUI layout");
    let pointer_x = toggle_rect.x + toggle_rect.width * 0.5;
    let pointer_y = toggle_rect.y + toggle_rect.height * 0.5;
    let mut input_frame = RuntimeInputFrame::new(1, "game-view");
    input_frame.pointer_position = Some(PointerPosition {
        x: pointer_x,
        y: pointer_y,
    });
    input_frame.events.push(RuntimeInputEvent::PointerDown {
        x: pointer_x,
        y: pointer_y,
        button: RuntimePointerButton::Primary,
    });
    input_frame.events.push(RuntimeInputEvent::PointerUp {
        x: pointer_x,
        y: pointer_y,
        button: RuntimePointerButton::Primary,
    });
    let mut interaction_state = AuiInteractionState::default();
    let interaction = AuiInteractionSystem::process_with_state(
        &initial_present.resolved_document,
        &initial_present.layout,
        &input_frame,
        &mut interaction_state,
        AuiInteractionConfig::default(),
    );
    report.input_action_order = interaction
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    report.evidence.aui_click_resolved =
        report.input_action_order == vec![TOGGLE_SWITCH_AUI_ACTION_ID];
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    let frame = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::EditorPlay).with_aui_interaction(interaction),
        &mut world,
        RuntimeFrameContext {
            package: &package,
            instance_loader: hydrator.instance_loader_mut(),
        },
    );
    report.session_frame_report = frame.project_runtime_session_report.clone();
    report.evidence.observation_published = matches!(
        frame.project_observation_state.as_ref(),
        Some(ProjectRuntimeObservationState::Published { snapshot })
            if snapshot.values.get("puzzle.solved")
                == Some(&ProjectObservationValue::Bool(true))
    );
    if let Some(session) = &report.session_frame_report {
        report.evidence.session_action_observed = session.stages.iter().any(|stage| {
            stage.stage == ProjectRuntimeSessionStage::AuiActionDispatch
                && stage.handled_action_count == 1
                && stage.committed_mutation_count == 1
        });
        report.evidence.session_fixed_update_observed = session.stages.iter().any(|stage| {
            stage.stage == ProjectRuntimeSessionStage::FixedUpdate
                && stage.committed_mutation_count == 2
        });
    }
    report.evidence.switch_changed =
        read_bool(&world, "entity-puzzle-switch", "puzzle.switchState", "on") == Some(true);
    report.evidence.moves_changed = read_i64(
        &world,
        "entity-puzzle-session",
        "puzzle.sessionState",
        "moves",
    ) == Some(1);
    report.evidence.solved_changed = read_bool(
        &world,
        "entity-puzzle-session",
        "puzzle.sessionState",
        "solved",
    ) == Some(true);

    let snapshot = producer.produce(
        ProjectUiStateProducerContext::new(2, &package, &world).with_active_binding_paths([
            "puzzle.moves_text".to_string(),
            "puzzle.solved".to_string(),
        ]),
    );
    report.evidence.moves_text = match snapshot.snapshot.values.get("puzzle.moves_text") {
        Some(AuiBindingValue::String(value)) => Some(value.clone()),
        _ => None,
    };
    report.evidence.solved_binding = match snapshot.snapshot.values.get("puzzle.solved") {
        Some(AuiBindingValue::Bool(value)) => Some(*value),
        _ => None,
    };
    let present = AuiRuntimePresenter::present_project_snapshot(document, snapshot);
    let moves_resolved = present
        .resolved_document
        .nodes
        .iter()
        .any(|node| node.node_id == "moves-text" && node.text.as_deref() == Some("MOVES 1"));
    let solved_resolved = present
        .layout
        .computed_nodes
        .iter()
        .any(|node| node.node_id == "solved-panel" && node.effective_visible);
    report.evidence.aui_resolved = moves_resolved
        && solved_resolved
        && present
            .report
            .ui_state_snapshot_report
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.missing_paths.is_empty() && snapshot.type_mismatch_paths.is_empty()
            });

    let editor_request = EditorRuntimePlayRequest {
        schema_version: editor_core::EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
        session_id: "switch-puzzle-second-project-gate".to_string(),
        project_root: project_root.to_path_buf(),
        runtime_package_path: package_dir.clone(),
        scene_ref: Some("scene-main".to_string()),
        run_profile: Some("editor-gameview".to_string()),
        frame_limit: 1,
        requested_by: "Automation".to_string(),
        preview_package_report_path: None,
    };
    let editor_output =
        EditorRuntimePlayInstance::start_with_linked_modules(editor_request, &linked);
    report.evidence.editor_gameview_succeeded = editor_output.present_report.status
        == GameViewPresentStatus::Success
        && editor_output
            .present_report
            .project_runtime_bind_receipt
            .as_ref()
            .is_some_and(|receipt| receipt.module_id == SWITCH_PUZZLE_MODULE_ID);
    report.evidence.editor_observation_projected = matches!(
        editor_output
            .present_report
            .project_observation_state
            .as_ref(),
        Some(ProjectRuntimeObservationState::Published { snapshot })
            if snapshot.contract_id == "switch-puzzle.runtime-observations"
                && snapshot.values.contains_key("puzzle.solved")
    );

    finish_report(output_root, report)
}

fn finish_report(
    output_root: &Path,
    mut report: SecondProjectRuntimeReport,
) -> SecondProjectRuntimeReport {
    let evidence = &report.evidence;
    let passed = evidence.aui_click_resolved
        && evidence.session_action_observed
        && evidence.session_fixed_update_observed
        && evidence.switch_changed
        && evidence.moves_changed
        && evidence.solved_changed
        && evidence.moves_text.as_deref() == Some("MOVES 1")
        && evidence.solved_binding == Some(true)
        && evidence.aui_resolved
        && evidence.editor_gameview_succeeded
        && evidence.observation_published
        && evidence.editor_observation_projected;
    if !passed {
        report
            .next_actions
            .push("fix_switch_puzzle_second_project_chain".to_string());
    }
    report.status = if passed && report.diagnostics.is_empty() {
        SecondProjectRuntimeStatus::Passed
    } else {
        SecondProjectRuntimeStatus::Failed
    };
    let report_path = output_root
        .join("reports")
        .join("project-runtime-module-second-project-report.json");
    if let Some(parent) = report_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_vec_pretty(&report) {
        Ok(bytes) => {
            if let Err(error) = fs::write(&report_path, bytes) {
                report.status = SecondProjectRuntimeStatus::Failed;
                report
                    .diagnostics
                    .push(format!("failed to write second project report: {error}"));
            }
        }
        Err(error) => {
            report.status = SecondProjectRuntimeStatus::Failed;
            report.diagnostics.push(format!(
                "failed to serialize second project report: {error}"
            ));
        }
    }
    report
}

fn assemble_runtime_package(
    project_root: &Path,
    package_dir: &Path,
) -> Result<RuntimePackage, String> {
    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(project_root),
    );
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        return Err(format!(
            "project assembly failed: {:?}",
            assembly.report.diagnostics
        ));
    }
    let input = assembly
        .build_input
        .ok_or_else(|| "successful assembly omitted build input".to_string())?;
    let active_scene_id = assembly
        .active_scene_id
        .unwrap_or_else(|| "scene-main".to_string());
    let build = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(package_dir, active_scene_id),
        &input,
    );
    if build.status != RuntimePackageBuildStatus::Success {
        return Err(format!(
            "runtime package build failed: {:?}",
            build.diagnostics
        ));
    }
    let loaded = load_runtime_package(package_dir);
    loaded.value.ok_or_else(|| {
        format!(
            "runtime package load failed: {:?}",
            loaded.diagnostics.issues
        )
    })
}

fn dynamic_fields(
    world: &World,
    entity_id: &str,
    component_type: &str,
) -> Option<std::collections::BTreeMap<String, RuntimeValue>> {
    match world.component_value(
        &EntityId::from(entity_id),
        &ComponentTypeId::from(component_type),
    )? {
        ComponentValue::Dynamic { value, .. } => match value {
            RuntimeValue::Object(fields) => Some(fields),
            _ => None,
        },
        _ => None,
    }
}

fn read_i64(world: &World, entity_id: &str, component_type: &str, field: &str) -> Option<i64> {
    let fields = dynamic_fields(world, entity_id, component_type)?;
    match fields.get(field)? {
        RuntimeValue::I64(value) => Some(*value),
        RuntimeValue::F64(value) => Some(*value as i64),
        _ => None,
    }
}

fn read_bool(world: &World, entity_id: &str, component_type: &str, field: &str) -> Option<bool> {
    let fields = dynamic_fields(world, entity_id, component_type)?;
    match fields.get(field)? {
        RuntimeValue::Bool(value) => Some(*value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_core::{
        DesktopExportPipeline, DesktopExportRequest, DesktopExportStatus, ExplicitExportOutput,
        ReleasePackageBuildRequest, ReleasePackageBuilder, ReleasePackageReportLevel,
        ReleasePackageStatus,
    };
    use engine_runtime::project_runtime_module::{
        LinkedProjectRuntimeSet, PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
    };
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn project_runtime_session_switch_puzzle_aui_to_next_snapshot_and_editor_gameview() {
        let output = unique_temp_dir("second-project-runtime");
        let report = run_second_project_runtime_report(switch_puzzle_project_root(), &output);

        assert_eq!(
            report.status,
            SecondProjectRuntimeStatus::Passed,
            "{report:#?}"
        );
        assert!(output
            .join("reports/project-runtime-module-second-project-report.json")
            .is_file());
    }

    #[test]
    fn project_runtime_session_project_module_negative_matrix() {
        let output = unique_temp_dir("project-runtime-module-negative");
        let puzzle = assemble_runtime_package(
            &switch_puzzle_project_root(),
            &output.join("puzzle-runtime-package"),
        )
        .unwrap();
        let shooter = assemble_runtime_package(
            &complex_shooter_project_root(),
            &output.join("shooter-runtime-package"),
        )
        .unwrap();

        assert_bind_error(
            &shooter,
            &crate::switch_puzzle_linked_set(),
            "project_runtime.module_id_mismatch",
        );
        assert_bind_error(
            &puzzle,
            &crate::complex_shooter_linked_set(),
            "project_runtime.module_id_mismatch",
        );

        let mut interface_mutation = puzzle.clone();
        interface_mutation
            .manifest
            .project
            .runtime_module
            .interface_version = "project-runtime-module.v999".to_string();
        assert_bind_error(
            &interface_mutation,
            &crate::switch_puzzle_linked_set(),
            "project_runtime.interface_version_mismatch",
        );

        let mut digest_mutation = puzzle.clone();
        digest_mutation
            .manifest
            .project
            .runtime_module
            .aot_content_digest = "sha256:mutated".to_string();
        assert_bind_error(
            &digest_mutation,
            &crate::switch_puzzle_linked_set(),
            "project_runtime.aot_digest_mismatch",
        );

        let mut artifact_mutation = puzzle.clone();
        artifact_mutation.rules.rules[0].artifact_id = Some("rule-artifact:mutated".to_string());
        assert_bind_error(
            &artifact_mutation,
            &crate::switch_puzzle_linked_set(),
            "project_runtime.rule_artifact_mismatch",
        );

        let descriptor = crate::switch_puzzle_linked_set()
            .only_descriptor()
            .unwrap()
            .clone();
        let module = crate::identity_only_project_runtime(descriptor);
        let mut duplicate = LinkedProjectRuntimeSet::new();
        duplicate.add(module.clone()).unwrap();
        assert_eq!(
            duplicate
                .add(module)
                .err()
                .expect("duplicate module must fail")
                .code,
            "project_runtime.duplicate_linked_module_id"
        );
        assert_bind_error(
            &puzzle,
            &LinkedProjectRuntimeSet::new(),
            "project_runtime.module_not_linked",
        );

        let mut missing_input = puzzle.clone();
        missing_input.default_input_mapping = None;
        assert_bind_error(
            &missing_input,
            &crate::switch_puzzle_linked_set(),
            "project_runtime.default_input_missing",
        );
    }

    #[test]
    fn project_runtime_native_module_consumers_share_public_abi_and_bootstrap() {
        let output = workspace_root().join("rust/target").join(format!(
            "project-runtime-native-module-consumers-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&output).unwrap();
        let abi_fixture =
            workspace_root().join("rust/fixtures/project_runtime_native_module_minimal");
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(abi_fixture.join("Cargo.toml")).unwrap()).unwrap();
        let dependencies = manifest["dependencies"].as_table().unwrap();
        for required in ["project_runtime_abi", "project_runtime_sdk"] {
            assert!(dependencies.contains_key(required), "{required}");
        }
        let consumers = [
            workspace_root().join("samples/tower_defense_project/RuntimeModule"),
            complex_shooter_project_root().join("RuntimeModule"),
            switch_puzzle_project_root().join("RuntimeModule"),
        ];
        for consumer in consumers {
            let manifest: toml::Value =
                toml::from_str(&fs::read_to_string(consumer.join("Cargo.toml")).unwrap()).unwrap();
            let dependencies = manifest["dependencies"].as_table().unwrap();
            assert!(
                dependencies.contains_key("project_game_sdk"),
                "{}: project_game_sdk",
                consumer.display()
            );
            for forbidden in [
                "project_runtime_abi",
                "project_runtime_sdk",
                "engine_runtime",
                "engine_input",
                "editor_core",
            ] {
                assert!(
                    !dependencies.contains_key(forbidden),
                    "{}: {forbidden}",
                    consumer.display()
                );
            }
        }

        let shooter = assemble_runtime_package(
            &complex_shooter_project_root(),
            &output.join("shooter-runtime-package"),
        )
        .unwrap();
        let shooter_bound =
            ProjectRuntimeBootstrap::bind(&shooter, &crate::complex_shooter_linked_set()).unwrap();
        assert_eq!(shooter_bound.receipt().module_id, COMPLEX_SHOOTER_MODULE_ID);

        let puzzle = assemble_runtime_package(
            &switch_puzzle_project_root(),
            &output.join("puzzle-runtime-package"),
        )
        .unwrap();
        let puzzle_bound =
            ProjectRuntimeBootstrap::bind(&puzzle, &crate::switch_puzzle_linked_set()).unwrap();
        assert_eq!(puzzle_bound.receipt().module_id, SWITCH_PUZZLE_MODULE_ID);

        drop(shooter_bound);
        drop(puzzle_bound);
        fs::remove_dir_all(&output).unwrap();
        assert!(!output.exists());
    }

    #[test]
    fn exported_player_switch_puzzle_desktop_release_process() {
        let project_root = switch_puzzle_project_root();
        let output = unique_temp_dir("switch-puzzle-exported-player");
        fs::create_dir_all(&output).unwrap();

        let mut desktop_request = DesktopExportRequest::windows_dev(&project_root)
            .with_explicit_output(ExplicitExportOutput::from_user_selected(&output));
        desktop_request.output_root = output.join("desktop");
        let desktop = DesktopExportPipeline::export(desktop_request);
        assert_eq!(desktop.status, DesktopExportStatus::Success, "{desktop:#?}");
        assert_eq!(desktop.player_exit_code, Some(0));
        assert_child_receipt(
            Path::new(&desktop.player_report_path),
            SWITCH_PUZZLE_MODULE_ID,
        );

        let release_output = output.join("release/SwitchPuzzle");
        let release_report_path = output.join("release-report.json");
        let mut release_request = ReleasePackageBuildRequest::windows_release(&project_root)
            .with_explicit_output(ExplicitExportOutput::from_user_selected(&output));
        release_request.output_dir = Some(release_output);
        release_request.report_path = Some(release_report_path);
        release_request.report_level = ReleasePackageReportLevel::Trace;
        release_request.verify_process = true;
        let release = ReleasePackageBuilder::build(&release_request);
        assert_eq!(
            release.status,
            ReleasePackageStatus::Success,
            "{release:#?}"
        );
        assert!(release.verification.explicit_process_passed);
        assert_eq!(release.verification.process_exit_code, Some(0));
        assert_eq!(release.verification.child_player_exit_code, Some(0));
        let process_report_path = PathBuf::from(
            release
                .verification
                .process_report_path
                .as_deref()
                .expect("release process report path"),
        );
        let process: serde_json::Value =
            serde_json::from_slice(&fs::read(process_report_path).unwrap()).unwrap();
        let child_report_path = PathBuf::from(
            process["childReportPath"]
                .as_str()
                .expect("release child report path"),
        );
        assert_child_receipt(&child_report_path, SWITCH_PUZZLE_MODULE_ID);
    }

    fn assert_bind_error(
        package: &RuntimePackage,
        linked: &LinkedProjectRuntimeSet,
        expected: &'static str,
    ) {
        assert_eq!(
            ProjectRuntimeBootstrap::bind(package, linked)
                .err()
                .expect("bind mutation must fail")
                .code,
            expected
        );
    }

    fn assert_child_receipt(path: &Path, module_id: &str) {
        let report: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(
            report["projectRuntimeBindReceipt"]["moduleId"].as_str(),
            Some(module_id),
            "{}",
            path.display()
        );
        assert_eq!(
            report["projectRuntimeBindReceipt"]["interfaceVersion"].as_str(),
            Some(PROJECT_RUNTIME_MODULE_INTERFACE_VERSION)
        );
        assert_eq!(report["projectRuntimeBindReceipt"]["status"], "passed");
    }

    fn switch_puzzle_project_root() -> PathBuf {
        workspace_root().join("samples/switch_puzzle_project")
    }

    fn complex_shooter_project_root() -> PathBuf {
        workspace_root().join("samples/complex_shooter_project")
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{stamp}"))
    }
}
