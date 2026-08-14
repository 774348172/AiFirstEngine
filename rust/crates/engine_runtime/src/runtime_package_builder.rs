use crate::animator2d::CookedAnimator2DRegistry;
use crate::atomic_directory_publish::{
    atomic_directory_publish_with_fault, AtomicDirectoryPublishError, AtomicDirectoryPublishFault,
};
use crate::canonical_digest::{
    canonical_json_bytes, payload_tree_digest, sha256_prefixed, CanonicalDigestError,
    ConsistencyDigest,
};
use crate::font_bundle::{
    RuntimeFontBundleManifest, RuntimeFontBundleManifestEntry, RuntimePackageSourceFontBundle,
    RUNTIME_FONT_BUNDLE_MANIFEST_SCHEMA_VERSION,
};
use crate::project_observation::ProjectObservationContract;
use crate::runtime_asset::{
    BundleRecord, CookedAssetRecord, RuntimeAssetDependencyRecord, RuntimeAssetRecord,
};
use crate::runtime_package::{
    load_runtime_package, CookedFontAtlasAsset, CookedTextureAsset, RuntimeAsset,
    RuntimeAssetManifest, RuntimeAssetRef, RuntimeAuiManifest, RuntimeEntity,
    RuntimeFontAtlasManifest, RuntimeFontAtlasManifestEntry, RuntimeInputManifest,
    RuntimeInputMappingManifestEntry, RuntimeManifestAssetIndex, RuntimeManifestAuiIndex,
    RuntimeManifestFontAtlasIndex, RuntimeManifestInputIndex, RuntimeManifestRuleIndex,
    RuntimePackageManifest, RuntimePrefabData, RuntimeProjectInfo, RuntimeRuleManifest,
    RuntimeScene, RuntimeSceneManifestEntry, RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION,
    RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION, RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION,
    RUNTIME_PACKAGE_MODE, RUNTIME_PACKAGE_SCHEMA_VERSION, RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
};
use crate::runtime_package_path::{
    safe_join_runtime_package, validate_package_path_segment, RuntimePackagePath,
    RuntimePackagePathClaims,
};
#[cfg(test)]
use engine_input::InputMappingAsset;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const BUILD_RUNTIME_PACKAGE_REPORT_SCHEMA_VERSION: &str = "build-runtime-package-report.v1";
pub const RUNTIME_PACKAGE_VALIDATION_REPORT_SCHEMA_VERSION: &str =
    "runtime-package-validation-report.v1";
pub const RUNTIME_PACKAGE_DIFF_REPORT_SCHEMA_VERSION: &str = "runtime-package-diff-report.v1";
pub const RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION: &str = "runtime-package-build-request.v1";
pub const CANONICAL_ENCODE_FAILED_DIAGNOSTIC: &str = "canonical_encode_failed";
pub const DIGEST_INSENSITIVE_DIAGNOSTIC: &str = "digest_insensitive";
pub const DIGEST_SCOPE_VIOLATION_DIAGNOSTIC: &str = "digest_scope_violation";
pub const UNSUPPORTED_CONTENT_HASH_ALGORITHM_DIAGNOSTIC: &str =
    "unsupported_content_hash_algorithm";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageBuildRequest {
    pub schema_version: String,
    pub project_root: PathBuf,
    pub active_scene_id: String,
    pub target: String,
    pub mode: String,
    pub output_dir: PathBuf,
    pub previous_package_manifest: Option<PreviousPackageManifest>,
    pub include_debug_readable_json: bool,
}

impl RuntimePackageBuildRequest {
    pub fn dev_desktop(output_dir: impl Into<PathBuf>, active_scene_id: impl Into<String>) -> Self {
        Self {
            schema_version: RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root: PathBuf::from("."),
            active_scene_id: active_scene_id.into(),
            target: "dev-desktop".to_string(),
            mode: "dev-run".to_string(),
            output_dir: output_dir.into(),
            previous_package_manifest: None,
            include_debug_readable_json: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousPackageManifest {
    pub package_id: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageBuildInput {
    pub project: RuntimeProjectInfo,
    pub observation_contract: Option<ProjectObservationContract>,
    pub scenes: Vec<RuntimeScene>,
    pub prefabs: Vec<RuntimePackageSourcePrefab>,
    pub assets: Vec<RuntimePackageSourceAsset>,
    pub input_mappings: Vec<RuntimePackageSourceJson>,
    pub aui_documents: Vec<RuntimePackageSourceJson>,
    pub font_atlases: Vec<RuntimePackageSourceFontAtlas>,
    pub font_bundles: Vec<RuntimePackageSourceFontBundle>,
    pub animator2d_registry: CookedAnimator2DRegistry,
    pub texture_payloads: Vec<RuntimePackageSourceTexture>,
    pub component_schema: Option<serde_json::Value>,
    pub rule_manifest: Option<RuntimeRuleManifest>,
    pub aui_manifest: Option<RuntimeAuiManifest>,
}

impl RuntimePackageBuildInput {
    pub fn new(project: RuntimeProjectInfo) -> Self {
        Self {
            project,
            observation_contract: None,
            scenes: Vec::new(),
            prefabs: Vec::new(),
            assets: Vec::new(),
            input_mappings: Vec::new(),
            aui_documents: Vec::new(),
            font_atlases: Vec::new(),
            font_bundles: Vec::new(),
            animator2d_registry: CookedAnimator2DRegistry::empty(),
            texture_payloads: Vec::new(),
            component_schema: None,
            rule_manifest: None,
            aui_manifest: None,
        }
    }

    pub fn assembly_input_digest(&self) -> Result<AssemblyInputDigest, CanonicalDigestError> {
        let normalized = self.normalized_top_level_collections();
        let value = serde_json::json!({
            "project": normalized.project,
            "observationContract": normalized.observation_contract,
            "scenes": normalized.scenes,
            "prefabs": normalized.prefabs,
            "assets": normalized.assets.iter().map(asset_assembly_digest_value).collect::<Vec<_>>(),
            "inputMappings": normalized.input_mappings,
            "auiDocuments": normalized.aui_documents,
            "fontAtlases": normalized.font_atlases.iter().map(|source| serde_json::json!({
                "metadata": source.metadata,
                "atlasAlpha": byte_payload_digest_value(&source.atlas_alpha),
            })).collect::<Vec<_>>(),
            "fontBundles": normalized.font_bundles.iter().map(|source| serde_json::json!({
                "metadata": source.metadata,
                "pagePayloads": source.page_payloads.iter().map(|payload| byte_payload_digest_value(payload)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "animator2dRegistry": normalized.animator2d_registry,
            "texturePayloads": normalized.texture_payloads.iter().map(|source| serde_json::json!({
                "metadata": source.metadata,
                "rgba8": byte_payload_digest_value(&source.rgba8),
            })).collect::<Vec<_>>(),
            "componentSchema": normalized.component_schema,
            "ruleManifest": normalized.rule_manifest,
            "auiManifest": normalized.aui_manifest,
        });
        Ok(AssemblyInputDigest(ConsistencyDigest::sha256_value(
            "runtime-package-assembly-input",
            "runtime-package-assembly-input.v1",
            &value,
        )?))
    }

    fn normalized_top_level_collections(&self) -> Self {
        let mut normalized = self.clone();
        normalized
            .scenes
            .sort_by(|left, right| left.id.cmp(&right.id));
        normalized
            .prefabs
            .sort_by(|left, right| left.prefab_id.cmp(&right.prefab_id));
        normalized
            .assets
            .sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        normalized
            .input_mappings
            .sort_by(|left, right| left.id.cmp(&right.id));
        normalized
            .aui_documents
            .sort_by(|left, right| left.id.cmp(&right.id));
        normalized.font_atlases.sort_by(|left, right| {
            left.metadata
                .font_atlas_id
                .cmp(&right.metadata.font_atlas_id)
        });
        normalized.font_bundles.sort_by(|left, right| {
            left.metadata
                .font_bundle_id
                .cmp(&right.metadata.font_bundle_id)
        });
        normalized
            .texture_payloads
            .sort_by(|left, right| left.metadata.asset_id.cmp(&right.metadata.asset_id));
        normalized
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageSourceAsset {
    pub asset_id: String,
    pub asset_guid: Option<String>,
    pub name: String,
    pub asset_type: String,
    pub source: String,
    pub runtime_uri: String,
    pub hash: Option<String>,
    pub dependencies: Vec<String>,
    #[serde(skip)]
    pub runtime_payload: Option<Vec<u8>>,
}

impl RuntimePackageSourceAsset {
    pub fn new(
        asset_id: impl Into<String>,
        name: impl Into<String>,
        asset_type: impl Into<String>,
        source: impl Into<String>,
        runtime_uri: impl Into<String>,
    ) -> Self {
        Self {
            asset_id: asset_id.into(),
            asset_guid: None,
            name: name.into(),
            asset_type: asset_type.into(),
            source: source.into(),
            runtime_uri: runtime_uri.into(),
            hash: None,
            dependencies: Vec::new(),
            runtime_payload: None,
        }
    }

    pub fn with_runtime_payload(mut self, payload: Vec<u8>) -> Self {
        self.runtime_payload = Some(payload);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageSourcePrefab {
    pub prefab_id: String,
    pub document: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageSourceJson {
    pub id: String,
    pub document: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageSourceFontAtlas {
    pub metadata: CookedFontAtlasAsset,
    pub atlas_alpha: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageSourceTexture {
    pub metadata: CookedTextureAsset,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyInputDigest(pub ConsistencyDigest);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeContentHash(pub ConsistencyDigest);

impl RuntimeContentHash {
    pub fn manifest_value(&self) -> String {
        self.0.prefixed_value()
    }
}

pub fn is_canonical_runtime_content_hash(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn validate_runtime_content_hash_evidence(value: &str) -> Result<(), RuntimePackageDiagnostic> {
    if is_canonical_runtime_content_hash(value) {
        return Ok(());
    }
    Err(RuntimePackageDiagnostic::error(
        UNSUPPORTED_CONTENT_HASH_ALGORITHM_DIAGNOSTIC,
        format!("Gate 236 requires contentHash=sha256:<64 lowercase hex>, got {value:?}"),
        None,
        Some("manifest.contentHash".to_string()),
        Some("Rebuild the RuntimePackage with the Gate 236 canonical digest builder.".to_string()),
    ))
}

pub fn verify_runtime_content_hash_mutation(
    before: &RuntimeContentHash,
    after: &RuntimeContentHash,
    mutation: impl AsRef<str>,
) -> Result<(), RuntimePackageDiagnostic> {
    if before != after {
        return Ok(());
    }
    Err(RuntimePackageDiagnostic::error(
        DIGEST_INSENSITIVE_DIAGNOSTIC,
        format!(
            "runtime content digest did not change after effective mutation: {}",
            mutation.as_ref()
        ),
        None,
        None,
        Some("Include the mutated runtime payload in RuntimePackageWritePlan.".to_string()),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadTreeDigest(pub ConsistencyDigest);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimePackageBuildStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimePackageDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageDiagnostic {
    pub severity: RuntimePackageDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub object_id: Option<String>,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl RuntimePackageDiagnostic {
    fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        object_id: Option<String>,
        path: Option<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self {
            severity: RuntimePackageDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            object_id,
            path,
            suggestion,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageCheckedCounts {
    pub scenes: usize,
    pub entities: usize,
    pub components: usize,
    pub assets: usize,
    pub prefabs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageValidationReport {
    pub schema_version: String,
    pub status: RuntimePackageBuildStatus,
    pub checked_counts: RuntimePackageCheckedCounts,
    pub diagnostics: Vec<RuntimePackageDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageDiffReport {
    pub schema_version: String,
    pub previous_package_id: Option<String>,
    pub current_package_id: String,
    pub changes: Vec<RuntimePackageDiffChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageDiffChange {
    pub kind: String,
    pub id: String,
    pub change: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageBuildOutputs {
    pub package_dir: String,
    pub manifest: String,
    #[serde(default)]
    pub runtime_content_hash: Option<String>,
    #[serde(default)]
    pub payload_tree_digest: Option<String>,
    #[serde(default)]
    pub payload_inventory: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageBuildStageReport {
    pub stage_id: String,
    pub status: RuntimePackageBuildStatus,
    pub duration_ms: u128,
    pub diagnostics: Vec<RuntimePackageDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageBuildReport {
    pub schema_version: String,
    pub request_id: String,
    pub status: RuntimePackageBuildStatus,
    pub outputs: RuntimePackageBuildOutputs,
    pub stages: Vec<RuntimePackageBuildStageReport>,
    pub source_reports: Vec<String>,
    pub diagnostics: Vec<RuntimePackageDiagnostic>,
}

pub struct RuntimePackageBuilder;

impl RuntimePackageBuilder {
    pub fn build(
        request: &RuntimePackageBuildRequest,
        input: &RuntimePackageBuildInput,
    ) -> RuntimePackageBuildReport {
        let started = Instant::now();
        let mut diagnostics = Vec::new();
        let mut stages = Vec::new();

        validate_request(request, &mut diagnostics);
        validate_input(request, input, &mut diagnostics);

        let package_dir = request.output_dir.clone();
        let reports_dir = package_dir.join("reports");
        let source_reports = vec![
            "reports/runtime-package-validation-report.json".to_string(),
            "reports/runtime-package-diff-report.json".to_string(),
        ];

        let write_plan = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == RuntimePackageDiagnosticSeverity::Error)
        {
            None
        } else {
            match RuntimePackageWritePlan::build(request, input) {
                Ok(plan) => Some(plan),
                Err(error) => {
                    let code = if matches!(&error, CanonicalDigestError::ScopeViolation(_)) {
                        DIGEST_SCOPE_VIOLATION_DIAGNOSTIC
                    } else {
                        CANONICAL_ENCODE_FAILED_DIAGNOSTIC
                    };
                    diagnostics.push(RuntimePackageDiagnostic::error(
                        code,
                        format!("failed to create canonical runtime package write plan: {error}"),
                        None,
                        None,
                        Some(
                            "Fix the runtime package payload so it can be canonically encoded."
                                .to_string(),
                        ),
                    ));
                    None
                }
            }
        };

        let publish_result = write_plan
            .as_ref()
            .map(|plan| publish_runtime_package(&request.output_dir, plan, PublishFaultPoint::None))
            .unwrap_or(Ok(()));

        if let Err(error) = publish_result {
            diagnostics.push(RuntimePackageDiagnostic::error(
                error.code,
                error.message,
                None,
                Some(error.path.display().to_string()),
                Some(error.next_action.to_string()),
            ));
        }

        let mut runtime_content_hash = None;
        let mut payload_tree_digest = None;
        if let Some(plan) = &write_plan {
            runtime_content_hash = Some(plan.runtime_content_hash.manifest_value());
            match plan.payload_tree_digest() {
                Ok(digest) => payload_tree_digest = Some(digest.0.prefixed_value()),
                Err(error) => diagnostics.push(RuntimePackageDiagnostic::error(
                    CANONICAL_ENCODE_FAILED_DIAGNOSTIC,
                    format!("failed to calculate payload tree digest: {error}"),
                    None,
                    None,
                    Some(
                        "Fix the runtime package payload so its evidence can be calculated."
                            .to_string(),
                    ),
                )),
            }
        }
        let checked_counts = checked_counts(input);
        let status = status_from_diagnostics(&diagnostics);
        let validation_report = RuntimePackageValidationReport {
            schema_version: RUNTIME_PACKAGE_VALIDATION_REPORT_SCHEMA_VERSION.to_string(),
            status: status.clone(),
            checked_counts,
            diagnostics: diagnostics.clone(),
        };
        let package_hash = runtime_content_hash
            .clone()
            .unwrap_or_else(|| "unavailable".to_string());
        let diff_report = build_diff_report(request, &package_hash);

        stages.push(RuntimePackageBuildStageReport {
            stage_id: "build-runtime-package".to_string(),
            status: status.clone(),
            duration_ms: started.elapsed().as_millis(),
            diagnostics: diagnostics.clone(),
        });

        let mut report = RuntimePackageBuildReport {
            schema_version: BUILD_RUNTIME_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
            request_id: format!("runtime-package-build-{}", package_hash),
            status,
            outputs: RuntimePackageBuildOutputs {
                package_dir: package_dir.display().to_string(),
                manifest: package_dir.join("manifest.json").display().to_string(),
                runtime_content_hash,
                payload_tree_digest,
                payload_inventory: write_plan
                    .as_ref()
                    .map(RuntimePackageWritePlan::inventory_paths)
                    .unwrap_or_default(),
            },
            stages,
            source_reports,
            diagnostics,
        };

        if matches!(report.status, RuntimePackageBuildStatus::Success) {
            let _ = fs::create_dir_all(&reports_dir);
            let _ = write_json(
                &reports_dir.join("runtime-package-validation-report.json"),
                &validation_report,
            );
            let _ = write_json(
                &reports_dir.join("runtime-package-diff-report.json"),
                &diff_report,
            );
            let _ = write_json(
                &reports_dir.join("build-runtime-package-report.json"),
                &report,
            );
            let load_result = load_runtime_package(&package_dir);
            if load_result.diagnostics.has_errors() {
                report.status = RuntimePackageBuildStatus::Failed;
                for issue in load_result.diagnostics.issues {
                    report.diagnostics.push(RuntimePackageDiagnostic::error(
                        "RuntimePackageLoaderRejectedOutput",
                        issue.message,
                        None,
                        Some(issue.path),
                        Some(
                            "Fix generated package so RuntimePackageLoader can read it."
                                .to_string(),
                        ),
                    ));
                }
                let _ = write_json(
                    &reports_dir.join("build-runtime-package-report.json"),
                    &report,
                );
            }
        }
        report
    }
}

fn validate_request(
    request: &RuntimePackageBuildRequest,
    diagnostics: &mut Vec<RuntimePackageDiagnostic>,
) {
    if request.schema_version != RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION {
        diagnostics.push(RuntimePackageDiagnostic::error(
            "InvalidBuildRequestSchema",
            format!(
                "schemaVersion must be {}",
                RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION
            ),
            None,
            Some("schemaVersion".to_string()),
            None,
        ));
    }
    if request.active_scene_id.is_empty() {
        diagnostics.push(RuntimePackageDiagnostic::error(
            "MissingActiveSceneId",
            "activeSceneId is required",
            None,
            Some("activeSceneId".to_string()),
            Some("Set activeSceneId to a saved scene id.".to_string()),
        ));
    }
    if request.target != "dev-desktop" {
        diagnostics.push(RuntimePackageDiagnostic::error(
            "UnsupportedTarget",
            "RuntimePackageBuilder C-min only supports dev-desktop",
            None,
            Some("target".to_string()),
            Some("Use target=dev-desktop for the first runtime package gate.".to_string()),
        ));
    }
}

fn validate_input(
    request: &RuntimePackageBuildRequest,
    input: &RuntimePackageBuildInput,
    diagnostics: &mut Vec<RuntimePackageDiagnostic>,
) {
    if let Some(contract) = &input.observation_contract {
        if let Err(contract_diagnostics) = contract.validate() {
            diagnostics.extend(contract_diagnostics.into_iter().map(|diagnostic| {
                RuntimePackageDiagnostic::error(
                    diagnostic.code,
                    diagnostic.message,
                    diagnostic.contract_id,
                    diagnostic.path,
                    Some(diagnostic.next_action.to_string()),
                )
            }));
        }
    }
    let mut asset_ids = HashSet::new();
    for asset in &input.assets {
        if asset.asset_id.is_empty() {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "MissingAssetId",
                "asset id is required",
                None,
                Some("assets[].assetId".to_string()),
                Some("Assign a stable asset id before building Runtime Package.".to_string()),
            ));
            continue;
        }
        if !asset_ids.insert(asset.asset_id.as_str()) {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "DuplicateAssetId",
                format!("duplicate asset id: {}", asset.asset_id),
                Some(asset.asset_id.clone()),
                Some("assets[].assetId".to_string()),
                Some("Ensure every asset id is stable and unique.".to_string()),
            ));
        }
    }
    for prefab in &input.prefabs {
        if prefab.prefab_id.is_empty() {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "MissingPrefabAssetId",
                "prefab id is required",
                None,
                Some("prefabs[].prefabId".to_string()),
                Some("Assign a stable prefab id before building Runtime Package.".to_string()),
            ));
            continue;
        }
        if !asset_ids.insert(prefab.prefab_id.as_str()) {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "DuplicateAssetId",
                format!("duplicate runtime asset id: {}", prefab.prefab_id),
                Some(prefab.prefab_id.clone()),
                Some("prefabs[].prefabId".to_string()),
                Some(
                    "Prefab ids share the RuntimeAsset namespace with imported assets.".to_string(),
                ),
            ));
        }
    }

    if !input
        .scenes
        .iter()
        .any(|scene| scene.id == request.active_scene_id)
    {
        diagnostics.push(RuntimePackageDiagnostic::error(
            "MissingActiveScene",
            format!("active scene does not exist: {}", request.active_scene_id),
            Some(request.active_scene_id.clone()),
            Some("activeSceneId".to_string()),
            Some("Save the active scene or choose an existing scene id.".to_string()),
        ));
    }

    for scene in &input.scenes {
        validate_scene_source(scene, &asset_ids, diagnostics);
    }
    for prefab in &input.prefabs {
        if let Err(diagnostic) = runtime_prefab_document_from_source(prefab) {
            diagnostics.push(diagnostic);
        }
    }
}

fn validate_scene_source(
    scene: &RuntimeScene,
    asset_ids: &HashSet<&str>,
    diagnostics: &mut Vec<RuntimePackageDiagnostic>,
) {
    if scene.id.is_empty() {
        diagnostics.push(RuntimePackageDiagnostic::error(
            "MissingSceneId",
            "scene id is required",
            None,
            Some("scenes[].id".to_string()),
            Some("Assign a stable scene id before package build.".to_string()),
        ));
    }
    let mut entity_ids = HashSet::new();
    for (entity_index, entity) in scene.entities.iter().enumerate() {
        let entity_path = format!("scenes[{}].entities[{}]", scene.id, entity_index);
        if entity.id.is_empty() {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "MissingEntityId",
                "entity id is required",
                None,
                Some(format!("{}.id", entity_path)),
                Some("Assign a stable source entity id.".to_string()),
            ));
            continue;
        }
        if !entity_ids.insert(entity.id.as_str()) {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "DuplicateEntityId",
                format!("duplicate entity id: {}", entity.id),
                Some(entity.id.clone()),
                Some(format!("{}.id", entity_path)),
                Some("Ensure entity ids are unique inside a scene.".to_string()),
            ));
        }
        if entity.transform.is_none() {
            diagnostics.push(RuntimePackageDiagnostic::error(
                "MissingTransform",
                "entity transform is required",
                Some(entity.id.clone()),
                Some(format!("{}.transform", entity_path)),
                Some("Every runtime entity needs a Transform component.".to_string()),
            ));
        }
        if let Some(mesh) = &entity.mesh {
            validate_asset_ref(
                mesh.asset_ref.as_ref(),
                "mesh.assetRef",
                &entity.id,
                &entity_path,
                asset_ids,
                diagnostics,
            );
            validate_asset_ref(
                mesh.material_ref.as_ref(),
                "mesh.materialRef",
                &entity.id,
                &entity_path,
                asset_ids,
                diagnostics,
            );
            validate_asset_ref(
                mesh.texture_ref.as_ref(),
                "mesh.textureRef",
                &entity.id,
                &entity_path,
                asset_ids,
                diagnostics,
            );
        }
    }
}

fn validate_asset_ref(
    asset_ref: Option<&RuntimeAssetRef>,
    field: &str,
    entity_id: &str,
    entity_path: &str,
    asset_ids: &HashSet<&str>,
    diagnostics: &mut Vec<RuntimePackageDiagnostic>,
) {
    let Some(asset_ref) = asset_ref else {
        return;
    };
    if asset_ref.id.is_empty() || !asset_ids.contains(asset_ref.id.as_str()) {
        diagnostics.push(RuntimePackageDiagnostic::error(
            "MissingAssetRef",
            format!("AssetRef cannot be resolved: {}", asset_ref.id),
            Some(entity_id.to_string()),
            Some(format!("{}.{}", entity_path, field)),
            Some("Import the missing asset or replace the AssetRef.".to_string()),
        ));
    }
}

fn runtime_prefab_document_from_source(
    prefab: &RuntimePackageSourcePrefab,
) -> Result<serde_json::Value, RuntimePackageDiagnostic> {
    let schema_version = prefab
        .document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match schema_version {
        "runtime-prefab.v1" => Ok(prefab.document.clone()),
        "authoring-prefab-asset.v1" => {
            let runtime_prefab = authoring_prefab_to_runtime_prefab(prefab)?;
            serde_json::to_value(runtime_prefab).map_err(|error| {
                RuntimePackageDiagnostic::error(
                    "PrefabRuntimeSerializationFailed",
                    format!(
                        "failed to serialize runtime prefab {}: {}",
                        prefab.prefab_id, error
                    ),
                    Some(prefab.prefab_id.clone()),
                    Some("prefabs[]".to_string()),
                    Some("Check authoring PrefabAsset fields.".to_string()),
                )
            })
        }
        _ => Err(RuntimePackageDiagnostic::error(
            "UnsupportedPrefabSchema",
            format!(
                "unsupported prefab schemaVersion for {}: {}",
                prefab.prefab_id, schema_version
            ),
            Some(prefab.prefab_id.clone()),
            Some("prefabs[].document.schemaVersion".to_string()),
            Some("Use runtime-prefab.v1 or authoring-prefab-asset.v1.".to_string()),
        )),
    }
}

fn authoring_prefab_to_runtime_prefab(
    prefab: &RuntimePackageSourcePrefab,
) -> Result<RuntimePrefabData, RuntimePackageDiagnostic> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AuthoringPrefabAsset {
        prefab_id: String,
        name: String,
        root_entity_id: String,
        #[serde(default)]
        entities: Vec<AuthoringPrefabEntity>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AuthoringPrefabEntity {
        source_entity_id: String,
        name: String,
        parent_source_entity_id: Option<String>,
        #[serde(default)]
        sibling_order: i32,
        #[serde(default = "default_authoring_prefab_entity_enabled")]
        enabled: bool,
        transform: Option<crate::runtime_package::RuntimeTransform>,
        #[serde(default)]
        components: Vec<AuthoringPrefabComponent>,
        #[serde(default)]
        mesh: Option<crate::runtime_package::RuntimeMesh>,
        #[serde(default, rename = "spriteRenderer2D", alias = "spriteRenderer2d")]
        sprite_renderer2d: Option<crate::runtime_package::RuntimeSpriteRenderer2D>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AuthoringPrefabComponent {
        #[serde(alias = "componentType")]
        component_type: String,
        #[serde(default, alias = "fields")]
        data: serde_json::Value,
    }

    fn default_authoring_prefab_entity_enabled() -> bool {
        true
    }

    let authoring = serde_json::from_value::<AuthoringPrefabAsset>(prefab.document.clone())
        .map_err(|error| {
            RuntimePackageDiagnostic::error(
                "AuthoringPrefabParseFailed",
                format!(
                    "failed to parse authoring prefab {}: {}",
                    prefab.prefab_id, error
                ),
                Some(prefab.prefab_id.clone()),
                Some("prefabs[].document".to_string()),
                Some("Fix PrefabAsset document shape.".to_string()),
            )
        })?;

    if authoring.prefab_id != prefab.prefab_id {
        return Err(RuntimePackageDiagnostic::error(
            "PrefabIdMismatch",
            format!(
                "source prefab id {} does not match document prefab id {}",
                prefab.prefab_id, authoring.prefab_id
            ),
            Some(prefab.prefab_id.clone()),
            Some("prefabs[].prefabId".to_string()),
            Some(
                "Keep RuntimePackageSourcePrefab.prefab_id equal to PrefabAsset.prefab_id."
                    .to_string(),
            ),
        ));
    }
    let entity_ids = authoring
        .entities
        .iter()
        .map(|entity| entity.source_entity_id.as_str())
        .collect::<HashSet<_>>();
    if !entity_ids.contains(authoring.root_entity_id.as_str()) {
        return Err(RuntimePackageDiagnostic::error(
            "PrefabRootMissing",
            format!(
                "prefab root entity is missing: {}",
                authoring.root_entity_id
            ),
            Some(prefab.prefab_id.clone()),
            Some("prefabs[].document.rootEntityId".to_string()),
            Some("Ensure PrefabAsset rootEntityId points to an entity in entities[].".to_string()),
        ));
    }

    let mut entities = Vec::with_capacity(authoring.entities.len());
    for entity in authoring.entities {
        let mut sprite_renderer2d = entity.sprite_renderer2d;
        let mut components = Vec::new();
        for component in entity.components {
            if component.component_type == "SpriteRenderer2D" {
                if sprite_renderer2d.is_some() {
                    return Err(RuntimePackageDiagnostic::error(
                        "DuplicateSpriteRenderer2D",
                        format!(
                            "authoring prefab {} entity {} declares SpriteRenderer2D twice",
                            prefab.prefab_id, entity.source_entity_id
                        ),
                        Some(prefab.prefab_id.clone()),
                        Some("prefabs[].document.entities[].components".to_string()),
                        Some("Keep one SpriteRenderer2D component per prefab entity.".to_string()),
                    ));
                }
                sprite_renderer2d =
                    Some(serde_json::from_value(component.data).map_err(|error| {
                        RuntimePackageDiagnostic::error(
                            "AuthoringPrefabSpriteRenderer2DParseFailed",
                            format!(
                                "failed to parse SpriteRenderer2D for prefab {} entity {}: {error}",
                                prefab.prefab_id, entity.source_entity_id
                            ),
                            Some(prefab.prefab_id.clone()),
                            Some("prefabs[].document.entities[].components".to_string()),
                            Some("Fix SpriteRenderer2D authoring fields.".to_string()),
                        )
                    })?);
            } else {
                components.push(crate::runtime_package::RuntimeProjectComponent {
                    component_type: component.component_type,
                    data: component.data,
                });
            }
        }
        entities.push(RuntimeEntity {
            schema_version: crate::runtime_package::RUNTIME_ENTITY_SCHEMA_VERSION.to_string(),
            id: entity.source_entity_id,
            name: entity.name,
            kind: "prefab_entity".to_string(),
            enabled: entity.enabled,
            parent_id: entity.parent_source_entity_id,
            sibling_order: entity.sibling_order,
            transform: entity.transform,
            mesh: entity.mesh,
            sprite_renderer2d,
            animator2d: None,
            components,
        });
    }

    Ok(RuntimePrefabData {
        schema_version: "runtime-prefab.v1".to_string(),
        id: authoring.prefab_id,
        name: authoring.name,
        root_entity_id: Some(authoring.root_entity_id),
        entities,
    })
}

#[derive(Debug, Clone)]
enum RuntimePackagePayloadContent {
    Json(serde_json::Value),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
struct RuntimePackagePayload {
    relative_path: String,
    content: RuntimePackagePayloadContent,
}

#[derive(Debug, Clone)]
struct RuntimePackageWritePlan {
    payloads: Vec<RuntimePackagePayload>,
    runtime_content_hash: RuntimeContentHash,
}

impl RuntimePackageWritePlan {
    fn build(
        request: &RuntimePackageBuildRequest,
        source_input: &RuntimePackageBuildInput,
    ) -> Result<Self, CanonicalDigestError> {
        let input = source_input.normalized_top_level_collections();
        validate_runtime_package_source_ids(request, &input)?;
        if input.input_mappings.is_empty() {
            return Err(CanonicalDigestError::ScopeViolation(
                "RuntimePackage v2 requires an explicit project InputMappingAsset; use input.none for projects without input."
                    .to_string(),
            ));
        }
        let mut payloads = Vec::new();

        for scene in &input.scenes {
            push_json_payload(&mut payloads, format!("scenes/{}.json", scene.id), scene)?;
        }
        for prefab in &input.prefabs {
            let document = runtime_prefab_document_from_source(prefab)
                .map_err(|diagnostic| CanonicalDigestError::Serialize(diagnostic.message))?;
            push_json_value_payload(
                &mut payloads,
                prefab_package_path(&prefab.prefab_id),
                document,
            );
        }
        for mapping in &input.input_mappings {
            push_json_value_payload(
                &mut payloads,
                format!("input/{}.json", mapping.id),
                mapping.document.clone(),
            );
        }

        let input_manifest = build_input_manifest(&input);
        push_json_payload(&mut payloads, "input/input-manifest.json", &input_manifest)?;

        let component_schema = input.component_schema.clone().unwrap_or_else(
            || serde_json::json!({ "schemaVersion": "component-schema.v1", "components": [] }),
        );
        push_json_value_payload(
            &mut payloads,
            "schema/component-schema.json",
            component_schema,
        );
        let rules = input
            .rule_manifest
            .clone()
            .unwrap_or_else(default_rule_manifest);
        push_json_payload(&mut payloads, "rules/rule-manifest.json", &rules)?;

        let aui_manifest = input
            .aui_manifest
            .clone()
            .unwrap_or_else(RuntimeAuiManifest::empty);
        for document in &input.aui_documents {
            let package_path = aui_manifest
                .documents
                .iter()
                .find(|entry| entry.document_id == document.id)
                .map(|entry| entry.path.clone())
                .unwrap_or_else(|| format!("aui/documents/{}.aui.json", document.id));
            push_json_value_payload(&mut payloads, package_path, document.document.clone());
        }
        push_json_payload(&mut payloads, "aui/aui-manifest.json", &aui_manifest)?;

        let font_atlas_manifest = build_font_atlas_manifest(&input);
        for source in &input.font_atlases {
            let metadata = normalized_font_atlas_metadata(source);
            push_json_payload(
                &mut payloads,
                format!("fonts/{}.fontatlas.json", metadata.font_atlas_id),
                &metadata,
            )?;
            push_bytes_payload(
                &mut payloads,
                metadata.atlas_image_path.clone(),
                source.atlas_alpha.clone(),
            );
        }
        push_json_payload(
            &mut payloads,
            "fonts/font-atlas-manifest.json",
            &font_atlas_manifest,
        )?;
        let font_bundle_manifest = build_font_bundle_manifest(&input);
        for source in &input.font_bundles {
            push_json_payload(
                &mut payloads,
                format!("fonts/{}/font-bundle.json", source.metadata.font_bundle_id),
                &source.metadata,
            )?;
            for (page, bytes) in source
                .metadata
                .pages
                .iter()
                .zip(source.page_payloads.iter())
            {
                push_bytes_payload(&mut payloads, page.payload_path.clone(), bytes.clone());
            }
        }
        push_json_payload(
            &mut payloads,
            "fonts/font-bundle-manifest.json",
            &font_bundle_manifest,
        )?;
        input
            .animator2d_registry
            .validate()
            .map_err(|diagnostics| {
                CanonicalDigestError::ScopeViolation(
                    diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.message)
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            })?;
        push_json_payload(
            &mut payloads,
            "animator2d/registry.json",
            &input.animator2d_registry,
        )?;
        for source in &input.texture_payloads {
            push_json_payload(
                &mut payloads,
                texture_metadata_package_path(&source.metadata.asset_id),
                &source.metadata,
            )?;
            push_bytes_payload(
                &mut payloads,
                source.metadata.pixel_data_path.clone(),
                source.rgba8.clone(),
            );
        }
        for asset in &input.assets {
            if let Some(payload) = &asset.runtime_payload {
                push_bytes_payload(&mut payloads, asset.runtime_uri.clone(), payload.clone());
            }
        }

        let asset_manifest = build_asset_manifest(&input);
        push_json_payload(&mut payloads, "assets/asset-manifest.json", &asset_manifest)?;
        push_json_payload(
            &mut payloads,
            "assets/runtime-asset-index.json",
            &asset_manifest.runtime_asset_index,
        )?;

        let manifest_without_content_hash = RuntimePackageManifest {
            schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION.to_string(),
            package_mode: RUNTIME_PACKAGE_MODE.to_string(),
            project: input.project.clone(),
            observation_contract: input
                .observation_contract
                .as_ref()
                .map(ProjectObservationContract::cook)
                .transpose()
                .map_err(|diagnostics| {
                    CanonicalDigestError::ScopeViolation(
                        diagnostics
                            .into_iter()
                            .map(|diagnostic| diagnostic.message)
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?,
            active_scene_id: request.active_scene_id.clone(),
            scenes: input
                .scenes
                .iter()
                .map(|scene| RuntimeSceneManifestEntry {
                    id: scene.id.clone(),
                    name: scene.name.clone(),
                    path: format!("scenes/{}.json", scene.id),
                    entity_count: scene.entities.len(),
                })
                .collect(),
            assets: RuntimeManifestAssetIndex {
                path: "assets/asset-manifest.json".to_string(),
                asset_count: asset_manifest.assets.len(),
            },
            rules: RuntimeManifestRuleIndex {
                path: "rules/rule-manifest.json".to_string(),
                mode: rules.mode.clone(),
            },
            input: RuntimeManifestInputIndex {
                path: "input/input-manifest.json".to_string(),
                default_mapping_id: input_manifest.default_mapping_id.clone(),
                mapping_count: input_manifest
                    .mappings
                    .iter()
                    .filter(|entry| entry.enabled)
                    .count(),
            },
            aui: Some(RuntimeManifestAuiIndex {
                path: "aui/aui-manifest.json".to_string(),
                document_count: aui_manifest.documents.len(),
            }),
            font_atlases: Some(RuntimeManifestFontAtlasIndex {
                path: "fonts/font-atlas-manifest.json".to_string(),
                atlas_count: font_atlas_manifest.atlases.len(),
                default_ui_font_atlas_id: font_atlas_manifest.default_ui_font_atlas_id.clone(),
            }),
            font_bundles: Some(crate::runtime_package::RuntimeManifestFontBundleIndex {
                path: "fonts/font-bundle-manifest.json".to_string(),
                bundle_count: font_bundle_manifest.bundles.len(),
                default_ui_font_bundle_id: font_bundle_manifest.default_ui_font_bundle_id.clone(),
            }),
            animator2d: Some(crate::runtime_package::RuntimeManifestAnimator2DIndex {
                path: "animator2d/registry.json".to_string(),
                registry_digest: input.animator2d_registry.registry_digest.clone(),
                clip_count: input.animator2d_registry.clips.len(),
                controller_count: input.animator2d_registry.controllers.len(),
            }),
            content_hash: None,
        };
        let runtime_content_hash = RuntimeContentHash(ConsistencyDigest::sha256_value(
            "runtime-package-content",
            RUNTIME_PACKAGE_SCHEMA_VERSION,
            &runtime_content_digest_value(&manifest_without_content_hash, &payloads)?,
        )?);
        let mut manifest = manifest_without_content_hash;
        manifest.content_hash = Some(runtime_content_hash.manifest_value());
        push_json_payload(&mut payloads, "manifest.json", &manifest)?;
        payloads.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        validate_digest_payload_scope(&payloads)?;

        Ok(Self {
            payloads,
            runtime_content_hash,
        })
    }

    fn payload_tree_digest(&self) -> Result<PayloadTreeDigest, CanonicalDigestError> {
        let entries = self
            .payloads
            .iter()
            .map(|payload| {
                let bytes = match &payload.content {
                    RuntimePackagePayloadContent::Json(value) => {
                        serde_json::to_vec_pretty(value)
                            .map_err(|error| CanonicalDigestError::Serialize(error.to_string()))?
                    }
                    RuntimePackagePayloadContent::Bytes(bytes) => bytes.clone(),
                };
                Ok((payload.relative_path.clone(), bytes))
            })
            .collect::<Result<Vec<_>, CanonicalDigestError>>()?;
        Ok(PayloadTreeDigest(payload_tree_digest(
            entries
                .iter()
                .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
        )?))
    }

    fn inventory_paths(&self) -> Vec<String> {
        self.payloads
            .iter()
            .map(|payload| payload.relative_path.clone())
            .collect()
    }
}

type RuntimePackagePublishError = AtomicDirectoryPublishError;
type PublishFaultPoint = AtomicDirectoryPublishFault;

#[cfg(test)]
type RuntimePackagePublishGuard = crate::atomic_directory_publish::AtomicDirectoryPublishGuard;

fn publish_runtime_package(
    final_dir: &Path,
    plan: &RuntimePackageWritePlan,
    fault: PublishFaultPoint,
) -> Result<(), RuntimePackagePublishError> {
    atomic_directory_publish_with_fault(
        final_dir,
        fault,
        |staging_dir| {
            write_runtime_package(staging_dir, plan).map_err(|error| {
                format!("failed to write RuntimePackage staging payloads: {error}")
            })
        },
        |package_dir| validate_published_package(package_dir, plan),
    )
}

fn validate_published_package(
    package_dir: &Path,
    plan: &RuntimePackageWritePlan,
) -> Result<(), String> {
    let load = load_runtime_package(package_dir);
    if load.diagnostics.has_errors() {
        return Err(format!(
            "formal RuntimePackage loader rejected output: {}",
            load.diagnostics
                .issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    for payload in &plan.payloads {
        let path = safe_join_runtime_package(package_dir, &payload.relative_path)
            .map_err(|error| error.to_string())?;
        if !path.is_file() {
            return Err(format!(
                "payload inventory entry is missing after write: {}",
                payload.relative_path
            ));
        }
    }
    Ok(())
}

fn write_runtime_package(
    package_dir: &Path,
    plan: &RuntimePackageWritePlan,
) -> std::io::Result<()> {
    for payload in &plan.payloads {
        let path = safe_join_runtime_package(package_dir, &payload.relative_path)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        match &payload.content {
            RuntimePackagePayloadContent::Json(value) => {
                let bytes = serde_json::to_vec_pretty(value)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
                fs::write(path, bytes)?;
            }
            RuntimePackagePayloadContent::Bytes(bytes) => fs::write(path, bytes)?,
        }
    }
    Ok(())
}

fn push_json_payload<T: Serialize>(
    payloads: &mut Vec<RuntimePackagePayload>,
    relative_path: impl Into<String>,
    value: &T,
) -> Result<(), CanonicalDigestError> {
    let value = serde_json::to_value(value)
        .map_err(|error| CanonicalDigestError::Serialize(error.to_string()))?;
    push_json_value_payload(payloads, relative_path, value);
    Ok(())
}

fn push_json_value_payload(
    payloads: &mut Vec<RuntimePackagePayload>,
    relative_path: impl Into<String>,
    value: serde_json::Value,
) {
    payloads.push(RuntimePackagePayload {
        relative_path: relative_path.into(),
        content: RuntimePackagePayloadContent::Json(value),
    });
}

fn push_bytes_payload(
    payloads: &mut Vec<RuntimePackagePayload>,
    relative_path: impl Into<String>,
    bytes: Vec<u8>,
) {
    payloads.push(RuntimePackagePayload {
        relative_path: relative_path.into(),
        content: RuntimePackagePayloadContent::Bytes(bytes),
    });
}

fn runtime_content_digest_value(
    manifest_without_content_hash: &RuntimePackageManifest,
    payloads: &[RuntimePackagePayload],
) -> Result<serde_json::Value, CanonicalDigestError> {
    let mut payloads = payloads.iter().collect::<Vec<_>>();
    payloads.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let payloads = payloads
        .into_iter()
        .map(|payload| match &payload.content {
            RuntimePackagePayloadContent::Json(value) => Ok(serde_json::json!({
                "path": payload.relative_path,
                "value": value,
            })),
            RuntimePackagePayloadContent::Bytes(bytes) => Ok(serde_json::json!({
                "path": payload.relative_path,
                "rawBytes": byte_payload_digest_value(bytes),
            })),
        })
        .collect::<Result<Vec<_>, CanonicalDigestError>>()?;
    Ok(serde_json::json!({
        "manifest": manifest_without_content_hash,
        "payloads": payloads,
    }))
}

fn byte_payload_digest_value(bytes: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "byteLength": bytes.len(),
        "sha256": sha256_prefixed(bytes),
    })
}

fn asset_assembly_digest_value(asset: &RuntimePackageSourceAsset) -> serde_json::Value {
    let mut value =
        serde_json::to_value(asset).expect("RuntimePackageSourceAsset serialization is infallible");
    if let (Some(object), Some(payload)) = (value.as_object_mut(), asset.runtime_payload.as_ref()) {
        object.insert(
            "runtimePayload".to_string(),
            byte_payload_digest_value(payload),
        );
    }
    value
}

fn validate_digest_payload_scope(
    payloads: &[RuntimePackagePayload],
) -> Result<(), CanonicalDigestError> {
    let mut claims = RuntimePackagePathClaims::default();
    for payload in payloads {
        if payload.relative_path == "reports" || payload.relative_path.starts_with("reports/") {
            return Err(CanonicalDigestError::ScopeViolation(
                "reports/** must not contribute to RuntimeContentHash".to_string(),
            ));
        }
        let package_path = RuntimePackagePath::parse(payload.relative_path.clone())
            .map_err(|error| CanonicalDigestError::ScopeViolation(error.to_string()))?;
        claims
            .claim(&package_path)
            .map_err(|error| CanonicalDigestError::ScopeViolation(error.to_string()))?;
    }
    Ok(())
}

fn validate_runtime_package_source_ids(
    request: &RuntimePackageBuildRequest,
    input: &RuntimePackageBuildInput,
) -> Result<(), CanonicalDigestError> {
    let mut ids = Vec::new();
    ids.push(("activeSceneId", request.active_scene_id.as_str()));
    ids.extend(
        input
            .scenes
            .iter()
            .map(|source| ("scene.id", source.id.as_str())),
    );
    ids.extend(
        input
            .prefabs
            .iter()
            .map(|source| ("prefab.prefabId", source.prefab_id.as_str())),
    );
    ids.extend(
        input
            .input_mappings
            .iter()
            .map(|source| ("input.id", source.id.as_str())),
    );
    ids.extend(
        input
            .aui_documents
            .iter()
            .map(|source| ("aui.documentId", source.id.as_str())),
    );
    ids.extend(input.font_atlases.iter().map(|source| {
        (
            "fontAtlas.fontAtlasId",
            source.metadata.font_atlas_id.as_str(),
        )
    }));
    ids.extend(
        input
            .texture_payloads
            .iter()
            .map(|source| ("texture.assetId", source.metadata.asset_id.as_str())),
    );
    for (field, id) in ids {
        validate_package_path_segment(id).map_err(|error| {
            CanonicalDigestError::ScopeViolation(format!("unsafe {field}: {error}"))
        })?;
    }
    Ok(())
}

fn normalized_font_atlas_metadata(source: &RuntimePackageSourceFontAtlas) -> CookedFontAtlasAsset {
    let mut metadata = source.metadata.clone();
    if metadata.atlas_image_path.trim().is_empty() {
        metadata.atlas_image_path = format!("fonts/{}.fontatlas.r8", metadata.font_atlas_id);
    }
    metadata
}

fn default_rule_manifest() -> RuntimeRuleManifest {
    RuntimeRuleManifest {
        schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
        mode: "none".to_string(),
        rules: Vec::new(),
        modules: Vec::new(),
    }
}

fn build_input_manifest(input: &RuntimePackageBuildInput) -> RuntimeInputManifest {
    let mappings = input
        .input_mappings
        .iter()
        .map(|mapping| RuntimeInputMappingManifestEntry {
            id: mapping.id.clone(),
            path: format!("input/{}.json", mapping.id),
            enabled: true,
        })
        .collect::<Vec<_>>();
    let default_mapping_id = mappings
        .first()
        .map(|mapping| mapping.id.clone())
        .unwrap_or_default();
    RuntimeInputManifest {
        schema_version: RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION.to_string(),
        default_mapping_id,
        mappings,
    }
}

fn build_asset_manifest(input: &RuntimePackageBuildInput) -> RuntimeAssetManifest {
    let texture_payload_by_asset = input
        .texture_payloads
        .iter()
        .map(|source| (source.metadata.asset_id.as_str(), &source.metadata))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut assets = input
        .assets
        .iter()
        .map(|asset| RuntimeAsset {
            id: asset.asset_id.clone(),
            name: asset.name.clone(),
            asset_type: asset.asset_type.clone(),
            source: asset.source.clone(),
            state: "available".to_string(),
            bundle_id: "startup".to_string(),
            data: None,
        })
        .collect::<Vec<_>>();
    assets.extend(input.prefabs.iter().map(prefab_runtime_asset));

    let mut runtime_asset_index = input
        .assets
        .iter()
        .map(|asset| {
            let texture_metadata = texture_payload_by_asset.get(asset.asset_id.as_str());
            RuntimeAssetRecord {
                asset_guid: asset
                    .asset_guid
                    .clone()
                    .unwrap_or_else(|| asset.asset_id.clone()),
                asset_id: asset.asset_id.clone(),
                asset_type: asset.asset_type.clone(),
                sub_asset_id: None,
                version: "1".to_string(),
                cooked_asset_id: texture_metadata
                    .map(|metadata| metadata.cooked_asset_id.clone())
                    .unwrap_or_else(|| format!("cooked-{}", asset.asset_id)),
                bundle_id: "startup".to_string(),
                loader_kind: if texture_metadata.is_some() {
                    "texture".to_string()
                } else {
                    asset.asset_type.clone()
                },
                dependencies: asset.dependencies.clone(),
                hash: texture_metadata
                    .map(|metadata| metadata.source_hash.clone())
                    .or_else(|| asset.hash.clone()),
                size: texture_metadata.map(|metadata| metadata.byte_length as u64),
                flags: vec!["runtime_package_builder".to_string()],
                source_map_debug: Some(asset.source.clone()),
            }
        })
        .collect::<Vec<_>>();
    runtime_asset_index.extend(input.prefabs.iter().map(|prefab| RuntimeAssetRecord {
        asset_guid: prefab.prefab_id.clone(),
        asset_id: prefab.prefab_id.clone(),
        asset_type: "prefab".to_string(),
        sub_asset_id: None,
        version: "1".to_string(),
        cooked_asset_id: format!("cooked-{}", prefab.prefab_id),
        bundle_id: "startup".to_string(),
        loader_kind: "prefab".to_string(),
        dependencies: Vec::new(),
        hash: Some(prefab_content_hash(prefab)),
        size: None,
        flags: vec![
            "runtime_package_builder".to_string(),
            "runtime_prefab_asset".to_string(),
        ],
        source_map_debug: Some(prefab_package_path(&prefab.prefab_id)),
    }));

    let mut cooked_asset_table = input
        .assets
        .iter()
        .map(|asset| {
            let texture_metadata = texture_payload_by_asset.get(asset.asset_id.as_str());
            CookedAssetRecord {
                cooked_asset_id: texture_metadata
                    .map(|metadata| metadata.cooked_asset_id.clone())
                    .unwrap_or_else(|| format!("cooked-{}", asset.asset_id)),
                bundle_id: "startup".to_string(),
                path: Some(
                    texture_metadata
                        .map(|metadata| texture_metadata_package_path(&metadata.asset_id))
                        .unwrap_or_else(|| asset.runtime_uri.clone()),
                ),
                offset: None,
                size: texture_metadata
                    .map(|metadata| metadata.byte_length as u64)
                    .or(None),
                compression: Some("none".to_string()),
                hash: texture_metadata
                    .map(|metadata| metadata.source_hash.clone())
                    .or_else(|| asset.hash.clone()),
            }
        })
        .collect::<Vec<_>>();
    cooked_asset_table.extend(input.prefabs.iter().map(|prefab| CookedAssetRecord {
        cooked_asset_id: format!("cooked-{}", prefab.prefab_id),
        bundle_id: "startup".to_string(),
        path: Some(prefab_package_path(&prefab.prefab_id)),
        offset: None,
        size: None,
        compression: Some("none".to_string()),
        hash: Some(prefab_content_hash(prefab)),
    }));
    let dependency_table = input
        .assets
        .iter()
        .filter(|asset| !asset.dependencies.is_empty())
        .map(|asset| RuntimeAssetDependencyRecord {
            asset_guid: asset
                .asset_guid
                .clone()
                .unwrap_or_else(|| asset.asset_id.clone()),
            dependencies: asset.dependencies.clone(),
        })
        .collect::<Vec<_>>();
    RuntimeAssetManifest {
        schema_version: RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        assets,
        runtime_asset_index,
        bundle_table: vec![BundleRecord {
            bundle_id: "startup".to_string(),
            mount_id: Some("local".to_string()),
            uri: "cooked-assets/startup".to_string(),
            hash: None,
            version: Some("1".to_string()),
            mounted: true,
        }],
        cooked_asset_table,
        dependency_table,
    }
}

fn prefab_runtime_asset(prefab: &RuntimePackageSourcePrefab) -> RuntimeAsset {
    let document = prefab_runtime_document(prefab);
    RuntimeAsset {
        id: prefab.prefab_id.clone(),
        name: document
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&prefab.prefab_id)
            .to_string(),
        asset_type: "prefab".to_string(),
        source: prefab_package_path(&prefab.prefab_id),
        state: "available".to_string(),
        bundle_id: "startup".to_string(),
        data: Some(document),
    }
}

fn prefab_runtime_document(prefab: &RuntimePackageSourcePrefab) -> serde_json::Value {
    runtime_prefab_document_from_source(prefab)
        .expect("validated RuntimePackageSourcePrefab should convert to runtime prefab")
}

fn prefab_content_hash(prefab: &RuntimePackageSourcePrefab) -> String {
    let document = prefab_runtime_document(prefab);
    let bytes = canonical_json_bytes(&document)
        .expect("validated RuntimePackageSourcePrefab must be canonically encodable");
    sha256_prefixed(&bytes)
}

fn prefab_package_path(prefab_id: &str) -> String {
    format!("prefabs/{prefab_id}.json")
}

fn texture_metadata_package_path(asset_id: &str) -> String {
    format!("cooked/textures/{asset_id}.texture.json")
}

fn build_font_atlas_manifest(input: &RuntimePackageBuildInput) -> RuntimeFontAtlasManifest {
    let atlases = input
        .font_atlases
        .iter()
        .map(|source| {
            let metadata = &source.metadata;
            let bitmap_path = if metadata.atlas_image_path.trim().is_empty() {
                format!("fonts/{}.fontatlas.r8", metadata.font_atlas_id)
            } else {
                metadata.atlas_image_path.clone()
            };
            RuntimeFontAtlasManifestEntry {
                font_atlas_id: metadata.font_atlas_id.clone(),
                metadata_path: format!("fonts/{}.fontatlas.json", metadata.font_atlas_id),
                bitmap_path,
                glyph_count: metadata.glyphs.len(),
                atlas_width: metadata.atlas_width,
                atlas_height: metadata.atlas_height,
                font_source_kind: metadata.font_source_kind.clone(),
                font_asset_status: metadata.font_asset_status.clone(),
                fallback_used: metadata.fallback_used,
            }
        })
        .collect::<Vec<_>>();
    RuntimeFontAtlasManifest {
        schema_version: RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION.to_string(),
        default_ui_font_atlas_id: atlases.first().map(|atlas| atlas.font_atlas_id.clone()),
        atlases,
    }
}

fn build_font_bundle_manifest(input: &RuntimePackageBuildInput) -> RuntimeFontBundleManifest {
    let bundles = input
        .font_bundles
        .iter()
        .map(|source| RuntimeFontBundleManifestEntry {
            font_bundle_id: source.metadata.font_bundle_id.clone(),
            metadata_path: format!("fonts/{}/font-bundle.json", source.metadata.font_bundle_id),
            page_paths: source
                .metadata
                .pages
                .iter()
                .map(|page| page.payload_path.clone())
                .collect(),
            bundle_digest: source.metadata.bundle_digest.clone(),
            legacy_mode: source.metadata.legacy_mode,
            fallback_used: source.metadata.fallback_used,
            quality_gate_eligible: source.metadata.quality_gate_eligible,
        })
        .collect::<Vec<_>>();
    RuntimeFontBundleManifest {
        schema_version: RUNTIME_FONT_BUNDLE_MANIFEST_SCHEMA_VERSION.to_string(),
        default_ui_font_bundle_id: bundles.first().map(|bundle| bundle.font_bundle_id.clone()),
        bundles,
    }
}

fn checked_counts(input: &RuntimePackageBuildInput) -> RuntimePackageCheckedCounts {
    RuntimePackageCheckedCounts {
        scenes: input.scenes.len(),
        entities: input.scenes.iter().map(|scene| scene.entities.len()).sum(),
        components: input
            .scenes
            .iter()
            .flat_map(|scene| scene.entities.iter())
            .map(count_entity_components)
            .sum(),
        assets: input.assets.len(),
        prefabs: input.prefabs.len(),
    }
}

fn count_entity_components(entity: &RuntimeEntity) -> usize {
    usize::from(entity.transform.is_some())
        + usize::from(entity.mesh.is_some())
        + entity.components.len()
}

fn build_diff_report(
    request: &RuntimePackageBuildRequest,
    package_hash: &str,
) -> RuntimePackageDiffReport {
    let mut changes = Vec::new();
    if let Some(previous) = &request.previous_package_manifest {
        if previous.hash != package_hash {
            changes.push(RuntimePackageDiffChange {
                kind: "manifest".to_string(),
                id: "manifest".to_string(),
                change: "modified".to_string(),
                summary: "runtime package manifest hash changed".to_string(),
            });
        }
    }
    RuntimePackageDiffReport {
        schema_version: RUNTIME_PACKAGE_DIFF_REPORT_SCHEMA_VERSION.to_string(),
        previous_package_id: request
            .previous_package_manifest
            .as_ref()
            .map(|previous| previous.package_id.clone()),
        current_package_id: format!("pkg-{}", package_hash),
        changes,
    }
}

fn status_from_diagnostics(diagnostics: &[RuntimePackageDiagnostic]) -> RuntimePackageBuildStatus {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == RuntimePackageDiagnosticSeverity::Error)
    {
        RuntimePackageBuildStatus::Failed
    } else {
        RuntimePackageBuildStatus::Success
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animator2d::{
        Animator2DPlayback, CookedAnimator2DRegistry, CookedAnimator2DState,
        CookedAnimatorController2D, CookedSpriteAnimationClip2D, CookedSpriteAnimationFrame2D,
    };
    use crate::font_bundle::{
        font_bundle_digest, CookedFontBundleAsset, CookedFontBundleGlyph, CookedFontBundlePage,
        FontBundleRenderMode, RuntimePackageSourceFontBundle, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
    };
    use crate::project_observation::{
        ProjectObservationContract, ProjectObservationEntry, ProjectObservationType,
        ProjectObservationValue, PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION,
    };
    use crate::runtime_package::{
        CookedFontAtlasGlyph, RuntimeAssetRef, RuntimeAuiManifestEntry, RuntimeMesh,
        RuntimeTransform, Vector3, COOKED_FONT_ATLAS_SCHEMA_VERSION, COOKED_TEXTURE_SCHEMA_VERSION,
        RUNTIME_AUI_MANIFEST_SCHEMA_VERSION, RUNTIME_ENTITY_SCHEMA_VERSION,
        RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_package_builder_writes_loadable_minimal_scene_package() {
        let root = temp_root("minimal");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        assert!(package_dir.join("manifest.json").exists());
        assert!(package_dir.join("scenes").join("scene-main.json").exists());
        assert!(package_dir
            .join("assets")
            .join("asset-manifest.json")
            .exists());
        assert!(package_dir
            .join("assets")
            .join("runtime-asset-index.json")
            .exists());
        let loaded = load_runtime_package(&package_dir);
        assert!(
            loaded.diagnostics.is_ok(),
            "{:?}",
            loaded.diagnostics.issues
        );
        let package = loaded.value.unwrap();
        assert_eq!(package.active_scene.id, "scene-main");
        let content_hash = package.manifest.content_hash.as_deref().unwrap();
        validate_runtime_content_hash_evidence(content_hash).unwrap();
        assert_eq!(
            report.outputs.runtime_content_hash.as_deref(),
            Some(content_hash)
        );
        assert!(report
            .outputs
            .payload_tree_digest
            .as_deref()
            .is_some_and(is_canonical_runtime_content_hash));

        let manifest_path = package_dir.join("manifest.json");
        let mut legacy_manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        legacy_manifest["contentHash"] = serde_json::json!("legacy-opaque-hash");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&legacy_manifest).unwrap(),
        )
        .unwrap();
        let legacy_loaded = load_runtime_package(&package_dir);
        assert!(legacy_loaded.diagnostics.is_ok());
        assert_eq!(
            legacy_loaded
                .value
                .unwrap()
                .manifest
                .content_hash
                .as_deref(),
            Some("legacy-opaque-hash")
        );
        assert_eq!(
            validate_runtime_content_hash_evidence("legacy-opaque-hash")
                .unwrap_err()
                .code,
            UNSUPPORTED_CONTENT_HASH_ALGORITHM_DIAGNOSTIC
        );
    }

    #[test]
    fn animator2d_package_round_trip_covers_manifest_digest_and_registry_payload() {
        let root = temp_root("animator2d-package-round-trip");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input.scenes.push(RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
            id: "scene-main".to_string(),
            name: "Main".to_string(),
            gravity: 0.0,
            background: "#000".to_string(),
            sky_color: "#000".to_string(),
            entities: Vec::new(),
        });
        input.animator2d_registry = fixture_animator2d_registry();
        let digest_with_registry = input.assembly_input_digest().unwrap();
        let mut without_registry = input.clone();
        without_registry.animator2d_registry = CookedAnimator2DRegistry::empty();
        assert_ne!(
            digest_with_registry,
            without_registry.assembly_input_digest().unwrap()
        );

        let report = RuntimePackageBuilder::build(&request, &input);
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{:?}",
            report.diagnostics
        );
        assert!(package_dir.join("animator2d/registry.json").is_file());
        let package = load_runtime_package(&package_dir)
            .value
            .expect("load package");
        assert_eq!(package.animator2d_registry, input.animator2d_registry);
        let index = package
            .manifest
            .animator2d
            .expect("Animator2D manifest index");
        assert_eq!(
            index.registry_digest,
            input.animator2d_registry.registry_digest
        );
        assert_eq!(index.clip_count, 1);
        assert_eq!(index.controller_count, 1);

        fs::remove_file(package_dir.join("animator2d/registry.json")).unwrap();
        let missing_registry = load_runtime_package(&package_dir);
        assert!(missing_registry.diagnostics.has_errors());
        assert!(missing_registry
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.path == "animator2d.registry"));
    }

    #[test]
    fn project_observation_contract_is_cooked_into_manifest_and_changes_assembly_digest() {
        let root = temp_root("project-observation-contract");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        let without_contract_digest = input.assembly_input_digest().unwrap();
        input.observation_contract = Some(fixture_observation_contract());

        let with_contract_digest = input.assembly_input_digest().unwrap();
        let report = RuntimePackageBuilder::build(&request, &input);

        assert_ne!(without_contract_digest, with_contract_digest);
        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        assert!(!report
            .outputs
            .payload_inventory
            .iter()
            .any(|path| path.starts_with("Observations/")));
        let loaded = load_runtime_package(&package_dir);
        assert!(
            loaded.diagnostics.is_ok(),
            "{:?}",
            loaded.diagnostics.issues
        );
        let cooked = loaded
            .value
            .unwrap()
            .manifest
            .observation_contract
            .expect("cooked observation contract");
        assert_eq!(cooked.contract_id, "sample.runtime-observations");
        assert!(cooked.contract_digest.starts_with("sha256:"));
        cooked.validate().unwrap();
    }

    #[test]
    fn font_preview_play_export_parity_uses_identical_cooked_bundle_digest() {
        let root = temp_root("font-preview-play-export-parity");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        input.font_bundles.push(test_font_bundle_source());
        let mut digests = Vec::new();

        for consumer in ["preview", "play", "export"] {
            let package_dir = root.join(consumer);
            let report = RuntimePackageBuilder::build(
                &RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main"),
                &input,
            );
            assert_eq!(
                report.status,
                RuntimePackageBuildStatus::Success,
                "{:?}",
                report.diagnostics
            );
            let package = load_runtime_package(&package_dir)
                .value
                .expect("load built package");
            let bundle = package
                .font_bundles
                .default_bundle()
                .expect("default v2 font bundle");
            digests.push(bundle.metadata.bundle_digest.clone());
            assert!(files_under(&package_dir).iter().all(|path| {
                !matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("ttf" | "otf" | "ttc")
                )
            }));
        }

        assert!(digests.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn runtime_package_builder_writes_prefab_input_schema_and_aui_files() {
        let root = temp_root("sections");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        input.prefabs.push(RuntimePackageSourcePrefab {
            prefab_id: "prefab-basic".to_string(),
            document: serde_json::json!({
                "schemaVersion": "runtime-prefab.v1",
                "id": "prefab-basic",
                "name": "Basic Prefab",
                "rootEntityId": null,
                "entities": []
            }),
        });
        input.input_mappings.push(RuntimePackageSourceJson {
            id: "input-default".to_string(),
            document: serde_json::to_value(InputMappingAsset::new(
                "input-default",
                vec![engine_input::InputActionDefinition::new(
                    "action.fire",
                    engine_input::InputActionValueType::Button,
                )],
                vec![engine_input::InputContextDefinition::new("gameplay", 0)],
                vec![engine_input::InputBindingDefinition::button(
                    "action.fire",
                    "Space",
                )],
            ))
            .expect("input mapping should serialize"),
        });
        input.component_schema =
            Some(serde_json::json!({ "schemaVersion": "component-schema.v1", "components": [] }));
        input.aui_documents.push(RuntimePackageSourceJson {
            id: "hud".to_string(),
            document: serde_json::json!({
                "schema_version": "aui-document.v2",
                "document_id": "hud",
                "canvases": [{
                    "canvas_id": "main",
                    "mode": "ScreenOverlay",
                    "layer": 0,
                    "sorting_order": 0,
                    "reference_resolution": { "x": 1280.0, "y": 720.0 },
                    "scale_mode": "ConstantPixelSize",
                    "root_node": "root"
                }],
                "nodes": [{
                    "node_id": "root",
                    "name": "root",
                    "kind": "Panel",
                    "parent": null,
                    "children": [],
                    "rect": {
                        "anchor_min": { "x": 0.0, "y": 0.0 },
                        "anchor_max": { "x": 1.0, "y": 1.0 },
                        "offset_min": { "x": 0.0, "y": 0.0 },
                        "offset_max": { "x": 0.0, "y": 0.0 },
                        "pivot": { "x": 0.5, "y": 0.5 },
                        "size": { "x": 0.0, "y": 0.0 }
                    },
                    "visible": true,
                    "interactable": false,
                    "consume_input": true,
                    "style": null,
                    "text": null,
                    "image": null,
                    "progress_value": null,
                    "binding_refs": [],
                    "action_refs": []
                }]
            }),
        });
        input.aui_manifest = Some(RuntimeAuiManifest {
            schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.to_string(),
            documents: vec![RuntimeAuiManifestEntry {
                document_id: "hud".to_string(),
                path: "aui/documents/hud.aui.json".to_string(),
                canvas_count: 1,
                node_count: 1,
                binding_count: 0,
                action_count: 0,
                asset_refs: Vec::new(),
            }],
        });

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        assert!(package_dir
            .join("prefabs")
            .join("prefab-basic.json")
            .exists());
        assert!(package_dir
            .join("input")
            .join("input-default.json")
            .exists());
        assert!(package_dir
            .join("schema")
            .join("component-schema.json")
            .exists());
        assert!(package_dir.join("aui").join("aui-manifest.json").exists());
        assert!(package_dir
            .join("aui")
            .join("documents")
            .join("hud.aui.json")
            .exists());
        let manifest = load_runtime_package(&package_dir)
            .value
            .expect("runtime package should load");
        assert_eq!(
            manifest.manifest.aui.as_ref().map(|aui| aui.document_count),
            Some(1)
        );
        assert_eq!(manifest.aui_manifest.documents[0].document_id, "hud");
        assert_eq!(manifest.aui_documents.len(), 1);
        assert_eq!(
            manifest.manifest.assets.asset_count,
            manifest.assets.assets.len()
        );
        let prefab_asset = manifest
            .assets
            .assets
            .iter()
            .find(|asset| asset.id == "prefab-basic" && asset.asset_type == "prefab")
            .expect("prefab should be listed as a runtime asset");
        assert_eq!(
            prefab_asset
                .data
                .as_ref()
                .and_then(|data| data.get("schemaVersion"))
                .and_then(serde_json::Value::as_str),
            Some("runtime-prefab.v1")
        );
        let prefab_record = manifest
            .runtime_asset_index
            .resolve(&RuntimeAssetRef {
                id: "prefab-basic".to_string(),
                asset_type: "prefab".to_string(),
                guid: None,
                sub_asset: None,
            })
            .expect("prefab should resolve through RuntimeAssetIndex");
        assert_eq!(prefab_record.loader_kind, "prefab");
        assert!(manifest.assets.cooked_asset_table.iter().any(|cooked| {
            cooked.cooked_asset_id == prefab_record.cooked_asset_id
                && cooked.path.as_deref() == Some("prefabs/prefab-basic.json")
        }));
    }

    #[test]
    fn runtime_package_builder_writes_and_loads_cooked_font_atlas() {
        let root = temp_root("font-atlas");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        input.font_atlases.push(test_font_atlas_source());

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        assert!(package_dir
            .join("fonts")
            .join("font-atlas-manifest.json")
            .exists());
        assert!(package_dir
            .join("fonts")
            .join("ui-default-cmin.fontatlas.json")
            .exists());
        assert!(package_dir
            .join("fonts")
            .join("ui-default-cmin.fontatlas.r8")
            .exists());
        let package = load_runtime_package(&package_dir)
            .value
            .expect("runtime package should load");
        assert_eq!(package.font_atlas_manifest.atlases.len(), 1);
        assert_eq!(package.font_atlases.len(), 1);
        assert_eq!(
            package.font_atlases.default_ui_font_atlas_id.as_deref(),
            Some("ui-default-cmin")
        );
        let atlas = package
            .font_atlases
            .default_atlas()
            .expect("default atlas should load");
        assert_eq!(
            atlas.metadata.glyph('H').map(|glyph| glyph.codepoint),
            Some('H' as u32)
        );
        assert_eq!(package.font_atlases.load_report.loaded_atlas_count, 1);
        let legacy_report = &package.font_atlases.load_report.atlases[0];
        assert!(legacy_report.legacy_mode);
        assert!(!legacy_report.quality_gate_eligible);
    }

    #[test]
    fn runtime_package_builder_writes_and_loads_cooked_font_bundle_v2() {
        let root = temp_root("font-bundle");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        input.font_bundles.push(test_font_bundle_source());

        let report = RuntimePackageBuilder::build(&request, &input);
        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        let package = load_runtime_package(&package_dir)
            .value
            .expect("runtime package should load");
        assert_eq!(package.font_bundle_manifest.bundles.len(), 1);
        let bundle = package
            .font_bundles
            .default_bundle()
            .expect("default v2 bundle");
        assert_eq!(
            bundle.metadata.schema_version,
            COOKED_FONT_BUNDLE_SCHEMA_VERSION
        );
        assert!(bundle.metadata.quality_gate_eligible);
        assert_eq!(bundle.page_payloads.len(), 1);
    }

    #[test]
    fn runtime_package_builder_writes_cooked_texture_payload_and_index() {
        let root = temp_root("texture-payload");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.asset",
            "cooked/texture-main.asset",
        ));
        input.texture_payloads.push(RuntimePackageSourceTexture {
            metadata: CookedTextureAsset {
                schema_version: COOKED_TEXTURE_SCHEMA_VERSION.to_string(),
                asset_id: "texture-main".to_string(),
                cooked_asset_id: "cooked-texture-main".to_string(),
                source_hash: "hash-texture-main".to_string(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".to_string(),
                color_space: "srgb".to_string(),
                mip_count: 1,
                byte_length: 4,
                pixel_data_path: "cooked/textures/texture-main.rgba8".to_string(),
                sampler: "linearClamp".to_string(),
            },
            rgba8: vec![32, 64, 128, 255],
        });

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        let metadata_path = package_dir.join("cooked/textures/texture-main.texture.json");
        let payload_path = package_dir.join("cooked/textures/texture-main.rgba8");
        assert!(metadata_path.exists());
        assert_eq!(fs::read(payload_path).unwrap(), vec![32, 64, 128, 255]);
        let metadata: CookedTextureAsset =
            serde_json::from_str(&fs::read_to_string(metadata_path).unwrap()).unwrap();
        assert_eq!(metadata.source_hash, "hash-texture-main");
        let index_text =
            fs::read_to_string(package_dir.join("assets").join("runtime-asset-index.json"))
                .unwrap();
        assert!(index_text.contains("\"loaderKind\": \"texture\""));
        assert!(index_text.contains("\"hash\": \"hash-texture-main\""));
        let manifest_text =
            fs::read_to_string(package_dir.join("assets").join("asset-manifest.json")).unwrap();
        assert!(manifest_text.contains("\"path\": \"cooked/textures/texture-main.texture.json\""));
        assert!(manifest_text.contains("\"hash\": \"hash-texture-main\""));
    }

    #[test]
    fn runtime_package_builder_owns_generic_asset_payload_bytes() {
        let root = temp_root("generic-asset-payload");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "font-main"));
        let payload = br#"{"schemaVersion":"font-asset.v1","assetId":"font-main"}"#.to_vec();
        input.assets.push(
            RuntimePackageSourceAsset::new(
                "font-main",
                "Main Font",
                "asset",
                "Assets/font-main.asset",
                "cooked/font-main.asset",
            )
            .with_runtime_payload(payload.clone()),
        );

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        assert_eq!(
            fs::read(package_dir.join("cooked/font-main.asset")).unwrap(),
            payload
        );
        assert!(load_runtime_package(&package_dir).diagnostics.is_ok());
    }

    #[test]
    fn runtime_package_builder_converts_authoring_prefab_to_runtime_prefab() {
        let root = temp_root("authoring-prefab");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        input.prefabs.push(RuntimePackageSourcePrefab {
            prefab_id: "prefab-ship".to_string(),
            document: serde_json::json!({
                "schemaVersion": "authoring-prefab-asset.v1",
                "prefabId": "prefab-ship",
                "name": "Ship",
                "rootEntityId": "entity-root",
                "entities": [
                    {
                        "sourceEntityId": "entity-root",
                        "name": "ShipRoot",
                        "parentSourceEntityId": null,
                        "siblingOrder": 0,
                        "enabled": true,
                        "transform": {
                            "localPosition": { "x": 0.0, "y": 1.0, "z": 0.0 },
                            "localRotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
                            "localScale": { "x": 1.0, "y": 1.0, "z": 1.0 }
                        },
                        "components": [
                            {
                                "componentType": "SpriteRenderer2D",
                                "fields": {
                                    "spriteRef": { "id": "texture-main", "type": "texture" },
                                    "sortingLayer": 0,
                                    "orderInLayer": 3,
                                    "visible": true
                                }
                            },
                            {
                                "componentType": "project.stats",
                                "fields": { "speed": 3.0 }
                            }
                        ]
                    }
                ]
            }),
        });

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        let prefab_text =
            fs::read_to_string(package_dir.join("prefabs").join("prefab-ship.json")).unwrap();
        let prefab_json: serde_json::Value = serde_json::from_str(&prefab_text).unwrap();
        assert_eq!(prefab_json["schemaVersion"], "runtime-prefab.v1");
        assert_eq!(prefab_json["id"], "prefab-ship");
        assert_eq!(prefab_json["entities"][0]["id"], "entity-root");
        assert_eq!(
            prefab_json["entities"][0]["spriteRenderer2D"]["spriteRef"]["id"],
            "texture-main"
        );
        assert_eq!(
            prefab_json["entities"][0]["components"][0]["componentType"],
            "project.stats"
        );
        assert_eq!(
            prefab_json["entities"][0]["components"][0]["data"]["speed"],
            3.0
        );
    }

    #[test]
    fn runtime_package_builder_reports_missing_asset_ref() {
        let root = temp_root("missing-asset");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "missing-texture"));

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MissingAssetRef"));
        assert!(!package_dir.exists());
    }

    #[test]
    fn runtime_package_builder_writes_manifest_diff_report() {
        let root = temp_root("diff");
        let package_dir = root.join("runtime-package");
        let mut request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        request.previous_package_manifest = Some(PreviousPackageManifest {
            package_id: "pkg-old".to_string(),
            hash: "old".to_string(),
        });
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));

        let report = RuntimePackageBuilder::build(&request, &input);

        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        let diff_text = fs::read_to_string(
            package_dir
                .join("reports")
                .join("runtime-package-diff-report.json"),
        )
        .unwrap();
        assert!(diff_text.contains("\"change\": \"modified\""));
    }

    #[test]
    fn runtime_package_digest_changes_for_effective_payload_but_not_recipe_only_request_fields() {
        let root = temp_root("digest-scope");
        let request = RuntimePackageBuildRequest::dev_desktop(root.join("one"), "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        input.component_schema = Some(serde_json::json!({
            "schemaVersion": "component-schema.v1",
            "components": [{ "type": "project.stats", "fields": ["health"] }]
        }));
        input.font_atlases.push(test_font_atlas_source());
        input.texture_payloads.push(RuntimePackageSourceTexture {
            metadata: CookedTextureAsset {
                schema_version: COOKED_TEXTURE_SCHEMA_VERSION.to_string(),
                asset_id: "texture-main".to_string(),
                cooked_asset_id: "cooked-texture-main".to_string(),
                source_hash: "hash-texture-main".to_string(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".to_string(),
                color_space: "srgb".to_string(),
                mip_count: 1,
                byte_length: 4,
                pixel_data_path: "cooked/textures/texture-main.rgba8".to_string(),
                sampler: "linearClamp".to_string(),
            },
            rgba8: vec![32, 64, 128, 255],
        });

        let initial = RuntimePackageWritePlan::build(&request, &input).unwrap();
        assert!(initial
            .runtime_content_hash
            .manifest_value()
            .starts_with("sha256:"));
        assert_eq!(initial.runtime_content_hash.manifest_value().len(), 71);

        let mut recipe_only_request = request.clone();
        recipe_only_request.output_dir = root.join("two");
        recipe_only_request.mode = "release-export".to_string();
        recipe_only_request.target = "windows".to_string();
        recipe_only_request.previous_package_manifest = Some(PreviousPackageManifest {
            package_id: "previous".to_string(),
            hash: "legacy-opaque-hash".to_string(),
        });
        let recipe_only = RuntimePackageWritePlan::build(&recipe_only_request, &input).unwrap();
        assert_eq!(
            initial.runtime_content_hash.manifest_value(),
            recipe_only.runtime_content_hash.manifest_value()
        );

        let mut effective_change = input.clone();
        effective_change.scenes[0].entities[0].name = "Changed Root".to_string();
        let changed = RuntimePackageWritePlan::build(&request, &effective_change).unwrap();
        verify_runtime_content_hash_mutation(
            &initial.runtime_content_hash,
            &changed.runtime_content_hash,
            "scene entity name",
        )
        .unwrap();
        let mut component_schema_change = input.clone();
        component_schema_change.component_schema = Some(serde_json::json!({
            "schemaVersion": "component-schema.v1",
            "components": [{ "type": "project.stats", "fields": ["shield"] }]
        }));
        let component_schema_changed =
            RuntimePackageWritePlan::build(&request, &component_schema_change).unwrap();
        verify_runtime_content_hash_mutation(
            &initial.runtime_content_hash,
            &component_schema_changed.runtime_content_hash,
            "component schema field",
        )
        .unwrap();
        let mut font_bitmap_change = input.clone();
        font_bitmap_change.font_atlases[0].atlas_alpha[0] = 0;
        let font_bitmap_changed =
            RuntimePackageWritePlan::build(&request, &font_bitmap_change).unwrap();
        verify_runtime_content_hash_mutation(
            &initial.runtime_content_hash,
            &font_bitmap_changed.runtime_content_hash,
            "font atlas bitmap byte",
        )
        .unwrap();
        let mut texture_rgba_change = input.clone();
        texture_rgba_change.texture_payloads[0].rgba8[0] = 0;
        let texture_rgba_changed =
            RuntimePackageWritePlan::build(&request, &texture_rgba_change).unwrap();
        verify_runtime_content_hash_mutation(
            &initial.runtime_content_hash,
            &texture_rgba_changed.runtime_content_hash,
            "texture RGBA byte",
        )
        .unwrap();
        validate_runtime_content_hash_evidence(&initial.runtime_content_hash.manifest_value())
            .unwrap();
        let legacy = validate_runtime_content_hash_evidence("legacy-opaque-hash").unwrap_err();
        assert_eq!(legacy.code, UNSUPPORTED_CONTENT_HASH_ALGORITHM_DIAGNOSTIC);
    }

    #[test]
    fn runtime_package_digest_normalizes_top_level_source_order_and_preserves_payload_tree() {
        let root = temp_root("digest-order");
        let request =
            RuntimePackageBuildRequest::dev_desktop(root.join("runtime-package"), "scene-main");
        let mut first = fixture_input();
        first
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        first.assets.push(RuntimePackageSourceAsset::new(
            "asset-b",
            "B",
            "texture",
            "Assets/b.png",
            "cooked-assets/b",
        ));
        first.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        first.aui_documents.push(RuntimePackageSourceJson {
            id: "doc-b".to_string(),
            document: serde_json::from_str("{\"b\": 2, \"a\": 1}").unwrap(),
        });
        first.aui_documents.push(RuntimePackageSourceJson {
            id: "doc-a".to_string(),
            document: serde_json::from_str("{ \"a\" : 1, \"b\" : 2 }").unwrap(),
        });

        let mut second = first.clone();
        second.assets.reverse();
        second.aui_documents.reverse();
        let first_plan = RuntimePackageWritePlan::build(&request, &first).unwrap();
        let second_plan = RuntimePackageWritePlan::build(&request, &second).unwrap();
        assert_eq!(
            first_plan.runtime_content_hash.manifest_value(),
            second_plan.runtime_content_hash.manifest_value()
        );
        assert_eq!(
            first_plan.payload_tree_digest().unwrap().0.value,
            second_plan.payload_tree_digest().unwrap().0.value
        );

        let assembly_first = first.assembly_input_digest().unwrap();
        let assembly_second = second.assembly_input_digest().unwrap();
        assert_eq!(assembly_first.0.value, assembly_second.0.value);
    }

    #[test]
    fn runtime_package_staging_replaces_final_and_removes_stale_payloads() {
        let root = temp_root("staging-stale");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        assert_eq!(
            RuntimePackageBuilder::build(&request, &input).status,
            RuntimePackageBuildStatus::Success
        );
        fs::write(package_dir.join("stale-sentinel.bin"), b"stale").unwrap();
        assert_eq!(
            RuntimePackageBuilder::build(&request, &input).status,
            RuntimePackageBuildStatus::Success
        );
        assert!(!package_dir.join("stale-sentinel.bin").exists());
        assert!(load_runtime_package(&package_dir).diagnostics.is_ok());
    }

    #[test]
    fn runtime_package_publish_faults_restore_last_good_final() {
        let root = temp_root("publish-rollback");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        assert_eq!(
            RuntimePackageBuilder::build(&request, &input).status,
            RuntimePackageBuildStatus::Success
        );
        let last_good_manifest = fs::read(package_dir.join("manifest.json")).unwrap();
        let mut changed = input.clone();
        changed.scenes[0].name = "Changed".to_string();
        let changed_plan = RuntimePackageWritePlan::build(&request, &changed).unwrap();

        for fault in [
            PublishFaultPoint::AfterStagingWrite,
            PublishFaultPoint::BeforePublishRename,
            PublishFaultPoint::AfterPublishRename,
        ] {
            let error = publish_runtime_package(&package_dir, &changed_plan, fault).unwrap_err();
            assert!(error.code.contains("staging") || error.code.contains("publish"));
            assert_eq!(
                fs::read(package_dir.join("manifest.json")).unwrap(),
                last_good_manifest,
                "fault={fault:?}"
            );
            assert!(load_runtime_package(&package_dir).diagnostics.is_ok());
        }
    }

    #[test]
    fn runtime_package_publish_guard_is_single_writer_and_returns_busy_without_waiting() {
        use std::sync::mpsc;
        use std::time::Duration;

        let root = temp_root("publish-lock");
        fs::create_dir_all(&root).unwrap();
        let package_dir = root.join("runtime-package");
        let first = RuntimePackagePublishGuard::acquire(&package_dir).unwrap();
        let (sender, receiver) = mpsc::channel();
        let competing_dir = package_dir.clone();
        let worker = std::thread::spawn(move || {
            sender
                .send(RuntimePackagePublishGuard::acquire(&competing_dir))
                .unwrap();
        });
        let result = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("competing publisher must return within hard timeout");
        assert_eq!(result.unwrap_err().code, "output_publish_busy");
        drop(first);
        worker.join().unwrap();
        RuntimePackagePublishGuard::acquire(&package_dir).unwrap();
    }

    #[test]
    fn runtime_package_path_rejects_unsafe_generated_ids_and_case_collisions() {
        let root = temp_root("unsafe-id");
        let request =
            RuntimePackageBuildRequest::dev_desktop(root.join("runtime-package"), "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("../escape", "texture-main"));
        assert!(RuntimePackageWritePlan::build(&request, &input).is_err());

        let mut collision = fixture_input();
        collision
            .scenes
            .push(scene_with_texture_ref("Scene-Main", "texture-main"));
        collision
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        assert!(RuntimePackageWritePlan::build(&request, &collision).is_err());
    }

    #[test]
    fn runtime_package_path_loader_rejects_manifest_traversal_before_reading() {
        let root = temp_root("loader-traversal");
        let package_dir = root.join("runtime-package");
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = fixture_input();
        input
            .scenes
            .push(scene_with_texture_ref("scene-main", "texture-main"));
        input.assets.push(RuntimePackageSourceAsset::new(
            "texture-main",
            "Main Texture",
            "texture",
            "Assets/main.png",
            "cooked-assets/texture-main",
        ));
        assert_eq!(
            RuntimePackageBuilder::build(&request, &input).status,
            RuntimePackageBuildStatus::Success
        );
        let manifest_path = package_dir.join("manifest.json");
        let mut manifest: RuntimePackageManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.scenes[0].path = "../outside-scene.json".to_string();
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let loaded = load_runtime_package(&package_dir);
        assert!(loaded.diagnostics.has_errors());
        assert!(loaded
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.path == "manifest.scenes.path"));
    }

    fn fixture_input() -> RuntimePackageBuildInput {
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "project-fixture",
            "Fixture",
            "0.0.1",
        ));
        let mapping = InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        input
    }

    fn fixture_observation_contract() -> ProjectObservationContract {
        ProjectObservationContract {
            schema_version: PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION.to_string(),
            contract_id: "sample.runtime-observations".to_string(),
            observations: vec![ProjectObservationEntry {
                path: "sample.phase".to_string(),
                value_type: ProjectObservationType::String,
                description: "Current authoritative phase".to_string(),
                allowed_values: Some(vec![ProjectObservationValue::String("ready".to_string())]),
            }],
        }
    }

    fn fixture_animator2d_registry() -> CookedAnimator2DRegistry {
        CookedAnimator2DRegistry::from_parts(
            vec![CookedSpriteAnimationClip2D {
                id: "clip-idle".to_string(),
                playback: Animator2DPlayback::Loop,
                frames: vec![CookedSpriteAnimationFrame2D {
                    sprite_asset_id: "sprite-idle".to_string(),
                    duration_ticks: 2,
                }],
            }],
            vec![CookedAnimatorController2D {
                id: "controller-main".to_string(),
                entry_state_index: 0,
                parameters: Vec::new(),
                states: vec![CookedAnimator2DState {
                    id: "idle".to_string(),
                    clip_index: 0,
                    speed_permille: 1000,
                }],
                transitions: Vec::new(),
            }],
        )
        .unwrap()
    }

    fn test_font_atlas_source() -> RuntimePackageSourceFontAtlas {
        RuntimePackageSourceFontAtlas {
            metadata: CookedFontAtlasAsset {
                schema_version: COOKED_FONT_ATLAS_SCHEMA_VERSION.to_string(),
                font_atlas_id: "ui-default-cmin".to_string(),
                font_asset_id: "font-main".to_string(),
                font_source_kind: "engine_builtin_cooked_fallback".to_string(),
                font_asset_status: "placeholder".to_string(),
                atlas_image_path: "fonts/ui-default-cmin.fontatlas.r8".to_string(),
                atlas_format: "r8Alpha".to_string(),
                atlas_width: 8,
                atlas_height: 8,
                atlas_generation: 1,
                atlas_alpha_byte_len: 64,
                glyphs: vec![CookedFontAtlasGlyph {
                    codepoint: 'H' as u32,
                    glyph_id: "builtin-0048".to_string(),
                    uv_rect: [0.0, 0.0, 0.625, 0.875],
                    pixel_rect: [0, 0, 5, 7],
                    bearing_x: 0.0,
                    bearing_y: 7.0,
                    advance: 6.0,
                    page_index: 0,
                }],
                fallback_used: true,
                diagnostics: Vec::new(),
            },
            atlas_alpha: vec![255; 64],
        }
    }

    fn test_font_bundle_source() -> RuntimePackageSourceFontBundle {
        let payload = vec![255; 64];
        let mut metadata = CookedFontBundleAsset {
            schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
            font_bundle_id: "font-ui-v2".to_string(),
            font_stack_id: "stack-ui".to_string(),
            generation: 1,
            max_bitmap_pages: 2,
            max_msdf_pages: 2,
            legacy_mode: false,
            fallback_used: false,
            quality_gate_eligible: true,
            pages: vec![CookedFontBundlePage {
                page_index: 0,
                render_mode: FontBundleRenderMode::BitmapR8,
                format: "r8Unorm".to_string(),
                width: 8,
                height: 8,
                byte_len: payload.len(),
                sha256: sha256_prefixed(&payload),
                payload_path: "fonts/font-ui-v2/bitmap-page-000.r8".to_string(),
            }],
            glyphs: vec![CookedFontBundleGlyph {
                font_family_id: "family-ui".to_string(),
                font_face_id: "face-ui".to_string(),
                style: crate::font_bundle::FontBundleStyle::Normal,
                weight: 400,
                glyph_id: 1,
                codepoint: u32::from('中'),
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: 16,
                page_index: 0,
                pixel_rect: [0, 0, 5, 5],
                bearing_x: 0,
                bearing_y: 5,
                advance_per_em_millionths: 1_000_000,
            }],
            kerning_adjustments: Vec::new(),
            bundle_digest: String::new(),
        };
        metadata.bundle_digest = font_bundle_digest(&metadata);
        RuntimePackageSourceFontBundle {
            metadata,
            page_payloads: vec![payload],
        }
    }

    fn scene_with_texture_ref(id: &str, texture_asset_id: &str) -> RuntimeScene {
        RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            name: "Main".to_string(),
            gravity: 0.0,
            background: "#000".to_string(),
            sky_color: "#111".to_string(),
            entities: vec![RuntimeEntity {
                schema_version: RUNTIME_ENTITY_SCHEMA_VERSION.to_string(),
                id: "entity-root".to_string(),
                name: "Root".to_string(),
                kind: "entity".to_string(),
                enabled: true,
                parent_id: None,
                sibling_order: 0,
                transform: Some(RuntimeTransform {
                    local_position: Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    local_rotation: Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    local_scale: Vector3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                }),
                mesh: Some(RuntimeMesh {
                    primitive: Some("quad".to_string()),
                    color: None,
                    label: None,
                    asset_ref: None,
                    material_ref: None,
                    texture_ref: Some(RuntimeAssetRef {
                        id: texture_asset_id.to_string(),
                        asset_type: "texture".to_string(),
                        guid: None,
                        sub_asset: None,
                    }),
                    visible: true,
                    layer: "default".to_string(),
                    metalness: None,
                    roughness: None,
                }),
                sprite_renderer2d: None,
                animator2d: None,
                components: Vec::new(),
            }],
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("runtime-package-builder-{}-{}", label, stamp))
    }

    fn files_under(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files
    }
}
