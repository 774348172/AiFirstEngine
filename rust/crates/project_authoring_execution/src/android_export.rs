use crate::{
    GameProjectCompiler, PreparedRuntimePackage, ProjectAuthoringSession, ProjectManifest,
    TargetProfile,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
    RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION,
};
use runtime_player_android::AndroidRuntimePackageAssetManifest;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub const ANDROID_DEV_EXPORT_REPORT_SCHEMA_VERSION: &str = "android-dev-export-report.v1";
pub const ANDROID_DEV_PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "android-dev-package-manifest.v1";
pub const ANDROID_COMPILE_SDK: u32 = 35;
pub const ANDROID_MIN_SDK: u32 = 26;
pub const ANDROID_NDK_VERSION: &str = "28.1.13356709";
pub const ANDROID_BUILD_TOOLS_VERSION: &str = "35.0.0";
pub const ANDROID_GRADLE_VERSION: &str = "8.11.1";
pub const ANDROID_GRADLE_PLUGIN_VERSION: &str = "8.9.1";
pub const ANDROID_RUST_TARGET: &str = "aarch64-linux-android";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AndroidDevAbi {
    Arm64V8a,
    X86_64,
}

impl AndroidDevAbi {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "arm64-v8a" => Ok(Self::Arm64V8a),
            "x86_64" => Ok(Self::X86_64),
            _ => Err(format!("android_export.unsupported_abi: {value}")),
        }
    }

    pub fn android_abi(self) -> &'static str {
        match self {
            Self::Arm64V8a => "arm64-v8a",
            Self::X86_64 => "x86_64",
        }
    }

    pub fn rust_target(self) -> &'static str {
        match self {
            Self::Arm64V8a => ANDROID_RUST_TARGET,
            Self::X86_64 => "x86_64-linux-android",
        }
    }

    fn output_directory_name(self) -> &'static str {
        match self {
            Self::Arm64V8a => "dev",
            Self::X86_64 => "emulator-x86_64",
        }
    }

    fn cargo_target_env_suffix(self) -> String {
        self.rust_target().replace('-', "_")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidDevExportStatus {
    Success,
    EnvironmentBlocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidToolchainCheck {
    pub id: String,
    pub available: bool,
    pub detail: String,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidToolchainPreflight {
    pub ready: bool,
    pub checks: Vec<AndroidToolchainCheck>,
    pub sdk_root: Option<PathBuf>,
    pub ndk_root: Option<PathBuf>,
    pub linker: Option<PathBuf>,
    pub gradle_command: Option<PathBuf>,
    pub apksigner: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AndroidToolchainFacts {
    pub java_17: bool,
    pub sdk_root: Option<PathBuf>,
    pub sdk_35: bool,
    pub ndk_root: Option<PathBuf>,
    pub ndk_r28b: bool,
    pub linker: Option<PathBuf>,
    pub gradle_command: Option<PathBuf>,
    pub gradle_available: bool,
    pub rust_target_installed: bool,
    pub apksigner: Option<PathBuf>,
}

impl AndroidToolchainPreflight {
    pub fn probe_host() -> Self {
        Self::probe_host_for_abi(AndroidDevAbi::Arm64V8a)
    }

    pub fn probe_host_for_abi(abi: AndroidDevAbi) -> Self {
        Self::from_facts_for_abi(abi, discover_toolchain_facts(abi))
    }

    pub fn from_facts(facts: AndroidToolchainFacts) -> Self {
        Self::from_facts_for_abi(AndroidDevAbi::Arm64V8a, facts)
    }

    pub fn from_facts_for_abi(abi: AndroidDevAbi, facts: AndroidToolchainFacts) -> Self {
        let mut checks = Vec::new();
        checks.push(check(
            "jdk17",
            facts.java_17,
            "JDK 17 is available.",
            "Install JDK 17 and expose java on PATH.",
        ));
        checks.push(check(
            "android-sdk-35",
            facts.sdk_root.is_some() && facts.sdk_35 && facts.apksigner.is_some(),
            "Android SDK platform and build-tools 35 are available.",
            "Install Android SDK platform 35 plus build-tools 35.0.0 and set ANDROID_SDK_ROOT.",
        ));
        checks.push(check(
            "android-ndk-r28b",
            facts.ndk_root.is_some() && facts.ndk_r28b && facts.linker.is_some(),
            &format!(
                "Android NDK r28b {} linker is available.",
                abi.android_abi()
            ),
            "Install NDK 28.1.13356709 in the configured Android SDK.",
        ));
        checks.push(check(
            "gradle-bootstrap",
            facts.gradle_command.is_some() && facts.gradle_available,
            "Gradle bootstrap is available for generating the locked wrapper.",
            "Install Gradle once; the export then locks wrapper 8.11.1 in run-owned staging.",
        ));
        checks.push(check(
            &format!("rust-{}", abi.rust_target()),
            facts.rust_target_installed,
            &format!("Rust {} target is installed.", abi.rust_target()),
            &format!("Install the {} Rust target.", abi.rust_target()),
        ));
        Self {
            ready: checks.iter().all(|entry| entry.available),
            checks,
            sdk_root: facts.sdk_root,
            ndk_root: facts.ndk_root,
            linker: facts.linker,
            gradle_command: facts.gradle_command,
            apksigner: facts.apksigner,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidDevExportRequest {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub build_profile_path: PathBuf,
    pub abi: AndroidDevAbi,
    pub target_profile: TargetProfile,
}

impl AndroidDevExportRequest {
    pub fn dev(project_root: impl Into<PathBuf>) -> Self {
        Self::for_abi(project_root, AndroidDevAbi::Arm64V8a)
    }

    pub fn release(project_root: impl Into<PathBuf>) -> Self {
        let mut request = Self::dev(project_root);
        request.target_profile = TargetProfile::AndroidRelease;
        request.output_dir = request.project_root.join("Build/Android/release");
        request.build_profile_path = request
            .project_root
            .join("BuildProfiles/android.release.json");
        request
    }
    fn is_release(&self) -> bool {
        self.target_profile == TargetProfile::AndroidRelease
    }
    fn profile(&self) -> &'static str {
        if self.is_release() {
            "release"
        } else {
            "dev"
        }
    }
    pub fn emulator_x86_64(project_root: impl Into<PathBuf>) -> Self {
        Self::for_abi(project_root, AndroidDevAbi::X86_64)
    }

    pub fn for_abi(project_root: impl Into<PathBuf>, abi: AndroidDevAbi) -> Self {
        let project_root = project_root.into();
        Self {
            output_dir: project_root
                .join("Build")
                .join("Android")
                .join(abi.output_directory_name()),
            build_profile_path: project_root.join("BuildProfiles").join("android.dev.json"),
            target_profile: TargetProfile::AndroidDev,
            project_root,
            abi,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevPackageManifest {
    pub schema_version: String,
    pub project_id: String,
    pub target: String,
    #[serde(default)]
    pub profile: String,
    pub abi: String,
    pub compile_sdk: u32,
    pub target_sdk: u32,
    pub min_sdk: u32,
    pub ndk_version: String,
    pub application_id: String,
    pub signing_kind: String,
    pub signing_certificate_sha256: String,
    pub project_manifest_sha256: String,
    pub profile_id: String,
    pub runtime_package_digest: String,
    pub runtime_module_id: String,
    pub runtime_module_interface_version: String,
    pub runtime_module_aot_content_digest: String,
    pub native_library_sha256: String,
    pub apk_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevExportReport {
    pub schema_version: String,
    pub status: AndroidDevExportStatus,
    pub package_status: String,
    pub device_qualification: String,
    pub project_root: String,
    pub output_dir: String,
    pub staging_root: Option<String>,
    pub apk_path: Option<String>,
    pub preflight: AndroidToolchainPreflight,
    pub completed_stages: Vec<String>,
    pub diagnostic: Option<String>,
}

pub struct AndroidDevExportPipeline;

impl AndroidDevExportPipeline {
    pub fn export(request: AndroidDevExportRequest) -> AndroidDevExportReport {
        let preflight = AndroidToolchainPreflight::probe_host_for_abi(request.abi);
        if !preflight.ready {
            return Self::export_with_preflight_and_glue(request, preflight, None);
        }
        let prepared = (|| {
            let mut session =
                ProjectAuthoringSession::open(&request.project_root).map_err(|e| e.to_string())?;
            let paths = session
                .source_inventory()
                .map_err(|e| e.to_string())?
                .entries
                .into_iter()
                .map(|e| e.relative_path)
                .collect();
            let lease = session
                .acquire_snapshot_lease("android-export", paths)
                .map_err(|e| e.to_string())?;
            let compiler = GameProjectCompiler::bind(&lease).map_err(|e| e.to_string())?;
            compiler
                .prepare(&lease, request.target_profile)
                .map_err(|e| e.to_string())
        })();
        match prepared {
            Ok(prepared) => {
                Self::export_with_preflight_and_glue(request, preflight, Some(&prepared))
            }
            Err(error) => {
                let mut report = base_report(&request, preflight);
                report.diagnostic = Some(error);
                report
            }
        }
    }

    pub fn export_prepared(
        request: AndroidDevExportRequest,
        prepared: &crate::PreparedRuntimePackage,
    ) -> AndroidDevExportReport {
        let preflight = AndroidToolchainPreflight::probe_host_for_abi(request.abi);
        Self::export_with_preflight_and_glue(request, preflight, Some(prepared))
    }

    #[cfg(test)]
    fn export_with_preflight(
        request: AndroidDevExportRequest,
        preflight: AndroidToolchainPreflight,
    ) -> AndroidDevExportReport {
        Self::export_with_preflight_and_glue(request, preflight, None)
    }

    fn export_with_preflight_and_glue(
        request: AndroidDevExportRequest,
        preflight: AndroidToolchainPreflight,
        prepared: Option<&PreparedRuntimePackage>,
    ) -> AndroidDevExportReport {
        let mut report = base_report(&request, preflight);
        if !matches!(
            request.target_profile,
            TargetProfile::AndroidDev | TargetProfile::AndroidRelease
        ) || (request.is_release() && request.abi != AndroidDevAbi::Arm64V8a)
            || prepared.is_some_and(|p| p.lineage().target_profile() != request.target_profile)
        {
            report.diagnostic = Some("android_export.target_profile_mismatch".into());
            return report;
        }
        if !report.preflight.ready {
            report.status = AndroidDevExportStatus::EnvironmentBlocked;
            report.diagnostic = Some("android_export.toolchain_preflight_blocked".to_string());
            return report;
        }
        if request.is_release() {
            let valid = prepared
                .and_then(|p| p.source().bytes("BuildProfiles/android.release.json"))
                .and_then(|b| serde_json::from_slice::<crate::BuildProfile>(b).ok())
                .is_some_and(|p| {
                    p.validation_issues().is_empty()
                        && p.release
                            .is_some_and(|r| r.layout == "compiler-delivery-v1")
                });
            if !valid {
                report.diagnostic = Some("android_export.release_profile_invalid".into());
                return report;
            }
        }
        let resolved_preflight = report.preflight.clone();
        match stage_build_and_publish(&request, &resolved_preflight, prepared, &mut report) {
            Ok(apk_path) => {
                report.status = AndroidDevExportStatus::Success;
                report.package_status = "success".to_string();
                report.apk_path = Some(apk_path.display().to_string());
            }
            Err(error) => {
                report.status = AndroidDevExportStatus::Failed;
                report.diagnostic = Some(error);
            }
        }
        if request.output_dir.is_dir() {
            if let Err(error) = write_project_json(
                &request,
                &request
                    .output_dir
                    .join("reports/android-dev-export-report.json"),
                &report,
            ) {
                report.status = AndroidDevExportStatus::Failed;
                report.package_status = "failed".into();
                report.diagnostic = Some(error);
            }
        }
        report
    }
}

fn stage_build_and_publish(
    request: &AndroidDevExportRequest,
    preflight: &AndroidToolchainPreflight,
    prepared: Option<&PreparedRuntimePackage>,
    report: &mut AndroidDevExportReport,
) -> Result<PathBuf, String> {
    let relative_output = request
        .output_dir
        .strip_prefix(&request.project_root)
        .map_err(|_| "android_export.external_output_not_authorized")?;
    let safe =
        crate::ProjectRelativePath::parse(relative_output.to_string_lossy().replace('\\', "/"))
            .map_err(|e| e.to_string())?;
    if safe
        .as_path()
        .components()
        .next()
        .is_none_or(|part| part.as_os_str() != "Build")
    {
        return Err("android_export.external_output_not_authorized".into());
    }
    if request.is_release()
        && [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_RUSTFLAGS",
        ]
        .iter()
        .any(|key| env::var_os(key).is_some_and(|v| !v.is_empty()))
    {
        return Err("android_export.release_flags_unsupported".into());
    }
    let toolchain_lock = env::var_os("AIFE_ANDROID_TOOLCHAIN_LOCK")
        .map(PathBuf::from)
        .ok_or("android_export.toolchain_lock_missing")?;
    let gradle_distribution_url = locked_gradle_distribution_url(&toolchain_lock)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    let staging_root = request
        .project_root
        .join("Library")
        .join("AndroidExport")
        .join(format!(
            "{}-{}-{}",
            request.profile(),
            std::process::id(),
            unix_millis()
        ));
    report.staging_root = Some(staging_root.display().to_string());
    let write_scope =
        crate::ProjectWriteScope::open(&request.project_root).map_err(|error| error.to_string())?;
    write_scope
        .ensure_directory(
            staging_root
                .strip_prefix(&request.project_root)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

    let prepared = prepared.ok_or("android_export.prepared_source_required")?;
    if prepared.lineage().target_profile() != request.target_profile
        || (request.is_release() && request.abi != AndroidDevAbi::Arm64V8a)
    {
        return Err("android_export.target_profile_mismatch".into());
    }
    let runtime_input = prepared.runtime_package_build_input().clone();
    let active_scene_id = runtime_input
        .scenes
        .first()
        .map(|scene| scene.id.clone())
        .ok_or("android_export.active_scene_missing")?;
    report.completed_stages.push("profile".into());
    let (normalized_project, staged_source, plan) = stage_leased_source(prepared, &staging_root)?;
    let mut frozen_request = request.clone();
    frozen_request.project_root = staged_source;
    let project_manifest_bytes = prepared
        .source()
        .bytes("project.aife.json")
        .ok_or("android_export.manifest_missing")?;
    let project_manifest: ProjectManifest =
        serde_json::from_slice(project_manifest_bytes).map_err(|e| e.to_string())?;
    let android_project = staging_root.join("android-project");
    let runtime_package_dir = android_project.join("app/src/main/assets/aife/runtime-package");
    let package_request = RuntimePackageBuildRequest {
        schema_version: RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
        project_root: request.project_root.clone(),
        active_scene_id,
        target: format!(
            "android-{}-{}",
            if request.abi == AndroidDevAbi::Arm64V8a {
                "arm64"
            } else {
                "x86_64"
            },
            request.profile()
        ),
        mode: if request.is_release() {
            "release"
        } else {
            "dev-run"
        }
        .to_string(),
        output_dir: runtime_package_dir.clone(),
        previous_package_manifest: None,
        include_debug_readable_json: !request.is_release(),
    };
    let runtime_report = RuntimePackageBuilder::build(&package_request, &runtime_input);
    if runtime_report.status != RuntimePackageBuildStatus::Success {
        return Err("android_export.runtime_package_build_failed".to_string());
    }
    let asset_manifest = AndroidRuntimePackageAssetManifest::from_directory(&runtime_package_dir)?;
    write_json(
        &android_project.join("app/src/main/assets/aife/runtime-package-asset-manifest.json"),
        &asset_manifest,
    )?;
    report.completed_stages.push("runtimePackage".to_string());

    let application_id = android_application_id(&project_manifest.project_id);
    generate_android_project(
        &android_project,
        &frozen_request,
        &normalized_project,
        &application_id,
        preflight.sdk_root.as_deref().expect("ready SDK"),
        prepared.generated_runtime_glue(),
    )?;
    report.completed_stages.push("launcher".to_string());

    let sdk_toolchain: toml::Value = toml::from_str(
        &fs::read_to_string(plan.sdk_root.join("rust-toolchain.toml"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let channel = sdk_toolchain
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or("android_export.toolchain_channel_missing")?;
    let launcher_root = staging_root.join("launcher");
    let cache_override = env::var_os("AIFE_ANDROID_CARGO_TARGET_DIR").map(PathBuf::from);
    let cargo_target = android_native_cargo_target(
        &request.project_root,
        &staging_root,
        cache_override.as_deref(),
    )?;
    let native_environment = native_environment(preflight, request.abi)?;
    fs::write(
        launcher_root.join("Cargo.lock"),
        &plan.dependency_lock_bytes,
    )
    .map_err(|e| e.to_string())?;
    run_android_build_command(
        Command::new("cargo")
            .env("RUSTUP_TOOLCHAIN", channel)
            .env("RUSTUP_AUTO_INSTALL", "0")
            .current_dir(&launcher_root)
            .args(["generate-lockfile", "--offline"]),
        &staging_root,
        "lock_output",
        deadline,
    )?;
    run_android_build_command(
        Command::new("cargo")
            .env("RUSTUP_TOOLCHAIN", channel)
            .env("RUSTUP_AUTO_INSTALL", "0")
            .current_dir(&launcher_root)
            .args([
                "build",
                "--manifest-path",
                &launcher_root.join("Cargo.toml").display().to_string(),
                "--target",
                request.abi.rust_target(),
            ])
            .args(if request.is_release() {
                vec!["--release", "--locked", "--offline"]
            } else {
                vec!["--locked", "--offline"]
            })
            .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
            .env("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", "false")
            .env("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS", "false")
            .env("CARGO_TARGET_DIR", &cargo_target)
            .envs(native_environment),
        &staging_root,
        "cargo_output",
        deadline,
    )?;
    let native_library = cargo_target
        .join(request.abi.rust_target())
        .join(if request.is_release() {
            "release"
        } else {
            "debug"
        })
        .join("libmain.so");
    validate_native_library(&native_library, request.abi)?;
    let jni_library = android_project.join(format!(
        "app/src/main/jniLibs/{}/libmain.so",
        request.abi.android_abi()
    ));
    copy_file(&native_library, &jni_library)?;
    // The Rust GameActivity/Jolt composition links libc++_shared. It is not
    // an Android system library or supplied by the games-activity AAR.
    let ndk_toolchain = preflight
        .linker
        .as_ref()
        .and_then(|linker| linker.parent())
        .and_then(Path::parent)
        .ok_or("android_export.ndk_toolchain_missing")?;
    let shared_cpp = ndk_toolchain.join(format!(
        "sysroot/usr/lib/{}/libc++_shared.so",
        request.abi.rust_target()
    ));
    validate_native_library(&shared_cpp, request.abi)?;
    copy_file(&shared_cpp, &jni_library.with_file_name("libc++_shared.so"))?;
    report.completed_stages.push("rustNative".to_string());

    let gradle = preflight
        .gradle_command
        .as_ref()
        .expect("ready Gradle bootstrap");
    run_android_build_command(
        Command::new(gradle).current_dir(&android_project).args([
            "--no-daemon",
            "wrapper",
            "--gradle-distribution-url",
            &gradle_distribution_url,
        ]),
        &staging_root,
        "wrapper_output",
        deadline,
    )?;
    let gradlew = if cfg!(windows) {
        android_project.join("gradlew.bat")
    } else {
        android_project.join("gradlew")
    };
    run_android_build_command(
        Command::new(&gradlew).current_dir(&android_project).args([
            "--no-daemon",
            if request.is_release() {
                ":app:assembleRelease"
            } else {
                ":app:assembleDebug"
            },
        ]),
        &staging_root,
        "gradle_output",
        deadline,
    )?;
    report.completed_stages.push("gradle".to_string());

    let staged_apk = android_project.join(if request.is_release() {
        "app/build/outputs/apk/release/app-release.apk"
    } else {
        "app/build/outputs/apk/debug/app-debug.apk"
    });
    verify_apk_structure(&staged_apk, request.abi)?;
    let signing_certificate_sha256 = read_signing_certificate_sha256(
        preflight.apksigner.as_ref().expect("ready apksigner"),
        &staged_apk,
    )?;
    report.completed_stages.push("apkVerify".to_string());
    write_scope
        .ensure_directory(safe.as_path())
        .map_err(|error| error.to_string())?;
    let apk_path = request.output_dir.join(if request.is_release() {
        "Game-release.apk"
    } else {
        "Game-debug.apk"
    });
    let apk_bytes = fs::read(&staged_apk).map_err(|error| error.to_string())?;
    write_scope
        .write_atomic(
            apk_path
                .strip_prefix(&request.project_root)
                .map_err(|error| error.to_string())?,
            &apk_bytes,
        )
        .map_err(|error| error.to_string())?;
    let native_bytes = fs::read(&native_library).map_err(|error| error.to_string())?;

    let runtime_manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(runtime_package_dir.join("manifest.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let runtime_module_aot_content_digest = runtime_manifest
        .pointer("/project/runtimeModule/aotContentDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "android_export.runtime_module_aot_digest_missing".to_string())?
        .to_string();
    let package_manifest = AndroidDevPackageManifest {
        schema_version: ANDROID_DEV_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
        project_id: project_manifest.project_id,
        target: "android".to_string(),
        profile: request.profile().into(),
        abi: request.abi.android_abi().to_string(),
        compile_sdk: ANDROID_COMPILE_SDK,
        target_sdk: ANDROID_COMPILE_SDK,
        min_sdk: ANDROID_MIN_SDK,
        ndk_version: ANDROID_NDK_VERSION.to_string(),
        application_id,
        signing_kind: "android-debug".to_string(),
        signing_certificate_sha256,
        project_manifest_sha256: sha256_prefixed(&project_manifest_bytes),
        profile_id: format!("android.{}", request.profile()),
        runtime_package_digest: asset_manifest.runtime_package_digest,
        runtime_module_id: project_manifest.runtime_module.module_id,
        runtime_module_interface_version: project_manifest.runtime_module.interface_version,
        runtime_module_aot_content_digest,
        native_library_sha256: sha256_prefixed(&native_bytes),
        apk_sha256: sha256_prefixed(&apk_bytes),
    };
    write_project_json(
        request,
        &request.output_dir.join("package-manifest.json"),
        &package_manifest,
    )?;
    report.completed_stages.push("publish".to_string());
    Ok(apk_path)
}

fn android_native_cargo_target(
    project_root: &Path,
    staging_root: &Path,
    cache_override: Option<&Path>,
) -> Result<PathBuf, String> {
    let Some(cache) = cache_override else {
        return Ok(staging_root.join("cargo-target"));
    };
    // An explicit, serial caller may retain its completed private target after
    // a bounded export fails later (for example in Gradle). Do not silently
    // adopt a shared target or allow this override to write outside the project.
    if !cache.is_absolute() || !cache.is_dir() {
        return Err("android_export.native_cache_existing_directory_required".into());
    }
    let project = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let cache = fs::canonicalize(cache).map_err(|error| error.to_string())?;
    let relative = cache
        .strip_prefix(&project)
        .map_err(|_| "android_export.native_cache_outside_project")?;
    if relative.components().count() < 2
        || relative
            .components()
            .next()
            .is_none_or(|part| part.as_os_str() != "Library")
    {
        return Err("android_export.native_cache_library_child_required".into());
    }
    Ok(PathBuf::from(
        crate::project_player_artifact::cargo_target_directory(&cache),
    ))
}

fn run_android_build_command(
    command: &mut Command,
    root: &Path,
    stage: &str,
    deadline: std::time::Instant,
) -> Result<(), String> {
    let current_dir = command
        .get_current_dir()
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    let result = runtime_cli::run_bounded_child_process(runtime_cli::BoundedChildProcessRequest {
        executable: PathBuf::from(command.get_program()),
        args: command.get_args().map(|a| a.to_os_string()).collect(),
        current_dir,
        environment: command
            .get_envs()
            .filter_map(|(key, value)| value.map(|v| (key.to_os_string(), v.to_os_string())))
            .collect(),
        timeout: deadline.saturating_duration_since(std::time::Instant::now()),
        stdout_capture_limit_bytes: 4 * 1024 * 1024,
        stderr_capture_limit_bytes: 4 * 1024 * 1024,
        priority: Default::default(),
    });
    write_json(
        &root.join(format!("reports/{stage}.json")),
        &serde_json::json!({"program":command.get_program().to_string_lossy(),"args":command.get_args().map(|a|a.to_string_lossy()).collect::<Vec<_>>(),"result":result}),
    )?;
    if !result.owned_process_cleanup_confirmed()
        || result.exit_reason != runtime_cli::BoundedChildProcessExitReason::Completed
        || result.exit_code != Some(0)
        || result.stdout_truncated
        || result.stderr_truncated
    {
        return Err(format!(
            "android_export.{stage}_failed: {:?}; exit={:?}; ownership={:?}; stderr={}",
            result.exit_reason, result.exit_code, result.ownership, result.stderr_summary
        ));
    }
    Ok(())
}

fn stage_leased_source(
    prepared: &PreparedRuntimePackage,
    staging_root: &Path,
) -> Result<
    (
        ProjectManifest,
        PathBuf,
        crate::ProjectRuntimePlayerStagingPlan,
    ),
    String,
> {
    let frozen = staging_root.join("Source");
    for (path, bytes) in &prepared.source().files {
        let safe = crate::ProjectRelativePath::parse(path).map_err(|e| e.to_string())?;
        let destination = frozen.join(safe.as_path());
        fs::create_dir_all(
            destination
                .parent()
                .ok_or("android_export.source_parent_missing")?,
        )
        .map_err(|e| e.to_string())?;
        fs::write(destination, bytes).map_err(|e| e.to_string())?;
    }
    let engine_rust_root = crate::default_engine_sdk_root();
    let plan = crate::ProjectRuntimeProductionStaging::plan(&frozen, &engine_rust_root)
        .map_err(|e| e.message)?;
    let staged_source = staging_root.join("Staged");
    crate::ProjectRuntimeProductionStaging::stage(&frozen, &staged_source, &plan)
        .map_err(|e| e.message)?;
    let project_manifest_bytes = prepared
        .source()
        .bytes("project.aife.json")
        .ok_or("android_export.manifest_missing")?;
    let project_manifest: ProjectManifest =
        serde_json::from_slice(project_manifest_bytes).map_err(|e| e.to_string())?;
    // The production staging owner normalizes Cargo inputs in RuntimeModuleBuild.
    let mut normalized_project = project_manifest.clone();
    normalized_project.runtime_module.cargo_manifest = "RuntimeModuleBuild/Cargo.toml".into();
    Ok((normalized_project, staged_source, plan))
}

/// Package integrity is independent of (and does not claim) device execution.
pub fn verify_android_package(
    root: &Path,
    target: TargetProfile,
) -> Result<AndroidDevPackageManifest, String> {
    let manifest: AndroidDevPackageManifest = serde_json::from_slice(
        &fs::read(root.join("package-manifest.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let profile = match target {
        TargetProfile::AndroidDev => "dev",
        TargetProfile::AndroidRelease => "release",
        _ => return Err("android_export.target_profile_mismatch".into()),
    };
    if manifest.schema_version != ANDROID_DEV_PACKAGE_MANIFEST_SCHEMA_VERSION
        || manifest.target != "android"
        || manifest.profile != profile
        || manifest.profile_id != format!("android.{profile}")
        || (profile == "release" && manifest.abi != "arm64-v8a")
    {
        return Err("android_export.target_profile_mismatch".into());
    }
    let apk = root.join(if profile == "release" {
        "Game-release.apk"
    } else {
        "Game-debug.apk"
    });
    let bytes = fs::read(&apk).map_err(|e| e.to_string())?;
    if sha256_prefixed(&bytes) != manifest.apk_sha256 {
        return Err("android_export.apk_identity_mismatch".into());
    }
    verify_apk_structure(&apk, AndroidDevAbi::parse(&manifest.abi)?)?;
    Ok(manifest)
}

fn generate_android_project(
    android_project: &Path,
    request: &AndroidDevExportRequest,
    project: &ProjectManifest,
    application_id: &str,
    sdk_root: &Path,
    prepared_runtime_glue: Option<&crate::PreparedRuntimeGlue>,
) -> Result<(), String> {
    let engine_rust_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "android_export.engine_rust_root_missing".to_string())?;
    let launcher_root = android_project
        .parent()
        .ok_or_else(|| "android_export.staging_parent_missing".to_string())?
        .join("launcher");
    let runtime_module = request
        .project_root
        .join(&project.runtime_module.cargo_manifest);
    let runtime_module_root = runtime_module
        .parent()
        .ok_or_else(|| "android_export.runtime_module_root_missing".to_string())?;
    let runtime_dependency_root = if let Some(glue) = prepared_runtime_glue {
        let glue_root = android_project
            .parent()
            .ok_or_else(|| "android_export.staging_parent_missing".to_string())?
            .join("RuntimeGlue");
        glue.materialize(&glue_root, engine_rust_root, runtime_module_root)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        glue_root
    } else {
        runtime_module_root.to_path_buf()
    };
    let runtime_dependency_package =
        runtime_dependency_package(project, prepared_runtime_glue.is_some());
    write_text(
        &launcher_root.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"aife_android_launcher\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"main\"\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nruntime_player_android = {{ path = \"{}\" }}\nproject_runtime = {{ package = \"{}\", path = \"{}\" }}\nwinit = {{ version = \"0.30\", features = [\"android-game-activity\"] }}\n\n[workspace]\n",
            toml_path(&engine_rust_root.join("crates/runtime_player_android")),
            runtime_dependency_package,
            toml_path(&runtime_dependency_root),
        ),
    )?;
    write_text(
        &launcher_root.join("src/lib.rs"),
        "use winit::platform::android::activity::AndroidApp;\n\n#[no_mangle]\npub fn android_main(app: AndroidApp) {\n    let api = unsafe { *project_runtime::aife_project_runtime_entry_v1() };\n    runtime_player_android::run_packaged_android_app(app, api)\n        .expect(\"AI First Android Player startup failed\");\n}\n",
    )?;
    write_text(
        &android_project.join("settings.gradle.kts"),
        "pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }\nrootProject.name = \"AiFirstAndroidPlayer\"\ninclude(\":app\")\n",
    )?;
    write_text(
        &android_project.join("build.gradle.kts"),
        &format!(
            "plugins {{ id(\"com.android.application\") version \"{}\" apply false }}\n",
            ANDROID_GRADLE_PLUGIN_VERSION
        ),
    )?;
    write_text(
        &android_project.join("local.properties"),
        &format!("sdk.dir={}\n", gradle_path(sdk_root)),
    )?;
    write_text(
        &android_project.join("gradle.properties"),
        "android.useAndroidX=true\n",
    )?;
    write_text(
        &android_project.join("app/build.gradle.kts"),
        &format!(
            "plugins {{ id(\"com.android.application\") }}\n\nandroid {{\n    namespace = \"{application_id}\"\n    compileSdk = {ANDROID_COMPILE_SDK}\n    ndkVersion = \"{ANDROID_NDK_VERSION}\"\n    defaultConfig {{\n        applicationId = \"{application_id}\"\n        minSdk = {ANDROID_MIN_SDK}\n        targetSdk = {ANDROID_COMPILE_SDK}\n        versionCode = 1\n        versionName = \"0.1.0\"\n        ndk {{ abiFilters += \"{}\" }}\n    }}\n    buildTypes {{ getByName(\"release\") {{ isDebuggable = false; isMinifyEnabled = false; signingConfig = signingConfigs.getByName(\"debug\") }} }}\n}}\n\ndependencies {{\n    implementation(\"androidx.games:games-activity:3.0.5\")\n    implementation(\"androidx.appcompat:appcompat:1.7.1\")\n    implementation(platform(\"org.jetbrains.kotlin:kotlin-bom:1.8.22\"))\n}}\n",
            request.abi.android_abi()
        ),
    )?;
    write_text(
        &android_project.join("app/src/main/AndroidManifest.xml"),
        &format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">\n  <application android:label=\"{}\" android:hasCode=\"true\" android:allowBackup=\"false\">\n    <activity android:name=\"com.google.androidgamesdk.GameActivity\" android:theme=\"@style/Theme.AppCompat.NoActionBar\" android:screenOrientation=\"portrait\" android:configChanges=\"orientation|screenSize|keyboardHidden\" android:exported=\"true\">\n      <meta-data android:name=\"android.app.lib_name\" android:value=\"main\" />\n      <intent-filter>\n        <action android:name=\"android.intent.action.MAIN\" />\n        <category android:name=\"android.intent.category.LAUNCHER\" />\n      </intent-filter>\n    </activity>\n  </application>\n</manifest>\n",
            xml_escape(&project.project_name)
        ),
    )?;
    Ok(())
}

fn runtime_dependency_package(
    project: &ProjectManifest,
    uses_generated_runtime_glue: bool,
) -> &str {
    if uses_generated_runtime_glue {
        "aife_generated_runtime_glue"
    } else {
        project.runtime_module.cargo_package.as_str()
    }
}

fn discover_toolchain_facts(abi: AndroidDevAbi) -> AndroidToolchainFacts {
    let java_output = command_output("java", &["-version"]);
    let java_text = java_output
        .as_ref()
        .map(combined_output)
        .unwrap_or_default();
    let sdk_root = env::var_os("ANDROID_SDK_ROOT")
        .or_else(|| env::var_os("ANDROID_HOME"))
        .map(PathBuf::from)
        .filter(|path| path.is_dir());
    let ndk_root = sdk_root
        .as_ref()
        .map(|root| root.join("ndk").join(ANDROID_NDK_VERSION))
        .filter(|path| path.is_dir());
    let linker = ndk_root
        .as_ref()
        .map(|root| {
            root.join("toolchains/llvm/prebuilt/windows-x86_64/bin")
                .join(format!("{}{ANDROID_MIN_SDK}-clang.cmd", abi.rust_target()))
        })
        .filter(|path| path.is_file());
    let apksigner = sdk_root
        .as_ref()
        .map(|root| {
            root.join("build-tools")
                .join(ANDROID_BUILD_TOOLS_VERSION)
                .join(if cfg!(windows) {
                    "apksigner.bat"
                } else {
                    "apksigner"
                })
        })
        .filter(|path| path.is_file());
    let gradle_program = gradle_program();
    let gradle_available = command_output(gradle_program, &["--version"])
        .is_some_and(|output| output.status.success());
    let rust_targets = command_output("rustup", &["target", "list", "--installed"])
        .filter(|output| output.status.success())
        .map(|output| combined_output(&output))
        .unwrap_or_default();
    AndroidToolchainFacts {
        java_17: command_output("jar", &["--version"])
            .is_some_and(|output| output.status.success())
            && java_output.is_some_and(|output| output.status.success())
            && (java_text.contains("version \"17") || java_text.contains("openjdk 17")),
        sdk_35: sdk_root
            .as_ref()
            .is_some_and(|root| root.join("platforms/android-35/android.jar").is_file()),
        sdk_root,
        ndk_r28b: ndk_root.is_some(),
        ndk_root,
        linker,
        gradle_command: gradle_available.then(|| PathBuf::from(gradle_program)),
        gradle_available,
        rust_target_installed: rust_targets
            .lines()
            .any(|line| line.trim() == abi.rust_target()),
        apksigner,
    }
}

fn base_report(
    request: &AndroidDevExportRequest,
    preflight: AndroidToolchainPreflight,
) -> AndroidDevExportReport {
    AndroidDevExportReport {
        schema_version: ANDROID_DEV_EXPORT_REPORT_SCHEMA_VERSION.to_string(),
        status: AndroidDevExportStatus::Failed,
        package_status: "notBuilt".to_string(),
        device_qualification: "notRun".to_string(),
        project_root: request.project_root.display().to_string(),
        output_dir: request.output_dir.display().to_string(),
        staging_root: None,
        apk_path: None,
        preflight,
        completed_stages: vec!["toolchain".to_string()],
        diagnostic: None,
    }
}

fn check(id: &str, available: bool, detail: &str, next_action: &str) -> AndroidToolchainCheck {
    AndroidToolchainCheck {
        id: id.to_string(),
        available,
        detail: if available {
            detail.to_string()
        } else {
            format!("Missing: {id}")
        },
        next_action: (!available).then(|| next_action.to_string()),
    }
}

fn command_output(program: &str, args: &[&str]) -> Option<Output> {
    Command::new(program).args(args).output().ok()
}

fn gradle_program() -> &'static str {
    if cfg!(windows) {
        "gradle.bat"
    } else {
        "gradle"
    }
}

pub(crate) fn native_environment(
    preflight: &AndroidToolchainPreflight,
    abi: AndroidDevAbi,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
    if !preflight.ready {
        return Err("android_export.toolchain_preflight_blocked".into());
    }
    let linker = preflight
        .linker
        .as_ref()
        .ok_or("android_export.ndk_linker_missing")?;
    let ndk = preflight
        .ndk_root
        .as_ref()
        .ok_or("android_export.ndk_missing")?;
    let suffix = abi.cargo_target_env_suffix();
    Ok(vec![
        (
            format!("CARGO_TARGET_{}_LINKER", suffix.to_ascii_uppercase()).into(),
            linker.as_os_str().into(),
        ),
        (format!("CC_{suffix}").into(), linker.as_os_str().into()),
        (
            format!("CXX_{suffix}").into(),
            android_cxx_compiler(linker, abi)?.into_os_string(),
        ),
        (
            format!("AR_{suffix}").into(),
            android_archiver(linker)?.into_os_string(),
        ),
        ("ANDROID_NDK_HOME".into(), ndk.as_os_str().into()),
    ])
}

fn android_cxx_compiler(linker: &Path, abi: AndroidDevAbi) -> Result<PathBuf, String> {
    let compiler = linker.with_file_name(if cfg!(windows) {
        format!("{}{ANDROID_MIN_SDK}-clang++.cmd", abi.rust_target())
    } else {
        format!("{}{ANDROID_MIN_SDK}-clang++", abi.rust_target())
    });
    compiler
        .is_file()
        .then_some(compiler.clone())
        .ok_or_else(|| format!("android_export.ndk_cxx_missing: {}", compiler.display()))
}

fn android_archiver(linker: &Path) -> Result<PathBuf, String> {
    let archiver = linker.with_file_name(if cfg!(windows) {
        "llvm-ar.exe"
    } else {
        "llvm-ar"
    });
    archiver
        .is_file()
        .then_some(archiver.clone())
        .ok_or_else(|| {
            format!(
                "android_export.ndk_archiver_missing: {}",
                archiver.display()
            )
        })
}

fn locked_gradle_distribution_url(lock_path: &Path) -> Result<String, String> {
    let toolchain_root = lock_path
        .parent()
        .ok_or_else(|| "android_export.toolchain_lock_parent_missing".to_string())?;
    let archive = toolchain_root
        .join("downloads")
        .join(format!("gradle-{ANDROID_GRADLE_VERSION}-bin.zip"));
    if !archive.is_file() {
        return Err(format!(
            "android_export.locked_gradle_distribution_missing: {}",
            archive.display()
        ));
    }
    url::Url::from_file_path(&archive)
        .map(|url| url.to_string())
        .map_err(|_| {
            format!(
                "android_export.locked_gradle_distribution_url_invalid: {}",
                archive.display()
            )
        })
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn ensure_command_success(code: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("{code}: {}", combined_output(output)))
    }
}

fn validate_native_library(path: &Path, abi: AndroidDevAbi) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let machine = if abi == AndroidDevAbi::Arm64V8a {
        183_u16
    } else {
        62_u16
    };
    if bytes.len() < 64
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || u16::from_le_bytes([bytes[18], bytes[19]]) != machine
    {
        return Err("android_export.native_target_mismatch".into());
    }
    Ok(())
}

fn verify_apk_structure(apk_path: &Path, abi: AndroidDevAbi) -> Result<(), String> {
    let result = runtime_cli::run_bounded_child_process(runtime_cli::BoundedChildProcessRequest {
        executable: PathBuf::from("jar"),
        args: vec!["tf".into(), apk_path.as_os_str().into()],
        current_dir: std::env::current_dir().map_err(|e| e.to_string())?,
        environment: Vec::new(),
        timeout: std::time::Duration::from_secs(20),
        stdout_capture_limit_bytes: 4 * 1024 * 1024,
        stderr_capture_limit_bytes: 1024 * 1024,
        priority: Default::default(),
    });
    if !result.owned_process_cleanup_confirmed()
        || result.exit_code != Some(0)
        || result.stdout_truncated
        || result.stderr_truncated
    {
        return Err(format!(
            "android_export.apk_archive_invalid: {:?}; {}",
            result.exit_reason, result.stderr_summary
        ));
    }
    let entries = result
        .stdout_summary
        .lines()
        .collect::<std::collections::BTreeSet<_>>();
    validate_apk_entries(&entries, abi)
}

fn validate_apk_entries(
    entries: &std::collections::BTreeSet<&str>,
    abi: AndroidDevAbi,
) -> Result<(), String> {
    let native_library_entry = format!("lib/{}/libmain.so", abi.android_abi());
    for required in [
        "AndroidManifest.xml".to_string(),
        native_library_entry,
        format!("lib/{}/libc++_shared.so", abi.android_abi()),
        "assets/aife/runtime-package/manifest.json".to_string(),
        "assets/aife/runtime-package-asset-manifest.json".to_string(),
    ] {
        if !entries.contains(required.as_str()) {
            return Err(format!("android_export.apk_entry_missing: {required}"));
        }
    }
    if ["arm64-v8a", "x86_64", "x86", "armeabi-v7a"]
        .iter()
        .filter(|candidate| **candidate != abi.android_abi())
        .map(|candidate| format!("lib/{candidate}/"))
        .any(|prefix| entries.iter().any(|entry| entry.starts_with(&prefix)))
    {
        return Err("android_export.apk_unexpected_abi".to_string());
    }
    Ok(())
}

fn read_signing_certificate_sha256(apksigner: &Path, apk_path: &Path) -> Result<String, String> {
    let output = Command::new(apksigner)
        .args(["verify", "--print-certs"])
        .arg(apk_path)
        .output()
        .map_err(|error| format!("android_export.apksigner_spawn_failed: {error}"))?;
    ensure_command_success("android_export.apk_signature_verify_failed", &output)?;
    combined_output(&output)
        .lines()
        .find_map(|line| {
            line.split_once("certificate SHA-256 digest:")
                .map(|(_, digest)| digest.trim().to_ascii_lowercase())
        })
        .filter(|digest| {
            digest.len() == 64 && digest.chars().all(|value| value.is_ascii_hexdigit())
        })
        .ok_or_else(|| "android_export.apk_signing_fingerprint_missing".to_string())
}

fn android_application_id(project_id: &str) -> String {
    let suffix = project_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    format!(
        "com.aifirst.dev.{}",
        if suffix.is_empty() {
            "project"
        } else {
            &suffix
        }
    )
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, text).map_err(|error| error.to_string())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| format!("android_export.copy_failed: {error}"))
}

fn write_project_json<T: Serialize>(
    request: &AndroidDevExportRequest,
    path: &Path,
    value: &T,
) -> Result<(), String> {
    let relative = path
        .strip_prefix(&request.project_root)
        .map_err(|_| "android_export.external_output_not_authorized")?;
    let relative = crate::ProjectRelativePath::parse(relative).map_err(|e| e.to_string())?;
    if relative
        .as_path()
        .components()
        .next()
        .is_none_or(|p| p.as_os_str() != "Build")
    {
        return Err("android_export.external_output_not_authorized".into());
    }
    crate::ProjectWriteScope::open(&request.project_root)
        .map_err(|e| e.to_string())?
        .write_atomic(
            relative.as_path(),
            &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn gradle_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_apk_requires_its_shared_cpp_runtime() {
        let entries = [
            "AndroidManifest.xml",
            "lib/arm64-v8a/libmain.so",
            "assets/aife/runtime-package/manifest.json",
            "assets/aife/runtime-package-asset-manifest.json",
        ]
        .into_iter()
        .collect();
        assert_eq!(
            validate_apk_entries(&entries, AndroidDevAbi::Arm64V8a),
            Err("android_export.apk_entry_missing: lib/arm64-v8a/libc++_shared.so".into())
        );
    }

    #[test]
    fn android_native_cargo_cache_is_explicit_existing_and_project_owned() {
        let root = env::temp_dir().join(format!(
            "android-native-cache-{}-{}",
            std::process::id(),
            unix_millis()
        ));
        let project = root.join("project");
        let staging = project.join("Library/AndroidExport/fresh-run");
        let cache = project.join("Library/AndroidExport/completed-run/cargo-target");
        let external = root.join("other-project/cargo-target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&external).unwrap();
        assert_eq!(
            android_native_cargo_target(&project, &staging, None).unwrap(),
            staging.join("cargo-target")
        );
        assert_eq!(
            android_native_cargo_target(&project, &staging, Some(&cache)).unwrap(),
            if cfg!(windows) {
                cache.clone()
            } else {
                fs::canonicalize(&cache).unwrap()
            }
        );
        #[cfg(windows)]
        assert_eq!(
            android_native_cargo_target(
                &project,
                &staging,
                Some(&fs::canonicalize(&cache).unwrap())
            )
            .unwrap(),
            cache,
            "validated canonical ownership must not put a verbatim drive into NDK linker arguments"
        );
        for forbidden in [
            &external,
            &project,
            &project.join("Library"),
            &project.join("RuntimeModule"),
            &project.join("Library/missing"),
        ] {
            assert!(
                android_native_cargo_target(&project, &staging, Some(forbidden)).is_err(),
                "{}",
                forbidden.display()
            );
        }
    }

    #[test]
    fn android_native_tools_are_bound_to_locked_target_and_api_level() {
        for abi in [AndroidDevAbi::Arm64V8a, AndroidDevAbi::X86_64] {
            let root = std::env::temp_dir().join(format!(
                "android-cxx-{}-{}",
                abi.android_abi(),
                unix_millis()
            ));
            fs::create_dir_all(&root).unwrap();
            let linker = root.join(format!(
                "{}{ANDROID_MIN_SDK}-clang{}",
                abi.rust_target(),
                if cfg!(windows) { ".cmd" } else { "" }
            ));
            let cxx = root.join(format!(
                "{}{ANDROID_MIN_SDK}-clang++{}",
                abi.rust_target(),
                if cfg!(windows) { ".cmd" } else { "" }
            ));
            fs::write(&linker, b"linker").unwrap();
            assert!(android_cxx_compiler(&linker, abi).is_err());
            assert!(android_archiver(&linker).is_err());
            fs::write(&cxx, b"cxx").unwrap();
            assert_eq!(android_cxx_compiler(&linker, abi), Ok(cxx));
            let archiver = root.join(if cfg!(windows) {
                "llvm-ar.exe"
            } else {
                "llvm-ar"
            });
            fs::write(&archiver, b"archiver").unwrap();
            assert_eq!(android_archiver(&linker), Ok(archiver));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn locked_gradle_distribution_uses_local_verified_archive() {
        let root = std::env::temp_dir().join(format!("android-gradle-lock-{}", unix_millis()));
        let lock_path = root.join("toolchain-lock.json");
        let archive = root
            .join("downloads")
            .join(format!("gradle-{ANDROID_GRADLE_VERSION}-bin.zip"));
        fs::create_dir_all(archive.parent().unwrap()).unwrap();
        fs::write(&lock_path, b"lock").unwrap();
        assert!(locked_gradle_distribution_url(&lock_path).is_err());
        fs::write(&archive, b"locked-gradle").unwrap();
        let url = locked_gradle_distribution_url(&lock_path).unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().to_file_path(), Ok(archive));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gradle_program_matches_host_launcher_contract() {
        if cfg!(windows) {
            assert_eq!(gradle_program(), "gradle.bat");
        } else {
            assert_eq!(gradle_program(), "gradle");
        }

        if std::env::var_os("AIFE_ANDROID_TOOLCHAIN_LOCK").is_some() {
            assert!(command_output(gradle_program(), &["--version"])
                .is_some_and(|output| output.status.success()));
        }
    }

    #[test]
    fn preflight_is_fail_closed_and_actionable() {
        let preflight = AndroidToolchainPreflight::from_facts(AndroidToolchainFacts::default());
        assert!(!preflight.ready);
        assert_eq!(preflight.checks.len(), 5);
        assert!(preflight
            .checks
            .iter()
            .all(|entry| !entry.available && entry.next_action.is_some()));

        let emulator = AndroidToolchainPreflight::from_facts_for_abi(
            AndroidDevAbi::X86_64,
            AndroidToolchainFacts::default(),
        );
        assert!(emulator
            .checks
            .iter()
            .any(|entry| entry.id == "rust-x86_64-linux-android"));
    }

    #[test]
    fn blocked_export_does_not_create_output_or_staging() {
        let root = std::env::temp_dir().join(format!("android-export-blocked-{}", unix_millis()));
        let request = AndroidDevExportRequest::dev(&root);
        let report = AndroidDevExportPipeline::export_with_preflight(
            request.clone(),
            AndroidToolchainPreflight::from_facts(AndroidToolchainFacts::default()),
        );
        assert_eq!(report.status, AndroidDevExportStatus::EnvironmentBlocked);
        assert_eq!(report.package_status, "notBuilt");
        assert_eq!(report.device_qualification, "notRun");
        assert!(!request.output_dir.exists());
        assert!(!root.join("Library/AndroidExport").exists());

        let emulator = AndroidDevExportRequest::emulator_x86_64(&root);
        assert_eq!(
            emulator.output_dir,
            root.join("Build/Android/emulator-x86_64")
        );
        assert_eq!(emulator.abi, AndroidDevAbi::X86_64);
    }

    #[test]
    fn generated_identity_is_stable_and_android_legal() {
        assert_eq!(
            android_application_id("project-4966952341520437268"),
            "com.aifirst.dev.project4966952341520437268"
        );
        assert_eq!(android_application_id("---"), "com.aifirst.dev.project");
        let entries = [
            "AndroidManifest.xml",
            "lib/arm64-v8a/libmain.so",
            "lib/arm64-v8a/libc++_shared.so",
            "assets/aife/runtime-package/manifest.json",
            "assets/aife/runtime-package-asset-manifest.json",
        ]
        .into_iter()
        .collect();
        assert_eq!(
            validate_apk_entries(&entries, AndroidDevAbi::Arm64V8a),
            Ok(())
        );
        assert!(validate_apk_entries(&entries, AndroidDevAbi::X86_64).is_err());
        let mut wrong = entries.clone();
        wrong.insert("lib/x86/libmain.so");
        assert_eq!(
            validate_apk_entries(&wrong, AndroidDevAbi::Arm64V8a),
            Err("android_export.apk_unexpected_abi".into())
        );
        let apk_path = std::env::temp_dir().join(format!(
            "android-not-an-apk-{}-{}.apk",
            std::process::id(),
            unix_millis()
        ));
        fs::write(&apk_path, b"AndroidManifest.xml lib/arm64-v8a/libmain.so assets/aife/runtime-package/manifest.json assets/aife/runtime-package-asset-manifest.json").unwrap();
        assert!(
            verify_apk_structure(&apk_path, AndroidDevAbi::Arm64V8a).is_err(),
            "byte substrings cannot qualify an APK archive"
        );
        fs::remove_file(apk_path).unwrap();
    }

    #[test]
    fn generated_project_links_selected_runtime_module_and_game_activity() {
        let root = std::env::temp_dir().join(format!("android-export-source-{}", unix_millis()));
        let project_root = root.join("project");
        let android_project = root.join("staging/android-project");
        let sdk_root = root.join("sdk");
        fs::create_dir_all(project_root.join("RuntimeModule")).unwrap();
        let request = AndroidDevExportRequest::dev(&project_root);
        let project: ProjectManifest = serde_json::from_value(serde_json::json!({
            "schemaVersion": "aife-project.v2",
            "projectId": "tower-test",
            "projectName": "Tower Test",
            "engineVersion": "0.1.0",
            "createdAt": "1",
            "lastOpenedAt": "1",
            "defaultScene": "Scenes/Main.scene.json",
            "assetRoot": "Assets",
            "settingsVersion": "aife-project-settings.v1",
            "runtimeModule": {
                "sourceKind": "projectRust",
                "moduleId": "sample.tower.runtime",
                "interfaceVersion": "project-runtime-module.v2",
                "cargoManifest": "RuntimeModule/Cargo.toml",
                "cargoPackage": "tower_runtime",
                "playerBinary": "tower_player"
            }
        }))
        .unwrap();
        assert_eq!(
            runtime_dependency_package(&project, true),
            "aife_generated_runtime_glue"
        );

        generate_android_project(
            &android_project,
            &request,
            &project,
            "com.aifirst.dev.towertest",
            &sdk_root,
            None,
        )
        .unwrap();

        let launcher = fs::read_to_string(root.join("staging/launcher/src/lib.rs")).unwrap();
        let cargo = fs::read_to_string(root.join("staging/launcher/Cargo.toml")).unwrap();
        let manifest =
            fs::read_to_string(android_project.join("app/src/main/AndroidManifest.xml")).unwrap();
        let gradle = fs::read_to_string(android_project.join("app/build.gradle.kts")).unwrap();
        let gradle_properties =
            fs::read_to_string(android_project.join("gradle.properties")).unwrap();
        assert!(launcher.contains("project_runtime::aife_project_runtime_entry_v1"));
        assert!(launcher.contains("run_packaged_android_app"));
        assert!(cargo.contains("package = \"tower_runtime\""));
        assert!(manifest.contains("com.google.androidgamesdk.GameActivity"));
        assert!(manifest.contains("android:theme=\"@style/Theme.AppCompat.NoActionBar\""));
        assert!(manifest.contains("android:screenOrientation=\"portrait\""));
        assert!(gradle.contains("abiFilters += \"arm64-v8a\""));
        assert!(gradle.contains("androidx.games:games-activity:3.0.5"));
        assert!(gradle.contains("androidx.appcompat:appcompat:1.7.1"));
        assert!(gradle.contains("org.jetbrains.kotlin:kotlin-bom:1.8.22"));
        assert_eq!(gradle_properties, "android.useAndroidX=true\n");

        let emulator_project = root.join("emulator-staging/android-project");
        let emulator_request = AndroidDevExportRequest::emulator_x86_64(&project_root);
        generate_android_project(
            &emulator_project,
            &emulator_request,
            &project,
            "com.aifirst.dev.towertest",
            &sdk_root,
            None,
        )
        .unwrap();
        let emulator_gradle =
            fs::read_to_string(emulator_project.join("app/build.gradle.kts")).unwrap();
        assert!(emulator_gradle.contains("abiFilters += \"x86_64\""));
        assert!(!emulator_gradle.contains("abiFilters += \"arm64-v8a\""));
    }
    #[test]
    fn android_release_native_target_rejects_host_and_wrong_architecture() {
        let path = std::env::temp_dir().join(format!(
            "android-native-header-{}-{}",
            std::process::id(),
            unix_millis()
        ));
        fs::write(&path, b"MZ Windows DLL").unwrap();
        assert!(validate_native_library(&path, AndroidDevAbi::Arm64V8a).is_err());
        let mut elf = [0_u8; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[18..20].copy_from_slice(&183_u16.to_le_bytes());
        fs::write(&path, elf).unwrap();
        validate_native_library(&path, AndroidDevAbi::Arm64V8a).unwrap();
        assert!(validate_native_library(&path, AndroidDevAbi::X86_64).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn android_release_staging_uses_lease_after_live_source_change_and_removal() {
        let root = std::env::temp_dir().join(format!(
            "aife-android-source-{}-{}",
            std::process::id(),
            unix_millis()
        ));
        let project = root.join("project");
        fs::create_dir_all(project.join("RuntimeModule/src")).unwrap();
        fs::create_dir_all(project.join("BuildProfiles")).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../samples/platform_3d_benchmark_project/project.aife.json"
        ))
        .unwrap();
        fs::create_dir_all(project.join("Scenes")).unwrap();
        fs::write(
            project.join("Scenes/Main.scene.json"),
            br#"{"id":"main","name":"Main","entities":[]}"#,
        )
        .unwrap();
        fs::write(
            project.join("project.aife.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(project.join("BuildProfiles/android.release.json"),include_bytes!("../../../../samples/platform_3d_benchmark_project/BuildProfiles/android.release.json")).unwrap();
        fs::write(
            project.join("RuntimeModule/Cargo.toml"),
            include_bytes!(
                "../../../../samples/platform_3d_benchmark_project/RuntimeModule/Cargo.toml"
            ),
        )
        .unwrap();
        let source = include_bytes!(
            "../../../../samples/platform_3d_benchmark_project/RuntimeModule/src/lib.rs"
        );
        fs::write(project.join("RuntimeModule/src/lib.rs"), source).unwrap();
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("android-stage-r1", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let prepared = compiler
            .prepare(&lease, TargetProfile::AndroidRelease)
            .unwrap();
        fs::write(project.join("RuntimeModule/src/lib.rs"), b"invalid live r2").unwrap();
        fs::remove_file(project.join("project.aife.json")).unwrap();
        let staging = root.join("staging");
        let (mut normalized, staged, _) = stage_leased_source(&prepared, &staging).unwrap();
        assert_eq!(
            fs::read(staged.join("RuntimeModule/src/lib.rs")).unwrap(),
            source
        );
        normalized.runtime_module.cargo_manifest = "RuntimeModuleBuild/Cargo.toml".into();
        let mut request = AndroidDevExportRequest::release(&project);
        request.project_root = staged.clone();
        generate_android_project(
            &staging.join("android-project"),
            &request,
            &normalized,
            "com.aifirst.dev.fixture",
            &root.join("sdk"),
            prepared.generated_runtime_glue(),
        )
        .unwrap();
        let cargo = fs::read_to_string(staging.join("launcher/Cargo.toml")).unwrap();
        assert!(cargo.contains("aife_generated_runtime_glue"));
        assert!(!cargo.contains(&toml_path(&project)));
        let gradle =
            fs::read_to_string(staging.join("android-project/app/build.gradle.kts")).unwrap();
        assert!(gradle.contains("isDebuggable = false"));
        assert!(gradle.contains("abiFilters += \"arm64-v8a\""));
        fs::remove_dir_all(root).unwrap();
    }
}
