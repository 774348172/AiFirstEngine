use project_runtime_abi::{
    ProjectRuntimeAbiStatus, ProjectRuntimeApi, ProjectRuntimeByteBuffer, ProjectRuntimeByteSlice,
    ProjectRuntimeCallContext, ProjectRuntimeModuleCall, ProjectRuntimeOpaqueHandle,
    PROJECT_RUNTIME_ABI_MAJOR, PROJECT_RUNTIME_ABI_MINOR, PROJECT_RUNTIME_API_STRUCT_SIZE,
    PROJECT_RUNTIME_CAP_AUI_ACTIONS, PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS,
    PROJECT_RUNTIME_CAP_FIXED_UPDATE, PROJECT_RUNTIME_CAP_OBSERVATIONS, PROJECT_RUNTIME_CAP_RULES,
    PROJECT_RUNTIME_CAP_SESSIONS, PROJECT_RUNTIME_CAP_UI_STATE, PROJECT_RUNTIME_CAP_WORLD_READ,
};
use project_runtime_sdk::{
    ffi_boundary, project_runtime_contract_digest, read_input, ProjectRuntimeAuiActionRequest,
    ProjectRuntimeFrameRequest, ProjectRuntimeModuleDescriptor, ProjectRuntimeObservationOutput,
    ProjectRuntimeRuleDescriptor, ProjectRuntimeRuleOutput, ProjectRuntimeRuleRequest,
    ProjectRuntimeSessionCreateRequest, ProjectRuntimeSessionCreateResponse,
    ProjectRuntimeSessionOutput, ProjectRuntimeStatus, ProjectRuntimeUiStateResolveOutput,
    ProjectRuntimeUiStateResolveRequest, ProjectRuntimeValue,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

pub const MODULE_ID: &str = "fixture.native.runtime";
pub const INTERFACE_VERSION: &str = "project-runtime-module.v1";
pub const AOT_CONTENT_DIGEST: &str =
    "sha256:3333333333333333333333333333333333333333333333333333333333333333";

static DESTROY_COUNT: AtomicU64 = AtomicU64::new(0);
static ACTION_COUNT: AtomicU64 = AtomicU64::new(0);
static FIXED_COUNT: AtomicU64 = AtomicU64::new(0);

fn write<T: serde::Serialize>(
    output: *mut ProjectRuntimeByteBuffer,
    value: T,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || Ok(value))
}

unsafe extern "C" fn descriptor(
    _module: ProjectRuntimeOpaqueHandle,
    _session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    _request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    write(
        output,
        ProjectRuntimeModuleDescriptor {
            module_id: MODULE_ID.to_string(),
            interface_version: INTERFACE_VERSION.to_string(),
            aot_content_digest: AOT_CONTENT_DIGEST.to_string(),
            ui_state_producer_id: "fixture.native.ui".to_string(),
            rules: vec![ProjectRuntimeRuleDescriptor {
                rule_id: "fixture.native.rule".to_string(),
                artifact_id: "fixture.native.rule.v1".to_string(),
            }],
        },
    )
}

unsafe extern "C" fn create_session(
    _module: ProjectRuntimeOpaqueHandle,
    _session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    // SAFETY: the host owns the request bytes for this call.
    let Ok(request) = (unsafe { read_input::<ProjectRuntimeSessionCreateRequest>(request) }) else {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    };
    if request.module_id != MODULE_ID {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    write(
        output,
        ProjectRuntimeSessionCreateResponse {
            handle_value: 7,
            handle_generation: 1,
        },
    )
}

fn valid_session(session: ProjectRuntimeOpaqueHandle) -> bool {
    session.value == 7 && session.generation == 1
}

unsafe extern "C" fn destroy_session(
    _module: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    _request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    if !valid_session(session) {
        return ProjectRuntimeAbiStatus::INVALID_HANDLE;
    }
    DESTROY_COUNT.fetch_add(1, Ordering::SeqCst);
    write(output, ())
}

unsafe extern "C" fn session_id(
    _module: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    _request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    if !valid_session(session) {
        return ProjectRuntimeAbiStatus::INVALID_HANDLE;
    }
    write(output, "fixture.native.session")
}

unsafe extern "C" fn invoke_rule(
    _module: ProjectRuntimeOpaqueHandle,
    _session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    // SAFETY: the host owns the request bytes for this call.
    let Ok(request) = (unsafe { read_input::<ProjectRuntimeRuleRequest>(request) }) else {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    };
    write(
        output,
        ProjectRuntimeRuleOutput {
            status: if request.rule_id == "fixture.native.rule" {
                ProjectRuntimeStatus::Applied
            } else {
                ProjectRuntimeStatus::Unhandled
            },
            mutations: Vec::new(),
            diagnostics: Vec::new(),
        },
    )
}

unsafe extern "C" fn handle_aui_actions(
    _module: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    if !valid_session(session) {
        return ProjectRuntimeAbiStatus::INVALID_HANDLE;
    }
    ffi_boundary(output, || {
        // SAFETY: the host owns the request bytes for this call.
        let request: ProjectRuntimeAuiActionRequest = unsafe { read_input(request) }?;
        if request
            .actions
            .iter()
            .any(|action| action.action_id == "fixture.panic")
        {
            panic!("fixture panic");
        }
        ACTION_COUNT.fetch_add(request.actions.len() as u64, Ordering::SeqCst);
        Ok(ProjectRuntimeSessionOutput {
            status: ProjectRuntimeStatus::Applied,
            handled_action_count: request.actions.len() as u64,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations: Vec::new(),
            diagnostics: Vec::new(),
        })
    })
}

unsafe extern "C" fn fixed_update(
    _module: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    if !valid_session(session) {
        return ProjectRuntimeAbiStatus::INVALID_HANDLE;
    }
    // SAFETY: the host owns the request bytes for this call.
    if unsafe { read_input::<ProjectRuntimeFrameRequest>(request) }.is_err() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    FIXED_COUNT.fetch_add(1, Ordering::SeqCst);
    write(output, ProjectRuntimeSessionOutput::no_op())
}

unsafe extern "C" fn produce_ui_state(
    _module: ProjectRuntimeOpaqueHandle,
    _session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    // SAFETY: the host owns the request bytes for this call.
    if unsafe { read_input::<ProjectRuntimeUiStateResolveRequest>(request) }.is_err() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    write(
        output,
        ProjectRuntimeUiStateResolveOutput::Uncacheable {
            producer_id: "fixture.native.ui".to_string(),
            values: BTreeMap::from([
                (
                    "fixture.action_count".to_string(),
                    ProjectRuntimeValue::Integer(ACTION_COUNT.load(Ordering::SeqCst) as i64),
                ),
                (
                    "fixture.fixed_count".to_string(),
                    ProjectRuntimeValue::Integer(FIXED_COUNT.load(Ordering::SeqCst) as i64),
                ),
                (
                    "fixture.destroy_count".to_string(),
                    ProjectRuntimeValue::Integer(DESTROY_COUNT.load(Ordering::SeqCst) as i64),
                ),
            ]),
        },
    )
}

unsafe extern "C" fn observe(
    _module: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    _context: *const ProjectRuntimeCallContext,
    _request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    if !valid_session(session) {
        return ProjectRuntimeAbiStatus::INVALID_HANDLE;
    }
    write(
        output,
        ProjectRuntimeObservationOutput {
            values: BTreeMap::from([
                (
                    "fixture.destroy_count".to_string(),
                    ProjectRuntimeValue::Integer(DESTROY_COUNT.load(Ordering::SeqCst) as i64),
                ),
                (
                    "fixture.action_count".to_string(),
                    ProjectRuntimeValue::Integer(ACTION_COUNT.load(Ordering::SeqCst) as i64),
                ),
            ]),
        },
    )
}

static API: OnceLock<ProjectRuntimeApi> = OnceLock::new();

#[no_mangle]
pub unsafe extern "C" fn aife_project_runtime_entry_v1() -> *const ProjectRuntimeApi {
    API.get_or_init(|| ProjectRuntimeApi {
        struct_size: PROJECT_RUNTIME_API_STRUCT_SIZE,
        abi_major: PROJECT_RUNTIME_ABI_MAJOR,
        abi_minor: PROJECT_RUNTIME_ABI_MINOR,
        reserved: 0,
        capabilities: PROJECT_RUNTIME_CAP_RULES
            | PROJECT_RUNTIME_CAP_SESSIONS
            | PROJECT_RUNTIME_CAP_AUI_ACTIONS
            | PROJECT_RUNTIME_CAP_FIXED_UPDATE
            | PROJECT_RUNTIME_CAP_UI_STATE
            | PROJECT_RUNTIME_CAP_OBSERVATIONS
            | PROJECT_RUNTIME_CAP_WORLD_READ
            | PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS,
        module_context: ProjectRuntimeOpaqueHandle {
            value: 1,
            generation: 1,
        },
        contract_digest: project_runtime_contract_digest(),
        descriptor: Some(descriptor as ProjectRuntimeModuleCall),
        create_session: Some(create_session),
        destroy_session: Some(destroy_session),
        session_id: Some(session_id),
        invoke_rule: Some(invoke_rule),
        handle_aui_actions: Some(handle_aui_actions),
        fixed_update: Some(fixed_update),
        resolve_ui_state: Some(produce_ui_state),
        observe: Some(observe),
    })
}
