use super::*;

#[test]
fn animator2d_authoring_model_exposes_table_controls_without_editable_graph() {
    let model = Animator2DAuthoringModel::default();
    assert!(model.sprite_picker_enabled);
    assert!(model.controller_picker_enabled);
    assert!(!model.relationship_graph_editable);
    assert!(model
        .controls
        .iter()
        .any(|control| control.command_id == "animator2d.preview.step"));
    assert!(!model
        .controls
        .iter()
        .any(|control| control.command_id.contains("graph.edit")));
}

#[test]
fn json_schema_shared_project_patch_value_types_are_derived() {
    for schema in [
        schemars::schema_for!(AssetKind),
        schemars::schema_for!(AssetPlacementMode),
        schemars::schema_for!(InputActionValueKind),
        schemars::schema_for!(Vec3),
    ] {
        let value = serde_json::to_value(schema).expect("shared value schema must serialize");
        assert!(value.is_object());
    }
}

#[test]
fn ai_panel_prompt_model_carries_editable_draft_and_stage() {
    let panel = AiPanelModel {
        prompt_placeholder: "Describe an editor change...".to_string(),
        prompt_draft: "create a fire point".to_string(),
        messages: Vec::new(),
        gateway_access: Default::default(),
        proposed_commands: Vec::new(),
        allowed_command_ids: vec!["generate_project_patch_from_prompt".to_string()],
        busy: true,
        stage: AiPanelStage::Generating,
        status_summary: Some("Generating ProjectPatch".to_string()),
    };
    let value = serde_json::to_value(&panel).unwrap();

    assert_eq!(value["prompt_draft"], "create a fire point");
    assert_eq!(value["stage"], "Generating");
    assert_eq!(value["busy"], true);
}

#[test]
fn gateway_access_inbox_is_a_dedicated_ai_panel_model() {
    let panel = AiPanelModel {
        prompt_placeholder: "Describe an editor change...".to_string(),
        prompt_draft: String::new(),
        messages: Vec::new(),
        gateway_access: GatewayAccessInboxModel {
            requests: vec![GatewayAccessRequestModel {
                request_id: "access-request-1".to_string(),
                operation_short_id: "operation-1".to_string(),
                client_session_id: "gateway-session-1".to_string(),
                session_short_id: "session-1".to_string(),
                client_kind: "MCP".to_string(),
                client_version: "codex-desktop.v1".to_string(),
                project_identity: "project.fixture".to_string(),
                connected_age_ms: 250,
                expires_in_ms: 30_000,
                state: "awaiting_user".to_string(),
                requested_profile: "project_owned_low_risk".to_string(),
                risk_class: "ProjectOwnedLowRisk".to_string(),
                capabilities: vec!["mutate_project".to_string()],
                blocked_capabilities: vec!["engine_core".to_string()],
                goal_id: "goal-1".to_string(),
                user_visible_outcome: "Apply the requested project change.".to_string(),
                completion_policy: "CommitVerified".to_string(),
                allowed_paths: vec!["Assets".to_string()],
                denied_paths: vec!["Engine".to_string()],
                allowed_objects: Vec::new(),
                max_mutation_count: 16,
                time_budget_ms: 900_000,
                external_cost_budget_microunits: 0,
                allow_delete: false,
                allow_dependency_change: false,
                allow_network: false,
                approval_digest: "sha256:test".to_string(),
            }],
            page_index: 0,
            page_count: 2,
            total_count: 5,
        },
        proposed_commands: Vec::new(),
        allowed_command_ids: Vec::new(),
        busy: false,
        stage: AiPanelStage::Idle,
        status_summary: None,
    };

    let value = serde_json::to_value(panel).expect("Gateway access model should serialize");
    assert_eq!(value["gateway_access"]["total_count"], 5);
    assert_eq!(
        value["gateway_access"]["requests"][0]["client_session_id"],
        "gateway-session-1"
    );
    assert_eq!(value["proposed_commands"].as_array().unwrap().len(), 0);
}

#[test]
fn gateway_access_commands_keep_request_and_page_identity() {
    let approve = UiCommandPayload::ApproveGatewayAccessRequest {
        request_id: "access-request-1".to_string(),
    };
    let reject = UiCommandPayload::RejectGatewayAccessRequest {
        request_id: "access-request-2".to_string(),
    };
    let page = UiCommandPayload::SetGatewayAccessPage { page_index: 3 };

    assert_eq!(
        ui_command_id_for_payload(&approve),
        "approve_gateway_access_request"
    );
    assert_eq!(
        ui_command_id_for_payload(&reject),
        "reject_gateway_access_request"
    );
    assert_eq!(ui_command_id_for_payload(&page), "set_gateway_access_page");
}

#[test]
fn panel_layout_model_has_fixed_mvp_regions() {
    let layout = PanelLayoutModel::fixed_mvp();
    assert_eq!(layout.mode, PanelLayoutMode::Fixed);
    assert_eq!(layout.regions.len(), 5);
    assert!(layout
        .regions
        .iter()
        .any(|region| region.region_id == "center"));
}

#[test]
fn ui_model_serializes_to_json_for_debugging() {
    let model = EditorUiModel {
        revision: 1,
        frame: 0,
        mode: EditorUiMode::ProjectLauncher,
        project_launcher: ProjectLauncherModel::empty(),
        project_intent: ProjectIntentWorkspaceModel::empty(),
        project_browser: ProjectBrowserModel::empty(),
        asset_browser: AssetBrowserModel::empty(),
        build_export: BuildExportModel::empty(),
        report_panel: ReportPanelModel::empty(),
        input_mapping_authoring: InputMappingAuthoringModel::empty(),
        rule_authoring: RuleAuthoringModel::empty(),
        animator2d_authoring: Animator2DAuthoringModel::default(),
        project_authoring_workspace: ProjectAuthoringWorkspaceModel::empty(),
        authoring_workflow: AuthoringWorkflowModel::empty(),
        workspace_view_mode: WorkspaceViewMode::SceneView,
        active_runtime_package: None,
        panels: PanelLayoutModel::fixed_mvp(),
        toolbar: ToolbarModel {
            commands: Vec::new(),
            runtime_state: RuntimeRunState::NoPackage,
            game_view_layout: GameViewLayoutState::default(),
        },
        hierarchy: HierarchyModel {
            scene_id: None,
            roots: Vec::new(),
            selected_entity_id: None,
            authoring_view: HierarchyAuthoringView::EntityTree,
            visual_order: None,
            source_domain: HierarchySourceDomain::Empty,
            status: "empty".to_string(),
        },
        inspector: InspectorModel {
            selected_entity_id: None,
            title: "No Selection".to_string(),
            sections: Vec::new(),
            readonly: true,
            persistence: InspectorPersistence::ReadOnly,
        },
        viewport: ViewportModel {
            scene_id: None,
            frame: 0,
            frame_hash: None,
            texture_id: None,
            target_id: None,
            renderable_count: 0,
            selected_entity: None,
            renderables: Vec::new(),
            collider_overlay: ColliderOverlayModel::default(),
        },
        console: ConsoleModel {
            entries: Vec::new(),
            unread_error_count: 0,
            unread_warning_count: 0,
        },
        runtime_trace: RuntimeTraceModel {
            frame: 0,
            entries: Vec::new(),
            selected_entry_id: None,
        },
        ai_panel: AiPanelModel {
            prompt_placeholder: "Describe an editor change...".to_string(),
            prompt_draft: String::new(),
            messages: Vec::new(),
            gateway_access: Default::default(),
            proposed_commands: Vec::new(),
            allowed_command_ids: Vec::new(),
            busy: false,
            stage: AiPanelStage::Idle,
            status_summary: None,
        },
        project_runtime_trust_prompt: None,
        interaction_feedback: None,
        diagnostics: Vec::new(),
    };
    let json = serde_json::to_string(&model).expect("model should serialize");
    assert!(json.contains("NoPackage"));
    assert!(json.contains("No Selection"));
    assert!(json.contains("ProjectLauncher"));
    assert!(json.contains("authoring-workflow.v1"));
}

#[test]
fn report_panel_model_serializes_registered_reports_for_ai_context() {
    let descriptor = ReportDescriptor::new(
        "build.export",
        "Build Export",
        WorkspaceDomainKind::Build,
        "desktop_export",
        ReportSourceKind::InMemory,
    );
    let diagnostic = EvidenceEntry::diagnostic(
        "build.export.missing_runtime_package",
        WorkspaceDomainKind::Build,
        DiagnosticSeverity::Error,
        "MissingRuntimePackage",
        "Runtime package was not produced.",
    );
    let mut report = UnifiedReportEntry::new(
        "report-build-export",
        "build.export",
        "Latest Build Export",
        WorkspaceDomainKind::Build,
        "desktop_export",
    )
    .with_summary("Build failed before player launch.");
    report.status = ReportStatus::Failed;
    report.report_path = Some("Build/Windows/dev/reports/desktop-export-report.json".to_string());
    report.next_actions.push("inspect_build_report".to_string());
    report.diagnostics.push(diagnostic);
    report.artifacts.push(ReportArtifactRef {
        artifact_id: "desktop-export-report".to_string(),
        label: "Desktop Export Report".to_string(),
        path: "Build/Windows/dev/reports/desktop-export-report.json".to_string(),
        kind: "json".to_string(),
    });
    report = report.finalize_counts();

    let registry = ReportRegistrySummary::from_descriptors(vec![descriptor]);
    let panel = ReportPanelModel::from_reports(registry, vec![report], None);
    let json = serde_json::to_string(&panel).expect("report panel should serialize");

    assert_eq!(panel.schema_version, REPORT_PANEL_SCHEMA_VERSION);
    assert_eq!(panel.summary.provider_count, 1);
    assert_eq!(panel.summary.report_count, 1);
    assert_eq!(panel.summary.diagnostic_count, 1);
    assert_eq!(panel.summary.error_count, 1);
    assert_eq!(
        panel.selected_report_id.as_deref(),
        Some("report-build-export")
    );
    assert!(json.contains("Build Export"));
    assert!(json.contains("copy_ai_context"));
    assert!(json.contains("MissingRuntimePackage"));
}

#[test]
fn report_panel_commands_stay_in_report_domain() {
    let commands = [
        UiCommandPayload::SelectReportEntry {
            report_id: "report-build-export".to_string(),
        },
        UiCommandPayload::RefreshReports,
        UiCommandPayload::CopyReportAiContext {
            report_id: "report-build-export".to_string(),
        },
        UiCommandPayload::OpenRawReport {
            report_id: "report-build-export".to_string(),
        },
        UiCommandPayload::RevealReportPath {
            report_id: "report-build-export".to_string(),
        },
        UiCommandPayload::OpenRelatedReportArtifact {
            report_id: "report-build-export".to_string(),
            artifact_id: "desktop-export-report".to_string(),
        },
    ];

    for command in commands {
        assert_eq!(
            workspace_domain_for_payload(&command),
            WorkspaceDomainKind::Report
        );
        assert!(ui_command_id_for_payload(&command).contains("report"));
        assert!(workspace_payload_kind(&command).contains("Report"));
    }
}

#[test]
fn authoring_workflow_empty_model_exposes_project_entry_only() {
    let workflow = AuthoringWorkflowModel::empty();

    assert_eq!(workflow.schema_version, AUTHORING_WORKFLOW_SCHEMA_VERSION);
    assert_eq!(workflow.active_step, AuthoringStepId::Project);
    assert_eq!(workflow.steps.len(), AuthoringStepId::all().len());
    assert!(!workflow.can_play);
    assert!(!workflow.can_build);
    assert_eq!(
        workflow.step(AuthoringStepId::Project).unwrap().status,
        AuthoringStepStatus::Ready
    );
    assert_eq!(
        workflow.step(AuthoringStepId::Scene).unwrap().status,
        AuthoringStepStatus::NotAvailable
    );
    assert_eq!(AuthoringStepId::Build.domain(), WorkspaceDomainKind::Build);
    assert!(workflow
        .ai_context
        .missing_required_items
        .contains(&"project".to_string()));
}

#[test]
fn authoring_workflow_model_serializes_for_ai_context() {
    let workflow = AuthoringWorkflowModel::empty();
    let json = serde_json::to_string(&workflow).expect("workflow should serialize");
    let decoded: AuthoringWorkflowModel =
        serde_json::from_str(&json).expect("workflow should deserialize");

    assert_eq!(decoded, workflow);
    assert!(json.contains("Open Project"));
    assert!(json.contains("open_or_create_project"));
    assert_eq!(
        "build".parse::<AuthoringStepId>(),
        Ok(AuthoringStepId::Build)
    );
    assert!("missing".parse::<AuthoringStepId>().is_err());
}

#[test]
fn project_authoring_workspace_model_serializes_domain_summaries() {
    let mut model = ProjectAuthoringWorkspaceModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        project_id: Some("project-plane-game".to_string()),
        active_scene_id: Some("scene-main".to_string()),
        active_document: Some(WorkspaceDocumentSummary {
            document_kind: "scene".to_string(),
            document_id: Some("scene-main".to_string()),
            path: Some("Scenes/Main.scene.json".to_string()),
            dirty: true,
        }),
        selection: WorkspaceSelectionSummary {
            primary: Some(WorkspaceSelectionTarget::Entity {
                entity_id: "entity-player".to_string(),
            }),
            secondary: Vec::new(),
        },
        domains: vec![
            WorkspaceDomainSummary::new(
                WorkspaceDomainKind::Project,
                "Project",
                WorkspaceDomainStatus::Ready,
                "project open",
            ),
            WorkspaceDomainSummary::new(
                WorkspaceDomainKind::Scene,
                "Scene",
                WorkspaceDomainStatus::Dirty,
                "scene-main entity_count=1",
            ),
        ],
        dirty_domains: vec![WorkspaceDomainKind::Scene],
        diagnostics: WorkspaceDiagnosticsSummary {
            info_count: 1,
            warning_count: 0,
            error_count: 0,
            last_code: Some("workspace.ready".to_string()),
        },
        empty_message: String::new(),
        report: WorkspaceReportSummary {
            project_status: "ready".to_string(),
            dirty_domains: vec![WorkspaceDomainKind::Scene],
            diagnostics: WorkspaceDiagnosticsSummary::default(),
            report_count: 0,
            evidence_count: 0,
            next_action_count: 0,
            last_command: None,
            last_transaction: None,
            build_status: Some("not_exported".to_string()),
            play_status: Some("stopped".to_string()),
        },
    };
    model.domains[1].item_count = 1;
    model.domains[1].dirty = true;
    model.domains[1].selected_id = Some("entity-player".to_string());

    let json = serde_json::to_string(&model).expect("workspace model should serialize");

    assert!(json.contains("project-plane-game"));
    assert!(json.contains("Scene"));
    assert!(json.contains("entity-player"));
    assert_eq!(WorkspaceDomainKind::all().len(), 10);
    assert_eq!(WorkspaceDomainKind::Build.as_str(), "build");
}

#[test]
fn workspace_command_summary_maps_ui_payload_to_domain() {
    let command = UiCommand {
        command_id: "build_and_run_desktop_package".to_string(),
        source: UiCommandSource::Toolbar,
        request_id: "request-build-1".to_string(),
        payload: UiCommandPayload::BuildAndRunDesktopPackage {
            profile_id: Some("windows-dev".to_string()),
        },
    };

    let summary = WorkspaceCommandSummary::from_ui_command(&command);
    let json = serde_json::to_string(&summary).expect("workspace command should serialize");

    assert_eq!(summary.target_domain, WorkspaceDomainKind::Build);
    assert_eq!(summary.payload_kind, "BuildAndRunDesktopPackage");
    assert!(json.contains("request-build-1"));
}

#[test]
fn build_export_model_serializes_commands_and_report_summary() {
    let model = BuildExportModel {
        selected_profile_id: Some("windows-dev".to_string()),
        profiles: vec![BuildProfileSummary {
            profile_id: "windows-dev".to_string(),
            label: "Windows Dev".to_string(),
            target: "windows".to_string(),
            output_dir: "Build/Windows/dev".to_string(),
            active: true,
        }],
        release_profile: None,
        commands: vec![
            BuildExportCommand::new("export_desktop_package", "Export", true, None),
            BuildExportCommand::new("build_and_run_desktop_package", "Build & Run", true, None),
            BuildExportCommand::new("build_release_package", "Build Release", true, None),
        ],
        last_report: Some(BuildExportReportSummary {
            status: "success".to_string(),
            profile: "dev".to_string(),
            target: "windows".to_string(),
            package_dir: "Build/Windows/dev".to_string(),
            report_path: "Build/Windows/dev/reports/desktop-export-report.json".to_string(),
            runtime_package_dir: "Build/Windows/dev/data/runtime_package".to_string(),
            player_exit_code: Some(0),
            player_exit_reason: "completed".to_string(),
            diagnostic_count: 0,
        }),
        last_release_report: None,
        empty_message: String::new(),
    };

    let json = serde_json::to_string(&model).expect("build export model should serialize");

    assert!(json.contains("windows-dev"));
    assert!(json.contains("export_desktop_package"));
    assert!(json.contains("build_and_run_desktop_package"));
    assert!(json.contains("build_release_package"));
    assert!(json.contains("desktop-export-report.json"));
}

#[test]
fn build_export_commands_are_ui_payloads() {
    let export = UiCommandPayload::ExportDesktopPackage {
        profile_id: Some("windows-dev".to_string()),
    };
    let build_and_run = UiCommandPayload::BuildAndRunDesktopPackage {
        profile_id: Some("windows-dev".to_string()),
    };
    let output = UiCommandPayload::OpenBuildOutput;
    let report = UiCommandPayload::OpenBuildReport;

    assert!(serde_json::to_string(&export)
        .expect("payload should serialize")
        .contains("ExportDesktopPackage"));
    assert!(serde_json::to_string(&build_and_run)
        .expect("payload should serialize")
        .contains("BuildAndRunDesktopPackage"));
    assert_eq!(
        ui_command_id_for_payload(&build_and_run),
        "build_and_run_desktop_package"
    );
    assert!(serde_json::to_string(&output)
        .expect("payload should serialize")
        .contains("OpenBuildOutput"));
    assert!(serde_json::to_string(&report)
        .expect("payload should serialize")
        .contains("OpenBuildReport"));
}

#[test]
fn input_mapping_authoring_model_serializes_report_and_commands() {
    let model = InputMappingAuthoringModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        selected_path: Some("Input/input.default.json".to_string()),
        mapping_id: Some("input.default".to_string()),
        selected_context_id: Some("gameplay".to_string()),
        selected_action_id: Some("action.test".to_string()),
        selected_binding_id: Some("binding.test".to_string()),
        source_hash: Some("fnv1a64:test".to_string()),
        dirty: true,
        capture_binding_id: None,
        capture_accepts_pointer_position: false,
        preview: None,
        report_level: InputMappingReportLevel::Summary,
        actions: vec![InputMappingActionSummary {
            action_id: "action.test".to_string(),
            value_type: InputActionValueKind::Button,
            binding_count: 1,
        }],
        contexts: vec![InputMappingContextSummary {
            context_id: "gameplay".to_string(),
            priority: 0,
            consume_input: false,
            enabled_by_default: true,
        }],
        bindings: vec![InputMappingBindingSummary {
            binding_id: "binding.test".to_string(),
            binding_index: 0,
            context_id: "gameplay".to_string(),
            action_id: "action.test".to_string(),
            device_path: "keyboard/T".to_string(),
            processor: "None".to_string(),
            trigger: "Pressed".to_string(),
        }],
        control_catalog: InputControlCatalogModel::default(),
        report: InputMappingAuthoringReport {
            mapping_count: 1,
            action_count: 1,
            context_count: 1,
            binding_count: 1,
            validation_status: InputMappingValidationStatus::Ok,
            ..InputMappingAuthoringReport::default()
        },
        commands: vec![InputMappingAuthoringCommand::new(
            "validate_input_mapping",
            "Validate",
            true,
            None,
        )],
        empty_message: String::new(),
    };

    let json = serde_json::to_string(&model).expect("input mapping model should serialize");

    assert!(json.contains("input.default"));
    assert!(json.contains("keyboard/T"));
    assert!(json.contains("input-mapping-authoring-report.v1"));
}

#[test]
fn input_mapping_authoring_commands_route_to_input_domain() {
    let payload = UiCommandPayload::AddInputBinding {
        path: "Input/input.default.json".to_string(),
        context_id: "gameplay".to_string(),
        action_id: "action.test".to_string(),
        device_path: "keyboard/T".to_string(),
    };
    let summary = WorkspaceCommandSummary::from_ui_command(&UiCommand {
        command_id: "add_input_binding".to_string(),
        source: UiCommandSource::Inspector,
        request_id: "request-input-1".to_string(),
        payload: payload.clone(),
    });
    let json = serde_json::to_string(&payload).expect("payload should serialize");

    assert_eq!(summary.target_domain, WorkspaceDomainKind::Input);
    assert_eq!(summary.payload_kind, "AddInputBinding");
    assert!(json.contains("keyboard/T"));
}

#[test]
fn rule_authoring_model_serializes_report_and_commands() {
    let model = RuleAuthoringModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        selected_path: Some("Rules/fire.rule.json".to_string()),
        rule_count: 1,
        document: RuleAuthoringDocument {
            asset_path: Some("Rules/fire.rule.json".to_string()),
            asset_id: Some("asset.rule.fire".to_string()),
            rule_id: Some("project.rule.fire".to_string()),
            display_name: Some("Fire".to_string()),
            dirty: false,
            selected_statement_path: None,
            selected_operation_path: None,
            human_summary: "Rule Fire: runs when input action fire is pressed.".to_string(),
            report: RuleAuthoringReport {
                schema_version: RULE_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
                status: RuleAuthoringStatus::Valid,
                asset_id: Some("asset.rule.fire".to_string()),
                rule_id: Some("project.rule.fire".to_string()),
                ir_hash: Some("hash".to_string()),
                human_summary: "Rule Fire: runs when input action fire is pressed.".to_string(),
                diagnostics: Vec::new(),
                changed_paths: Vec::new(),
                next_actions: vec!["build_rule_artifact".to_string()],
                generated_rust_source: RuleAuthoringStageEvidence::skipped(
                    "not_requested",
                    "Run build_rule_artifact.",
                ),
                static_registry_source: RuleAuthoringStageEvidence::skipped(
                    "not_requested",
                    "Run build_rule_artifact.",
                ),
                artifact_lifecycle: RuleAuthoringStageEvidence::skipped(
                    "not_requested",
                    "Run build_rule_artifact.",
                ),
                runtime_package_manifest: RuleAuthoringStageEvidence::skipped(
                    "not_requested",
                    "Run export.",
                ),
                cargo_build: RuleAuthoringStageEvidence::skipped(
                    "skipped_by_v1",
                    "Run project export.",
                ),
            },
        },
        card_authoring: RuleCardAuthoringModel::empty(),
        commands: vec![RuleAuthoringCommand::new(
            "validate_rule_asset",
            "Validate",
            true,
            None,
        )],
        empty_message: String::new(),
    };

    let json = serde_json::to_string(&model).expect("rule authoring model should serialize");

    assert!(json.contains("rule-authoring-report.v1"));
    assert!(json.contains("project.rule.fire"));
    assert!(json.contains("validate_rule_asset"));
}

#[test]
fn rule_card_authoring_model_serializes_read_only_graph_preview() {
    let model = RuleCardAuthoringModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        selected_path: Some("Rules/fire.rule.json".to_string()),
        rule_count: 1,
        document: RuleAuthoringDocument::empty(),
        selected_card_id: Some("card:trigger".to_string()),
        cards: vec![RuleCardModel {
            card_id: "card:trigger".to_string(),
            kind: RuleCardKind::Trigger,
            asset_path: Some("Rules/fire.rule.json".to_string()),
            rule_id: Some("project.rule.fire".to_string()),
            source_path: "canonicalIr.trigger".to_string(),
            title: "Trigger".to_string(),
            summary: "always".to_string(),
            human_explanation: "This rule starts when always.".to_string(),
            fields: vec![RuleCardFieldModel {
                field_id: "trigger.kind".to_string(),
                label: "Trigger Kind".to_string(),
                field_path: "canonicalIr.trigger.kind".to_string(),
                value_kind: RuleCardFieldValueKind::Enum,
                value_preview: "always".to_string(),
                editable: true,
                enum_options: vec!["always".to_string()],
                asset_ref_options: Vec::new(),
                validation_state: RuleCardValidationState::Valid,
            }],
            allowed_commands: vec![RuleAuthoringCommand::new(
                "set_rule_card_field",
                "Edit Card Field",
                true,
                None,
            )],
            diagnostics: Vec::new(),
        }],
        graph_preview: RuleGraphPreviewModel {
            schema_version: RULE_GRAPH_PREVIEW_SCHEMA_VERSION.to_string(),
            asset_path: Some("Rules/fire.rule.json".to_string()),
            rule_id: Some("project.rule.fire".to_string()),
            ir_hash: Some("hash".to_string()),
            nodes: vec![RuleGraphPreviewNode {
                node_id: "node:trigger".to_string(),
                card_id: Some("card:trigger".to_string()),
                source_path: "canonicalIr.trigger".to_string(),
                kind: RuleGraphPreviewNodeKind::Trigger,
                label: "always".to_string(),
                status: RuleGraphPreviewNodeStatus::Selected,
                diagnostic_refs: Vec::new(),
            }],
            edges: Vec::new(),
            groups: Vec::new(),
            selected_node_id: Some("node:trigger".to_string()),
            source_mappings: vec![RuleCardSourceMapping {
                source_path: "canonicalIr.trigger".to_string(),
                card_id: Some("card:trigger".to_string()),
                node_id: Some("node:trigger".to_string()),
            }],
            read_only: true,
        },
        commands: vec![RuleAuthoringCommand::new(
            "refresh_rule_graph_preview",
            "Refresh Graph Preview",
            true,
            None,
        )],
        report_summary: RuleCardAuthoringReport {
            schema_version: RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            status: RuleAuthoringStatus::Valid,
            asset_path: Some("Rules/fire.rule.json".to_string()),
            rule_id: Some("project.rule.fire".to_string()),
            ir_hash: Some("hash".to_string()),
            card_count: 1,
            graph_node_count: 1,
            graph_edge_count: 0,
            editable_card_count: 1,
            read_only_graph: true,
            changed_paths: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: vec!["refresh_rule_graph_preview".to_string()],
            source_mappings: Vec::new(),
        },
    };

    let json = serde_json::to_string(&model).expect("rule card model should serialize");

    assert!(json.contains("rule-card-authoring-report.v1"));
    assert!(json.contains("rule-graph-preview.v1"));
    assert!(json.contains("readOnly"));
    assert!(json.contains("card:trigger"));
}

#[test]
fn rule_graph_preview_model_serializes_source_mapping_as_read_only() {
    let preview = RuleGraphPreviewModel {
        schema_version: RULE_GRAPH_PREVIEW_SCHEMA_VERSION.to_string(),
        asset_path: Some("Rules/fire.rule.json".to_string()),
        rule_id: Some("project.rule.fire".to_string()),
        ir_hash: Some("hash".to_string()),
        nodes: vec![RuleGraphPreviewNode {
            node_id: "node:trigger".to_string(),
            card_id: Some("card:trigger".to_string()),
            source_path: "canonicalIr.trigger".to_string(),
            kind: RuleGraphPreviewNodeKind::Trigger,
            label: "always".to_string(),
            status: RuleGraphPreviewNodeStatus::Normal,
            diagnostic_refs: Vec::new(),
        }],
        edges: Vec::new(),
        groups: Vec::new(),
        selected_node_id: None,
        source_mappings: vec![RuleCardSourceMapping {
            source_path: "canonicalIr.trigger".to_string(),
            card_id: Some("card:trigger".to_string()),
            node_id: Some("node:trigger".to_string()),
        }],
        read_only: true,
    };

    let json = serde_json::to_string(&preview).expect("graph preview should serialize");

    assert!(json.contains("rule-graph-preview.v1"));
    assert!(json.contains("canonicalIr.trigger"));
    assert!(json.contains("readOnly"));
}

#[test]
fn rule_authoring_commands_route_to_rule_domain() {
    let payload = UiCommandPayload::SetRuleCardField {
        path: "Rules/fire.rule.json".to_string(),
        card_id: "card:trigger".to_string(),
        field_path: "canonicalIr.trigger.actionId".to_string(),
        value: serde_json::json!("action.fire"),
        expected_ir_hash: None,
    };
    let summary = WorkspaceCommandSummary::from_ui_command(&UiCommand {
        command_id: "set_rule_card_field".to_string(),
        source: UiCommandSource::Inspector,
        request_id: "request-rule-1".to_string(),
        payload: payload.clone(),
    });
    let json = serde_json::to_string(&payload).expect("payload should serialize");

    assert_eq!(summary.target_domain, WorkspaceDomainKind::Rule);
    assert_eq!(summary.payload_kind, "SetRuleCardField");
    assert!(json.contains("card:trigger"));
}

#[test]
fn collider_overlay_model_serializes_summary_items_and_diagnostics() {
    let model = ColliderOverlayModel {
        collider_count: 1,
        draw_item_count: 1,
        selected_entity_id: Some("entity-a".to_string()),
        invalid_collider_count: 0,
        missing_transform_count: 1,
        draw_items: vec![ColliderOverlayItem {
            entity_id: "entity-a".to_string(),
            shape: ColliderOverlayShape::Aabb {
                half_extents: Vec3 {
                    x: 0.5,
                    y: 0.5,
                    z: 0.0,
                },
            },
            center: Vec3 {
                x: 1.0,
                y: 2.0,
                z: 0.0,
            },
            enabled: true,
            sensor: false,
            selected: true,
            layer: 1,
            mask: u32::MAX,
        }],
        diagnostics: vec![ColliderOverlayDiagnostic {
            severity: "warning".to_string(),
            entity_id: Some("entity-b".to_string()),
            component_type: "engine.collider2d".to_string(),
            field_path: "transform".to_string(),
            message: "Collider2D entity is missing Transform".to_string(),
            suggestion: "Add Transform before drawing collider overlay.".to_string(),
        }],
    };

    let json = serde_json::to_string(&model).expect("collider overlay should serialize");

    assert!(json.contains("entity-a"));
    assert!(json.contains("missingTransformCount"));
    assert!(json.contains("shapeKind"));
}

#[test]
fn project_launcher_model_serializes_recent_projects() {
    let mut launcher = ProjectLauncherModel::empty();
    launcher.recent_projects.push(RecentProjectEntry {
        name: "PlaneGame".to_string(),
        path: "D:/Projects/PlaneGame".to_string(),
        engine_version: "0.0.2".to_string(),
        last_opened_at: Some("2026-06-30T00:00:00Z".to_string()),
        last_modified_at: None,
        valid: true,
        status: "ready".to_string(),
    });

    let json = serde_json::to_string_pretty(&launcher).expect("launcher should serialize");

    assert!(json.contains("PlaneGame"));
    assert!(json.contains("open_project"));
    assert!(json.contains("create_project"));
}

#[test]
fn project_browser_model_serializes_entries_and_commands() {
    let model = ProjectBrowserModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        selected_path: Some("Scenes/Main.scene.json".to_string()),
        entries: vec![ProjectBrowserEntry::new(
            "Scenes/Main.scene.json",
            "Main.scene.json",
            ProjectBrowserEntryKind::Scene,
            true,
            true,
            true,
        )],
        empty_message: "No files.".to_string(),
    };
    let select = UiCommandPayload::SelectProjectBrowserEntry {
        path: "Scenes/Main.scene.json".to_string(),
    };
    let open = UiCommandPayload::OpenProjectBrowserEntry {
        path: "Scenes/Main.scene.json".to_string(),
    };

    let json = serde_json::to_string_pretty(&model).expect("model should serialize");
    let select_json = serde_json::to_string(&select).expect("select should serialize");
    let open_json = serde_json::to_string(&open).expect("open should serialize");

    assert!(json.contains("Main.scene.json"));
    assert!(json.contains("Scene"));
    assert!(select_json.contains("SelectProjectBrowserEntry"));
    assert!(open_json.contains("OpenProjectBrowserEntry"));
}

#[test]
fn asset_browser_model_serializes_entries_query_selection_and_report() {
    let mut entry =
        AssetBrowserEntry::new("Assets/Textures/icon.png", "icon.png", AssetKind::Texture);
    entry.asset_id = Some("asset-icon".to_string());
    entry.selected = true;
    entry.preview.thumbnail_asset_id = Some("asset-icon".to_string());
    let model = AssetBrowserModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        index_status: AssetBrowserIndexStatus::Ready,
        index_progress: AssetBrowserIndexProgress::default(),
        scan_generation: 1,
        view_mode: AssetBrowserViewMode::List,
        current_folder: None,
        scroll_offset: 0.0,
        thumbnail_size: 96,
        query: AssetQuery {
            search_text: "icon".to_string(),
            kinds: vec![AssetKind::Texture],
            ..AssetQuery::default()
        },
        selection: AssetSelection::single(
            "Assets/Textures/icon.png",
            Some("asset-icon".to_string()),
        ),
        folder_entries: Vec::new(),
        entries: vec![entry],
        picker: None,
        report: AssetBrowserReport {
            asset_count: 1,
            selected_count: 1,
            ..AssetBrowserReport::default()
        },
        empty_message: "No assets.".to_string(),
    };

    let json = serde_json::to_string(&model).expect("asset browser model should serialize");

    assert!(json.contains("icon.png"));
    assert!(json.contains("asset-browser-report.v1"));
    assert!(json.contains("asset-icon"));
}

#[test]
fn asset_browser_query_filters_by_text_kind_and_missing_flags() {
    let entry = AssetBrowserEntry::new("Assets/Textures/icon.png", "icon.png", AssetKind::Texture);
    let mut missing = AssetBrowserEntry::new("Assets/Missing.asset", "Missing", AssetKind::Unknown);
    missing.exists = false;
    let query = AssetQuery {
        search_text: "icon".to_string(),
        kinds: vec![AssetKind::Texture],
        include_missing: false,
        include_unimported: false,
        folder: Some("Assets".to_string()),
    };

    assert!(query.matches(&entry));
    assert!(!query.matches(&missing));
}

#[test]
fn asset_browser_model_projects_to_legacy_project_browser_model() {
    let model = AssetBrowserModel {
        project_root: Some("D:/Projects/PlaneGame".to_string()),
        index_status: AssetBrowserIndexStatus::Ready,
        index_progress: AssetBrowserIndexProgress::default(),
        scan_generation: 1,
        view_mode: AssetBrowserViewMode::List,
        current_folder: None,
        scroll_offset: 0.0,
        thumbnail_size: 96,
        query: AssetQuery::default(),
        selection: AssetSelection::single("Scenes/Main.scene.json", Some("scene-main".to_string())),
        folder_entries: Vec::new(),
        entries: vec![AssetBrowserEntry::new(
            "Scenes/Main.scene.json",
            "Main.scene.json",
            AssetKind::Scene,
        )],
        picker: None,
        report: AssetBrowserReport::default(),
        empty_message: "No assets.".to_string(),
    };

    let legacy = model.to_project_browser_model();

    assert_eq!(
        legacy.selected_path.as_deref(),
        Some("Scenes/Main.scene.json")
    );
    assert_eq!(legacy.entries[0].kind, ProjectBrowserEntryKind::Scene);
}

#[test]
fn asset_browser_editor_asset_ref_round_trips_structured_identity() {
    let asset_ref = EditorAssetRef {
        asset_id: "texture-player".to_string(),
        asset_type_id: "texture".to_string(),
        guid: Some("guid-texture-player".to_string()),
        sub_asset_id: Some("sprite-main".to_string()),
    };

    let json = serde_json::to_string(&asset_ref).expect("asset ref should serialize");
    let restored: EditorAssetRef =
        serde_json::from_str(&json).expect("asset ref should deserialize");

    assert_eq!(restored, asset_ref);
    assert!(json.contains("\"id\":\"texture-player\""));
    assert!(json.contains("\"type\":\"texture\""));
    assert!(json.contains("\"subAsset\":\"sprite-main\""));
}

#[test]
fn asset_browser_editor_asset_ref_accepts_legacy_string_but_serializes_structured() {
    let restored: EditorAssetRef =
        serde_json::from_str("\"legacy-texture\"").expect("legacy string should deserialize");
    let json = serde_json::to_string(&restored).expect("asset ref should serialize");

    assert_eq!(restored.asset_id, "legacy-texture");
    assert_eq!(restored.asset_type_id, "asset");
    assert_eq!(json, r#"{"id":"legacy-texture","type":"asset"}"#);
}

#[test]
fn asset_browser_selection_prefers_stable_entry_key() {
    let entry = AssetBrowserEntry::authoring(
        "Assets/player.asset",
        "player.asset",
        AssetKind::Texture,
        EditorAssetRef::new("texture-player", "texture"),
    );
    let selection = AssetSelection::single_entry(&entry);

    assert!(selection.contains_entry(&entry));
    assert_eq!(selection.primary_entry_key, Some(entry.entry_key));
}

#[test]
fn inspector_field_can_be_readonly_vec3() {
    let field = InspectorField {
        field_id: "transform.localPosition".to_string(),
        label: "localPosition".to_string(),
        value: InspectorValue::Vec3(Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
        value_type: InspectorValueType::Vec3,
        path: "transform.localPosition".to_string(),
        readonly: true,
        editable: false,
    };
    assert!(field.readonly);
    assert_eq!(field.value_type, InspectorValueType::Vec3);
}

#[test]
fn ui_command_payload_serializes_open_scene_document() {
    let payload = UiCommandPayload::OpenSceneDocument {
        path: "Assets/Scenes/main.scene.json".to_string(),
    };

    let json = serde_json::to_string(&payload).expect("payload should serialize");

    assert!(json.contains("OpenSceneDocument"));
    assert!(json.contains("main.scene.json"));
}

#[test]
fn ui_command_payload_serializes_project_launcher_commands() {
    let payload = UiCommandPayload::CreateProject {
        path: "D:/Projects/PlaneGame".to_string(),
        name: "PlaneGame".to_string(),
    };

    let json = serde_json::to_string(&payload).expect("payload should serialize");

    assert!(json.contains("CreateProject"));
    assert!(json.contains("PlaneGame"));
}

#[test]
fn ui_command_payload_serializes_set_scene_transform() {
    let payload = UiCommandPayload::SetSceneTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
        local_rotation: None,
        local_scale: None,
    };

    let json = serde_json::to_string(&payload).expect("payload should serialize");

    assert!(json.contains("SetSceneTransform"));
    assert!(json.contains("local_position"));
    assert!(json.contains("entity-player"));
}

#[test]
fn ui_command_payload_serializes_set_scene_component_field() {
    let payload = UiCommandPayload::SetSceneComponentField {
        entity_id: "entity-player".to_string(),
        component_type: "game.health".to_string(),
        field_path: "hp".to_string(),
        value: serde_json::json!(100),
    };

    let json = serde_json::to_string(&payload).expect("payload should serialize");

    assert!(json.contains("SetSceneComponentField"));
    assert!(json.contains("game.health"));
    assert!(json.contains("hp"));
}

#[test]
fn ui_command_payload_serializes_place_asset_into_scene() {
    let payload = UiCommandPayload::PlaceAssetIntoScene {
        asset_id: "model-player".to_string(),
        asset_type: "model".to_string(),
        asset_guid: Some("guid-player".to_string()),
        target_parent_id: Some("entity-root".to_string()),
        local_position: Some(Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
        placement_mode: AssetPlacementMode::UnderSelectedOrRoot,
    };

    let json = serde_json::to_string_pretty(&payload).expect("payload should serialize");

    assert!(json.contains("PlaceAssetIntoScene"));
    assert!(json.contains("model-player"));
    assert!(json.contains("model"));
    assert!(json.contains("underSelectedOrRoot"));
    assert!(json.contains("target_parent_id"));
    assert!(json.contains("local_position"));
}

#[test]
fn ui_model_contains_ai_panel_for_ai_first_editor() {
    let panel = AiPanelModel {
        prompt_placeholder: "Describe an editor change...".to_string(),
        prompt_draft: String::new(),
        messages: vec![AiPanelMessage {
            message_id: "ai-message-1".to_string(),
            role: AiPanelMessageRole::Assistant,
            text: "Ready.".to_string(),
        }],
        gateway_access: Default::default(),
        proposed_commands: vec![AiProposedCommand {
            proposal_id: "proposal-1".to_string(),
            label: "Rename entity".to_string(),
            explanation: "Renames the selected entity.".to_string(),
            command: UiCommandPayload::RenameSceneEntity {
                entity_id: "entity-player".to_string(),
                name: "Hero".to_string(),
            },
            project_patch: None,
            imported_project_patch: None,
            review_state: AiCommandReviewState::Proposed,
        }],
        allowed_command_ids: vec!["rename_scene_entity".to_string()],
        busy: false,
        stage: AiPanelStage::Idle,
        status_summary: None,
    };

    let json = serde_json::to_string_pretty(&panel).expect("ai panel should serialize");

    assert!(json.contains("RenameSceneEntity"));
    assert!(json.contains("rename_scene_entity"));
}

#[test]
fn ai_panel_imported_project_patch_evidence_serializes() {
    let panel = AiPanelModel {
        prompt_placeholder: "Describe an editor change...".to_string(),
        prompt_draft: String::new(),
        messages: Vec::new(),
        gateway_access: Default::default(),
        proposed_commands: vec![AiProposedCommand {
            proposal_id: "imported-project-patch-patch-1".to_string(),
            label: "Apply imported patch".to_string(),
            explanation: "Review imported patch before apply.".to_string(),
            command: UiCommandPayload::ApplyImportedProjectPatch {
                proposal_id: "imported-project-patch-patch-1".to_string(),
            },
            project_patch: Some(ProjectPatchEvidence {
                patch_id: "patch-1".to_string(),
                patch_title: "Patch 1".to_string(),
                touched_domains: vec!["Scene".to_string()],
                operation_count: 1,
                validation_status: true,
                risk_level: "Low".to_string(),
                repaired_once: false,
                diagnostics: Vec::new(),
                requires_confirmation: true,
            }),
            imported_project_patch: Some(ImportedProjectPatchEvidence {
                source_kind: "JsonString".to_string(),
                source_label: "test-json".to_string(),
                patch_id: Some("patch-1".to_string()),
                parse_status: "Parsed".to_string(),
                validation_status: Some(true),
                review_state: "Proposed".to_string(),
            }),
            review_state: AiCommandReviewState::Proposed,
        }],
        allowed_command_ids: vec!["apply_imported_project_patch".to_string()],
        busy: false,
        stage: AiPanelStage::Idle,
        status_summary: None,
    };

    let json = serde_json::to_string_pretty(&panel).expect("ai panel should serialize");

    assert!(json.contains("imported_project_patch"));
    assert!(json.contains("ApplyImportedProjectPatch"));
    assert!(json.contains("JsonString"));
}

#[test]
fn ui_command_payload_scene_edit_commands_are_ai_readable() {
    let command = UiCommand {
        command_id: "set_scene_component_field".to_string(),
        source: UiCommandSource::Inspector,
        request_id: "request-scene-edit".to_string(),
        payload: UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "hp".to_string(),
            value: serde_json::json!(80),
        },
    };

    let json = serde_json::to_string_pretty(&command).expect("command should serialize");

    assert!(json.contains("set_scene_component_field"));
    assert!(json.contains("Inspector"));
    assert!(json.contains("game.health"));
}

#[test]
fn ui_command_payload_prefab_authoring_commands_are_ai_readable() {
    let command = UiCommand {
        command_id: "enter_prefab_stage".to_string(),
        source: UiCommandSource::Test,
        request_id: "request-prefab-stage".to_string(),
        payload: UiCommandPayload::EnterPrefabStage {
            path: "Prefabs/ship.prefab.json".to_string(),
            mode: PrefabStageMode::Isolated,
            opened_from_instance_entity_id: Some("entity-prefab-instance".to_string()),
        },
    };

    let json = serde_json::to_string_pretty(&command).expect("command should serialize");

    assert!(json.contains("enter_prefab_stage"));
    assert!(json.contains("EnterPrefabStage"));
    assert!(json.contains("Prefabs/ship.prefab.json"));
    assert_eq!(
        ui_command_id_for_payload(&command.payload),
        "enter_prefab_stage"
    );
    assert_eq!(
        workspace_domain_for_payload(&command.payload),
        WorkspaceDomainKind::Prefab
    );
}

#[test]
fn ui_command_payload_aui_authoring_commands_are_ai_readable() {
    let command = UiCommand {
        command_id: "set_aui_binding_path".to_string(),
        source: UiCommandSource::AiAssistant,
        request_id: "request-aui-binding".to_string(),
        payload: UiCommandPayload::SetAuiBindingPath {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score_text".to_string(),
            target_field: "text.text".to_string(),
            binding_id: "bind.score".to_string(),
            binding_path: "game.score_text".to_string(),
            fallback: Some(serde_json::json!("Score: 0")),
        },
    };

    let json = serde_json::to_string_pretty(&command).expect("command should serialize");

    assert!(json.contains("set_aui_binding_path"));
    assert!(json.contains("SetAuiBindingPath"));
    assert!(json.contains("game.score_text"));
    assert_eq!(
        ui_command_id_for_payload(&command.payload),
        "set_aui_binding_path"
    );
    assert_eq!(
        workspace_domain_for_payload(&command.payload),
        WorkspaceDomainKind::Aui
    );
    assert_eq!(
        workspace_payload_kind(&command.payload),
        "SetAuiBindingPath"
    );
}

#[test]
fn ui_command_payload_select_aui_node_is_ai_readable() {
    let payload = UiCommandPayload::SelectAuiNode {
        document_path: "AUI/hud.aui.json".to_string(),
        document_id: "hud".to_string(),
        node_id: "score_text".to_string(),
    };
    let json = serde_json::to_string_pretty(&payload).expect("payload should serialize");

    assert!(json.contains("SelectAuiNode"));
    assert!(json.contains("score_text"));
    assert_eq!(ui_command_id_for_payload(&payload), "select_aui_node");
    assert_eq!(
        workspace_domain_for_payload(&payload),
        WorkspaceDomainKind::Aui
    );
    assert_eq!(workspace_payload_kind(&payload), "SelectAuiNode");
}

#[test]
fn aui_scene_authoring_models_are_ai_readable_and_truthful() {
    let proxy = AuiNodeAuthoringProxy {
        document_path: "AUI/hud.aui.json".to_string(),
        document_id: "hud".to_string(),
        node_id: "score_text".to_string(),
        parent_node_id: Some("root".to_string()),
        name: "Score Text".to_string(),
        kind: "Text".to_string(),
        source_rect: AuiSourceRect::fixed_position(16.0, 16.0, 220.0, 40.0),
        rect: AuiComputedAuthoringRect {
            x: 16.0,
            y: 16.0,
            width: 220.0,
            height: 40.0,
        },
        visible: true,
        interactable: false,
        binding_count: 1,
        action_count: 0,
        selectable: true,
        diagnostics: Vec::new(),
    };
    let entry = SceneVisualOrderAuthoringEntry {
        entry_id: "aui-node:AUI/hud.aui.json:score_text".to_string(),
        display_name: proxy.name.clone(),
        target_kind: SceneVisualOrderTargetKind::AuiNode,
        target_ref: proxy.node_id.clone(),
        parent_entry_id: Some("aui-canvas:AUI/hud.aui.json:hud".to_string()),
        visual_order_key: VisualOrderKey::screen_overlay(0, 10, 1),
        visual_order_intent: VisualOrderIntent::none(),
        runtime_supported: true,
        runtime_support_reason: "same_canvas_sibling_order".to_string(),
        can_reorder: true,
        reorder_scope: "aui_sibling_order".to_string(),
        diagnostics: Vec::new(),
    };
    let model = SceneVisualOrderAuthoringModel {
        schema_version: SCENE_VISUAL_ORDER_AUTHORING_MODEL_SCHEMA_VERSION.to_string(),
        scene_path: Some("Scenes/Main.scene.json".to_string()),
        entries: vec![entry],
        default_view_is_visual_order: true,
        debug_bucket_view_available: true,
        diagnostics: Vec::new(),
    };
    let selection = WorkspaceSelectionSummary {
        primary: Some(WorkspaceSelectionTarget::AuiNode {
            document_path: proxy.document_path.clone(),
            document_id: proxy.document_id.clone(),
            node_id: proxy.node_id.clone(),
        }),
        secondary: Vec::new(),
    };
    let mut report =
        AuiSceneUnifiedAuthoringReport::empty(Some("Scenes/Main.scene.json".to_string()));
    report.aui_document_count = 1;
    report.proxy_count = 1;
    report.selectable_proxy_count = 1;
    report.selected_target_kind = Some("AuiNode".to_string());
    report.selected_document_path = Some(proxy.document_path.clone());
    report.selected_node_id = Some(proxy.node_id.clone());
    report.visual_order_entry_count = model.entries.len();
    report.selected_visual_order_key = Some(VisualOrderKey::screen_overlay(0, 10, 1));
    report.command_roundtrip_ok = true;

    let json = serde_json::to_string_pretty(&report).expect("report should serialize");
    let selection_json = serde_json::to_string(&selection).expect("selection should serialize");

    assert!(proxy.rect.contains(32.0, 24.0));
    assert_eq!(model.runtime_gap_count(), 0);
    assert!(json.contains(AUI_SCENE_UNIFIED_AUTHORING_REPORT_SCHEMA_VERSION));
    assert!(json.contains("ScreenOverlay"));
    assert!(!json.contains("WorldSpaceUi"));
    assert!(selection_json.contains("AuiNode"));
    assert!(selection_json.contains("score_text"));
}

#[test]
fn visual_order_key_serializes_before_world_composition_stage() {
    let key = VisualOrderKey::before_world(-1, 3, 2);
    let json = serde_json::to_string_pretty(&key).expect("key should serialize");
    let decoded: VisualOrderKey = serde_json::from_str(&json).expect("key should deserialize");

    assert_eq!(decoded, key);
    assert_eq!(key.render_space.as_str(), "BeforeWorld");
    assert!(json.contains("BeforeWorld"));
}

#[test]
fn aui_scene_authoring_rejects_single_node_cross_world_reorder() {
    let mut report =
        AuiSceneUnifiedAuthoringReport::empty(Some("Scenes/Main.scene.json".to_string()));
    report.selected_target_kind = Some("AuiNode".to_string());
    report.selected_node_id = Some("score_text".to_string());
    report.selected_visual_order_intent = Some(VisualOrderIntent::after(
        SceneVisualOrderTargetKind::SceneEntity,
        "entity-player",
        "single AUI node cannot be placed across World without an explicit LayerGroup or Canvas",
    ));

    report.mark_runtime_deferred("cross_world_visual_order_requires_runtime_composition_pass");
    report.reject_single_aui_node_cross_world_reorder();

    assert_eq!(report.last_reorder_status, AuiSceneReorderStatus::Rejected);
    assert!(!report.reorder_supported);
    assert!(!report.visual_order_runtime_supported);
    assert!(report.deferred_to_runtime_composition_gate);
    assert!(report
        .diagnostics
        .contains(&"extract_to_aui_layer_group_or_canvas".to_string()));
    assert!(report
        .next_actions
        .contains(&"RuntimeRenderer Multi-stage UI Composition Pass".to_string()));
}

#[test]
fn hierarchy_visual_order_model_is_default_authoring_view() {
    let mut visual_order =
        SceneVisualOrderAuthoringModel::empty(Some("Scenes/Main.scene.json".to_string()));
    visual_order.entries.push(SceneVisualOrderAuthoringEntry {
        entry_id: "aui-canvas:AUI/hud.aui.json:hud_canvas".to_string(),
        display_name: "hud_canvas".to_string(),
        target_kind: SceneVisualOrderTargetKind::AuiCanvas,
        target_ref: "hud_canvas".to_string(),
        parent_entry_id: None,
        visual_order_key: VisualOrderKey::screen_overlay(0, 0, 0),
        visual_order_intent: VisualOrderIntent::none(),
        runtime_supported: true,
        runtime_support_reason: "screen_overlay_runtime_pass_supported".to_string(),
        can_reorder: true,
        reorder_scope: "aui_canvas_visual_order".to_string(),
        diagnostics: Vec::new(),
    });
    let hierarchy = HierarchyModel {
        scene_id: Some("scene-main".to_string()),
        roots: Vec::new(),
        selected_entity_id: None,
        authoring_view: HierarchyAuthoringView::VisualOrder,
        visual_order: Some(visual_order),
        source_domain: HierarchySourceDomain::AuthoringScene,
        status: "authoring_scene".to_string(),
    };

    let json = serde_json::to_string_pretty(&hierarchy).expect("hierarchy should serialize");

    assert!(json.contains("VisualOrder"));
    assert!(json.contains("hud_canvas"));
    assert!(json.contains("default_view_is_visual_order"));
}

#[test]
fn ui_command_payload_imported_project_patch_commands_are_ai_readable() {
    let preview = UiCommandPayload::PreviewImportedProjectPatch {
        source_label: "fixture".to_string(),
        raw_json: Some("{}".to_string()),
        file_path: None,
        expected_patch_id: Some("patch-1".to_string()),
    };
    let apply = UiCommandPayload::ApplyImportedProjectPatch {
        proposal_id: "imported-project-patch-patch-1".to_string(),
    };

    let json = serde_json::to_string_pretty(&preview).expect("payload should serialize");

    assert!(json.contains("PreviewImportedProjectPatch"));
    assert_eq!(
        ui_command_id_for_payload(&preview),
        "preview_imported_project_patch"
    );
    assert_eq!(
        ui_command_id_for_payload(&apply),
        "apply_imported_project_patch"
    );
    assert_eq!(
        workspace_domain_for_payload(&preview),
        WorkspaceDomainKind::Report
    );
    assert_eq!(
        workspace_payload_kind(&preview),
        "PreviewImportedProjectPatch"
    );
}

#[test]
fn workflow_command_resolver_maps_core_commands_to_payloads() {
    for (command_id, domain, payload) in [
        ("play", WorkspaceDomainKind::Play, UiCommandPayload::Play),
        (
            "export_desktop_package",
            WorkspaceDomainKind::Build,
            UiCommandPayload::ExportDesktopPackage { profile_id: None },
        ),
        (
            "build_and_run_desktop_package",
            WorkspaceDomainKind::Build,
            UiCommandPayload::BuildAndRunDesktopPackage { profile_id: None },
        ),
        (
            "clear_console",
            WorkspaceDomainKind::Report,
            UiCommandPayload::ClearConsole,
        ),
    ] {
        let command = AuthoringCommand::new(
            command_id,
            domain,
            command_id,
            AuthoringCommandAvailability::Available,
            workspace_payload_kind(&payload),
        );

        assert_eq!(
            WorkflowCommandResolver::resolve(&command),
            WorkflowCommandResolution::Command(payload)
        );
    }
}

#[test]
fn game_view_target_payload_is_typed_and_owned_by_play_domain() {
    let payload = UiCommandPayload::SetGameViewTarget {
        width: 720,
        height: 1280,
        scale_policy: EditorGameViewScalePolicy::Contain,
    };

    assert_eq!(ui_command_id_for_payload(&payload), "set_game_view_target");
    assert_eq!(
        workspace_domain_for_payload(&payload),
        WorkspaceDomainKind::Play
    );
    assert_eq!(workspace_payload_kind(&payload), "SetGameViewTarget");
    let roundtrip: UiCommandPayload =
        serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap();
    assert_eq!(roundtrip, payload);
}

#[test]
fn workflow_command_resolver_maps_prefab_authoring_commands_to_payloads() {
    let commands = [
        (
            "open_prefab_document",
            "OpenPrefabDocument",
            "open_prefab_document",
        ),
        (
            "enter_prefab_stage",
            "EnterPrefabStage",
            "enter_prefab_stage",
        ),
        (
            "instantiate_prefab_in_scene",
            "InstantiatePrefabInScene",
            "instantiate_prefab_in_scene",
        ),
        (
            "apply_prefab_changes",
            "ApplyPrefabOverrideToAsset",
            "apply_prefab_override_to_asset",
        ),
        (
            "revert_prefab_override",
            "RevertPrefabOverride",
            "revert_prefab_override",
        ),
        (
            "validate_prefab_references",
            "ValidatePrefabReferences",
            "validate_prefab_references",
        ),
    ];

    for (command_id, payload_kind, expected_command_id) in commands {
        let command = AuthoringCommand::new(
            command_id,
            WorkspaceDomainKind::Prefab,
            command_id,
            AuthoringCommandAvailability::Available,
            payload_kind,
        );

        let WorkflowCommandResolution::Command(payload) =
            WorkflowCommandResolver::resolve(&command)
        else {
            panic!("{command_id} should resolve to a command");
        };

        assert_eq!(
            workspace_domain_for_payload(&payload),
            WorkspaceDomainKind::Prefab
        );
        assert_eq!(ui_command_id_for_payload(&payload), expected_command_id);
    }
}

#[test]
fn workflow_command_resolver_maps_aui_authoring_commands_to_payloads() {
    let commands = [
        (
            "create_aui_document",
            "CreateAuiDocument",
            "create_aui_document",
        ),
        ("open_aui_document", "OpenAuiDocument", "open_aui_document"),
        ("add_aui_node", "AddAuiNode", "add_aui_node"),
        (
            "set_aui_node_field",
            "SetAuiNodeField",
            "set_aui_node_field",
        ),
        (
            "set_aui_binding_path",
            "SetAuiBindingPath",
            "set_aui_binding_path",
        ),
        (
            "set_aui_action_ref",
            "SetAuiActionRef",
            "set_aui_action_ref",
        ),
        (
            "validate_aui_document",
            "ValidateAuiDocument",
            "validate_aui_document",
        ),
        ("save_aui_document", "SaveAuiDocument", "save_aui_document"),
        (
            "preview_aui_overlay",
            "PreviewAuiOverlay",
            "preview_aui_overlay",
        ),
    ];

    for (command_id, payload_kind, expected_command_id) in commands {
        let command = AuthoringCommand::new(
            command_id,
            WorkspaceDomainKind::Aui,
            command_id,
            AuthoringCommandAvailability::Available,
            payload_kind,
        );

        let WorkflowCommandResolution::Command(payload) =
            WorkflowCommandResolver::resolve(&command)
        else {
            panic!("{command_id} should resolve to a command");
        };

        assert_eq!(
            workspace_domain_for_payload(&payload),
            WorkspaceDomainKind::Aui
        );
        assert_eq!(ui_command_id_for_payload(&payload), expected_command_id);
    }
}

#[test]
fn workflow_command_resolver_maps_imported_project_patch_commands_to_payloads() {
    for (command_id, payload_kind, expected_command_id) in [
        (
            "import_project_patch",
            "ImportProjectPatch",
            "import_project_patch",
        ),
        (
            "preview_imported_project_patch",
            "PreviewImportedProjectPatch",
            "preview_imported_project_patch",
        ),
        (
            "apply_imported_project_patch",
            "ApplyImportedProjectPatch",
            "apply_imported_project_patch",
        ),
    ] {
        let command = AuthoringCommand::new(
            command_id,
            WorkspaceDomainKind::Report,
            command_id,
            AuthoringCommandAvailability::Available,
            payload_kind,
        );

        let WorkflowCommandResolution::Command(payload) =
            WorkflowCommandResolver::resolve(&command)
        else {
            panic!("{command_id} should resolve to a command");
        };

        assert_eq!(
            workspace_domain_for_payload(&payload),
            WorkspaceDomainKind::Report
        );
        assert_eq!(ui_command_id_for_payload(&payload), expected_command_id);
    }
}

#[test]
fn workflow_command_resolver_does_not_fabricate_missing_parameters() {
    let command = AuthoringCommand::new(
        "create_scene_entity",
        WorkspaceDomainKind::Scene,
        "Create Entity",
        AuthoringCommandAvailability::Available,
        "CreateSceneEntity",
    );

    assert!(matches!(
        WorkflowCommandResolver::resolve(&command),
        WorkflowCommandResolution::FocusDomainPanel {
            domain: WorkspaceDomainKind::Scene,
            ..
        }
    ));
}

#[test]
fn workflow_command_resolver_blocks_disabled_commands() {
    let command = AuthoringCommand::new(
        "play",
        WorkspaceDomainKind::Play,
        "Play",
        AuthoringCommandAvailability::Disabled,
        "Play",
    );

    assert!(matches!(
        WorkflowCommandResolver::resolve(&command),
        WorkflowCommandResolution::Disabled { .. }
    ));
}

#[test]
fn manual_walkthrough_requirements_cover_authoring_domains() {
    let requirements = manual_authoring_operation_requirements();

    assert!(requirements.len() >= 60);
    for domain in [
        WorkspaceDomainKind::Project,
        WorkspaceDomainKind::Asset,
        WorkspaceDomainKind::Scene,
        WorkspaceDomainKind::Prefab,
        WorkspaceDomainKind::Rule,
        WorkspaceDomainKind::Input,
        WorkspaceDomainKind::Aui,
        WorkspaceDomainKind::Play,
        WorkspaceDomainKind::Build,
        WorkspaceDomainKind::Report,
    ] {
        assert!(
            requirements
                .iter()
                .any(|requirement| requirement.domain == domain),
            "missing domain {domain:?}"
        );
    }
}

#[test]
fn manual_walkthrough_requirements_are_unique_and_generic() {
    let requirements = manual_authoring_operation_requirements();
    let mut ids = std::collections::BTreeSet::new();
    let forbidden = [
        "player", "enemy", "bullet", "health", "damage", "score", "wave", "weapon", "boss", "drop",
    ];

    for requirement in requirements {
        assert!(
            ids.insert(requirement.operation_id.clone()),
            "duplicate operation id {}",
            requirement.operation_id
        );
        let haystack = format!(
            "{} {} {}",
            requirement.operation_id, requirement.title, requirement.user_goal
        )
        .to_lowercase();
        for word in forbidden {
            assert!(
                !haystack.contains(word),
                "operation {} contains gameplay word {word}",
                requirement.operation_id
            );
        }
        if requirement.required_for_complex_project {
            assert!(!requirement.required_context.is_empty());
            assert!(!requirement.fallback_behavior.is_empty());
        }
    }
}

#[test]
fn manual_walkthrough_status_models_parameter_context_gap() {
    let status = ManualAuthoringOperationStatus::ExecutableCommandNeedsContext;
    let json = serde_json::to_string(&status).expect("status should serialize");
    let decoded: ManualAuthoringOperationStatus =
        serde_json::from_str(&json).expect("status should deserialize");

    assert_eq!(decoded, status);
    assert!(json.contains("ExecutableCommandNeedsContext"));
}

#[test]
fn manual_walkthrough_report_summarizes_gaps_and_next_actions() {
    let requirement = manual_authoring_operation_requirements()
        .into_iter()
        .find(|requirement| requirement.operation_id == "open_scene_document")
        .expect("open scene requirement");
    let operation = ManualWalkthroughOperationCoverage {
        requirement,
        status: ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
        resolution_summary: "OpenSceneDocument command needs a non-empty path.".to_string(),
        next_action: Some("select_scene_document".to_string()),
        gap_id: Some("gap.open_scene_document.context".to_string()),
        diagnostics: Vec::new(),
    };
    let report = ManualWalkthroughCoverageReport::from_operations(
        Some("project-test".to_string()),
        "manual-walkthrough-test",
        vec![operation],
        Vec::new(),
    );
    let summary = report.summary();

    assert_eq!(
        report.schema_version,
        MANUAL_WALKTHROUGH_COVERAGE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.status, ManualWalkthroughCoverageStatus::Partial);
    assert_eq!(report.needs_context_count, 1);
    assert_eq!(report.blocking_gaps.len(), 1);
    assert!(report
        .next_actions
        .contains(&"select_scene_document".to_string()));
    assert_eq!(summary.needs_context_count, 1);
}
