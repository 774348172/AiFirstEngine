use engine_public_api::{aife_engine_api_info_v1, aife_engine_runtime_start_v1, compatible_with, ENGINE_OK};

#[test]
fn project_client_uses_only_public_engine_contract() {
    let info = aife_engine_api_info_v1();
    assert!(compatible_with(info.api_major, 0));
    assert_eq!(aife_engine_runtime_start_v1().status, ENGINE_OK);
}

#[test]
fn incompatible_engine_major_fails_closed() {
    assert!(!compatible_with(99, 0));
}
