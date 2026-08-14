use crate::ai_capability_tool_kernel::{
    AiToolCapability, AiToolDescriptor, TOOL_ID_PROJECT_BUILD_EXPORT, TOOL_ID_PROJECT_CREATE,
    TOOL_ID_PROJECT_DELIVERY_VERIFY, TOOL_ID_PROJECT_MUTATE, TOOL_ID_PROJECT_PREVIEW,
    TOOL_ID_PROJECT_ROLLBACK, TOOL_ID_PROJECT_TRACE_UI_OWNER, TOOL_ID_RUNTIME_CAPTURE_ISSUE,
    TOOL_ID_UI_EXPLAIN_VISIBILITY, TOOL_ID_UI_LOCATE,
};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

pub const AI_TOOL_CATALOG_V1_SCHEMA_VERSION: &str = "ai-tool-catalog.v1";
pub const AI_TOOL_CATALOG_SCHEMA_VERSION: &str = "ai-tool-catalog.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolCatalogRequest {
    pub schema_version: String,
}

impl AiToolCatalogRequest {
    pub fn v1() -> Self {
        Self {
            schema_version: AI_TOOL_CATALOG_V1_SCHEMA_VERSION.to_string(),
        }
    }

    pub fn v2() -> Self {
        Self::default()
    }
}

impl Default for AiToolCatalogRequest {
    fn default() -> Self {
        Self {
            schema_version: AI_TOOL_CATALOG_SCHEMA_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolAvailabilityState {
    Ready,
    AuthorizationRequired,
    Blocked,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolAvailabilityReasonCategory {
    Authorization,
    ProjectState,
    RuntimeModule,
    Platform,
    Host,
    Implementation,
    OperationConflict,
    SessionFreshness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolAvailabilityOwner {
    GatewayAuthority,
    ProjectSession,
    RuntimeModule,
    Platform,
    EditorHost,
    ToolImplementation,
    OperationRegistry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolAvailabilityResolutionKind {
    None,
    RequestAuthorization,
    AwaitUserDecision,
    RefreshSessionFacts,
    OpenOrSwitchProject,
    ResolveProjectState,
    BindRuntimeModule,
    SelectSupportedPlatform,
    WaitOrCancelConflictingOperation,
    InstallOrEnableSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolAvailabilityReason {
    pub code: String,
    pub category: AiToolAvailabilityReasonCategory,
    pub message: String,
    pub resolution_kind: AiToolAvailabilityResolutionKind,
    pub owner: AiToolAvailabilityOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolAvailabilityBasis {
    pub project_identity: Option<String>,
    pub project_digest: Option<String>,
    pub read_generation: Option<u64>,
    pub runtime_binding_digest: Option<String>,
    pub access_generation: Option<u64>,
    pub operation_generation: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiToolReadAvailabilityState {
    #[default]
    Unavailable,
    Active,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiToolMutationAvailabilityState {
    #[default]
    NotRequested,
    AwaitingUser,
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiToolAvailabilityContext {
    pub basis: AiToolAvailabilityBasis,
    pub read_state: AiToolReadAvailabilityState,
    pub mutation_state: AiToolMutationAvailabilityState,
    pub runtime_ready: bool,
    pub delivery_supported: bool,
    pub operation_conflict: bool,
    pub rollback_lineage_known: bool,
}

impl Default for AiToolAvailabilityContext {
    fn default() -> Self {
        Self {
            basis: AiToolAvailabilityBasis::default(),
            read_state: AiToolReadAvailabilityState::Unavailable,
            mutation_state: AiToolMutationAvailabilityState::NotRequested,
            runtime_ready: false,
            delivery_supported: true,
            operation_conflict: false,
            rollback_lineage_known: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolAvailability {
    pub state: AiToolAvailabilityState,
    pub reasons: Vec<AiToolAvailabilityReason>,
    pub basis: AiToolAvailabilityBasis,
    pub input_dependent_checks_remain: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolCatalogEntry {
    pub descriptor: AiToolDescriptor,
    pub availability: AiToolAvailability,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiToolCatalog {
    pub schema_version: String,
    pub tools: Vec<AiToolDescriptor>,
    catalog_digest: Option<String>,
    availability_digest: Option<String>,
    basis: Option<AiToolAvailabilityBasis>,
    availability_by_tool: BTreeMap<String, AiToolAvailability>,
}

impl AiToolCatalog {
    pub fn v1(tools: Vec<AiToolDescriptor>) -> Self {
        Self {
            schema_version: AI_TOOL_CATALOG_V1_SCHEMA_VERSION.to_string(),
            tools,
            catalog_digest: None,
            availability_digest: None,
            basis: None,
            availability_by_tool: BTreeMap::new(),
        }
    }

    pub fn v2(tools: Vec<AiToolDescriptor>, context: AiToolAvailabilityContext) -> Self {
        let catalog_digest = catalog_digest(&tools);
        let availability_by_tool = tools
            .iter()
            .map(|descriptor| {
                (
                    descriptor.tool_id.clone(),
                    probe_tool_availability(descriptor, &context),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let availability_digest =
            availability_digest(&catalog_digest, &context.basis, &availability_by_tool);
        Self {
            schema_version: AI_TOOL_CATALOG_SCHEMA_VERSION.to_string(),
            tools,
            catalog_digest: Some(catalog_digest),
            availability_digest: Some(availability_digest),
            basis: Some(context.basis),
            availability_by_tool,
        }
    }

    pub fn catalog_digest(&self) -> String {
        self.catalog_digest
            .clone()
            .unwrap_or_else(|| catalog_digest(&self.tools))
    }

    pub fn availability(&self, tool_id: &str) -> Option<&AiToolAvailability> {
        self.availability_by_tool.get(tool_id)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogV1Wire {
    schema_version: String,
    tools: Vec<AiToolDescriptor>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogV2Wire {
    schema_version: String,
    catalog_digest: String,
    availability_digest: String,
    basis: AiToolAvailabilityBasis,
    tools: Vec<AiToolCatalogEntry>,
}

impl Serialize for AiToolCatalog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.schema_version == AI_TOOL_CATALOG_V1_SCHEMA_VERSION {
            return CatalogV1Wire {
                schema_version: self.schema_version.clone(),
                tools: self.tools.clone(),
            }
            .serialize(serializer);
        }
        CatalogV2Wire {
            schema_version: self.schema_version.clone(),
            catalog_digest: self.catalog_digest.clone().expect("v2 catalog digest"),
            availability_digest: self
                .availability_digest
                .clone()
                .expect("v2 availability digest"),
            basis: self.basis.clone().expect("v2 basis"),
            tools: self
                .tools
                .iter()
                .map(|descriptor| AiToolCatalogEntry {
                    descriptor: descriptor.clone(),
                    availability: self
                        .availability_by_tool
                        .get(&descriptor.tool_id)
                        .expect("registered tool availability")
                        .clone(),
                })
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AiToolCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str)
        {
            Some(AI_TOOL_CATALOG_V1_SCHEMA_VERSION) => {
                let wire: CatalogV1Wire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self::v1(wire.tools))
            }
            Some(AI_TOOL_CATALOG_SCHEMA_VERSION) => {
                let wire: CatalogV2Wire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                let descriptors = wire
                    .tools
                    .iter()
                    .map(|entry| entry.descriptor.clone())
                    .collect::<Vec<_>>();
                let availability_by_tool = wire
                    .tools
                    .iter()
                    .map(|entry| (entry.descriptor.tool_id.clone(), entry.availability.clone()))
                    .collect::<BTreeMap<_, _>>();
                if availability_by_tool.len() != wire.tools.len() {
                    return Err(serde::de::Error::custom(
                        "Tool Catalog contains duplicate tool ids",
                    ));
                }
                if wire
                    .tools
                    .iter()
                    .any(|entry| entry.availability.basis != wire.basis)
                {
                    return Err(serde::de::Error::custom(
                        "Tool availability basis differs from the Catalog basis",
                    ));
                }
                let expected_catalog_digest = catalog_digest(&descriptors);
                if wire.catalog_digest != expected_catalog_digest {
                    return Err(serde::de::Error::custom(
                        "Tool Catalog descriptor digest is malformed or mismatched",
                    ));
                }
                let expected_availability_digest =
                    availability_digest(&wire.catalog_digest, &wire.basis, &availability_by_tool);
                if wire.availability_digest != expected_availability_digest {
                    return Err(serde::de::Error::custom(
                        "Tool Catalog availability digest is malformed or mismatched",
                    ));
                }
                Ok(Self {
                    schema_version: wire.schema_version,
                    tools: descriptors,
                    catalog_digest: Some(wire.catalog_digest),
                    availability_digest: Some(wire.availability_digest),
                    basis: Some(wire.basis),
                    availability_by_tool,
                })
            }
            _ => Err(serde::de::Error::custom("unsupported Tool Catalog schema")),
        }
    }
}

fn probe_tool_availability(
    descriptor: &AiToolDescriptor,
    context: &AiToolAvailabilityContext,
) -> AiToolAvailability {
    let mut reasons = Vec::new();
    if context.basis.project_identity.is_none() && descriptor.tool_id != TOOL_ID_PROJECT_CREATE {
        reasons.push(reason(
            "ai_tool.availability.project_required",
            AiToolAvailabilityReasonCategory::ProjectState,
            "Tool requires an active project.",
            AiToolAvailabilityResolutionKind::OpenOrSwitchProject,
            AiToolAvailabilityOwner::ProjectSession,
        ));
    }
    if context.basis.project_identity.is_some() && descriptor.tool_id == TOOL_ID_PROJECT_CREATE {
        reasons.push(reason(
            "ai_tool.availability.launcher_required",
            AiToolAvailabilityReasonCategory::ProjectState,
            "project.create is available only in the Editor launcher.",
            AiToolAvailabilityResolutionKind::ResolveProjectState,
            AiToolAvailabilityOwner::ProjectSession,
        ));
    }
    if context.read_state == AiToolReadAvailabilityState::Stale {
        reasons.push(reason(
            "ai_tool.availability.read_stale",
            AiToolAvailabilityReasonCategory::SessionFreshness,
            "Session project facts are stale.",
            AiToolAvailabilityResolutionKind::RefreshSessionFacts,
            AiToolAvailabilityOwner::GatewayAuthority,
        ));
    }
    let requires_mutation = descriptor
        .required_capabilities
        .contains(&AiToolCapability::MutateProject)
        && descriptor.tool_id != TOOL_ID_PROJECT_ROLLBACK;
    if requires_mutation && context.mutation_state != AiToolMutationAvailabilityState::Active {
        let awaiting = context.mutation_state == AiToolMutationAvailabilityState::AwaitingUser;
        reasons.push(reason(
            if awaiting {
                "ai_tool.availability.await_user_decision"
            } else {
                "ai_tool.availability.authorization_required"
            },
            AiToolAvailabilityReasonCategory::Authorization,
            if awaiting {
                "Mutation authority is awaiting the existing user decision."
            } else {
                "Tool requires active mutation authority."
            },
            if awaiting {
                AiToolAvailabilityResolutionKind::AwaitUserDecision
            } else {
                AiToolAvailabilityResolutionKind::RequestAuthorization
            },
            AiToolAvailabilityOwner::GatewayAuthority,
        ));
    }
    if runtime_tool(&descriptor.tool_id)
        && context.basis.project_identity.is_some()
        && !context.runtime_ready
    {
        reasons.push(reason(
            "ai_tool.availability.runtime_binding_missing",
            AiToolAvailabilityReasonCategory::RuntimeModule,
            "The active project RuntimeModule is not linked to this host.",
            AiToolAvailabilityResolutionKind::BindRuntimeModule,
            AiToolAvailabilityOwner::RuntimeModule,
        ));
    }
    if delivery_tool(&descriptor.tool_id) && !context.delivery_supported {
        reasons.push(reason(
            "ai_tool.availability.delivery_unsupported",
            AiToolAvailabilityReasonCategory::Host,
            "The current Editor host does not implement this delivery target.",
            AiToolAvailabilityResolutionKind::InstallOrEnableSupport,
            AiToolAvailabilityOwner::EditorHost,
        ));
    }
    if context.operation_conflict {
        reasons.push(reason(
            "ai_tool.availability.operation_conflict",
            AiToolAvailabilityReasonCategory::OperationConflict,
            "The bounded Tool operation queue cannot accept another operation.",
            AiToolAvailabilityResolutionKind::WaitOrCancelConflictingOperation,
            AiToolAvailabilityOwner::OperationRegistry,
        ));
    }
    reasons.sort_by_key(|reason| (reason.category, reason.owner, reason.code.clone()));
    let state = reasons
        .iter()
        .map(reason_state)
        .max()
        .unwrap_or(AiToolAvailabilityState::Ready);
    AiToolAvailability {
        state,
        reasons,
        basis: context.basis.clone(),
        input_dependent_checks_remain: input_dependent_tool(&descriptor.tool_id)
            || (descriptor.tool_id == TOOL_ID_PROJECT_ROLLBACK && !context.rollback_lineage_known),
    }
}

fn reason_state(reason: &AiToolAvailabilityReason) -> AiToolAvailabilityState {
    match reason.category {
        AiToolAvailabilityReasonCategory::Implementation
        | AiToolAvailabilityReasonCategory::Platform
        | AiToolAvailabilityReasonCategory::Host => AiToolAvailabilityState::Unsupported,
        AiToolAvailabilityReasonCategory::Authorization => {
            AiToolAvailabilityState::AuthorizationRequired
        }
        _ => AiToolAvailabilityState::Blocked,
    }
}

fn reason(
    code: &str,
    category: AiToolAvailabilityReasonCategory,
    message: &str,
    resolution_kind: AiToolAvailabilityResolutionKind,
    owner: AiToolAvailabilityOwner,
) -> AiToolAvailabilityReason {
    AiToolAvailabilityReason {
        code: code.to_string(),
        category,
        message: message.to_string(),
        resolution_kind,
        owner,
    }
}

fn runtime_tool(tool_id: &str) -> bool {
    matches!(
        tool_id,
        TOOL_ID_PROJECT_PREVIEW
            | TOOL_ID_RUNTIME_CAPTURE_ISSUE
            | TOOL_ID_UI_LOCATE
            | TOOL_ID_UI_EXPLAIN_VISIBILITY
            | TOOL_ID_PROJECT_TRACE_UI_OWNER
    )
}

fn delivery_tool(tool_id: &str) -> bool {
    matches!(
        tool_id,
        TOOL_ID_PROJECT_BUILD_EXPORT | TOOL_ID_PROJECT_DELIVERY_VERIFY
    )
}

fn input_dependent_tool(tool_id: &str) -> bool {
    matches!(
        tool_id,
        TOOL_ID_PROJECT_MUTATE
            | TOOL_ID_PROJECT_ROLLBACK
            | TOOL_ID_PROJECT_BUILD_EXPORT
            | TOOL_ID_PROJECT_DELIVERY_VERIFY
    )
}

fn catalog_digest(tools: &[AiToolDescriptor]) -> String {
    digest(tools, "Tool Catalog descriptors")
}

fn availability_digest(
    catalog_digest: &str,
    basis: &AiToolAvailabilityBasis,
    availability_by_tool: &BTreeMap<String, AiToolAvailability>,
) -> String {
    digest(
        &(catalog_digest, basis, availability_by_tool),
        "Tool Catalog availability",
    )
}

fn digest<T: Serialize + ?Sized>(value: &T, label: &str) -> String {
    let value = serde_json::to_value(value)
        .unwrap_or_else(|error| panic!("{label} serialization failed: {error}"));
    let bytes = canonical_json_bytes(&value)
        .unwrap_or_else(|error| panic!("{label} canonical serialization failed: {error}"));
    sha256_prefixed(&bytes)
}
