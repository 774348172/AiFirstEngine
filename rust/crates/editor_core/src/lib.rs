mod ai_capability_tool_kernel;
mod ai_goal_grant;
mod ai_image_generation;
mod ai_tool_catalog;
mod android_export;
mod animator2d_authoring;
mod animator2d_cooker;
mod asset_browser;
mod asset_placement;
mod asset_thumbnail;
mod aui_authoring;
mod aui_document_cooker;
mod aui_font_atlas_cooker;
mod aui_scene_authoring;
mod aui_template;
mod authoring_system;
mod authoring_workflow;
mod authoring_workspace;
mod candidate_project_revision;
mod controlled_source_patch;
mod desktop_export;
mod editable_project_loop;
mod editor_command;
mod editor_command_executor;
mod editor_command_registry;
mod editor_gameview_play;
mod editor_preview_package;
mod editor_project_composition_launcher;
mod engine_builtin_font_pack;
mod goal_mutation;
mod input_mapping_authoring;
mod inspector_details;
mod manual_walkthrough;
mod play_session;
mod prefab_workflow;
mod project_assembly_artifact_cache;
mod project_asset_import;
mod project_candidate_entry;
mod project_consistency;
mod project_delivery_tools;
mod project_editor_composition;
mod project_editor_composition_artifact;
mod project_editor_composition_cache_promotion;
mod project_font_assets;
mod project_font_bundle;
mod project_font_cook;
mod project_intent_workflow;
mod project_launcher;
mod project_observation;
mod project_open_preparation;
mod project_patch;
mod project_player_artifact;
mod project_preview_evidence;
mod project_readiness;
mod project_runtime_native_module;
mod project_runtime_package_assembler;
mod project_runtime_player_staging;
mod project_runtime_preparation;
mod project_runtime_trust;
mod project_visual_diagnostics;
mod project_write_scope;
mod property_editing;
mod release_package;
mod report_panel;
mod rule_authoring;
mod runtime_selection;
mod scene_editing;
mod services;
mod session;
mod transaction;
mod ui_model_composer;
mod windows_executable_resources;

pub use ai_capability_tool_kernel::{
    AiCandidateToolInput, AiCapabilityGrant, AiCapabilityGrantKind, AiCapabilityScopeMode,
    AiCapabilityToolKernel, AiElevatedGrantSpec, AiGrantLineage, AiMutationKind,
    AiProjectInspection, AiToolAccepted, AiToolCancellationReceipt, AiToolCancellationStatus,
    AiToolCapability, AiToolContractRegistry, AiToolCostClass, AiToolDescriptor, AiToolDiagnostic,
    AiToolDiagnosticSeverity, AiToolDurationClass, AiToolExecutionStatus, AiToolInspectKind,
    AiToolInspectPayload, AiToolInspectRequest, AiToolInspectResult, AiToolInvocation,
    AiToolInvocationPayload, AiToolKernelError, AiToolMutationReceipt, AiToolOperationSnapshot,
    AiToolOperationState, AiToolOperationTransition, AiToolOutput, AiToolPreviewEvidence,
    AiToolResult, AiToolRollbackReceipt, AiToolSideEffect, AiToolStartOutcome,
    ExternalProjectRollbackInput, ProjectCreateDirectInput, ProjectCreateToolReceipt,
    AI_CAPABILITY_GRANT_SCHEMA_VERSION, AI_TOOL_ACCEPTED_SCHEMA_VERSION,
    AI_TOOL_CANCELLATION_RECEIPT_SCHEMA_VERSION, AI_TOOL_DESCRIPTOR_SCHEMA_VERSION,
    AI_TOOL_IMPLEMENTATION_VERSION_V1, AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION,
    AI_TOOL_INSPECT_RESULT_SCHEMA_VERSION, AI_TOOL_INVOCATION_SCHEMA_VERSION,
    AI_TOOL_KERNEL_JOURNAL_SCHEMA_VERSION, AI_TOOL_MUTATION_RECEIPT_SCHEMA_VERSION,
    AI_TOOL_OPERATION_SCHEMA_VERSION, AI_TOOL_RESULT_SCHEMA_VERSION,
    AI_TOOL_ROLLBACK_RECEIPT_SCHEMA_VERSION, EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
    TOOL_ID_EVIDENCE_READ, TOOL_ID_PROJECT_BUILD_EXPORT, TOOL_ID_PROJECT_CREATE,
    TOOL_ID_PROJECT_DELIVERY_VERIFY, TOOL_ID_PROJECT_DIAGNOSTICS, TOOL_ID_PROJECT_INSPECT,
    TOOL_ID_PROJECT_MUTATE, TOOL_ID_PROJECT_PREVIEW, TOOL_ID_PROJECT_READ_OBJECT,
    TOOL_ID_PROJECT_REFERENCES, TOOL_ID_PROJECT_ROLLBACK, TOOL_ID_PROJECT_SEARCH,
    TOOL_ID_PROJECT_SOURCE_SYMBOLS, TOOL_ID_PROJECT_TRACE_UI_OWNER, TOOL_ID_RUNTIME_CAPTURE_ISSUE,
    TOOL_ID_UI_EXPLAIN_VISIBILITY, TOOL_ID_UI_LOCATE,
};
pub use ai_goal_grant::{
    AiGoalBinding, AiGoalCompletionPolicy, AiGoalGrantError, AiGoalGrantSpec, AiGoalRiskClass,
    AiRiskEnvelope, AiRiskEnvelopeSpec, AI_GOAL_BINDING_SCHEMA_VERSION,
    AI_GOAL_GRANT_SPEC_SCHEMA_VERSION, AI_RISK_ENVELOPE_SCHEMA_VERSION,
};
pub use ai_image_generation::{
    import_generated_image_formally, run_ai_image_generation_loop_headless,
    AiImageGenerationDiagnostic, AiImageGenerationDiagnosticSeverity, AiImageGenerationLoopReport,
    AiImageGenerationRequest, AiImageGenerationResult, AiImageGenerationStatus,
    GeneratedImageImportResult, GeneratedImageMetadata, GeneratedImageSource,
    ImageGenerationProvider, ImageKind, MockImageGenerationProvider,
    AI_IMAGE_GENERATION_LOOP_REPORT_SCHEMA_VERSION, AI_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION,
    AI_IMAGE_GENERATION_RESULT_SCHEMA_VERSION, GENERATED_IMAGE_METADATA_SCHEMA_VERSION,
    GENERATED_IMAGE_SOURCE_SCHEMA_VERSION,
};
pub use ai_tool_catalog::{
    AiToolAvailability, AiToolAvailabilityBasis, AiToolAvailabilityContext,
    AiToolAvailabilityOwner, AiToolAvailabilityReason, AiToolAvailabilityReasonCategory,
    AiToolAvailabilityResolutionKind, AiToolAvailabilityState, AiToolCatalog, AiToolCatalogEntry,
    AiToolCatalogRequest, AiToolMutationAvailabilityState, AiToolReadAvailabilityState,
    AI_TOOL_CATALOG_SCHEMA_VERSION, AI_TOOL_CATALOG_V1_SCHEMA_VERSION,
};
pub use android_export::*;
pub use animator2d_authoring::*;
pub use animator2d_cooker::*;
pub use asset_browser::{
    AssetBrowserBuildRequest, AssetBrowserIndex, AssetBrowserIndexSnapshot,
    AssetBrowserReportLevel, AssetBrowserService, AssetBrowserSessionState, AssetBrowserUiState,
    AssetPickCommitPlan, AssetPickerSessionState,
    ASSET_BROWSER_NATIVE_PRODUCTIZATION_REPORT_SCHEMA_VERSION, ASSET_BROWSER_REPORT_SCHEMA_VERSION,
};
pub use asset_placement::{
    AssetPlacementDiagnostic, AssetPlacementDiagnosticSeverity, AssetPlacementPlan,
    AssetPlacementReport, AssetPlacementRequest, AssetPlacementResolver,
    ASSET_AUTHORING_LOOP_REPORT_SCHEMA_VERSION, ASSET_PLACEMENT_REPORT_SCHEMA_VERSION,
};
pub use asset_thumbnail::{
    AssetThumbnailCpuPayload, AssetThumbnailRequest, AssetThumbnailService,
    AssetThumbnailServiceSummary, ASSET_THUMBNAIL_MAX_CPU_BYTES, ASSET_THUMBNAIL_MAX_ITEMS,
    ASSET_THUMBNAIL_MAX_PENDING,
};
pub use aui_authoring::{
    AuiAuthoringDiagnostic, AuiAuthoringReport, AuiAuthoringService, AuiNodeFieldValue,
    AuiTransaction, AuiTransactionStatus, AUI_AUTHORING_REPORT_SCHEMA_VERSION,
};
pub use aui_document_cooker::{
    AuiDocumentCookDiagnostic, AuiDocumentCookDiagnosticSeverity, AuiDocumentCookOutput,
    AuiDocumentCookReport, AuiDocumentCookRequest, AuiDocumentCookStatus, AuiDocumentCooker,
    AUI_DOCUMENT_COOK_REPORT_SCHEMA_VERSION,
};
pub use aui_scene_authoring::{AuiSceneAuthoringBuildOutput, AuiSceneAuthoringService};
pub use aui_template::{
    AuiTemplateAsset, AuiTemplateDependencyRef, AuiTemplateDiagnostic,
    AuiTemplateDiagnosticSeverity, AuiTemplateInstantiateReport, AuiTemplateInstantiateRequest,
    AuiTemplateNodeIdRemap, AuiTemplateOperationStatus, AuiTemplateRef, AuiTemplateWorkflow,
    AUI_TEMPLATE_ASSET_SCHEMA_VERSION, AUI_TEMPLATE_INSTANTIATE_REPORT_SCHEMA_VERSION,
};
pub use authoring_system::{
    run_editor_authoring_system_headless, EditorAuthoringDiagnostic, EditorAuthoringReport,
    EDITOR_AUTHORING_REPORT_SCHEMA_VERSION,
};
pub use authoring_workflow::AuthoringWorkflowComposer;
pub use authoring_workspace::{
    workspace_command_id_for_payload, EditorAuthoringWorkspace, WorkspaceCommandExecutionReport,
    WorkspaceContext, WorkspaceDiagnosticsSummary, WorkspaceReport, WorkspaceSelection,
    WorkspaceState,
};
pub use candidate_project_revision::{
    CandidateBaseVerification, CandidateBaseVerificationStatus, CandidateDiscardOutcome,
    CandidateDiscardReceipt, CandidateFileChange, CandidateProjectRevision,
    CandidateProjectRevisionError, CandidateProjectRevisionRequest, CandidateProjectRevisionStatus,
    CandidateProjectRevisionStore, CANDIDATE_PROJECT_REVISION_SCHEMA_VERSION,
};
pub use controlled_source_patch::{
    ControlledSourcePatch, ControlledSourcePatchApplyReceipt, ControlledSourcePatchApplyRequest,
    ControlledSourcePatchApproval, ControlledSourcePatchCandidate, ControlledSourcePatchDiagnostic,
    ControlledSourcePatchDiagnosticSeverity, ControlledSourcePatchDocument,
    ControlledSourcePatchError, ControlledSourcePatchExecutionPolicy,
    ControlledSourcePatchOperation, ControlledSourcePatchPrepareRequest,
    ControlledSourcePatchRollbackReceipt, ControlledSourcePatchValidationReport,
    ControlledSourcePatchValidationRequest, ControlledSourcePatchValidationStatus,
    ControlledSourcePatchValidationStep, ControlledSourcePatchValidationStepStatus,
    TrustedEngineSdkLocator, CONTROLLED_SOURCE_PATCH_APPLY_RECEIPT_SCHEMA_VERSION,
    CONTROLLED_SOURCE_PATCH_APPROVAL_SCHEMA_VERSION,
    CONTROLLED_SOURCE_PATCH_CANDIDATE_SCHEMA_VERSION,
    CONTROLLED_SOURCE_PATCH_ROLLBACK_RECEIPT_SCHEMA_VERSION,
    CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION,
    CONTROLLED_SOURCE_PATCH_VALIDATION_REPORT_SCHEMA_VERSION,
};
pub use desktop_export::{
    DesktopExportDiagnostic, DesktopExportDiagnosticSeverity, DesktopExportPipeline,
    DesktopExportReport, DesktopExportRequest, DesktopExportStatus, DesktopExportTarget,
    DesktopPackageManifest, ExplicitExportOutput, DESKTOP_EXPORT_REPORT_SCHEMA_VERSION,
    DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION,
};
pub use editable_project_loop::{
    create_default_editable_project_fixture, open_default_editable_scene_for_test,
    run_asset_authoring_loop_headless, run_editable_project_loop_headless,
    AssetAuthoringLoopDiagnostic, AssetAuthoringLoopReport, DefaultEditableProjectFixture,
    EditableProjectLoopDiagnostic, EditableProjectLoopDiagnosticSeverity,
    EditableProjectLoopReport, EDITABLE_PROJECT_LOOP_REPORT_SCHEMA_VERSION,
};
pub use editor_command::{
    EditorCommandAvailability, EditorCommandCategory, EditorCommandContext,
    EditorCommandDescriptor, EditorCommandId, EditorCommandOwnerDomain, EditorCommandPayload,
    EditorCommandPayloadKind, EditorCommandRequest,
};
pub use editor_command_executor::{
    execute_editor_command, execute_ui_payload_as_editor_command,
    ui_command_to_editor_command_request,
};
pub use editor_command_registry::{
    builtin_editor_command_descriptors, command_id_for_payload, EditorCommandRegistry,
};
pub use editor_gameview_play::{
    stable_game_view_surface_id, ApplyRuntimeChangeCandidate, ApplyRuntimeChangeCandidateStatus,
    ApplyRuntimeChangeReport, EditorGameViewPlayOutput, EditorGameViewPlayRunner,
    EditorRuntimePlayInstance, EditorRuntimePlayRequest, EditorRuntimePlayState,
    GameViewAuiActionTarget, GameViewPresentDiagnostic, GameViewPresentDiagnosticSeverity,
    GameViewPresentReport, GameViewPresentStatus, GameViewRuntimeFrame, RuntimeAuthoringOrigin,
    RuntimeAuthoringOriginKind, RuntimeTemporaryEditApplyPolicy, RuntimeTemporaryEditRecord,
    APPLY_RUNTIME_CHANGE_REPORT_SCHEMA_VERSION, EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION,
    GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION, GAME_VIEW_RUNTIME_FRAME_SCHEMA_VERSION,
};
pub use editor_preview_package::{
    EditorPlayPreviewPackageReport, EditorPreviewPackageCacheManifest,
    EditorPreviewPackageCacheStatus, EditorPreviewPackageDiagnostic,
    EditorPreviewPackageDiagnosticSeverity, EditorPreviewPackageDirtyDomain,
    EditorPreviewPackageFingerprint, EditorPreviewPackageRequest, EditorPreviewPackageService,
    EditorPreviewPackageStageReport, EditorPreviewPackageStageStatus, EditorPreviewPackageStatus,
    EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION,
    EDITOR_PREVIEW_PACKAGE_CACHE_MANIFEST_SCHEMA_VERSION,
    EDITOR_PREVIEW_PACKAGE_REQUEST_SCHEMA_VERSION,
};
pub use editor_project_composition_launcher::*;
pub use engine_builtin_font_pack::{
    EngineBuiltInFontPack, EngineBuiltInFontPackError, EngineBuiltInFontPackManifest,
    EngineDefaultGlyphSetEntry, EngineDefaultGlyphSetLock, EngineDefaultGlyphSetSpec,
    ENGINE_BUILT_IN_FONT_PACK_ID, ENGINE_BUILT_IN_FONT_PACK_MANIFEST_SCHEMA_VERSION,
    ENGINE_DEFAULT_GLYPH_SET_LOCK_SCHEMA_VERSION, ENGINE_DEFAULT_GLYPH_SET_SPEC_SCHEMA_VERSION,
};
pub use goal_mutation::{
    BoundGoalMutation, ExternalProjectMutationChange, ExternalProjectMutationGoal,
    ExternalProjectMutationIntent, GoalMutationError, GoalMutationModule, GoalMutationOwnerFacts,
    BOUND_GOAL_MUTATION_SCHEMA_VERSION, EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION,
};
pub use input_mapping_authoring::{
    load_first_input_mapping, scan_input_action_references, scan_input_mapping_paths,
    InputMappingAuthoringService, InputMappingEditCommand, InputMappingEditorState,
    INPUT_MAPPING_AUTHORING_REPORT_SCHEMA_VERSION,
    INPUT_MAPPING_VISUAL_AUTHORING_REPORT_SCHEMA_VERSION,
};
pub use inspector_details::{
    AssetFilter, ComponentSchema, ComponentSchemaRegistry, EnumOption, FieldConstraint,
    FieldSchema, InspectableComponentData, InspectableTarget, InspectableTargetKind,
    InspectorDiagnostic, InspectorDiagnosticCode, InspectorDiagnosticSeverity, InspectorReport,
    InspectorSourceData, ObjectSchema, PropertyEditorWidgetDescriptor, PropertyHandle,
    PropertyHandleState, PropertyOverrideState, PropertyTransactionRoute,
    PropertyTransactionRouter, PropertyTreeBuildResult, PropertyTreeBuilder,
    INSPECTOR_REPORT_SCHEMA_VERSION,
};
pub use manual_walkthrough::{ManualWalkthroughCoverageAnalyzer, ManualWalkthroughCoverageInput};
pub use play_session::{
    PlayRunner, PlaySessionController, PlaySessionDiagnostic, PlaySessionDiagnosticSeverity,
    PlaySessionMode, PlaySessionReport, PlaySessionRequest, PlaySessionRequestedBy,
    PlaySessionState, PLAY_SESSION_REPORT_SCHEMA_VERSION,
};
pub use prefab_workflow::{
    detect_cyclic_prefab_references, validate_prefab_asset, PrefabAsset, PrefabAssetRef,
    PrefabAuthoringModel, PrefabAuthoringReport, PrefabAuthoringStatus, PrefabDiagnostic,
    PrefabDiagnosticCode, PrefabDiagnosticSeverity, PrefabEntity, PrefabInstance, PrefabOverride,
    PrefabRef, PrefabStageModel, PrefabStageReport, PrefabWorkflowReport, PrefabWorkflowService,
    ResolvedPrefabEntity, ResolvedPrefabView, PREFAB_ASSET_SCHEMA_VERSION,
    PREFAB_AUTHORING_REPORT_SCHEMA_VERSION, PREFAB_INSTANCE_COMPONENT_TYPE,
    PREFAB_OVERRIDE_COMPONENT_TYPE, PREFAB_STAGE_REPORT_SCHEMA_VERSION,
    PREFAB_WORKFLOW_REPORT_SCHEMA_VERSION,
};
pub use project_assembly_artifact_cache::{
    ProjectAssemblyArtifactCache, ProjectAssemblyArtifactCacheError,
    ProjectAssemblyArtifactCacheStatus, ProjectAssemblyArtifactEnvelope,
    ProjectAssemblyArtifactLookup, ProjectAssemblyArtifactPublishResult,
    ProjectAssemblyArtifactPublishStatus, ProjectAssemblyProducerReport,
    ProjectAssemblyProducerSubstageReport, PROJECT_ASSEMBLY_ARTIFACT_ENVELOPE_SCHEMA_VERSION,
    PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION,
};
pub use project_asset_import::{
    AssetDatabaseDocument, AssetDatabaseRecord, AssetDatabaseRecordState, AssetGraphDocument,
    AssetGraphNode, AssetImportConflictPolicy, AssetImportSourceKind, AssetImportSourceMetadata,
    AssetLicenseKind, AssetLicenseMetadata, AssetRegistryDocument, AssetRegistryEntry,
    ProjectAssetImport, ProjectAssetImportApplyReceipt, ProjectAssetImportApplyRequest,
    ProjectAssetImportApproval, ProjectAssetImportCandidate, ProjectAssetImportDiagnostic,
    ProjectAssetImportDiagnosticSeverity, ProjectAssetImportError,
    ProjectAssetImportPrepareRequest, ProjectAssetImportRollbackReceipt,
    ProjectAssetImportValidationReport, ProjectAssetImportValidationStatus, ProjectAssetMeta,
    TextureImportSettings, FONT_SOURCE_ASSET_TYPE, FONT_SOURCE_IMPORTER_ID,
    FONT_SOURCE_IMPORTER_VERSION, PROJECT_ASSET_DATABASE_SCHEMA_VERSION,
    PROJECT_ASSET_GRAPH_SCHEMA_VERSION, PROJECT_ASSET_IMPORT_APPLY_RECEIPT_SCHEMA_VERSION,
    PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION, PROJECT_ASSET_IMPORT_CANDIDATE_SCHEMA_VERSION,
    PROJECT_ASSET_IMPORT_ROLLBACK_RECEIPT_SCHEMA_VERSION,
    PROJECT_ASSET_IMPORT_VALIDATION_REPORT_SCHEMA_VERSION, PROJECT_ASSET_META_SCHEMA_VERSION,
    PROJECT_ASSET_REGISTRY_SCHEMA_VERSION,
};
pub use project_candidate_entry::{
    PreparedProjectCandidatePayload, ProjectCandidate, ProjectCandidateAppliedPayload,
    ProjectCandidateApplyReceipt, ProjectCandidateApproval, ProjectCandidateEntry,
    ProjectCandidateEnvelope, ProjectCandidateError, ProjectCandidatePayload,
    ProjectCandidatePrepareRequest, ProjectCandidateProjectBinding,
    ProjectCandidateRollbackReceipt, ProjectCandidateRolledBackPayload, ProjectCandidateSourceKind,
    ProjectCandidateValidationContext, ProjectCandidateValidationPayload,
    ProjectCandidateValidationReport, ProjectCandidateValidationStatus,
    PROJECT_CANDIDATE_APPLY_RECEIPT_SCHEMA_VERSION, PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION,
    PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION, PROJECT_CANDIDATE_PROJECT_BINDING_SCHEMA_VERSION,
    PROJECT_CANDIDATE_ROLLBACK_RECEIPT_SCHEMA_VERSION, PROJECT_CANDIDATE_SCHEMA_VERSION,
    PROJECT_CANDIDATE_VALIDATION_REPORT_SCHEMA_VERSION,
};
pub use project_consistency::{
    read_consistency_report, write_consistency_report_atomic,
    write_consistency_report_external_atomic, write_consistency_report_in_scope, BuildRecipeDigest,
    BuildRecipeDigestInput, ConsistencyComparison, ConsistencyDomainDigest,
    ConsistencyMutationEvidence, ConsistencyProcessEvidence, ConsistencyReportLevel,
    SaveReloadRebuildCheckpoint, SaveReloadRebuildConsistencyReport, SaveReloadRebuildDiagnostic,
    SaveReloadRebuildStatus, SourceRuntimeWitness, BUILD_RECIPE_DIGEST_SCHEMA_VERSION,
    SAVE_RELOAD_REBUILD_CHECKPOINT_SCHEMA_VERSION,
    SAVE_RELOAD_REBUILD_CONSISTENCY_REPORT_SCHEMA_VERSION,
    SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH,
};
pub use project_delivery_tools::*;
pub use project_editor_composition::{
    generated_composition_lock_lineage, resolve_project_editor_composition_build_qos,
    GeneratedCompositionLockInput, GeneratedCompositionLockLineage,
    ProjectEditorCompositionArtifact, ProjectEditorCompositionBuildDeadlinePolicy,
    ProjectEditorCompositionBuildQosDecision, ProjectEditorCompositionBuildQosPolicy,
    ProjectEditorCompositionBuildReport, ProjectEditorCompositionBuildRequest,
    ProjectEditorCompositionBuildSourceKind, ProjectEditorCompositionBuildStatus,
    ProjectEditorCompositionBuildStep, ProjectEditorCompositionCachePolicy,
    ProjectEditorCompositionCacheStatus, ProjectEditorCompositionCompilationCacheAffinity,
    ProjectEditorCompositionContractError, ProjectEditorCompositionDescriptor,
    ProjectEditorCompositionDiagnostic, ProjectEditorCompositionHandoffTicket,
    ProjectEditorCompositionIdentity, ProjectEditorCompositionLaunchReceipt,
    ProjectEditorCompositionLaunchStatus, ProjectEditorCompositionPreparationControl,
    ProjectEditorCompositionPreparationPhase, ProjectEditorCompositionProcessPriority,
    ProjectEditorCompositionPromotionBackupStatus, ProjectEditorCompositionPromotionCleanupStatus,
    ProjectEditorCompositionPromotionReport, ProjectEditorCompositionPromotionRequest,
    ProjectEditorCompositionPromotionRollbackStatus, ProjectEditorCompositionPromotionStage,
    ProjectEditorCompositionPromotionStatus, ProjectEditorCompositionQualificationKind,
    ProjectEditorCompositionQualificationSeal, ProjectEditorCompositionResolvedIdentity,
    ProjectEditorCompositionSystemFacts, GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_BUILD_DEADLINE_POLICY_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_BUILD_QOS_POLICY_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V2,
    PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION_V2,
    PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_HANDOFF_TICKET_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION_V1,
    PROJECT_EDITOR_COMPOSITION_RESOLVED_IDENTITY_SCHEMA_VERSION,
};
pub use project_editor_composition_cache_promotion::ProjectEditorCompositionCacheAdmin;
pub use project_font_assets::{
    diagnostics_for_level, resolve_font_family_face, validate_font_face_source,
    validate_font_stack, FontAtlasProfileAsset, FontAtlasProfileRole, FontDiagnostic,
    FontDiagnosticSeverity, FontDiagnosticStage, FontFaceAsset, FontFaceDeclaredMetadata,
    FontFaceSource, FontFamilyAsset, FontFamilyFace, FontGlyphSet, FontHintingMode,
    FontMissingGlyphPolicy, FontMissingStylePolicy, FontPackingProfile, FontRasterPolicy,
    FontRasterProfile, FontReportLevel, FontSourceKind, FontStackAsset, FontStyle,
    ProjectFontAssetSet, ValidatedFontFaceSource, FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION,
    FONT_FACE_ASSET_SCHEMA_VERSION, FONT_FAMILY_ASSET_SCHEMA_VERSION,
    FONT_STACK_ASSET_SCHEMA_VERSION, PROJECT_FONT_ASSET_GRAPH_SCHEMA_VERSION,
    PROJECT_FONT_RECIPE_VERSION,
};
pub use project_font_bundle::{
    select_auto_hybrid, FontAutoHybridDecision, FontAutoHybridRequest, ProjectFontBundleBuilder,
};
pub use project_font_cook::{
    ProjectFontCodepointResolution, ProjectFontCookFailure, ProjectFontCookModule,
    ProjectFontCookOutput, ProjectFontCookRequest, ProjectFontFaceMetrics,
    ProjectFontHintedGlyphVariant, ProjectFontKerningAdjustment, ProjectFontMsdfGlyphVariant,
    ProjectFontProfileInventoryEntry, ProjectFontRuntimePackageCook, ProjectTextSourceAsset,
    PROJECT_FONT_COOK_OUTPUT_SCHEMA_VERSION, PROJECT_TEXT_SOURCE_SCHEMA_VERSION,
};
pub use project_intent_workflow::{
    CandidatePayloadKind, CandidatePlanStep, CandidateValidationProfile, ChangePreparationBlocker,
    ChangePreparationRequest, ChangePreparationResult, ChangeSetApproval, ChangeSetApprovalInput,
    ChangeSetProposal, ChangeSetTargetKind, DiagnosisHypothesis, DiagnosisState, DiagnosisSummary,
    DiagnosisUpdate, DiagnosticCapability, ImportedChangePlanSource, IntentCaptureInput,
    IntentCaptureReceipt, IntentEvent, IntentNormalizationProposal, IntentPrivacyClass,
    IntentSourceKind, ProductionStepSnapshot, ProductionStepState, ProjectCreateSpec,
    ProjectDiagnosisSession, ProjectGoalSnapshot, ProjectIntentJournalDocument,
    ProjectIntentJournalEntry, ProjectIntentJournalRecord, ProjectIntentProjectBinding,
    ProjectIntentQuery, ProjectIntentSnapshot, ProjectIntentStorageKind, ProjectIntentWorkflow,
    ProjectIntentWorkflowCommand, ProjectIntentWorkflowError, ProjectProductionRun,
    ProjectProductionRunKind, ProjectProductionRunState, SanitizedIntentEventContext,
    SanitizedProjectIntentContext, SanitizedWorkItemContext, WorkItem, WorkItemDraft, WorkItemKind,
    WorkItemPriority, WorkItemRelationship, WorkItemRelationshipKind, WorkItemRevisionBinding,
    WorkItemStatus, WorkItemSummary, CHANGE_SET_APPROVAL_SCHEMA_VERSION,
    CHANGE_SET_PROPOSAL_SCHEMA_VERSION, IMPORTED_CHANGE_PLAN_SOURCE_SCHEMA_VERSION,
    INTENT_EVENT_SCHEMA_VERSION, INTENT_NORMALIZATION_PROPOSAL_SCHEMA_VERSION,
    PROJECT_DIAGNOSIS_SCHEMA_VERSION, PROJECT_INTENT_JOURNAL_SCHEMA_VERSION,
    PROJECT_INTENT_PROJECT_BINDING_SCHEMA_VERSION, PROJECT_INTENT_SNAPSHOT_SCHEMA_VERSION,
    PROJECT_PRODUCTION_RUN_SCHEMA_VERSION, SANITIZED_PROJECT_INTENT_CONTEXT_SCHEMA_VERSION,
    WORK_ITEM_SCHEMA_VERSION,
};
pub use project_launcher::{
    validate_project_root, ProjectCreateCleanupOutcome, ProjectCreateError,
    ProjectCreateOwnedOutcome, ProjectLauncherEvent, ProjectLauncherEventKind,
    ProjectLauncherEventResult, ProjectLauncherState, ProjectManifest,
    ProjectRecentProjectsDocument, ProjectRecentStore, ProjectRuntimeModuleBuildSpec,
    ProjectRuntimeSourceKind, ProjectSession, ProjectTemplateDescriptor, ProjectTemplateRegistry,
    ProjectValidationStatus, EDITOR_RECENT_PROJECTS_SCHEMA_VERSION,
    LEGACY_PROJECT_MANIFEST_SCHEMA_VERSION, PROJECT_LAUNCHER_EVENT_SCHEMA_VERSION,
    PROJECT_MANIFEST_SCHEMA_VERSION, PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
    PROJECT_SETTINGS_SCHEMA_VERSION, PROJECT_TEMPLATE_REGISTRY_SCHEMA_VERSION,
};
pub use project_observation::*;
pub use project_open_preparation::*;
pub use project_patch::{
    build_project_patch_generation_prompt, build_project_patch_repair_prompt,
    diagnostic_fingerprint, import_diagnostics, project_patch_import_accepted,
    project_patch_json_schema, project_patch_json_schema_hash, project_patch_json_schema_string,
    repair_decision, summarize_patch_history, validate_llm_join_timeout_fail_closed,
    validate_repair_scope, AssetPatchOperation, AuiPatchOperation, BuildPatchOperation,
    CancelSource, CredentialOwnerStatus, InputBindingProcessorPatch, InputPatchOperation,
    LlmAsyncExecutor, LlmAttemptDecision, LlmCancelReceipt, LlmCredentialLease,
    LlmLifecycleDiagnostic, LlmLifecycleState, LlmLocalExecutionStatus, LlmPatchAttemptSummary,
    LlmPatchRepairScopeEvidence, LlmPatchReportLevel, LlmPatchRequestReport, LlmPatchSourceConfig,
    LlmPatchSourceKind, LlmPatchSourceResult, LlmPatchSourceStatus, LlmRemoteExecutionStatus,
    LlmRepairSpec, LlmRequestController, LlmRequestEvent, LlmRequestId, LlmRequestSpec,
    LlmShutdownReceipt, LlmStructuredOutputMode, LlmTaskJoinStatus, LlmTerminalStatus,
    LlmTransportCancelCapability, LlmTransportConfig, PatchApplier, PatchApplyReport,
    PatchApplyStatus, PatchCapability, PatchDiagnostic, PatchDiagnosticSeverity, PatchHistory,
    PatchHistoryEntry, PatchHistorySummary, PatchOperation, PatchOperationApplyStatus,
    PatchOperationResult, PatchReviewModel, PatchRiskLevel, PatchSource, PatchValidationReport,
    PatchValidator, PrefabPatchOperation, ProjectPatchDocument, ProjectPatchImportParseStatus,
    ProjectPatchImportProductizationReport, ProjectPatchImportProductizationStatus,
    ProjectPatchImportRequest, ProjectPatchImportResult, ProjectPatchImportService,
    ProjectPatchImportSourceKind, ProjectPatchLlmContextSnapshot, ProjectPatchProductizationReport,
    ProjectPatchProductizationStatus, RedactedSecret, RepairDecision, RepairScopePolicy,
    RepairScopeValidation, RepairScopeValidationStatus, RulePatchOperation, ScenePatchOperation,
    ThinLlmPatchSource, LLM_CANCEL_JOIN_DEADLINE, LLM_DROP_JOIN_BUDGET,
    LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION, LLM_SESSION_SHUTDOWN_DEADLINE,
    PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
    PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION, PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION,
    PROJECT_PATCH_LLM_CONTEXT_SCHEMA_VERSION, PROJECT_PATCH_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
    PROJECT_PATCH_SCHEMA_VERSION, REPAIR_SCOPE_UNPROVABLE_MAX_OPERATIONS,
};
pub use project_player_artifact::{
    default_engine_sdk_root, default_project_runtime_player_build_root, ProjectPlayerArtifact,
    ProjectPlayerArtifactError, ProjectRuntimePlayerArtifactBuildDiagnostic,
    ProjectRuntimePlayerArtifactBuildReport, ProjectRuntimePlayerArtifactBuildRequest,
    ProjectRuntimePlayerArtifactBuildStatus, ProjectRuntimePlayerArtifactBuildStep,
    PROJECT_PLAYER_ARTIFACT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REQUEST_SCHEMA_VERSION,
};
pub use project_preview_evidence::{
    ProjectPreviewCaptureKind, ProjectPreviewEvidence, ProjectPreviewEvidenceError,
    ProjectPreviewFrameCapture, ProjectPreviewFrameEvidence, ProjectPreviewFrameReadback,
    ProjectPreviewFrameResult, ProjectPreviewFrameResultStatus, ProjectPreviewFrameTicket,
    ProjectPreviewPixelFormat, PROJECT_PREVIEW_EVIDENCE_ROOT,
    PROJECT_PREVIEW_FRAME_EVIDENCE_SCHEMA_VERSION, PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION,
};
pub use project_readiness::{
    ProjectReadiness, ProjectReadinessCheck, ProjectReadinessCheckStatus, ProjectReadinessReport,
    ProjectReadinessStatus, PROJECT_READINESS_REPORT_SCHEMA_VERSION,
};
pub use project_runtime_native_module::{
    ProjectNativeModuleIdentity, ProjectRuntimeNativeModuleArtifact,
    ProjectRuntimeNativeModuleBuildControl, ProjectRuntimeNativeModuleBuildReport,
    ProjectRuntimeNativeModuleBuildRequest, ProjectRuntimeNativeModuleBuildStatus,
    ProjectRuntimeNativeModuleBuildStep, ProjectRuntimeNativeModuleBuilder,
    ProjectRuntimeNativeModuleCacheStatus, ProjectRuntimeNativeModuleDescriptor,
    ProjectRuntimeNativeModuleDiagnostic, ProjectRuntimeNativeModuleLoader,
    ProjectRuntimeNativeModuleSeal, PROJECT_RUNTIME_NATIVE_MODULE_ARTIFACT_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_BUILD_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_LOAD_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_SEAL_SCHEMA_VERSION,
};
pub use project_runtime_package_assembler::{
    BuildProfile, BuildProfileApplication, BuildProfileIconRef, BuildProfileRelease,
    BuildProfileValidationIssue, PrefabRuntimeBakeInstanceEntry, PrefabRuntimeBakeReport,
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyDiagnostic,
    ProjectRuntimePackageAssemblyDomain, ProjectRuntimePackageAssemblyReport,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyResult,
    ProjectRuntimePackageAssemblySeverity, ProjectRuntimePackageAssemblyStatus,
    ProjectRuntimeSourceMapping, BUILD_PROFILE_SCHEMA_VERSION, BUILD_PROFILE_SCHEMA_VERSION_V1,
    PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION,
};
pub use project_runtime_player_staging::ProjectRuntimePlayerDependencyIdentity;
pub use project_runtime_preparation::{
    ProjectRuntimePreparationBlocker, ProjectRuntimePreparationModule,
    ProjectRuntimePreparationState, ProjectRuntimePreparationTicket,
};
pub use project_runtime_trust::{
    ProjectRuntimeRepositoryTrustEntry, ProjectRuntimeRepositoryTrustPolicy,
    ProjectRuntimeTrustDecision, ProjectRuntimeTrustDecisionKind,
    ProjectRuntimeTrustDecisionSource, ProjectRuntimeTrustError, ProjectRuntimeTrustEvaluation,
    ProjectRuntimeTrustInspection, ProjectRuntimeTrustModule, ProjectRuntimeTrustRequest,
    ProjectRuntimeTrustStatus, PROJECT_RUNTIME_TRUST_DECISION_SCHEMA_VERSION,
};
pub use project_visual_diagnostics::*;
pub use project_write_scope::{
    ProjectDirectoryWriter, ProjectRelativePath, ProjectWriteError, ProjectWriteOperation,
    ProjectWriteOutcome, ProjectWriteReceipt, ProjectWriteScope,
};
pub use property_editing::{
    InspectorPluginDescriptor, PropertyColor, PropertyCurve, PropertyCurveKey, PropertyEditBuffer,
    PropertyEditCommand, PropertyEditCommandKind, PropertyEditCommitReport,
    PropertyEditCommitStatus, PropertyEditDiagnostic, PropertyEditDiagnosticSeverity,
    PropertyEditTarget, PropertyEditorKind, PropertyMetadata, PropertyNode, PropertyPath,
    PropertyRichText, PropertyRichTextSpan, PropertyTree, PropertyTreeSummary, PropertyValue,
    PropertyValueType, RichTextBuffer, TextCompositionState,
};
pub use release_package::{
    verify_release_package_directory, ReleasePackageApplicationReport, ReleasePackageBuildRequest,
    ReleasePackageBuilder, ReleasePackageDiagnostic, ReleasePackageEntrypointReport,
    ReleasePackageLayoutReport, ReleasePackagePayloadHashReport, ReleasePackagePlan,
    ReleasePackageReport, ReleasePackageReportLevel, ReleasePackageResourceReport,
    ReleasePackageRuntimeReport, ReleasePackageStatus, ReleasePackageVerification,
    ReleasePackageVerificationReport, RELEASE_PACKAGE_PLAN_SCHEMA_VERSION,
    RELEASE_PACKAGE_REPORT_RELATIVE_PATH, RELEASE_PACKAGE_REPORT_SCHEMA_VERSION,
};
pub use report_panel::{ReportProvider, ReportProviderContext, ReportRegistry};
pub use rule_authoring::{
    decode_rule_operation, decode_rule_statement, decode_rule_trigger, explain_rule_diagnostic,
    is_rule_asset_relative_path, scan_rule_asset_paths, RuleAuthoringEditCommand,
    RuleAuthoringService, RULE_AUTHORING_DEFAULT_GENERATED_ROOT,
};
pub use runtime_selection::{
    EntitySelectionSource, InspectorContextAnchor, RuntimePickStatus, RuntimeWorldPickReport,
    RuntimeWorldPickRequest, WorldPickCollector,
};
pub use scene_editing::{
    ColliderDebugDiagnostic, ColliderDebugDrawItem, ColliderDebugDrawList, ColliderDebugShape,
    EditorAssetRef, EditorMesh, EditorSceneComponent, EditorSceneDocument, EditorSceneEntity,
    EditorTransform, EditorVec3, PreviewWorldSync, PreviewWorldSyncReport, SceneDirtyState,
    SceneEditCommand, SceneEditDiagnostic, SceneEditDiagnosticSeverity, SceneEditRequest,
    SceneEditRequestSource, SceneEditTransaction, SceneEditTransactionReport,
    SceneEditTransactionStatus, SceneSavePipeline, SceneSaveReport, SceneSaveStatus,
    SceneSelection, SceneUndoRecord, SceneUndoStack, EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION,
    PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION, SCENE_EDIT_TRANSACTION_REPORT_SCHEMA_VERSION,
};
pub use services::aui_service::{
    AuiDocumentAuthoringProductizationReport, AuiDocumentAuthoringProductizationStatus,
    AUI_DOCUMENT_AUTHORING_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
};
pub use services::build_service::{
    EditorBuildAndRunArtifact, EditorBuildAndRunDesktopExportSummary, EditorBuildAndRunDiagnostic,
    EditorBuildAndRunDiagnosticSeverity, EditorBuildAndRunDurationSummary,
    EditorBuildAndRunLaunchSummary, EditorBuildAndRunMode, EditorBuildAndRunReport,
    EditorBuildAndRunStatus, EditorBuildAndRunVerificationSummary,
    EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION,
};
pub use services::play_service::{EditorPlayPreparationError, EditorPlayPreparationTicket};
pub use session::EditorSession;
pub use transaction::{
    command_for_test, CommandResult, CommandStatus, CommandTransaction, StateChangeSummary,
    UndoPolicy,
};
pub use windows_executable_resources::{
    read_windows_executable_resources, resolve_release_icon_asset,
    stamp_windows_executable_resources, verify_windows_executable_resource_contract,
    ResolvedReleaseIcon, WindowsExecutableResourceError, WindowsExecutableResourceExpectation,
    WindowsExecutableResourceReadback, WINDOWS_APPLICATION_ICON_SIZES,
};

#[cfg(test)]
mod tests;
