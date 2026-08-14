use ai_tool_gateway::{
    default_codex_config_artifact_root, default_codex_config_path, default_stable_mcp_path,
    install_codex_mcp_config, migrate_codex_to_stable_mcp, persist_codex_config_install_receipt,
    replace_codex_mcp_config, rollback_codex_mcp_config, rollback_stable_mcp_migration,
    CodexConfigInstallReceipt, StableMcpMigrationReceipt, StableMcpMigrationRequest,
};
use std::path::PathBuf;

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let result = match args.first().and_then(|value| value.to_str()) {
        Some("install") => install(&args[1..]),
        Some("replace") => replace(&args[1..]),
        Some("rollback") => rollback(&args[1..]),
        Some("migrate-stable") => migrate_stable(&args[1..]),
        Some("rollback-stable") => rollback_stable(&args[1..]),
        _ => Err("usage: ai_engine_gateway_codex_config install <mcp-command> [config-path] [artifact-root] | replace <expected-current-command> <mcp-command> [config-path] [artifact-root] | rollback <install-receipt> | migrate-stable <expected-current-command> <source-mcp> [config-path] [artifact-root] | rollback-stable <migration-receipt>".to_string()),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn migrate_stable(args: &[std::ffi::OsString]) -> Result<(), String> {
    let expected = args
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "migrate-stable requires the expected current MCP command".to_string())?;
    let source = args
        .get(1)
        .map(PathBuf::from)
        .ok_or_else(|| "migrate-stable requires the source MCP binary".to_string())?;
    let config = args
        .get(2)
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_codex_config_path)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let artifacts = args
        .get(3)
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_codex_config_artifact_root)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let stable =
        default_stable_mcp_path().map_err(|error| format!("{}: {}", error.code, error.message))?;
    let outcome = migrate_codex_to_stable_mcp(&StableMcpMigrationRequest {
        config_path: config,
        expected_current_command: expected,
        source_mcp_path: source,
        stable_mcp_path: stable,
        artifact_root: artifacts,
    })
    .map_err(|error| format!("{}: {}", error.code, error.message))?;
    println!(
        "migrated={} reload_required={} command={} config={} receipt={}",
        outcome.receipt.binary_changed || outcome.receipt.config.changed,
        outcome.receipt.config.reload_or_new_task_required,
        outcome.receipt.installed_mcp_path.display(),
        outcome.receipt.config.config_path.display(),
        outcome.receipt_path.display()
    );
    Ok(())
}

fn rollback_stable(args: &[std::ffi::OsString]) -> Result<(), String> {
    let receipt_path = args
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "rollback-stable requires a migration receipt path".to_string())?;
    let bytes = std::fs::read(&receipt_path).map_err(|error| error.to_string())?;
    let receipt: StableMcpMigrationReceipt =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let rollback = rollback_stable_mcp_migration(&receipt)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    println!(
        "rolled_back=true binary_restored={} removed_new_binary={} command={} config={}",
        rollback.binary_restored,
        rollback.removed_new_binary,
        rollback.installed_mcp_path.display(),
        rollback.config.config_path.display()
    );
    Ok(())
}

fn replace(args: &[std::ffi::OsString]) -> Result<(), String> {
    let expected = args
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "replace requires the expected current MCP command path".to_string())?;
    let command = args
        .get(1)
        .map(PathBuf::from)
        .ok_or_else(|| "replace requires the new frozen MCP command path".to_string())?;
    let config = args
        .get(2)
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_codex_config_path)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let artifacts = args
        .get(3)
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_codex_config_artifact_root)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let receipt = replace_codex_mcp_config(&config, &expected, &command, &artifacts)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let receipt_path = persist_codex_config_install_receipt(&receipt, &artifacts)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    println!(
        "replaced={} reload_required={} config={} receipt={}",
        receipt.changed,
        receipt.reload_or_new_task_required,
        receipt.config_path.display(),
        receipt_path.display()
    );
    Ok(())
}

fn install(args: &[std::ffi::OsString]) -> Result<(), String> {
    let command = args
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "install requires the frozen MCP command path".to_string())?;
    let config = args
        .get(1)
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_codex_config_path)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let artifacts = args
        .get(2)
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_codex_config_artifact_root)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let receipt = install_codex_mcp_config(&config, &command, &artifacts)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let receipt_path = persist_codex_config_install_receipt(&receipt, &artifacts)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    println!(
        "installed={} reload_required={} config={} receipt={}",
        receipt.changed,
        receipt.reload_or_new_task_required,
        receipt.config_path.display(),
        receipt_path.display()
    );
    Ok(())
}

fn rollback(args: &[std::ffi::OsString]) -> Result<(), String> {
    let receipt_path = args
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "rollback requires an install receipt path".to_string())?;
    let bytes = std::fs::read(&receipt_path).map_err(|error| error.to_string())?;
    let receipt: CodexConfigInstallReceipt =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let rollback = rollback_codex_mcp_config(&receipt)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    println!(
        "rolled_back=true removed_new_config={} config={}",
        rollback.removed_new_config,
        rollback.config_path.display()
    );
    Ok(())
}
