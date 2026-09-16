#![allow(clippy::missing_safety_doc)]

use std::sync::OnceLock;

pub const ENGINE_API_MAJOR: u32 = 1;
pub const ENGINE_API_MINOR: u32 = 0;
pub const ENGINE_API_CAPABILITIES: u64 = 0b1111;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EngineApiInfo {
    pub struct_size: u32,
    pub api_major: u32,
    pub api_minor: u32,
    pub capabilities: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EngineApiResult {
    pub status: i32,
    pub detail_code: u32,
}

pub const ENGINE_OK: i32 = 0;
pub const ENGINE_INVALID_ARGUMENT: i32 = 1;

static API_INFO: OnceLock<EngineApiInfo> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn aife_engine_api_info_v1() -> EngineApiInfo {
    *API_INFO.get_or_init(|| EngineApiInfo {
        struct_size: std::mem::size_of::<EngineApiInfo>() as u32,
        api_major: ENGINE_API_MAJOR,
        api_minor: ENGINE_API_MINOR,
        capabilities: ENGINE_API_CAPABILITIES,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn aife_engine_runtime_start_v1() -> EngineApiResult {
    EngineApiResult { status: ENGINE_OK, detail_code: 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn aife_engine_runtime_stop_v1() -> EngineApiResult {
    EngineApiResult { status: ENGINE_OK, detail_code: 0 }
}

pub fn compatible_with(required_major: u32, required_minor: u32) -> bool {
    let info = aife_engine_api_info_v1();
    info.api_major == required_major && info.api_minor >= required_minor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_has_stable_version_and_layout() {
        let info = aife_engine_api_info_v1();
        assert_eq!(info.api_major, ENGINE_API_MAJOR);
        assert_eq!(info.struct_size as usize, std::mem::size_of::<EngineApiInfo>());
        assert_eq!(aife_engine_runtime_start_v1().status, ENGINE_OK);
        assert_eq!(aife_engine_runtime_stop_v1().status, ENGINE_OK);
        assert!(compatible_with(1, 0));
        assert!(!compatible_with(2, 0));
    }
}
