use ai_tool_gateway::{
    migrate_codex_to_stable_mcp, rollback_stable_mcp_migration, stable_mcp_path_in_local_app_data,
    StableMcpMigrationRequest, STABLE_MCP_MIGRATION_RECEIPT_SCHEMA_VERSION,
};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("aife-259-{label}-{}-{stamp}", std::process::id()))
}

fn write_config(path: &Path, command: &Path) -> Vec<u8> {
    let bytes = format!(
        "model = \"keep-model\"\n\n[mcp_servers.other]\nurl = \"https://example.invalid/mcp\"\n\n[mcp_servers.ai_first_game_engine]\ncommand = {:?}\n",
        command.display().to_string()
    )
    .into_bytes();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

#[test]
fn stable_migration_installs_exact_binary_preserves_config_and_rolls_back() {
    let root = unique_root("stable-success");
    let config = root.join("codex/config.toml");
    let expected = root.join("historical/ai_engine_gateway_mcp.exe");
    let source = root.join("build/ai_engine_gateway_mcp.exe");
    let stable = stable_mcp_path_in_local_app_data(&root.join("local-app-data"));
    let artifacts = root.join("artifacts");
    let before_config = write_config(&config, &expected);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::create_dir_all(stable.parent().unwrap()).unwrap();
    fs::write(&source, b"new-production-mcp").unwrap();
    fs::write(&stable, b"previous-stable-mcp").unwrap();

    let outcome = migrate_codex_to_stable_mcp(&StableMcpMigrationRequest {
        config_path: config.clone(),
        expected_current_command: expected,
        source_mcp_path: source.clone(),
        stable_mcp_path: stable.clone(),
        artifact_root: artifacts,
    })
    .unwrap();

    assert_eq!(
        outcome.receipt.schema_version,
        STABLE_MCP_MIGRATION_RECEIPT_SCHEMA_VERSION
    );
    assert!(outcome.receipt.binary_changed);
    assert_eq!(fs::read(&stable).unwrap(), b"new-production-mcp");
    assert_eq!(
        outcome.receipt.source_mcp_digest,
        outcome.receipt.installed_mcp_digest
    );
    assert!(outcome.receipt.previous_installed_backup_path.is_some());
    assert!(outcome.receipt.config.changed);
    assert!(outcome.receipt.config.reload_or_new_task_required);
    assert!(outcome.receipt_path.is_file());
    let installed_config = fs::read_to_string(&config).unwrap();
    assert!(installed_config.contains("keep-model"));
    assert!(installed_config.contains("mcp_servers.other"));
    assert!(installed_config.contains(&stable.display().to_string()));
    assert!(!installed_config.contains(&source.display().to_string()));

    let rollback = rollback_stable_mcp_migration(&outcome.receipt).unwrap();
    assert!(rollback.binary_restored);
    assert_eq!(fs::read(&stable).unwrap(), b"previous-stable-mcp");
    assert_eq!(fs::read(&config).unwrap(), before_config);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stable_migration_exact_config_drift_restores_existing_stable_binary() {
    let root = unique_root("stable-config-drift");
    let config = root.join("codex/config.toml");
    let actual = root.join("actual/ai_engine_gateway_mcp.exe");
    let expected = root.join("different/ai_engine_gateway_mcp.exe");
    let source = root.join("build/ai_engine_gateway_mcp.exe");
    let stable = stable_mcp_path_in_local_app_data(&root.join("local-app-data"));
    let artifacts = root.join("artifacts");
    let before_config = write_config(&config, &actual);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::create_dir_all(stable.parent().unwrap()).unwrap();
    fs::write(&source, b"new-production-mcp").unwrap();
    fs::write(&stable, b"previous-stable-mcp").unwrap();

    let error = migrate_codex_to_stable_mcp(&StableMcpMigrationRequest {
        config_path: config.clone(),
        expected_current_command: expected,
        source_mcp_path: source,
        stable_mcp_path: stable.clone(),
        artifact_root: artifacts,
    })
    .unwrap_err();

    assert_eq!(
        error.code,
        "gateway.codex_config.server_replace_precondition_failed"
    );
    assert_eq!(fs::read(&stable).unwrap(), b"previous-stable-mcp");
    assert_eq!(fs::read(&config).unwrap(), before_config);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stable_migration_exact_config_drift_removes_newly_installed_binary() {
    let root = unique_root("stable-new-config-drift");
    let config = root.join("codex/config.toml");
    let actual = root.join("actual/ai_engine_gateway_mcp.exe");
    let expected = root.join("different/ai_engine_gateway_mcp.exe");
    let source = root.join("build/ai_engine_gateway_mcp.exe");
    let stable = stable_mcp_path_in_local_app_data(&root.join("local-app-data"));
    let artifacts = root.join("artifacts");
    let before_config = write_config(&config, &actual);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, b"new-production-mcp").unwrap();

    let error = migrate_codex_to_stable_mcp(&StableMcpMigrationRequest {
        config_path: config.clone(),
        expected_current_command: expected,
        source_mcp_path: source,
        stable_mcp_path: stable.clone(),
        artifact_root: artifacts,
    })
    .unwrap_err();

    assert_eq!(
        error.code,
        "gateway.codex_config.server_replace_precondition_failed"
    );
    assert!(!stable.exists());
    assert_eq!(fs::read(&config).unwrap(), before_config);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stable_migration_updates_binary_when_config_already_targets_stable_path() {
    let root = unique_root("stable-config-unchanged");
    let config = root.join("codex/config.toml");
    let source = root.join("build/ai_engine_gateway_mcp.exe");
    let stable = stable_mcp_path_in_local_app_data(&root.join("local-app-data"));
    let artifacts = root.join("artifacts");
    let before_config = write_config(&config, &stable);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::create_dir_all(stable.parent().unwrap()).unwrap();
    fs::write(&source, b"new-production-mcp").unwrap();
    fs::write(&stable, b"previous-stable-mcp").unwrap();

    let outcome = migrate_codex_to_stable_mcp(&StableMcpMigrationRequest {
        config_path: config.clone(),
        expected_current_command: stable.clone(),
        source_mcp_path: source,
        stable_mcp_path: stable.clone(),
        artifact_root: artifacts,
    })
    .unwrap();

    assert!(outcome.receipt.binary_changed);
    assert!(!outcome.receipt.config.changed);
    assert_eq!(fs::read(&stable).unwrap(), b"new-production-mcp");
    let rollback = rollback_stable_mcp_migration(&outcome.receipt).unwrap();
    assert!(rollback.binary_restored);
    assert_eq!(fs::read(&stable).unwrap(), b"previous-stable-mcp");
    assert_eq!(fs::read(&config).unwrap(), before_config);
    let _ = fs::remove_dir_all(root);
}
