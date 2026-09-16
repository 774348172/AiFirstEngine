use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn e2e_gate_report_serializes_with_schema() {
    let report = ComplexProjectE2eGateReport::new("project", "output");

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(COMPLEX_PROJECT_E2E_GATE_REPORT_SCHEMA_VERSION));
    assert!(json.contains("complex-shooter-real-project-end-to-end-gate-v1"));
}

#[test]
fn sample_project_loads_and_counts_real_authoring_files() {
    let project_root = sample_project_root();

    let summary = load_sample_project_summary(&project_root).expect("sample project should load");

    assert_eq!(summary.project_name, "Complex Shooter Sample");
    assert!(summary.scene_count >= 1);
    assert!(summary.entity_count >= 6);
    assert!(summary.prefab_count >= 3);
    assert!(summary.asset_count >= 5);
    assert!(summary.rule_count >= 3);
    assert!(summary.input_action_count >= 3);
    assert!(summary.aui_document_count >= 1);
}

#[test]
fn visual_symptom_diagnosis_locates_traces_repairs_and_revalidates() {
    use editor_core::{
        AiCandidateToolInput, AiCapabilityGrant, AiCapabilityToolKernel, AiMutationKind,
        AiToolExecutionStatus, AiToolInvocation, AiToolInvocationPayload, AiToolOutput,
        AiToolStartOutcome, AuiPatchOperation, PatchOperation, PatchSource, ProjectCandidateEntry,
        ProjectCandidateSourceKind, ProjectPatchDocument, ProjectPreviewCaptureKind,
        ProjectPreviewFrameReadback, ProjectPreviewPixelFormat, ProjectRuntimeCaptureIssueInput,
        ProjectUiExplainInput, ProjectUiLocateInput, ProjectUiOwnerTraceInput,
        AI_TOOL_INVOCATION_SCHEMA_VERSION, PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION,
        TOOL_ID_PROJECT_MUTATE, TOOL_ID_PROJECT_PREVIEW, TOOL_ID_PROJECT_TRACE_UI_OWNER,
        TOOL_ID_RUNTIME_CAPTURE_ISSUE, TOOL_ID_UI_EXPLAIN_VISIBILITY, TOOL_ID_UI_LOCATE,
    };
    use engine_runtime::aui::{
        AuiActionRef, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect,
    };

    let project_root = temp_output_root("visual-symptom-diagnosis");
    let mut session = editor_core::EditorSession::new();
    let created = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::CreateProject {
            path: project_root.display().to_string(),
            name: "Visual Symptom Diagnosis".to_string(),
        },
    ));
    assert_eq!(created.status, editor_core::CommandStatus::Committed);

    std::fs::create_dir_all(project_root.join("UI")).unwrap();
    std::fs::create_dir_all(project_root.join("RuntimeModule/src")).unwrap();
    let root_node = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["start-button"]);
    let mut button = AuiNode::new(
        "start-button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(80.0, 80.0, 240.0, 64.0),
    )
    .with_parent("root")
    .with_text("Start Game")
    .with_action(AuiActionRef::click("menu.start_game"));
    button.visible = false;
    let document = AuiDocument::new(
        "main-menu",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root_node, button],
    );
    std::fs::write(
        project_root.join("UI/main-menu.aui.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    std::fs::write(
        project_root.join("RuntimeModule/src/menu.rs"),
        "pub fn start_game() {}\n",
    )
    .unwrap();
    let invoke_read = |session: &editor_core::EditorSession,
                       invocation_id: &str,
                       tool_id: &str,
                       payload: AiToolInvocationPayload| {
        let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
        let grant = AiCapabilityGrant::read(
            format!("grant-{invocation_id}"),
            binding.project_id,
            binding.project_digest.clone(),
            "visual-symptom-gate",
        )
        .unwrap();
        (
            AiToolInvocation {
                schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                invocation_id: invocation_id.to_string(),
                tool_id: tool_id.to_string(),
                expected_project_digest: binding.project_digest,
                payload,
            },
            grant,
        )
    };
    let mut kernel = AiCapabilityToolKernel::new();

    let capture_issue = |session: &mut editor_core::EditorSession,
                         kernel: &mut AiCapabilityToolKernel,
                         preview_operation_id: &str,
                         capture_invocation_id: &str|
     -> String {
        let (preview, preview_grant) = invoke_read(
            session,
            preview_operation_id,
            TOOL_ID_PROJECT_PREVIEW,
            AiToolInvocationPayload::Preview,
        );
        let AiToolStartOutcome::Accepted(accepted) = kernel.start(session, preview, &preview_grant)
        else {
            panic!("Preview must enter the asynchronous evidence barrier")
        };
        kernel.pump_operations(session, 3);
        let awaiting = kernel.observe(&accepted.operation_id).unwrap();
        assert_eq!(awaiting.stage, "awaiting_frame_evidence");
        assert!(awaiting.result.is_none());
        let ticket = session
            .pending_project_preview_frame_ticket()
            .cloned()
            .expect("Preview must retain an exact-frame ticket");
        let (width, height) = session
            .last_game_view_runtime_frame()
            .map(|frame| (frame.width.max(1), frame.height.max(1)))
            .expect("Preview must retain its runtime frame");
        session
            .record_project_preview_presented_frame(ProjectPreviewFrameReadback {
                game_view_session_id: ticket.game_view_session_id,
                texture_id: ticket.expected_texture_id,
                frame_index: ticket.expected_frame_index,
                width,
                height,
                pixel_format: ProjectPreviewPixelFormat::Rgba8Unorm,
                capture_kind: ProjectPreviewCaptureKind::DeterministicTestAdapter,
                rgba8: vec![0; (width * height * 4) as usize],
            })
            .expect("Preview frame receipt must persist exact evidence");
        kernel.pump_operations(session, 1);
        let completed = kernel.observe(&accepted.operation_id).unwrap();
        let result = completed
            .result
            .expect("Preview must complete after receipt");
        let Some(AiToolOutput::Preview(preview)) = result.output else {
            panic!("Preview completion must return frame evidence")
        };
        let frame_evidence_ref = preview.frame_evidence_ref;
        let (capture, capture_grant) = invoke_read(
            session,
            capture_invocation_id,
            TOOL_ID_RUNTIME_CAPTURE_ISSUE,
            AiToolInvocationPayload::RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput {
                schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
                frame_evidence_ref,
                symptom: Some("Start Game button is absent".to_string()),
            }),
        );
        let captured = kernel.execute(session, capture, &capture_grant);
        let Some(AiToolOutput::VisualIssueCaptured(bundle)) = captured.output else {
            panic!("visual issue capture failed: {:?}", captured.diagnostics)
        };
        bundle.issue_bundle_ref
    };

    let issue_bundle_ref = capture_issue(
        &mut session,
        &mut kernel,
        "preview-before-repair",
        "visual-capture-before-repair",
    );

    // The caller supplies an exact frame evidence ref first; later tools consume its issue bundle.
    let (locate, locate_grant) = invoke_read(
        &session,
        "visual-locate",
        TOOL_ID_UI_LOCATE,
        AiToolInvocationPayload::UiLocate(ProjectUiLocateInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            query: "Start Game".to_string(),
            issue_bundle_ref: Some(issue_bundle_ref.clone()),
        }),
    );
    let located = kernel.execute(&mut session, locate, &locate_grant);
    let Some(AiToolOutput::UiLocated(located)) = located.output else {
        panic!("visual label did not locate an AUI node")
    };
    assert_eq!(located.candidates.len(), 1);
    let target = &located.candidates[0];

    let explain_input = ProjectUiExplainInput {
        schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
        document_path: target.document_path.clone(),
        node_id: target.node_id.clone(),
        issue_bundle_ref: issue_bundle_ref.clone(),
    };
    let (explain, explain_grant) = invoke_read(
        &session,
        "visual-explain",
        TOOL_ID_UI_EXPLAIN_VISIBILITY,
        AiToolInvocationPayload::UiExplainVisibility(explain_input.clone()),
    );
    let explained = kernel.execute(&mut session, explain, &explain_grant);
    let Some(AiToolOutput::VisualIssue(issue)) = explained.output else {
        panic!("visual issue bundle was not produced")
    };
    assert_eq!(issue.node.first_failure_stage, "authored_visibility");

    let (trace, trace_grant) = invoke_read(
        &session,
        "visual-trace",
        TOOL_ID_PROJECT_TRACE_UI_OWNER,
        AiToolInvocationPayload::ProjectTraceUiOwner(ProjectUiOwnerTraceInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            document_path: target.document_path.clone(),
            node_id: target.node_id.clone(),
            issue_bundle_ref: Some(issue_bundle_ref),
        }),
    );
    let traced = kernel.execute(&mut session, trace, &trace_grant);
    let Some(AiToolOutput::UiOwnerTrace(owner)) = traced.output else {
        panic!("UI owner trace was not produced")
    };
    assert!(owner.action_ids.contains(&"menu.start_game".to_string()));
    assert!(owner
        .project_source_symbols
        .contains(&"RuntimeModule/src/menu.rs::start_game".to_string()));

    let patch = ProjectPatchDocument::new(
        "visual-start-button-repair",
        "Restore the missing Start Game button",
        PatchSource::Test,
        vec![PatchOperation::Aui(AuiPatchOperation::SetNodeField {
            operation_id: "set-start-visible".to_string(),
            depends_on: Vec::new(),
            path: target.document_path.clone(),
            node_id: target.node_id.clone(),
            schema_path: "visible".to_string(),
            value: serde_json::json!(true),
        })],
    );
    let envelope = ProjectCandidateEntry::project_patch_envelope(
        &session,
        "visual-start-button-repair",
        ProjectCandidateSourceKind::ImportedCodex,
        "visual-symptom-gate",
        patch,
    )
    .unwrap();
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mutation_grant = AiCapabilityGrant::scoped_mutation(
        "grant-visual-repair",
        binding.project_id,
        "sha256:restore-start-game-button",
        binding.project_digest.clone(),
        vec!["aui".to_string()],
        vec![AiMutationKind::ProjectPatch],
        "visual-symptom-gate",
    )
    .unwrap();
    let repaired = kernel.execute(
        &mut session,
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "visual-repair".to_string(),
            tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
            expected_project_digest: binding.project_digest,
            payload: AiToolInvocationPayload::Candidate(AiCandidateToolInput {
                envelope,
                source_file_path: None,
                controlled_source_patch_validation: None,
            }),
        },
        &mutation_grant,
    );
    assert_eq!(
        repaired.status,
        AiToolExecutionStatus::Completed,
        "mutation diagnostics: {:?}",
        repaired.diagnostics
    );
    assert!(matches!(
        repaired.output,
        Some(AiToolOutput::CandidateApplied(_))
    ));

    let repaired_issue_bundle_ref = capture_issue(
        &mut session,
        &mut kernel,
        "preview-after-repair",
        "visual-capture-after-repair",
    );
    let (revalidate, revalidate_grant) = invoke_read(
        &session,
        "visual-revalidate",
        TOOL_ID_UI_EXPLAIN_VISIBILITY,
        AiToolInvocationPayload::UiExplainVisibility(ProjectUiExplainInput {
            issue_bundle_ref: repaired_issue_bundle_ref,
            ..explain_input
        }),
    );
    let revalidated = kernel.execute(&mut session, revalidate, &revalidate_grant);
    let Some(AiToolOutput::VisualIssue(issue)) = revalidated.output else {
        panic!("revalidated visual issue bundle was not produced")
    };
    assert!(issue.node.authored_visible);
    assert_eq!(issue.node.resolved_visible, None);
    assert_eq!(
        issue.node.first_failure_stage,
        "presented_frame_semantic_trace_unavailable"
    );
    assert!(!issue.node.draw_command_present);
    assert!(issue
        .node
        .diagnostic_codes
        .contains(&"exact_presented_frame_evidence_verified".to_string()));
    assert!(issue
        .node
        .diagnostic_codes
        .contains(&"runtime_semantic_trace_not_captured".to_string()));
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn provider_build_delivery_exports_once_then_verifies_existing_package() {
    use engine_tool_provider::{
        CanonicalToolStatus, EngineToolProvider, HostSessionContext, HostToolCall,
        NativeEngineToolProvider,
    };
    use serde_json::json;

    let project_root = sample_project_root();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "build-delivery-contract".to_string(),
        workspace_root: project_root.clone(),
        project_root: Some(project_root.clone()),
    })
    .expect("Provider attaches directly to the project");

    let build = provider.invoke(HostToolCall {
        call_id: "build".to_string(),
        tool_name: "engine_project_build".to_string(),
        arguments: json!({"targetProfile":"windows-dev","frameLimit":1}),
        approved: true,
    });
    assert_eq!(
        build.status,
        CanonicalToolStatus::Completed,
        "build failed: {build:#?}"
    );
    let package_dir = std::path::PathBuf::from(
        build.output["packageDir"]
            .as_str()
            .expect("build returns packageDir"),
    );
    let package_manifest = package_dir.join("package-manifest.json");
    let manifest_digest_before = engine_runtime::canonical_digest::sha256_prefixed(
        &std::fs::read(&package_manifest).unwrap(),
    );
    let delivery_ref = build.output["deliveryRef"]
        .as_str()
        .expect("build returns deliveryRef");

    let verified = provider.invoke(HostToolCall {
        call_id: "verify".to_string(),
        tool_name: "engine_delivery_verify".to_string(),
        arguments: json!({
            "deliveryRef":delivery_ref,
            "mode":"headless",
            "timeoutMs":30000,
            "frameLimit":2,
            "screenshot":false
        }),
        approved: true,
    });
    assert_eq!(
        verified.status,
        CanonicalToolStatus::Completed,
        "delivery verification failed: {verified:#?}"
    );
    assert_eq!(verified.output["processExitCode"], 0);
    assert_eq!(verified.output["childPlayerExitCode"], 0);
    assert_eq!(
        engine_runtime::canonical_digest::sha256_prefixed(
            &std::fs::read(&package_manifest).unwrap()
        ),
        manifest_digest_before,
        "delivery verification must not rebuild the frozen package"
    );
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn provider_unknown_task_preflight_keeps_large_competition_margin() {
    use engine_tool_provider::{
        CanonicalToolStatus, EngineToolProvider, HostSessionContext, HostToolCall,
        NativeEngineToolProvider, ToolMaturity,
    };
    use serde_json::json;

    let preflight_started = Instant::now();
    let project_root = sample_project_root();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "unknown-task-preflight".to_string(),
        workspace_root: project_root.clone(),
        project_root: Some(project_root.clone()),
    })
    .expect("Provider attaches directly to the project without an Editor");

    let definitions = provider.tool_definitions();
    for tool_name in [
        "engine_project_inspect",
        "engine_project_search",
        "engine_project_mutate",
        "engine_runtime_run",
        "engine_project_build",
        "engine_delivery_verify",
    ] {
        let definition = definitions
            .iter()
            .find(|definition| definition.name == tool_name)
            .unwrap_or_else(|| panic!("missing concrete Provider tool {tool_name}"));
        assert_eq!(definition.maturity, ToolMaturity::Ready);
    }

    let invoke = |provider: &mut EngineToolProvider,
                  call_id: &str,
                  tool_name: &str,
                  arguments: serde_json::Value,
                  approved: bool| {
        let result = provider.invoke(HostToolCall {
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments,
            approved,
        });
        assert_eq!(
            result.status,
            CanonicalToolStatus::Completed,
            "{tool_name} failed: {result:#?}"
        );
        let operation = provider
            .observe(&result.operation_id)
            .expect("Provider operation remains observable in its session");
        assert!(operation.terminal);
        assert_eq!(operation.result.as_ref(), Some(&result));
        result
    };

    let inspect = invoke(
        &mut provider,
        "unknown-inspect",
        "engine_project_inspect",
        json!({}),
        false,
    );
    assert!(inspect.project_revision.is_some());

    let search = invoke(
        &mut provider,
        "unknown-search",
        "engine_project_search",
        json!({"query":"player"}),
        false,
    );
    assert_eq!(search.project_revision, inspect.project_revision);

    let mutation = invoke(
        &mut provider,
        "unknown-mutate",
        "engine_project_mutate",
        json!({
            "goal":"Add a previously unknown project note.",
            "domain":"project",
            "changes":[{
                "operation":"create_or_replace",
                "path":"unknown-task-note.txt",
                "content":"unknown task handled by the native Provider"
            }]
        }),
        true,
    );
    assert!(project_root.join("unknown-task-note.txt").is_file());
    assert_ne!(mutation.project_revision, inspect.project_revision);

    let build = invoke(
        &mut provider,
        "unknown-build",
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":1}),
        true,
    );
    let delivery_ref = build.output["deliveryRef"]
        .as_str()
        .expect("build returns an opaque deliveryRef")
        .to_string();
    let verified = invoke(
        &mut provider,
        "unknown-delivery",
        "engine_delivery_verify",
        json!({
            "deliveryRef":delivery_ref,
            "mode":"headless",
            "frameLimit":1,
            "timeoutMs":30000,
            "screenshot":false
        }),
        true,
    );
    assert_eq!(verified.output["status"], "passed");
    assert_eq!(verified.output["processExitCode"], 0);
    assert_eq!(verified.output["childPlayerExitCode"], 0);

    let elapsed_ms = preflight_started.elapsed().as_millis() as u64;
    assert!(elapsed_ms < 120_000, "preflight took {elapsed_ms} ms");
    assert!(2 * 60 * 60 * 1000 - elapsed_ms > 60 * 60 * 1000);
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn e2e_gate_runs_export_package_and_headless_player() {
    let output_root = temp_output_root("complex-project-e2e");
    let report = run_complex_project_e2e_gate(ComplexProjectE2eGateRequest::new(
        sample_project_root(),
        &output_root,
    ));

    assert_eq!(report.status, ComplexProjectE2eStatus::Passed);
    assert!(report.metrics.runtime_package_entity_count >= 6);
    assert!(report.metrics.frames_run >= 1);
    assert!(report.metrics.present_count >= 1);
    assert!(report.metrics.draw_item_count > 0);
    assert!(report.metrics.aui_package_document_count >= 1);
    assert!(report.metrics.aui_loaded_document_count >= 1);
    assert!(report.metrics.aui_draw_item_count > 0);
    assert!(report.metrics.aui_text_command_count > 0);
    assert!(report.metrics.aui_ui_pass_inserted);
    assert!(report.metrics.aui_composition_stage_count >= 1);
    assert_eq!(report.metrics.aui_before_world_item_count, 0);
    assert!(report.metrics.aui_screen_overlay_item_count > 0);
    assert_eq!(report.metrics.aui_modal_item_count, 0);
    assert!(!report.metrics.aui_before_world_pass_present);
    assert!(report.metrics.aui_screen_overlay_pass_present);
    assert!(!report.metrics.aui_modal_pass_present);
    assert!(report.metrics.aui_before_world_skipped);
    assert!(!report.metrics.aui_screen_overlay_skipped);
    assert!(report.metrics.aui_modal_skipped);
    assert!(!report.metrics.aui_modal_rendering_only);
    assert!(report.metrics.aui_glyph_present);
    assert!(report.metrics.aui_font_atlas_present);
    assert_eq!(
        report.metrics.aui_font_atlas_id.as_deref(),
        Some("aife-default-zh-cn-common-v1")
    );
    assert!(report.metrics.aui_requested_glyph_count > 0);
    assert_eq!(
        report.metrics.aui_rendered_glyph_count,
        report.metrics.aui_requested_glyph_count
    );
    assert!(report.metrics.aui_glyph_plan_hash.is_some());
    assert_eq!(report.metrics.aui_snapshot_source, "project_producer");
    assert_eq!(
        report.metrics.aui_producer_id.as_deref(),
        Some("complex_shooter_runtime_ui_state")
    );
    assert!(report.metrics.aui_snapshot_value_count >= 4);
    assert!(report.metrics.aui_produced_path_count >= 4);
    assert!(report.metrics.aui_declared_binding_path_count >= 1);
    assert_eq!(report.metrics.aui_missing_path_count, 0);
    assert_eq!(report.metrics.aui_type_mismatch_path_count, 0);
    assert_eq!(report.metrics.aui_status, "success");
    assert!(!report
        .metrics
        .aui_next_actions
        .contains(&"runtime_text_glyph_present".to_string()));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step_id == "aui-runtime-present"
            && step.status == ComplexProjectE2eStatus::Passed));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step_id == "optional-real-window-smoke"
            && step.status == ComplexProjectE2eStatus::Skipped));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-project-assembly-report.json")
        .exists());
    assert!(output_root
        .join("reports")
        .join("complex-project-e2e-gate-report.json")
        .exists());
}

#[test]
fn editor_play_preview_runtime_package_cache() {
    let project_root = sample_project_root();
    let mut session = crate::complex_shooter_editor_session();
    let open = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        },
    ));
    assert_eq!(open.status, editor_core::CommandStatus::Committed);

    let first = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::Play,
    ));
    assert_eq!(first.status, editor_core::CommandStatus::Committed);
    let first_preview = session
        .last_editor_preview_package_report()
        .expect("first preview package report")
        .clone();
    assert_eq!(
        first_preview.status,
        editor_core::EditorPreviewPackageStatus::Success
    );
    assert_eq!(
        first_preview.cache_status,
        editor_core::EditorPreviewPackageCacheStatus::Rebuilt
    );
    assert!(first_preview.runtime_package_dir.is_some());
    let first_stop = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::StopPlaySession,
    ));
    assert_eq!(first_stop.status, editor_core::CommandStatus::Committed);

    let second = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::Play,
    ));
    assert_eq!(second.status, editor_core::CommandStatus::Committed);
    let second_preview = session
        .last_editor_preview_package_report()
        .expect("second preview package report");
    assert_eq!(
        second_preview.cache_status,
        editor_core::EditorPreviewPackageCacheStatus::Hit
    );
    assert!(second_preview
        .stage_reports
        .iter()
        .any(|stage| { stage.stage_id == "build_runtime_package" && stage.skipped }));
    let second_play = session
        .last_play_session_report()
        .expect("second play session report");
    assert_eq!(second_play.preview_cache_status.as_deref(), Some("Hit"));
    let second_stop = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::StopPlaySession,
    ));
    assert_eq!(second_stop.status, editor_core::CommandStatus::Committed);

    let scene_path = project_root.join("Scenes").join("Main.scene.json");
    let mut scene_text = std::fs::read_to_string(&scene_path).unwrap();
    scene_text.push_str("\n");
    std::fs::write(&scene_path, scene_text).unwrap();

    let third = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::Play,
    ));
    assert_eq!(third.status, editor_core::CommandStatus::Committed);
    let third_preview = session
        .last_editor_preview_package_report()
        .expect("third preview package report");
    assert_eq!(
        third_preview.cache_status,
        editor_core::EditorPreviewPackageCacheStatus::Rebuilt
    );
    assert!(third_preview
        .dirty_domains
        .contains(&editor_core::EditorPreviewPackageDirtyDomain::Scene));
    let third_play = session
        .last_play_session_report()
        .expect("third play session report");
    assert_eq!(third_play.preview_cache_status.as_deref(), Some("Rebuilt"));
}

#[test]
fn editor_gameview_play_runner_productization_report_covers_218_gate() {
    let output_root = temp_output_root("editor-gameview-play-runner");
    let report = run_complex_shooter_editor_gameview_play_runner_report(
        ComplexShooterEditorGameViewPlayRunnerRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterEditorGameViewPlayRunnerStatus::Passed
    );
    assert!(report.metrics.preview_package_report_present);
    assert!(report.metrics.play_session_report_present);
    assert!(report.metrics.game_view_present_report_present);
    assert_eq!(
        report.play_runner_kind.as_deref(),
        Some("editor_in_process_gameview")
    );
    assert!(report.metrics.frame_count > 0);
    assert!(report.metrics.has_frame_hash);
    assert!(report.metrics.renderable_count > 0);
    assert!(report.metrics.ui_draw_item_count > 0);
    assert_eq!(report.metrics.texture_descriptor_status, "descriptor_only");
    assert!(!matches!(
        report.metrics.input_bridge_status.as_str(),
        "deferred" | "not_requested"
    ));
    assert!(report.metrics.runtime_input_event_count > 0);
    assert!(report
        .metrics
        .gameplay_action_ids
        .iter()
        .any(|action_id| action_id == "action.fire"));
    assert!(report.metrics.runtime_selection_pick_committed);
    assert_eq!(
        report
            .metrics
            .runtime_selection_selected_entity_id
            .as_deref(),
        Some("entity-player")
    );
    assert_eq!(
        report.metrics.runtime_selection_source,
        "active_game_view_runtime"
    );
    assert_eq!(
        report.metrics.runtime_hierarchy_source_domain,
        "ActiveGameViewRuntime"
    );
    assert!(!report.metrics.runtime_inspector_readonly);
    assert!(report.metrics.runtime_inspector_temporary_play_session);
    assert!(report.metrics.runtime_inspector_transform_present);
    assert!(report.metrics.runtime_inspector_transform_editable);
    assert!(report.metrics.runtime_temporary_edit_committed);
    assert!(report.metrics.runtime_apply_preview_ready);
    assert!(report.metrics.runtime_apply_committed);
    assert!(report.metrics.runtime_apply_authoring_scene_updated);
    assert!(report.metrics.runtime_apply_pending_summary_cleared);
    assert!(report.metrics.runtime_temporary_edit_discarded_on_stop);
    assert!(report.metrics.runtime_pick_blocked_by_aui_kept_selection);
    assert!(report.metrics.runtime_pick_miss_diagnostic_present);
    assert!(report.metrics.report_panel_provider_present);
    assert!(report.metrics.viewport_descriptor_present);
    assert!(report.metrics.game_view_report_path_exists);
    assert!(report.metrics.stop_cleared_instance);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-editor-gameview-play-runner-productization-report.json")
        .exists());
}

#[test]
fn editor_gameview_gpu_texture_present_report_covers_219_gate() {
    let output_root = temp_output_root("editor-gameview-gpu-texture-present");
    let report = run_complex_shooter_editor_gameview_gpu_texture_present_report(
        ComplexShooterEditorGameViewPlayRunnerRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterEditorGameViewGpuTexturePresentStatus::Passed
    );
    assert!(report.metrics.preview_package_report_present);
    assert!(report.metrics.play_session_report_present);
    assert!(report.metrics.game_view_present_report_present);
    assert!(report.metrics.viewport_descriptor_present);
    assert!(report.metrics.has_frame_hash);
    assert!(report.metrics.frame_count > 0);
    assert_eq!(report.metrics.texture_descriptor_status, "descriptor_only");
    assert_eq!(report.metrics.gpu_present_status, "gpu_unavailable");
    assert!(report.metrics.descriptor_only_not_presented);
    assert!(report.metrics.rhi_command_count > 0);
    assert!(report.metrics.render_graph_pass_count > 0);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-editor-gameview-gpu-texture-present-productization-report.json")
        .exists());
}

#[test]
fn real_texture_present_report_covers_228_gate() {
    let output_root = temp_output_root("real-texture-present");
    let report = run_complex_shooter_real_texture_present_report(
        ComplexShooterRealTexturePresentRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterRealTexturePresentStatus::Passed
    );
    assert!(report.metrics.assembled_texture_payload_count >= 4);
    assert!(report.metrics.package_texture_asset_count >= 4);
    assert!(report.metrics.loaded_texture_payload_count >= 4);
    assert!(report.metrics.loaded_texture_byte_count > 0);
    assert!(report.metrics.render_proxy_count > 0);
    assert!(report.metrics.sprite_draw_command_count > 0);
    assert!(report.metrics.non_fallback_sprite_draw_count > 0);
    assert!(report.metrics.rhi_textured_sprite_command_count > 0);
    assert!(report.metrics.rhi_non_fallback_texture_command_count > 0);
    assert!(report.metrics.sprite_texture_binding_ready);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-real-texture-present-report.json")
        .exists());
}

#[test]
fn complex_shooter_gameplay_rule_runtime_execution_report_covers_229_gate() {
    let output_root = temp_output_root("gameplay-rule-runtime");
    let report = run_complex_shooter_gameplay_rule_runtime_report(
        ComplexShooterGameplayRuleRuntimeRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterGameplayRuleRuntimeStatus::Passed
    );
    let core = report.core_report.as_ref().expect("core report");
    assert_eq!(core.manifest_rule_count, 5);
    assert!(core
        .observed_rule_ids
        .iter()
        .any(|rule_id| rule_id == "rule.fire-bullet"));
    assert!(report.metrics.player_move_write_count > 0);
    assert!(report.metrics.fire_command_enqueue_count > 0);
    assert!(report.metrics.bullet_prefab_apply_count > 0);
    assert!(report.metrics.linear_motion_write_count > 0);
    assert!(report.metrics.lifetime_write_count > 0);
    assert!(report.metrics.collision_pair_count > 0);
    assert!(report.metrics.enemy_hp_write_count > 0);
    assert!(report.metrics.session_score_write_count > 0);
    assert!(report.metrics.score_changed);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-gameplay-rule-runtime-execution-report.json")
        .exists());
}

#[test]
fn complex_shooter_project_rule_driven_ui_state_report_covers_230_gate() {
    let output_root = temp_output_root("project-rule-driven-ui-state");
    let report = run_complex_shooter_project_rule_driven_ui_state_report(
        ComplexShooterProjectRuleDrivenUiStateRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterProjectRuleDrivenUiStateStatus::Passed
    );
    assert!(report.metrics.score_after.unwrap_or(0) > 0);
    assert!(report.metrics.score_text_matches_runtime_score);
    assert!(report.metrics.used_project_producer);
    assert!(report.metrics.active_binding_path_count > 0);
    assert!(report.metrics.produced_path_count >= report.metrics.active_binding_path_count);
    assert_eq!(report.metrics.missing_path_count, 0);
    assert_eq!(report.metrics.source_path_count, 0);
    assert_eq!(report.metrics.cache_status, "not_reported");
    let snapshot_report = report
        .ui_state_snapshot_report
        .as_ref()
        .expect("ui state snapshot report");
    assert_eq!(
        snapshot_report.producer_id,
        "complex_shooter_runtime_ui_state"
    );
    assert!(snapshot_report
        .active_binding_paths
        .iter()
        .any(|path| path == "game.score_text"));
    assert!(snapshot_report.source_paths.is_empty());
    assert!(output_root
        .join("reports")
        .join("complex-shooter-project-rule-driven-ui-state-snapshot-report.json")
        .exists());
}

#[test]
fn exported_windows_playable_golden_report_serializes() {
    let output_root = temp_output_root("exported-windows-golden-report-fixture");
    let report = run_complex_shooter_exported_windows_playable_golden_report(
        ComplexShooterExportedWindowsPlayableGoldenRequest::new(
            sample_project_root(),
            &output_root,
        ),
    );

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_REPORT_SCHEMA_VERSION));
    assert!(json.contains(COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_SCENARIO_ID));
    assert_eq!(
        report.real_window_evidence.status,
        ComplexShooterRealWindowEvidenceStatus::LocalOnlySkipped
    );
    assert!(!report.real_window_evidence.blocking);
}

#[test]
fn exported_windows_playable_golden_process_and_evidence() {
    let output_root = temp_output_root("exported-windows-golden");
    let report = run_complex_shooter_exported_windows_playable_golden_report(
        ComplexShooterExportedWindowsPlayableGoldenRequest::new(
            sample_project_root(),
            &output_root,
        ),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterExportedWindowsPlayableGoldenStatus::Passed
    );
    assert!(report.process.exported_process_contract_used);
    assert_eq!(report.process.verifier_status, "passed");
    assert_eq!(report.process.child_player_exit_code, Some(0));
    assert!(report
        .process
        .child_frames_completed
        .is_some_and(|frames| frames >= report.process.requested_frames));
    assert_eq!(report.package.target_os, "windows");
    assert_eq!(report.package.actual_host_os, std::env::consts::OS);
    assert!(!report.package.executable_name.is_empty());
    assert_eq!(
        report.golden_evidence.texture_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.gameplay_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.hud_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.aui_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.render_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert!(report.texture_evidence.loaded_texture_count >= 4);
    assert!(report.texture_evidence.uploaded_texture_count > 0);
    assert!(report.texture_evidence.sprite_texture_binding_ready);
    assert_eq!(report.texture_evidence.fallback_count, 0);
    assert_eq!(
        report.gameplay_evidence.input_source,
        "deterministic_action_snapshot"
    );
    assert!(report.gameplay_evidence.fire_action_observed);
    assert!(report.gameplay_evidence.projectile_spawn_observed);
    assert!(report.gameplay_evidence.score_changed);
    assert_eq!(report.hud_evidence.snapshot_source, "project_producer");
    assert_eq!(
        report.hud_evidence.producer_id.as_deref(),
        Some("complex_shooter_runtime_ui_state")
    );
    assert!(report
        .hud_evidence
        .active_binding_paths
        .iter()
        .any(|path| path == "game.score_text"));
    assert!(report.hud_evidence.score_text_matches_score_after);
    assert!(report.hud_evidence.rendered_glyph_count > 0);
    assert_eq!(
        report.real_window_evidence.status,
        ComplexShooterRealWindowEvidenceStatus::LocalOnlySkipped
    );
    assert_eq!(
        report.status,
        ComplexShooterExportedWindowsPlayableGoldenStatus::Passed
    );
    assert!(output_root
        .join("reports")
        .join("complex-shooter-exported-windows-playable-golden-gate-report.json")
        .exists());
}

#[test]
fn exported_windows_playable_golden_evidence() {
    let output_root = temp_output_root("exported-windows-golden-evidence");
    let report = run_complex_shooter_exported_windows_playable_golden_report(
        ComplexShooterExportedWindowsPlayableGoldenRequest::new(
            sample_project_root(),
            &output_root,
        ),
    );

    assert_eq!(
        report.golden_evidence.texture_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.gameplay_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.hud_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.aui_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert_eq!(
        report.golden_evidence.render_status,
        ComplexShooterGoldenEvidenceStatus::Passed
    );
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.artifact_id == "real-texture-present-report"));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.artifact_id == "gameplay-rule-runtime-report"));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.artifact_id == "project-rule-driven-ui-state-report"));
    assert_eq!(
        report.status,
        ComplexShooterExportedWindowsPlayableGoldenStatus::Passed
    );
}

#[test]
fn exported_windows_playable_golden_optional_real_window_skipped_remains_passed() {
    let output_root = temp_output_root("exported-windows-golden-optional-window");
    let mut request = ComplexShooterExportedWindowsPlayableGoldenRequest::new(
        sample_project_root(),
        &output_root,
    );
    request.include_optional_real_window_step = false;

    let report = run_complex_shooter_exported_windows_playable_golden_report(request);

    assert_eq!(
        report.real_window_evidence.status,
        ComplexShooterRealWindowEvidenceStatus::LocalOnlySkipped
    );
    assert!(!report.real_window_evidence.blocking);
    assert_ne!(
        report.status,
        ComplexShooterExportedWindowsPlayableGoldenStatus::Partial
    );
    assert_eq!(
        report.status,
        ComplexShooterExportedWindowsPlayableGoldenStatus::Passed
    );
}

#[test]
fn editor_build_and_run_productization_report_covers_232_gate() {
    let output_root = temp_output_root("editor-build-and-run");
    let report = run_complex_shooter_editor_build_and_run_report(
        ComplexShooterEditorBuildAndRunRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_SCENARIO_ID
    );
    assert_eq!(report.status, ComplexShooterEditorBuildAndRunStatus::Passed);
    assert!(report.metrics.open_project_committed);
    assert!(report.metrics.command_committed);
    assert!(report.metrics.editor_report_present);
    assert_eq!(report.metrics.editor_report_status, "verification_passed");
    assert_eq!(report.metrics.export_status, "success");
    assert!(report.metrics.launch_attempted);
    assert!(report.metrics.launch_started);
    assert_eq!(report.metrics.editor_headless_verification_status, "passed");
    assert!(report.metrics.exported_process_contract_used);
    assert_eq!(report.metrics.verifier_status, "passed");
    assert_eq!(report.metrics.child_player_exit_code, Some(0));
    assert!(report
        .metrics
        .child_frames_completed
        .is_some_and(|frames| frames >= 3));
    assert!(report.metrics.report_panel_provider_present);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-editor-build-and-run-productization-report.json")
        .exists());
}

#[test]
fn e2e_gate_reports_missing_project_as_failed() {
    let output_root = temp_output_root("missing-project-e2e");
    let report = run_complex_project_e2e_gate(ComplexProjectE2eGateRequest::new(
        output_root.join("missing-project"),
        output_root.join("out"),
    ));

    assert_eq!(report.status, ComplexProjectE2eStatus::Failed);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SampleProjectManifestReadFailed"));
}

#[test]
fn authoring_to_playable_report_serializes() {
    let report = AuthoringToPlayableVerticalSliceReport {
        schema_version: AUTHORING_TO_PLAYABLE_VERTICAL_SLICE_REPORT_SCHEMA_VERSION.to_string(),
        gate_id: "complex-shooter-authoring-to-playable-vertical-slice-v1".to_string(),
        status: AuthoringToPlayableVerticalSliceStatus::Passed,
        source_project_path: "source".to_string(),
        working_project_path: "working".to_string(),
        output_root: "output".to_string(),
        editor_mode: None,
        project_id: None,
        active_scene_id: None,
        can_play: false,
        can_build: false,
        steps: Vec::new(),
        workspace_domains: Vec::new(),
        workflow_steps: Vec::new(),
        metrics: AuthoringToPlayableMetrics::default(),
        e2e_report: None,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(AUTHORING_TO_PLAYABLE_VERTICAL_SLICE_REPORT_SCHEMA_VERSION));
    assert!(json.contains("complex-shooter-authoring-to-playable-vertical-slice-v1"));
}

#[test]
fn vertical_slice_opens_sample_project_through_editor_session() {
    let output_root = temp_output_root("authoring-to-playable-editor-open");
    let mut request =
        AuthoringToPlayableVerticalSliceRequest::new(sample_project_root(), &output_root);
    request.include_optional_real_window_step = false;
    request.frame_limit = 1;

    let report = run_authoring_to_playable_vertical_slice(request);

    assert_eq!(
        report.editor_mode,
        Some(editor_ui_model::EditorUiMode::AuthoringWorkspace)
    );
    assert!(report.project_id.is_some());
    assert_eq!(report.active_scene_id.as_deref(), Some("scene-main"));
    assert!(report
        .workspace_domains
        .iter()
        .any(|domain| domain.domain == "scene" && domain.item_count >= 6));
    assert!(report
        .workspace_domains
        .iter()
        .any(|domain| domain.domain == "asset" && domain.item_count >= 5));
    assert!(report
        .workflow_steps
        .iter()
        .any(|step| step.step_id == "scene"));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step_id == "editor-authoring-readiness"));
}

#[test]
fn vertical_slice_runs_export_package_and_player_chain() {
    let output_root = temp_output_root("authoring-to-playable-full");
    let report = run_authoring_to_playable_vertical_slice(
        AuthoringToPlayableVerticalSliceRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.status,
        AuthoringToPlayableVerticalSliceStatus::Passed
    );
    assert!(report.metrics.runtime_package_entity_count >= 6);
    assert!(report.metrics.frames_run >= 1);
    assert!(report.metrics.present_count >= 1);
    assert!(report.metrics.draw_item_count > 0);
    assert!(report.e2e_report.is_some());
    assert!(output_root
        .join("reports")
        .join("authoring-to-playable-vertical-slice-report.json")
        .exists());
}

#[test]
fn manual_walkthrough_coverage_reports_complex_project_authoring_gaps() {
    let output_root = temp_output_root("manual-walkthrough-coverage");
    let report = run_complex_shooter_manual_walkthrough_coverage(
        ComplexShooterManualWalkthroughCoverageRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_MANUAL_WALKTHROUGH_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        editor_ui_model::ManualWalkthroughCoverageStatus::Partial
    );
    assert!(report.operation_count >= 60);
    assert!(!report.domain_summaries.is_empty());
    assert!(!report.blocking_gaps.is_empty());
    assert!(!report.next_actions.is_empty());
    assert!(report.domain_summaries.iter().any(|summary| summary.domain
        == editor_ui_model::WorkspaceDomainKind::Prefab
        && summary.missing_count == 0
        && summary.needs_context_count > 0));
    assert!(report.domain_summaries.iter().any(|summary| summary.domain
        == editor_ui_model::WorkspaceDomainKind::Rule
        && summary.missing_count == 0
        && summary.needs_context_count > 0));
    assert!(report.domain_summaries.iter().any(|summary| summary.domain
        == editor_ui_model::WorkspaceDomainKind::Aui
        && summary.missing_count == 0
        && summary.needs_context_count > 0));
    for operation in &report.operations {
        let id = operation.requirement.operation_id.to_ascii_lowercase();
        for forbidden in ["player", "enemy", "bullet", "score", "health", "weapon"] {
            assert!(
                !id.contains(forbidden),
                "operation id should stay generic: {}",
                operation.requirement.operation_id
            );
        }
    }
    assert!(output_root
        .join("reports")
        .join("manual-walkthrough-coverage-report.json")
        .exists());
}

#[test]
fn aui_authoring_productization_report_runs_command_smoke_and_sample_preview() {
    let output_root = temp_output_root("aui-authoring-productization");
    let report = run_complex_shooter_aui_authoring_report(ComplexShooterAuiAuthoringRequest::new(
        sample_project_root(),
        &output_root,
    ));

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_AUTHORING_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_AUTHORING_SCENARIO_ID
    );
    assert_eq!(report.status, ComplexShooterAuiAuthoringStatus::Partial);
    assert_eq!(report.metrics.sample_document_count, 1);
    assert_eq!(report.metrics.sample_legacy_document_count, 1);
    assert_eq!(report.metrics.smoke_command_count, 11);
    assert_eq!(report.metrics.smoke_committed_command_count, 11);
    assert_eq!(report.metrics.preview_partial_count, 0);
    assert!(!report
        .next_actions
        .contains(&"runtime_text_glyph_present".to_string()));
    assert!(report
        .sample_documents
        .iter()
        .any(|document| document.source_path == "AUI/hud.aui.json" && document.validation_ok));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-authoring-productization-report.json")
        .exists());
}

#[test]
fn aui_scene_unified_authoring_report_builds_proxy_visual_order_and_runtime_gap_evidence() {
    let output_root = temp_output_root("aui-scene-unified-authoring");
    let report = run_complex_shooter_aui_scene_authoring_report(
        ComplexShooterAuiSceneAuthoringRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterAuiSceneAuthoringStatus::Partial
    );
    assert_eq!(report.metrics.document_count, 1);
    assert!(report.metrics.proxy_count > 0);
    assert!(report.metrics.visual_order_entry_count > 0);
    assert!(report.visual_order_runtime_supported);
    assert_eq!(report.metrics.runtime_composition_gap_count, 0);
    assert!(report.next_required_runtime_gate.is_none());
    assert!(report
        .next_actions
        .contains(&"AUI Prefab / Template Reuse Productization v1".to_string()));
    assert!(report
        .documents
        .iter()
        .any(|document| document.source_path == "AUI/hud.aui.json"
            && document.selected_node_id.is_some()));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-scene-unified-authoring-report.json")
        .exists());
}

#[test]
fn aui_template_reuse_report_instantiates_repeated_equipment_slot_fixture() {
    let output_root = temp_output_root("aui-template-reuse");
    let report = run_complex_shooter_aui_template_reuse_report(
        ComplexShooterAuiTemplateReuseRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_SCENARIO_ID
    );
    assert_eq!(report.status, ComplexShooterAuiTemplateReuseStatus::Passed);
    assert_eq!(report.metrics.template_node_count, 4);
    assert_eq!(report.metrics.instance_count, 3);
    assert_eq!(report.metrics.inserted_node_count, 12);
    assert_eq!(report.metrics.node_id_remap_count, 12);
    assert_eq!(report.metrics.copied_binding_ref_count, 6);
    assert_eq!(report.metrics.copied_action_ref_count, 3);
    assert_eq!(report.metrics.copied_asset_ref_count, 3);
    assert!(report.metrics.warning_count >= 6);
    assert!(report.metrics.draw_command_count > 0);
    assert!(report.metrics.composition_draw_item_count > 0);
    assert!(report
        .instantiate_reports
        .iter()
        .all(|instance| instance.status == editor_core::AuiTemplateOperationStatus::Partial));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-template-reuse-productization-report.json")
        .exists());
}

#[test]
fn aui_interaction_runtime_productization_report_filters_click_and_reports_drag_drop() {
    let output_root = temp_output_root("aui-runtime-interaction");
    let report = run_complex_shooter_aui_runtime_interaction_report(
        ComplexShooterAuiRuntimeInteractionRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterAuiRuntimeInteractionStatus::Passed
    );
    assert_eq!(report.metrics.click_action_count, 1);
    assert_eq!(report.metrics.click_filtered_input_event_count, 0);
    assert!(!report.metrics.gameplay_fire_triggered_after_ui_click);
    assert_eq!(report.metrics.drag_start_count, 1);
    assert_eq!(report.metrics.drop_count, 1);
    assert_eq!(report.metrics.drag_cancel_count, 1);
    assert!(!report.metrics.payload_project_semantics_detected);
    assert_eq!(report.metrics.deferred_flag_count, 1);
    assert_eq!(report.click_report.snapshot_frame_lag, 1);
    assert!(report.click_report.authoring_action_payload_deferred);
    assert!(!report.click_report.modal_input_blocking_deferred);
    assert!(!report.click_report.editor_hit_test_deferred_to_209);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-runtime-interaction-productization-report.json")
        .exists());
}

#[test]
fn aui_complex_controls_productization_report_covers_modal_focus_scroll_and_scene_hit() {
    let output_root = temp_output_root("aui-complex-controls-productization");
    let report = run_complex_shooter_aui_complex_controls_report(
        ComplexShooterAuiComplexControlsRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterAuiComplexControlsStatus::Passed
    );
    assert_eq!(report.metrics.modal_consumed_pointer_count, 1);
    assert_eq!(report.metrics.modal_consumed_wheel_count, 1);
    assert_eq!(report.metrics.modal_consumed_keyboard_count, 1);
    assert!(report.metrics.focus_change_count > 0);
    assert_eq!(report.metrics.cancel_action_count, 1);
    assert!(report.metrics.wheel_scroll_offset_change_count > 0);
    assert!(report.metrics.drag_scroll_offset_change_count > 0);
    assert_eq!(report.metrics.scroll_offset_applied_count, 2);
    assert!(report.metrics.scene_selectable_proxy_count > 0);
    assert_eq!(report.metrics.deferred_flag_count, 2);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-complex-controls-productization-report.json")
        .exists());
}

#[test]
fn aui_rectclip_scrollbar_navigation_productization_report_covers_215_gate() {
    let output_root = temp_output_root("aui-rectclip-scrollbar-navigation-productization");
    let report = run_complex_shooter_aui_rectclip_scrollbar_navigation_report(
        ComplexShooterAuiRectClipScrollbarNavigationRequest::new(
            sample_project_root(),
            &output_root,
        ),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterAuiRectClipScrollbarNavigationStatus::Passed
    );
    assert!(report.metrics.clip_root_count > 0);
    assert!(report.metrics.effective_clip_item_count > 0);
    assert!(report.metrics.culled_draw_item_count > 0);
    assert!(report.metrics.hit_test_clip_rejected_count > 0);
    assert!(report.metrics.scrollbar_visible_count > 0);
    assert!(report.metrics.scrollbar_thumb_drag_count > 0);
    assert!(report.metrics.scrollbar_offset_change_count > 0);
    assert!(report.metrics.keyboard_navigation_event_count > 0);
    assert!(report.metrics.focus_move_count > 0);
    assert!(report.metrics.focus_visible_scroll_count > 0);
    assert_eq!(report.metrics.deferred_flag_count, 6);
    assert!(report
        .core_report
        .focused_node_after
        .as_deref()
        .is_some_and(|node| node == "equipment_slot_2"));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-rectclip-scrollbar-navigation-productization-report.json")
        .exists());
}

#[test]
fn aui_runtime_navigation_screenflow_textentry_productization_report_covers_216_gate() {
    let output_root = temp_output_root("aui-runtime-navigation-screenflow-textentry");
    let report = run_complex_shooter_aui_runtime_navigation_screenflow_textentry_report(
        ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryRequest::new(
            sample_project_root(),
            &output_root,
        ),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Passed
    );
    assert!(report.metrics.screen_stack_push_count > 0);
    assert!(report.metrics.screen_stack_pop_count > 0);
    assert!(report.metrics.default_focus_applied_count > 0);
    assert!(report.metrics.focus_restore_count > 0);
    assert!(report.metrics.gamepad_intent_count >= 2);
    assert!(report.metrics.keyboard_navigation_event_count > 0);
    assert!(report.metrics.submit_count > 0);
    assert!(report.metrics.cancel_count > 0);
    assert!(report.metrics.text_edit_session_count > 0);
    assert!(report.metrics.text_changed_count > 0);
    assert!(report.metrics.text_submitted_count > 0);
    assert!(report.metrics.ime_preedit_count > 0);
    assert!(report.metrics.ime_commit_count > 0);
    assert!(report.metrics.ime_cancel_count > 0);
    assert!(report.metrics.gameplay_input_filtered_count > 0);
    assert_eq!(report.metrics.deferred_flag_count, 10);
    assert_eq!(
        report.core_report.ime_platform_coverage,
        "schema_headless_and_winit_cmin"
    );
    assert!(output_root
        .join("reports")
        .join("complex-shooter-aui-runtime-navigation-screenflow-textentry-productization-report.json")
        .exists());
}

#[test]
fn unified_report_panel_productization_report_registers_all_report_domains() {
    let output_root = temp_output_root("unified-report-panel-productization");
    let report = run_complex_shooter_unified_report_panel(
        ComplexShooterUnifiedReportPanelRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ComplexShooterUnifiedReportPanelStatus::Passed
    );
    assert!(report.metrics.provider_count >= 9);
    assert_eq!(report.metrics.report_count, report.metrics.ai_context_count);
    for provider_id in [
        "build.export",
        "play.runtime",
        "authoring.asset_browser",
        "authoring.aui",
        "authoring.rule",
        "authoring.prefab",
        "project.patch",
        "authoring.manual_walkthrough",
        "editor.diagnostics",
        "project_e2e.complex_shooter",
    ] {
        assert!(
            report
                .provider_evidence
                .iter()
                .any(|evidence| evidence.provider_id == provider_id && evidence.report_present),
            "missing provider {provider_id}"
        );
    }
    for domain in ["build", "play", "asset", "aui", "rule", "prefab", "report"] {
        assert!(
            report
                .domain_coverage
                .iter()
                .any(|covered| covered == domain),
            "missing domain {domain}"
        );
    }
    assert!(output_root
        .join("reports")
        .join("complex-shooter-unified-report-panel-productization-report.json")
        .exists());
}

#[test]
fn project_patch_productization_smoke_reports_all_domain_capability() {
    let output_root = temp_output_root("project-patch-productization");
    let report = run_complex_shooter_project_patch_smoke(ComplexShooterProjectPatchRequest::new(
        sample_project_root(),
        &output_root,
    ));

    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_PROJECT_PATCH_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        editor_core::ProjectPatchProductizationStatus::Pass
    );
    assert_eq!(
        report.supported_capabilities,
        vec![
            editor_core::PatchCapability::Asset,
            editor_core::PatchCapability::Prefab,
            editor_core::PatchCapability::Aui,
            editor_core::PatchCapability::Rule,
            editor_core::PatchCapability::Build
        ]
    );
    assert!(report.unsupported_capabilities.is_empty());
    assert_eq!(report.history_summary.applied_count, 1);
    assert!(report
        .apply_report
        .as_ref()
        .is_some_and(|apply| apply.status == editor_core::PatchApplyStatus::Committed));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-all-domain-project-patch-report.json")
        .exists());
}

#[test]
fn authoring_asset_completeness_report_detects_prefab_and_rule_assetization_gaps() {
    let project_root = sample_project_root();
    strip_authoring_asset_completeness_cmin(&project_root);
    let output_root = temp_output_root("authoring-asset-completeness-gaps");

    let report = run_complex_shooter_authoring_asset_completeness_report(
        ProjectAuthoringAssetCompletenessRequest::new(&project_root, &output_root),
    );

    assert_eq!(
        report.schema_version,
        PROJECT_AUTHORING_ASSET_COMPLETENESS_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        PROJECT_AUTHORING_ASSET_COMPLETENESS_SCENARIO_ID
    );
    assert_eq!(
        report.status,
        ProjectAuthoringAssetCompletenessStatus::Partial
    );
    assert_eq!(report.prefab_summary.prefab_asset_count, 3);
    assert_eq!(report.prefab_summary.scene_prefab_instance_count, 0);
    assert!(report.prefab_summary.missing_scene_instance_evidence);
    assert_eq!(report.rule_summary.runtime_manifest_rule_count, 5);
    assert_eq!(report.rule_summary.rule_authoring_asset_count, 0);
    assert_eq!(report.rule_summary.missing_authoring_rule_ids.len(), 5);
    assert_eq!(report.rule_summary.migration_candidate_count, 5);
    assert!(report.candidates.iter().any(|candidate| {
        candidate.domain == AssetizationCandidateDomain::Prefab
            && candidate.apply_route == "convert_scene_entity_to_prefab_instance"
            && candidate.scene_entity_id.as_deref() == Some("entity-enemy-a")
    }));
    assert!(report.candidates.iter().any(|candidate| {
        candidate.domain == AssetizationCandidateDomain::Rule
            && candidate.rule_id.as_deref() == Some("rule.fire-bullet")
            && candidate.target_path.as_deref() == Some("Rules/fire_bullet.rule.json")
    }));
    assert!(output_root
        .join("reports")
        .join("project-authoring-asset-completeness-report.json")
        .exists());
}

#[test]
fn authoring_asset_completeness_report_passes_after_cmin_assetization() {
    let output_root = temp_output_root("authoring-asset-completeness-cmin");
    let report = run_complex_shooter_authoring_asset_completeness_report(
        ProjectAuthoringAssetCompletenessRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.status,
        ProjectAuthoringAssetCompletenessStatus::Passed
    );
    assert_eq!(report.prefab_summary.prefab_asset_count, 3);
    assert!(report.prefab_summary.scene_prefab_instance_count >= 2);
    assert!(report.prefab_summary.runtime_spawn_reference_count >= 1);
    assert!(!report.prefab_summary.missing_scene_instance_evidence);
    assert_eq!(report.rule_summary.runtime_manifest_rule_count, 5);
    assert_eq!(report.rule_summary.rule_authoring_asset_count, 5);
    assert!(report.rule_summary.missing_authoring_rule_ids.is_empty());
    assert!(report.rule_summary.stale_authoring_rule_ids.is_empty());
    assert!(report.next_actions.is_empty());
    assert!(report.candidates.iter().any(|candidate| {
        candidate.domain == AssetizationCandidateDomain::Prefab
            && candidate.apply_route == "existing_runtime_spawn_reference"
            && candidate.prefab_asset_id.as_deref() == Some("prefab-player-bullet")
    }));
    assert!(!report.candidates.iter().any(|candidate| {
        candidate.domain == AssetizationCandidateDomain::Prefab
            && candidate.apply_route == "convert_scene_entity_to_prefab_instance"
            && candidate.scene_entity_id.as_deref() == Some("entity-enemy-b")
    }));
    assert!(output_root
        .join("reports")
        .join("project-authoring-asset-completeness-report.json")
        .exists());
}

#[test]
fn imported_project_patch_productization_smoke_writes_import_report() {
    let output_root = temp_output_root("imported-project-patch-productization");
    let report = run_complex_shooter_imported_project_patch_smoke(
        ComplexShooterProjectPatchRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_IMPORTED_PROJECT_PATCH_SCENARIO_ID
    );
    assert_eq!(
        report.schema_version,
        editor_core::PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.status,
        editor_core::ProjectPatchImportProductizationStatus::Pass
    );
    assert_eq!(
        report.parse_status,
        editor_core::ProjectPatchImportParseStatus::Parsed
    );
    assert!(report
        .validation
        .as_ref()
        .is_some_and(|validation| validation.accepted));
    assert!(report
        .review
        .as_ref()
        .is_some_and(|review| review.operation_count == 5));
    assert!(report
        .apply_report
        .as_ref()
        .is_some_and(|apply| apply.status == editor_core::PatchApplyStatus::Committed));
    assert_eq!(report.history_summary.applied_count, 1);
    assert_eq!(
        report.supported_capabilities,
        vec![
            editor_core::PatchCapability::Asset,
            editor_core::PatchCapability::Prefab,
            editor_core::PatchCapability::Aui,
            editor_core::PatchCapability::Rule,
            editor_core::PatchCapability::Build
        ]
    );
    assert!(report.unsupported_capabilities.is_empty());
    assert!(output_root
        .join("reports")
        .join("complex-shooter-imported-project-patch-productization-report.json")
        .exists());
    assert!(output_root
        .join("fixtures")
        .join("imported-project-patch-smoke.json")
        .exists());
}

#[test]
fn llm_patch_source_project_patch_productization_smoke_writes_ai_structured_output_report() {
    let output_root = temp_output_root("llm-patch-source-productization");
    let report = run_complex_shooter_llm_patch_source_smoke(
        ComplexShooterProjectPatchRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_LLM_PATCH_SOURCE_SCENARIO_ID
    );
    assert_eq!(
        report.schema_version,
        editor_core::PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.status,
        editor_core::ProjectPatchImportProductizationStatus::Pass
    );
    assert_eq!(
        report.source_kind,
        editor_core::ProjectPatchImportSourceKind::AiStructuredOutput
    );
    assert!(report
        .validation
        .as_ref()
        .is_some_and(|validation| validation.accepted));
    assert!(report
        .apply_report
        .as_ref()
        .is_some_and(|apply| apply.status == editor_core::PatchApplyStatus::Committed));
    assert_eq!(report.history_summary.applied_count, 1);
    assert_eq!(
        report.supported_capabilities,
        vec![
            editor_core::PatchCapability::Asset,
            editor_core::PatchCapability::Prefab,
            editor_core::PatchCapability::Aui,
            editor_core::PatchCapability::Rule,
            editor_core::PatchCapability::Build
        ]
    );
    assert!(report.unsupported_capabilities.is_empty());
    assert!(output_root
        .join("reports")
        .join("complex-shooter-llm-patch-source-productization-report.json")
        .exists());
}

#[test]
fn llm_provider_repair_complex_shooter_fake_http_e2e() {
    let project_root = sample_project_root();
    let mut session = complex_shooter_editor_session();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                editor_ui_model::UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                },
            ))
            .status,
        editor_core::CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                editor_ui_model::UiCommandPayload::OpenSceneDocument {
                    path: project_root
                        .join("Scenes/Main.scene.json")
                        .display()
                        .to_string(),
                },
            ))
            .status,
        editor_core::CommandStatus::Committed
    );

    let all_domain = editor_core::ThinLlmPatchSource::generate_project_patch_json(
        &editor_core::LlmPatchSourceConfig::deterministic_mock(),
        "create all_domain patch",
        "{}",
    )
    .raw_json
    .unwrap();
    let repaired_scene = editor_core::ThinLlmPatchSource::generate_project_patch_json(
        &editor_core::LlmPatchSourceConfig::deterministic_mock(),
        "create \"Repaired Fire Point\"",
        "{}",
    )
    .raw_json
    .unwrap();
    let (base_url, requests) =
        spawn_llm_fake_server(vec!["{not-project-patch".to_string(), repaired_scene]);
    let mut config = editor_core::LlmPatchSourceConfig::deterministic_mock();
    config.source_kind = editor_core::LlmPatchSourceKind::OpenAiCompatible;
    config.provider_id = "complex-shooter-fake-provider".to_string();
    config.model = "complex-shooter-project-patch".to_string();
    config.base_url = base_url;
    config.api_key = editor_core::RedactedSecret::new("complex-shooter-test-secret");
    session.set_llm_patch_source_config_for_test(config);

    let start = session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "repair then stage a complex shooter all-domain patch".to_string(),
        },
    ));
    assert_eq!(start.status, editor_core::CommandStatus::Committed);
    pump_e2e_llm(&mut session);

    let model = session.build_ui_model();
    assert_eq!(
        model.ai_panel.proposed_commands.len(),
        1,
        "status={:?} summary={:?} messages={:?} diagnostics={:?}",
        model.ai_panel.stage,
        model.ai_panel.status_summary,
        model.ai_panel.messages,
        model.console.entries
    );
    let proposal = &model.ai_panel.proposed_commands[0];
    let evidence = proposal.project_patch.as_ref().unwrap();
    assert!(evidence.repaired_once);
    assert!(evidence.requires_confirmation);
    assert_eq!(evidence.touched_domains, vec!["Scene"]);
    assert!(session.patch_history().entries.is_empty());
    assert!(requests
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .to_ascii_lowercase()
        .contains("authorization: bearer complex-shooter-test-secret"));
    assert!(requests.recv_timeout(Duration::from_secs(1)).is_ok());

    let apply = session.execute_command(editor_core::command_for_test(proposal.command.clone()));
    assert_eq!(
        apply.status,
        editor_core::CommandStatus::Committed,
        "{apply:#?}\npreview={:#?}\nconsole={:#?}",
        session.last_editor_preview_package_report(),
        session.build_ui_model().console.entries
    );
    assert_eq!(session.patch_history().entries.len(), 1);

    let (all_domain_url, _) = spawn_llm_fake_server(vec![all_domain]);
    let mut all_domain_config = editor_core::LlmPatchSourceConfig::deterministic_mock();
    all_domain_config.source_kind = editor_core::LlmPatchSourceKind::OpenAiCompatible;
    all_domain_config.provider_id = "complex-shooter-fake-provider".to_string();
    all_domain_config.model = "complex-shooter-all-domain".to_string();
    all_domain_config.base_url = all_domain_url;
    session.set_llm_patch_source_config_for_test(all_domain_config);
    session.execute_command(editor_core::command_for_test(
        editor_ui_model::UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "stage all complex shooter authoring domains".to_string(),
        },
    ));
    pump_e2e_llm(&mut session);
    let model = session.build_ui_model();
    let proposal = model
        .ai_panel
        .proposed_commands
        .iter()
        .find(|proposal| {
            proposal
                .project_patch
                .as_ref()
                .is_some_and(|evidence| evidence.patch_id == "llm-mock-all-domain")
        })
        .unwrap();
    let evidence = proposal.project_patch.as_ref().unwrap();
    assert!(!evidence.repaired_once);
    for domain in ["Asset", "Prefab", "Aui", "Rule", "Build"] {
        assert!(evidence.touched_domains.iter().any(|value| value == domain));
    }
    assert_eq!(session.patch_history().entries.len(), 1);
    assert!(!project_root.join("UI/llm-all-domain-hud.aui.json").exists());
    let apply = session.execute_command(editor_core::command_for_test(proposal.command.clone()));
    assert_eq!(apply.status, editor_core::CommandStatus::Committed);
    assert_eq!(session.patch_history().entries.len(), 2);
}

fn spawn_llm_fake_server(candidates: Vec<String>) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for candidate in candidates {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("fake LLM server failed to accept request: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 8192];
            let header_end = loop {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
                if let Some(index) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            while request.len() < header_end + content_length {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
            }
            let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
            let body = serde_json::json!({
                "choices": [{
                    "message": { "content": candidate, "refusal": null },
                    "finish_reason": "stop"
                }]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}/v1"), receiver)
}

fn pump_e2e_llm(session: &mut editor_core::EditorSession) {
    for _ in 0..500 {
        let _ = session.pump_llm_patch_request();
        if !session.has_active_llm_patch_request() {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("complex shooter fake LLM request did not settle");
}

#[test]
fn prefab_authoring_productization_report_distinguishes_assets_and_instances() {
    let output_root = temp_output_root("prefab-authoring-productization");
    let report = run_complex_shooter_prefab_authoring_report(
        ComplexShooterPrefabAuthoringRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_PREFAB_AUTHORING_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_PREFAB_AUTHORING_SCENARIO_ID
    );
    assert_eq!(report.status, ComplexShooterPrefabAuthoringStatus::Passed);
    assert_eq!(report.prefab_assets_count, 3);
    assert_eq!(report.prefab_instances_count, 2);
    assert!(report
        .prefab_assets
        .iter()
        .any(|asset| asset.prefab_id.as_deref() == Some("prefab-player-bullet")));
    assert!(!report.next_actions.contains(
        &"sample_scene_has_no_engine_prefab_instance_yet_use_instantiate_prefab_in_scene"
            .to_string()
    ));
    assert_eq!(report.metrics.smoke_command_count, 14);
    assert_eq!(report.metrics.smoke_committed_command_count, 14);
    assert_eq!(report.metrics.smoke_prefab_assets_count, 1);
    assert_eq!(report.metrics.smoke_prefab_instances_count, 2);
    assert_eq!(report.metrics.smoke_applied_override_count, 1);
    assert_eq!(report.metrics.smoke_reverted_override_count, 1);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-prefab-authoring-productization-report.json")
        .exists());
}

#[test]
fn prefab_runtime_bake_report_expands_scene_prefab_instances() {
    let output_root = temp_output_root("prefab-runtime-bake");
    let report = run_complex_shooter_prefab_runtime_bake_report(
        ComplexShooterPrefabRuntimeBakeRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_SCENARIO_ID
    );
    assert_eq!(report.status, ComplexShooterPrefabRuntimeBakeStatus::Passed);
    assert_eq!(
        report.assembly_status,
        Some(editor_core::ProjectRuntimePackageAssemblyStatus::Success)
    );
    assert!(report.metrics.baked_instance_count >= 2);
    assert!(report.metrics.enemy_a_baked);
    assert!(report.metrics.enemy_b_baked);
    assert_eq!(
        report.metrics.runtime_scene_prefab_instance_component_count,
        0
    );
    assert!(report
        .baked_instances
        .iter()
        .any(|instance| instance.scene_entity_id == "entity-enemy-a"));
    assert!(report
        .baked_instances
        .iter()
        .any(|instance| instance.scene_entity_id == "entity-enemy-b"));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-prefab-runtime-bake-report.json")
        .exists());
}

#[test]
fn rule_authoring_productization_report_is_honest_about_sample_authoring_assets() {
    let output_root = temp_output_root("rule-authoring-productization");
    let report = run_complex_shooter_rule_authoring_report(
        ComplexShooterRuleAuthoringRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_RULE_AUTHORING_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.runtime_manifest_count, 1);
    assert_eq!(report.status, ComplexShooterRuleAuthoringStatus::Passed);
    assert_eq!(report.rule_asset_count, 5);
    assert!(report.diagnostics.is_empty());
    assert!(!report
        .next_actions
        .contains(&"create_rule_asset".to_string()));
    assert!(output_root
        .join("reports")
        .join("rule-authoring-productization-report.json")
        .exists());
}

#[test]
fn rule_card_authoring_productization_report_proves_card_edit_and_graph_refresh() {
    let output_root = temp_output_root("rule-card-authoring-productization");
    let report = run_complex_shooter_rule_card_authoring_report(
        ComplexShooterRuleCardAuthoringRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.scenario_id,
        COMPLEX_SHOOTER_RULE_CARD_AUTHORING_SCENARIO_ID
    );
    assert_eq!(report.status, ComplexShooterRuleCardAuthoringStatus::Passed);
    assert_eq!(report.covered_rule_paths.len(), 3);
    assert!(report
        .rule_evidence
        .iter()
        .all(|evidence| evidence.card_count >= 1 && evidence.read_only_graph));
    let edit = report
        .edit_evidence
        .as_ref()
        .expect("edit evidence should exist");
    assert_eq!(edit.command_status, "Committed");
    assert_eq!(edit.validate_status, "Valid");
    assert_eq!(edit.build_status, "Built");
    assert!(edit.graph_refreshed);
    assert!(edit.operation_card_present_after_edit);
    assert!(edit.source_mappings.iter().any(|mapping| {
        mapping.source_path == "canonicalIr.operations[0]"
            && mapping.card_id.as_deref() == Some("card:operation:0")
            && mapping.node_id.as_deref() == Some("node:operation:0")
    }));
    assert!(output_root
        .join("reports")
        .join("complex-shooter-rule-card-authoring-productization-report.json")
        .exists());
}

#[test]
fn complex_shooter_input_mapping_visual_authoring_e2e_gate_passes() {
    let output_root = temp_output_root("input-mapping-visual-authoring");
    let report = run_complex_shooter_input_mapping_visual_authoring_report(
        ComplexShooterInputMappingVisualAuthoringRequest::new(sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.status,
        ComplexShooterInputMappingVisualAuthoringStatus::Passed,
        "diagnostics={:?}",
        report.diagnostics
    );
    assert!(report.unsaved_draft_excluded_from_package);
    assert!(report.saved_mapping_included_in_package);
    assert!(report.preview_resolved_pause);
    assert!(report.runtime_resolved_pause);
    assert_eq!(
        report.metrics.stable_binding_id_count,
        report.metrics.binding_count
    );
    assert!(output_root
        .join("reports")
        .join("complex-shooter-input-mapping-visual-authoring-report.json")
        .exists());
}

#[test]
fn complex_shooter_asset_browser_native_productization_e2e_gate_passes() {
    let output_root = temp_output_root("asset-browser-native-productization");
    let report = run_complex_shooter_asset_browser_native_report(
        ComplexShooterAssetBrowserNativeRequest::new(source_sample_project_root(), &output_root),
    );

    assert_eq!(
        report.schema_version,
        COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.status,
        ComplexShooterAssetBrowserNativeStatus::Passed,
        "diagnostics={:?}",
        report.diagnostics
    );
    assert!(report.no_rescan_for_300_frames);
    assert!(report.excluded_generated_roots);
    assert!(report.query_result_count >= 2);
    assert!(report.selected_entry_key.is_some());
    assert!(report.thumbnail.non_empty_alpha_pixel);
    assert!(report.thumbnail.within_budget);
    assert_eq!(report.drag_drop.transaction_status, "Committed");
    assert!(report.picker.cancel_preserved_source_hash);
    assert_eq!(report.picker.confirm_status, "Committed");
    assert!(report.picker.structured_reference_written);
    assert!(report.save_reload.reference_preserved);
    assert!(report.runtime_package.package_loaded);
    assert!(report.runtime_package.runtime_asset_index_resolved);
    assert!(report.path_safety.traversal_rejected);
    assert!(report.path_safety.root_escape_rejected);
    assert!(report.report_panel_provider_present);
    assert!(report.report_panel_trace_evidence_count > 1);
    assert!(output_root
        .join("reports")
        .join("complex-shooter-asset-browser-native-productization-report.json")
        .exists());
}

fn sample_project_root() -> std::path::PathBuf {
    let source = source_sample_project_root();
    let root = temp_output_root("sample-project-copy");
    copy_dir_recursive(&source, &root).expect("sample project copy should succeed");
    let _ = std::fs::remove_dir_all(root.join(".aife"));
    let _ = std::fs::remove_dir_all(root.join("Library/ProjectIntent"));
    root
}

fn source_sample_project_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("samples")
        .join("complex_shooter_project")
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        if source_path.is_dir() && entry.file_name() == "Build" {
            continue;
        }
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            std::fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn strip_authoring_asset_completeness_cmin(project_root: &Path) {
    for file_name in [
        "player_move.rule.json",
        "fire_bullet.rule.json",
        "linear_motion.rule.json",
        "lifetime_cleanup.rule.json",
        "collision_response.rule.json",
    ] {
        let _ = std::fs::remove_file(project_root.join("Rules").join(file_name));
    }

    let scene_path = project_root.join("Scenes").join("Main.scene.json");
    let text = std::fs::read_to_string(&scene_path).expect("scene should read");
    let mut scene: serde_json::Value = serde_json::from_str(&text).expect("scene should parse");
    let entities = scene
        .get_mut("entities")
        .and_then(serde_json::Value::as_array_mut)
        .expect("scene entities should be an array");
    for entity in entities {
        let entity_id = entity
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let Some(components) = entity
            .get_mut("components")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        components.retain(|component| {
            component
                .get("componentType")
                .or_else(|| component.get("component_type"))
                .and_then(serde_json::Value::as_str)
                != Some("engine.prefab_instance")
        });
        match entity_id.as_deref() {
            Some("entity-enemy-a") => {
                components.push(serde_json::json!({
                    "componentType": "SpriteRenderer2D",
                    "data": {
                        "spriteRef": { "id": "tex-enemy-scout", "type": "texture" },
                        "sortingLayer": 0,
                        "orderInLayer": 10,
                        "visible": true
                    }
                }));
                components.push(serde_json::json!({
                    "componentType": "project.linearMotion",
                    "data": { "velocity": { "x": 0.6, "y": -1.2 } }
                }));
            }
            Some("entity-enemy-b") => {
                components.push(serde_json::json!({
                    "componentType": "SpriteRenderer2D",
                    "data": {
                        "spriteRef": { "id": "tex-enemy-scout", "type": "texture" },
                        "sortingLayer": 0,
                        "orderInLayer": 11,
                        "visible": true
                    }
                }));
                components.push(serde_json::json!({
                    "componentType": "project.linearMotion",
                    "data": { "velocity": { "x": -0.5, "y": -1.0 } }
                }));
            }
            _ => {}
        }
    }
    let text = serde_json::to_string_pretty(&scene).expect("scene should serialize");
    std::fs::write(scene_path, text).expect("scene should write");
}

fn temp_output_root(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("project-e2e-gate-{name}-{stamp}"))
}

#[test]
fn llm_worker_lifecycle_gate_covers_transport_join_shutdown_and_privacy() {
    let report = crate::run_llm_worker_lifecycle_report();
    assert_eq!(
        report.schema_version,
        crate::LLM_WORKER_LIFECYCLE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.status,
        crate::LlmWorkerLifecycleStatus::Passed,
        "{report:#?}"
    );
    assert_eq!(report.scenarios.len(), 4);
    assert!(report
        .scenarios
        .iter()
        .all(
            |scenario| scenario.task_join_status == editor_core::LlmTaskJoinStatus::Joined
                && scenario.credential_owner_status == editor_core::CredentialOwnerStatus::Released
                && scenario.cancel_latency_ms < 2_000
        ));
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("llm-lifecycle-secret-must-not-leak"));
    assert!(!encoded.contains("llm lifecycle private prompt must not leak"));
}
