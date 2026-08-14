use serde::{Deserialize, Serialize};

use super::{
    AssetPatchOperation, AuiPatchOperation, BuildPatchOperation, LlmCredentialLease,
    PatchCapability, PatchOperation, PatchRiskLevel, PatchSource, PrefabPatchOperation,
    ProjectPatchDocument, RulePatchOperation, ScenePatchOperation, PROJECT_PATCH_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmPatchSourceKind {
    Mock,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmStructuredOutputMode {
    StrictJsonSchema,
    JsonObject,
}

pub type RedactedSecret = super::LlmCredentialLease;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmPatchSourceConfig {
    pub source_kind: LlmPatchSourceKind,
    pub provider_id: String,
    pub model: String,
    pub timeout_ms: u64,
    pub base_url: String,
    pub structured_output_mode: LlmStructuredOutputMode,
    pub maximum_request_bytes: usize,
    pub maximum_response_bytes: usize,
    pub maximum_candidate_bytes: usize,
    pub maximum_transport_retries: u8,
    pub maximum_retry_after_ms: u64,
    pub enabled: bool,
    #[serde(skip)]
    pub api_key: RedactedSecret,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmTransportConfig {
    pub source_kind: LlmPatchSourceKind,
    pub provider_id: String,
    pub model: String,
    pub timeout_ms: u64,
    pub base_url: String,
    pub structured_output_mode: LlmStructuredOutputMode,
    pub maximum_request_bytes: usize,
    pub maximum_response_bytes: usize,
    pub maximum_candidate_bytes: usize,
    pub maximum_transport_retries: u8,
    pub maximum_retry_after_ms: u64,
    pub enabled: bool,
}

impl LlmPatchSourceConfig {
    pub fn transport_config(&self) -> LlmTransportConfig {
        LlmTransportConfig {
            source_kind: self.source_kind,
            provider_id: self.provider_id.clone(),
            model: self.model.clone(),
            timeout_ms: self.timeout_ms,
            base_url: self.base_url.clone(),
            structured_output_mode: self.structured_output_mode,
            maximum_request_bytes: self.maximum_request_bytes,
            maximum_response_bytes: self.maximum_response_bytes,
            maximum_candidate_bytes: self.maximum_candidate_bytes,
            maximum_transport_retries: self.maximum_transport_retries,
            maximum_retry_after_ms: self.maximum_retry_after_ms,
            enabled: self.enabled,
        }
    }

    pub fn into_transport_parts(self) -> (LlmTransportConfig, LlmCredentialLease) {
        let transport = self.transport_config();
        (transport, self.api_key)
    }

    pub fn deterministic_mock() -> Self {
        Self {
            source_kind: LlmPatchSourceKind::Mock,
            provider_id: "mock-llm-patch-source".to_string(),
            model: "deterministic-project-patch-v1".to_string(),
            timeout_ms: 1_000,
            base_url: "http://127.0.0.1".to_string(),
            structured_output_mode: LlmStructuredOutputMode::StrictJsonSchema,
            maximum_request_bytes: 2 * 1024 * 1024,
            maximum_response_bytes: 2 * 1024 * 1024,
            maximum_candidate_bytes: 2 * 1024 * 1024,
            maximum_transport_retries: 1,
            maximum_retry_after_ms: 1_000,
            enabled: true,
            api_key: RedactedSecret::default(),
        }
    }

    pub fn openai_compatible_from_env() -> Self {
        Self {
            source_kind: LlmPatchSourceKind::OpenAiCompatible,
            provider_id: "openai-compatible".to_string(),
            model: std::env::var("AI_ENGINE_LLM_MODEL")
                .unwrap_or_else(|_| "not-configured".to_string()),
            timeout_ms: std::env::var("AI_ENGINE_LLM_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(30_000),
            base_url: std::env::var("AI_ENGINE_LLM_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            structured_output_mode: match std::env::var("AI_ENGINE_LLM_STRUCTURED_OUTPUT_MODE") {
                Ok(value) if value.eq_ignore_ascii_case("json_object") => {
                    LlmStructuredOutputMode::JsonObject
                }
                _ => LlmStructuredOutputMode::StrictJsonSchema,
            },
            maximum_request_bytes: 2 * 1024 * 1024,
            maximum_response_bytes: 2 * 1024 * 1024,
            maximum_candidate_bytes: 2 * 1024 * 1024,
            maximum_transport_retries: 1,
            maximum_retry_after_ms: 1_000,
            enabled: std::env::var("AI_ENGINE_LLM_PATCH_SOURCE")
                .is_ok_and(|value| value.eq_ignore_ascii_case("openai_compatible")),
            api_key: std::env::var("AI_ENGINE_LLM_API_KEY")
                .map(RedactedSecret::new)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmPatchSourceStatus {
    Success,
    Refused,
    Cancelled,
    TimedOut,
    TransportError,
    HttpClientError,
    HttpServerError,
    RateLimited,
    AuthFailed,
    StructuredOutputUnsupported,
    ResponseTooLarge,
    EmptyOutput,
    InvalidProviderResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmPatchSourceResult {
    pub provider_id: String,
    pub model: String,
    pub status: LlmPatchSourceStatus,
    pub structured_output_mode: LlmStructuredOutputMode,
    pub degraded: bool,
    pub raw_json: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub next_action: Option<String>,
    pub latency_ms: u64,
    pub http_status_class: Option<String>,
    pub transport_attempt_count: u8,
}

impl LlmPatchSourceResult {
    fn raw(config: &LlmTransportConfig, raw_json: String) -> Self {
        Self {
            provider_id: config.provider_id.clone(),
            model: config.model.clone(),
            status: LlmPatchSourceStatus::Success,
            structured_output_mode: config.structured_output_mode,
            degraded: config.structured_output_mode == LlmStructuredOutputMode::JsonObject,
            raw_json: Some(raw_json),
            error_code: None,
            error_message: None,
            next_action: None,
            latency_ms: 0,
            http_status_class: None,
            transport_attempt_count: 1,
        }
    }

    pub(crate) fn error(
        config: &LlmTransportConfig,
        status: LlmPatchSourceStatus,
        code: impl Into<String>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: config.provider_id.clone(),
            model: config.model.clone(),
            status,
            structured_output_mode: config.structured_output_mode,
            degraded: config.structured_output_mode == LlmStructuredOutputMode::JsonObject,
            raw_json: None,
            error_code: Some(code.into()),
            error_message: Some(message.into()),
            next_action: Some(next_action.into()),
            latency_ms: 0,
            http_status_class: None,
            transport_attempt_count: 0,
        }
    }

    pub(crate) fn success(config: &LlmTransportConfig, raw_json: String) -> Self {
        Self::raw(config, raw_json)
    }
}

pub struct ThinLlmPatchSource;

impl ThinLlmPatchSource {
    pub fn generate_project_patch_json(
        config: &LlmPatchSourceConfig,
        user_prompt: &str,
        context_summary: &str,
    ) -> LlmPatchSourceResult {
        let transport_config = config.transport_config();
        if !transport_config.enabled {
            return LlmPatchSourceResult::error(
                &transport_config,
                LlmPatchSourceStatus::HttpClientError,
                "llm_patch_source.not_enabled",
                "LLM patch source is disabled.",
                "Enable the editor-only LLM patch source before submitting.",
            );
        }

        match transport_config.source_kind {
            LlmPatchSourceKind::Mock => {
                generate_mock_project_patch_json(&transport_config, user_prompt)
            }
            LlmPatchSourceKind::OpenAiCompatible => crate::project_patch::llm_http::generate(
                &transport_config,
                &config.api_key,
                user_prompt,
                context_summary,
            ),
        }
    }
}

pub fn build_project_patch_generation_prompt(user_prompt: &str, context_summary: &str) -> String {
    format!(
        "Generate only a ProjectPatchDocument JSON object.\n\
         schema_version must be {PROJECT_PATCH_SCHEMA_VERSION}.\n\
         Supported capabilities in this stage: Scene, Input, Asset, Prefab, AUI, Rule, Build.\n\
         Only use documented ProjectPatchOperation schemas.\n\
         Do not write files directly.\n\
         Use Build.ExportDesktopPackage only as final verification.\n\
         Do not invent gameplay-specific operation domains such as Player, Enemy, or Bullet.\n\
         Current editor context:\n{context_summary}\n\
         User request:\n{user_prompt}"
    )
}

pub(crate) fn generate_mock_project_patch_json(
    config: &LlmTransportConfig,
    user_prompt: &str,
) -> LlmPatchSourceResult {
    let normalized = user_prompt.to_ascii_lowercase();
    if normalized.starts_with("repair_project_patch_once") && normalized.contains("invalid_json") {
        let raw_json =
            serde_json::to_string(&create_mock_scene_patch("create \"Repaired LLM Entity\""))
                .expect("deterministic repaired mock ProjectPatchDocument should serialize");
        return LlmPatchSourceResult::raw(config, raw_json);
    }
    if normalized.contains("provider_error") {
        return LlmPatchSourceResult::error(
            config,
            LlmPatchSourceStatus::TransportError,
            "llm_patch_source.provider_error",
            "Deterministic mock provider error requested by prompt.",
            "Retry with a supported deterministic mock prompt.",
        );
    }
    if normalized.contains("invalid_json") {
        return LlmPatchSourceResult::raw(config, "{not-project-patch-json".to_string());
    }

    let patch = if normalized.contains("all_domain") || normalized.contains("all-domain") {
        create_mock_all_domain_patch()
    } else if normalized.contains("asset") {
        create_mock_asset_patch()
    } else if normalized.contains("prefab") {
        create_mock_prefab_patch()
    } else if normalized.contains("aui") {
        create_mock_aui_patch()
    } else if normalized.contains("rule") {
        create_mock_rule_patch()
    } else if normalized.contains("build") {
        create_mock_build_patch()
    } else {
        create_mock_scene_patch(user_prompt)
    };

    let raw_json = serde_json::to_string(&patch)
        .expect("deterministic mock ProjectPatchDocument should serialize");
    LlmPatchSourceResult::raw(config, raw_json)
}

fn create_mock_asset_patch() -> ProjectPatchDocument {
    let mut patch = ProjectPatchDocument::new(
        "llm-mock-asset",
        "LLM mock asset patch",
        PatchSource::AiAssistant,
        vec![PatchOperation::Asset(
            AssetPatchOperation::GenerateMockImageAsset {
                operation_id: "llm-mock-op-generate-asset".to_string(),
                depends_on: Vec::new(),
                prompt: "mock sprite".to_string(),
                target_folder: "Assets/Generated".to_string(),
                asset_name: "llm-mock-sprite".to_string(),
                image_kind: "sprite".to_string(),
                width: 16,
                height: 16,
                transparent_background: true,
            },
        )],
    );
    patch.intent_summary =
        "Deterministic mock LLM source generated an Asset ProjectPatch.".to_string();
    patch.expected_outcome = "A generated mock image asset is staged.".to_string();
    patch
}

fn create_mock_prefab_patch() -> ProjectPatchDocument {
    let mut patch = ProjectPatchDocument::new(
        "llm-mock-prefab",
        "LLM mock prefab patch",
        PatchSource::AiAssistant,
        vec![PatchOperation::Prefab(
            PrefabPatchOperation::ValidateReferences {
                operation_id: "llm-mock-op-validate-prefabs".to_string(),
                depends_on: Vec::new(),
                path: None,
            },
        )],
    );
    patch.intent_summary =
        "Deterministic mock LLM source generated a Prefab ProjectPatch.".to_string();
    patch.expected_outcome = "Prefab references are validated.".to_string();
    patch
}

fn create_mock_aui_patch() -> ProjectPatchDocument {
    let mut patch = ProjectPatchDocument::new(
        "llm-mock-aui",
        "LLM mock AUI patch",
        PatchSource::AiAssistant,
        vec![PatchOperation::Aui(AuiPatchOperation::CreateDocument {
            operation_id: "llm-mock-op-create-aui".to_string(),
            depends_on: Vec::new(),
            path: "UI/llm-hud.aui.json".to_string(),
            document_id: "llm-hud".to_string(),
            width: 1280.0,
            height: 720.0,
        })],
    );
    patch.intent_summary =
        "Deterministic mock LLM source generated an AUI ProjectPatch.".to_string();
    patch.expected_outcome = "A mock AUI HUD document is staged.".to_string();
    patch
}

fn create_mock_rule_patch() -> ProjectPatchDocument {
    let mut patch = ProjectPatchDocument::new(
        "llm-mock-rule",
        "LLM mock rule patch",
        PatchSource::AiAssistant,
        vec![PatchOperation::Rule(RulePatchOperation::CreateAsset {
            operation_id: "llm-mock-op-create-rule".to_string(),
            depends_on: Vec::new(),
            path: "Rules/llm-fire.rule.json".to_string(),
            rule_id: "project.rule.llm_fire".to_string(),
            display_name: "LLM Fire".to_string(),
            phase: None,
        })],
    );
    patch.intent_summary =
        "Deterministic mock LLM source generated a Rule ProjectPatch.".to_string();
    patch.expected_outcome = "A mock rule asset is staged.".to_string();
    patch
}

fn create_mock_build_patch() -> ProjectPatchDocument {
    let mut patch = ProjectPatchDocument::new(
        "llm-mock-build",
        "LLM mock build patch",
        PatchSource::AiAssistant,
        vec![PatchOperation::Build(
            BuildPatchOperation::ExportDesktopPackage {
                operation_id: "llm-mock-op-build".to_string(),
                depends_on: Vec::new(),
                profile_id: Some("windows-dev".to_string()),
            },
        )],
    );
    patch.intent_summary =
        "Deterministic mock LLM source generated a Build ProjectPatch.".to_string();
    patch.expected_outcome = "A desktop export verification is staged.".to_string();
    patch
}

fn create_mock_all_domain_patch() -> ProjectPatchDocument {
    let mut patch = ProjectPatchDocument::new(
        "llm-mock-all-domain",
        "LLM mock all-domain patch",
        PatchSource::AiAssistant,
        vec![
            PatchOperation::Asset(AssetPatchOperation::ValidateAssetBrowserIndex {
                operation_id: "llm-mock-op-asset".to_string(),
                depends_on: Vec::new(),
                query_kind: Some(editor_ui_model::AssetKind::Sprite),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::ValidateReferences {
                operation_id: "llm-mock-op-prefab".to_string(),
                depends_on: Vec::new(),
                path: None,
            }),
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "llm-mock-op-aui".to_string(),
                depends_on: Vec::new(),
                path: "UI/llm-all-domain-hud.aui.json".to_string(),
                document_id: "llm-all-domain-hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
            PatchOperation::Rule(RulePatchOperation::CreateAsset {
                operation_id: "llm-mock-op-rule".to_string(),
                depends_on: Vec::new(),
                path: "Rules/llm-all-domain.rule.json".to_string(),
                rule_id: "project.rule.llm_all_domain".to_string(),
                display_name: "LLM All Domain".to_string(),
                phase: None,
            }),
            PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                operation_id: "llm-mock-op-build".to_string(),
                depends_on: vec!["llm-mock-op-rule".to_string()],
                profile_id: Some("windows-dev".to_string()),
            }),
        ],
    );
    patch.intent_summary =
        "Deterministic mock LLM source generated an all-domain ProjectPatch.".to_string();
    patch.expected_outcome =
        "Asset, Prefab, AUI, Rule, and Build operations are staged.".to_string();
    patch
}

fn create_mock_scene_patch(user_prompt: &str) -> ProjectPatchDocument {
    ProjectPatchDocument {
        schema_version: PROJECT_PATCH_SCHEMA_VERSION.to_string(),
        patch_id: "llm-mock-create-entity".to_string(),
        title: "LLM mock create entity".to_string(),
        source: PatchSource::AiAssistant,
        intent_summary: "Deterministic mock LLM source generated a Scene ProjectPatch.".to_string(),
        target_project_root: None,
        required_capabilities: vec![PatchCapability::Scene],
        operations: vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "llm-mock-op-create-entity".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: extract_entity_name(user_prompt),
        })],
        expected_outcome: "A reviewable empty scene entity proposal is staged.".to_string(),
        risk_level: PatchRiskLevel::Low,
        created_at: "0".to_string(),
    }
}

fn extract_entity_name(user_prompt: &str) -> String {
    let mut quote_start = None;
    for (index, ch) in user_prompt.char_indices() {
        if matches!(ch, '"' | '\'') {
            if let Some(start) = quote_start {
                let value = user_prompt[start..index].trim();
                if !value.is_empty() {
                    return value.to_string();
                }
                quote_start = None;
            } else {
                quote_start = Some(index + ch.len_utf8());
            }
        }
    }
    "LLM Mock Entity".to_string()
}
