use crate::{
    command_for_test, AssetDatabaseRecord, AssetImportConflictPolicy, AssetImportSourceKind,
    AssetImportSourceMetadata, AssetLicenseKind, AssetLicenseMetadata, CommandStatus,
    EditorSession, PlaySessionState, ProjectAssetImport, ProjectAssetImportApplyReceipt,
    ProjectAssetImportApplyRequest, ProjectAssetImportApproval, ProjectAssetImportPrepareRequest,
    TextureImportSettings, PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION,
};
use editor_ui_model::{AssetPlacementMode, UiCommandPayload, Vec3};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const AI_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION: &str = "ai-image-generation-request.v1";
pub const GENERATED_IMAGE_SOURCE_SCHEMA_VERSION: &str = "generated-image-source.v1";
pub const AI_IMAGE_GENERATION_RESULT_SCHEMA_VERSION: &str = "ai-image-generation-result.v1";
pub const GENERATED_IMAGE_METADATA_SCHEMA_VERSION: &str = "generated-image-metadata.v1";
pub const AI_IMAGE_GENERATION_LOOP_REPORT_SCHEMA_VERSION: &str =
    "ai-image-generation-loop-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageKind {
    #[serde(rename = "texture")]
    Texture,
    #[serde(rename = "sprite")]
    Sprite,
    #[serde(rename = "uiImage")]
    UiImage,
    #[serde(rename = "referenceImage")]
    ReferenceImage,
}

impl ImageKind {
    pub fn asset_type(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Sprite => "sprite",
            Self::UiImage => "uiImage",
            Self::ReferenceImage => "referenceImage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiImageGenerationStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiImageGenerationDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageGenerationDiagnostic {
    pub severity: AiImageGenerationDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

impl AiImageGenerationDiagnostic {
    pub fn error(
        code: impl Into<String>,
        source_stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: AiImageGenerationDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            source_stage: source_stage.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageGenerationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub prompt: String,
    pub reference_image_paths: Vec<PathBuf>,
    pub target_folder: PathBuf,
    pub asset_name: String,
    pub image_kind: ImageKind,
    pub width: u32,
    pub height: u32,
    pub transparent_background: bool,
}

impl AiImageGenerationRequest {
    pub fn new(
        request_id: impl Into<String>,
        prompt: impl Into<String>,
        target_folder: impl Into<PathBuf>,
        asset_name: impl Into<String>,
        image_kind: ImageKind,
    ) -> Self {
        Self {
            schema_version: AI_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION.to_string(),
            request_id: request_id.into(),
            prompt: prompt.into(),
            reference_image_paths: Vec::new(),
            target_folder: target_folder.into(),
            asset_name: asset_name.into(),
            image_kind,
            width: 16,
            height: 16,
            transparent_background: false,
        }
    }

    pub fn validate(&self, project_root: impl AsRef<Path>) -> Vec<AiImageGenerationDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema_version != AI_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION {
            diagnostics.push(AiImageGenerationDiagnostic::error(
                "ai_image_generation.schema_version_invalid",
                "validate_request",
                "AI image generation request schema version is not supported.",
            ));
        }
        if self.prompt.trim().is_empty() {
            diagnostics.push(AiImageGenerationDiagnostic::error(
                "ai_image_generation.prompt_required",
                "validate_request",
                "Prompt is required before generating an image.",
            ));
        }
        if sanitize_asset_name(&self.asset_name).is_empty() {
            diagnostics.push(AiImageGenerationDiagnostic::error(
                "ai_image_generation.asset_name_invalid",
                "validate_request",
                "Asset name must contain at least one safe filename character.",
            ));
        }
        if self.width == 0 || self.height == 0 || self.width > 4096 || self.height > 4096 {
            diagnostics.push(AiImageGenerationDiagnostic::error(
                "ai_image_generation.image_size_invalid",
                "validate_request",
                "Image width and height must be between 1 and 4096.",
            ));
        }
        if resolve_project_path(project_root.as_ref(), &self.target_folder).is_none() {
            diagnostics.push(AiImageGenerationDiagnostic::error(
                "ai_image_generation.target_folder_outside_project",
                "validate_request",
                "Target folder must resolve inside the current project root.",
            ));
        }
        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImageSource {
    pub schema_version: String,
    pub source_id: String,
    pub request_id: String,
    pub path: PathBuf,
    pub image_kind: ImageKind,
    pub content_hash: String,
    pub metadata_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageGenerationResult {
    pub schema_version: String,
    pub request_id: String,
    pub status: AiImageGenerationStatus,
    pub generated_images: Vec<GeneratedImageSource>,
    pub imported_assets: Vec<AssetDatabaseRecord>,
    pub diagnostics: Vec<AiImageGenerationDiagnostic>,
}

impl AiImageGenerationResult {
    pub fn failed(
        request_id: impl Into<String>,
        diagnostics: Vec<AiImageGenerationDiagnostic>,
    ) -> Self {
        Self {
            schema_version: AI_IMAGE_GENERATION_RESULT_SCHEMA_VERSION.to_string(),
            request_id: request_id.into(),
            status: AiImageGenerationStatus::Failed,
            generated_images: Vec::new(),
            imported_assets: Vec::new(),
            diagnostics,
        }
    }

    pub fn succeeded(
        request_id: impl Into<String>,
        generated_images: Vec<GeneratedImageSource>,
    ) -> Self {
        Self {
            schema_version: AI_IMAGE_GENERATION_RESULT_SCHEMA_VERSION.to_string(),
            request_id: request_id.into(),
            status: AiImageGenerationStatus::Succeeded,
            generated_images,
            imported_assets: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImageMetadata {
    pub schema_version: String,
    pub request_id: String,
    pub prompt: String,
    pub reference_image_paths: Vec<PathBuf>,
    pub image_kind: ImageKind,
    pub width: u32,
    pub height: u32,
    pub transparent_background: bool,
    pub provider_id: String,
    pub created_at: String,
}

pub trait ImageGenerationProvider {
    fn provider_id(&self) -> &'static str;

    fn generate_image(
        &self,
        project_root: impl AsRef<Path>,
        request: &AiImageGenerationRequest,
    ) -> AiImageGenerationResult;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MockImageGenerationProvider;

impl ImageGenerationProvider for MockImageGenerationProvider {
    fn provider_id(&self) -> &'static str {
        "mock-image-generation-provider.v1"
    }

    fn generate_image(
        &self,
        project_root: impl AsRef<Path>,
        request: &AiImageGenerationRequest,
    ) -> AiImageGenerationResult {
        let project_root = project_root.as_ref();
        let diagnostics = request.validate(project_root);
        if !diagnostics.is_empty() {
            return AiImageGenerationResult::failed(request.request_id.clone(), diagnostics);
        }

        let Some(_) = resolve_project_path(project_root, &request.target_folder) else {
            return AiImageGenerationResult::failed(
                request.request_id.clone(),
                vec![AiImageGenerationDiagnostic::error(
                    "ai_image_generation.target_folder_outside_project",
                    "generate_image",
                    "Target folder must resolve inside the current project root.",
                )],
            );
        };
        let scope = match crate::ProjectWriteScope::open(project_root) {
            Ok(scope) => scope,
            Err(error) => {
                return AiImageGenerationResult::failed(
                    request.request_id.clone(),
                    vec![AiImageGenerationDiagnostic::error(
                        error.code,
                        "generate_image",
                        error.to_string(),
                    )],
                );
            }
        };
        let request_folder = sanitize_asset_name(&request.request_id);
        let relative_folder =
            PathBuf::from("Library")
                .join("GeneratedSources")
                .join(if request_folder.is_empty() {
                    "request"
                } else {
                    request_folder.as_str()
                });
        if let Err(error) = scope.create_dir_all(&relative_folder) {
            return AiImageGenerationResult::failed(
                request.request_id.clone(),
                vec![AiImageGenerationDiagnostic::error(
                    "ai_image_generation.create_target_folder_failed",
                    "generate_image",
                    format!("Failed to create target folder: {error}"),
                )],
            );
        }

        let safe_name = sanitize_asset_name(&request.asset_name);
        let target_folder = project_root.join(&relative_folder);
        let image_path = target_folder.join(format!("{safe_name}.png"));
        let metadata_path = target_folder.join(format!("{safe_name}.ai.json"));
        let image_relative_path = relative_folder.join(format!("{safe_name}.png"));
        let metadata_relative_path = relative_folder.join(format!("{safe_name}.ai.json"));
        let rgba = if request.transparent_background {
            [40, 120, 220, 0]
        } else {
            [40, 120, 220, 255]
        };
        let png = encode_solid_rgba_png(request.width, request.height, rgba);
        let content_hash = stable_hash_hex(&png);
        if let Err(error) = scope.write_atomic(&image_relative_path, &png) {
            return AiImageGenerationResult::failed(
                request.request_id.clone(),
                vec![AiImageGenerationDiagnostic::error(
                    "ai_image_generation.write_png_failed",
                    "generate_image",
                    format!("Failed to write generated png: {error}"),
                )],
            );
        }

        let metadata = GeneratedImageMetadata {
            schema_version: GENERATED_IMAGE_METADATA_SCHEMA_VERSION.to_string(),
            request_id: request.request_id.clone(),
            prompt: request.prompt.clone(),
            reference_image_paths: request.reference_image_paths.clone(),
            image_kind: request.image_kind,
            width: request.width,
            height: request.height,
            transparent_background: request.transparent_background,
            provider_id: self.provider_id().to_string(),
            created_at: "mock-time-0".to_string(),
        };
        let metadata_json = match serde_json::to_vec_pretty(&metadata) {
            Ok(json) => json,
            Err(error) => {
                return AiImageGenerationResult::failed(
                    request.request_id.clone(),
                    vec![AiImageGenerationDiagnostic::error(
                        "ai_image_generation.serialize_metadata_failed",
                        "generate_image",
                        format!("Failed to serialize generated image metadata: {error}"),
                    )],
                );
            }
        };
        if let Err(error) = scope.write_atomic(&metadata_relative_path, &metadata_json) {
            return AiImageGenerationResult::failed(
                request.request_id.clone(),
                vec![AiImageGenerationDiagnostic::error(
                    "ai_image_generation.write_metadata_failed",
                    "generate_image",
                    format!("Failed to write generated image metadata: {error}"),
                )],
            );
        }

        let source = GeneratedImageSource {
            schema_version: GENERATED_IMAGE_SOURCE_SCHEMA_VERSION.to_string(),
            source_id: format!("generated-image-{content_hash}"),
            request_id: request.request_id.clone(),
            path: image_path,
            image_kind: request.image_kind,
            content_hash,
            metadata_path,
        };
        AiImageGenerationResult::succeeded(request.request_id.clone(), vec![source])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImageImportResult {
    pub record: AssetDatabaseRecord,
    pub apply_receipt: ProjectAssetImportApplyReceipt,
}

pub fn import_generated_image_formally(
    project_root: impl AsRef<Path>,
    source: &GeneratedImageSource,
    target_folder: impl AsRef<Path>,
    approved_by: impl Into<String>,
) -> Result<GeneratedImageImportResult, AiImageGenerationDiagnostic> {
    if source.schema_version != GENERATED_IMAGE_SOURCE_SCHEMA_VERSION {
        return Err(AiImageGenerationDiagnostic::error(
            "asset_pipeline.generated_source_schema_invalid",
            "import_image",
            "Generated image source schema version is not supported.",
        ));
    }
    let project_root = project_root.as_ref();
    let Some(source_path) = resolve_project_path(project_root, &source.path) else {
        return Err(AiImageGenerationDiagnostic::error(
            "asset_pipeline.source_outside_project",
            "import_image",
            "Generated image source must be inside the current project root.",
        ));
    };
    let metadata_path =
        resolve_project_path(project_root, &source.metadata_path).ok_or_else(|| {
            AiImageGenerationDiagnostic::error(
                "asset_pipeline.metadata_outside_project",
                "import_image",
                "Generated image metadata must be inside the current project root.",
            )
        })?;
    let metadata: GeneratedImageMetadata =
        serde_json::from_slice(&fs::read(&metadata_path).map_err(|error| {
            AiImageGenerationDiagnostic::error(
                "asset_pipeline.metadata_missing",
                "import_image",
                format!("Generated image metadata cannot be read: {error}"),
            )
        })?)
        .map_err(|error| {
            AiImageGenerationDiagnostic::error(
                "asset_pipeline.metadata_invalid",
                "import_image",
                format!("Generated image metadata cannot be decoded: {error}"),
            )
        })?;
    if metadata.schema_version != GENERATED_IMAGE_METADATA_SCHEMA_VERSION
        || metadata.request_id != source.request_id
        || metadata.image_kind != source.image_kind
    {
        return Err(AiImageGenerationDiagnostic::error(
            "asset_pipeline.metadata_binding_mismatch",
            "import_image",
            "Generated image metadata does not bind the recorded provider source.",
        ));
    }
    let metadata_relative = metadata_path
        .strip_prefix(lexical_normalize(project_root))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| source.metadata_path.display().to_string());
    let source_bytes = fs::read(&source_path).map_err(|error| {
        AiImageGenerationDiagnostic::error(
            "asset_pipeline.source_missing",
            "import_image",
            format!("Generated image source cannot be read: {error}"),
        )
    })?;
    if stable_hash_hex(&source_bytes) != source.content_hash {
        return Err(AiImageGenerationDiagnostic::error(
            "asset_pipeline.generated_source_drifted",
            "import_image",
            "Generated image source changed after provider output was recorded.",
        ));
    }
    let extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("png") {
        return Err(AiImageGenerationDiagnostic::error(
            "asset_pipeline.unsupported_image_source",
            "import_image",
            "Formal generated-image import v1 only supports PNG sources.",
        ));
    }
    let asset_id = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .map(sanitize_asset_name)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            AiImageGenerationDiagnostic::error(
                "asset_pipeline.asset_id_invalid",
                "import_image",
                "Generated source does not produce a valid asset id.",
            )
        })?;
    let target_folder =
        resolve_project_path(project_root, target_folder.as_ref()).ok_or_else(|| {
            AiImageGenerationDiagnostic::error(
                "asset_pipeline.target_outside_project",
                "import_image",
                "Formal import target must resolve inside the current project root.",
            )
        })?;
    let normalized_root = lexical_normalize(project_root);
    let target_relative = target_folder
        .strip_prefix(&normalized_root)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .ok_or_else(|| {
            AiImageGenerationDiagnostic::error(
                "asset_pipeline.target_invalid",
                "import_image",
                "Formal import target must name a project child directory.",
            )
        })?;
    let transaction_seed = format!("{}\0{}\0{}", source.source_id, source.request_id, asset_id);
    let transaction_id = stable_hash_hex(transaction_seed.as_bytes());
    let candidate_store_root = generated_candidate_store_root(project_root)?;
    let candidate = ProjectAssetImport::prepare(ProjectAssetImportPrepareRequest {
        import_id: format!("ai-image-{transaction_id}"),
        revision_id: format!("ai-image-{transaction_id}"),
        project_root: project_root.to_path_buf(),
        candidate_store_root,
        source_path,
        target_directory: target_relative,
        asset_id,
        display_name: source.source_id.clone(),
        conflict_policy: AssetImportConflictPolicy::RejectExisting,
        source_metadata: AssetImportSourceMetadata {
            kind: AssetImportSourceKind::AiGenerated,
            source_uri: None,
            creator: Some("image-generation-provider".to_string()),
            note: Some(format!(
                "Generated request {}; provider {}; declared image kind {}; metadata {}.",
                source.request_id,
                metadata.provider_id,
                source.image_kind.asset_type(),
                metadata_relative
            )),
        },
        license: AssetLicenseMetadata {
            kind: AssetLicenseKind::Unknown,
            identifier: None,
            license_uri: None,
            attribution: None,
            note: Some(
                "AI-generated output; this declaration does not determine distribution rights."
                    .to_string(),
            ),
        },
        texture_settings: TextureImportSettings::default(),
    })
    .map_err(asset_import_diagnostic)?;
    let validation_report =
        ProjectAssetImport::validate(&candidate).map_err(asset_import_diagnostic)?;
    let record = candidate.record.clone();
    let approval = ProjectAssetImportApproval {
        schema_version: PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION.to_string(),
        approved_by: approved_by.into(),
        candidate_digest: candidate.candidate_digest.clone(),
        validation_digest: validation_report.validation_digest.clone(),
        allow_replace: false,
    };
    let apply_receipt = ProjectAssetImport::apply(ProjectAssetImportApplyRequest {
        candidate,
        validation_report,
        approval,
    })
    .map_err(asset_import_diagnostic)?;
    Ok(GeneratedImageImportResult {
        record,
        apply_receipt,
    })
}

fn generated_candidate_store_root(
    project_root: &Path,
) -> Result<PathBuf, AiImageGenerationDiagnostic> {
    let canonical = fs::canonicalize(project_root).map_err(|error| {
        AiImageGenerationDiagnostic::error(
            "asset_pipeline.project_root_invalid",
            "import_image",
            format!("Project root cannot be resolved: {error}"),
        )
    })?;
    let parent = canonical.parent().ok_or_else(|| {
        AiImageGenerationDiagnostic::error(
            "asset_pipeline.candidate_store_unavailable",
            "import_image",
            "Project root has no parent for an isolated candidate store.",
        )
    })?;
    let project_key = stable_hash_hex(canonical.to_string_lossy().as_bytes());
    Ok(parent
        .join(".aife-candidates")
        .join(format!("ai-image-{project_key}")))
}

fn asset_import_diagnostic(error: crate::ProjectAssetImportError) -> AiImageGenerationDiagnostic {
    AiImageGenerationDiagnostic::error(error.code, "import_image", error.message)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageGenerationLoopReport {
    pub schema_version: String,
    pub request_id: String,
    pub prompt: String,
    pub generated_image_path: Option<PathBuf>,
    pub metadata_path: Option<PathBuf>,
    pub imported_asset_id: Option<String>,
    pub imported_asset_type: Option<String>,
    pub placed_entity_id: Option<String>,
    pub dirty_before_save: Option<bool>,
    pub dirty_after_save: Option<bool>,
    pub play_finished: bool,
    pub diagnostics: Vec<AiImageGenerationDiagnostic>,
}

impl AiImageGenerationLoopReport {
    pub fn new(request: &AiImageGenerationRequest) -> Self {
        Self {
            schema_version: AI_IMAGE_GENERATION_LOOP_REPORT_SCHEMA_VERSION.to_string(),
            request_id: request.request_id.clone(),
            prompt: request.prompt.clone(),
            generated_image_path: None,
            metadata_path: None,
            imported_asset_id: None,
            imported_asset_type: None,
            placed_entity_id: None,
            dirty_before_save: None,
            dirty_after_save: None,
            play_finished: false,
            diagnostics: Vec::new(),
        }
    }
}

pub fn run_ai_image_generation_loop_headless() -> AiImageGenerationLoopReport {
    let fixture = crate::create_default_editable_project_fixture();
    let request = AiImageGenerationRequest::new(
        "ai-image-request-1",
        "blue player ship sprite",
        fixture.root_dir.join("Assets").join("Generated"),
        "blue-player-ship",
        ImageKind::Sprite,
    );
    let mut report = AiImageGenerationLoopReport::new(&request);
    let mut session = EditorSession::new();
    let project = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: fixture.root_dir.display().to_string(),
    }));
    if project.status != CommandStatus::Committed {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.open_project_failed",
            "open_project",
            "Default editable project scope did not open.",
        ));
        return report;
    }
    let provider = MockImageGenerationProvider;
    let mut generation = provider.generate_image(&fixture.root_dir, &request);
    if generation.status != AiImageGenerationStatus::Succeeded {
        report.diagnostics.extend(generation.diagnostics);
        return report;
    }
    let Some(source) = generation.generated_images.first().cloned() else {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.no_generated_image",
            "generate_image",
            "Provider succeeded without returning a generated image.",
        ));
        return report;
    };
    report.generated_image_path = Some(source.path.clone());
    report.metadata_path = Some(source.metadata_path.clone());

    let imported = match import_generated_image_formally(
        &fixture.root_dir,
        &source,
        "Assets/Generated",
        "headless-ai-image-loop",
    ) {
        Ok(result) => result.record,
        Err(diagnostic) => {
            report.diagnostics.push(diagnostic);
            return report;
        }
    };
    generation.imported_assets.push(imported.clone());
    report.imported_asset_id = Some(imported.asset_id.clone());
    report.imported_asset_type = Some(imported.asset_type.clone());

    let open = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: fixture.scene_path.display().to_string(),
    }));
    if open.status != CommandStatus::Committed {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.open_scene_failed",
            "open_scene",
            "Default editable scene did not open.",
        ));
        return report;
    }

    let place = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: imported.asset_id.clone(),
        asset_type: imported.asset_type.clone(),
        asset_guid: Some(imported.asset_guid.clone()),
        target_parent_id: None,
        local_position: Some(Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }),
        placement_mode: AssetPlacementMode::WorldOrigin,
    }));
    if place.status != CommandStatus::Committed {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.place_asset_failed",
            "place_asset",
            "Generated image asset could not be placed into Scene.",
        ));
        return report;
    }
    let model = session.build_ui_model();
    report.placed_entity_id = model.hierarchy.selected_entity_id.clone();
    report.dirty_before_save = session.scene_dirty();

    let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    if save.status != CommandStatus::Committed {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.save_scene_failed",
            "save_scene",
            "Scene did not save after placing generated image.",
        ));
    }
    report.dirty_after_save = session.scene_dirty();

    let mut runtime_session = EditorSession::new();
    let runtime_open =
        runtime_session.execute_command(command_for_test(UiCommandPayload::OpenRuntimePackage {
            path: fixture.runtime_package_dir.display().to_string(),
        }));
    if runtime_open.status != CommandStatus::Committed {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.open_runtime_package_failed",
            "open_runtime_package",
            "Runtime package fixture did not open.",
        ));
        return report;
    }

    let play = runtime_session.execute_command(command_for_test(UiCommandPayload::Play));
    report.play_finished = play.status == CommandStatus::Committed
        && runtime_session
            .last_play_session_report()
            .is_some_and(|play_report| play_report.state == PlaySessionState::Completed);
    if !report.play_finished {
        report.diagnostics.push(AiImageGenerationDiagnostic::error(
            "ai_image_generation.play_failed",
            "play_session",
            "Play current scene did not complete.",
        ));
    }
    report
}

fn sanitize_asset_name(value: &str) -> String {
    let mut safe = String::new();
    let mut last_was_dash = false;
    for ch in value.trim().chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch == '_' || ch == '-' || ch.is_whitespace() {
            Some('-')
        } else {
            None
        };
        if let Some(ch) = normalized {
            if ch == '-' {
                if !last_was_dash && !safe.is_empty() {
                    safe.push(ch);
                    last_was_dash = true;
                }
            } else {
                safe.push(ch);
                last_was_dash = false;
            }
        }
    }
    safe.trim_matches('-').to_string()
}

fn resolve_project_path(project_root: &Path, path: &Path) -> Option<PathBuf> {
    let root = lexical_normalize(project_root);
    let candidate = if path.is_absolute() {
        lexical_normalize(path)
    } else {
        lexical_normalize(&root.join(path))
    };
    candidate.starts_with(&root).then_some(candidate)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(value) => normalized.push(value),
        }
    }
    normalized
}

fn stable_hash_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn encode_solid_rgba_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((height as usize) * (1 + width as usize * 4));
    for _ in 0..height {
        raw.push(0);
        for _ in 0..width {
            raw.extend_from_slice(&rgba);
        }
    }
    let compressed = zlib_store_blocks(&raw);
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &compressed);
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn zlib_store_blocks(data: &[u8]) -> Vec<u8> {
    let mut zlib = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(u16::MAX as usize);
        let is_final = offset + block_len == data.len();
        zlib.push(if is_final { 0x01 } else { 0x00 });
        let len = block_len as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(&data[offset..offset + block_len]);
        offset += block_len;
    }
    zlib.extend_from_slice(&adler32(data).to_be_bytes());
    zlib
}

fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(kind.len() + data.len());
    crc_data.extend_from_slice(kind);
    crc_data.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_image_generation_data_serializes_and_validates() {
        let project_root = test_project_root("data");
        let request = AiImageGenerationRequest::new(
            "req-1",
            "blue ship",
            project_root.join("Project Library").join("Generated"),
            "Blue Ship",
            ImageKind::Sprite,
        );
        assert!(request.validate(&project_root).is_empty());
        let json = serde_json::to_string(&request).expect("request serializes");
        assert!(json.contains("ai-image-generation-request.v1"));
        assert!(json.contains("\"sprite\""));

        let mut empty_prompt = request.clone();
        empty_prompt.prompt = " ".to_string();
        assert!(empty_prompt
            .validate(&project_root)
            .iter()
            .any(|diagnostic| diagnostic.code == "ai_image_generation.prompt_required"));

        let mut outside = request.clone();
        outside.target_folder = project_root.join("..").join("outside");
        assert!(outside.validate(&project_root).iter().any(|diagnostic| {
            diagnostic.code == "ai_image_generation.target_folder_outside_project"
        }));

        let invalid_kind = r#""notAnImageKind""#;
        assert!(serde_json::from_str::<ImageKind>(invalid_kind).is_err());
    }

    #[test]
    fn mock_image_generation_provider_generates_png_and_metadata() {
        let project_root = test_project_root("mock-provider");
        let request = AiImageGenerationRequest::new(
            "req-1",
            "blue ship",
            project_root.join("Project Library").join("Generated"),
            "Blue Ship",
            ImageKind::Sprite,
        );
        let provider = MockImageGenerationProvider;

        let first = provider.generate_image(&project_root, &request);
        let second = provider.generate_image(&project_root, &request);

        assert_eq!(first.status, AiImageGenerationStatus::Succeeded);
        assert!(first.imported_assets.is_empty());
        let first_source = first.generated_images.first().expect("generated image");
        let second_source = second.generated_images.first().expect("generated image");
        assert!(first_source.path.exists());
        assert!(first_source.metadata_path.exists());
        assert_eq!(first_source.path, second_source.path);
        assert_eq!(first_source.content_hash, second_source.content_hash);
    }

    #[test]
    fn ai_image_generation_imports_generated_image() {
        let project_root = test_project_root("import");
        let source = generate_source(&project_root, ImageKind::Sprite);
        let record = import_generated_image_formally(
            &project_root,
            &source,
            "Assets/Generated",
            "test-maintainer",
        )
        .expect("generated image imports")
        .record;

        assert_eq!(record.asset_type, "texture");
        assert!(project_root.join(&record.descriptor_path).is_file());
        assert!(project_root.join(&record.meta_path).is_file());

        let texture_source = generate_source(&project_root, ImageKind::Texture);
        let texture = import_generated_image_formally(
            &project_root,
            &texture_source,
            "Assets/Generated",
            "test-maintainer",
        )
        .expect("texture imports")
        .record;
        assert_eq!(texture.asset_type, "texture");
        assert_eq!(
            ProjectAssetImport::load_database(&project_root)
                .unwrap()
                .unwrap()
                .assets
                .len(),
            2
        );
    }

    #[test]
    fn ai_image_generation_project_dock_visible() {
        let project_root = test_project_root("project-dock");
        let source = generate_source(&project_root, ImageKind::Sprite);
        let record = import_generated_image_formally(
            &project_root,
            &source,
            "Assets/Generated",
            "test-maintainer",
        )
        .expect("generated image imports")
        .record;

        let model = crate::AssetBrowserIndex::build(crate::AssetBrowserBuildRequest {
            project_root: project_root.clone(),
            query: editor_ui_model::AssetQuery::default(),
            selection: editor_ui_model::AssetSelection::default(),
        });
        let entry = model
            .entries
            .iter()
            .find(|entry| entry.asset_id.as_deref() == Some(record.asset_id.as_str()))
            .expect("formal descriptor is visible");
        assert_eq!(entry.guid.as_deref(), Some(record.asset_guid.as_str()));
        assert!(!model
            .entries
            .iter()
            .any(|entry| entry.path.ends_with(".meta.json")));
    }

    #[test]
    fn ai_image_generation_to_scene_authoring_loop() {
        let fixture = crate::create_default_editable_project_fixture();
        let mut session = EditorSession::new();
        let project = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
            path: fixture.root_dir.display().to_string(),
        }));
        assert_eq!(project.status, CommandStatus::Committed);
        let source = generate_source(&fixture.root_dir, ImageKind::Sprite);
        let record = import_generated_image_formally(
            &fixture.root_dir,
            &source,
            "Assets/Generated",
            "test-maintainer",
        )
        .expect("generated image imports")
        .record;
        let open = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
            path: fixture.scene_path.display().to_string(),
        }));
        assert_eq!(open.status, CommandStatus::Committed);

        let place =
            session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
                asset_id: record.asset_id.clone(),
                asset_type: record.asset_type.clone(),
                asset_guid: Some(record.asset_guid.clone()),
                target_parent_id: None,
                local_position: None,
                placement_mode: AssetPlacementMode::WorldOrigin,
            }));
        assert_eq!(place.status, CommandStatus::Committed);

        let model = session.build_ui_model();
        let selected = model
            .hierarchy
            .selected_entity_id
            .expect("placed entity selected");
        assert!(model
            .hierarchy
            .roots
            .iter()
            .any(|node| node.entity_id == selected));
        assert!(model.inspector.sections.iter().any(|section| {
            section.section_id == "mesh"
                && section.fields.iter().any(|field| {
                    matches!(&field.value, editor_ui_model::InspectorValue::AssetRef(value) if value.asset_id == record.asset_id)
                })
        }));
        assert_eq!(session.scene_dirty(), Some(true));

        let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
            path: None,
        }));
        assert_eq!(save.status, CommandStatus::Committed);
        assert_eq!(session.scene_dirty(), Some(false));
    }

    #[test]
    fn ai_image_generation_loop() {
        let report = run_ai_image_generation_loop_headless();
        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(json.contains(AI_IMAGE_GENERATION_LOOP_REPORT_SCHEMA_VERSION));
        assert!(report.generated_image_path.is_some());
        assert!(report.metadata_path.is_some());
        assert_eq!(report.imported_asset_type.as_deref(), Some("texture"));
        assert!(report.placed_entity_id.is_some());
        assert_eq!(report.dirty_before_save, Some(true));
        assert_eq!(report.dirty_after_save, Some(false));
        assert!(report.play_finished);
        assert!(report.diagnostics.is_empty());
    }

    fn generate_source(project_root: &Path, image_kind: ImageKind) -> GeneratedImageSource {
        let request = AiImageGenerationRequest::new(
            format!("req-{}", image_kind.asset_type()),
            "blue ship",
            project_root.join("Project Library").join("Generated"),
            format!("blue-ship-{}", image_kind.asset_type()),
            image_kind,
        );
        let provider = MockImageGenerationProvider;
        provider
            .generate_image(project_root, &request)
            .generated_images
            .into_iter()
            .next()
            .expect("source generated")
    }

    fn test_project_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("ai-image-generation-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        crate::ProjectLauncherState::new("0.0.3")
            .create_project(&root, "AI Image Test")
            .expect("test project created");
        if let Ok(store) = generated_candidate_store_root(&root) {
            let _ = fs::remove_dir_all(store);
        }
        root
    }
}
