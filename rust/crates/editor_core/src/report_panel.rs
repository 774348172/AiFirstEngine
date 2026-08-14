use editor_ui_model::{
    AuthoringWorkflowModel, DiagnosticSeverity, EditorDiagnostic, EvidenceEntry,
    ManualWalkthroughCoverageReport, ManualWalkthroughCoverageStatus, MissingOperationSeverity,
    ProjectAuthoringWorkspaceModel, ReportArtifactRef, ReportDescriptor, ReportPanelModel,
    ReportRegistrySummary, ReportSourceKind, ReportStatus, UnifiedReportEntry, WorkspaceDomainKind,
    WorkspaceDomainStatus,
};

use crate::{
    summarize_patch_history, AssetBrowserReportLevel, CommandResult, CommandStatus,
    CommandTransaction, ConsistencyReportLevel, DesktopExportDiagnostic,
    DesktopExportDiagnosticSeverity, DesktopExportReport, DesktopExportStatus,
    EditorBuildAndRunDiagnostic, EditorBuildAndRunDiagnosticSeverity, EditorBuildAndRunReport,
    EditorBuildAndRunStatus, EditorPlayPreviewPackageReport, EditorPreviewPackageDiagnostic,
    EditorPreviewPackageDiagnosticSeverity, EditorPreviewPackageStatus, EditorSession,
    GameViewPresentDiagnostic, GameViewPresentDiagnosticSeverity, GameViewPresentReport,
    GameViewPresentStatus, LlmPatchReportLevel, PatchDiagnostic, PatchDiagnosticSeverity,
    PlaySessionDiagnostic, PlaySessionDiagnosticSeverity, PlaySessionReport, PlaySessionState,
    PrefabAuthoringReport, PrefabAuthoringStatus, PrefabDiagnostic, PrefabDiagnosticSeverity,
    ReleasePackageDiagnostic, ReleasePackageReport, ReleasePackageReportLevel,
    ReleasePackageStatus, SaveReloadRebuildConsistencyReport, SaveReloadRebuildStatus,
    ASSET_BROWSER_NATIVE_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
    DESKTOP_EXPORT_REPORT_SCHEMA_VERSION, EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION,
    EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION, GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION,
    LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION, PLAY_SESSION_REPORT_SCHEMA_VERSION,
    PREFAB_AUTHORING_REPORT_SCHEMA_VERSION, PROJECT_PATCH_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
    RELEASE_PACKAGE_REPORT_SCHEMA_VERSION, SAVE_RELOAD_REBUILD_CONSISTENCY_REPORT_SCHEMA_VERSION,
    SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH,
};
use std::path::Path;

pub struct ReportProviderContext<'a> {
    pub session: &'a EditorSession,
    pub workspace: &'a ProjectAuthoringWorkspaceModel,
    pub authoring_workflow: &'a AuthoringWorkflowModel,
    pub manual_walkthrough_report: &'a ManualWalkthroughCoverageReport,
}

pub trait ReportProvider {
    fn descriptor(&self) -> ReportDescriptor;
    fn enabled(&self, _context: &ReportProviderContext<'_>) -> bool {
        true
    }
    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry;
}

pub struct ReportRegistry {
    providers: Vec<Box<dyn ReportProvider>>,
}

impl ReportRegistry {
    pub fn standard() -> Self {
        Self {
            providers: vec![
                Box::new(BuildExportReportProvider),
                Box::new(BuildAndRunReportProvider),
                Box::new(ReleasePackageReportProvider),
                Box::new(EditorPreviewPackageReportProvider),
                Box::new(PlayRuntimeReportProvider),
                Box::new(GameViewPresentReportProvider),
                Box::new(ManualWalkthroughCoverageReportProvider),
                Box::new(RuleAuthoringReportProvider),
                Box::new(InputMappingAuthoringReportProvider),
                Box::new(AssetBrowserReportProvider),
                Box::new(PrefabAuthoringReportProvider),
                Box::new(AuiAuthoringReportProvider),
                Box::new(ProjectPatchReportProvider),
                Box::new(SaveReloadRebuildReportProvider),
                Box::new(DiagnosticsReportProvider),
                Box::new(QualityArchitectureReportProvider),
                Box::new(EditorUiReachabilityReportProvider),
                Box::new(ComplexShooterE2eReportProvider),
            ],
        }
    }

    pub fn descriptors(&self) -> Vec<ReportDescriptor> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor())
            .collect()
    }

    pub fn build_model(
        &self,
        context: &ReportProviderContext<'_>,
        selected_report_id: Option<String>,
    ) -> ReportPanelModel {
        let descriptors = self
            .providers
            .iter()
            .map(|provider| {
                let mut descriptor = provider.descriptor();
                descriptor.enabled = provider.enabled(context);
                descriptor
            })
            .collect::<Vec<_>>();
        let reports = self
            .providers
            .iter()
            .filter(|provider| provider.enabled(context))
            .map(|provider| provider.collect(context).finalize_counts())
            .collect::<Vec<_>>();
        ReportPanelModel::from_reports(
            ReportRegistrySummary::from_descriptors(descriptors),
            reports,
            selected_report_id,
        )
    }
}

struct BuildExportReportProvider;
struct BuildAndRunReportProvider;
struct ReleasePackageReportProvider;
struct EditorPreviewPackageReportProvider;
struct PlayRuntimeReportProvider;
struct GameViewPresentReportProvider;
struct ManualWalkthroughCoverageReportProvider;
struct RuleAuthoringReportProvider;
struct InputMappingAuthoringReportProvider;
struct AssetBrowserReportProvider;
struct PrefabAuthoringReportProvider;
struct AuiAuthoringReportProvider;
struct ProjectPatchReportProvider;
struct SaveReloadRebuildReportProvider;
struct DiagnosticsReportProvider;
struct QualityArchitectureReportProvider;
struct EditorUiReachabilityReportProvider;
struct ComplexShooterE2eReportProvider;

impl ReportProvider for EditorUiReachabilityReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "editor.ui_reachability",
            "Editor UI Reachability",
            WorkspaceDomainKind::Report,
            "editor_ui_reachability",
            ReportSourceKind::Artifact,
            &["editor-ui-reachability-report.v1"],
        )
    }

    fn collect(&self, _context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        placeholder(
            "report-editor-ui-reachability",
            "editor.ui_reachability",
            "Editor UI Reachability",
            WorkspaceDomainKind::Report,
            "editor_ui_reachability",
            "No Editor UI Reachability Summary artifact is loaded.",
            vec![
                "Run the explicit editor_ui_authority gate and load its redacted Summary artifact."
                    .to_string(),
            ],
        )
    }
}

impl ReportProvider for QualityArchitectureReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "quality.architecture",
            "Quality Architecture",
            WorkspaceDomainKind::Report,
            "quality_architecture",
            ReportSourceKind::Artifact,
            &["quality-gate-report.v2"],
        )
    }

    fn collect(&self, _context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        placeholder(
            "report-quality-architecture",
            "quality.architecture",
            "Quality Architecture",
            WorkspaceDomainKind::Report,
            "quality_architecture",
            "No project-scoped Quality Architecture Summary artifact is loaded.",
            vec![
                "Run the quality gate explicitly and load its redacted Summary artifact."
                    .to_string(),
            ],
        )
    }
}

impl ReportProvider for BuildExportReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "build.export",
            "Build Export",
            WorkspaceDomainKind::Build,
            "desktop_export",
            ReportSourceKind::InMemory,
            &[DESKTOP_EXPORT_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = &context.session.last_desktop_export_report else {
            return placeholder(
                "report-build-export",
                "build.export",
                "Latest Build Export",
                WorkspaceDomainKind::Build,
                "desktop_export",
                "No desktop export report has been produced yet.",
                vec!["export_desktop_package".to_string()],
            );
        };

        build_export_entry(report)
    }
}

impl ReportProvider for BuildAndRunReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "build.and_run",
            "Build And Run",
            WorkspaceDomainKind::Build,
            "editor_build_and_run",
            ReportSourceKind::InMemory,
            &[EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = &context.session.last_build_and_run_report else {
            return placeholder(
                "report-build-and-run",
                "build.and_run",
                "Latest Build And Run",
                WorkspaceDomainKind::Build,
                "editor_build_and_run",
                "No Build And Run report has been produced yet.",
                vec!["build_and_run_desktop_package".to_string()],
            );
        };

        build_and_run_entry(report)
    }
}

impl ReportProvider for ReleasePackageReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "build.release_package",
            "Release Package",
            WorkspaceDomainKind::Build,
            "release_package",
            ReportSourceKind::Artifact,
            &[RELEASE_PACKAGE_REPORT_SCHEMA_VERSION],
        )
    }

    fn enabled(&self, context: &ReportProviderContext<'_>) -> bool {
        context
            .session
            .last_release_package_report
            .as_ref()
            .is_none_or(|report| report.report_level != ReleasePackageReportLevel::Off)
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = &context.session.last_release_package_report else {
            return placeholder(
                "report-release-package",
                "build.release_package",
                "Latest Release Package",
                WorkspaceDomainKind::Build,
                "release_package",
                "No release package report has been produced for the active project.",
                vec!["build_release_package".to_string()],
            );
        };
        release_package_entry(report)
    }
}

impl ReportProvider for PlayRuntimeReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "play.runtime",
            "Play Runtime",
            WorkspaceDomainKind::Play,
            "play_session",
            ReportSourceKind::InMemory,
            &[PLAY_SESSION_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = &context.session.last_play_session_report else {
            return placeholder(
                "report-play-runtime",
                "play.runtime",
                "Latest Play Session",
                WorkspaceDomainKind::Play,
                "play_session",
                "No play session report has been produced yet.",
                vec!["play".to_string()],
            );
        };

        play_session_entry(report)
    }
}

impl ReportProvider for EditorPreviewPackageReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "play.preview_package",
            "Editor Play Preview Package",
            WorkspaceDomainKind::Play,
            "editor_play_preview_package",
            ReportSourceKind::InMemory,
            &[EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = &context.session.last_editor_preview_package_report else {
            return placeholder(
                "report-play-preview-package",
                "play.preview_package",
                "Latest Editor Play Preview Package",
                WorkspaceDomainKind::Play,
                "editor_play_preview_package",
                "No Editor Play preview package report has been produced yet.",
                vec!["play".to_string()],
            );
        };

        preview_package_entry(report)
    }
}

impl ReportProvider for GameViewPresentReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "play.game_view_present",
            "Editor GameView Present",
            WorkspaceDomainKind::Play,
            "editor_gameview_present",
            ReportSourceKind::InMemory,
            &[GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = &context.session.last_game_view_present_report else {
            return placeholder(
                "report-play-gameview-present",
                "play.game_view_present",
                "Latest Editor GameView Present",
                WorkspaceDomainKind::Play,
                "editor_gameview_present",
                "No Editor GameView present report has been produced yet.",
                vec!["play".to_string()],
            );
        };

        game_view_present_entry(report)
    }
}

impl ReportProvider for ManualWalkthroughCoverageReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "authoring.manual_walkthrough",
            "Manual Walkthrough Coverage",
            WorkspaceDomainKind::Report,
            "manual_walkthrough_coverage",
            ReportSourceKind::Derived,
            &[editor_ui_model::MANUAL_WALKTHROUGH_COVERAGE_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let report = context.manual_walkthrough_report;
        let mut entry = UnifiedReportEntry::new(
            "report-manual-walkthrough",
            "authoring.manual_walkthrough",
            "Manual Walkthrough Coverage",
            WorkspaceDomainKind::Report,
            "manual_walkthrough_coverage",
        )
        .with_summary(format!(
            "operations={} executable={} needs_context={} missing_command={} missing_service={} blocked={}",
            report.operation_count,
            report.executable_count,
            report.needs_context_count,
            report.missing_command_count,
            report.missing_service_count,
            report.blocked_count
        ));
        entry.status = match report.status {
            ManualWalkthroughCoverageStatus::Pass => ReportStatus::Passed,
            ManualWalkthroughCoverageStatus::Partial => ReportStatus::Partial,
            ManualWalkthroughCoverageStatus::Fail => ReportStatus::Failed,
        };
        entry.schema_version = Some(report.schema_version.clone());
        entry.source_kind = ReportSourceKind::Derived;
        entry.next_actions = report.next_actions.clone();
        entry.evidence = report
            .blocking_gaps
            .iter()
            .map(|gap| {
                let mut evidence = EvidenceEntry::diagnostic(
                    format!("manual_walkthrough.{}", gap.gap_id),
                    gap.domain,
                    severity_from_missing_gap(gap.severity),
                    gap.operation_id.clone(),
                    gap.reason.clone(),
                );
                evidence.suggested_action = Some(gap.suggested_next_action.clone());
                evidence.next_actions = vec![gap.suggested_next_action.clone()];
                evidence
            })
            .collect();
        entry
    }
}

impl ReportProvider for RuleAuthoringReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "authoring.rule",
            "Rule Authoring",
            WorkspaceDomainKind::Rule,
            "rule_authoring",
            ReportSourceKind::Derived,
            &[
                editor_ui_model::RULE_AUTHORING_REPORT_SCHEMA_VERSION,
                editor_ui_model::RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION,
                editor_ui_model::RULE_GRAPH_PREVIEW_SCHEMA_VERSION,
            ],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let model = context.session.build_rule_authoring_model();
        let report = &model.document.report;
        let card_report = &model.card_authoring.report_summary;
        let mut entry = UnifiedReportEntry::new(
            "report-rule-authoring",
            "authoring.rule",
            "Rule Authoring",
            WorkspaceDomainKind::Rule,
            "rule_authoring",
        )
        .with_summary(format!(
            "{} cards={} graph_nodes={} graph_edges={} read_only_graph={}",
            report.human_summary,
            card_report.card_count,
            card_report.graph_node_count,
            card_report.graph_edge_count,
            card_report.read_only_graph
        ));
        entry.status = match report.status {
            editor_ui_model::RuleAuthoringStatus::Missing => ReportStatus::Empty,
            editor_ui_model::RuleAuthoringStatus::Ready
            | editor_ui_model::RuleAuthoringStatus::Dirty => ReportStatus::Partial,
            editor_ui_model::RuleAuthoringStatus::Valid
            | editor_ui_model::RuleAuthoringStatus::Built => ReportStatus::Passed,
            editor_ui_model::RuleAuthoringStatus::Invalid
            | editor_ui_model::RuleAuthoringStatus::Failed => ReportStatus::Failed,
        };
        entry.source_path = model.selected_path.clone();
        entry.schema_version = Some(card_report.schema_version.clone());
        entry.next_actions = card_report.next_actions.clone();
        entry.evidence.push(EvidenceEntry::diagnostic(
            "rule_card_authoring.summary",
            WorkspaceDomainKind::Rule,
            DiagnosticSeverity::Info,
            "rule_card_authoring",
            format!(
                "cards={} editable_cards={} graph_nodes={} graph_edges={} read_only_graph={}",
                card_report.card_count,
                card_report.editable_card_count,
                card_report.graph_node_count,
                card_report.graph_edge_count,
                card_report.read_only_graph
            ),
        ));
        for mapping in card_report.source_mappings.iter().take(8) {
            let mut evidence = EvidenceEntry::diagnostic(
                format!("rule_card_authoring.mapping.{}", mapping.source_path),
                WorkspaceDomainKind::Rule,
                DiagnosticSeverity::Info,
                "rule_card_source_mapping",
                format!(
                    "source_path={} card_id={} node_id={}",
                    mapping.source_path,
                    mapping.card_id.as_deref().unwrap_or("none"),
                    mapping.node_id.as_deref().unwrap_or("none")
                ),
            );
            evidence.source_path = Some(mapping.source_path.clone());
            evidence.node_id = mapping.node_id.clone();
            entry.evidence.push(evidence);
        }
        entry.diagnostics = report
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let mut evidence = EvidenceEntry::diagnostic(
                    format!("rule_authoring.{}", diagnostic.code),
                    WorkspaceDomainKind::Rule,
                    severity_from_rule(diagnostic.severity.clone()),
                    diagnostic.code.clone(),
                    diagnostic.human_explanation.clone(),
                );
                evidence.source_path = diagnostic
                    .path
                    .clone()
                    .or_else(|| model.selected_path.clone());
                evidence.suggested_action = diagnostic.suggested_fix.clone();
                evidence
            })
            .collect();
        for stage in [
            &report.generated_rust_source,
            &report.static_registry_source,
            &report.artifact_lifecycle,
            &report.runtime_package_manifest,
            &report.cargo_build,
        ] {
            if let Some(path) = &stage.path {
                entry.artifacts.push(artifact(
                    stage.artifact_id.as_deref().unwrap_or(path),
                    &stage.summary,
                    path,
                    "rule_authoring_stage",
                ));
            }
        }
        entry
    }
}

impl ReportProvider for InputMappingAuthoringReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "authoring.input_mapping",
            "Input Mapping Authoring",
            WorkspaceDomainKind::Input,
            "input_mapping_visual_authoring",
            ReportSourceKind::Derived,
            &[crate::INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION],
        )
    }

    fn enabled(&self, context: &ReportProviderContext<'_>) -> bool {
        context
            .session
            .input_mapping_editor_state
            .as_ref()
            .is_none_or(|state| state.report_level != editor_ui_model::InputMappingReportLevel::Off)
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let model = context.session.build_input_mapping_authoring_model();
        let level = model.report_level;
        let mut entry = UnifiedReportEntry::new(
            "report-input-mapping-authoring",
            "authoring.input_mapping",
            "Input Mapping Authoring",
            WorkspaceDomainKind::Input,
            "input_mapping_visual_authoring",
        )
        .with_summary(format!(
            "level={level:?} mapping={} dirty={} contexts={} actions={} bindings={} diagnostics={} preview={}",
            model.mapping_id.as_deref().unwrap_or("missing"),
            model.dirty,
            model.report.context_count,
            model.report.action_count,
            model.report.binding_count,
            model.report.diagnostics.len(),
            model.preview.is_some()
        ));
        entry.source_path = model.selected_path.clone();
        entry.schema_version =
            Some(crate::INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION.to_string());
        entry.status = match model.report.validation_status {
            editor_ui_model::InputMappingValidationStatus::Missing => ReportStatus::Empty,
            editor_ui_model::InputMappingValidationStatus::Ok => ReportStatus::Passed,
            editor_ui_model::InputMappingValidationStatus::Warning => ReportStatus::Partial,
            editor_ui_model::InputMappingValidationStatus::Error => ReportStatus::Failed,
        };
        entry.evidence.push(EvidenceEntry::diagnostic(
            "input_mapping.summary",
            WorkspaceDomainKind::Input,
            DiagnosticSeverity::Info,
            "input_mapping.summary",
            format!(
                "source_hash={} selected_context={} selected_action={} selected_binding={}",
                model.source_hash.as_deref().unwrap_or("not-open"),
                model.selected_context_id.as_deref().unwrap_or("none"),
                model.selected_action_id.as_deref().unwrap_or("none"),
                model.selected_binding_id.as_deref().unwrap_or("none")
            ),
        ));
        let diagnostic_limit = if level == editor_ui_model::InputMappingReportLevel::Trace {
            usize::MAX
        } else {
            5
        };
        entry.diagnostics = model
            .report
            .diagnostics
            .iter()
            .take(diagnostic_limit)
            .map(|diagnostic| {
                let mut evidence = EvidenceEntry::diagnostic(
                    format!(
                        "input_mapping.{}.{}",
                        diagnostic.code,
                        diagnostic.binding_id.as_deref().unwrap_or("asset")
                    ),
                    WorkspaceDomainKind::Input,
                    match diagnostic.severity {
                        editor_ui_model::InputMappingDiagnosticSeverity::Info => {
                            DiagnosticSeverity::Info
                        }
                        editor_ui_model::InputMappingDiagnosticSeverity::Warning => {
                            DiagnosticSeverity::Warning
                        }
                        editor_ui_model::InputMappingDiagnosticSeverity::Error => {
                            DiagnosticSeverity::Error
                        }
                    },
                    diagnostic.code.clone(),
                    diagnostic.message.clone(),
                );
                evidence.source_path = diagnostic
                    .path
                    .clone()
                    .or_else(|| model.selected_path.clone());
                evidence.node_id = diagnostic.binding_id.clone();
                evidence.suggested_action = diagnostic.suggested_fix.clone();
                evidence
            })
            .collect();
        if let Some(preview) = &model.preview {
            entry.evidence.push(EvidenceEntry::diagnostic(
                "input_mapping.preview",
                WorkspaceDomainKind::Input,
                DiagnosticSeverity::Info,
                "input_mapping.preview",
                format!(
                    "status={:?} device_path={} event={} matched={} shadowed={} actions={}",
                    preview.status,
                    preview.device_path,
                    preview.input_event_kind,
                    preview.matched_binding_ids.join(","),
                    preview.shadowed_binding_ids.join(","),
                    preview
                        .actions
                        .iter()
                        .map(|action| format!("{}={}", action.action_id, action.value))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ));
        }
        if level == editor_ui_model::InputMappingReportLevel::Trace {
            entry.evidence.extend(model.bindings.iter().map(|binding| {
                let mut evidence = EvidenceEntry::diagnostic(
                    format!("input_mapping.binding.{}", binding.binding_id),
                    WorkspaceDomainKind::Input,
                    DiagnosticSeverity::Info,
                    "input_mapping.binding_trace",
                    format!(
                        "context={} action={} device={} trigger={} processor={}",
                        binding.context_id,
                        binding.action_id,
                        binding.device_path,
                        binding.trigger,
                        binding.processor
                    ),
                );
                evidence.source_path = model.selected_path.clone();
                evidence.node_id = Some(binding.binding_id.clone());
                evidence
            }));
        }
        if model.dirty {
            entry.next_actions.push("save_input_mapping".to_string());
        }
        if entry.status == ReportStatus::Failed {
            entry
                .next_actions
                .push("fix_input_mapping_diagnostics".to_string());
        }
        entry
    }
}

impl ReportProvider for AssetBrowserReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "authoring.asset_browser",
            "Asset Browser",
            WorkspaceDomainKind::Asset,
            "asset_browser_native_productization",
            ReportSourceKind::Derived,
            &[ASSET_BROWSER_NATIVE_PRODUCTIZATION_REPORT_SCHEMA_VERSION],
        )
    }

    fn enabled(&self, context: &ReportProviderContext<'_>) -> bool {
        context.session.asset_browser_state.report_level != AssetBrowserReportLevel::Off
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let state = &context.session.asset_browser_state;
        let level = state.report_level;
        let model = state.model(
            state.ui_state.query.clone(),
            state.ui_state.selection.clone(),
        );
        let thumbnail = state.thumbnail_summary();
        let snapshot = state.index_snapshot.as_ref();
        let index_diagnostics = snapshot
            .map(|snapshot| snapshot.report.diagnostics.as_slice())
            .unwrap_or_default();
        let error_count = index_diagnostics
            .iter()
            .chain(thumbnail.diagnostics.iter())
            .filter(|diagnostic| {
                diagnostic.severity == editor_ui_model::AssetBrowserDiagnosticSeverity::Error
            })
            .count();
        let warning_count = index_diagnostics
            .iter()
            .chain(thumbnail.diagnostics.iter())
            .filter(|diagnostic| {
                diagnostic.severity == editor_ui_model::AssetBrowserDiagnosticSeverity::Warning
            })
            .count();
        let index_revision = snapshot.map_or(0, |snapshot| snapshot.revision);
        let scan_generation = snapshot.map_or(0, |snapshot| snapshot.scan_generation);
        let total_entries = snapshot.map_or(0, |snapshot| snapshot.entries.len());
        let mut entry = UnifiedReportEntry::new(
            "report-asset-browser",
            "authoring.asset_browser",
            "Asset Browser",
            WorkspaceDomainKind::Asset,
            "asset_browser_native_productization",
        )
        .with_summary(format!(
            "level={level:?} index={:?} revision={index_revision} scan_generation={scan_generation} entries={total_entries} visible={} selected={} folders={} missing={} unimported={} thumbnails_ready={} thumbnails_pending={} thumbnail_bytes={} errors={error_count}",
            state.index_status,
            model.entries.len(),
            model.selection.selected_entry_keys.len(),
            snapshot.map_or(0, |snapshot| snapshot.report.folder_count),
            snapshot.map_or(0, |snapshot| snapshot.report.missing_count),
            snapshot.map_or(0, |snapshot| snapshot.report.unimported_count),
            thumbnail.ready_count,
            thumbnail.pending_count,
            thumbnail.cpu_bytes,
        ));
        entry.schema_version =
            Some(ASSET_BROWSER_NATIVE_PRODUCTIZATION_REPORT_SCHEMA_VERSION.to_string());
        entry.source_kind = ReportSourceKind::Derived;
        entry.source_path = snapshot.map(|snapshot| snapshot.project_root.display().to_string());
        entry.status = match state.index_status {
            editor_ui_model::AssetBrowserIndexStatus::NotBuilt => ReportStatus::Empty,
            editor_ui_model::AssetBrowserIndexStatus::Scanning
            | editor_ui_model::AssetBrowserIndexStatus::Stale => ReportStatus::Partial,
            editor_ui_model::AssetBrowserIndexStatus::Failed => ReportStatus::Failed,
            editor_ui_model::AssetBrowserIndexStatus::Ready if error_count > 0 => {
                ReportStatus::Failed
            }
            editor_ui_model::AssetBrowserIndexStatus::Ready if warning_count > 0 => {
                ReportStatus::Partial
            }
            editor_ui_model::AssetBrowserIndexStatus::Ready => ReportStatus::Passed,
        };
        if matches!(
            state.index_status,
            editor_ui_model::AssetBrowserIndexStatus::Stale
                | editor_ui_model::AssetBrowserIndexStatus::Failed
        ) {
            entry.next_actions.push("refresh_asset_browser".to_string());
        }
        if thumbnail.failed_count > 0 {
            entry
                .next_actions
                .push("inspect_asset_thumbnail_diagnostics".to_string());
        }

        let diagnostic_limit = if level == AssetBrowserReportLevel::Trace {
            usize::MAX
        } else {
            5
        };
        entry.diagnostics = index_diagnostics
            .iter()
            .chain(thumbnail.diagnostics.iter())
            .take(diagnostic_limit)
            .map(asset_browser_diagnostic_evidence)
            .collect();
        let mut summary_evidence = EvidenceEntry::diagnostic(
            "asset_browser.summary",
            WorkspaceDomainKind::Asset,
            if error_count > 0 {
                DiagnosticSeverity::Error
            } else if warning_count > 0 {
                DiagnosticSeverity::Warning
            } else {
                DiagnosticSeverity::Info
            },
            "asset_browser.summary",
            format!(
                "scan_started={} scan_committed={} cache_records={} cache_hits={} decodes={} evictions={}",
                state.scan_started_count,
                state.scan_committed_count,
                thumbnail.record_count,
                thumbnail.cache_hit_count,
                thumbnail.decode_count,
                thumbnail.eviction_count
            ),
        );
        summary_evidence.stage = Some("summary".to_string());
        summary_evidence.source_path = entry.source_path.clone();
        entry.evidence.push(summary_evidence);

        if level == AssetBrowserReportLevel::Trace {
            if let Some(snapshot) = snapshot {
                let mut index = EvidenceEntry::diagnostic(
                    "asset_browser.index_trace",
                    WorkspaceDomainKind::Asset,
                    DiagnosticSeverity::Info,
                    "asset_browser.index_trace",
                    format!(
                        "revision={} generation={} fingerprint={} reasons={}",
                        snapshot.revision,
                        snapshot.scan_generation,
                        snapshot.source_fingerprint,
                        snapshot.dirty_reasons.join(",")
                    ),
                );
                index.stage = Some("index".to_string());
                index.source_path = Some(snapshot.project_root.display().to_string());
                index.suggested_action = Some("refresh_asset_browser".to_string());
                entry.evidence.push(index);

                for asset in &snapshot.entries {
                    let mut identity = EvidenceEntry::diagnostic(
                        format!("asset_browser.identity.{}", asset.entry_key.stable_token()),
                        WorkspaceDomainKind::Asset,
                        DiagnosticSeverity::Info,
                        "asset_browser.identity_resolution",
                        format!(
                            "stage=identity source={} role={:?} kind={:?} identity={:?} asset_id={} asset_type={} source_status={:?}",
                            asset.canonical_path,
                            asset.role,
                            asset.kind,
                            asset.identity_status,
                            asset.asset_id.as_deref().unwrap_or("none"),
                            asset.asset_type_id.as_deref().unwrap_or("none"),
                            asset.source_status,
                        ),
                    );
                    identity.stage = Some("identity".to_string());
                    identity.source_path = Some(asset.canonical_path.clone());
                    identity.node_id = asset.asset_id.clone();
                    entry.evidence.push(identity);
                }
            }

            let mut query = EvidenceEntry::diagnostic(
                "asset_browser.query_trace",
                WorkspaceDomainKind::Asset,
                DiagnosticSeverity::Info,
                "asset_browser.query_trace",
                format!(
                    "search={} folder={} kinds={:?} include_missing={} include_unimported={} visible={} selected={}",
                    model.query.search_text,
                    model.query.folder.as_deref().unwrap_or("root"),
                    model.query.kinds,
                    model.query.include_missing,
                    model.query.include_unimported,
                    model.entries.len(),
                    model.selection.selected_entry_keys.len()
                ),
            );
            query.stage = Some("query".to_string());
            query.source_path = entry.source_path.clone();
            entry.evidence.push(query);

            let mut thumbnail_trace = EvidenceEntry::diagnostic(
                "asset_browser.thumbnail_trace",
                WorkspaceDomainKind::Asset,
                if thumbnail.failed_count > 0 {
                    DiagnosticSeverity::Warning
                } else {
                    DiagnosticSeverity::Info
                },
                "asset_browser.thumbnail_trace",
                format!(
                    "records={} pending={} ready={} failed={} cpu_bytes={} decode={} cache_hit={} eviction={}",
                    thumbnail.record_count,
                    thumbnail.pending_count,
                    thumbnail.ready_count,
                    thumbnail.failed_count,
                    thumbnail.cpu_bytes,
                    thumbnail.decode_count,
                    thumbnail.cache_hit_count,
                    thumbnail.eviction_count
                ),
            );
            thumbnail_trace.stage = Some("thumbnail".to_string());
            thumbnail_trace.source_path = entry.source_path.clone();
            thumbnail_trace.suggested_action = (thumbnail.failed_count > 0)
                .then(|| "inspect_asset_thumbnail_diagnostics".to_string());
            entry.evidence.push(thumbnail_trace);

            if let Some(picker) = &state.active_picker {
                let mut picker_trace = EvidenceEntry::diagnostic(
                    "asset_browser.picker_trace",
                    WorkspaceDomainKind::Asset,
                    DiagnosticSeverity::Info,
                    "asset_browser.picker_trace",
                    format!(
                        "target={:?}:{}:{} candidate={} accepted={}",
                        picker.request.target_kind,
                        picker.request.target_path.as_deref().unwrap_or("none"),
                        picker
                            .request
                            .target_field_path
                            .as_deref()
                            .unwrap_or("none"),
                        picker.candidate.is_some(),
                        picker
                            .candidate
                            .as_ref()
                            .is_some_and(|candidate| candidate.accepted)
                    ),
                );
                picker_trace.stage = Some("picker".to_string());
                picker_trace.source_path = picker.request.target_path.clone();
                picker_trace.request_id = Some(picker.request.request_id.clone());
                picker_trace.suggested_action = Some("confirm_or_cancel_asset_pick".to_string());
                entry.evidence.push(picker_trace);
            }
            if let Some(plan) = &state.last_pick_commit_plan {
                let mut commit_trace = EvidenceEntry::diagnostic(
                    "asset_browser.pick_commit_trace",
                    WorkspaceDomainKind::Asset,
                    DiagnosticSeverity::Info,
                    "asset_browser.pick_commit_trace",
                    format!(
                        "source={} target={}:{} old={} new={}:{}",
                        plan.target_document_path,
                        plan.target_object_id,
                        plan.target_field_path,
                        plan.old_asset_ref
                            .as_ref()
                            .map(|asset| asset.asset_id.as_str())
                            .unwrap_or("none"),
                        plan.new_asset_ref.asset_id,
                        plan.new_asset_ref.asset_type_id
                    ),
                );
                commit_trace.stage = Some("pick_commit".to_string());
                commit_trace.source_path = Some(plan.target_document_path.clone());
                commit_trace.node_id = Some(plan.target_object_id.clone());
                commit_trace.request_id = Some(plan.request_id.clone());
                entry.evidence.push(commit_trace);
            }
        }
        entry
    }
}

impl ReportProvider for PrefabAuthoringReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "authoring.prefab",
            "Prefab Authoring",
            WorkspaceDomainKind::Prefab,
            "prefab_authoring",
            ReportSourceKind::InMemory,
            &[PREFAB_AUTHORING_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        prefab_authoring_entry(&context.session.prefab_authoring.validation_report)
    }
}

impl ReportProvider for AuiAuthoringReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "authoring.aui",
            "AUI Authoring",
            WorkspaceDomainKind::Aui,
            "aui_authoring",
            ReportSourceKind::Derived,
            &[crate::AUI_AUTHORING_REPORT_SCHEMA_VERSION],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let domain = context
            .workspace
            .domains
            .iter()
            .find(|domain| domain.kind == WorkspaceDomainKind::Aui);
        let mut entry = UnifiedReportEntry::new(
            "report-aui-authoring",
            "authoring.aui",
            "AUI Authoring",
            WorkspaceDomainKind::Aui,
            "aui_authoring",
        )
        .with_summary(
            domain
                .map(|domain| domain.summary.clone())
                .unwrap_or_else(|| "AUI domain summary is unavailable.".to_string()),
        );
        entry.status = domain
            .map(|domain| status_from_workspace_domain(domain.status))
            .unwrap_or(ReportStatus::Unknown);
        entry.source_path = domain.and_then(|domain| domain.active_document_path.clone());
        if matches!(entry.status, ReportStatus::Empty | ReportStatus::Partial) {
            entry.next_actions.push("create_aui_document".to_string());
        }
        if let Some(selection) = &context.session.selected_aui_node {
            entry.evidence.push(EvidenceEntry::diagnostic(
                "aui_authoring.selected_node",
                WorkspaceDomainKind::Aui,
                DiagnosticSeverity::Info,
                "selected_aui_node",
                format!("Selected AUI target: {selection:?}"),
            ));
        }
        entry
    }
}

impl ReportProvider for ProjectPatchReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "project.patch",
            "Project Patch",
            WorkspaceDomainKind::Report,
            "project_patch",
            ReportSourceKind::InMemory,
            &[
                PROJECT_PATCH_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
                LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION,
            ],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let summary = summarize_patch_history(&context.session.patch_history.entries);
        let mut entry = UnifiedReportEntry::new(
            "report-project-patch",
            "project.patch",
            "Project Patch",
            WorkspaceDomainKind::Report,
            "project_patch",
        )
        .with_summary(format!(
            "applied_count={} reversible_count={} last_patch={}",
            summary.applied_count,
            summary.reversible_count,
            summary.last_patch_id.as_deref().unwrap_or("none")
        ));
        entry.status = if summary.applied_count == 0 {
            ReportStatus::Empty
        } else if summary
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.severity, PatchDiagnosticSeverity::Error))
        {
            ReportStatus::Failed
        } else {
            ReportStatus::Passed
        };
        entry.next_actions = if summary.applied_count == 0 {
            vec!["import_project_patch".to_string()]
        } else {
            Vec::new()
        };
        entry.diagnostics = summary
            .diagnostics
            .iter()
            .map(|diagnostic| patch_diagnostic_evidence(diagnostic, "project_patch.history"))
            .collect();
        if let Some(report) = &context.session.last_llm_patch_report {
            entry.summary.push_str(&format!(
                " llm_provider={} model={} mode={:?} status={} repair_attempts={}",
                report.provider_id,
                report.model,
                report.structured_output_mode,
                report.final_status,
                report.repair_attempt_count
            ));
            let mut evidence = EvidenceEntry::diagnostic(
                "project_patch.llm_summary",
                WorkspaceDomainKind::Report,
                DiagnosticSeverity::Info,
                "llm_patch_request",
                format!(
                    "request={} status={} attempts={} repair_attempts={} repair_scope={} candidate_hash={} codes={}",
                    report.request_id,
                    report.final_status,
                    report.attempts.len(),
                    report.repair_attempt_count,
                    report
                        .repair_scope
                        .as_ref()
                        .map(|scope| format!(
                            "{:?}:{:?}->{},changed={:?},rejection={}",
                            scope.status,
                            scope.initial_operation_count,
                            scope.repaired_operation_count,
                            scope.changed_slots,
                            scope.rejection_code.as_deref().unwrap_or("none")
                        ))
                        .unwrap_or_else(|| "none".to_string()),
                    report.candidate_hash.as_deref().unwrap_or("none"),
                    report.diagnostic_codes.join(",")
                ),
            );
            evidence.request_id = Some(report.request_id.clone());
            evidence.stage = Some("llm_project_patch".to_string());
            entry.evidence.push(evidence);
            if report.report_level == LlmPatchReportLevel::Trace {
                for attempt in &report.attempts {
                    let mut trace = EvidenceEntry::diagnostic(
                        format!("project_patch.llm_attempt.{}", attempt.attempt_index),
                        WorkspaceDomainKind::Report,
                        DiagnosticSeverity::Info,
                        "llm_patch_attempt",
                        format!(
                            "kind={} index={} status={:?} latency_ms={} http={} transport_attempts={} context_hash={} schema_hash={}",
                            attempt.attempt_kind,
                            attempt.attempt_index,
                            attempt.status,
                            attempt.latency_ms,
                            attempt.http_status_class.as_deref().unwrap_or("none"),
                            attempt.transport_attempt_count,
                            report.context_hash.as_deref().unwrap_or("none"),
                            report.schema_hash.as_deref().unwrap_or("none")
                        ),
                    );
                    trace.request_id = Some(report.request_id.clone());
                    trace.stage = Some(attempt.attempt_kind.clone());
                    entry.evidence.push(trace);
                }
            }
            if matches!(
                report.final_status.as_str(),
                "context_stale" | "scope_rejected" | "no_progress" | "attempt_limit_reached"
            ) {
                entry.status = ReportStatus::Failed;
            }
        }
        entry
    }
}

impl ReportProvider for SaveReloadRebuildReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "validation.save_reload_rebuild",
            "Save Reload Rebuild Consistency",
            WorkspaceDomainKind::Report,
            "save_reload_rebuild_consistency",
            ReportSourceKind::Artifact,
            &[SAVE_RELOAD_REBUILD_CONSISTENCY_REPORT_SCHEMA_VERSION],
        )
    }

    fn enabled(&self, context: &ReportProviderContext<'_>) -> bool {
        context.session.save_reload_rebuild_report_level != ConsistencyReportLevel::Off
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let Some(report) = context.session.save_reload_rebuild_report.as_ref() else {
            return placeholder(
                "report-save-reload-rebuild-consistency",
                "validation.save_reload_rebuild",
                "Save Reload Rebuild Consistency",
                WorkspaceDomainKind::Report,
                "save_reload_rebuild_consistency",
                "Not run for the active project.",
                vec!["Run the explicit save/reload/rebuild consistency gate.".to_string()],
            );
        };

        save_reload_rebuild_entry(report, context.session.save_reload_rebuild_report_level)
    }
}

impl ReportProvider for DiagnosticsReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "editor.diagnostics",
            "Editor Diagnostics",
            WorkspaceDomainKind::Report,
            "editor_diagnostics",
            ReportSourceKind::InMemory,
            &["editor-diagnostics.v1"],
        )
    }

    fn collect(&self, context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        diagnostics_entry(&context.session.diagnostics)
    }
}

impl ReportProvider for ComplexShooterE2eReportProvider {
    fn descriptor(&self) -> ReportDescriptor {
        descriptor(
            "project_e2e.complex_shooter",
            "Complex Shooter E2E",
            WorkspaceDomainKind::Report,
            "complex_shooter_e2e",
            ReportSourceKind::Placeholder,
            &["complex-shooter-e2e-report.v1"],
        )
    }

    fn collect(&self, _context: &ReportProviderContext<'_>) -> UnifiedReportEntry {
        let mut entry = placeholder(
            "report-complex-shooter-e2e",
            "project_e2e.complex_shooter",
            "Complex Shooter E2E",
            WorkspaceDomainKind::Report,
            "complex_shooter_e2e",
            "Complex shooter E2E evidence is produced by project_e2e_gate, not by the editor panel.",
            vec!["cargo test -p project_e2e_gate unified_report_panel".to_string()],
        );
        entry.status = ReportStatus::Skipped;
        entry
    }
}

impl EditorSession {
    pub fn set_save_reload_rebuild_report_level(&mut self, level: ConsistencyReportLevel) {
        self.save_reload_rebuild_report_level = level;
    }

    pub fn save_reload_rebuild_report_level(&self) -> ConsistencyReportLevel {
        self.save_reload_rebuild_report_level
    }

    pub fn cached_save_reload_rebuild_report(&self) -> Option<&SaveReloadRebuildConsistencyReport> {
        self.save_reload_rebuild_report.as_ref()
    }

    pub(crate) fn reload_save_reload_rebuild_report_cache(&mut self) -> Result<bool, String> {
        self.save_reload_rebuild_report = None;
        let Some(project) = self.active_project_session.as_ref() else {
            return Ok(false);
        };
        let report_path = project
            .project_root
            .join(SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH);
        if !report_path.is_file() {
            return Ok(false);
        }
        let report = crate::read_consistency_report(&report_path)?;
        if report.project_id != project.manifest.project_id {
            return Err(format!(
                "consistency report projectId '{}' does not match active projectId '{}'",
                report.project_id, project.manifest.project_id
            ));
        }
        self.save_reload_rebuild_report = Some(report);
        Ok(true)
    }

    pub(crate) fn select_report_entry(
        &mut self,
        mut transaction: CommandTransaction,
        report_id: String,
    ) -> CommandResult {
        self.selected_report_id = Some(report_id.clone());
        self.push_info(
            &mut transaction,
            "editor.report_panel.selected",
            format!("Selected report entry: {report_id}"),
        );
        self.finish_transaction(transaction, CommandStatus::Committed)
    }

    pub(crate) fn refresh_reports(&mut self, mut transaction: CommandTransaction) -> CommandResult {
        transaction.read_set.push(format!(
            "project.{}",
            SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH
        ));
        transaction.read_set.push(format!(
            "project.{}",
            crate::RELEASE_PACKAGE_REPORT_RELATIVE_PATH
        ));
        let consistency = self.reload_save_reload_rebuild_report_cache();
        let release = self.reload_release_package_report_cache();
        for (kind, result) in [
            ("save/reload/rebuild", consistency),
            ("release package", release),
        ] {
            if let Err(message) = result {
                self.push_warning(
                    &mut transaction,
                    "editor.report_panel.artifact_rejected",
                    message,
                    Some("Regenerate the report for the active project."),
                );
            } else {
                transaction.state_changes.push(crate::StateChangeSummary {
                    kind: "report_panel.cache_refresh".to_string(),
                    path: kind.to_string(),
                    before_summary: None,
                    after_summary: Some("refreshed".to_string()),
                });
            }
        }
        self.push_info(
            &mut transaction,
            "editor.report_panel.refreshed",
            "Unified report panel refreshed from active-project artifact caches.",
        );
        self.finish_transaction(transaction, CommandStatus::Committed)
    }

    pub(crate) fn copy_report_ai_context(
        &mut self,
        mut transaction: CommandTransaction,
        report_id: String,
    ) -> CommandResult {
        self.push_info(
            &mut transaction,
            "editor.report_panel.ai_context",
            format!("AI context is available from ReportPanelModel for report: {report_id}"),
        );
        self.finish_transaction(transaction, CommandStatus::Committed)
    }

    pub(crate) fn open_raw_report(
        &mut self,
        mut transaction: CommandTransaction,
        report_id: String,
    ) -> CommandResult {
        self.push_info(
            &mut transaction,
            "editor.report_panel.open_raw",
            format!("Raw report open requested for report: {report_id}"),
        );
        self.finish_transaction(transaction, CommandStatus::Committed)
    }

    pub(crate) fn reveal_report_path(
        &mut self,
        mut transaction: CommandTransaction,
        report_id: String,
    ) -> CommandResult {
        self.push_info(
            &mut transaction,
            "editor.report_panel.reveal_path",
            format!("Report path reveal requested for report: {report_id}"),
        );
        self.finish_transaction(transaction, CommandStatus::Committed)
    }

    pub(crate) fn open_related_report_artifact(
        &mut self,
        mut transaction: CommandTransaction,
        report_id: String,
        artifact_id: String,
    ) -> CommandResult {
        self.push_info(
            &mut transaction,
            "editor.report_panel.open_artifact",
            format!("Related artifact open requested: report={report_id} artifact={artifact_id}"),
        );
        self.finish_transaction(transaction, CommandStatus::Committed)
    }
}

fn descriptor(
    provider_id: &str,
    label: &str,
    domain: WorkspaceDomainKind,
    kind: &str,
    source_kind: ReportSourceKind,
    schema_versions: &[&str],
) -> ReportDescriptor {
    let mut descriptor = ReportDescriptor::new(provider_id, label, domain, kind, source_kind);
    descriptor.supported_schema_versions = schema_versions
        .iter()
        .map(|version| (*version).to_string())
        .collect();
    descriptor
}

fn placeholder(
    report_id: &str,
    provider_id: &str,
    title: &str,
    domain: WorkspaceDomainKind,
    kind: &str,
    summary: &str,
    next_actions: Vec<String>,
) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(report_id, provider_id, title, domain, kind)
        .with_summary(summary.to_string());
    entry.status = ReportStatus::Empty;
    entry.source_kind = ReportSourceKind::Placeholder;
    entry.next_actions = next_actions;
    entry
}

fn save_reload_rebuild_entry(
    report: &SaveReloadRebuildConsistencyReport,
    level: ConsistencyReportLevel,
) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-save-reload-rebuild-consistency",
        "validation.save_reload_rebuild",
        "Save Reload Rebuild Consistency",
        WorkspaceDomainKind::Report,
        "save_reload_rebuild_consistency",
    )
    .with_summary(format!(
        "status={:?} checkpoints={} comparisons={} mutations={} witnesses={} diagnostics={}",
        report.status,
        report.checkpoints.len(),
        report.comparisons.len(),
        report.mutations.len(),
        report.source_runtime_witnesses.len(),
        report.diagnostics.len()
    ));
    entry.status = match report.status {
        SaveReloadRebuildStatus::NotRun => ReportStatus::Empty,
        SaveReloadRebuildStatus::Passed => ReportStatus::Passed,
        SaveReloadRebuildStatus::Failed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::Artifact;
    entry.report_path = Some(SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH.to_string());
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = if level == ConsistencyReportLevel::Trace {
        report.next_actions.clone()
    } else if report.status == SaveReloadRebuildStatus::Failed {
        vec!["Open Trace evidence and rerun the failing consistency gate.".to_string()]
    } else {
        Vec::new()
    };

    entry.evidence.push(EvidenceEntry::diagnostic(
        "save_reload_rebuild.summary",
        WorkspaceDomainKind::Report,
        DiagnosticSeverity::Info,
        "save_reload_rebuild.summary",
        format!(
            "process_isolated={} comparisons_passed={}/{} mutations_observed={}/{} witnesses_resolved={}/{}",
            report.reopen_mode == "process_isolated",
            report.comparisons.iter().filter(|item| item.equal).count(),
            report.comparisons.len(),
            report.mutations.iter().filter(|item| item.observed).count(),
            report.mutations.len(),
            report
                .source_runtime_witnesses
                .iter()
                .filter(|item| item.resolved)
                .count(),
            report.source_runtime_witnesses.len()
        ),
    ));

    if level == ConsistencyReportLevel::Trace {
        for comparison in &report.comparisons {
            entry.evidence.push(EvidenceEntry::diagnostic(
                format!(
                    "save_reload_rebuild.comparison.{}",
                    comparison.comparison_id
                ),
                WorkspaceDomainKind::Report,
                if comparison.equal {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                "save_reload_rebuild.comparison",
                format!(
                    "{} equal={} left={} right={} detail={}",
                    comparison.comparison_id,
                    comparison.equal,
                    comparison.left,
                    comparison.right,
                    comparison.detail
                ),
            ));
        }
        for mutation in &report.mutations {
            entry.evidence.push(EvidenceEntry::diagnostic(
                format!("save_reload_rebuild.mutation.{}", mutation.mutation_id),
                WorkspaceDomainKind::Report,
                if mutation.observed {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                mutation
                    .diagnostic_code
                    .as_deref()
                    .unwrap_or("save_reload_rebuild.mutation"),
                format!(
                    "{} observed={} expected={}",
                    mutation.mutation_id, mutation.observed, mutation.expected_effect
                ),
            ));
        }
        for process in &report.processes {
            entry.evidence.push(EvidenceEntry::diagnostic(
                format!("save_reload_rebuild.process.{}", process.invocation_id),
                WorkspaceDomainKind::Report,
                if process.status == "passed" {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                "save_reload_rebuild.process",
                format!(
                    "mode={} invocation={} pid={} exit={:?} executable={}",
                    process.mode,
                    process.invocation_id,
                    process.process_id,
                    process.exit_code,
                    process.executable
                ),
            ));
        }
        for witness in &report.source_runtime_witnesses {
            let mut evidence = EvidenceEntry::diagnostic(
                format!(
                    "save_reload_rebuild.witness.{}.{}",
                    witness.domain, witness.object_id
                ),
                WorkspaceDomainKind::Report,
                if witness.resolved {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                "save_reload_rebuild.source_runtime_witness",
                format!(
                    "domain={} object={} source={} build_input={} runtime={} field={}",
                    witness.domain,
                    witness.object_id,
                    witness.source_path,
                    witness.build_input_path,
                    witness.runtime_path,
                    witness.field_path.as_deref().unwrap_or("none")
                ),
            );
            evidence.source_path = Some(witness.source_path.clone());
            entry.evidence.push(evidence);
        }
        entry.artifacts = report
            .artifacts
            .iter()
            .enumerate()
            .map(|(index, path)| {
                artifact(
                    &format!("save-reload-rebuild-artifact-{index}"),
                    "Consistency Gate Artifact",
                    path,
                    "save_reload_rebuild",
                )
            })
            .collect();
    }

    entry.diagnostics = report
        .diagnostics
        .iter()
        .enumerate()
        .map(|(index, diagnostic)| {
            let mut evidence = EvidenceEntry::diagnostic(
                format!("save_reload_rebuild.diagnostic.{index}"),
                WorkspaceDomainKind::Report,
                DiagnosticSeverity::Error,
                diagnostic.code.clone(),
                if level == ConsistencyReportLevel::Trace {
                    diagnostic.message.clone()
                } else {
                    format!(
                        "{} failed{}{}",
                        diagnostic.domain.as_deref().unwrap_or("consistency gate"),
                        diagnostic
                            .object_id
                            .as_deref()
                            .map(|id| format!(" for object {id}"))
                            .unwrap_or_default(),
                        diagnostic
                            .next_action
                            .as_deref()
                            .map(|action| format!("; next action: {action}"))
                            .unwrap_or_default()
                    )
                },
            );
            if level == ConsistencyReportLevel::Trace {
                evidence.source_path = diagnostic.path.clone();
                evidence.suggested_action = diagnostic.next_action.clone();
            }
            evidence
        })
        .collect();
    entry
}

fn build_export_entry(report: &DesktopExportReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-build-export",
        "build.export",
        "Latest Build Export",
        WorkspaceDomainKind::Build,
        "desktop_export",
    )
    .with_summary(format!(
        "status={:?} target={} profile={} runtime_package_status={:?} player_exit={:?}",
        report.status,
        report.target,
        report.profile,
        report.runtime_package_status,
        report.player_exit_code
    ));
    entry.status = match report.status {
        DesktopExportStatus::Success => ReportStatus::Passed,
        DesktopExportStatus::Failed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.source_path = Some(report.project_root.clone());
    entry.report_path = Some(desktop_export_report_path(report));
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = if entry.status == ReportStatus::Failed {
        vec!["inspect_build_report".to_string()]
    } else {
        Vec::new()
    };
    entry.diagnostics = report
        .diagnostics
        .iter()
        .map(desktop_export_diagnostic_evidence)
        .collect();
    entry.artifacts = vec![
        artifact(
            "desktop-export-report",
            "Desktop Export Report",
            entry.report_path.as_deref().unwrap_or_default(),
            "json",
        ),
        artifact(
            "package-manifest",
            "Desktop Package Manifest",
            &report.package_manifest_path,
            "json",
        ),
        artifact(
            "runtime-package-report",
            "RuntimePackage Build Report",
            &report.runtime_package_report_path,
            "json",
        ),
        artifact(
            "windowed-player-report",
            "Windowed Player Report",
            &report.player_report_path,
            "json",
        ),
    ];
    entry
}

fn build_and_run_entry(report: &EditorBuildAndRunReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-build-and-run",
        "build.and_run",
        "Latest Build And Run",
        WorkspaceDomainKind::Build,
        "editor_build_and_run",
    )
    .with_summary(format!(
        "status={:?} profile={} mode={:?} package={} process={:?} child={:?}",
        report.status,
        report.profile_id,
        report.run_mode,
        report
            .desktop_export
            .package_dir
            .as_deref()
            .unwrap_or("none"),
        report.verification.process_exit_code,
        report.verification.child_player_exit_code
    ));
    entry.status = match report.status {
        EditorBuildAndRunStatus::Launched | EditorBuildAndRunStatus::VerificationPassed => {
            ReportStatus::Passed
        }
        EditorBuildAndRunStatus::NotStarted => ReportStatus::Empty,
        EditorBuildAndRunStatus::EnvironmentBlocked => ReportStatus::Skipped,
        EditorBuildAndRunStatus::ExportFailed
        | EditorBuildAndRunStatus::LaunchFailed
        | EditorBuildAndRunStatus::VerificationFailed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.source_path = report.project_root.clone();
    entry.report_path = report.report_path.clone();
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = if entry.status == ReportStatus::Failed {
        vec!["inspect_build_and_run_report".to_string()]
    } else {
        Vec::new()
    };
    entry.diagnostics = report
        .diagnostics
        .iter()
        .map(build_and_run_diagnostic_evidence)
        .collect();
    entry.evidence.push(EvidenceEntry::diagnostic(
        "build_and_run.launch",
        WorkspaceDomainKind::Build,
        DiagnosticSeverity::Info,
        "launch",
        format!(
            "attempted={} started={} pid={:?} exe={} cwd={}",
            report.launch.attempted,
            report.launch.started,
            report.launch.process_id,
            report.launch.executable_path.as_deref().unwrap_or("none"),
            report.launch.working_dir.as_deref().unwrap_or("none")
        ),
    ));
    entry.evidence.push(EvidenceEntry::diagnostic(
        "build_and_run.verification",
        WorkspaceDomainKind::Build,
        DiagnosticSeverity::Info,
        "verification",
        format!(
            "attempted={} status={} frames={:?}",
            report.verification.attempted,
            report.verification.status,
            report.verification.child_frames_completed
        ),
    ));
    entry.artifacts = report
        .artifacts
        .iter()
        .map(|artifact_ref| {
            artifact(
                &artifact_ref.artifact_id,
                &artifact_ref.label,
                &artifact_ref.path,
                &artifact_ref.kind,
            )
        })
        .collect();
    if let Some(path) = &report.verification.verification_report_path {
        entry.artifacts.push(artifact(
            "editor-build-and-run-process-verification",
            "Editor Build And Run Process Verification Summary",
            path,
            "json",
        ));
    }
    if let Some(path) = &report.verification.child_report_path {
        entry.artifacts.push(artifact(
            "windowed-player-child-report",
            "Windowed Player Child Report",
            path,
            "json",
        ));
    }
    entry
}

fn release_package_entry(report: &ReleasePackageReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-release-package",
        "build.release_package",
        "Latest Release Package",
        WorkspaceDomainKind::Build,
        "release_package",
    )
    .with_summary(format!(
        "product={} version={} entrypoint={} payload_hash={} status={:?} next_action={}",
        report.display_name,
        report.display_version,
        report.entrypoint,
        report.release_payload_hash,
        report.status,
        report.next_action
    ));
    entry.status = match report.status {
        ReleasePackageStatus::Success => ReportStatus::Passed,
        ReleasePackageStatus::Failed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::Artifact;
    entry.schema_version = Some(report.schema_version.clone());
    if !report.next_action.is_empty() {
        entry.next_actions.push(report.next_action.clone());
    }
    entry.diagnostics = report
        .diagnostics
        .iter()
        .enumerate()
        .map(|(index, diagnostic)| {
            release_package_diagnostic_evidence(
                index,
                diagnostic,
                report.report_level == ReleasePackageReportLevel::Trace,
            )
        })
        .collect();
    if report.report_level == ReleasePackageReportLevel::Trace {
        entry.source_path = Some(report.output_dir.clone());
        entry.report_path = (!report.report_path.is_empty()).then(|| report.report_path.clone());
        entry.evidence.push(EvidenceEntry::diagnostic(
            "release_package.layout",
            WorkspaceDomainKind::Build,
            DiagnosticSeverity::Info,
            "release_layout",
            format!(
                "output={} manifest={} runtime_package={} files={}",
                report.output_dir,
                report.manifest_path,
                report.runtime_package,
                report.payload_file_count
            ),
        ));
        entry.evidence.push(EvidenceEntry::diagnostic(
            "release_package.hash",
            WorkspaceDomainKind::Build,
            DiagnosticSeverity::Info,
            "release_payload_hash",
            format!(
                "runtime_content_hash={} release_payload_hash={}",
                report.runtime_content_hash, report.release_payload_hash
            ),
        ));
        if let Some(readback) = &report.resource_readback {
            entry.evidence.push(EvidenceEntry::diagnostic(
                "release_package.resource_readback",
                WorkspaceDomainKind::Build,
                DiagnosticSeverity::Info,
                "release_resource_readback",
                serde_json::to_string(readback)
                    .unwrap_or_else(|_| "resource readback unavailable".to_string()),
            ));
        }
        entry.evidence.push(EvidenceEntry::diagnostic(
            "release_package.process_verification",
            WorkspaceDomainKind::Build,
            if report.verification.explicit_process_passed {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            "release_process_verification",
            format!(
                "status={} process={:?} child={:?} frames={:?} report={}",
                report.verification.explicit_process_status,
                report.verification.process_exit_code,
                report.verification.child_player_exit_code,
                report.verification.child_frames_completed,
                report
                    .verification
                    .process_report_path
                    .as_deref()
                    .unwrap_or("none")
            ),
        ));
        entry.artifacts = vec![
            artifact(
                "release-package-manifest",
                "Release Package Manifest",
                &report.manifest_path,
                "json",
            ),
            artifact(
                "release-package-entrypoint",
                "Release Entrypoint",
                &Path::new(&report.output_dir)
                    .join(&report.entrypoint)
                    .display()
                    .to_string(),
                "executable",
            ),
        ];
        if let Some(path) = &report.verification.process_report_path {
            entry.artifacts.push(artifact(
                "release-process-verification",
                "Release Process Verification",
                path,
                "json",
            ));
        }
    }
    entry
}

fn release_package_diagnostic_evidence(
    index: usize,
    diagnostic: &ReleasePackageDiagnostic,
    trace: bool,
) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("release_package.diagnostic.{index}"),
        WorkspaceDomainKind::Build,
        DiagnosticSeverity::Error,
        diagnostic.code.clone(),
        if trace {
            diagnostic.message.clone()
        } else {
            format!(
                "{}; next action: {}",
                diagnostic.stage, diagnostic.next_action
            )
        },
    );
    evidence.stage = Some(diagnostic.stage.clone());
    evidence.suggested_action = Some(diagnostic.next_action.clone());
    if trace {
        evidence.source_path = diagnostic.path.clone();
    }
    evidence
}

fn desktop_export_report_path(report: &DesktopExportReport) -> String {
    Path::new(&report.package_dir)
        .join("reports")
        .join("desktop-export-report.json")
        .display()
        .to_string()
}

fn play_session_entry(report: &PlaySessionReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-play-runtime",
        "play.runtime",
        "Latest Play Session",
        WorkspaceDomainKind::Play,
        "play_session",
    )
    .with_summary(format!(
        "session={} state={:?} mode={:?} process_status={} exit={:?}",
        report.session_id,
        report.state,
        report.mode,
        report.process_summary.status,
        report.process_summary.exit_code
    ));
    entry.status = match report.state {
        PlaySessionState::Completed => ReportStatus::Passed,
        PlaySessionState::Failed => ReportStatus::Failed,
        PlaySessionState::Idle => ReportStatus::Empty,
        _ => ReportStatus::Partial,
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.source_path = Some(report.request_summary.runtime_package_path.clone());
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = if entry.status == ReportStatus::Failed {
        vec!["inspect_runtime_trace".to_string()]
    } else {
        Vec::new()
    };
    entry.diagnostics = report
        .diagnostics
        .iter()
        .map(play_session_diagnostic_evidence)
        .collect();
    entry.artifacts.push(artifact(
        "runtime-package",
        "Runtime Package",
        &report.request_summary.runtime_package_path,
        "runtime_package",
    ));
    if let Some(path) = &report.preview_package_report_path {
        entry.artifacts.push(artifact(
            "editor-play-preview-package-report",
            "Editor Play Preview Package Report",
            path,
            "json",
        ));
    }
    if let Some(path) = &report.game_view_present_report_path {
        entry.artifacts.push(artifact(
            "editor-gameview-present-report",
            "Editor GameView Present Report",
            path,
            "json",
        ));
    }
    entry
}

fn game_view_present_entry(report: &GameViewPresentReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-play-gameview-present",
        "play.game_view_present",
        "Latest Editor GameView Present",
        WorkspaceDomainKind::Play,
        "editor_gameview_present",
    )
    .with_summary(format!(
        "status={:?} control={:?}/{} frames={} advanced={} step={} target={} descriptor={} gpu={} input={} aui={} stop={}",
        report.status,
        report.control_state,
        report.control_command,
        report.frame_count,
        report.runtime_advanced,
        report.step_count,
        report.target_runtime_domain,
        report.texture_descriptor_status,
        report.gpu_present_status,
        report.input_bridge_status,
        report.aui_present_status,
        report.stop_status
    ));
    entry.status = match report.status {
        GameViewPresentStatus::Success | GameViewPresentStatus::Stopped => ReportStatus::Passed,
        GameViewPresentStatus::Failed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.source_path = Some(report.runtime_package_path.clone());
    entry.report_path = report.report_path.clone();
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = report.next_actions.clone();
    entry.diagnostics = report
        .diagnostics
        .iter()
        .map(game_view_present_diagnostic_evidence)
        .collect();
    entry.evidence.push(EvidenceEntry::diagnostic(
        "gameview_present.play_control",
        WorkspaceDomainKind::Play,
        DiagnosticSeverity::Info,
        "editor_gameview_play_control",
        format!(
            "control_state={:?} command={} runtime_advanced={} paused_last_frame_reused={} step_count={} target_runtime_domain={}",
            report.control_state,
            report.control_command,
            report.runtime_advanced,
            report.paused_last_frame_reused,
            report.step_count,
            report.target_runtime_domain
        ),
    ));
    entry.evidence.push(EvidenceEntry::diagnostic(
        "gameview_present.frame_descriptor",
        WorkspaceDomainKind::Play,
        DiagnosticSeverity::Info,
        "viewport_texture_descriptor",
        format!(
            "frame_count={} last_hash={} texture_status={}",
            report.frame_count,
            report.last_frame_hash.as_deref().unwrap_or("none"),
            report.texture_descriptor_status
        ),
    ));
    entry.evidence.push(EvidenceEntry::diagnostic(
        "gameview_present.gpu_texture",
        WorkspaceDomainKind::Play,
        DiagnosticSeverity::Info,
        "gpu_texture_present",
        format!(
            "gpu_present_status={} shared_gpu_context_status={} report={}",
            report.gpu_present_status,
            report.shared_gpu_context_status,
            report.gpu_present_report_path.as_deref().unwrap_or("none")
        ),
    ));
    entry.evidence.push(EvidenceEntry::diagnostic(
        "gameview_present.input_deferred",
        WorkspaceDomainKind::Play,
        DiagnosticSeverity::Info,
        "gameview_input_deferred",
        format!("input_bridge_status={}", report.input_bridge_status),
    ));
    if let Some(path) = &report.report_path {
        entry.artifacts.push(artifact(
            "editor-gameview-present-report",
            "Editor GameView Present Report",
            path,
            "json",
        ));
    }
    if let Some(path) = &report.preview_package_report_path {
        entry.artifacts.push(artifact(
            "editor-play-preview-package-report",
            "Editor Play Preview Package Report",
            path,
            "json",
        ));
    }
    entry
}

fn preview_package_entry(report: &EditorPlayPreviewPackageReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-play-preview-package",
        "play.preview_package",
        "Latest Editor Play Preview Package",
        WorkspaceDomainKind::Play,
        "editor_play_preview_package",
    )
    .with_summary(format!(
        "status={:?} cache={} dirty={:?} duration_ms={} runtime_package={}",
        report.status,
        report.cache_status.as_report_str(),
        report.dirty_domain_labels(),
        report.duration_total_ms,
        report.runtime_package_dir.as_deref().unwrap_or("none")
    ));
    entry.status = match report.status {
        EditorPreviewPackageStatus::Success => ReportStatus::Passed,
        EditorPreviewPackageStatus::Failed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.source_path = Some(report.project_root.clone());
    entry.report_path = report.report_path.clone();
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = report.next_actions.clone();
    entry.diagnostics = report
        .diagnostics
        .iter()
        .map(preview_package_diagnostic_evidence)
        .collect();
    entry.evidence = report
        .stage_reports
        .iter()
        .map(|stage| {
            let mut evidence = EvidenceEntry::diagnostic(
                format!("preview_package.stage.{}", stage.stage_id),
                WorkspaceDomainKind::Play,
                match stage.status {
                    crate::EditorPreviewPackageStageStatus::Failed => DiagnosticSeverity::Error,
                    crate::EditorPreviewPackageStageStatus::Skipped => DiagnosticSeverity::Info,
                    crate::EditorPreviewPackageStageStatus::Success => DiagnosticSeverity::Info,
                },
                stage.stage_id.clone(),
                format!(
                    "status={:?} skipped={} duration_ms={} cache={:?} dirty={:?}",
                    stage.status,
                    stage.skipped,
                    stage.duration_ms,
                    stage.cache_status,
                    stage
                        .dirty_domains
                        .iter()
                        .map(|domain| domain.as_report_str())
                        .collect::<Vec<_>>()
                ),
            );
            evidence.stage = Some(stage.stage_id.clone());
            evidence
        })
        .collect();
    if let Some(path) = &report.report_path {
        entry.artifacts.push(artifact(
            "editor-play-preview-package-report",
            "Editor Play Preview Package Report",
            path,
            "json",
        ));
    }
    if let Some(path) = &report.runtime_package_dir {
        entry.artifacts.push(artifact(
            "preview-runtime-package",
            "Preview RuntimePackage",
            path,
            "runtime_package",
        ));
    }
    if let Some(path) = &report.runtime_package_build_report_path {
        entry.artifacts.push(artifact(
            "runtime-package-build-report",
            "RuntimePackage Build Report",
            path,
            "json",
        ));
    }
    if let Some(path) = &report.runtime_package_validation_report_path {
        entry.artifacts.push(artifact(
            "runtime-package-validation-report",
            "RuntimePackage Validation Report",
            path,
            "json",
        ));
    }
    if let Some(path) = &report.runtime_package_diff_report_path {
        entry.artifacts.push(artifact(
            "runtime-package-diff-report",
            "RuntimePackage Diff Report",
            path,
            "json",
        ));
    }
    entry
}

fn prefab_authoring_entry(report: &PrefabAuthoringReport) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-prefab-authoring",
        "authoring.prefab",
        "Prefab Authoring",
        WorkspaceDomainKind::Prefab,
        "prefab_authoring",
    )
    .with_summary(format!(
        "status={:?} assets={} instances={} overrides={} active_stage={}",
        report.status,
        report.prefab_assets_count,
        report.prefab_instances_count,
        report.overrides_count,
        report.active_stage_id.as_deref().unwrap_or("none")
    ));
    entry.status = match report.status {
        PrefabAuthoringStatus::Ready | PrefabAuthoringStatus::Saved => ReportStatus::Passed,
        PrefabAuthoringStatus::Dirty => ReportStatus::Partial,
        PrefabAuthoringStatus::Invalid | PrefabAuthoringStatus::Failed => ReportStatus::Failed,
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.source_path = report.active_prefab_path.clone();
    entry.schema_version = Some(report.schema_version.clone());
    entry.next_actions = report.next_actions.clone();
    entry.diagnostics = report
        .diagnostics
        .iter()
        .map(prefab_diagnostic_evidence)
        .collect();
    for path in &report.created_prefab_paths {
        entry
            .artifacts
            .push(artifact(path, "Created Prefab", path, "prefab"));
    }
    entry
}

fn diagnostics_entry(diagnostics: &[EditorDiagnostic]) -> UnifiedReportEntry {
    let mut entry = UnifiedReportEntry::new(
        "report-editor-diagnostics",
        "editor.diagnostics",
        "Editor Diagnostics",
        WorkspaceDomainKind::Report,
        "editor_diagnostics",
    )
    .with_summary(format!("editor_diagnostic_count={}", diagnostics.len()));
    entry.status = if diagnostics.is_empty() {
        ReportStatus::Empty
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        ReportStatus::Failed
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
    {
        ReportStatus::Partial
    } else {
        ReportStatus::Passed
    };
    entry.source_kind = ReportSourceKind::InMemory;
    entry.diagnostics = diagnostics
        .iter()
        .enumerate()
        .map(|(index, diagnostic)| {
            let mut evidence = EvidenceEntry::diagnostic(
                format!("editor_diagnostics.{}", index + 1),
                WorkspaceDomainKind::Report,
                diagnostic.severity,
                diagnostic.code.clone(),
                diagnostic.message.clone(),
            );
            evidence.source_path = diagnostic.path.clone();
            evidence.entity_id = diagnostic.entity_id.clone();
            evidence.command_id = diagnostic.command_id.clone();
            evidence.request_id = diagnostic.request_id.clone();
            evidence.trace_entry_id = diagnostic.trace_entry_id.clone();
            evidence.suggested_action = diagnostic.suggested_action.clone();
            evidence.raw_payload_summary = Some(format!("{:?}", diagnostic.source));
            evidence
        })
        .collect();
    entry
}

fn desktop_export_diagnostic_evidence(diagnostic: &DesktopExportDiagnostic) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("desktop_export.{}", diagnostic.code),
        WorkspaceDomainKind::Build,
        severity_from_desktop_export(diagnostic.severity),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.source_path = diagnostic.path.clone();
    evidence.suggested_action = diagnostic.suggestion.clone();
    evidence
}

fn build_and_run_diagnostic_evidence(diagnostic: &EditorBuildAndRunDiagnostic) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("build_and_run.{}", diagnostic.code),
        WorkspaceDomainKind::Build,
        severity_from_build_and_run(diagnostic.severity),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.stage = Some(diagnostic.stage.clone());
    evidence.source_path = diagnostic.path.clone();
    evidence.suggested_action = diagnostic.next_action.clone();
    evidence
}

fn play_session_diagnostic_evidence(diagnostic: &PlaySessionDiagnostic) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("play_session.{}", diagnostic.code),
        WorkspaceDomainKind::Play,
        severity_from_play_session(diagnostic.severity),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.stage = Some(diagnostic.layer.clone());
    evidence.source_path = diagnostic.path.clone();
    evidence
}

fn preview_package_diagnostic_evidence(
    diagnostic: &EditorPreviewPackageDiagnostic,
) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("preview_package.{}", diagnostic.code),
        WorkspaceDomainKind::Play,
        severity_from_preview_package(diagnostic.severity),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.source_path = diagnostic.path.clone();
    evidence.suggested_action = diagnostic.suggestion.clone();
    evidence
}

fn game_view_present_diagnostic_evidence(diagnostic: &GameViewPresentDiagnostic) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("gameview_present.{}", diagnostic.code),
        WorkspaceDomainKind::Play,
        severity_from_game_view_present(diagnostic.severity),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.stage = Some(diagnostic.layer.clone());
    evidence.source_path = diagnostic.path.clone();
    evidence
}

fn prefab_diagnostic_evidence(diagnostic: &PrefabDiagnostic) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("prefab_authoring.{}", diagnostic.code.as_str()),
        WorkspaceDomainKind::Prefab,
        severity_from_prefab(diagnostic.severity.clone()),
        diagnostic.code.as_str().to_string(),
        diagnostic.message.clone(),
    );
    evidence.source_path = diagnostic.prefab_ref.clone();
    evidence.entity_id = diagnostic.instance_id.clone();
    evidence.node_id = diagnostic.source_entity_id.clone();
    evidence.raw_payload_summary = diagnostic.field_path.clone();
    evidence
}

fn patch_diagnostic_evidence(diagnostic: &PatchDiagnostic, prefix: &str) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("{prefix}.{}", diagnostic.code),
        WorkspaceDomainKind::Report,
        severity_from_patch(diagnostic.severity.clone()),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.command_id = diagnostic.operation_id.clone();
    evidence.source_path = diagnostic.target.clone();
    evidence
}

fn artifact(id: &str, label: &str, path: &str, kind: &str) -> ReportArtifactRef {
    ReportArtifactRef {
        artifact_id: id.to_string(),
        label: label.to_string(),
        path: path.to_string(),
        kind: kind.to_string(),
    }
}

fn status_from_workspace_domain(status: WorkspaceDomainStatus) -> ReportStatus {
    match status {
        WorkspaceDomainStatus::Ready => ReportStatus::Passed,
        WorkspaceDomainStatus::Dirty | WorkspaceDomainStatus::Warning => ReportStatus::Partial,
        WorkspaceDomainStatus::Error => ReportStatus::Failed,
        WorkspaceDomainStatus::Empty => ReportStatus::Empty,
        WorkspaceDomainStatus::NotConfigured => ReportStatus::Skipped,
    }
}

fn severity_from_desktop_export(severity: DesktopExportDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        DesktopExportDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        DesktopExportDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        DesktopExportDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_build_and_run(
    severity: EditorBuildAndRunDiagnosticSeverity,
) -> DiagnosticSeverity {
    match severity {
        EditorBuildAndRunDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        EditorBuildAndRunDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        EditorBuildAndRunDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_play_session(severity: PlaySessionDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        PlaySessionDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        PlaySessionDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        PlaySessionDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_preview_package(
    severity: EditorPreviewPackageDiagnosticSeverity,
) -> DiagnosticSeverity {
    match severity {
        EditorPreviewPackageDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        EditorPreviewPackageDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        EditorPreviewPackageDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_game_view_present(
    severity: GameViewPresentDiagnosticSeverity,
) -> DiagnosticSeverity {
    match severity {
        GameViewPresentDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        GameViewPresentDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        GameViewPresentDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_prefab(severity: PrefabDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        PrefabDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        PrefabDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        PrefabDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn asset_browser_diagnostic_evidence(
    diagnostic: &editor_ui_model::AssetBrowserDiagnostic,
) -> EvidenceEntry {
    let mut evidence = EvidenceEntry::diagnostic(
        format!("asset_browser.diagnostic.{}", diagnostic.code),
        WorkspaceDomainKind::Asset,
        match diagnostic.severity {
            editor_ui_model::AssetBrowserDiagnosticSeverity::Info => DiagnosticSeverity::Info,
            editor_ui_model::AssetBrowserDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            editor_ui_model::AssetBrowserDiagnosticSeverity::Error => DiagnosticSeverity::Error,
        },
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    );
    evidence.stage = Some("diagnostic".to_string());
    evidence.source_path = diagnostic.path.clone();
    evidence.suggested_action = Some("refresh_or_fix_asset_source".to_string());
    evidence.next_actions = vec!["refresh_or_fix_asset_source".to_string()];
    evidence
}

fn severity_from_rule(
    severity: editor_ui_model::RuleAuthoringDiagnosticSeverity,
) -> DiagnosticSeverity {
    match severity {
        editor_ui_model::RuleAuthoringDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        editor_ui_model::RuleAuthoringDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        editor_ui_model::RuleAuthoringDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_patch(severity: PatchDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        PatchDiagnosticSeverity::Info => DiagnosticSeverity::Info,
        PatchDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
        PatchDiagnosticSeverity::Error => DiagnosticSeverity::Error,
    }
}

fn severity_from_missing_gap(severity: MissingOperationSeverity) -> DiagnosticSeverity {
    match severity {
        MissingOperationSeverity::Info => DiagnosticSeverity::Info,
        MissingOperationSeverity::Warning => DiagnosticSeverity::Warning,
        MissingOperationSeverity::Error | MissingOperationSeverity::Critical => {
            DiagnosticSeverity::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EditorSession;
    use editor_ui_model::{
        AuthoringWorkflowModel, DiagnosticSource, ManualWalkthroughCoverageReport,
        ProjectAuthoringWorkspaceModel, UiCommandPayload,
    };

    fn unique_report_project_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "aife-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn consistency_report_fixture(
        project_id: &str,
        root: &Path,
    ) -> SaveReloadRebuildConsistencyReport {
        let mut report =
            SaveReloadRebuildConsistencyReport::new(project_id, ConsistencyReportLevel::Trace);
        report.comparisons.push(crate::ConsistencyComparison {
            comparison_id: "runtime_content_hash".to_string(),
            left: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            right: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            equal: true,
            detail: "deterministic runtime content".to_string(),
        });
        report.processes.push(crate::ConsistencyProcessEvidence {
            mode: "author-save-child".to_string(),
            invocation_id: "author-save-1".to_string(),
            executable: root.join("project_e2e_gate.exe").display().to_string(),
            process_id: 42,
            exit_code: Some(0),
            status: "passed".to_string(),
        });
        report
            .artifacts
            .push(root.join("RuntimePackage-A").display().to_string());
        report.recompute_status();
        report
    }

    fn report_panel_for(session: &EditorSession) -> ReportPanelModel {
        let workspace = ProjectAuthoringWorkspaceModel::empty();
        let workflow = AuthoringWorkflowModel::empty();
        let manual = ManualWalkthroughCoverageReport::from_operations(
            None,
            "save-reload-report-panel-test",
            Vec::new(),
            Vec::new(),
        );
        ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        )
    }

    #[test]
    fn registry_builds_unified_report_panel_from_static_providers() {
        let session = EditorSession::new();
        let workspace = ProjectAuthoringWorkspaceModel::empty();
        let workflow = AuthoringWorkflowModel::empty();
        let manual = ManualWalkthroughCoverageReport::from_operations(
            None,
            "report-panel-test",
            Vec::new(),
            Vec::new(),
        );
        let context = ReportProviderContext {
            session: &session,
            workspace: &workspace,
            authoring_workflow: &workflow,
            manual_walkthrough_report: &manual,
        };

        let panel = ReportRegistry::standard().build_model(&context, None);

        assert!(panel.registry.provider_count >= 9);
        assert!(panel
            .registry
            .descriptors
            .iter()
            .any(|descriptor| descriptor.provider_id == "play.preview_package"));
        assert!(panel
            .registry
            .descriptors
            .iter()
            .any(|descriptor| descriptor.provider_id == "build.export"));
        assert!(panel
            .registry
            .descriptors
            .iter()
            .any(|descriptor| descriptor.provider_id == "build.and_run"));
        assert!(panel
            .registry
            .descriptors
            .iter()
            .any(|descriptor| descriptor.provider_id == "authoring.asset_browser"));
        assert!(panel
            .reports
            .iter()
            .any(|report| report.provider_id == "project_e2e.complex_shooter"
                && report.status == ReportStatus::Skipped));
        assert!(panel
            .registry
            .descriptors
            .iter()
            .any(|descriptor| descriptor.provider_id == "validation.save_reload_rebuild"));
        assert!(panel
            .registry
            .descriptors
            .iter()
            .any(
                |descriptor| descriptor.provider_id == "quality.architecture"
                    && descriptor
                        .supported_schema_versions
                        .contains(&"quality-gate-report.v2".to_string())
            ));
        assert!(panel.registry.descriptors.iter().any(|descriptor| {
            descriptor.provider_id == "editor.ui_reachability"
                && descriptor
                    .supported_schema_versions
                    .contains(&"editor-ui-reachability-report.v1".to_string())
        }));
    }

    #[test]
    fn save_reload_rebuild_provider_uses_project_scoped_cache_and_report_levels() {
        let root = unique_report_project_root("save-reload-cache");
        let mut creator = EditorSession::new();
        let created =
            creator.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
                path: root.display().to_string(),
                name: "ConsistencyReport".to_string(),
            }));
        assert_eq!(created.status, CommandStatus::Committed);
        let project_id = creator
            .active_project_session
            .as_ref()
            .unwrap()
            .manifest
            .project_id
            .clone();
        let report_path = root.join(SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH);
        std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        let report = consistency_report_fixture(&project_id, &root);
        crate::write_consistency_report_atomic(&report_path, &report).unwrap();

        let mut session = EditorSession::new();
        let opened =
            session.execute_command(crate::command_for_test(UiCommandPayload::OpenProject {
                path: root.display().to_string(),
            }));
        assert_eq!(opened.status, CommandStatus::Committed);
        assert!(session.cached_save_reload_rebuild_report().is_some());

        std::fs::remove_file(&report_path).unwrap();
        let summary = report_panel_for(&session);
        let summary_entry = summary
            .reports
            .iter()
            .find(|entry| entry.provider_id == "validation.save_reload_rebuild")
            .unwrap();
        assert_eq!(summary_entry.status, ReportStatus::Passed);
        assert_eq!(
            summary_entry.report_path.as_deref(),
            Some(SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH)
        );
        assert!(summary_entry.artifacts.is_empty());
        assert!(summary_entry
            .evidence
            .iter()
            .chain(summary_entry.diagnostics.iter())
            .all(|evidence| {
                !evidence.message.contains(&root.display().to_string())
                    && !evidence.message.contains("sha256:")
                    && !evidence.message.contains("author-save-child")
            }));

        session.set_save_reload_rebuild_report_level(ConsistencyReportLevel::Trace);
        let trace = report_panel_for(&session);
        let trace_entry = trace
            .reports
            .iter()
            .find(|entry| entry.provider_id == "validation.save_reload_rebuild")
            .unwrap();
        assert!(trace_entry
            .evidence
            .iter()
            .any(|evidence| evidence.message.contains("sha256:")));
        assert!(trace_entry
            .evidence
            .iter()
            .any(|evidence| evidence.message.contains("author-save-child")));
        assert!(trace_entry.artifacts.iter().any(|artifact| {
            artifact.path == root.join("RuntimePackage-A").display().to_string()
        }));

        session.set_save_reload_rebuild_report_level(ConsistencyReportLevel::Off);
        let off = report_panel_for(&session);
        assert!(!off
            .reports
            .iter()
            .any(|entry| entry.provider_id == "validation.save_reload_rebuild"));

        session.set_save_reload_rebuild_report_level(ConsistencyReportLevel::Summary);
        let refreshed =
            session.execute_command(crate::command_for_test(UiCommandPayload::RefreshReports));
        assert_eq!(refreshed.status, CommandStatus::Committed);
        assert!(session.cached_save_reload_rebuild_report().is_none());
        let not_run = report_panel_for(&session);
        assert!(not_run.reports.iter().any(|entry| {
            entry.provider_id == "validation.save_reload_rebuild"
                && entry.status == ReportStatus::Empty
        }));
    }

    #[test]
    fn save_reload_rebuild_cache_rejects_another_projects_artifact() {
        let first_root = unique_report_project_root("save-reload-first");
        let second_root = unique_report_project_root("save-reload-second");
        let mut session = EditorSession::new();
        session.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
            path: first_root.display().to_string(),
            name: "First".to_string(),
        }));
        let first_project_id = session
            .active_project_session
            .as_ref()
            .unwrap()
            .manifest
            .project_id
            .clone();
        session.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
            path: second_root.display().to_string(),
            name: "Second".to_string(),
        }));
        let report_path = second_root.join(SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH);
        std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        crate::write_consistency_report_atomic(
            &report_path,
            &consistency_report_fixture(&first_project_id, &second_root),
        )
        .unwrap();

        let mut reopened = EditorSession::new();
        let result =
            reopened.execute_command(crate::command_for_test(UiCommandPayload::OpenProject {
                path: second_root.display().to_string(),
            }));
        assert_eq!(result.status, CommandStatus::Committed);
        assert!(reopened.cached_save_reload_rebuild_report().is_none());
    }

    #[test]
    fn diagnostics_provider_preserves_ai_actionable_evidence() {
        let mut session = EditorSession::new();
        session.diagnostics.push(EditorDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: "editor.test.failure".to_string(),
            message: "Test diagnostic".to_string(),
            source: DiagnosticSource::EditorCore,
            command_id: Some("test_command".to_string()),
            request_id: Some("request-test".to_string()),
            path: Some("Scenes/Main.scene.json".to_string()),
            entity_id: Some("entity-player".to_string()),
            trace_entry_id: None,
            suggested_action: Some("fix_test_diagnostic".to_string()),
        });
        let workspace = ProjectAuthoringWorkspaceModel::empty();
        let workflow = AuthoringWorkflowModel::empty();
        let manual = ManualWalkthroughCoverageReport::from_operations(
            None,
            "report-panel-test",
            Vec::new(),
            Vec::new(),
        );
        let context = ReportProviderContext {
            session: &session,
            workspace: &workspace,
            authoring_workflow: &workflow,
            manual_walkthrough_report: &manual,
        };

        let panel = ReportRegistry::standard().build_model(&context, None);

        let diagnostics = panel
            .reports
            .iter()
            .find(|report| report.provider_id == "editor.diagnostics")
            .expect("diagnostics report should be registered");
        assert_eq!(diagnostics.status, ReportStatus::Failed);
        assert!(diagnostics.ai_context.top_diagnostics.iter().any(|line| {
            line.contains("editor.test.failure") && line.contains("Test diagnostic")
        }));
    }

    #[test]
    fn input_mapping_provider_honors_off_summary_and_trace_levels() {
        let mapping = engine_input::InputMappingAsset::gameplay_default();
        let binding_count = mapping.bindings.len();
        let mut session = EditorSession::new();
        let root = std::env::temp_dir().join(format!(
            "input-mapping-report-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        session.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "InputReport".to_string(),
        }));
        crate::InputMappingAuthoringService::save(&root, "Input/input.default.json", &mapping)
            .unwrap();
        let mut editor_state = crate::InputMappingAuthoringService::open_editor_state(
            &root,
            "Input/input.default.json",
        )
        .unwrap();
        editor_state.selected_action_id = Some("action.fire".to_string());
        editor_state.selected_binding_id = Some(mapping.bindings[0].binding_id.clone());
        editor_state.dirty = true;
        session.selected_project_browser_path = Some("Input/input.default.json".to_string());
        session.input_mapping_editor_state = Some(editor_state);
        let workspace = ProjectAuthoringWorkspaceModel::empty();
        let workflow = AuthoringWorkflowModel::empty();
        let manual = ManualWalkthroughCoverageReport::from_operations(
            None,
            "input-report-level",
            Vec::new(),
            Vec::new(),
        );

        let summary = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: &session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        );
        let summary_entry = summary
            .reports
            .iter()
            .find(|report| report.provider_id == "authoring.input_mapping")
            .unwrap();
        assert_eq!(summary_entry.evidence.len(), 1);

        session
            .input_mapping_editor_state
            .as_mut()
            .unwrap()
            .report_level = editor_ui_model::InputMappingReportLevel::Trace;
        let trace = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: &session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        );
        let trace_entry = trace
            .reports
            .iter()
            .find(|report| report.provider_id == "authoring.input_mapping")
            .unwrap();
        assert_eq!(trace_entry.evidence.len(), binding_count + 1);

        session
            .input_mapping_editor_state
            .as_mut()
            .unwrap()
            .report_level = editor_ui_model::InputMappingReportLevel::Off;
        let off = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: &session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        );
        assert!(!off
            .reports
            .iter()
            .any(|report| report.provider_id == "authoring.input_mapping"));
        assert!(off
            .registry
            .descriptors
            .iter()
            .find(|descriptor| descriptor.provider_id == "authoring.input_mapping")
            .is_some_and(|descriptor| !descriptor.enabled));
    }

    #[test]
    fn asset_browser_provider_honors_off_summary_and_trace_levels() {
        let mut session = EditorSession::new();
        let root = std::env::temp_dir().join(format!(
            "asset-browser-report-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let create =
            session.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
                path: root.display().to_string(),
                name: "AssetBrowserReport".to_string(),
            }));
        assert_eq!(create.status, CommandStatus::Committed);
        let workspace = ProjectAuthoringWorkspaceModel::empty();
        let workflow = AuthoringWorkflowModel::empty();
        let manual = ManualWalkthroughCoverageReport::from_operations(
            None,
            "asset-browser-report-level",
            Vec::new(),
            Vec::new(),
        );

        let summary = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: &session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        );
        let summary_entry = summary
            .reports
            .iter()
            .find(|report| report.provider_id == "authoring.asset_browser")
            .expect("Asset Browser Summary report");
        assert_eq!(
            summary_entry.schema_version.as_deref(),
            Some(ASSET_BROWSER_NATIVE_PRODUCTIZATION_REPORT_SCHEMA_VERSION)
        );
        assert_eq!(summary_entry.evidence.len(), 1);
        assert!(!summary_entry
            .evidence
            .iter()
            .any(|evidence| evidence.stage.as_deref() == Some("identity")));

        session.set_asset_browser_report_level(AssetBrowserReportLevel::Trace);
        let trace = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: &session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        );
        let trace_entry = trace
            .reports
            .iter()
            .find(|report| report.provider_id == "authoring.asset_browser")
            .expect("Asset Browser Trace report");
        assert!(trace_entry.evidence.len() > summary_entry.evidence.len());
        assert!(trace_entry
            .evidence
            .iter()
            .any(|evidence| evidence.stage.as_deref() == Some("index")));
        assert!(trace_entry
            .evidence
            .iter()
            .any(|evidence| evidence.stage.as_deref() == Some("query")));
        assert!(trace_entry
            .evidence
            .iter()
            .any(|evidence| evidence.stage.as_deref() == Some("identity")));

        session.set_asset_browser_report_level(AssetBrowserReportLevel::Off);
        let off = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: &session,
                workspace: &workspace,
                authoring_workflow: &workflow,
                manual_walkthrough_report: &manual,
            },
            None,
        );
        assert!(!off
            .reports
            .iter()
            .any(|report| report.provider_id == "authoring.asset_browser"));
        assert!(off
            .registry
            .descriptors
            .iter()
            .find(|descriptor| descriptor.provider_id == "authoring.asset_browser")
            .is_some_and(|descriptor| !descriptor.enabled));
    }

    #[test]
    fn release_package_provider_honors_off_summary_trace_and_active_project_cache() {
        let mut session = EditorSession::new();
        let mut report = release_report_fixture(ReleasePackageReportLevel::Summary);
        session.last_release_package_report = Some(report.clone());

        let summary = report_panel_for(&session);
        let summary_entry = summary
            .reports
            .iter()
            .find(|entry| entry.provider_id == "build.release_package")
            .expect("release Summary provider");
        assert!(summary_entry.summary.contains("product=Complex Shooter"));
        assert!(summary_entry
            .summary
            .contains("entrypoint=ComplexShooter.exe"));
        assert!(!summary_entry.summary.contains("C:\\owned\\release"));
        assert!(summary_entry.source_path.is_none());
        assert!(summary_entry.report_path.is_none());
        assert!(summary_entry.evidence.is_empty());
        assert!(summary_entry.artifacts.is_empty());

        report.report_level = ReleasePackageReportLevel::Trace;
        session.last_release_package_report = Some(report.clone());
        let trace = report_panel_for(&session);
        let trace_entry = trace
            .reports
            .iter()
            .find(|entry| entry.provider_id == "build.release_package")
            .expect("release Trace provider");
        assert_eq!(
            trace_entry.source_path.as_deref(),
            Some("C:\\owned\\release")
        );
        assert!(trace_entry.report_path.is_some());
        assert!(trace_entry.evidence.len() >= 3);
        assert!(!trace_entry.artifacts.is_empty());

        report.report_level = ReleasePackageReportLevel::Off;
        session.last_release_package_report = Some(report);
        let off = report_panel_for(&session);
        assert!(!off
            .reports
            .iter()
            .any(|entry| entry.provider_id == "build.release_package"));
        assert!(off
            .registry
            .descriptors
            .iter()
            .find(|descriptor| descriptor.provider_id == "build.release_package")
            .is_some_and(|descriptor| !descriptor.enabled));
    }

    #[test]
    fn release_package_report_cache_is_active_project_scoped_and_frame_io_free() {
        let root = unique_report_project_root("release-report-cache");
        let mut session = EditorSession::new();
        session.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "ReleaseReportCache".to_string(),
        }));
        let project_id = session
            .active_project_session
            .as_ref()
            .unwrap()
            .manifest
            .project_id
            .clone();
        let path = root.join(crate::RELEASE_PACKAGE_REPORT_RELATIVE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut report = release_report_fixture(ReleasePackageReportLevel::Summary);
        std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        assert!(session.reload_release_package_report_cache().is_err());
        assert!(session.last_release_package_report.is_none());

        report.project_id = project_id;
        std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        assert_eq!(session.reload_release_package_report_cache(), Ok(true));
        std::fs::remove_file(&path).unwrap();
        let panel = report_panel_for(&session);
        assert!(panel
            .reports
            .iter()
            .any(|entry| entry.provider_id == "build.release_package"
                && entry.status == ReportStatus::Passed));
    }

    fn release_report_fixture(level: ReleasePackageReportLevel) -> ReleasePackageReport {
        ReleasePackageReport {
            schema_version: RELEASE_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
            status: ReleasePackageStatus::Success,
            report_level: level,
            project_id: "project-release-report".to_string(),
            profile: "release".to_string(),
            target: "windows".to_string(),
            architecture: "x86_64".to_string(),
            display_name: "Complex Shooter".to_string(),
            display_version: "1.0.0".to_string(),
            entrypoint: "ComplexShooter.exe".to_string(),
            runtime_package: "data/runtime_package".to_string(),
            report_path: "C:\\owned\\project\\.aife\\reports\\release-package\\latest.json"
                .to_string(),
            output_dir: "C:\\owned\\release".to_string(),
            manifest_path: "C:\\owned\\release\\package-manifest.json".to_string(),
            runtime_content_hash: format!("sha256:{}", "a".repeat(64)),
            release_payload_hash: format!("sha256:{}", "b".repeat(64)),
            payload_file_count: 12,
            resource_readback: Some(crate::WindowsExecutableResourceReadback {
                product_name: "Complex Shooter".to_string(),
                company_name: "AI First Engine Studio".to_string(),
                file_description: "Complex Shooter".to_string(),
                product_version: "1.0.0".to_string(),
                file_version: "1.0.0.0".to_string(),
                copyright: "Copyright AI First Engine Studio".to_string(),
                original_filename: "ComplexShooter.exe".to_string(),
                fixed_file_version: [1, 0, 0, 0],
                fixed_product_version: [1, 0, 0, 0],
                icon_sizes: vec![16, 32, 48, 64, 128, 256],
                manifest_present: true,
            }),
            application: crate::ReleasePackageApplicationReport {
                display_name: "Complex Shooter".to_string(),
                executable_name: "ComplexShooter".to_string(),
                company_name: "AI First Engine Studio".to_string(),
                display_version: "1.0.0".to_string(),
            },
            runtime: crate::ReleasePackageRuntimeReport {
                relative_path: "data/runtime_package".to_string(),
                content_hash: format!("sha256:{}", "a".repeat(64)),
                formal_load_passed: true,
            },
            entrypoint_evidence: crate::ReleasePackageEntrypointReport {
                relative_path: "ComplexShooter.exe".to_string(),
                exists: true,
                role_verified: true,
            },
            resource: crate::ReleasePackageResourceReport {
                stamp_readback_verified: true,
                icon_sizes: vec![16, 32, 48, 64, 128, 256],
            },
            layout: crate::ReleasePackageLayoutReport {
                kind: "portable-directory-v1".to_string(),
                portable: true,
                include_reports: false,
                payload_file_count: 12,
            },
            payload_hash: crate::ReleasePackagePayloadHashReport {
                algorithm: "sha256".to_string(),
                value: format!("sha256:{}", "b".repeat(64)),
                verified: true,
            },
            verification: crate::ReleasePackageVerificationReport {
                manifest_valid: true,
                inventory_valid: true,
                runtime_load_passed: true,
                resource_readback_passed: true,
                publish_validated: true,
                explicit_process_status: "passed".to_string(),
                explicit_process_passed: true,
                process_exit_code: Some(0),
                child_player_exit_code: Some(0),
                child_frames_completed: Some(3),
                process_report_path: Some(
                    "C:\\owned\\project\\.aife\\reports\\release-package\\process-verification.json"
                        .to_string(),
                ),
            },
            diagnostics: Vec::new(),
            next_action: "Release package is ready.".to_string(),
        }
    }
}
