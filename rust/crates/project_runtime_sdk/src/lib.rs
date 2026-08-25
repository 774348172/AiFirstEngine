//! Project-facing schema and safe helpers for the stable project runtime ABI.

use project_runtime_abi::{
    hex_digest, ProjectRuntimeAbiStatus, ProjectRuntimeByteBuffer, ProjectRuntimeByteSlice,
    ProjectRuntimeCallContext, ProjectRuntimeHostCall, ProjectRuntimeModuleCall,
    ProjectRuntimeOpaqueHandle, PROJECT_RUNTIME_ABI_SCHEMA,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES: usize = 256 * 1024;

pub const PROJECT_RUNTIME_SDK_SCHEMA: &str = concat!(
    "project-runtime-sdk.v1;json-utf8;",
    "descriptor;rules;session-create;session-call;rule-call;aui-actions;",
    "fixed-update;conditional-ui-state;observations;world-query;world-read;",
    "rule-input-actions;rule-collision-pairs;deferred-mutations;spawn-despawn"
);

pub struct ProjectRuntimeAotDigestSource<'a> {
    pub relative_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeAotDigestPayload<'a> {
    module_id: &'a str,
    interface_version: &'a str,
    cargo_manifest: &'a str,
    cargo_package: &'a str,
    player_binary: &'a str,
    sources: Vec<ProjectRuntimeAotDigestSourceHash<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeAotDigestSourceHash<'a> {
    relative_path: &'a str,
    content_hash: String,
}

pub fn project_runtime_aot_digest<'a>(
    module_id: &str,
    interface_version: &str,
    cargo_manifest: &str,
    cargo_package: &str,
    player_binary: &str,
    sources: impl IntoIterator<Item = ProjectRuntimeAotDigestSource<'a>>,
) -> Result<String, serde_json::Error> {
    let mut sources = sources
        .into_iter()
        .map(|source| ProjectRuntimeAotDigestSourceHash {
            relative_path: source.relative_path,
            content_hash: format!("sha256:{:x}", Sha256::digest(source.bytes)),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.relative_path.cmp(right.relative_path));
    let payload = serde_json::to_value(ProjectRuntimeAotDigestPayload {
        module_id,
        interface_version,
        cargo_manifest,
        cargo_package,
        player_binary,
        sources,
    })?;
    let payload = serde_json::to_vec(&payload)?;
    let mut hasher = Sha256::new();
    hasher.update(b"AIFE-CONSISTENCY\0");
    for field in [
        b"consistency-digest.v1".as_slice(),
        b"project-runtime-module-aot-input".as_slice(),
        b"project-runtime-module-aot-input.v1".as_slice(),
        payload.as_slice(),
    ] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeModuleDescriptor {
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
    pub ui_state_producer_id: String,
    pub rules: Vec<ProjectRuntimeRuleDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeRuleDescriptor {
    pub rule_id: String,
    pub artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeSessionCreateRequest {
    pub project_id: String,
    pub module_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeSessionCreateResponse {
    pub handle_value: u64,
    pub handle_generation: u64,
}

impl From<ProjectRuntimeSessionCreateResponse> for ProjectRuntimeOpaqueHandle {
    fn from(value: ProjectRuntimeSessionCreateResponse) -> Self {
        Self {
            value: value.handle_value,
            generation: value.handle_generation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeFrameRequest {
    pub frame_index: u64,
    pub time: ProjectRuntimeTime,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTime {
    pub time: f32,
    pub delta_time: f32,
    pub unscaled_time: f32,
    pub unscaled_delta_time: f32,
    pub fixed_time: f32,
    pub fixed_delta_time: f32,
    pub frame_count: u64,
    pub fixed_frame_count: u64,
    pub time_scale: f32,
    pub in_fixed_step: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeRuleRequest {
    pub rule_id: String,
    pub frame: ProjectRuntimeFrameRequest,
    #[serde(default)]
    pub input_actions: Vec<ProjectRuntimeInputAction>,
    #[serde(default)]
    pub collision_pairs: Vec<ProjectRuntimeCollisionPair>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeInputAction {
    pub action_id: String,
    pub phase: Option<String>,
    pub axis1: Option<f32>,
    pub axis2: Option<[f32; 2]>,
}

impl ProjectRuntimeInputAction {
    pub fn is_pressed(&self) -> bool {
        matches!(self.phase.as_deref(), Some("pressed" | "held"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeCollisionPair {
    pub entity_a: String,
    pub entity_b: String,
    pub is_sensor_pair: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeRuleOutput {
    pub status: ProjectRuntimeStatus,
    pub mutations: Vec<ProjectRuntimeDeferredMutation>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeAuiActionRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub actions: Vec<ProjectRuntimeAuiAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeAuiAction {
    pub action_id: String,
    pub node_id: String,
    pub event: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateIdentity {
    pub producer_epoch: u64,
    pub visible_revision: u64,
    pub binding_set_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeUiBindingSet {
    Known {
        digest: String,
    },
    Replace {
        digest: String,
        active_binding_paths: Vec<String>,
    },
}

impl ProjectRuntimeUiBindingSet {
    pub fn digest(&self) -> &str {
        match self {
            Self::Known { digest } | Self::Replace { digest, .. } => digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateResolveRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub previous_identity: Option<ProjectRuntimeUiStateIdentity>,
    pub binding_set: ProjectRuntimeUiBindingSet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeUiStateResolveOutput {
    Reuse {
        identity: ProjectRuntimeUiStateIdentity,
    },
    Replace {
        identity: ProjectRuntimeUiStateIdentity,
        producer_id: String,
        values: BTreeMap<String, ProjectRuntimeValue>,
    },
    Uncacheable {
        producer_id: String,
        values: BTreeMap<String, ProjectRuntimeValue>,
    },
}

/// Temporary source compatibility for in-process fixtures. Native production modules use the
/// conditional resolve types above.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub active_binding_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateOutput {
    pub producer_id: String,
    pub values: BTreeMap<String, ProjectRuntimeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeObservationOutput {
    pub values: BTreeMap<String, ProjectRuntimeValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeStatus {
    Applied,
    NoOp,
    Unhandled,
    Rejected,
    Faulted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeSessionOutput {
    pub status: ProjectRuntimeStatus,
    pub handled_action_count: u64,
    pub unhandled_action_count: u64,
    pub rejected_action_count: u64,
    pub mutations: Vec<ProjectRuntimeDeferredMutation>,
    pub diagnostics: Vec<String>,
}

impl ProjectRuntimeSessionOutput {
    pub fn no_op() -> Self {
        Self {
            status: ProjectRuntimeStatus::NoOp,
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectRuntimeDeferredMutation {
    WriteTransform {
        entity_id: String,
        transform: ProjectRuntimeTransform,
    },
    WriteComponentField {
        entity_id: String,
        component_type: String,
        field_path: String,
        value: ProjectRuntimeValue,
    },
    ReplaceDynamicComponent {
        entity_id: String,
        component_type: String,
        fields: BTreeMap<String, ProjectRuntimeValue>,
    },
    InstantiatePrefab {
        prefab_id: String,
    },
    DespawnEntity {
        entity_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ProjectRuntimeValue {
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
    EntityRef(String),
    AssetRef(String),
    Object(BTreeMap<String, ProjectRuntimeValue>),
    Array(Vec<ProjectRuntimeValue>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldQueryRequest {
    pub all: Vec<String>,
    pub none: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldQueryResponse {
    pub entity_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldReadRequest {
    pub entity_id: String,
    pub component_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldReadResponse {
    pub value: ProjectRuntimeValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeSdkError {
    pub status: ProjectRuntimeAbiStatus,
    pub message: String,
}

pub fn project_runtime_contract_digest() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_RUNTIME_ABI_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(PROJECT_RUNTIME_SDK_SCHEMA.as_bytes());
    hasher.finalize().into()
}

pub fn project_runtime_contract_digest_hex() -> String {
    hex_digest(project_runtime_contract_digest())
}

pub fn ffi_boundary<T: Serialize>(
    output: *mut ProjectRuntimeByteBuffer,
    callback: impl FnOnce() -> Result<T, ProjectRuntimeAbiStatus>,
) -> ProjectRuntimeAbiStatus {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(Ok(value)) => match serde_json::to_vec(&value) {
            Ok(bytes) => write_output(output, &bytes),
            Err(_) => ProjectRuntimeAbiStatus::FAILED,
        },
        Ok(Err(status)) => status,
        Err(_) => ProjectRuntimeAbiStatus::PANICKED,
    }
}

/// Rejects an undersized caller buffer before invoking a stateful module callback.
///
/// The regular caller-owned-buffer retry protocol may invoke a callback more than once. Project
/// callbacks that mutate session state must use a capacity floor large enough for their bounded
/// output so the retry only performs sizing and the callback itself executes exactly once.
pub fn ffi_boundary_with_capacity_floor<T: Serialize>(
    output: *mut ProjectRuntimeByteBuffer,
    capacity_floor: usize,
    callback: impl FnOnce() -> Result<T, ProjectRuntimeAbiStatus>,
) -> ProjectRuntimeAbiStatus {
    if output.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    let Ok(required) = u64::try_from(capacity_floor) else {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    };
    // SAFETY: null was rejected and the ABI caller owns a live buffer descriptor for this call.
    let output_ref = unsafe { &mut *output };
    if output_ref.capacity < required {
        output_ref.written = required;
        return ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL;
    }
    if required > 0 && output_ref.data.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    ffi_boundary(output, callback)
}

/// # Safety
/// `input.data` must point to `input.length` readable bytes for this call, or be null when length is
/// zero. The bytes are copied before this function returns.
pub unsafe fn read_input<T: DeserializeOwned>(
    input: ProjectRuntimeByteSlice,
) -> Result<T, ProjectRuntimeAbiStatus> {
    let length =
        usize::try_from(input.length).map_err(|_| ProjectRuntimeAbiStatus::INVALID_ARGUMENT)?;
    if length > 0 && input.data.is_null() {
        return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: upheld by the caller contract above.
        unsafe { std::slice::from_raw_parts(input.data, length) }
    };
    serde_json::from_slice(bytes).map_err(|_| ProjectRuntimeAbiStatus::INVALID_ARGUMENT)
}

pub fn call_json<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeModuleCall,
    module_context: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    call_context: Option<&ProjectRuntimeCallContext>,
    request: &Request,
) -> Result<Response, ProjectRuntimeSdkError> {
    let request = serde_json::to_vec(request).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
        message: error.to_string(),
    })?;
    let mut bytes = vec![0_u8; 4096];
    for _ in 0..2 {
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: all pointers borrow live values for only this call; the callee must follow ABI.
        let status = unsafe {
            call(
                module_context,
                session,
                call_context.map_or(std::ptr::null(), std::ptr::from_ref),
                ProjectRuntimeByteSlice::from_slice(&request),
                &mut output,
            )
        };
        if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            let required = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "module returned an invalid output length".to_string(),
            })?;
            bytes.resize(required, 0);
            continue;
        }
        if !status.is_ok() {
            return Err(ProjectRuntimeSdkError {
                status,
                message: format!("project runtime ABI call failed with status {}", status.0),
            });
        }
        let written = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: "module returned an invalid output length".to_string(),
        })?;
        if written > bytes.len() {
            return Err(ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "module wrote beyond the caller-owned output buffer".to_string(),
            });
        }
        return serde_json::from_slice(&bytes[..written]).map_err(|error| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: error.to_string(),
        });
    }
    Err(ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: "module output size changed after retry".to_string(),
    })
}

pub fn call_json_once_with_buffer<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeModuleCall,
    module_context: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    call_context: Option<&ProjectRuntimeCallContext>,
    request: &Request,
    bytes: &mut [u8],
) -> Result<Response, ProjectRuntimeSdkError> {
    let request = serde_json::to_vec(request).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
        message: error.to_string(),
    })?;
    let mut output = ProjectRuntimeByteBuffer::from_slice(bytes);
    // SAFETY: all pointers borrow live values for only this call; the callee must follow ABI.
    let status = unsafe {
        call(
            module_context,
            session,
            call_context.map_or(std::ptr::null(), std::ptr::from_ref),
            ProjectRuntimeByteSlice::from_slice(&request),
            &mut output,
        )
    };
    if !status.is_ok() {
        let detail = if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            format!(
                "stateful project runtime output requires {} bytes but the one-shot buffer has {} bytes",
                output.written, output.capacity
            )
        } else {
            format!("project runtime ABI call failed with status {}", status.0)
        };
        return Err(ProjectRuntimeSdkError {
            status,
            message: detail,
        });
    }
    let written = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: "module returned an invalid output length".to_string(),
    })?;
    if written > bytes.len() {
        return Err(ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: "module wrote beyond the caller-owned output buffer".to_string(),
        });
    }
    serde_json::from_slice(&bytes[..written]).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: error.to_string(),
    })
}

pub fn call_host_json<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeHostCall,
    host_context: ProjectRuntimeOpaqueHandle,
    request: &Request,
) -> Result<Response, ProjectRuntimeSdkError> {
    let request = serde_json::to_vec(request).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
        message: error.to_string(),
    })?;
    let mut bytes = vec![0_u8; 4096];
    for _ in 0..2 {
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: all pointers borrow caller-owned storage for only this callback.
        let status = unsafe {
            call(
                host_context,
                ProjectRuntimeByteSlice::from_slice(&request),
                &mut output,
            )
        };
        if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            let required = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "host returned an invalid output length".to_string(),
            })?;
            bytes.resize(required, 0);
            continue;
        }
        if !status.is_ok() {
            return Err(ProjectRuntimeSdkError {
                status,
                message: format!("project runtime host call failed with status {}", status.0),
            });
        }
        let written = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: "host returned an invalid output length".to_string(),
        })?;
        if written > bytes.len() {
            return Err(ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "host wrote beyond the caller-owned output buffer".to_string(),
            });
        }
        return serde_json::from_slice(&bytes[..written]).map_err(|error| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: error.to_string(),
        });
    }
    Err(ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: "host output size changed after retry".to_string(),
    })
}

fn write_output(output: *mut ProjectRuntimeByteBuffer, bytes: &[u8]) -> ProjectRuntimeAbiStatus {
    if output.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    // SAFETY: null was rejected and the ABI caller owns a live buffer descriptor for this call.
    let output = unsafe { &mut *output };
    output.written = bytes.len() as u64;
    if output.capacity < bytes.len() as u64 {
        return ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL;
    }
    if !bytes.is_empty() && output.data.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    if !bytes.is_empty() {
        // SAFETY: capacity was checked and the caller promises writable memory for the call.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output.data, bytes.len()) };
    }
    ProjectRuntimeAbiStatus::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn echo_call(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        ffi_boundary(output, || {
            // SAFETY: the test caller provides the serialized request for this call.
            let value: ProjectRuntimeFrameRequest = unsafe { read_input(request) }?;
            Ok(value)
        })
    }

    #[test]
    fn sdk_digest_is_stable_and_includes_wire_schema() {
        assert_eq!(
            project_runtime_contract_digest(),
            project_runtime_contract_digest()
        );
        assert_eq!(project_runtime_contract_digest_hex().len(), 64);
        assert_ne!(
            project_runtime_contract_digest(),
            project_runtime_abi::project_runtime_abi_digest()
        );
    }

    #[test]
    fn caller_owned_buffer_retries_without_transferring_memory() {
        let request = ProjectRuntimeFrameRequest {
            frame_index: 9,
            time: ProjectRuntimeTime {
                delta_time: 0.25,
                ..ProjectRuntimeTime::default()
            },
        };
        let response: ProjectRuntimeFrameRequest = call_json(
            echo_call,
            ProjectRuntimeOpaqueHandle::NULL,
            ProjectRuntimeOpaqueHandle::NULL,
            None,
            &request,
        )
        .expect("echo call");
        assert_eq!(response, request);
    }

    #[test]
    fn facade_contains_panics_before_the_extern_boundary() {
        let mut bytes = [0_u8; 16];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        let status = ffi_boundary::<ProjectRuntimeFrameRequest>(&mut output, || {
            panic!("project callback panic")
        });
        assert_eq!(status, ProjectRuntimeAbiStatus::PANICKED);
    }

    #[test]
    fn invalid_input_and_small_output_fail_closed() {
        // SAFETY: an empty slice is the only valid null input.
        let invalid = unsafe {
            read_input::<ProjectRuntimeFrameRequest>(ProjectRuntimeByteSlice {
                data: std::ptr::null(),
                length: 4,
            })
        };
        assert_eq!(
            invalid.unwrap_err(),
            ProjectRuntimeAbiStatus::INVALID_ARGUMENT
        );

        let mut bytes = [0_u8; 1];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        let status = ffi_boundary(&mut output, || Ok(ProjectRuntimeSessionOutput::no_op()));
        assert_eq!(status, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert!(output.written > output.capacity);
    }

    #[test]
    fn stateful_capacity_floor_sizes_without_invoking_the_callback() {
        let mut small_bytes = [0_u8; 8];
        let mut small_output = ProjectRuntimeByteBuffer::from_slice(&mut small_bytes);
        let callback_count = std::cell::Cell::new(0);
        let status = ffi_boundary_with_capacity_floor(&mut small_output, 4096, || {
            callback_count.set(callback_count.get() + 1);
            Ok(ProjectRuntimeSessionOutput::no_op())
        });
        assert_eq!(status, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert_eq!(small_output.written, 4096);
        assert_eq!(callback_count.get(), 0);

        let mut adequate_bytes = vec![0_u8; 4096];
        let mut adequate_output = ProjectRuntimeByteBuffer::from_slice(&mut adequate_bytes);
        let status = ffi_boundary_with_capacity_floor(&mut adequate_output, 4096, || {
            callback_count.set(callback_count.get() + 1);
            Ok(ProjectRuntimeSessionOutput::no_op())
        });
        assert_eq!(status, ProjectRuntimeAbiStatus::OK);
        assert_eq!(callback_count.get(), 1);
    }

    #[test]
    fn crate_has_no_engine_or_editor_dependency() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in [
            "engine_runtime",
            "engine_input",
            "editor_core",
            "editor_window",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "forbidden dependency: {forbidden}"
            );
        }
    }
}
