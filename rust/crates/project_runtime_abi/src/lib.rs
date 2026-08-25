//! Stable C ABI shared by the Editor host and project native runtime modules.
//!
//! All pointers are borrowed only for the duration of the call that receives them. The caller
//! owns input and output memory. Opaque handles are values, not pointers, and are valid only while
//! their owning module or host context remains alive. No Rust-owned collection, trait object, ECS
//! object, or unwinding exception may cross this boundary.

use sha2::{Digest, Sha256};
use std::mem::size_of;

pub const PROJECT_RUNTIME_ABI_MAJOR: u32 = 1;
pub const PROJECT_RUNTIME_ABI_MINOR: u32 = 1;
pub const PROJECT_RUNTIME_ENTRY_SYMBOL: &[u8] = b"aife_project_runtime_entry_v1\0";
pub const PROJECT_RUNTIME_ABI_SCHEMA: &str = concat!(
    "project-runtime-abi.v1;",
    "status:i32;handle:u64,u64;slice:*const-u8,u64;buffer:*mut-u8,u64,u64;",
    "host-api:u32,u32,world-query,world-read-component;",
    "call-context:u32,u32,handle,*host-api,u64,time-context;",
    "module-api:u32,u32,u32,u32,u64,handle,[u8;32],",
    "descriptor,create-session,destroy-session,session-id,invoke-rule,",
    "handle-aui-actions,fixed-update,resolve-ui-state,observe"
);

pub const PROJECT_RUNTIME_CAP_RULES: u64 = 1 << 0;
pub const PROJECT_RUNTIME_CAP_SESSIONS: u64 = 1 << 1;
pub const PROJECT_RUNTIME_CAP_AUI_ACTIONS: u64 = 1 << 2;
pub const PROJECT_RUNTIME_CAP_FIXED_UPDATE: u64 = 1 << 3;
pub const PROJECT_RUNTIME_CAP_UI_STATE: u64 = 1 << 4;
pub const PROJECT_RUNTIME_CAP_OBSERVATIONS: u64 = 1 << 5;
pub const PROJECT_RUNTIME_CAP_WORLD_READ: u64 = 1 << 6;
pub const PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS: u64 = 1 << 7;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectRuntimeAbiStatus(pub i32);

impl ProjectRuntimeAbiStatus {
    pub const OK: Self = Self(0);
    pub const INVALID_ARGUMENT: Self = Self(1);
    pub const BUFFER_TOO_SMALL: Self = Self(2);
    pub const INVALID_HANDLE: Self = Self(3);
    pub const UNSUPPORTED: Self = Self(4);
    pub const FAILED: Self = Self(5);
    pub const PANICKED: Self = Self(6);
    pub const TERMINAL_FAULT: Self = Self(7);

    pub const fn is_ok(self) -> bool {
        self.0 == Self::OK.0
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProjectRuntimeOpaqueHandle {
    pub value: u64,
    pub generation: u64,
}

impl ProjectRuntimeOpaqueHandle {
    pub const NULL: Self = Self {
        value: 0,
        generation: 0,
    };

    pub const fn is_null(self) -> bool {
        self.value == 0 && self.generation == 0
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProjectRuntimeByteSlice {
    pub data: *const u8,
    pub length: u64,
}

impl ProjectRuntimeByteSlice {
    pub const EMPTY: Self = Self {
        data: std::ptr::null(),
        length: 0,
    };

    pub fn from_slice(value: &[u8]) -> Self {
        Self {
            data: value.as_ptr(),
            length: value.len() as u64,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ProjectRuntimeByteBuffer {
    pub data: *mut u8,
    pub capacity: u64,
    /// On return, the callee writes the produced or required byte count here.
    pub written: u64,
}

impl ProjectRuntimeByteBuffer {
    pub fn from_slice(value: &mut [u8]) -> Self {
        Self {
            data: value.as_mut_ptr(),
            capacity: value.len() as u64,
            written: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ProjectRuntimeTimeContext {
    pub time: f32,
    pub delta_time: f32,
    pub unscaled_time: f32,
    pub unscaled_delta_time: f32,
    pub fixed_time: f32,
    pub fixed_delta_time: f32,
    pub frame_count: u64,
    pub fixed_frame_count: u64,
    pub time_scale: f32,
    pub in_fixed_step: u32,
}

pub type ProjectRuntimeHostCall = unsafe extern "C" fn(
    host_context: ProjectRuntimeOpaqueHandle,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus;

/// Host callbacks never expose a World/ECS pointer. `host_context` is resolved by the host and is
/// valid only for the containing module call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProjectRuntimeHostApi {
    pub struct_size: u32,
    pub reserved: u32,
    pub world_query: Option<ProjectRuntimeHostCall>,
    pub world_read_component: Option<ProjectRuntimeHostCall>,
}

pub const PROJECT_RUNTIME_HOST_API_STRUCT_SIZE: u32 = size_of::<ProjectRuntimeHostApi>() as u32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProjectRuntimeCallContext {
    pub struct_size: u32,
    pub reserved: u32,
    pub host_context: ProjectRuntimeOpaqueHandle,
    pub host_api: *const ProjectRuntimeHostApi,
    pub frame_index: u64,
    pub time: ProjectRuntimeTimeContext,
}

pub const PROJECT_RUNTIME_CALL_CONTEXT_STRUCT_SIZE: u32 =
    size_of::<ProjectRuntimeCallContext>() as u32;

pub type ProjectRuntimeModuleCall = unsafe extern "C" fn(
    module_context: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    call_context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus;

/// The module owns `module_context` and all returned session handles. The host must call
/// `destroy_session` exactly once for each successful `create_session` before releasing the DLL.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProjectRuntimeApi {
    pub struct_size: u32,
    pub abi_major: u32,
    pub abi_minor: u32,
    pub reserved: u32,
    pub capabilities: u64,
    pub module_context: ProjectRuntimeOpaqueHandle,
    pub contract_digest: [u8; 32],
    pub descriptor: Option<ProjectRuntimeModuleCall>,
    pub create_session: Option<ProjectRuntimeModuleCall>,
    pub destroy_session: Option<ProjectRuntimeModuleCall>,
    pub session_id: Option<ProjectRuntimeModuleCall>,
    pub invoke_rule: Option<ProjectRuntimeModuleCall>,
    pub handle_aui_actions: Option<ProjectRuntimeModuleCall>,
    pub fixed_update: Option<ProjectRuntimeModuleCall>,
    pub resolve_ui_state: Option<ProjectRuntimeModuleCall>,
    pub observe: Option<ProjectRuntimeModuleCall>,
}

pub const PROJECT_RUNTIME_API_STRUCT_SIZE: u32 = size_of::<ProjectRuntimeApi>() as u32;

pub type ProjectRuntimeEntry = unsafe extern "C" fn() -> *const ProjectRuntimeApi;

pub fn project_runtime_abi_digest() -> [u8; 32] {
    digest_schema(PROJECT_RUNTIME_ABI_SCHEMA)
}

pub fn project_runtime_abi_digest_hex() -> String {
    hex_digest(project_runtime_abi_digest())
}

pub fn digest_schema(schema: &str) -> [u8; 32] {
    Sha256::digest(schema.as_bytes()).into()
}

pub fn hex_digest(digest: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_layout_uses_fixed_width_fields_and_explicit_struct_sizes() {
        assert_eq!(size_of::<ProjectRuntimeAbiStatus>(), size_of::<i32>());
        assert_eq!(size_of::<ProjectRuntimeOpaqueHandle>(), 16);
        assert_eq!(
            PROJECT_RUNTIME_HOST_API_STRUCT_SIZE as usize,
            size_of::<ProjectRuntimeHostApi>()
        );
        assert_eq!(
            PROJECT_RUNTIME_CALL_CONTEXT_STRUCT_SIZE as usize,
            size_of::<ProjectRuntimeCallContext>()
        );
        assert_eq!(
            PROJECT_RUNTIME_API_STRUCT_SIZE as usize,
            size_of::<ProjectRuntimeApi>()
        );
    }

    #[test]
    fn canonical_digest_is_stable_and_schema_sensitive() {
        let first = project_runtime_abi_digest();
        assert_eq!(first, project_runtime_abi_digest());
        assert_ne!(
            first,
            digest_schema(&format!("{PROJECT_RUNTIME_ABI_SCHEMA};changed"))
        );
        assert_eq!(project_runtime_abi_digest_hex().len(), 64);
    }

    #[test]
    fn null_and_generation_handles_are_distinct() {
        assert!(ProjectRuntimeOpaqueHandle::NULL.is_null());
        assert_ne!(
            ProjectRuntimeOpaqueHandle {
                value: 7,
                generation: 1
            },
            ProjectRuntimeOpaqueHandle {
                value: 7,
                generation: 2
            }
        );
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
