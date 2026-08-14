use crate::{GatewayControlError, GATEWAY_PROTOCOL_VERSION};
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const GATEWAY_DISCOVERY_SCHEMA_VERSION: &str = "ai-tool-gateway-discovery.v2";
pub const MAX_GATEWAY_DISCOVERY_RECORDS: usize = 64;
pub const MAX_GATEWAY_DISCOVERY_RECORD_BYTES: u64 = 64 * 1024;
static LOCATOR_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn new_editor_instance_id() -> String {
    let sequence = LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let epoch_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    sha256_prefixed(format!("{}|{}|{}", std::process::id(), epoch_nanos, sequence).as_bytes())
}

pub fn default_editor_instance_id() -> String {
    sha256_prefixed(format!("editor-process-{}", std::process::id()).as_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayDiscoveryRecord {
    pub schema_version: String,
    pub gateway_protocol_version: String,
    pub editor_instance_id: String,
    pub editor_process_id: u32,
    pub pipe_locator: String,
    pub published_at_epoch_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyGatewayDiscoveryRecord {
    schema_version: String,
    gateway_protocol_version: String,
    editor_process_id: u32,
    project_identity: String,
    canonical_project_root_digest: String,
    pipe_locator: String,
    published_at_epoch_ms: u64,
}

enum DiscoveryCandidate {
    ActiveCompatible(GatewayDiscoveryRecord),
    ActiveIncompatible { schema_version: String },
    DeadStale,
}

impl GatewayDiscoveryRecord {
    pub fn new(editor_instance_id: impl Into<String>) -> Self {
        let editor_instance_id = editor_instance_id.into();
        let now = now_epoch_ms();
        let suffix = sha256_prefixed(editor_instance_id.as_bytes())
            .trim_start_matches("sha256:")
            .chars()
            .take(32)
            .collect::<String>();
        Self {
            schema_version: GATEWAY_DISCOVERY_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            editor_instance_id,
            editor_process_id: std::process::id(),
            pipe_locator: format!(r"\\.\pipe\ai-first-game-engine\{suffix}"),
            published_at_epoch_ms: now,
        }
    }

    pub fn validate(&self) -> Result<(), GatewayControlError> {
        if self.schema_version != GATEWAY_DISCOVERY_SCHEMA_VERSION
            || self.gateway_protocol_version != GATEWAY_PROTOCOL_VERSION
            || self.editor_instance_id.trim().is_empty()
            || self.editor_process_id == 0
            || !self
                .pipe_locator
                .starts_with(r"\\.\pipe\ai-first-game-engine\")
        {
            return Err(discovery_error(
                "gateway.discovery.invalid",
                "Gateway discovery record is incomplete or uses an unsupported schema.",
                "Discard the record and let the active Editor republish it.",
            ));
        }
        Ok(())
    }
}

pub fn default_discovery_root() -> Result<PathBuf, GatewayControlError> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        discovery_error(
            "gateway.discovery.local_app_data_unavailable",
            "LOCALAPPDATA is unavailable for per-user Gateway discovery.",
            "Run the Editor in a normal interactive user session.",
        )
    })?;
    Ok(PathBuf::from(local)
        .join("AiFirstGameEngine")
        .join("Gateway")
        .join("discovery"))
}

pub fn discovery_record_path(root: &Path, editor_instance_id: &str) -> PathBuf {
    let name = sha256_prefixed(editor_instance_id.as_bytes())
        .trim_start_matches("sha256:")
        .chars()
        .take(64)
        .collect::<String>();
    root.join(format!("{name}.json"))
}

pub fn resolve_gateway_discovery_path(
    explicit_path: Option<&Path>,
    expected_editor_instance_id: Option<&str>,
) -> Result<PathBuf, GatewayControlError> {
    if let Some(path) = explicit_path {
        let record = read_discovery_record(path)?;
        validate_resolved_record(path, &record, expected_editor_instance_id, false, true)?;
        return Ok(path.to_path_buf());
    }

    let root = default_discovery_root()?;
    resolve_gateway_discovery_path_in_root(&root, expected_editor_instance_id)
}

pub fn resolve_gateway_discovery_path_in_root(
    root: &Path,
    expected_editor_instance_id: Option<&str>,
) -> Result<PathBuf, GatewayControlError> {
    validate_discovery_root(root)?;
    let entries = fs::read_dir(root).map_err(|error| {
        discovery_error(
            "gateway.discovery.root_read_failed",
            format!("Failed to read the per-user Gateway discovery root: {error}"),
            "Open the target project in the Editor and retry.",
        )
    })?;
    let mut json_paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            discovery_error(
                "gateway.discovery.entry_read_failed",
                format!("Failed to enumerate a Gateway discovery entry: {error}"),
                "Repair the current user's discovery directory and retry.",
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            discovery_error(
                "gateway.discovery.entry_type_failed",
                format!("Failed to inspect a Gateway discovery entry: {error}"),
                "Repair the current user's discovery directory and retry.",
            )
        })?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(discovery_error(
                "gateway.discovery.entry_not_regular_file",
                "Gateway discovery contains a JSON entry that is not a regular file.",
                "Remove the invalid entry and let the active Editor republish it.",
            ));
        }
        json_paths.push(path);
        if json_paths.len() > MAX_GATEWAY_DISCOVERY_RECORDS {
            return Err(discovery_error(
                "gateway.discovery.record_limit_exceeded",
                "Gateway discovery contains more records than the bounded resolver permits.",
                "Close stale Editor instances and remove stale discovery records.",
            ));
        }
    }
    json_paths.sort();

    let mut active_matches = Vec::new();
    let mut active_incompatible = Vec::new();
    for path in json_paths {
        match classify_discovery_candidate(&path)? {
            DiscoveryCandidate::ActiveCompatible(record) => {
                if expected_editor_instance_id
                    .map(|expected| expected != record.editor_instance_id)
                    .unwrap_or(false)
                {
                    continue;
                }
                active_matches.push(path);
            }
            DiscoveryCandidate::ActiveIncompatible { schema_version } => {
                active_incompatible.push(schema_version);
            }
            DiscoveryCandidate::DeadStale => {
                let _ = fs::remove_file(path);
            }
        }
    }

    match active_matches.as_slice() {
        [path] => Ok(path.clone()),
        [] if !active_incompatible.is_empty() => {
            active_incompatible.sort();
            active_incompatible.dedup();
            Err(discovery_error(
                "gateway.discovery.active_incompatible",
                format!(
                    "Only incompatible live Editor Gateway records are active: {}.",
                    active_incompatible.join(", ")
                ),
                "Upgrade or close the incompatible Editor, then open one matching this Gateway version.",
            ))
        }
        [] => Err(discovery_error(
            "gateway.discovery.no_active_match",
            "No active Editor Gateway discovery record matched the requested Editor instance.",
            "Open the target Editor, or provide its exact editorInstanceId.",
        )),
        _ => Err(discovery_error(
            "gateway.discovery.ambiguous_editor_instance",
            "More than one active Editor Gateway matches this discovery request.",
            "Provide the exact editorInstanceId selector.",
        )),
    }
}

fn validate_discovery_root(root: &Path) -> Result<(), GatewayControlError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        discovery_error(
            "gateway.discovery.root_metadata_failed",
            format!("Failed to inspect the per-user Gateway discovery root: {error}"),
            "Open the target Editor and retry from a regular current-user discovery directory.",
        )
    })?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
    {
        return Err(discovery_error(
            "gateway.discovery.root_not_regular_directory",
            "Gateway discovery root is not a regular non-reparse directory.",
            "Use the engine-owned current-user discovery directory.",
        ));
    }
    Ok(())
}

fn classify_discovery_candidate(path: &Path) -> Result<DiscoveryCandidate, GatewayControlError> {
    let value = read_bounded_discovery_value(path)?;
    let schema_version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            discovery_error(
                "gateway.discovery.schema_missing",
                "Gateway discovery record has no schemaVersion.",
                "Remove the invalid record and let the active Editor republish it.",
            )
        })?;
    match schema_version {
        GATEWAY_DISCOVERY_SCHEMA_VERSION => {
            let record: GatewayDiscoveryRecord =
                serde_json::from_value(value).map_err(|error| {
                    discovery_error(
                        "gateway.discovery.record_parse_failed",
                        format!("Gateway discovery record is invalid JSON: {error}"),
                        "Discard the record and let the active Editor republish it.",
                    )
                })?;
            record.validate()?;
            validate_resolved_record(path, &record, None, true, false)?;
            if process_is_alive(record.editor_process_id) {
                Ok(DiscoveryCandidate::ActiveCompatible(record))
            } else {
                Ok(DiscoveryCandidate::DeadStale)
            }
        }
        "ai-tool-gateway-discovery.v1" => {
            let record: LegacyGatewayDiscoveryRecord =
                serde_json::from_value(value).map_err(|error| {
                    discovery_error(
                        "gateway.discovery.legacy_record_invalid",
                        format!("Legacy Gateway discovery record is invalid: {error}"),
                        "Remove the invalid record and let the active Editor republish it.",
                    )
                })?;
            validate_legacy_record(path, &record)?;
            if process_is_alive(record.editor_process_id) {
                Ok(DiscoveryCandidate::ActiveIncompatible {
                    schema_version: record.schema_version,
                })
            } else {
                Ok(DiscoveryCandidate::DeadStale)
            }
        }
        other => Err(discovery_error(
            "gateway.discovery.schema_unsupported",
            format!(
                "Gateway discovery schema {other} is unsupported; expected {GATEWAY_DISCOVERY_SCHEMA_VERSION}."
            ),
            "Remove the unknown record or use an Editor matching this Gateway version.",
        )),
    }
}

fn validate_legacy_record(
    path: &Path,
    record: &LegacyGatewayDiscoveryRecord,
) -> Result<(), GatewayControlError> {
    let digest = record
        .canonical_project_root_digest
        .strip_prefix("sha256:")
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| {
            discovery_error(
                "gateway.discovery.legacy_project_digest_invalid",
                "Legacy Gateway discovery project digest is invalid.",
                "Remove the invalid legacy record.",
            )
        })?;
    if path.file_name().and_then(|value| value.to_str()) != Some(&format!("{digest}.json")) {
        return Err(discovery_error(
            "gateway.discovery.legacy_filename_digest_mismatch",
            "Legacy Gateway discovery filename does not match its project root digest.",
            "Remove the unowned legacy record.",
        ));
    }
    if record.schema_version != "ai-tool-gateway-discovery.v1"
        || record.gateway_protocol_version != "ai-tool-gateway.v1"
        || record.editor_process_id == 0
        || record.project_identity.trim().is_empty()
        || !record
            .pipe_locator
            .starts_with(r"\\.\pipe\ai-first-game-engine\")
        || record.published_at_epoch_ms == 0
    {
        return Err(discovery_error(
            "gateway.discovery.legacy_record_invalid",
            "Legacy Gateway discovery record is incomplete or inconsistent.",
            "Remove the invalid legacy record.",
        ));
    }
    Ok(())
}

fn read_bounded_discovery_value(path: &Path) -> Result<serde_json::Value, GatewayControlError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        discovery_error(
            "gateway.discovery.metadata_failed",
            format!("Failed to inspect Gateway discovery record metadata: {error}"),
            "Discard stale discovery data and let the active Editor republish it.",
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
    {
        return Err(discovery_error(
            "gateway.discovery.record_not_regular_file",
            "Gateway discovery record is not a regular non-reparse file.",
            "Use a regular per-user discovery JSON file.",
        ));
    }
    if metadata.len() > MAX_GATEWAY_DISCOVERY_RECORD_BYTES {
        return Err(discovery_error(
            "gateway.discovery.record_oversize",
            "Gateway discovery record exceeds the bounded resolver size limit.",
            "Discard the record and let the active Editor republish it.",
        ));
    }
    let bytes = fs::read(path).map_err(|error| {
        discovery_error(
            "gateway.discovery.record_read_failed",
            format!("Failed to read Gateway discovery record: {error}"),
            "Open the target project in the Editor and retry.",
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        discovery_error(
            "gateway.discovery.record_parse_failed",
            format!("Gateway discovery record is invalid JSON: {error}"),
            "Discard the record and let the active Editor republish it.",
        )
    })
}

fn read_discovery_record(path: &Path) -> Result<GatewayDiscoveryRecord, GatewayControlError> {
    let value = read_bounded_discovery_value(path)?;
    if let Some(schema_version) = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
    {
        if schema_version != GATEWAY_DISCOVERY_SCHEMA_VERSION {
            return Err(discovery_error(
                "gateway.discovery.schema_unsupported",
                format!(
                    "Gateway discovery schema {schema_version} is unsupported; expected {GATEWAY_DISCOVERY_SCHEMA_VERSION}."
                ),
                "Discard the legacy record and let the active Editor republish it.",
            ));
        }
    }
    let record: GatewayDiscoveryRecord = serde_json::from_value(value).map_err(|error| {
        discovery_error(
            "gateway.discovery.record_parse_failed",
            format!("Gateway discovery record is invalid JSON: {error}"),
            "Discard the record and let the active Editor republish it.",
        )
    })?;
    record.validate()?;
    Ok(record)
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

fn validate_resolved_record(
    path: &Path,
    record: &GatewayDiscoveryRecord,
    expected_editor_instance_id: Option<&str>,
    require_digest_filename: bool,
    require_alive: bool,
) -> Result<(), GatewayControlError> {
    if require_digest_filename {
        let expected_name = discovery_record_path(Path::new(""), &record.editor_instance_id);
        if path.file_name() != expected_name.file_name() {
            return Err(discovery_error(
                "gateway.discovery.filename_digest_mismatch",
                "Gateway discovery filename does not match its editorInstanceId.",
                "Discard the record and let the active Editor republish it.",
            ));
        }
    }
    if expected_editor_instance_id
        .map(|expected| expected != record.editor_instance_id)
        .unwrap_or(false)
    {
        return Err(discovery_error(
            "gateway.discovery.editor_instance_selector_mismatch",
            "Gateway discovery record does not match the requested editorInstanceId.",
            "Select the exact active Editor instance or remove the stale selector.",
        ));
    }
    if require_alive && !process_is_alive(record.editor_process_id) {
        return Err(discovery_error(
            "gateway.discovery.editor_process_stale",
            "Gateway discovery record belongs to an Editor process that is no longer running.",
            "Open the project in the Editor and use the newly published record.",
        ));
    }
    Ok(())
}

pub struct GatewayDiscoveryPublication {
    path: PathBuf,
}

impl GatewayDiscoveryPublication {
    pub fn publish(
        root: &Path,
        record: &GatewayDiscoveryRecord,
    ) -> Result<Self, GatewayControlError> {
        record.validate()?;
        fs::create_dir_all(root).map_err(|error| {
            discovery_error(
                "gateway.discovery.create_root_failed",
                format!("Failed to create per-user discovery root: {error}"),
                "Repair the current user's local application data permissions.",
            )
        })?;
        validate_discovery_root(root)?;
        let path = discovery_record_path(root, &record.editor_instance_id);
        if path.exists() {
            let previous = read_discovery_record(&path)?;
            if process_is_alive(previous.editor_process_id) {
                return Err(discovery_error(
                    "gateway.discovery.editor_instance_already_published",
                    "Another live Editor already publishes this editorInstanceId.",
                    "Use a fresh engine-owned editorInstanceId.",
                ));
            }
            fs::remove_file(&path).map_err(|error| {
                discovery_error(
                    "gateway.discovery.stale_remove_failed",
                    format!("Failed to remove a stale discovery record: {error}"),
                    "Repair the current user's discovery directory and retry.",
                )
            })?;
        }
        let temp = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let bytes = serde_json::to_vec_pretty(record).map_err(|error| {
            discovery_error(
                "gateway.discovery.serialize_failed",
                format!("Failed to serialize discovery record: {error}"),
                "Regenerate the discovery record from the active Editor project.",
            )
        })?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|error| {
                discovery_error(
                    "gateway.discovery.temp_create_failed",
                    format!("Failed to create discovery temp file: {error}"),
                    "Remove stale temp files and retry from the same user account.",
                )
            })?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| {
                discovery_error(
                    "gateway.discovery.write_failed",
                    format!("Failed to persist discovery record: {error}"),
                    "Repair local application data storage and republish.",
                )
            })?;
        fs::rename(&temp, &path).map_err(|error| {
            let _ = fs::remove_file(&temp);
            discovery_error(
                "gateway.discovery.publish_failed",
                format!("Failed to atomically publish discovery record: {error}"),
                "Close stale Editor instances and retry.",
            )
        })?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for GatewayDiscoveryPublication {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn discovery_error(
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

#[cfg(windows)]
fn process_is_alive(process_id: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const STILL_ACTIVE_EXIT_CODE: u32 = 259;
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if handle.is_null() {
        return false;
    }
    let mut exit_code = 0;
    let queried = unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0;
    unsafe {
        CloseHandle(handle);
    }
    queried && exit_code == STILL_ACTIVE_EXIT_CODE
}

#[cfg(not(windows))]
fn process_is_alive(process_id: u32) -> bool {
    process_id == std::process::id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_discovery_is_per_user_bounded_and_removed_on_drop() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-discovery-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let record = GatewayDiscoveryRecord::new("editor-instance-test");
        assert!(record
            .pipe_locator
            .starts_with(r"\\.\pipe\ai-first-game-engine\"));
        assert!(!record.pipe_locator.contains("editor-instance-test"));
        let publication = GatewayDiscoveryPublication::publish(&root, &record).unwrap();
        let path = publication.path().to_path_buf();
        assert!(path.starts_with(&root));
        let decoded: GatewayDiscoveryRecord =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(decoded, record);
        drop(publication);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_discovery_rejects_explicit_stale_process() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-discovery-stale-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut record = GatewayDiscoveryRecord::new("editor-instance-stale");
        record.editor_process_id = u32::MAX;
        let path = discovery_record_path(&root, &record.editor_instance_id);
        std::fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();

        let error = resolve_gateway_discovery_path(Some(&path), None).unwrap_err();

        assert_eq!(error.code, "gateway.discovery.editor_process_stale");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_discovery_rejects_invalid_filename_but_ignores_stale_process() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-discovery-root-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut stale = GatewayDiscoveryRecord::new("editor-instance-stale");
        stale.editor_process_id = u32::MAX;
        let stale_path = discovery_record_path(&root, &stale.editor_instance_id);
        std::fs::write(&stale_path, serde_json::to_vec_pretty(&stale).unwrap()).unwrap();
        let active = GatewayDiscoveryRecord::new("editor-instance-active");
        let publication = GatewayDiscoveryPublication::publish(&root, &active).unwrap();

        assert_eq!(
            resolve_gateway_discovery_path_in_root(&root, None).unwrap(),
            publication.path()
        );

        let invalid_path = root.join("wrong-name.json");
        std::fs::write(&invalid_path, serde_json::to_vec_pretty(&active).unwrap()).unwrap();
        let error = resolve_gateway_discovery_path_in_root(&root, None).unwrap_err();
        assert_eq!(error.code, "gateway.discovery.filename_digest_mismatch");
        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editor_instance_discovery_is_stable_and_exactly_selectable() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-editor-instance-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let editor_instance_id = "editor-instance-stable";
        let first = GatewayDiscoveryRecord::new(editor_instance_id);
        let second = GatewayDiscoveryRecord::new(editor_instance_id);

        assert_eq!(first.editor_instance_id, editor_instance_id);
        assert_eq!(first.pipe_locator, second.pipe_locator);
        let publication = GatewayDiscoveryPublication::publish(&root, &first).unwrap();
        assert_eq!(
            resolve_gateway_discovery_path_in_root(&root, Some(editor_instance_id)).unwrap(),
            publication.path()
        );

        drop(publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editor_instance_discovery_fails_closed_without_an_exact_selector() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-editor-ambiguity-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let first = GatewayDiscoveryRecord::new("editor-instance-a");
        let second = GatewayDiscoveryRecord::new("editor-instance-b");
        let first_publication = GatewayDiscoveryPublication::publish(&root, &first).unwrap();
        let second_publication = GatewayDiscoveryPublication::publish(&root, &second).unwrap();

        let error = resolve_gateway_discovery_path_in_root(&root, None).unwrap_err();
        assert_eq!(error.code, "gateway.discovery.ambiguous_editor_instance");
        assert_eq!(
            resolve_gateway_discovery_path_in_root(&root, Some("editor-instance-b")).unwrap(),
            second_publication.path()
        );

        drop(first_publication);
        drop(second_publication);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editor_instance_discovery_rejects_legacy_v1_with_typed_diagnostic() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-legacy-discovery-{}-{}",
            std::process::id(),
            LOCATOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("legacy.json");
        std::fs::write(
            &path,
            br#"{
                "schemaVersion":"ai-tool-gateway-discovery.v1",
                "gatewayProtocolVersion":"ai-tool-gateway.v1",
                "editorProcessId":1,
                "projectIdentity":"legacy-project",
                "canonicalProjectRootDigest":"sha256:legacy",
                "pipeLocator":"\\\\.\\pipe\\ai-first-game-engine\\legacy",
                "publishedAtEpochMs":1
            }"#,
        )
        .unwrap();

        let error = resolve_gateway_discovery_path(Some(&path), None).unwrap_err();
        assert_eq!(error.code, "gateway.discovery.schema_unsupported");

        let _ = std::fs::remove_dir_all(root);
    }
}
