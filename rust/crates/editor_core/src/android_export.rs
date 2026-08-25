use crate::{
    ProjectManifest, ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus,
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

    fn runtime_package_target(self) -> &'static str {
        match self {
            Self::Arm64V8a => "android-arm64-dev",
            Self::X86_64 => "android-x86_64-dev",
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
}

impl AndroidDevExportRequest {
    pub fn dev(project_root: impl Into<PathBuf>) -> Self {
        Self::for_abi(project_root, AndroidDevAbi::Arm64V8a)
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
        Self::export_with_preflight(request, preflight)
    }

    fn export_with_preflight(
        request: AndroidDevExportRequest,
        preflight: AndroidToolchainPreflight,
    ) -> AndroidDevExportReport {
        let mut report = base_report(&request, preflight);
        if !report.preflight.ready {
            report.status = AndroidDevExportStatus::EnvironmentBlocked;
            report.diagnostic = Some("android_export.toolchain_preflight_blocked".to_string());
            return report;
        }
        let resolved_preflight = report.preflight.clone();
        match stage_build_and_publish(&request, &resolved_preflight, &mut report) {
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
            let _ = write_json(
                &request
                    .output_dir
                    .join("reports/android-dev-export-report.json"),
                &report,
            );
        }
        report
    }
}

fn stage_build_and_publish(
    request: &AndroidDevExportRequest,
    preflight: &AndroidToolchainPreflight,
    report: &mut AndroidDevExportReport,
) -> Result<PathBuf, String> {
    if request.output_dir
        != request
            .project_root
            .join("Build")
            .join("Android")
            .join(request.abi.output_directory_name())
    {
        return Err("android_export.external_output_not_authorized".to_string());
    }
    let staging_root = request
        .project_root
        .join("Library")
        .join("AndroidExport")
        .join(format!("dev-{}", unix_millis()));
    report.staging_root = Some(staging_root.display().to_string());
    fs::create_dir_all(&staging_root).map_err(|error| error.to_string())?;

    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&request.project_root)
            .with_build_profile_path(&request.build_profile_path),
    );
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        return Err("android_export.runtime_package_assembly_failed".to_string());
    }
    report.completed_stages.push("profile".to_string());
    let runtime_input = assembly
        .build_input
        .ok_or_else(|| "android_export.runtime_package_input_missing".to_string())?;
    let active_scene_id = assembly
        .active_scene_id
        .ok_or_else(|| "android_export.active_scene_missing".to_string())?;

    let android_project = staging_root.join("android-project");
    let runtime_package_dir = android_project.join("app/src/main/assets/aife/runtime-package");
    let package_request = RuntimePackageBuildRequest {
        schema_version: RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
        project_root: request.project_root.clone(),
        active_scene_id,
        target: request.abi.runtime_package_target().to_string(),
        mode: "dev-run".to_string(),
        output_dir: runtime_package_dir.clone(),
        previous_package_manifest: None,
        include_debug_readable_json: true,
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

    let project_manifest: ProjectManifest = serde_json::from_slice(
        &fs::read(request.project_root.join("project.aife.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let application_id = android_application_id(&project_manifest.project_id);
    generate_android_project(
        &android_project,
        request,
        &project_manifest,
        &application_id,
        preflight.sdk_root.as_deref().expect("ready SDK"),
    )?;
    report.completed_stages.push("launcher".to_string());

    let launcher_root = staging_root.join("launcher");
    let cargo_target = staging_root.join("cargo-target");
    let linker = preflight.linker.as_ref().expect("ready linker");
    let cxx = android_cxx_compiler(linker, request.abi)?;
    let archiver = android_archiver(linker)?;
    let cargo_target_env = format!(
        "CARGO_TARGET_{}_LINKER",
        request.abi.cargo_target_env_suffix().to_ascii_uppercase()
    );
    let cc_env = format!("CC_{}", request.abi.cargo_target_env_suffix());
    let cxx_env = format!("CXX_{}", request.abi.cargo_target_env_suffix());
    let ar_env = format!("AR_{}", request.abi.cargo_target_env_suffix());
    let cargo_output = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            &launcher_root.join("Cargo.toml").display().to_string(),
            "--target",
            request.abi.rust_target(),
        ])
        .env("CARGO_TARGET_DIR", &cargo_target)
        .env(cargo_target_env, linker)
        .env(cc_env, linker)
        .env(cxx_env, &cxx)
        .env(ar_env, &archiver)
        .env(
            "ANDROID_NDK_HOME",
            preflight.ndk_root.as_ref().expect("ready NDK"),
        )
        .output()
        .map_err(|error| format!("android_export.rust_native_spawn_failed: {error}"))?;
    ensure_command_success("android_export.rust_native_build_failed", &cargo_output)?;
    let native_library = cargo_target
        .join(request.abi.rust_target())
        .join("debug")
        .join("libmain.so");
    let jni_library = android_project.join(format!(
        "app/src/main/jniLibs/{}/libmain.so",
        request.abi.android_abi()
    ));
    copy_file(&native_library, &jni_library)?;
    report.completed_stages.push("rustNative".to_string());

    let gradle = preflight
        .gradle_command
        .as_ref()
        .expect("ready Gradle bootstrap");
    let toolchain_lock = env::var_os("AIFE_ANDROID_TOOLCHAIN_LOCK")
        .map(PathBuf::from)
        .ok_or_else(|| "android_export.toolchain_lock_missing".to_string())?;
    let gradle_distribution_url = locked_gradle_distribution_url(&toolchain_lock)?;
    let wrapper_output = Command::new(gradle)
        .current_dir(&android_project)
        .args([
            "--no-daemon",
            "wrapper",
            "--gradle-distribution-url",
            &gradle_distribution_url,
        ])
        .output()
        .map_err(|error| format!("android_export.gradle_wrapper_spawn_failed: {error}"))?;
    ensure_command_success("android_export.gradle_wrapper_failed", &wrapper_output)?;
    let gradlew = if cfg!(windows) {
        android_project.join("gradlew.bat")
    } else {
        android_project.join("gradlew")
    };
    let gradle_output = Command::new(&gradlew)
        .current_dir(&android_project)
        .args(["--no-daemon", ":app:assembleDebug"])
        .output()
        .map_err(|error| format!("android_export.gradle_spawn_failed: {error}"))?;
    ensure_command_success("android_export.gradle_build_failed", &gradle_output)?;
    report.completed_stages.push("gradle".to_string());

    let staged_apk = android_project.join("app/build/outputs/apk/debug/app-debug.apk");
    verify_apk_structure(&staged_apk, request.abi)?;
    let signing_certificate_sha256 = read_signing_certificate_sha256(
        preflight.apksigner.as_ref().expect("ready apksigner"),
        &staged_apk,
    )?;
    report.completed_stages.push("apkVerify".to_string());
    fs::create_dir_all(&request.output_dir).map_err(|error| error.to_string())?;
    let apk_path = request.output_dir.join("TowerDefense-debug.apk");
    atomic_copy(&staged_apk, &apk_path)?;
    let native_bytes = fs::read(&native_library).map_err(|error| error.to_string())?;
    let apk_bytes = fs::read(&apk_path).map_err(|error| error.to_string())?;
    let project_manifest_bytes = fs::read(request.project_root.join("project.aife.json"))
        .map_err(|error| error.to_string())?;
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
        abi: request.abi.android_abi().to_string(),
        compile_sdk: ANDROID_COMPILE_SDK,
        target_sdk: ANDROID_COMPILE_SDK,
        min_sdk: ANDROID_MIN_SDK,
        ndk_version: ANDROID_NDK_VERSION.to_string(),
        application_id,
        signing_kind: "android-debug".to_string(),
        signing_certificate_sha256,
        project_manifest_sha256: sha256_prefixed(&project_manifest_bytes),
        profile_id: "android.dev".to_string(),
        runtime_package_digest: asset_manifest.runtime_package_digest,
        runtime_module_id: project_manifest.runtime_module.module_id,
        runtime_module_interface_version: project_manifest.runtime_module.interface_version,
        runtime_module_aot_content_digest,
        native_library_sha256: sha256_prefixed(&native_bytes),
        apk_sha256: sha256_prefixed(&apk_bytes),
    };
    write_json(
        &request.output_dir.join("package-manifest.json"),
        &package_manifest,
    )?;
    report.completed_stages.push("publish".to_string());
    Ok(apk_path)
}

fn generate_android_project(
    android_project: &Path,
    request: &AndroidDevExportRequest,
    project: &ProjectManifest,
    application_id: &str,
    sdk_root: &Path,
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
    write_text(
        &launcher_root.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"aife_android_launcher\"\nversion = \"0.0.2\"\nedition = \"2021\"\n\n[lib]\nname = \"main\"\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nruntime_player_android = {{ path = \"{}\" }}\nproject_runtime = {{ package = \"{}\", path = \"{}\" }}\nwinit = {{ version = \"0.30\", features = [\"android-game-activity\"] }}\n\n[workspace]\n",
            toml_path(&engine_rust_root.join("crates/runtime_player_android")),
            project.runtime_module.cargo_package,
            toml_path(runtime_module_root),
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
            "plugins {{ id(\"com.android.application\") }}\n\nandroid {{\n    namespace = \"{application_id}\"\n    compileSdk = {ANDROID_COMPILE_SDK}\n    ndkVersion = \"{ANDROID_NDK_VERSION}\"\n    defaultConfig {{\n        applicationId = \"{application_id}\"\n        minSdk = {ANDROID_MIN_SDK}\n        targetSdk = {ANDROID_COMPILE_SDK}\n        versionCode = 1\n        versionName = \"0.0.2-dev\"\n        ndk {{ abiFilters += \"{}\" }}\n    }}\n}}\n\ndependencies {{\n    implementation(\"androidx.games:games-activity:3.0.5\")\n    implementation(\"androidx.appcompat:appcompat:1.7.1\")\n    implementation(platform(\"org.jetbrains.kotlin:kotlin-bom:1.8.22\"))\n}}\n",
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
        java_17: java_output.is_some_and(|output| output.status.success())
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

fn verify_apk_structure(apk_path: &Path, abi: AndroidDevAbi) -> Result<(), String> {
    let bytes = fs::read(apk_path).map_err(|error| error.to_string())?;
    if bytes.is_empty() {
        return Err("android_export.apk_missing_or_empty".to_string());
    }
    let native_library_entry = format!("lib/{}/libmain.so", abi.android_abi());
    for required in [
        "AndroidManifest.xml".to_string(),
        native_library_entry,
        "assets/aife/runtime-package/manifest.json".to_string(),
        "assets/aife/runtime-package-asset-manifest.json".to_string(),
    ] {
        if !contains_bytes(&bytes, required.as_bytes()) {
            return Err(format!("android_export.apk_entry_missing: {required}"));
        }
    }
    if ["arm64-v8a", "x86_64", "x86", "armeabi-v7a"]
        .iter()
        .filter(|candidate| **candidate != abi.android_abi())
        .map(|candidate| format!("lib/{candidate}/"))
        .any(|entry| contains_bytes(&bytes, entry.as_bytes()))
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

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
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

fn atomic_copy(source: &Path, destination: &Path) -> Result<(), String> {
    let temporary = destination.with_extension("apk.tmp");
    copy_file(source, &temporary)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary, destination).map_err(|error| error.to_string())
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
        let apk_fixture = b"AndroidManifest.xml lib/arm64-v8a/libmain.so assets/aife/runtime-package/manifest.json assets/aife/runtime-package-asset-manifest.json";
        let apk_path = std::env::temp_dir().join(format!("android-apk-{}.apk", unix_millis()));
        fs::write(&apk_path, apk_fixture).unwrap();
        assert_eq!(
            verify_apk_structure(&apk_path, AndroidDevAbi::Arm64V8a),
            Ok(())
        );
        fs::write(
            &apk_path,
            [apk_fixture.as_slice(), b" lib/x86/libmain.so"].concat(),
        )
        .unwrap();
        assert_eq!(
            verify_apk_structure(&apk_path, AndroidDevAbi::Arm64V8a),
            Err("android_export.apk_unexpected_abi".to_string())
        );

        let x86_fixture = b"AndroidManifest.xml lib/x86_64/libmain.so assets/aife/runtime-package/manifest.json assets/aife/runtime-package-asset-manifest.json";
        fs::write(&apk_path, x86_fixture).unwrap();
        assert_eq!(
            verify_apk_structure(&apk_path, AndroidDevAbi::X86_64),
            Ok(())
        );
        assert_eq!(
            verify_apk_structure(&apk_path, AndroidDevAbi::Arm64V8a),
            Err("android_export.apk_entry_missing: lib/arm64-v8a/libmain.so".to_string())
        );
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
            "engineVersion": "0.0.2",
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

        generate_android_project(
            &android_project,
            &request,
            &project,
            "com.aifirst.dev.towertest",
            &sdk_root,
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
        )
        .unwrap();
        let emulator_gradle =
            fs::read_to_string(emulator_project.join("app/build.gradle.kts")).unwrap();
        assert!(emulator_gradle.contains("abiFilters += \"x86_64\""));
        assert!(!emulator_gradle.contains("abiFilters += \"arm64-v8a\""));
    }
}
