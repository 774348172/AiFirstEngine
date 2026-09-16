#[cfg(test)]
mod animator2d_integration;
mod assembly;
mod asset_browser_native;
mod aui_authoring;
mod aui_complex_controls;
mod aui_rectclip_scrollbar_navigation;
mod aui_runtime_interaction;
mod aui_runtime_navigation_screenflow_textentry;
mod aui_scene_authoring;
mod aui_template_reuse;
mod authoring_asset_completeness;
mod c01_from_blank_creation;
mod c01_golden_gate;
mod critical_correctness_safety;
mod editor_build_and_run;
mod editor_gameview_play;
mod exported_windows_playable_golden;
#[cfg(test)]
mod font_bundle_compatibility;
mod gameplay_rule_runtime;
mod gate;
mod input_mapping_visual_authoring;
mod llm_worker_lifecycle;
mod manual_walkthrough;
mod prefab_authoring;
mod prefab_runtime_bake;
#[cfg(test)]
mod project_editor_composition;
mod project_patch;
mod project_rule_driven_ui_state;
#[cfg(test)]
mod project_runtime_native_module;
mod project_write_containment;
mod real_texture_present;
mod release_package;
mod report;
mod rule_authoring;
mod rule_card_authoring;
mod sample_project;
mod save_reload_rebuild;
mod second_project_runtime;
mod unified_report_panel;
mod vertical_slice;

pub(crate) fn complex_shooter_linked_set(
) -> engine_runtime::project_runtime_module::LinkedProjectRuntimeSet {
    static ADAPTER: std::sync::OnceLock<
        engine_runtime::project_runtime_native_adapter::LoadedProjectRuntimeModuleAdapter,
    > = std::sync::OnceLock::new();
    generated_project_linked_set("complex_shooter_project", "complex-shooter", &ADAPTER)
}

pub(crate) fn switch_puzzle_linked_set(
) -> engine_runtime::project_runtime_module::LinkedProjectRuntimeSet {
    static ADAPTER: std::sync::OnceLock<
        engine_runtime::project_runtime_native_adapter::LoadedProjectRuntimeModuleAdapter,
    > = std::sync::OnceLock::new();
    generated_project_linked_set("switch_puzzle_project", "switch-puzzle", &ADAPTER)
}

fn generated_project_linked_set(
    sample_directory: &str,
    build_label: &str,
    adapter: &'static std::sync::OnceLock<
        engine_runtime::project_runtime_native_adapter::LoadedProjectRuntimeModuleAdapter,
    >,
) -> engine_runtime::project_runtime_module::LinkedProjectRuntimeSet {
    let adapter = adapter.get_or_init(|| {
        let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            .expect("project_e2e_gate must live under rust/crates")
            .to_path_buf();
        let rust_root = repository_root.join("rust").canonicalize().unwrap();
        let project_root = repository_root
            .join("samples")
            .join(sample_directory)
            .canonicalize()
            .unwrap();
        let mut session =
            project_authoring_execution::ProjectAuthoringSession::open(&project_root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease(format!("project-e2e-{build_label}"), paths)
            .unwrap();
        let prepared = project_authoring_execution::GameProjectCompiler::bind(&lease)
            .unwrap()
            .prepare(
                &lease,
                project_authoring_execution::TargetProfile::WindowsDev,
            )
            .unwrap();
        let glue = prepared
            .generated_runtime_glue()
            .expect("Project Game SDK sample must generate runtime glue")
            .clone();
        let staging = project_authoring_execution::ProjectRuntimeProductionStaging::plan(
            &project_root,
            &rust_root,
        )
        .unwrap();
        let build_input = prepared.runtime_package_build_input();
        let runtime_module = &build_input.project.runtime_module;
        let identity = editor_core::ProjectNativeModuleIdentity {
            schema_version: editor_core::PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION
                .to_string(),
            project_runtime_abi_digest: format!(
                "sha256:{}",
                project_runtime_abi::project_runtime_abi_digest_hex()
            ),
            project_runtime_sdk_digest: format!(
                "sha256:{}",
                project_runtime_sdk::project_runtime_contract_digest_hex()
            ),
            project_id: build_input.project.project_id.clone(),
            module_id: runtime_module.module_id.clone(),
            logical_interface_version: runtime_module.interface_version.clone(),
            aot_content_digest: runtime_module.aot_content_digest.clone(),
            normalized_manifest_digest: staging.normalized_manifest_digest,
            normalized_dependency_digest: staging.normalized_dependency_digest,
            dependency_lock_digest: staging.trusted_lock_digest,
            toolchain_identity: "project-e2e-generated-runtime-host".to_string(),
            target_triple: "host".to_string(),
            profile: "release".to_string(),
            features: Vec::new(),
            builder_schema_version:
                editor_core::PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION.to_string(),
        };
        let report = editor_core::ProjectRuntimeNativeModuleBuilder::prepare(
            &editor_core::ProjectRuntimeNativeModuleBuildRequest {
                source_crate_root: project_root.join("RuntimeModule"),
                engine_sdk_root: rust_root,
                build_root: std::env::temp_dir().join(format!(
                    "aife-project-e2e-generated-runtime-{build_label}-{}",
                    std::process::id()
                )),
                identity,
                cargo_executable: None,
                metadata_hard_deadline_ms: 120_000,
                build_hard_deadline_ms: 1_200_000,
                capture_limit_bytes: 1024 * 1024,
                prepared_runtime_glue: Some(glue),
            },
        );
        assert_eq!(
            report.status,
            editor_core::ProjectRuntimeNativeModuleBuildStatus::Success,
            "Compiler-generated {build_label} runtime failed: {:#?}",
            report.diagnostics
        );
        editor_core::ProjectRuntimeNativeModuleLoader::load(
            report
                .artifact
                .as_ref()
                .expect("generated runtime artifact"),
        )
        .expect("load Compiler-generated project runtime")
    });
    engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(std::sync::Arc::new(
        adapter.clone(),
    ))
    .expect("Compiler-generated project runtime descriptor must be linkable")
}

pub(crate) fn complex_shooter_linked_project_runtimes(
) -> std::sync::Arc<engine_runtime::project_runtime_module::LinkedProjectRuntimeSet> {
    std::sync::Arc::new(complex_shooter_linked_set())
}

pub(crate) fn complex_shooter_editor_session() -> editor_core::EditorSession {
    let linked = complex_shooter_linked_project_runtimes();
    let descriptor = linked
        .only_descriptor()
        .expect("Complex Shooter E2E runtime must be the only linked module")
        .clone();
    let mut session = editor_core::EditorSession::with_linked_project_runtimes(linked.clone());
    let ticket = session.begin_project_runtime_preparation(
        "project-complex-shooter-sample",
        descriptor.module_id.clone(),
        descriptor.interface_version.clone(),
    );
    let test_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let identity = editor_core::ProjectNativeModuleIdentity {
        schema_version: editor_core::PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION
            .to_string(),
        project_runtime_abi_digest: test_digest.to_string(),
        project_runtime_sdk_digest: test_digest.to_string(),
        project_id: ticket.project_id.clone(),
        module_id: descriptor.module_id,
        logical_interface_version: descriptor.interface_version,
        aot_content_digest: descriptor.aot_content_digest,
        normalized_manifest_digest: test_digest.to_string(),
        normalized_dependency_digest: test_digest.to_string(),
        dependency_lock_digest: test_digest.to_string(),
        toolchain_identity: "project-e2e-linked-fixture".to_string(),
        target_triple: std::env::consts::ARCH.to_string(),
        profile: "test".to_string(),
        features: Vec::new(),
        builder_schema_version: editor_core::PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION
            .to_string(),
    };
    assert!(
        session.install_prepared_project_runtime(&ticket, identity, linked),
        "Complex Shooter E2E runtime readiness must match its linked descriptor"
    );
    session
}

struct IdentityOnlyProjectRuntime {
    descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor,
}

impl engine_runtime::project_runtime_module::ProjectRuntimeModule for IdentityOnlyProjectRuntime {
    fn descriptor(
        &self,
    ) -> &engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        _registration: &mut engine_runtime::project_runtime_module::ProjectRuntimeRegistration,
    ) -> Result<(), engine_runtime::project_runtime_module::ProjectRuntimeError> {
        Ok(())
    }
}

pub(crate) fn identity_only_project_runtime(
    descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor,
) -> std::sync::Arc<dyn engine_runtime::project_runtime_module::ProjectRuntimeModule> {
    std::sync::Arc::new(IdentityOnlyProjectRuntime { descriptor })
}

pub(crate) fn identity_only_editor_session(module_id: &str) -> editor_core::EditorSession {
    let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
        identity_only_project_runtime(
            engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                module_id,
                "sha256:project-e2e-identity-only-runtime",
            ),
        ),
    )
    .expect("identity-only E2E runtime must form a singleton composition");
    editor_core::EditorSession::with_linked_project_runtimes(std::sync::Arc::new(linked))
}

pub use assembly::{
    AssemblyDomainStatus, ComplexShooterProjectAssemblyDiagnostic,
    ComplexShooterProjectAssemblyDomain, ComplexShooterProjectAssemblyMetrics,
    ComplexShooterProjectAssemblyReport, ComplexShooterProjectAssemblySpec,
    ComplexShooterProjectAssemblyValidator, COMPLEX_SHOOTER_PROJECT_ASSEMBLY_REPORT_SCHEMA_VERSION,
};
pub use asset_browser_native::{
    run_complex_shooter_asset_browser_native_report, AssetBrowserDragDropEvidence,
    AssetBrowserPathSafetyEvidence, AssetBrowserPickerEvidence, AssetBrowserRuntimePackageEvidence,
    AssetBrowserSaveReloadEvidence, AssetBrowserThumbnailEvidence,
    ComplexShooterAssetBrowserNativeReport, ComplexShooterAssetBrowserNativeRequest,
    ComplexShooterAssetBrowserNativeStatus,
    COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_ASSET_BROWSER_NATIVE_SCENARIO_ID,
};
pub use aui_authoring::{
    run_complex_shooter_aui_authoring_report, AuiAuthoringCommandEvidence,
    AuiAuthoringDocumentEvidence, ComplexShooterAuiAuthoringMetrics,
    ComplexShooterAuiAuthoringReport, ComplexShooterAuiAuthoringRequest,
    ComplexShooterAuiAuthoringStatus, COMPLEX_SHOOTER_AUI_AUTHORING_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_AUTHORING_SCENARIO_ID,
};
pub use aui_complex_controls::{
    run_complex_shooter_aui_complex_controls_report, ComplexShooterAuiComplexControlsMetrics,
    ComplexShooterAuiComplexControlsReport, ComplexShooterAuiComplexControlsRequest,
    ComplexShooterAuiComplexControlsStatus,
    COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_SCENARIO_ID,
};
pub use aui_rectclip_scrollbar_navigation::{
    run_complex_shooter_aui_rectclip_scrollbar_navigation_report,
    ComplexShooterAuiRectClipScrollbarNavigationMetrics,
    ComplexShooterAuiRectClipScrollbarNavigationReport,
    ComplexShooterAuiRectClipScrollbarNavigationRequest,
    ComplexShooterAuiRectClipScrollbarNavigationStatus,
    COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_SCENARIO_ID,
};
pub use aui_runtime_interaction::{
    run_complex_shooter_aui_runtime_interaction_report, ComplexShooterAuiRuntimeInteractionMetrics,
    ComplexShooterAuiRuntimeInteractionReport, ComplexShooterAuiRuntimeInteractionRequest,
    ComplexShooterAuiRuntimeInteractionStatus,
    COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_SCENARIO_ID,
};
pub use aui_runtime_navigation_screenflow_textentry::{
    run_complex_shooter_aui_runtime_navigation_screenflow_textentry_report,
    ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryMetrics,
    ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport,
    ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryRequest,
    ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus,
    COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_SCENARIO_ID,
};
pub use aui_scene_authoring::{
    run_complex_shooter_aui_scene_authoring_report, AuiSceneAuthoringDocumentEvidence,
    ComplexShooterAuiSceneAuthoringMetrics, ComplexShooterAuiSceneAuthoringReport,
    ComplexShooterAuiSceneAuthoringRequest, ComplexShooterAuiSceneAuthoringStatus,
    COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_SCENARIO_ID,
};
pub use aui_template_reuse::{
    run_complex_shooter_aui_template_reuse_report, ComplexShooterAuiTemplateReuseMetrics,
    ComplexShooterAuiTemplateReuseReport, ComplexShooterAuiTemplateReuseRequest,
    ComplexShooterAuiTemplateReuseStatus, COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_SCENARIO_ID,
};
pub use authoring_asset_completeness::{
    run_complex_shooter_authoring_asset_completeness_report,
    run_project_authoring_asset_completeness_report, AssetizationCandidate,
    AssetizationCandidateConfidence, AssetizationCandidateDomain, AssetizationCandidateStatus,
    PrefabCompletenessSummary, ProjectAuthoringAssetCompletenessReport,
    ProjectAuthoringAssetCompletenessRequest, ProjectAuthoringAssetCompletenessStatus,
    RuleCompletenessSummary, PROJECT_AUTHORING_ASSET_COMPLETENESS_REPORT_SCHEMA_VERSION,
    PROJECT_AUTHORING_ASSET_COMPLETENESS_SCENARIO_ID,
};
pub use c01_from_blank_creation::{
    run_c01_from_blank_creation_gate, C01EngineSourceSnapshot, C01FromBlankCreationReport,
    C01FromBlankCreationRequest, C01FromBlankCreationStatus, C01FromBlankPreflightEvidence,
    C01FromBlankTimingEvidence, C01GoalApprovalEvidence, C01PriorAttemptEvidence,
    C01_FROM_BLANK_CREATION_ENTRY_MODE, C01_FROM_BLANK_CREATION_REPORT_SCHEMA_VERSION,
    C01_FROM_BLANK_PROVIDER_MODE,
};
pub use c01_golden_gate::{
    run_c01_golden_gate, validate_existing_c01_project, C01AssetEvidence, C01CandidateEvidence,
    C01ExportEvidence, C01GoldenGateEntryMode, C01GoldenGateReport, C01GoldenGateRequest,
    C01GoldenGateStatus, C01GoldenGateTimingEvidence, C01PreviewEvidence, C01ReopenEvidence,
    C01RuntimeAssertions, C01_GOLDEN_GATE_REPORT_SCHEMA_VERSION, C01_GOLDEN_GATE_SCENARIO_ID,
};
pub use critical_correctness_safety::{
    CriticalCorrectnessSafetyDiagnostic, CriticalCorrectnessSafetyGateReport,
    CriticalCorrectnessSafetyStatus, PeContractSummary, ProcessLifecycleSummary,
    PublishLockSummary, RepairScopeSummary, CRITICAL_CORRECTNESS_SAFETY_GATE_REPORT_SCHEMA_VERSION,
};
pub use editor_build_and_run::{
    run_complex_shooter_editor_build_and_run_report, ComplexShooterEditorBuildAndRunArtifact,
    ComplexShooterEditorBuildAndRunDiagnostic, ComplexShooterEditorBuildAndRunMetrics,
    ComplexShooterEditorBuildAndRunReport, ComplexShooterEditorBuildAndRunRequest,
    ComplexShooterEditorBuildAndRunStatus,
    COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_SCENARIO_ID,
};
pub use editor_gameview_play::{
    run_complex_shooter_editor_gameview_gpu_texture_present_report,
    run_complex_shooter_editor_gameview_play_runner_report,
    ComplexShooterEditorGameViewGpuTexturePresentMetrics,
    ComplexShooterEditorGameViewGpuTexturePresentReport,
    ComplexShooterEditorGameViewGpuTexturePresentStatus,
    ComplexShooterEditorGameViewPlayRunnerMetrics, ComplexShooterEditorGameViewPlayRunnerReport,
    ComplexShooterEditorGameViewPlayRunnerRequest, ComplexShooterEditorGameViewPlayRunnerStatus,
    COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_SCENARIO_ID,
    COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_SCENARIO_ID,
};
pub use exported_windows_playable_golden::{
    run_complex_shooter_exported_windows_playable_golden_report,
    ComplexShooterExportedWindowsPlayableGoldenReport,
    ComplexShooterExportedWindowsPlayableGoldenRequest,
    ComplexShooterExportedWindowsPlayableGoldenStatus, ComplexShooterGoldenArtifact,
    ComplexShooterGoldenDiagnostic, ComplexShooterGoldenEvidenceStatus,
    ComplexShooterGoldenEvidenceSummary, ComplexShooterGoldenGameplayEvidence,
    ComplexShooterGoldenHudEvidence, ComplexShooterGoldenPackageEvidence,
    ComplexShooterGoldenProcessEvidence, ComplexShooterGoldenRealWindowEvidence,
    ComplexShooterGoldenReportMode, ComplexShooterGoldenTextureEvidence,
    ComplexShooterRealWindowEvidenceStatus,
    COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_SCENARIO_ID,
};
pub use gameplay_rule_runtime::{
    run_complex_shooter_gameplay_rule_runtime_report, ComplexShooterGameplayRuleRuntimeMetrics,
    ComplexShooterGameplayRuleRuntimeReport, ComplexShooterGameplayRuleRuntimeRequest,
    ComplexShooterGameplayRuleRuntimeStatus,
    COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_SCENARIO_ID,
};
pub use gate::{run_complex_project_e2e_gate, ComplexProjectE2eGateRequest};
pub use input_mapping_visual_authoring::{
    run_complex_shooter_input_mapping_visual_authoring_report,
    ComplexShooterInputMappingVisualAuthoringMetrics,
    ComplexShooterInputMappingVisualAuthoringReport,
    ComplexShooterInputMappingVisualAuthoringRequest,
    ComplexShooterInputMappingVisualAuthoringStatus,
    COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_INPUT_MAPPING_VISUAL_AUTHORING_SCENARIO_ID,
};
pub use llm_worker_lifecycle::{
    run_llm_worker_lifecycle_report, LlmWorkerLifecycleReport, LlmWorkerLifecycleScenarioEvidence,
    LlmWorkerLifecycleStatus, LLM_WORKER_LIFECYCLE_REPORT_SCHEMA_VERSION,
};
pub use manual_walkthrough::{
    run_complex_shooter_manual_walkthrough_coverage,
    ComplexShooterManualWalkthroughCoverageRequest, COMPLEX_SHOOTER_MANUAL_WALKTHROUGH_SCENARIO_ID,
};
pub use prefab_authoring::{
    run_complex_shooter_prefab_authoring_report, ComplexShooterPrefabAuthoringMetrics,
    ComplexShooterPrefabAuthoringReport, ComplexShooterPrefabAuthoringRequest,
    ComplexShooterPrefabAuthoringStatus, PrefabAuthoringAssetEvidence,
    PrefabAuthoringCommandEvidence, PrefabAuthoringInstanceEvidence,
    COMPLEX_SHOOTER_PREFAB_AUTHORING_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_PREFAB_AUTHORING_SCENARIO_ID,
};
pub use prefab_runtime_bake::{
    run_complex_shooter_prefab_runtime_bake_report, ComplexShooterPrefabRuntimeBakeMetrics,
    ComplexShooterPrefabRuntimeBakeReport, ComplexShooterPrefabRuntimeBakeRequest,
    ComplexShooterPrefabRuntimeBakeStatus,
    COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_PREFAB_RUNTIME_BAKE_SCENARIO_ID,
};
pub use project_patch::{
    run_complex_shooter_imported_project_patch_smoke, run_complex_shooter_llm_patch_source_smoke,
    run_complex_shooter_project_patch_smoke, ComplexShooterProjectPatchRequest,
    COMPLEX_SHOOTER_IMPORTED_PROJECT_PATCH_SCENARIO_ID,
    COMPLEX_SHOOTER_LLM_PATCH_SOURCE_SCENARIO_ID, COMPLEX_SHOOTER_PROJECT_PATCH_SCENARIO_ID,
};
pub use project_rule_driven_ui_state::{
    run_complex_shooter_project_rule_driven_ui_state_report,
    ComplexShooterProjectRuleDrivenUiStateMetrics, ComplexShooterProjectRuleDrivenUiStateReport,
    ComplexShooterProjectRuleDrivenUiStateRequest, ComplexShooterProjectRuleDrivenUiStateStatus,
    COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_SCENARIO_ID,
};
pub use project_write_containment::{
    run_project_write_containment_report, ProjectWriteContainmentEvidence,
    ProjectWriteContainmentReport, ProjectWriteContainmentStatus,
    PROJECT_WRITE_CONTAINMENT_REPORT_SCHEMA_VERSION,
};
pub use real_texture_present::{
    run_complex_shooter_real_texture_present_report, ComplexShooterRealTexturePresentMetrics,
    ComplexShooterRealTexturePresentReport, ComplexShooterRealTexturePresentRequest,
    ComplexShooterRealTexturePresentStatus,
    COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_SCENARIO_ID,
};
pub use release_package::{
    run_complex_shooter_release_package_report, ComplexShooterReleasePackageMetrics,
    ComplexShooterReleasePackageReport, ComplexShooterReleasePackageRequest,
    ComplexShooterReleasePackageStatus, COMPLEX_SHOOTER_RELEASE_PACKAGE_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_RELEASE_PACKAGE_SCENARIO_ID,
};
pub use report::{
    ComplexProjectE2eArtifact, ComplexProjectE2eDiagnostic, ComplexProjectE2eGap,
    ComplexProjectE2eGateReport, ComplexProjectE2eMetrics, ComplexProjectE2eStatus,
    ComplexProjectE2eStep, COMPLEX_PROJECT_E2E_GATE_REPORT_SCHEMA_VERSION,
};
pub use rule_authoring::{
    run_complex_shooter_rule_authoring_report, ComplexShooterRuleAuthoringReport,
    ComplexShooterRuleAuthoringRequest, ComplexShooterRuleAuthoringStatus,
    COMPLEX_SHOOTER_RULE_AUTHORING_REPORT_SCHEMA_VERSION,
};
pub use rule_card_authoring::{
    run_complex_shooter_rule_card_authoring_report, ComplexShooterRuleCardAuthoringReport,
    ComplexShooterRuleCardAuthoringRequest, ComplexShooterRuleCardAuthoringStatus,
    RuleCardAuthoringEditEvidence, RuleCardAuthoringRuleEvidence,
    COMPLEX_SHOOTER_RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_RULE_CARD_AUTHORING_SCENARIO_ID,
};
pub use sample_project::{load_sample_project_summary, SampleProjectSummary};
pub use save_reload_rebuild::{
    run_author_save_child, run_process_isolated_authoring, run_reopen_read_child,
    run_save_reload_rebuild_consistency, SaveReloadRebuildConsistencyRequest,
};
pub use second_project_runtime::{
    run_second_project_runtime_report, SecondProjectRuntimeEvidence, SecondProjectRuntimeReport,
    SecondProjectRuntimeStatus, SECOND_PROJECT_RUNTIME_REPORT_SCHEMA_VERSION,
};
pub use unified_report_panel::{
    run_complex_shooter_unified_report_panel, ComplexShooterUnifiedReportPanelMetrics,
    ComplexShooterUnifiedReportPanelReport, ComplexShooterUnifiedReportPanelRequest,
    ComplexShooterUnifiedReportPanelStatus, UnifiedReportPanelProviderEvidence,
    COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_REPORT_SCHEMA_VERSION,
    COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_SCENARIO_ID,
};
pub use vertical_slice::{
    run_authoring_to_playable_vertical_slice, AuthoringToPlayableDiagnostic,
    AuthoringToPlayableMetrics, AuthoringToPlayableStep, AuthoringToPlayableVerticalSliceReport,
    AuthoringToPlayableVerticalSliceRequest, AuthoringToPlayableVerticalSliceStatus,
    AuthoringWorkflowStepEvidence, AuthoringWorkspaceDomainEvidence,
    AUTHORING_TO_PLAYABLE_VERTICAL_SLICE_REPORT_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests;
