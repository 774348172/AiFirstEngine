use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "android", test))]
use std::sync::{Arc, Mutex};

pub const ANDROID_RUNTIME_PACKAGE_ASSET_MANIFEST_SCHEMA_VERSION: &str =
    "android-runtime-package-asset-manifest.v1";
#[cfg(any(target_os = "android", test))]
const ANDROID_USER_RUN_FRAME_LIMIT: u64 = u64::MAX;
pub const ANDROID_STARTUP_DIAGNOSTIC_FILE_NAME: &str = "aife-startup-diagnostic.json";
#[cfg(any(target_os = "android", test))]
const ANDROID_STARTUP_DIAGNOSTIC_SCHEMA_VERSION: &str = "android-startup-diagnostic.v1";
#[cfg(any(target_os = "android", test))]
const ANDROID_STARTUP_DIAGNOSTIC_MAX_STAGES: usize = 24;
#[cfg(any(target_os = "android", test))]
const ANDROID_STARTUP_DIAGNOSTIC_MAX_TEXT_BYTES: usize = 2_048;

#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidStartupDiagnostic {
    schema_version: String,
    outcome: String,
    latest_stage: String,
    stages: Vec<String>,
    storage: String,
    diagnostic_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    panic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_report: Option<serde_json::Value>,
}

#[cfg(any(target_os = "android", test))]
#[derive(Clone)]
struct AndroidStartupDiagnosticRecorder {
    path: Option<PathBuf>,
    state: Arc<Mutex<AndroidStartupDiagnostic>>,
}

#[cfg(any(target_os = "android", test))]
impl AndroidStartupDiagnosticRecorder {
    fn new(path: Option<PathBuf>, storage: impl Into<String>) -> Self {
        let diagnostic_path = path.as_ref().map(|value| value.display().to_string());
        Self {
            path,
            state: Arc::new(Mutex::new(AndroidStartupDiagnostic {
                schema_version: ANDROID_STARTUP_DIAGNOSTIC_SCHEMA_VERSION.to_string(),
                outcome: "starting".to_string(),
                latest_stage: "created".to_string(),
                stages: Vec::new(),
                storage: storage.into(),
                diagnostic_path,
                terminal_error: None,
                panic: None,
                terminal_report: None,
            })),
        }
    }

    fn stage(&self, stage: &str) {
        self.update(|diagnostic| {
            diagnostic.outcome = "running".to_string();
            diagnostic.latest_stage = stage.to_string();
            if diagnostic.stages.len() == ANDROID_STARTUP_DIAGNOSTIC_MAX_STAGES {
                diagnostic.stages.remove(0);
            }
            diagnostic.stages.push(stage.to_string());
        });
    }

    fn error(&self, error: &str) {
        self.update(|diagnostic| {
            diagnostic.outcome = "error".to_string();
            diagnostic.terminal_error = Some(bounded_text(error));
        });
    }

    fn panic(&self, panic: &str) {
        self.update(|diagnostic| {
            diagnostic.outcome = "panic".to_string();
            diagnostic.panic = Some(bounded_text(panic));
        });
    }

    fn terminal_report(&self, report: serde_json::Value) {
        self.update(|diagnostic| {
            diagnostic.outcome = "returned".to_string();
            diagnostic.terminal_report = Some(report);
        });
    }

    fn update(&self, update: impl FnOnce(&mut AndroidStartupDiagnostic)) {
        let snapshot = {
            let mut diagnostic = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            update(&mut diagnostic);
            diagnostic.clone()
        };
        if let Some(path) = &self.path {
            let _ = write_startup_diagnostic(path, &snapshot);
        }
    }

    #[cfg(target_os = "android")]
    fn install_panic_hook(&self) {
        let recorder = self.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = if let Some(message) = info.payload().downcast_ref::<&str>() {
                (*message).to_string()
            } else if let Some(message) = info.payload().downcast_ref::<String>() {
                message.clone()
            } else {
                "non-string panic payload".to_string()
            };
            let message = match info.location() {
                Some(location) => format!(
                    "{} at {}:{}:{}",
                    payload,
                    location.file(),
                    location.line(),
                    location.column()
                ),
                None => payload,
            };
            recorder.panic(&message);
            previous(info);
        }));
    }
}

#[cfg(any(target_os = "android", test))]
fn write_startup_diagnostic(
    path: &Path,
    diagnostic: &AndroidStartupDiagnostic,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut bytes = serde_json::to_vec_pretty(diagnostic).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| error.to_string())
}

#[cfg(any(target_os = "android", test))]
fn bounded_text(value: &str) -> String {
    if value.len() <= ANDROID_STARTUP_DIAGNOSTIC_MAX_TEXT_BYTES {
        return value.to_string();
    }
    let mut end = ANDROID_STARTUP_DIAGNOSTIC_MAX_TEXT_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &value[..end])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidRuntimePackageAssetEntry {
    pub path: String,
    pub sha256: String,
    pub byte_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidRuntimePackageAssetManifest {
    pub schema_version: String,
    pub runtime_package_digest: String,
    pub files: Vec<AndroidRuntimePackageAssetEntry>,
}

impl AndroidRuntimePackageAssetManifest {
    pub fn from_directory(root: &Path) -> Result<Self, String> {
        let mut files = Vec::new();
        collect_files(root, root, &mut files)?;
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let digest_value = serde_json::to_value(&files).map_err(|error| error.to_string())?;
        let digest_input =
            canonical_json_bytes(&digest_value).map_err(|error| error.to_string())?;
        Ok(Self {
            schema_version: ANDROID_RUNTIME_PACKAGE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            runtime_package_digest: sha256_prefixed(&digest_input),
            files,
        })
    }
}

pub fn materialize_runtime_package_directory(
    asset_root: &Path,
    manifest: &AndroidRuntimePackageAssetManifest,
    internal_data_root: &Path,
) -> Result<PathBuf, String> {
    materialize_runtime_package_with_reader(manifest, internal_data_root, |path| {
        fs::read(asset_root.join(checked_relative_path(path)?))
            .map_err(|error| format!("android_runtime_package.asset_read_failed: {error}"))
    })
}

pub fn materialize_runtime_package_with_reader<F>(
    manifest: &AndroidRuntimePackageAssetManifest,
    internal_data_root: &Path,
    mut read_asset: F,
) -> Result<PathBuf, String>
where
    F: FnMut(&str) -> Result<Vec<u8>, String>,
{
    if manifest.schema_version != ANDROID_RUNTIME_PACKAGE_ASSET_MANIFEST_SCHEMA_VERSION {
        return Err("android_runtime_package.asset_manifest_schema_unsupported".to_string());
    }
    let digest = manifest
        .runtime_package_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "android_runtime_package.asset_manifest_digest_invalid".to_string())?;
    let package_parent = internal_data_root.join("aife").join("runtime-packages");
    let destination = package_parent.join(digest);
    if verify_materialized_directory(&destination, manifest).is_ok() {
        return Ok(destination);
    }

    fs::create_dir_all(&package_parent).map_err(|error| error.to_string())?;
    let staging = package_parent.join(format!(".{digest}.staging"));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    for entry in &manifest.files {
        let relative = checked_relative_path(&entry.path)?;
        let bytes = read_asset(&entry.path)?;
        if bytes.len() as u64 != entry.byte_size || sha256_prefixed(&bytes) != entry.sha256 {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!(
                "android_runtime_package.asset_hash_mismatch: {}",
                entry.path
            ));
        }
        let target = staging.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(target, bytes).map_err(|error| error.to_string())?;
    }
    verify_materialized_directory(&staging, manifest)?;
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    fs::rename(&staging, &destination).map_err(|error| error.to_string())?;
    Ok(destination)
}

fn verify_materialized_directory(
    root: &Path,
    manifest: &AndroidRuntimePackageAssetManifest,
) -> Result<(), String> {
    for entry in &manifest.files {
        let path = root.join(checked_relative_path(&entry.path)?);
        let bytes = fs::read(&path).map_err(|_| {
            format!(
                "android_runtime_package.materialized_file_missing: {}",
                entry.path
            )
        })?;
        if bytes.len() as u64 != entry.byte_size || sha256_prefixed(&bytes) != entry.sha256 {
            return Err(format!(
                "android_runtime_package.materialized_hash_mismatch: {}",
                entry.path
            ));
        }
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<AndroidRuntimePackageAssetEntry>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            return Err("android_runtime_package.asset_symlink_forbidden".to_string());
        }
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(AndroidRuntimePackageAssetEntry {
                path: relative,
                sha256: sha256_prefixed(&bytes),
                byte_size: bytes.len() as u64,
            });
        }
    }
    Ok(())
}

fn checked_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("android_runtime_package.asset_path_invalid".to_string());
    }
    Ok(path.to_path_buf())
}

#[cfg(target_os = "android")]
pub fn run_android_app(
    android_app: winit::platform::android::activity::AndroidApp,
    runtime_package_path: PathBuf,
    project_api: project_runtime_abi::ProjectRuntimeApi,
) -> Result<runtime_player_winit::NativeWindowHostReport, String> {
    let recorder = android_startup_diagnostic_recorder(&android_app);
    recorder.install_panic_hook();
    recorder.stage("direct_entry");
    let result =
        run_android_app_with_recorder(android_app, runtime_package_path, project_api, &recorder);
    if let Err(error) = &result {
        recorder.error(error);
    }
    result
}

#[cfg(target_os = "android")]
fn run_android_app_with_recorder(
    android_app: winit::platform::android::activity::AndroidApp,
    runtime_package_path: PathBuf,
    project_api: project_runtime_abi::ProjectRuntimeApi,
    recorder: &AndroidStartupDiagnosticRecorder,
) -> Result<runtime_player_winit::NativeWindowHostReport, String> {
    let linked =
        engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_api(
            project_api,
        )
        .map_err(|error| error.to_string())?;
    recorder.stage("project_runtime_api_linked");
    let mut request =
        runtime_player_winit::NativePlayerWindowRunRequest::windowed(runtime_package_path)
            .with_game_view_target(
                engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_720x1280(),
            );
    request.frame_limit = ANDROID_USER_RUN_FRAME_LIMIT;
    recorder.stage("event_loop_started");
    let report = runtime_player_winit::run_android_native_player_from_package_with_linked_modules(
        android_app,
        request,
        std::sync::Arc::new(linked),
    );
    recorder.stage("event_loop_returned");
    recorder.terminal_report(compact_terminal_report(&report));
    Ok(report)
}

#[cfg(target_os = "android")]
pub fn run_packaged_android_app(
    android_app: winit::platform::android::activity::AndroidApp,
    project_api: project_runtime_abi::ProjectRuntimeApi,
) -> Result<runtime_player_winit::NativeWindowHostReport, String> {
    use std::ffi::CString;
    use std::io::Read;

    let recorder = android_startup_diagnostic_recorder(&android_app);
    recorder.install_panic_hook();
    recorder.stage("entry");
    let result = (|| {
        let asset_manager = android_app.asset_manager();
        let manifest_name = CString::new("aife/runtime-package-asset-manifest.json")
            .map_err(|error| error.to_string())?;
        let mut manifest_asset = asset_manager
            .open(&manifest_name)
            .ok_or_else(|| "android_runtime_package.asset_manifest_missing".to_string())?;
        recorder.stage("asset_manifest_opened");
        let mut manifest_bytes = Vec::new();
        manifest_asset
            .read_to_end(&mut manifest_bytes)
            .map_err(|error| error.to_string())?;
        recorder.stage("asset_manifest_read");
        let manifest: AndroidRuntimePackageAssetManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("android_runtime_package.asset_manifest_invalid: {error}"))?;
        recorder.stage("asset_manifest_parsed");
        let internal_data_root = android_app
            .internal_data_path()
            .ok_or_else(|| "android_runtime_package.internal_data_path_missing".to_string())?;
        recorder.stage("internal_data_path_resolved");
        let runtime_package_path =
            materialize_runtime_package_with_reader(&manifest, &internal_data_root, |relative| {
                let name = CString::new(format!("aife/runtime-package/{relative}"))
                    .map_err(|error| error.to_string())?;
                let mut asset = asset_manager
                    .open(&name)
                    .ok_or_else(|| format!("android_runtime_package.asset_missing: {relative}"))?;
                let mut bytes = Vec::new();
                asset
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
                Ok(bytes)
            })?;
        recorder.stage("runtime_package_materialized");
        run_android_app_with_recorder(android_app, runtime_package_path, project_api, &recorder)
    })();
    if let Err(error) = &result {
        recorder.error(error);
    }
    result
}

#[cfg(target_os = "android")]
fn android_startup_diagnostic_recorder(
    android_app: &winit::platform::android::activity::AndroidApp,
) -> AndroidStartupDiagnosticRecorder {
    if let Some(root) = android_app.external_data_path() {
        return AndroidStartupDiagnosticRecorder::new(
            Some(root.join(ANDROID_STARTUP_DIAGNOSTIC_FILE_NAME)),
            "externalData",
        );
    }
    AndroidStartupDiagnosticRecorder::new(
        android_app
            .internal_data_path()
            .map(|root| root.join(ANDROID_STARTUP_DIAGNOSTIC_FILE_NAME)),
        "internalDataFallback",
    )
}

#[cfg(target_os = "android")]
fn compact_terminal_report(
    report: &runtime_player_winit::NativeWindowHostReport,
) -> serde_json::Value {
    let diagnostics = report
        .diagnostics
        .iter()
        .take(16)
        .map(|diagnostic| {
            serde_json::json!({
                "severity": diagnostic.severity,
                "code": diagnostic.code,
                "layer": diagnostic.layer,
                "message": bounded_text(&diagnostic.message),
                "path": diagnostic.path.as_deref().map(bounded_text),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "framesCompleted": report.frames_completed,
        "exitCode": report.exit_code,
        "packageStatus": report.package_status,
        "sceneStatus": report.scene_status,
        "worldStatus": report.world_status,
        "logicStatus": report.logic_status,
        "renderStatus": report.render_status,
        "rhiStatus": report.rhi_status,
        "inputStatus": report.input_status,
        "windowStatus": report.window_status,
        "surfaceStatus": report.surface_status,
        "presentStatus": report.present_status,
        "diagnosticCount": report.diagnostics.len(),
        "diagnostics": diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn materialization_is_digest_addressed_reusable_and_hash_checked() {
        let root = temp_root("materialize");
        let assets = root.join("assets");
        fs::create_dir_all(assets.join("scenes")).unwrap();
        fs::write(assets.join("manifest.json"), b"manifest").unwrap();
        fs::write(assets.join("scenes/main.json"), b"scene").unwrap();
        let manifest = AndroidRuntimePackageAssetManifest::from_directory(&assets).unwrap();

        let first = materialize_runtime_package_directory(&assets, &manifest, &root).unwrap();
        let second = materialize_runtime_package_directory(&assets, &manifest, &root).unwrap();
        assert_eq!(first, second);
        assert!(first.join("scenes/main.json").is_file());

        fs::write(assets.join("scenes/main.json"), b"tampered").unwrap();
        fs::remove_dir_all(&first).unwrap();
        assert!(
            materialize_runtime_package_directory(&assets, &manifest, &root)
                .unwrap_err()
                .contains("asset_hash_mismatch")
        );
    }

    #[test]
    fn android_user_run_does_not_stop_after_the_first_frame() {
        assert_eq!(ANDROID_USER_RUN_FRAME_LIMIT, u64::MAX);
        assert!(ANDROID_USER_RUN_FRAME_LIMIT > 1);
    }

    #[test]
    fn startup_diagnostic_overwrites_one_bounded_file_and_preserves_failure_context() {
        let root = temp_root("startup-diagnostic");
        let path = root.join(ANDROID_STARTUP_DIAGNOSTIC_FILE_NAME);
        let recorder = AndroidStartupDiagnosticRecorder::new(Some(path.clone()), "externalData");

        recorder.stage("entry");
        recorder.stage("asset_manifest_parsed");
        recorder.error(&"x".repeat(ANDROID_STARTUP_DIAGNOSTIC_MAX_TEXT_BYTES + 100));
        recorder.panic("launcher panic");

        let bytes = fs::read(&path).unwrap();
        let diagnostic: AndroidStartupDiagnostic = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            diagnostic.schema_version,
            ANDROID_STARTUP_DIAGNOSTIC_SCHEMA_VERSION
        );
        assert_eq!(diagnostic.outcome, "panic");
        assert_eq!(diagnostic.latest_stage, "asset_manifest_parsed");
        assert_eq!(diagnostic.stages, ["entry", "asset_manifest_parsed"]);
        assert!(diagnostic
            .terminal_error
            .unwrap()
            .ends_with("...[truncated]"));
        assert_eq!(diagnostic.panic.as_deref(), Some("launcher panic"));
        assert!(bytes.len() < 16 * 1024);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_diagnostic_persists_terminal_report_summary() {
        let root = temp_root("startup-terminal-report");
        let path = root.join(ANDROID_STARTUP_DIAGNOSTIC_FILE_NAME);
        let recorder = AndroidStartupDiagnosticRecorder::new(Some(path.clone()), "externalData");

        recorder.stage("event_loop_returned");
        recorder.terminal_report(serde_json::json!({
            "framesCompleted": 3,
            "exitCode": 1,
            "diagnostics": [{"code": "surface.failed"}],
        }));

        let diagnostic: AndroidStartupDiagnostic =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(diagnostic.outcome, "returned");
        assert_eq!(
            diagnostic.terminal_report.unwrap()["diagnostics"][0]["code"],
            "surface.failed"
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("runtime-player-android-{name}-{stamp}"))
    }
}
