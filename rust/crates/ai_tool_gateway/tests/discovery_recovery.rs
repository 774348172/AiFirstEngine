use ai_tool_gateway::{
    discovery_record_path, resolve_gateway_discovery_path, resolve_gateway_discovery_path_in_root,
    GatewayDiscoveryRecord,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "aife-259-discovery-{label}-{}-{stamp}",
        std::process::id()
    ))
}

fn write_v2(root: &Path, instance: &str, process_id: u32) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    let mut record = GatewayDiscoveryRecord::new(instance);
    record.editor_process_id = process_id;
    let path = discovery_record_path(root, instance);
    fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    path
}

fn write_v1(root: &Path, digest_hex: &str, process_id: u32) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    let path = root.join(format!("{digest_hex}.json"));
    let value = json!({
        "schemaVersion": "ai-tool-gateway-discovery.v1",
        "gatewayProtocolVersion": "ai-tool-gateway.v1",
        "editorProcessId": process_id,
        "projectIdentity": "legacy-project",
        "canonicalProjectRootDigest": format!("sha256:{digest_hex}"),
        "pipeLocator": "\\\\.\\pipe\\ai-first-game-engine\\legacy",
        "publishedAtEpochMs": 1
    });
    fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    path
}

#[test]
fn unique_live_v2_wins_over_owned_dead_v1_and_v2_records() {
    let root = unique_root("mixed");
    let live = write_v2(&root, "live-compatible", std::process::id());
    let dead_v2 = write_v2(&root, "dead-v2", u32::MAX);
    let dead_v1 = write_v1(
        &root,
        "1111111111111111111111111111111111111111111111111111111111111111",
        u32::MAX,
    );

    assert_eq!(
        resolve_gateway_discovery_path_in_root(&root, None).unwrap(),
        live
    );
    assert!(!dead_v1.exists());
    assert!(!dead_v2.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn multiple_live_compatible_editors_remain_ambiguous() {
    let root = unique_root("ambiguous");
    write_v2(&root, "live-a", std::process::id());
    write_v2(&root, "live-b", std::process::id());

    let error = resolve_gateway_discovery_path_in_root(&root, None).unwrap_err();

    assert_eq!(error.code, "gateway.discovery.ambiguous_editor_instance");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn only_live_legacy_editor_returns_active_incompatible_diagnostic() {
    let root = unique_root("incompatible");
    write_v1(
        &root,
        "2222222222222222222222222222222222222222222222222222222222222222",
        std::process::id(),
    );

    let error = resolve_gateway_discovery_path_in_root(&root, None).unwrap_err();

    assert_eq!(error.code, "gateway.discovery.active_incompatible");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn malformed_or_unowned_legacy_record_fails_closed_even_when_pid_is_dead() {
    let root = unique_root("invalid-legacy");
    let path = write_v1(
        &root,
        "3333333333333333333333333333333333333333333333333333333333333333",
        u32::MAX,
    );
    let wrong_path = root.join("wrong-name.json");
    fs::rename(path, &wrong_path).unwrap();

    let error = resolve_gateway_discovery_path_in_root(&root, None).unwrap_err();

    assert_eq!(
        error.code,
        "gateway.discovery.legacy_filename_digest_mismatch"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn best_effort_dead_cleanup_failure_does_not_hide_unique_live_editor() {
    let root = unique_root("cleanup-failure");
    let live = write_v2(&root, "live-compatible", std::process::id());
    let dead = write_v1(
        &root,
        "4444444444444444444444444444444444444444444444444444444444444444",
        u32::MAX,
    );
    let mut permissions = fs::metadata(&dead).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&dead, permissions).unwrap();

    assert_eq!(
        resolve_gateway_discovery_path_in_root(&root, None).unwrap(),
        live
    );

    if dead.exists() {
        let mut permissions = fs::metadata(&dead).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&dead, permissions).unwrap();
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_legacy_path_remains_strictly_unsupported() {
    let root = unique_root("explicit");
    let legacy = write_v1(
        &root,
        "5555555555555555555555555555555555555555555555555555555555555555",
        u32::MAX,
    );

    let error = resolve_gateway_discovery_path(Some(&legacy), None).unwrap_err();

    assert_eq!(error.code, "gateway.discovery.schema_unsupported");
    let _ = fs::remove_dir_all(root);
}
