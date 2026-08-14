use crate::GatewayControlError;
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{value, DocumentMut, Item, Table};

pub const CODEX_CONFIG_INSTALL_RECEIPT_SCHEMA_VERSION: &str =
    "ai-tool-gateway-codex-config-install-receipt.v1";
pub const CODEX_CONFIG_ROLLBACK_RECEIPT_SCHEMA_VERSION: &str =
    "ai-tool-gateway-codex-config-rollback-receipt.v1";
pub const STABLE_MCP_MIGRATION_RECEIPT_SCHEMA_VERSION: &str =
    "ai-tool-gateway-stable-mcp-migration-receipt.v1";
pub const STABLE_MCP_MIGRATION_ROLLBACK_SCHEMA_VERSION: &str =
    "ai-tool-gateway-stable-mcp-migration-rollback.v1";
const CODEX_MCP_SERVER_ID: &str = "ai_first_game_engine";
const MAX_CODEX_CONFIG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MCP_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexConfigInstallReceipt {
    pub schema_version: String,
    pub config_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub fragment_path: PathBuf,
    pub before_digest: String,
    pub after_digest: String,
    pub command_digest: String,
    pub changed: bool,
    pub reload_or_new_task_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexConfigRollbackReceipt {
    pub schema_version: String,
    pub config_path: PathBuf,
    pub restored_digest: String,
    pub removed_new_config: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableMcpMigrationRequest {
    pub config_path: PathBuf,
    pub expected_current_command: PathBuf,
    pub source_mcp_path: PathBuf,
    pub stable_mcp_path: PathBuf,
    pub artifact_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableMcpMigrationReceipt {
    pub schema_version: String,
    pub source_mcp_path: PathBuf,
    pub source_mcp_digest: String,
    pub installed_mcp_path: PathBuf,
    pub installed_mcp_digest: String,
    pub binary_changed: bool,
    pub previous_installed_digest: Option<String>,
    pub previous_installed_backup_path: Option<PathBuf>,
    pub config: CodexConfigInstallReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableMcpMigrationOutcome {
    pub receipt: StableMcpMigrationReceipt,
    pub receipt_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableMcpMigrationRollbackReceipt {
    pub schema_version: String,
    pub installed_mcp_path: PathBuf,
    pub binary_restored: bool,
    pub removed_new_binary: bool,
    pub config: CodexConfigRollbackReceipt,
}

pub fn default_codex_config_path() -> Result<PathBuf, GatewayControlError> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|value| PathBuf::from(value).join(".codex"))
        })
        .ok_or_else(|| {
            config_error(
                "gateway.codex_config.home_unavailable",
                "Neither CODEX_HOME nor USERPROFILE is available for Codex configuration.",
                "Run the installer in the same interactive user account as Codex Desktop.",
            )
        })?;
    Ok(home.join("config.toml"))
}

pub fn default_codex_config_artifact_root() -> Result<PathBuf, GatewayControlError> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        config_error(
            "gateway.codex_config.local_app_data_unavailable",
            "LOCALAPPDATA is unavailable for private install and rollback artifacts.",
            "Run the installer in a normal interactive user session.",
        )
    })?;
    Ok(PathBuf::from(local)
        .join("AiFirstGameEngine")
        .join("Gateway")
        .join("codex-config"))
}

pub fn stable_mcp_path_in_local_app_data(local_app_data: &Path) -> PathBuf {
    local_app_data
        .join("AiFirstGameEngine")
        .join("bin")
        .join("ai_engine_gateway_mcp.exe")
}

pub fn default_stable_mcp_path() -> Result<PathBuf, GatewayControlError> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        config_error(
            "gateway.stable_install.local_app_data_unavailable",
            "LOCALAPPDATA is unavailable for the stable MCP installation.",
            "Run the installer in a normal interactive user session.",
        )
    })?;
    Ok(stable_mcp_path_in_local_app_data(&PathBuf::from(local)))
}

pub fn migrate_codex_to_stable_mcp(
    request: &StableMcpMigrationRequest,
) -> Result<StableMcpMigrationOutcome, GatewayControlError> {
    validate_absolute_regular_file(&request.source_mcp_path, "source")?;
    if !request.config_path.is_absolute()
        || !request.expected_current_command.is_absolute()
        || !request.stable_mcp_path.is_absolute()
        || !request.artifact_root.is_absolute()
    {
        return Err(config_error(
            "gateway.stable_install.path_invalid",
            "Stable MCP migration paths must all be absolute.",
            "Resolve the current-user config, source, install, and artifact paths before retrying.",
        ));
    }
    if request.source_mcp_path == request.stable_mcp_path {
        return Err(config_error(
            "gateway.stable_install.source_is_destination",
            "Stable MCP source and destination paths must be different.",
            "Build the MCP outside the installation directory and retry.",
        ));
    }
    ensure_no_reparse_ancestors(&request.stable_mcp_path)?;
    ensure_no_reparse_ancestors(&request.artifact_root)?;
    verify_codex_replace_precondition(&request.config_path, &request.expected_current_command)?;

    fs::create_dir_all(&request.artifact_root).map_err(|error| {
        config_error(
            "gateway.stable_install.artifact_root_create_failed",
            format!("Failed to create stable MCP artifact root: {error}"),
            "Repair current-user local application data permissions.",
        )
    })?;
    let source = read_bounded_regular_file(
        &request.source_mcp_path,
        MAX_MCP_EXECUTABLE_BYTES,
        "gateway.stable_install.source",
    )?;
    let source_digest = sha256_prefixed(&source);
    let stamp = now_epoch_ms();
    let previous = read_optional_bounded_regular_file(
        &request.stable_mcp_path,
        MAX_MCP_EXECUTABLE_BYTES,
        "gateway.stable_install.destination",
    )?;
    let previous_installed_digest = previous.as_ref().map(|bytes| sha256_prefixed(bytes));
    let previous_installed_backup_path = if let Some(bytes) = &previous {
        let path = request
            .artifact_root
            .join(format!("{stamp}-stable-mcp-before.exe"));
        write_atomic(&path, bytes)?;
        Some(path)
    } else {
        None
    };
    let binary_changed = previous.as_deref() != Some(source.as_slice());
    if binary_changed {
        write_atomic(&request.stable_mcp_path, &source)?;
    }
    let installed = read_bounded_regular_file(
        &request.stable_mcp_path,
        MAX_MCP_EXECUTABLE_BYTES,
        "gateway.stable_install.installed",
    )?;
    let installed_digest = sha256_prefixed(&installed);
    if installed_digest != source_digest {
        restore_installed_binary(
            &request.stable_mcp_path,
            previous.as_deref(),
            binary_changed,
        )?;
        return Err(config_error(
            "gateway.stable_install.readback_mismatch",
            "Installed MCP bytes do not match the selected source binary.",
            "Do not update Codex config; inspect the installation storage.",
        ));
    }

    let config = match replace_codex_mcp_config(
        &request.config_path,
        &request.expected_current_command,
        &request.stable_mcp_path,
        &request.artifact_root,
    ) {
        Ok(receipt) => receipt,
        Err(error) => {
            restore_installed_binary(
                &request.stable_mcp_path,
                previous.as_deref(),
                binary_changed,
            )?;
            return Err(error);
        }
    };
    let receipt = StableMcpMigrationReceipt {
        schema_version: STABLE_MCP_MIGRATION_RECEIPT_SCHEMA_VERSION.to_string(),
        source_mcp_path: request.source_mcp_path.clone(),
        source_mcp_digest: source_digest,
        installed_mcp_path: request.stable_mcp_path.clone(),
        installed_mcp_digest: installed_digest,
        binary_changed,
        previous_installed_digest,
        previous_installed_backup_path,
        config,
    };
    let receipt_path = request
        .artifact_root
        .join(format!("{stamp}-stable-mcp-migration-receipt.json"));
    let receipt_bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| {
        config_error(
            "gateway.stable_install.receipt_serialize_failed",
            format!("Failed to serialize stable MCP migration receipt: {error}"),
            "Rollback the config and installed binary before retrying.",
        )
    })?;
    if let Err(error) = write_atomic(&receipt_path, &receipt_bytes) {
        let config_rollback = rollback_codex_mcp_config(&receipt.config);
        let binary_rollback = restore_installed_binary(
            &receipt.installed_mcp_path,
            previous.as_deref(),
            receipt.binary_changed,
        );
        return match (config_rollback, binary_rollback) {
            (Ok(_), Ok(())) => Err(error),
            (config_result, binary_result) => Err(config_error(
                "gateway.stable_install.receipt_persist_rollback_failed",
                format!(
                    "Migration receipt persistence failed and rollback was incomplete; config={:?}, binary={:?}",
                    config_result.err().map(|value| value.code),
                    binary_result.err().map(|value| value.code)
                ),
                "Preserve the config backup and installed binary before manual recovery.",
            )),
        };
    }
    Ok(StableMcpMigrationOutcome {
        receipt,
        receipt_path,
    })
}

fn verify_codex_replace_precondition(
    config_path: &Path,
    expected_current_command: &Path,
) -> Result<(), GatewayControlError> {
    let before = read_bounded_config(config_path)?;
    let document = std::str::from_utf8(&before)
        .map_err(|_| {
            config_error(
                "gateway.codex_config.not_utf8",
                "Codex config is not valid UTF-8.",
                "Repair the config before installing the MCP entry.",
            )
        })?
        .parse::<DocumentMut>()
        .map_err(|error| {
            config_error(
                "gateway.codex_config.toml_invalid",
                format!("Codex config is invalid TOML: {error}"),
                "Repair the config before installing the MCP entry.",
            )
        })?;
    let actual = document
        .get("mcp_servers")
        .and_then(Item::as_table)
        .and_then(|servers| servers.get(CODEX_MCP_SERVER_ID))
        .and_then(Item::as_table)
        .and_then(|server| server.get("command"))
        .and_then(Item::as_str);
    let expected = expected_current_command.display().to_string();
    if actual != Some(expected.as_str()) {
        return Err(config_error(
            "gateway.codex_config.server_replace_precondition_failed",
            "Codex ai_first_game_engine MCP entry does not match the expected current command.",
            "Reload the current config and retry with its exact MCP command path.",
        ));
    }
    Ok(())
}

pub fn rollback_stable_mcp_migration(
    receipt: &StableMcpMigrationReceipt,
) -> Result<StableMcpMigrationRollbackReceipt, GatewayControlError> {
    if receipt.schema_version != STABLE_MCP_MIGRATION_RECEIPT_SCHEMA_VERSION {
        return Err(config_error(
            "gateway.stable_install.receipt_schema_unsupported",
            "Stable MCP migration receipt uses an unsupported schema.",
            "Use the exact receipt created by this installer version.",
        ));
    }
    let installed = read_bounded_regular_file(
        &receipt.installed_mcp_path,
        MAX_MCP_EXECUTABLE_BYTES,
        "gateway.stable_install.rollback_installed",
    )?;
    if sha256_prefixed(&installed) != receipt.installed_mcp_digest {
        return Err(config_error(
            "gateway.stable_install.rollback_binary_drifted",
            "Installed MCP changed after migration; automatic rollback is unsafe.",
            "Preserve the current binary and review the migration receipt manually.",
        ));
    }
    let current_config = read_bounded_config(&receipt.config.config_path)?;
    if sha256_prefixed(&current_config) != receipt.config.after_digest {
        return Err(config_error(
            "gateway.codex_config.rollback_drifted",
            "Codex config changed after installation; automatic rollback is unsafe.",
            "Review the current config and merge the rollback manually.",
        ));
    }

    let previous = match &receipt.previous_installed_backup_path {
        Some(path) => {
            let bytes = read_bounded_regular_file(
                path,
                MAX_MCP_EXECUTABLE_BYTES,
                "gateway.stable_install.rollback_backup",
            )?;
            if receipt.previous_installed_digest.as_deref() != Some(&sha256_prefixed(&bytes)) {
                return Err(config_error(
                    "gateway.stable_install.rollback_backup_tampered",
                    "Stable MCP backup digest does not match the migration receipt.",
                    "Do not overwrite the installed binary; inspect the backup manually.",
                ));
            }
            Some(bytes)
        }
        None => None,
    };
    restore_installed_binary(
        &receipt.installed_mcp_path,
        previous.as_deref(),
        receipt.binary_changed,
    )?;
    let config = rollback_codex_mcp_config(&receipt.config)?;
    Ok(StableMcpMigrationRollbackReceipt {
        schema_version: STABLE_MCP_MIGRATION_ROLLBACK_SCHEMA_VERSION.to_string(),
        installed_mcp_path: receipt.installed_mcp_path.clone(),
        binary_restored: receipt.binary_changed && previous.is_some(),
        removed_new_binary: receipt.binary_changed && previous.is_none(),
        config,
    })
}

fn validate_absolute_regular_file(path: &Path, role: &str) -> Result<(), GatewayControlError> {
    if !path.is_absolute() {
        return Err(config_error(
            format!("gateway.stable_install.{role}_path_invalid"),
            format!("Stable MCP {role} path must be absolute."),
            "Resolve the path before retrying.",
        ));
    }
    let _ = read_bounded_regular_file(
        path,
        MAX_MCP_EXECUTABLE_BYTES,
        &format!("gateway.stable_install.{role}"),
    )?;
    Ok(())
}

fn read_optional_bounded_regular_file(
    path: &Path,
    limit: u64,
    code_prefix: &str,
) -> Result<Option<Vec<u8>>, GatewayControlError> {
    if !path.exists() {
        return Ok(None);
    }
    read_bounded_regular_file(path, limit, code_prefix).map(Some)
}

fn read_bounded_regular_file(
    path: &Path,
    limit: u64,
    code_prefix: &str,
) -> Result<Vec<u8>, GatewayControlError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        config_error(
            format!("{code_prefix}_metadata_failed"),
            format!("Failed to inspect regular file metadata: {error}"),
            "Repair the path and retry.",
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
    {
        return Err(config_error(
            format!("{code_prefix}_not_regular_file"),
            "Path is not a regular non-symlink file.",
            "Use a regular file owned by the current installation transaction.",
        ));
    }
    if metadata.len() > limit {
        return Err(config_error(
            format!("{code_prefix}_oversize"),
            "File exceeds the bounded migration size limit.",
            "Inspect the file before retrying.",
        ));
    }
    fs::read(path).map_err(|error| {
        config_error(
            format!("{code_prefix}_read_failed"),
            format!("Failed to read regular file: {error}"),
            "Repair file permissions and retry.",
        )
    })
}

fn ensure_no_reparse_ancestors(path: &Path) -> Result<(), GatewayControlError> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate.exists() {
            let metadata = fs::symlink_metadata(candidate).map_err(|error| {
                config_error(
                    "gateway.stable_install.ancestor_metadata_failed",
                    format!("Failed to inspect installation path ancestor: {error}"),
                    "Repair the installation path and retry.",
                )
            })?;
            if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
                return Err(config_error(
                    "gateway.stable_install.reparse_path_rejected",
                    "Stable MCP installation paths cannot traverse a symlink or reparse point.",
                    "Use the regular current-user installation and artifact directories.",
                ));
            }
        }
        current = candidate.parent();
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn restore_installed_binary(
    installed_path: &Path,
    previous: Option<&[u8]>,
    binary_changed: bool,
) -> Result<(), GatewayControlError> {
    if !binary_changed {
        return Ok(());
    }
    match previous {
        Some(bytes) => write_atomic(installed_path, bytes),
        None => {
            if installed_path.exists() {
                fs::remove_file(installed_path).map_err(|error| {
                    config_error(
                        "gateway.stable_install.rollback_remove_failed",
                        format!("Failed to remove newly installed MCP binary: {error}"),
                        "Preserve the config backup and remove the binary manually.",
                    )
                })?;
            }
            Ok(())
        }
    }
}

pub fn install_codex_mcp_config(
    config_path: &Path,
    mcp_command: &Path,
    artifact_root: &Path,
) -> Result<CodexConfigInstallReceipt, GatewayControlError> {
    write_codex_mcp_config(config_path, None, mcp_command, artifact_root)
}

pub fn replace_codex_mcp_config(
    config_path: &Path,
    expected_current_command: &Path,
    mcp_command: &Path,
    artifact_root: &Path,
) -> Result<CodexConfigInstallReceipt, GatewayControlError> {
    if !expected_current_command.is_absolute() {
        return Err(config_error(
            "gateway.codex_config.expected_command_invalid",
            "Expected current Codex MCP command must be an absolute path.",
            "Pass the exact frozen MCP command path recorded by the current config.",
        ));
    }
    write_codex_mcp_config(
        config_path,
        Some(expected_current_command),
        mcp_command,
        artifact_root,
    )
}

fn write_codex_mcp_config(
    config_path: &Path,
    expected_current_command: Option<&Path>,
    mcp_command: &Path,
    artifact_root: &Path,
) -> Result<CodexConfigInstallReceipt, GatewayControlError> {
    if !mcp_command.is_absolute() || !mcp_command.is_file() {
        return Err(config_error(
            "gateway.codex_config.command_invalid",
            "Codex MCP command must be an existing absolute executable path.",
            "Build the frozen ai_engine_gateway_mcp binary and retry with its absolute path.",
        ));
    }
    fs::create_dir_all(artifact_root).map_err(|error| {
        config_error(
            "gateway.codex_config.artifact_root_create_failed",
            format!("Failed to create private Codex config artifact root: {error}"),
            "Repair current-user local application data permissions.",
        )
    })?;
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            config_error(
                "gateway.codex_config.parent_create_failed",
                format!("Failed to create Codex config directory: {error}"),
                "Repair the current user's Codex config directory permissions.",
            )
        })?;
    }

    let before = read_bounded_config(config_path)?;
    let before_digest = sha256_prefixed(&before);
    let mut document = if before.is_empty() {
        DocumentMut::new()
    } else {
        std::str::from_utf8(&before)
            .map_err(|_| {
                config_error(
                    "gateway.codex_config.not_utf8",
                    "Codex config is not valid UTF-8.",
                    "Repair the config before installing the MCP entry.",
                )
            })?
            .parse::<DocumentMut>()
            .map_err(|error| {
                config_error(
                    "gateway.codex_config.toml_invalid",
                    format!("Codex config is invalid TOML: {error}"),
                    "Repair the config before installing the MCP entry.",
                )
            })?
    };
    let command = mcp_command.display().to_string();
    let changed = match expected_current_command {
        Some(expected) => {
            replace_server_entry(&mut document, &expected.display().to_string(), &command)?
        }
        None => install_server_entry(&mut document, &command)?,
    };
    let after = document.to_string().into_bytes();
    let after_digest = sha256_prefixed(&after);
    let stamp = now_epoch_ms();
    let fragment_path = artifact_root.join(format!("{stamp}-ai-first-game-engine-mcp.toml"));
    write_atomic(&fragment_path, config_fragment(&command).as_bytes())?;

    let backup_path = if changed && config_path.exists() {
        let path = artifact_root.join(format!("{stamp}-config-before.toml"));
        write_atomic(&path, &before)?;
        Some(path)
    } else {
        None
    };
    if changed {
        write_atomic(config_path, &after)?;
    }
    Ok(CodexConfigInstallReceipt {
        schema_version: CODEX_CONFIG_INSTALL_RECEIPT_SCHEMA_VERSION.to_string(),
        config_path: config_path.to_path_buf(),
        backup_path,
        fragment_path,
        before_digest,
        after_digest,
        command_digest: sha256_prefixed(command.as_bytes()),
        changed,
        reload_or_new_task_required: true,
    })
}

pub fn persist_codex_config_install_receipt(
    receipt: &CodexConfigInstallReceipt,
    artifact_root: &Path,
) -> Result<PathBuf, GatewayControlError> {
    let path = artifact_root.join(format!("{}-install-receipt.json", now_epoch_ms()));
    let bytes = serde_json::to_vec_pretty(receipt).map_err(|error| {
        config_error(
            "gateway.codex_config.receipt_serialize_failed",
            format!("Failed to serialize Codex config install receipt: {error}"),
            "Do not accept the install without a durable receipt.",
        )
    })?;
    write_atomic(&path, &bytes)?;
    Ok(path)
}

pub fn rollback_codex_mcp_config(
    receipt: &CodexConfigInstallReceipt,
) -> Result<CodexConfigRollbackReceipt, GatewayControlError> {
    if receipt.schema_version != CODEX_CONFIG_INSTALL_RECEIPT_SCHEMA_VERSION {
        return Err(config_error(
            "gateway.codex_config.receipt_schema_unsupported",
            "Codex config install receipt uses an unsupported schema.",
            "Use the receipt created by this installer version.",
        ));
    }
    let current = read_bounded_config(&receipt.config_path)?;
    if sha256_prefixed(&current) != receipt.after_digest {
        return Err(config_error(
            "gateway.codex_config.rollback_drifted",
            "Codex config changed after installation; automatic rollback is unsafe.",
            "Review the current config and merge the rollback manually.",
        ));
    }
    if !receipt.changed {
        return Ok(CodexConfigRollbackReceipt {
            schema_version: CODEX_CONFIG_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
            config_path: receipt.config_path.clone(),
            restored_digest: receipt.before_digest.clone(),
            removed_new_config: false,
        });
    }
    let removed_new_config = match &receipt.backup_path {
        Some(path) => {
            let backup = read_bounded_config(path)?;
            if sha256_prefixed(&backup) != receipt.before_digest {
                return Err(config_error(
                    "gateway.codex_config.backup_tampered",
                    "Codex config backup digest does not match the install receipt.",
                    "Do not overwrite the current config; inspect the backup manually.",
                ));
            }
            write_atomic(&receipt.config_path, &backup)?;
            false
        }
        None if receipt.before_digest == sha256_prefixed(&[]) => {
            if receipt.config_path.exists() {
                fs::remove_file(&receipt.config_path).map_err(|error| {
                    config_error(
                        "gateway.codex_config.rollback_remove_failed",
                        format!("Failed to remove newly created Codex config: {error}"),
                        "Remove the file manually after verifying its digest.",
                    )
                })?;
            }
            true
        }
        None => {
            return Err(config_error(
                "gateway.codex_config.rollback_backup_missing",
                "Rollback requires a backup but the install receipt does not reference one.",
                "Do not modify the config automatically; inspect the install artifacts.",
            ));
        }
    };
    Ok(CodexConfigRollbackReceipt {
        schema_version: CODEX_CONFIG_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
        config_path: receipt.config_path.clone(),
        restored_digest: receipt.before_digest.clone(),
        removed_new_config,
    })
}

fn install_server_entry(
    document: &mut DocumentMut,
    command: &str,
) -> Result<bool, GatewayControlError> {
    if document.get("mcp_servers").is_none() {
        document["mcp_servers"] = Item::Table(Table::new());
    }
    let servers = document["mcp_servers"].as_table_mut().ok_or_else(|| {
        config_error(
            "gateway.codex_config.mcp_servers_not_table",
            "Codex mcp_servers entry is not a TOML table.",
            "Repair the existing config before installing this MCP server.",
        )
    })?;
    if let Some(existing) = servers.get(CODEX_MCP_SERVER_ID) {
        let existing_command = existing
            .as_table()
            .and_then(|table| table.get("command"))
            .and_then(Item::as_str);
        return match existing_command {
            Some(value) if value == command => Ok(false),
            _ => Err(config_error(
                "gateway.codex_config.server_conflict",
                "Codex already contains a different ai_first_game_engine MCP entry.",
                "Review or remove the conflicting entry before retrying installation.",
            )),
        };
    }
    let mut server = Table::new();
    server["command"] = value(command);
    servers[CODEX_MCP_SERVER_ID] = Item::Table(server);
    Ok(true)
}

fn replace_server_entry(
    document: &mut DocumentMut,
    expected_current_command: &str,
    command: &str,
) -> Result<bool, GatewayControlError> {
    let existing_command = document
        .get("mcp_servers")
        .and_then(Item::as_table)
        .and_then(|servers| servers.get(CODEX_MCP_SERVER_ID))
        .and_then(Item::as_table)
        .and_then(|server| server.get("command"))
        .and_then(Item::as_str);
    if existing_command != Some(expected_current_command) {
        return Err(config_error(
            "gateway.codex_config.server_replace_precondition_failed",
            "Codex ai_first_game_engine MCP entry does not match the expected current command.",
            "Reload the current config and retry with its exact frozen MCP command path.",
        ));
    }
    if expected_current_command == command {
        return Ok(false);
    }
    document["mcp_servers"][CODEX_MCP_SERVER_ID]["command"] = value(command);
    Ok(true)
}

fn config_fragment(command: &str) -> String {
    let mut document = DocumentMut::new();
    let mut servers = Table::new();
    let mut server = Table::new();
    server["command"] = value(command);
    servers[CODEX_MCP_SERVER_ID] = Item::Table(server);
    document["mcp_servers"] = Item::Table(servers);
    document.to_string()
}

fn read_bounded_config(path: &Path) -> Result<Vec<u8>, GatewayControlError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        config_error(
            "gateway.codex_config.metadata_failed",
            format!("Failed to inspect Codex config metadata: {error}"),
            "Repair the config path and retry.",
        )
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(config_error(
            "gateway.codex_config.not_regular_file",
            "Codex config path is not a regular file.",
            "Use the regular current-user config.toml file.",
        ));
    }
    if metadata.len() > MAX_CODEX_CONFIG_BYTES {
        return Err(config_error(
            "gateway.codex_config.oversize",
            "Codex config exceeds the bounded installer size limit.",
            "Review the config manually before installing the MCP entry.",
        ));
    }
    fs::read(path).map_err(|error| {
        config_error(
            "gateway.codex_config.read_failed",
            format!("Failed to read Codex config: {error}"),
            "Repair the current user's Codex config permissions.",
        )
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), GatewayControlError> {
    let parent = path.parent().ok_or_else(|| {
        config_error(
            "gateway.codex_config.parent_missing",
            "Codex config artifact path has no parent directory.",
            "Use an absolute config or artifact path.",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        config_error(
            "gateway.codex_config.write_parent_failed",
            format!("Failed to create config artifact parent: {error}"),
            "Repair current-user directory permissions.",
        )
    })?;
    let temp = path.with_extension(format!("tmp-{}-{}", std::process::id(), now_epoch_ms()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| {
            config_error(
                "gateway.codex_config.temp_create_failed",
                format!("Failed to create atomic config temp file: {error}"),
                "Remove stale temp files and retry.",
            )
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(config_error(
            "gateway.codex_config.temp_write_failed",
            format!("Failed to persist atomic config temp file: {error}"),
            "Repair storage permissions and retry.",
        ));
    }
    drop(file);
    atomic_replace(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        config_error(
            "gateway.codex_config.atomic_replace_failed",
            format!("Failed to atomically replace the config artifact: {error}"),
            "Close processes locking the file and retry.",
        )
    })
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH, REPLACEFILE_WRITE_THROUGH,
    };

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    };
    let temp_wide = wide(temp);
    let destination_wide = wide(destination);
    let replaced = if destination.exists() {
        unsafe {
            ReplaceFileW(
                destination_wide.as_ptr(),
                temp_wide.as_ptr(),
                null(),
                REPLACEFILE_WRITE_THROUGH,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        }
    } else {
        unsafe {
            MoveFileExW(
                temp_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(temp, destination)
}

fn config_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> GatewayControlError {
    GatewayControlError {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_config_artifact_preserves_existing_settings_and_rolls_back_exactly() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-codex-config-{}-{}",
            std::process::id(),
            now_epoch_ms()
        ));
        let config = root.join(".codex/config.toml");
        let artifacts = root.join("artifacts");
        let command = root.join("ai_engine_gateway_mcp.exe");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&command, b"fixture").unwrap();
        let before = b"model = \"existing-model\"\napi_key = \"keep-private\"\n\n[mcp_servers.existing]\nurl = \"https://example.invalid/mcp\"\n";
        fs::write(&config, before).unwrap();

        let receipt = install_codex_mcp_config(&config, &command, &artifacts).unwrap();
        assert!(receipt.changed);
        let installed = fs::read_to_string(&config).unwrap();
        assert!(installed.contains("existing-model"));
        assert!(installed.contains("keep-private"));
        assert!(installed.contains("mcp_servers.existing"));
        assert!(installed.contains("mcp_servers.ai_first_game_engine"));
        assert!(!serde_json::to_string(&receipt)
            .unwrap()
            .contains("keep-private"));

        let repeated = install_codex_mcp_config(&config, &command, &artifacts).unwrap();
        assert!(!repeated.changed);
        let rollback = rollback_codex_mcp_config(&receipt).unwrap();
        assert!(!rollback.removed_new_config);
        assert_eq!(fs::read(&config).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn codex_config_artifact_rejects_conflict_and_rollback_drift() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-codex-config-negative-{}-{}",
            std::process::id(),
            now_epoch_ms()
        ));
        let config = root.join("config.toml");
        let artifacts = root.join("artifacts");
        let command = root.join("ai_engine_gateway_mcp.exe");
        let other = root.join("other.exe");
        fs::create_dir_all(&root).unwrap();
        fs::write(&command, b"fixture").unwrap();
        fs::write(&other, b"fixture").unwrap();
        fs::write(
            &config,
            format!(
                "[mcp_servers.ai_first_game_engine]\ncommand = {:?}\n",
                other.display().to_string()
            ),
        )
        .unwrap();
        let error = install_codex_mcp_config(&config, &command, &artifacts).unwrap_err();
        assert_eq!(error.code, "gateway.codex_config.server_conflict");

        fs::write(&config, "model = \"before\"\n").unwrap();
        let receipt = install_codex_mcp_config(&config, &command, &artifacts).unwrap();
        fs::write(&config, "model = \"user-changed-after-install\"\n").unwrap();
        let error = rollback_codex_mcp_config(&receipt).unwrap_err();
        assert_eq!(error.code, "gateway.codex_config.rollback_drifted");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn codex_config_replace_requires_exact_current_command_and_rolls_back() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-codex-config-replace-{}-{}",
            std::process::id(),
            now_epoch_ms()
        ));
        let config = root.join("config.toml");
        let artifacts = root.join("artifacts");
        let old_command = root.join("old/ai_engine_gateway_mcp.exe");
        let wrong_command = root.join("wrong/ai_engine_gateway_mcp.exe");
        let new_command = root.join("new/ai_engine_gateway_mcp.exe");
        fs::create_dir_all(new_command.parent().unwrap()).unwrap();
        fs::write(&new_command, b"fixture").unwrap();
        let before = format!(
            "model = \"keep-model\"\n\n[mcp_servers.ai_first_game_engine]\ncommand = {:?}\n",
            old_command.display().to_string()
        );
        fs::write(&config, before.as_bytes()).unwrap();

        let error = replace_codex_mcp_config(&config, &wrong_command, &new_command, &artifacts)
            .unwrap_err();
        assert_eq!(
            error.code,
            "gateway.codex_config.server_replace_precondition_failed"
        );
        assert_eq!(fs::read_to_string(&config).unwrap(), before);

        let receipt =
            replace_codex_mcp_config(&config, &old_command, &new_command, &artifacts).unwrap();
        assert!(receipt.changed);
        let installed = fs::read_to_string(&config).unwrap();
        assert!(installed.contains("keep-model"));
        assert!(installed.contains(&new_command.display().to_string()));
        assert!(!installed.contains(&old_command.display().to_string()));

        let rollback = rollback_codex_mcp_config(&receipt).unwrap();
        assert!(!rollback.removed_new_config);
        assert_eq!(fs::read_to_string(&config).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }
}
