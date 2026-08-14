use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};

pub const AI_GOAL_BINDING_SCHEMA_VERSION: &str = "ai-goal-binding.v1";
pub const AI_RISK_ENVELOPE_SCHEMA_VERSION: &str = "ai-risk-envelope.v1";
pub const AI_GOAL_GRANT_SPEC_SCHEMA_VERSION: &str = "ai-goal-grant-spec.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiGoalGrantError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for AiGoalGrantError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AiGoalGrantError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiGoalCompletionPolicy {
    CommitVerified,
    DeliverVerified,
    RestoreInitial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiGoalBinding {
    pub schema_version: String,
    pub goal_id: String,
    pub user_visible_outcome: String,
    pub project_identity: String,
    pub initial_project_digest: String,
    pub completion_policy: AiGoalCompletionPolicy,
    pub binding_digest: String,
}

impl AiGoalBinding {
    pub fn new(
        goal_id: impl Into<String>,
        user_visible_outcome: impl Into<String>,
        project_identity: impl Into<String>,
        initial_project_digest: impl Into<String>,
        completion_policy: AiGoalCompletionPolicy,
    ) -> Result<Self, AiGoalGrantError> {
        let mut binding = Self {
            schema_version: AI_GOAL_BINDING_SCHEMA_VERSION.to_string(),
            goal_id: goal_id.into(),
            user_visible_outcome: user_visible_outcome.into(),
            project_identity: project_identity.into(),
            initial_project_digest: initial_project_digest.into(),
            completion_policy,
            binding_digest: String::new(),
        };
        binding.validate_fields()?;
        binding.binding_digest = digest_with_empty_field(&binding, "bindingDigest")?;
        Ok(binding)
    }

    pub fn validate_integrity(&self) -> Result<(), AiGoalGrantError> {
        self.validate_fields()?;
        if digest_with_empty_field(self, "bindingDigest")? != self.binding_digest {
            return Err(error(
                "ai_goal.binding_digest_mismatch",
                "GoalBinding content does not match its digest.",
            ));
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<(), AiGoalGrantError> {
        if self.schema_version != AI_GOAL_BINDING_SCHEMA_VERSION {
            return Err(error(
                "ai_goal.binding_schema_unsupported",
                "GoalBinding schema is unsupported.",
            ));
        }
        require_non_empty(&self.goal_id, "goal id")?;
        require_non_empty(&self.user_visible_outcome, "user-visible outcome")?;
        require_non_empty(&self.project_identity, "project identity")?;
        require_digest(&self.initial_project_digest, "initial project digest")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiGoalRiskClass {
    ExactDomains,
    ProjectOwnedLowRisk,
    Elevated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRiskEnvelopeSpec {
    pub risk_class: AiGoalRiskClass,
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_objects: Vec<String>,
    pub max_mutation_count: u32,
    pub time_budget_ms: u64,
    pub external_cost_budget_microunits: u64,
    pub allow_delete: bool,
    pub allow_dependency_change: bool,
    pub allow_network: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRiskEnvelope {
    pub schema_version: String,
    pub risk_class: AiGoalRiskClass,
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_objects: Vec<String>,
    pub max_mutation_count: u32,
    pub time_budget_ms: u64,
    pub external_cost_budget_microunits: u64,
    pub allow_delete: bool,
    pub allow_dependency_change: bool,
    pub allow_network: bool,
    pub envelope_digest: String,
}

impl AiRiskEnvelope {
    pub fn new(spec: AiRiskEnvelopeSpec) -> Result<Self, AiGoalGrantError> {
        let mut envelope = Self {
            schema_version: AI_RISK_ENVELOPE_SCHEMA_VERSION.to_string(),
            risk_class: spec.risk_class,
            allowed_paths: spec.allowed_paths,
            denied_paths: spec.denied_paths,
            allowed_objects: spec.allowed_objects,
            max_mutation_count: spec.max_mutation_count,
            time_budget_ms: spec.time_budget_ms,
            external_cost_budget_microunits: spec.external_cost_budget_microunits,
            allow_delete: spec.allow_delete,
            allow_dependency_change: spec.allow_dependency_change,
            allow_network: spec.allow_network,
            envelope_digest: String::new(),
        };
        normalize_sorted(&mut envelope.allowed_paths);
        normalize_sorted(&mut envelope.denied_paths);
        normalize_sorted(&mut envelope.allowed_objects);
        envelope.validate_fields()?;
        envelope.envelope_digest = digest_with_empty_field(&envelope, "envelopeDigest")?;
        Ok(envelope)
    }

    pub fn project_owned_low_risk(
        allowed_paths: Vec<String>,
        denied_paths: Vec<String>,
        allowed_objects: Vec<String>,
        max_mutation_count: u32,
        time_budget_ms: u64,
        external_cost_budget_microunits: u64,
    ) -> Result<Self, AiGoalGrantError> {
        Self::new(AiRiskEnvelopeSpec {
            risk_class: AiGoalRiskClass::ProjectOwnedLowRisk,
            allowed_paths,
            denied_paths,
            allowed_objects,
            max_mutation_count,
            time_budget_ms,
            external_cost_budget_microunits,
            allow_delete: false,
            allow_dependency_change: false,
            allow_network: false,
        })
    }

    pub fn default_project_owned_low_risk() -> Result<Self, AiGoalGrantError> {
        Self::project_owned_low_risk(Vec::new(), Vec::new(), Vec::new(), 16, 900_000, 0)
    }

    pub fn validate_integrity(&self) -> Result<(), AiGoalGrantError> {
        self.validate_fields()?;
        if !is_normalized(&self.allowed_paths)
            || !is_normalized(&self.denied_paths)
            || !is_normalized(&self.allowed_objects)
        {
            return Err(error(
                "ai_goal.risk_scope_not_canonical",
                "RiskEnvelope scopes must be sorted, non-empty, and deduplicated.",
            ));
        }
        if digest_with_empty_field(self, "envelopeDigest")? != self.envelope_digest {
            return Err(error(
                "ai_goal.envelope_digest_mismatch",
                "RiskEnvelope content does not match its digest.",
            ));
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<(), AiGoalGrantError> {
        if self.schema_version != AI_RISK_ENVELOPE_SCHEMA_VERSION {
            return Err(error(
                "ai_goal.risk_schema_unsupported",
                "RiskEnvelope schema is unsupported.",
            ));
        }
        if self.max_mutation_count == 0 || self.time_budget_ms == 0 {
            return Err(error(
                "ai_goal.risk_budget_invalid",
                "RiskEnvelope requires positive mutation and time budgets.",
            ));
        }
        if self.risk_class == AiGoalRiskClass::ProjectOwnedLowRisk
            && (self.allow_delete || self.allow_dependency_change || self.allow_network)
        {
            return Err(error(
                "ai_goal.low_risk_escalated",
                "ProjectOwnedLowRisk cannot authorize delete, dependency, or network effects.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiGoalGrantSpec {
    pub schema_version: String,
    pub goal_binding: AiGoalBinding,
    pub risk_envelope: AiRiskEnvelope,
    pub client_session_id: String,
    pub issued_by: String,
    pub expires_at_epoch_ms: Option<u64>,
    pub approval_digest: String,
}

impl AiGoalGrantSpec {
    pub fn new(
        goal_binding: AiGoalBinding,
        risk_envelope: AiRiskEnvelope,
        client_session_id: impl Into<String>,
        issued_by: impl Into<String>,
        expires_at_epoch_ms: Option<u64>,
    ) -> Result<Self, AiGoalGrantError> {
        let mut spec = Self {
            schema_version: AI_GOAL_GRANT_SPEC_SCHEMA_VERSION.to_string(),
            goal_binding,
            risk_envelope,
            client_session_id: client_session_id.into(),
            issued_by: issued_by.into(),
            expires_at_epoch_ms,
            approval_digest: String::new(),
        };
        spec.validate_fields()?;
        spec.approval_digest = digest_with_empty_field(&spec, "approvalDigest")?;
        Ok(spec)
    }

    pub fn validate_integrity(&self) -> Result<(), AiGoalGrantError> {
        self.validate_fields()?;
        if digest_with_empty_field(self, "approvalDigest")? != self.approval_digest {
            return Err(error(
                "ai_goal.approval_digest_mismatch",
                "Goal grant approval content does not match its digest.",
            ));
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<(), AiGoalGrantError> {
        if self.schema_version != AI_GOAL_GRANT_SPEC_SCHEMA_VERSION {
            return Err(error(
                "ai_goal.grant_schema_unsupported",
                "Goal grant spec schema is unsupported.",
            ));
        }
        self.goal_binding.validate_integrity()?;
        self.risk_envelope.validate_integrity()?;
        require_non_empty(&self.client_session_id, "client session id")?;
        require_non_empty(&self.issued_by, "issuer")
    }
}

fn digest_with_empty_field<T: Serialize>(
    value: &T,
    field: &str,
) -> Result<String, AiGoalGrantError> {
    let mut value = serde_json::to_value(value).map_err(|source| {
        error(
            "ai_goal.digest_failed",
            format!("Failed to serialize goal grant content: {source}"),
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        error(
            "ai_goal.digest_failed",
            "Goal grant content must serialize as an object.",
        )
    })?;
    object.insert(field.to_string(), serde_json::Value::String(String::new()));
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|source| error("ai_goal.digest_failed", source.to_string()))
}

fn normalize_sorted(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
}

fn is_normalized(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.windows(2).all(|window| window[0] < window[1])
}

fn require_non_empty(value: &str, role: &str) -> Result<(), AiGoalGrantError> {
    if value.trim().is_empty() {
        return Err(error(
            "ai_goal.identity_missing",
            format!("Goal grant {role} is required."),
        ));
    }
    Ok(())
}

fn require_digest(value: &str, role: &str) -> Result<(), AiGoalGrantError> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(error(
            "ai_goal.digest_invalid",
            format!("Goal grant {role} must be a sha256 digest."),
        ));
    }
    Ok(())
}

fn error(code: impl Into<String>, message: impl Into<String>) -> AiGoalGrantError {
    AiGoalGrantError {
        code: code.into(),
        message: message.into(),
    }
}
