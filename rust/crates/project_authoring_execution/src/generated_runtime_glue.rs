use crate::game_project_compiler::{CompilerRuntimeModuleSpec, CompilerSourceView, TargetProfile};
use project_game_sdk::PROJECT_GAME_SDK_CONTRACT_ID;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

mod rule_slots;
mod ui_views;

pub const GENERATED_RUNTIME_GLUE_REPORT_SCHEMA_VERSION: &str =
    "aife-generated-runtime-glue-report.v1";
const GENERATED_RUNTIME_GLUE_GENERATOR_ID: &str = "aife-generated-runtime-glue.v1";
const ENGINE_SDK_PATH_TOKEN: &str = "__AIFE_ENGINE_SDK__";
const PROJECT_GAME_PATH_TOKEN: &str = "__AIFE_PROJECT_GAME__";
const PROJECT_GAME_DEPENDENCY_ALIAS: &str = "project_game";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedRuntimeGlueSourceMapEntry {
    pub generated_item: String,
    pub origin_kind: String,
    pub source_path: String,
    pub source_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedRuntimeGlueReport {
    pub schema_version: String,
    pub status: String,
    pub project_game_sdk_contract: String,
    pub runtime_contract_digest: String,
    pub generator_identity: String,
    pub target_profile: String,
    pub runtime_module_source_identity: String,
    pub generation_digest: String,
    pub generated_source_path: String,
    pub generated_manifest_path: String,
    pub source_map: Vec<GeneratedRuntimeGlueSourceMapEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRuntimeGlue {
    normalized_source: String,
    ui_views: ui_views::GeneratedViews,
    normalized_manifest: String,
    report: GeneratedRuntimeGlueReport,
}

impl PreparedRuntimeGlue {
    pub fn report(&self) -> &GeneratedRuntimeGlueReport {
        &self.report
    }

    pub fn generation_digest(&self) -> &str {
        &self.report.generation_digest
    }

    #[doc(hidden)]
    pub fn materialize(
        &self,
        destination_root: &Path,
        engine_sdk_root: &Path,
        project_game_path: &Path,
    ) -> Result<(), GeneratedRuntimeGlueMaterializationError> {
        materialize_generated_runtime_glue(
            self,
            destination_root,
            engine_sdk_root,
            project_game_path,
        )
        .map_err(|error| GeneratedRuntimeGlueMaterializationError {
            code: error.code.to_string(),
            message: error.message,
            next_action: error.next_action.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedRuntimeGlueMaterializationError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedRuntimeGlueError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) next_action: &'static str,
    pub(crate) source_location: Option<Box<crate::game_project_compiler::SourceLocation>>,
}

pub(crate) fn generate_runtime_glue(
    source_view: &CompilerSourceView,
    runtime_module: Option<&CompilerRuntimeModuleSpec>,
    target_profile: TargetProfile,
) -> Result<Option<PreparedRuntimeGlue>, GeneratedRuntimeGlueError> {
    let Some(runtime_module) = runtime_module else {
        return Ok(None);
    };
    if runtime_module.project_game_sdk.trim().is_empty() {
        if runtime_module.module_id
            == engine_runtime::project_runtime_module::EMPTY_PROJECT_RUNTIME_MODULE_ID
        {
            return Ok(None);
        }
        return Err(glue_error(
            "generated_runtime_glue.sdk_contract_required",
            "ProjectRust runtimeModule requires projectGameSdk=project-game-sdk.v1.".to_string(),
            "Add the Project Game SDK contract to project.aife.json and migrate the RuntimeModule registration.",
        ));
    }
    if runtime_module.project_game_sdk != PROJECT_GAME_SDK_CONTRACT_ID {
        return Err(glue_error(
            "generated_runtime_glue.sdk_contract_unsupported",
            format!(
                "Project requests unsupported Project Game SDK contract '{}'.",
                runtime_module.project_game_sdk
            ),
            "Use projectGameSdk=project-game-sdk.v1 or a compiler that supports the requested contract.",
        ));
    }
    validate_runtime_module(runtime_module)?;

    let runtime_source_identity = runtime_module_source_identity(source_view, runtime_module)?;
    let runtime_contract_digest = project_runtime_sdk::project_runtime_contract_digest_hex();
    let slots = rule_slots::generate(source_view)?;
    let normalized_source = generated_source(
        runtime_module,
        &runtime_source_identity,
        &rule_artifact_match_arms(source_view)?,
    ).replace("__AIFE_COMPILED_RULE_SLOTS__", &slots.source)
     .replace("__AIFE_INSTALL_RULE_SLOTS__", if slots.source.is_empty() { "" } else {
         "project_game_sdk::rule_slots::install_compiled_slots(compiled_rule_slots::dispatch).expect(\"install Compiler-generated RuleSlots\");"
     });
    let mut normalized_manifest = generated_manifest(runtime_module)?;
    if !slots.source.is_empty() {
        let mut manifest: toml::Value =
            toml::from_str(&normalized_manifest).expect("generated manifest");
        manifest["dependencies"].as_table_mut().unwrap().insert(
            "engine_runtime".into(),
            dependency(
                &format!("{ENGINE_SDK_PATH_TOKEN}/crates/engine_runtime"),
                None,
            ),
        );
        manifest["dependencies"]
            .as_table_mut()
            .unwrap()
            .insert("serde_json".into(), toml::Value::String("1".into()));
        normalized_manifest = toml::to_string(&manifest).expect("generated slot manifest");
    }
    let ui_views = ui_views::generate(source_view, runtime_module)?;
    let generation_digest = generation_digest(
        &format!("{normalized_source}\n{}", ui_views.source),
        &normalized_manifest,
        &runtime_source_identity,
        &runtime_contract_digest,
        target_profile,
    );
    let registration_path = registration_source_path(runtime_module);
    let mut source_map: Vec<_> = generated_items()
        .iter()
        .map(|item| GeneratedRuntimeGlueSourceMapEntry {
            generated_item: (*item).to_string(),
            origin_kind: if *item == "aife_project_runtime_entry_v1" {
                "generator_synthetic".to_string()
            } else {
                "typed_registration".to_string()
            },
            source_path: registration_path.clone(),
            source_symbol: project_game_sdk::PROJECT_GAME_REGISTRATION_SYMBOL.to_string(),
        })
        .collect();
    source_map.extend(ui_views.source_map.iter().cloned());
    source_map.extend(slots.source_map);
    Ok(Some(PreparedRuntimeGlue {
        normalized_source,
        ui_views,
        normalized_manifest,
        report: GeneratedRuntimeGlueReport {
            schema_version: GENERATED_RUNTIME_GLUE_REPORT_SCHEMA_VERSION.to_string(),
            status: "generated".to_string(),
            project_game_sdk_contract: PROJECT_GAME_SDK_CONTRACT_ID.to_string(),
            runtime_contract_digest,
            generator_identity: GENERATED_RUNTIME_GLUE_GENERATOR_ID.to_string(),
            target_profile: target_profile.as_str().to_string(),
            runtime_module_source_identity: runtime_source_identity,
            generation_digest,
            generated_source_path: "RuntimeGlue/src/lib.rs".to_string(),
            generated_manifest_path: "RuntimeGlue/Cargo.toml".to_string(),
            source_map,
        },
    }))
}

pub(crate) fn materialize_generated_runtime_glue(
    artifact: &PreparedRuntimeGlue,
    destination_root: &Path,
    engine_sdk_root: &Path,
    project_game_path: &Path,
) -> Result<(), GeneratedRuntimeGlueError> {
    let source_root = destination_root.join("src");
    fs::create_dir_all(&source_root).map_err(|error| {
        glue_error(
            "generated_runtime_glue.materialize_failed",
            format!("Generated source directory cannot be created: {error}"),
            "Use a writable Compiler-owned staging directory and retry prepare.",
        )
    })?;
    let engine_sdk_path = cargo_manifest_path(engine_sdk_root);
    let project_game_path = cargo_manifest_path(project_game_path);
    let manifest = artifact
        .normalized_manifest
        .replace(ENGINE_SDK_PATH_TOKEN, &engine_sdk_path)
        .replace(PROJECT_GAME_PATH_TOKEN, &project_game_path);
    fs::write(source_root.join("lib.rs"), &artifact.normalized_source).map_err(|error| {
        glue_error(
            "generated_runtime_glue.materialize_failed",
            format!("Generated source cannot be written: {error}"),
            "Use a writable Compiler-owned staging directory and retry prepare.",
        )
    })?;
    fs::write(destination_root.join("Cargo.toml"), manifest).map_err(|error| {
        glue_error(
            "generated_runtime_glue.materialize_failed",
            format!("Generated Cargo manifest cannot be written: {error}"),
            "Use a writable Compiler-owned staging directory and retry prepare.",
        )
    })?;
    fs::write(
        destination_root.join("ui_views.rs"),
        &artifact.ui_views.source,
    )
    .map_err(|error| {
        glue_error(
            "generated_runtime_glue.materialize_failed",
            format!("Generated UI references cannot be written: {error}"),
            "Use a writable Compiler-owned staging directory and retry prepare.",
        )
    })?;
    let report = serde_json::to_vec_pretty(&artifact.report).map_err(|error| {
        glue_error(
            "generated_runtime_glue.report_encode_failed",
            format!("Generation report cannot be encoded: {error}"),
            "Report this as an Engine generator defect; do not edit generated files.",
        )
    })?;
    fs::write(
        destination_root.join("generated-runtime-glue-report.json"),
        report,
    )
    .map_err(|error| {
        glue_error(
            "generated_runtime_glue.materialize_failed",
            format!("Generation report cannot be written: {error}"),
            "Use a writable Compiler-owned staging directory and retry prepare.",
        )
    })
}

fn cargo_manifest_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if let Some(unc_path) = normalized.strip_prefix("//?/UNC/") {
        return format!("//{unc_path}");
    }
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn validate_runtime_module(
    runtime_module: &CompilerRuntimeModuleSpec,
) -> Result<(), GeneratedRuntimeGlueError> {
    for (field, value) in [
        ("moduleId", runtime_module.module_id.as_str()),
        (
            "interfaceVersion",
            runtime_module.interface_version.as_str(),
        ),
        ("cargoManifest", runtime_module.cargo_manifest.as_str()),
        ("cargoPackage", runtime_module.cargo_package.as_str()),
        ("playerBinary", runtime_module.player_binary.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(glue_error(
                "generated_runtime_glue.manifest_field_missing",
                format!("runtimeModule.{field} is required for generated runtime glue."),
                "Repair project.aife.json and acquire a new operation-bound snapshot lease.",
            ));
        }
    }
    if runtime_module.cargo_manifest != "RuntimeModule/Cargo.toml" {
        return Err(glue_error(
            "generated_runtime_glue.cargo_manifest_unsupported",
            "Generated runtime glue v1 requires RuntimeModule/Cargo.toml.".to_string(),
            "Move the project gameplay crate to RuntimeModule or use a future compiler contract.",
        ));
    }
    Ok(())
}

fn runtime_module_source_identity(
    source_view: &CompilerSourceView,
    runtime_module: &CompilerRuntimeModuleSpec,
) -> Result<String, GeneratedRuntimeGlueError> {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"runtime-module-source-identity.v1");
    for field in [
        runtime_module.module_id.as_bytes(),
        runtime_module.interface_version.as_bytes(),
        runtime_module.cargo_manifest.as_bytes(),
        runtime_module.cargo_package.as_bytes(),
        runtime_module.player_binary.as_bytes(),
        runtime_module.project_game_sdk.as_bytes(),
    ] {
        hash_field(&mut hasher, field);
    }
    let mut source_count = 0usize;
    for path in source_view.paths().filter(|path| {
        *path == "RuntimeModule/Cargo.toml"
            || *path == "RuntimeModule/Cargo.lock"
            || path.starts_with("RuntimeModule/src/")
            || path.starts_with("RuntimeModule/tests/")
    }) {
        let bytes = source_view.bytes(path).ok_or_else(|| {
            glue_error(
                "generated_runtime_glue.source_missing",
                format!("Leased runtime source disappeared: {path}"),
                "Acquire a new operation-bound snapshot lease.",
            )
        })?;
        hash_field(&mut hasher, path.as_bytes());
        hash_field(&mut hasher, bytes);
        source_count += 1;
    }
    if source_count == 0 {
        return Err(glue_error(
            "generated_runtime_glue.runtime_source_missing",
            "The snapshot lease contains no RuntimeModule source.".to_string(),
            "Include RuntimeModule/Cargo.toml and RuntimeModule/src in the compiler snapshot lease.",
        ));
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn generated_manifest(
    runtime_module: &CompilerRuntimeModuleSpec,
) -> Result<String, GeneratedRuntimeGlueError> {
    let mut root = toml::map::Map::new();
    let mut package = toml::map::Map::new();
    package.insert(
        "name".to_string(),
        toml::Value::String("aife_generated_runtime_glue".to_string()),
    );
    package.insert(
        "version".to_string(),
        toml::Value::String("0.1.0".to_string()),
    );
    package.insert(
        "edition".to_string(),
        toml::Value::String("2021".to_string()),
    );
    package.insert("publish".to_string(), toml::Value::Boolean(false));
    root.insert("package".to_string(), toml::Value::Table(package));

    let mut library = toml::map::Map::new();
    library.insert(
        "path".to_string(),
        toml::Value::String("src/lib.rs".to_string()),
    );
    library.insert(
        "crate-type".to_string(),
        toml::Value::Array(
            ["rlib", "cdylib"]
                .into_iter()
                .map(|value| toml::Value::String(value.to_string()))
                .collect(),
        ),
    );
    root.insert("lib".to_string(), toml::Value::Table(library));

    let mut features = toml::map::Map::new();
    features.insert("default".to_string(), toml::Value::Array(Vec::new()));
    features.insert("static-link".to_string(), toml::Value::Array(Vec::new()));
    root.insert("features".to_string(), toml::Value::Table(features));

    let mut dependencies = toml::map::Map::new();
    dependencies.insert(
        PROJECT_GAME_DEPENDENCY_ALIAS.to_string(),
        dependency(
            PROJECT_GAME_PATH_TOKEN,
            Some(runtime_module.cargo_package.as_str()),
        ),
    );
    for dependency_name in [
        "project_game_sdk",
        "project_runtime_abi",
        "project_runtime_sdk",
    ] {
        dependencies.insert(
            dependency_name.to_string(),
            dependency(
                &format!("{ENGINE_SDK_PATH_TOKEN}/crates/{dependency_name}"),
                None,
            ),
        );
    }
    root.insert("dependencies".to_string(), toml::Value::Table(dependencies));
    toml::to_string(&toml::Value::Table(root)).map_err(|error| {
        glue_error(
            "generated_runtime_glue.manifest_encode_failed",
            format!("Generated Cargo manifest cannot be encoded: {error}"),
            "Report this as an Engine generator defect; do not edit generated files.",
        )
    })
}

fn dependency(path: &str, package: Option<&str>) -> toml::Value {
    let mut value = toml::map::Map::new();
    value.insert("path".to_string(), toml::Value::String(path.to_string()));
    if let Some(package) = package {
        value.insert(
            "package".to_string(),
            toml::Value::String(package.to_string()),
        );
    }
    toml::Value::Table(value)
}

fn generated_source(
    runtime_module: &CompilerRuntimeModuleSpec,
    runtime_source_identity: &str,
    rule_artifact_match_arms: &str,
) -> String {
    include_str!("generated_runtime_glue_template.rs.txt")
        .replace(
            "__AIFE_MODULE_ID__",
            &rust_string(&runtime_module.module_id),
        )
        .replace(
            "__AIFE_INTERFACE_VERSION__",
            &rust_string(&runtime_module.interface_version),
        )
        .replace(
            "__AIFE_AOT_CONTENT_DIGEST__",
            &rust_string(runtime_source_identity),
        )
        .replace(
            "__AIFE_RULE_ARTIFACT_MATCH_ARMS__",
            rule_artifact_match_arms,
        )
}

fn rule_artifact_match_arms(
    source_view: &CompilerSourceView,
) -> Result<String, GeneratedRuntimeGlueError> {
    let Some(bytes) = source_view.bytes("Rules/rule-manifest.json") else {
        return Ok(String::new());
    };
    let manifest: engine_runtime::runtime_package::RuntimeRuleManifest =
        serde_json::from_slice(bytes).map_err(|error| {
            glue_error(
                "generated_runtime_glue.rule_manifest_invalid",
                format!("Rules/rule-manifest.json cannot be decoded: {error}"),
                "Repair the leased Rule manifest and prepare the project again.",
            )
        })?;
    let mut artifacts = std::collections::BTreeMap::new();
    for rule in manifest.rules {
        let Some(artifact_id) = rule.artifact_id else {
            continue;
        };
        artifacts.insert(rule.rule_id, artifact_id);
    }
    Ok(artifacts
        .into_iter()
        .map(|(rule_id, artifact_id)| {
            format!(
                "        {} => {}.to_string(),\n",
                rust_string(&rule_id),
                rust_string(&artifact_id)
            )
        })
        .collect())
}

fn registration_source_path(runtime_module: &CompilerRuntimeModuleSpec) -> String {
    runtime_module
        .cargo_manifest
        .strip_suffix("Cargo.toml")
        .map(|root| format!("{root}src/lib.rs"))
        .unwrap_or_else(|| "RuntimeModule/src/lib.rs".to_string())
}

fn generated_items() -> &'static [&'static str] {
    &[
        "descriptor",
        "create_session",
        "destroy_session",
        "session_id",
        "invoke_rule",
        "handle_aui_actions",
        "fixed_update",
        "lifecycle",
        "ui_event",
        "resolve_ui_state",
        "observe",
        "aife_project_runtime_entry_v1",
    ]
}

fn generation_digest(
    normalized_source: &str,
    normalized_manifest: &str,
    runtime_source_identity: &str,
    runtime_contract_digest: &str,
    target_profile: TargetProfile,
) -> String {
    let mut hasher = Sha256::new();
    for field in [
        GENERATED_RUNTIME_GLUE_GENERATOR_ID.as_bytes(),
        PROJECT_GAME_SDK_CONTRACT_ID.as_bytes(),
        runtime_contract_digest.as_bytes(),
        runtime_source_identity.as_bytes(),
        target_profile.as_str().as_bytes(),
        normalized_manifest.as_bytes(),
        normalized_source.as_bytes(),
    ] {
        hash_field(&mut hasher, field);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn glue_error(
    code: &'static str,
    message: String,
    next_action: &'static str,
) -> GeneratedRuntimeGlueError {
    GeneratedRuntimeGlueError {
        code,
        message,
        next_action,
        source_location: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Source diagnostic at the existing generator seam. No asset assembly or Cook.
    #[cfg(windows)]
    #[test]
    #[ignore = "requires an explicit existing ordinary project/package and fresh output"]
    fn materialize_existing_project_module_without_recooking_assets() {
        use crate::ProjectAuthoringSession;
        use std::path::PathBuf;
        let project = PathBuf::from(
            std::env::var_os("AIFE_MODULE_REBUILD_SOURCE").expect("ordinary project"),
        );
        let package = PathBuf::from(
            std::env::var_os("AIFE_MODULE_REBUILD_PACKAGE").expect("unchanged cooked package"),
        );
        let output = PathBuf::from(
            std::env::var_os("AIFE_MODULE_REBUILD_EVIDENCE").expect("fresh evidence"),
        );
        fs::create_dir(&output).unwrap();
        let started = std::time::Instant::now();
        let package_digest = runtime_cli::runtime_package_digest(&package).unwrap();
        let expected = engine_runtime::runtime_package::load_runtime_package(&package)
            .value
            .unwrap()
            .manifest
            .project
            .runtime_module;
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("module-only-regeneration", paths)
            .unwrap();
        let source = CompilerSourceView::from_snapshot(lease.snapshot()).unwrap();
        let spec: CompilerRuntimeModuleSpec = serde_json::from_value(
            source.project_manifest_value().unwrap()["runtimeModule"].clone(),
        )
        .unwrap();
        let glue = generate_runtime_glue(&source, Some(&spec), TargetProfile::WindowsRelease)
            .unwrap()
            .unwrap();
        assert_eq!(
            glue.report.runtime_module_source_identity, expected.aot_content_digest,
            "changed project logic requires a new package; cannot reuse"
        );
        fs::write(
            output.join("glue.json"),
            serde_json::to_vec_pretty(glue.report()).unwrap(),
        )
        .unwrap();
        let generated = output.join("RuntimeGlue");
        glue.materialize(
            &generated,
            &crate::default_engine_sdk_root(),
            &project.join("RuntimeModule"),
        )
        .unwrap();
        let lock = PathBuf::from(
            std::env::var_os("AIFE_MODULE_REBUILD_LOCK")
                .expect("previous producer lock, unchanged dependency graph"),
        );
        fs::copy(&lock, generated.join("Cargo.lock")).unwrap();
        fs::write(output.join("timings.json"),serde_json::to_vec_pretty(&serde_json::json!({"prepareMs":started.elapsed().as_millis(),"packageDigest":package_digest,"lockSource":lock,"scope":"generator output only; compile with --locked, original project/package unchanged; no delivery qualification"})).unwrap()).unwrap();
        assert_eq!(
            runtime_cli::runtime_package_digest(&package).unwrap(),
            package_digest
        );
    }
    use std::collections::BTreeMap;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    pub(super) fn runtime_module() -> CompilerRuntimeModuleSpec {
        CompilerRuntimeModuleSpec {
            module_id: "fixture.game.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            cargo_manifest: "RuntimeModule/Cargo.toml".to_string(),
            cargo_package: "fixture_game_runtime".to_string(),
            player_binary: "fixture_game_player".to_string(),
            project_game_sdk: PROJECT_GAME_SDK_CONTRACT_ID.to_string(),
        }
    }

    pub(super) fn source_view(gameplay_source: &str, scene_source: &str) -> CompilerSourceView {
        CompilerSourceView {
            files: BTreeMap::from([
                (
                    "RuntimeModule/Cargo.toml".to_string(),
                    b"[package]\nname='fixture_game_runtime'".to_vec(),
                ),
                (
                    "RuntimeModule/src/lib.rs".to_string(),
                    gameplay_source.as_bytes().to_vec(),
                ),
                (
                    "Scenes/Main.scene.json".to_string(),
                    scene_source.as_bytes().to_vec(),
                ),
            ]),
        }
    }

    #[test]
    fn generated_runtime_glue_is_deterministic_and_path_neutral() {
        let first = generate_runtime_glue(
            &source_view("fn project_game() {}", "scene-a"),
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        let rebuilt = generate_runtime_glue(
            &source_view("fn project_game() {}", "scene-b"),
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        assert_eq!(first.normalized_source, rebuilt.normalized_source);
        assert_eq!(first.normalized_manifest, rebuilt.normalized_manifest);
        assert_eq!(
            first.report.generation_digest,
            rebuilt.report.generation_digest
        );
        assert!(!first.normalized_manifest.contains("G:\\"));
        assert!(first.normalized_manifest.contains(ENGINE_SDK_PATH_TOKEN));
    }

    #[cfg(windows)]
    #[test]
    fn cargo_manifest_path_removes_windows_extended_path_prefix() {
        assert_eq!(
            cargo_manifest_path(Path::new(r"\\?\G:\gameEngin\rust")),
            "G:/gameEngin/rust"
        );
        assert_eq!(
            cargo_manifest_path(Path::new(r"\\?\UNC\server\share\rust")),
            "//server/share/rust"
        );
    }

    #[test]
    fn runtime_source_or_target_changes_invalidate_generation() {
        let windows = generate_runtime_glue(
            &source_view("fn project_game() { one(); }", "scene"),
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        let changed_source = generate_runtime_glue(
            &source_view("fn project_game() { two(); }", "scene"),
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        let android = generate_runtime_glue(
            &source_view("fn project_game() { one(); }", "scene"),
            Some(&runtime_module()),
            TargetProfile::AndroidDev,
        )
        .unwrap()
        .unwrap();
        assert_ne!(
            windows.report.generation_digest,
            changed_source.report.generation_digest
        );
        assert_ne!(
            windows.report.generation_digest,
            android.report.generation_digest
        );
    }

    #[test]
    fn rule_artifact_identity_is_generated_from_the_leased_manifest() {
        let mut first_source = source_view("fn project_game() {}", "scene");
        first_source.files.insert(
            "Rules/rule-manifest.json".to_string(),
            rule_manifest("artifact-a").into_bytes(),
        );
        let first = generate_runtime_glue(
            &first_source,
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        assert!(first
            .normalized_source
            .contains(r#""rule.fixture" => "rule-artifact:rule.fixture:artifact-a".to_string()"#));

        let mut changed_source = source_view("fn project_game() {}", "scene");
        changed_source.files.insert(
            "Rules/rule-manifest.json".to_string(),
            rule_manifest("artifact-b").into_bytes(),
        );
        let changed = generate_runtime_glue(
            &changed_source,
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            first.report.runtime_module_source_identity,
            changed.report.runtime_module_source_identity
        );
        assert_ne!(
            first.report.generation_digest,
            changed.report.generation_digest
        );
    }

    #[test]
    fn source_map_covers_every_generated_callback() {
        let artifact = generate_runtime_glue(
            &source_view("fn project_game() {}", "scene"),
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        assert_eq!(artifact.report.source_map.len(), generated_items().len());
        assert!(artifact.report.source_map.iter().all(|entry| {
            entry.source_path == "RuntimeModule/src/lib.rs" && entry.source_symbol == "project_game"
        }));
    }

    #[test]
    fn project_rust_without_sdk_requirement_fails_closed() {
        let mut module = runtime_module();
        module.project_game_sdk.clear();
        let error = generate_runtime_glue(
            &source_view("legacy", "scene"),
            Some(&module),
            TargetProfile::WindowsDev,
        )
        .unwrap_err();
        assert_eq!(error.code, "generated_runtime_glue.sdk_contract_required");
        assert!(error.message.contains("projectGameSdk"));
    }

    #[test]
    fn generated_runtime_glue_compiles_and_exposes_the_runtime_entry() {
        let root = temp_root("compile-entry");
        let _cleanup = TestDirectoryGuard(root.clone());
        let runtime_root = root.join("RuntimeModule");
        let glue_root = root.join("RuntimeGlue");
        let smoke_root = root.join("Smoke");
        fs::create_dir_all(runtime_root.join("src")).unwrap();
        fs::create_dir_all(smoke_root.join("src")).unwrap();
        let engine_sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("rust workspace root");
        let engine_sdk_path = engine_sdk_root.to_string_lossy().replace('\\', "/");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=['RuntimeModule','RuntimeGlue','Smoke']\nresolver='2'\n",
        )
        .unwrap();
        fs::write(
            runtime_root.join("Cargo.toml"),
            format!(
                "[package]\nname='fixture_game_runtime'\nversion='0.1.0'\nedition='2021'\npublish=false\n\n[dependencies]\nproject_game_sdk={{path='{engine_sdk_path}/crates/project_game_sdk'}}\n"
            ),
        )
        .unwrap();
        let project_source = r#"use project_game_sdk::aui::UiRead;
use project_game_sdk::{
    GameCallContext, GameResult, HandlerStatus, ProjectGameDefinition, ProjectGameSession,
    RuleOutput, RuleRequest, SessionCreateRequest, UiBindingStore, UiStateResolveOutput,
    UiStateResolveRequest, Value,
};

mod views {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../RuntimeGlue/ui_views.rs"));
}
pub struct Session { ticket:Option<project_game_sdk::SpawnTicket>, view:Option<views::SettingsViewRefs>, events:u32, hp:i64, ui:UiBindingStore, animation:Option<project_game_sdk::aui_animation::AnimationHandle> }

impl ProjectGameSession for Session {
    fn session_id(&self) -> &str { "fixture.session" }
}

fn create(_: &SessionCreateRequest) -> GameResult<Session> { Ok(Session {ticket:None, view:None, events:0, hp:3, ui:UiBindingStore::new("fixture.ui", 1)?, animation:None}) }

fn rule(
    _: &mut Session,
    ctx: &mut project_game_sdk::RuleContext<'_, '_>,
    _: &RuleRequest,
) -> GameResult<RuleOutput> {
    ctx.mutations().instantiate_prefab("fixture.prefab");
    Ok(RuleOutput {
        status: HandlerStatus::Applied,
        mutations: Vec::new(),
        diagnostics: Vec::new(),
    })
}

pub fn project_game() -> ProjectGameDefinition<Session> {
    ProjectGameDefinition::new(create).rule_typed("rule.fixture", rule).lifecycle(lifecycle).ui_event(7, ui_event).ui_extension(ui_extension_descriptor(), ui_extension).ui_state(ui_state)
}
fn ui_state(session: &mut Session, _: &mut GameCallContext<'_>, request: &UiStateResolveRequest) -> GameResult<UiStateResolveOutput> {
    let plan = session.ui.prepare_resolve(request)?;
    let refreshed = plan.refresh_paths().iter().map(|path| {
        let value = match path.as_str() {
            "fixture.hp" => Some(Value::Integer(session.hp)),
            _ => None,
        };
        (path.clone(), value)
    }).collect::<Vec<_>>();
    session.ui.finish_resolve(plan, refreshed)
}
fn ui_extension_descriptor() -> project_game_sdk::aui_extension::UiExtensionTypeDescriptor {
    project_game_sdk::aui_extension::UiExtensionTypeDescriptor {
        type_id: "fixture.circle".into(), schema_version: 1, callback_id: 9,
        properties: vec![], measure: true, layout: false, paint: true,
    }
}
fn ui_extension(_: &mut Session, request: &project_game_sdk::aui_extension::UiExtensionRequest) -> GameResult<project_game_sdk::aui_extension::UiExtensionOutput> {
    use project_game_sdk::aui_extension::*;
    match request.input {
        UiExtensionInput::Measure { .. } => Ok(UiExtensionOutput::Measure { output: UiExtensionMeasureOutput { size:[32.,32.], first_baseline:None } }),
        UiExtensionInput::Paint { .. } => {
            use project_game_sdk::aui_draw2d::{Draw2D, Path2D, FillRule};
            let mut draw = Draw2D::new();
            draw.fill(Path2D::circle([16.,16.],12.).unwrap(),[0.,1.,0.,1.],FillRule::EvenOdd).unwrap();
            Ok(UiExtensionOutput::Paint { output: UiExtensionPaintOutput::from_graphics(draw.finish()) })
        },
        _ => Err(project_game_sdk::GameError::new(project_game_sdk::GameErrorCode::Unsupported, "unexpected extension stage")),
    }
}
fn ui_event(session: &mut Session, ctx: &mut GameCallContext<'_>, request: &project_game_sdk::aui::UiEventRequest) -> GameResult<project_game_sdk::SessionOutput> {
    assert_eq!(request.callback_id, project_game_sdk::aui::UiCallbackId(7));
    if let project_game_sdk::aui::UiNotificationKind::Animation { handle, outcome } = request.notification.kind {
        assert_eq!(Some(handle), session.animation);
        let ui=ctx.ui()?;
        let label=ui.text("settings", "label")?;
        label.set_text(ui, match outcome {
            project_game_sdk::aui_animation::AnimationOutcome::Completed => "动画完成",
            project_game_sdk::aui_animation::AnimationOutcome::Cancelled => "动画取消",
            _ => panic!("unexpected animation outcome"),
        })?;
        let mut output=project_game_sdk::SessionOutput::no_op(); output.status=HandlerStatus::Applied;
        return Ok(output);
    }
    session.events += 1;
    ctx.mutations().instantiate_prefab(format!("event.prefab.{}", session.events));
    let mut output = project_game_sdk::SessionOutput::no_op();
    output.status = HandlerStatus::Applied;
    Ok(output)
}
fn lifecycle(session: &mut Session, ctx: &mut GameCallContext<'_>, request: &project_game_sdk::LifecycleRequest) -> GameResult<project_game_sdk::SessionOutput> {
    let mut output = project_game_sdk::SessionOutput::no_op();
    if (200..=205).contains(&request.frame.frame_index) {
        use project_game_sdk::aui_animation::*;
        let ui=ctx.ui()?;
        let target=ui.node("settings", "label")?.handle();
        match request.frame.frame_index {
            200 | 204 => {
                let node=project_game_sdk::aui::UiNodeRef::new(target);
                node.set_opacity(ui, 0.8)?;
                session.animation=Some(ui.play_animation(AnimationPlay {
                    owner:target, scope:project_game_sdk::aui::UiSubscriptionScope::Active,
                    clock:AnimationClock::Ui, conflict:TrackConflictPolicy::Reject,
                    callback_id:Some(project_game_sdk::aui::UiCallbackId(7)),
                    animation:Animation2D { version:ANIMATION_2D_VERSION,
                        root:AnimationStep::Tween { track:TweenTrack { target, property:TweenProperty::Translation,
                            from:TweenValue::Vec2([0.,0.]), to:TweenValue::Vec2([100.,0.]),
                            duration_us:10_000, delay_us:0, easing:Easing::Linear } },
                        repeat:Repeat::Once, direction:PlaybackDirection::Forward, completion:CompletionPolicy::Hold,
                    },
                })?);
                node.set_tint(ui, [0.7,1.,1.,1.])?;
            },
            201 => ui.pause_animation(session.animation.unwrap(), true)?,
            202 => ui.pause_animation(session.animation.unwrap(), false)?,
            203 => ui.cancel_animation(session.animation.unwrap())?,
            _ => {},
        }
        output.status=if request.frame.frame_index==204 {HandlerStatus::NoOp} else {HandlerStatus::Applied};
        return Ok(output);
    }
    if request.frame.frame_index >= 100 {
        let ui = ctx.ui()?;
        let view = match session.view {
            Some(view) => view,
            None => { let view = views::SettingsViewRefs::resolve(ui)?; session.view = Some(view); view }
        };
        let label = view.title;
        let screen=label.node().local_to_screen(ui,[3.0,4.0])?;
        let local=label.node().screen_to_local(ui,screen)?;
        assert!((local[0]-3.0).abs()<0.001 && (local[1]-4.0).abs()<0.001);
        label.set_text(ui, "骑兵")?;
        assert_eq!(label.text(ui)?, "骑兵");
        label.node().set_rotation(ui, 0.25)?;
        label.node().set_scale(ui, [-1.0, 1.5])?;
        label.node().set_pivot(ui, [0.0, 1.0])?;
        label.node().set_opacity(ui, 0.6)?;
        label.node().set_tint(ui, [0.8, 1.0, 0.5, 1.0])?;
        assert_eq!(ui.read(label.node().handle(), project_game_sdk::aui::UiProperty::Rotation)?, project_game_sdk::aui::UiValue::Number(0.25));
        let button = view.start;
        button.set_interactable(ui, false)?;
        match request.frame.frame_index {
            101 => output.status = HandlerStatus::NoOp,
            102 => return Err(project_game_sdk::GameError::new(project_game_sdk::GameErrorCode::Failed, "project UI failure")),
            103 => panic!("project UI panic"),
            104 => {
                let _ = label.node().set_translation(ui, [f32::NAN, 0.]);
                output.status = HandlerStatus::Applied;
            }
            _ => output.status = HandlerStatus::Applied,
        }
        return Ok(output);
    }
    if matches!(request.event, project_game_sdk::LifecycleEvent::FocusChanged { focused: true }) {
        let content = ctx.world().query_generated_content(project_game_sdk::generated_content::ContentQuery::Request { request_id: 1 })?;
        assert_eq!(content.unwrap_err().code, project_game_sdk::generated_content::ContentErrorCode::CapabilityGap);
        let storage = ctx.world().query_project_storage(project_game_sdk::generated_content::StorageQuery::Request { request_id: 2 })?;
        assert_eq!(storage.unwrap_err().code, project_game_sdk::generated_content::ContentErrorCode::CapabilityGap);
        let bytes = ctx.world().take_project_bytes(project_game_sdk::generated_content::ContentTicket { session_generation: 1, request_id: 2 })?;
        assert_eq!(bytes.unwrap_err().code, project_game_sdk::generated_content::ContentErrorCode::CapabilityGap);
        session.hp = 2;
        session.ui.notify("fixture.hp")?;
        session.ticket=Some(ctx.mutations().instantiate_spatial_prefab(7,project_game_sdk::SpatialAssetRef {id:"p".into(),asset_type:"prefab".into(),guid:Some("guid-p".into()),sub_asset:None})?);
        output.status=HandlerStatus::Applied;
    }
    if matches!(request.event, project_game_sdk::LifecycleEvent::FocusChanged { focused: false }) {
        let camera=ctx.world().resolve_entity("camera")?;
        let mut transform=ctx.world().read_transform3d(&camera)?;
        transform.translation[0]+=1.0;
        ctx.mutations().write_local_transform3d(&camera,transform)?;
        let root=ctx.world().resolve_spawn(session.ticket.expect("earlier request"))?.expect("committed prior request fixture");
        ctx.mutations().select_main_camera(&root)?;
        ctx.request_exit(); output.status = HandlerStatus::Applied;
    }
    Ok(output)
}
"#;
        fs::write(runtime_root.join("src/lib.rs"), project_source).unwrap();

        let mut source_view = CompilerSourceView {
            files: BTreeMap::from([
                (
                    "RuntimeModule/Cargo.toml".to_string(),
                    fs::read(runtime_root.join("Cargo.toml")).unwrap(),
                ),
                (
                    "RuntimeModule/src/lib.rs".to_string(),
                    project_source.as_bytes().to_vec(),
                ),
            ]),
        };
        ui_views::tests::add_views(&mut source_view);
        let artifact = generate_runtime_glue(
            &source_view,
            Some(&runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        materialize_generated_runtime_glue(
            &artifact,
            &glue_root,
            engine_sdk_root,
            Path::new("../RuntimeModule"),
        )
        .unwrap();

        fs::write(
            smoke_root.join("Cargo.toml"),
            format!(
                "[package]\nname='generated_glue_smoke'\nversion='0.1.0'\nedition='2021'\npublish=false\n\n[dependencies]\naife_generated_runtime_glue={{path='../RuntimeGlue'}}\nproject_runtime_abi={{path='{engine_sdk_path}/crates/project_runtime_abi'}}\nproject_runtime_sdk={{path='{engine_sdk_path}/crates/project_runtime_sdk'}}\nserde_json='1'\n"
            ),
        )
        .unwrap();
        fs::write(
            smoke_root.join("src/main.rs"),
            r#"use project_runtime_abi::ProjectRuntimeOpaqueHandle;
use project_runtime_sdk::{call_json, ProjectRuntimeModuleDescriptor};
use project_runtime_abi::{ProjectRuntimeByteSlice,ProjectRuntimeByteBuffer,ProjectRuntimeAbiStatus};
unsafe extern "C" fn query(_:ProjectRuntimeOpaqueHandle,request:ProjectRuntimeByteSlice,output:*mut ProjectRuntimeByteBuffer)->ProjectRuntimeAbiStatus {
    if let Ok(request) = unsafe { project_runtime_sdk::read_input::<project_runtime_sdk::project_storage::StorageTakeRequest>(request) } {
        let mut error=project_runtime_sdk::generated_content::ContentError::invalid("storage.owner");
        error.code=project_runtime_sdk::generated_content::ContentErrorCode::CapabilityGap;
        error.request_id=request.project_storage_take.request_id;
        return unsafe { project_runtime_sdk::project_storage::write_project_bytes_reply(output,Err(&error)) };
    }
    project_runtime_sdk::ffi_boundary(output,|| {
        let value:serde_json::Value=unsafe { project_runtime_sdk::read_input(request)? };
        if value.get("projectStorage").is_some() {
            let request:project_runtime_sdk::project_storage::StorageQueryRequest=serde_json::from_value(value).map_err(|_|ProjectRuntimeAbiStatus::INVALID_ARGUMENT)?;
            assert_eq!(request.project_storage,project_runtime_sdk::project_storage::StorageQuery::Request {request_id:2});
            let mut error=project_runtime_sdk::generated_content::ContentError::invalid("storage.owner");
            error.code=project_runtime_sdk::generated_content::ContentErrorCode::CapabilityGap;
            return Ok(serde_json::to_value(project_runtime_sdk::project_storage::StorageQueryReply::Err(error)).unwrap());
        }
        if value.get("generatedContent").is_some() {
            let request:project_runtime_sdk::generated_content::ContentQueryRequest=serde_json::from_value(value).map_err(|_|ProjectRuntimeAbiStatus::INVALID_ARGUMENT)?;
            assert_eq!(request.generated_content,project_runtime_sdk::generated_content::ContentQuery::Request {request_id:1});
            let mut error=project_runtime_sdk::generated_content::ContentError::invalid("generatedContent.owner");
            error.code=project_runtime_sdk::generated_content::ContentErrorCode::CapabilityGap;
            return Ok(serde_json::to_value(project_runtime_sdk::generated_content::ContentQueryReply::Err(error)).unwrap());
        }
        let request:project_runtime_sdk::ProjectRuntimeWorldQueryRequest=serde_json::from_value(value).map_err(|_|ProjectRuntimeAbiStatus::INVALID_ARGUMENT)?;
        let (id,generation)=if request.spawn_request_id==Some(7) {("instance-actual-root",8589934593)} else {("camera",4294967297)};
        Ok(serde_json::to_value(project_runtime_sdk::ProjectRuntimeWorldQueryResponse {physics: None,cell: None,entity_ids:vec![id.into()],entity_generations:std::collections::BTreeMap::from([(id.into(),generation)])}).unwrap())
    })
}
unsafe extern "C" fn read(_:ProjectRuntimeOpaqueHandle,request:ProjectRuntimeByteSlice,output:*mut ProjectRuntimeByteBuffer)->ProjectRuntimeAbiStatus {
    project_runtime_sdk::ffi_boundary(output,|| {
        let request:project_runtime_sdk::ProjectRuntimeWorldReadRequest=unsafe { project_runtime_sdk::read_input(request)? };
        assert_eq!(request.entity_id,"camera");assert_eq!(request.generation,4294967297);assert_eq!(request.component_type,"engine.transform3d");
        use project_runtime_sdk::ProjectRuntimeValue as V;
        Ok(project_runtime_sdk::ProjectRuntimeWorldReadResponse {value:V::Object(std::collections::BTreeMap::from([
            ("translation".into(),V::Vec3([0.,0.,6.])),("scale".into(),V::Vec3([1.;3])),
            ("rotationXyzw".into(),V::Array(vec![V::Number(0.),V::Number(0.),V::Number(0.),V::Number(1.)])),
        ]))})
    })
}
fn main() {
    let api = unsafe { *aife_generated_runtime_glue::aife_project_runtime_entry_v1() };
    let descriptor: ProjectRuntimeModuleDescriptor = call_json(
        api.descriptor.expect("descriptor callback"),
        api.module_context,
        ProjectRuntimeOpaqueHandle::NULL,
        None,
        &(),
    )
    .expect("descriptor result");
    assert_eq!(descriptor.module_id, "fixture.game.runtime");
    assert_eq!(descriptor.rules[0].rule_id, "rule.fixture");
    let created: project_runtime_sdk::ProjectRuntimeSessionCreateResponse = call_json(
        api.create_session.unwrap(), api.module_context, ProjectRuntimeOpaqueHandle::NULL, None,
        &project_runtime_sdk::ProjectRuntimeSessionCreateRequest { project_id: "fixture".into(), module_id: "fixture.game.runtime".into() },
    ).unwrap();
    let session = ProjectRuntimeOpaqueHandle { value: created.handle_value, generation: created.handle_generation };
    let context = project_runtime_abi::ProjectRuntimeCallContext {
        struct_size: project_runtime_abi::PROJECT_RUNTIME_CALL_CONTEXT_STRUCT_SIZE, reserved: 0,
        host_context: ProjectRuntimeOpaqueHandle::NULL, host_api: std::ptr::null(), frame_index: 1, time: Default::default(),
    };
    let request = project_runtime_sdk::ProjectRuntimeLifecycleRequest {
        frame: project_runtime_sdk::ProjectRuntimeFrameRequest { frame_index: 1, time: Default::default() },
        event: project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged { focused: false },
    };
    // A valid host table is required by AbiWorldRead even when this handler does not query it.
    let host_api = project_runtime_abi::ProjectRuntimeHostApi { struct_size: project_runtime_abi::PROJECT_RUNTIME_HOST_API_STRUCT_SIZE, reserved: 0, world_query: Some(query), world_read_component: Some(read), ui_query: None, ui_stage: None, ui_subscribe: None, ui_cancel: None, ui_structure: None, ui_invalidate: None, spatial_resolve: None };
    let context = project_runtime_abi::ProjectRuntimeCallContext { host_api: &host_api, ..context };
    assert!(api.fixed_update.is_none());
    assert_eq!(api.capabilities & project_runtime_abi::PROJECT_RUNTIME_CAP_FIXED_UPDATE, 0);
    assert_ne!(api.capabilities & project_runtime_abi::PROJECT_RUNTIME_CAP_UI_EVENTS, 0);
    assert_ne!(api.capabilities & project_runtime_abi::PROJECT_RUNTIME_CAP_UI_EXTENSIONS, 0);
    assert_ne!(api.capabilities & project_runtime_abi::PROJECT_RUNTIME_CAP_UI_STATE, 0);
    let first_request = project_runtime_sdk::ProjectRuntimeUiStateResolveRequest {
        frame: project_runtime_sdk::ProjectRuntimeFrameRequest { frame_index: 1, time: Default::default() },
        previous_identity: None,
        binding_set: project_runtime_sdk::ProjectRuntimeUiBindingSet::Replace {
            digest: "sha256:fixture-bindings".into(),
            active_binding_paths: vec!["fixture.hp".into()],
        },
    };
    let first: project_runtime_sdk::ProjectRuntimeUiStateResolveOutput = call_json(
        api.resolve_ui_state.expect("UI state callback"), api.module_context, session, Some(&context), &first_request).unwrap();
    let first_identity = match first {
        project_runtime_sdk::ProjectRuntimeUiStateResolveOutput::Replace { identity, producer_id, values } => {
            assert_eq!(producer_id, "fixture.ui");
            assert_eq!(identity.visible_revision, 1);
            assert_eq!(values.get("fixture.hp"), Some(&project_runtime_sdk::ProjectRuntimeValue::Integer(3)));
            identity
        }
        other => panic!("first UI state resolve must replace: {other:?}"),
    };
    let clean_request = project_runtime_sdk::ProjectRuntimeUiStateResolveRequest {
        frame: project_runtime_sdk::ProjectRuntimeFrameRequest { frame_index: 2, time: Default::default() },
        previous_identity: Some(first_identity.clone()),
        binding_set: project_runtime_sdk::ProjectRuntimeUiBindingSet::Known { digest: first_identity.binding_set_digest.clone() },
    };
    let clean: project_runtime_sdk::ProjectRuntimeUiStateResolveOutput = call_json(
        api.resolve_ui_state.unwrap(), api.module_context, session, Some(&context), &clean_request).unwrap();
    assert!(matches!(clean, project_runtime_sdk::ProjectRuntimeUiStateResolveOutput::Reuse { identity } if identity == first_identity));
    let extension_request = project_runtime_sdk::aui_extension::UiExtensionRequestBatch {
        version: project_runtime_sdk::aui_extension::UI_EXTENSION_VERSION,
        requests: vec![project_runtime_sdk::aui_extension::UiExtensionRequest {
            request_id: 1,
            node: project_runtime_sdk::aui::UiNodeHandle { instance: 1, slot: 0, generation: 1 },
            type_id: "fixture.circle".into(),
            schema_version: 1,
            callback_id: 0,
            properties: std::collections::BTreeMap::new(),
            input: project_runtime_sdk::aui_extension::UiExtensionInput::Describe,
        }],
    };
    let extension_result: project_runtime_sdk::aui_extension::UiExtensionResponseBatch = call_json(
        api.ui_extension.expect("ui extension callback"), api.module_context, session, None, &extension_request).unwrap();
    assert!(matches!(&extension_result.responses[0].output, project_runtime_sdk::aui_extension::UiExtensionOutput::Describe { descriptor } if descriptor.type_id == "fixture.circle"));
    let mut paint_request = extension_request.clone();
    paint_request.requests[0].callback_id = 9;
    paint_request.requests[0].input = project_runtime_sdk::aui_extension::UiExtensionInput::Paint { size:[32.,32.] };
    let paint_result: project_runtime_sdk::aui_extension::UiExtensionResponseBatch = call_json(
        api.ui_extension.unwrap(), api.module_context, session, None, &paint_request).unwrap();
    project_runtime_sdk::aui_extension::validate_response_batch(&paint_result).unwrap();
    assert!(matches!(&paint_result.responses[0].output, project_runtime_sdk::aui_extension::UiExtensionOutput::Paint { output } if matches!(&output.primitives[0], project_runtime_sdk::aui_extension::UiExtensionPaintPrimitive::Graphics { graphics } if graphics.items().len()==1)));
    let rule_request = project_runtime_sdk::ProjectRuntimeRuleRequest {
        rule_id: "rule.fixture".into(), frame: request.frame.clone(), input_actions: vec![], collision_pairs: vec![],
    };
    let rule_result: project_runtime_sdk::ProjectRuntimeRuleOutput = call_json(
        api.invoke_rule.unwrap(), api.module_context, session, Some(&context), &rule_request).unwrap();
    assert_eq!(rule_result.status, project_runtime_sdk::ProjectRuntimeStatus::Applied);
    assert!(matches!(&rule_result.mutations[..], [project_runtime_sdk::ProjectRuntimeDeferredMutation::InstantiatePrefab { prefab_id, position: None }] if prefab_id == "fixture.prefab"));
    let event_request = project_runtime_sdk::aui::UiEventRequest {
        callback_id: project_runtime_sdk::aui::UiCallbackId(7),
        notification: project_runtime_sdk::aui::UiNotification {
            sequence: 1,
            target: project_runtime_sdk::aui::UiNodeHandle { instance: 1, slot: 0, generation: 1 },
            kind: project_runtime_sdk::aui::UiNotificationKind::Mount,
        },
    };
    let event_result: project_runtime_sdk::ProjectRuntimeSessionOutput = call_json(
        api.ui_event.expect("ui event callback"), api.module_context, session, Some(&context), &event_request).unwrap();
    assert!(matches!(&event_result.mutations[..], [project_runtime_sdk::ProjectRuntimeDeferredMutation::InstantiatePrefab { prefab_id, position: None }] if prefab_id == "event.prefab.1"));
    let unknown_event = project_runtime_sdk::aui::UiEventRequest {
        callback_id: project_runtime_sdk::aui::UiCallbackId(8),
        ..event_request
    };
    let unknown: Result<project_runtime_sdk::ProjectRuntimeSessionOutput, _> = call_json(
        api.ui_event.unwrap(), api.module_context, session, Some(&context), &unknown_event);
    assert!(unknown.is_err());
    let spawn_request=project_runtime_sdk::ProjectRuntimeLifecycleRequest {event:project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged {focused:true},..request.clone()};
    let spawned:project_runtime_sdk::ProjectRuntimeSessionOutput=call_json(api.lifecycle.unwrap(),api.module_context,session,Some(&context),&spawn_request).unwrap();
    assert!(matches!(&spawned.mutations[..],[project_runtime_sdk::ProjectRuntimeDeferredMutation::InstantiateSpatialPrefab {request_id:7,..}]));
    let changed_request = project_runtime_sdk::ProjectRuntimeUiStateResolveRequest {
        frame: project_runtime_sdk::ProjectRuntimeFrameRequest { frame_index: 3, time: Default::default() },
        previous_identity: Some(first_identity),
        binding_set: project_runtime_sdk::ProjectRuntimeUiBindingSet::Known { digest: "sha256:fixture-bindings".into() },
    };
    let changed: project_runtime_sdk::ProjectRuntimeUiStateResolveOutput = call_json(
        api.resolve_ui_state.unwrap(), api.module_context, session, Some(&context), &changed_request).unwrap();
    assert!(matches!(changed, project_runtime_sdk::ProjectRuntimeUiStateResolveOutput::Replace { identity, values, .. }
        if identity.visible_revision == 2 && values.get("fixture.hp") == Some(&project_runtime_sdk::ProjectRuntimeValue::Integer(2))));
    // The host fixture now exposes the root of that earlier committed request.
    let result: project_runtime_sdk::ProjectRuntimeSessionOutput = call_json(api.lifecycle.unwrap(), api.module_context, session, Some(&context), &request).unwrap();
    assert_eq!(result.host_control, Some(project_runtime_sdk::ProjectRuntimeHostControl::Exit));
    assert!(matches!(&result.mutations[..], [
        project_runtime_sdk::ProjectRuntimeDeferredMutation::Spatial {generation:4294967297,intent:project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform {transform},..},
        project_runtime_sdk::ProjectRuntimeDeferredMutation::Spatial {entity_id,generation:8589934593,intent:project_runtime_sdk::ProjectRuntimeSpatialIntent::SelectMainCamera {}},
    ] if transform.translation==[1.,0.,6.] && entity_id=="instance-actual-root"));
    let _: () = call_json(api.destroy_session.unwrap(), api.module_context, session, None, &()).unwrap();

}
"#,
        )
        .unwrap();
        let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args([
                "run",
                "--offline",
                "--quiet",
                "--manifest-path",
                root.join("Cargo.toml").to_str().unwrap(),
                "-p",
                "generated_glue_smoke",
                "--target-dir",
                root.join("target").to_str().unwrap(),
            ])
            .output()
            .expect("run generated glue smoke");
        assert!(
            output.status.success(),
            "generated glue smoke failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        #[cfg(windows)]
        verify_generated_ui_dll(&root);
    }

    #[cfg(windows)]
    fn verify_generated_ui_dll(root: &Path) {
        use engine_runtime::aui::*;
        use engine_runtime::project_runtime_module::ProjectRuntimeBootstrap;
        use engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
        use engine_runtime::project_runtime_session::{
            ProjectRuntimeSessionContext, ProjectRuntimeSessionStatus,
        };
        use engine_runtime::runtime_package::*;
        use engine_runtime::runtime_package_builder::*;
        use engine_runtime::runtime_time::TimeContext;
        use engine_runtime::world::World;
        use engine_runtime::world_api::WorldReadApi;
        use project_game_sdk::aui::{UiAccess, UiGeometryAccess, UiProperty, UiRead, UiValue};

        let build = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args([
                "build",
                "--offline",
                "--quiet",
                "--manifest-path",
                root.join("Cargo.toml").to_str().unwrap(),
                "-p",
                "aife_generated_runtime_glue",
                "--lib",
                "--target-dir",
                root.join("target").to_str().unwrap(),
            ])
            .output()
            .expect("build generated DLL");
        assert!(
            build.status.success(),
            "DLL build: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let linked = linked_project_runtime_set_from_dll(
            &root.join("target/debug/aife_generated_runtime_glue.dll"),
        )
        .unwrap();
        let descriptor = linked.only_descriptor().unwrap();
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::new(
            "ui-fixture",
            "UI fixture",
            "0.1.0",
            RuntimeProjectModuleRef::new(
                &descriptor.module_id,
                &descriptor.interface_version,
                &descriptor.aot_content_digest,
            ),
        ));
        input.scenes.push(RuntimeScene {
            main_camera: None,
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.into(),
            id: "main".into(),
            name: "Main".into(),
            gravity: 0.,
            background: "#000000".into(),
            sky_color: "#000000".into(),
            entities: vec![],
        });
        input.input_mappings.push(RuntimePackageSourceJson {id:"input.none".into(), document:serde_json::json!({
            "schema_version":"input-mapping.v2", "asset_id":"input.none", "actions":[], "contexts":[], "bindings":[]
        })});
        input.rule_manifest = Some(serde_json::from_value(serde_json::json!({"schemaVersion":"runtime-rule-manifest.v1","mode":"rust-aot","rules":[{
            "ruleId":"rule.fixture", "phase":"Update", "enabled":true, "executor":"rustAot", "irHash":descriptor.aot_content_digest, "irSource":"fixture",
            "artifactId":format!("rule-artifact:rule.fixture:{}",descriptor.aot_content_digest)
        }],"modules":[{"artifactId":format!("rule-artifact:rule.fixture:{}",descriptor.aot_content_digest),"moduleKind":"staticRegistry","path":"fixture"}]})).unwrap());
        let mut panel = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        panel.children = vec!["label".into(), "button".into(), "speed".into()];
        let mut label = AuiNode::new(
            "label",
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 100., 20.),
        );
        label.parent = Some("root".into());
        label.text = Some("人".into());
        let mut button = AuiNode::new(
            "button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(0., 30., 100., 20.),
        );
        button.parent = Some("root".into());
        button.interactable = true;
        let mut slider = AuiNode::new(
            "speed",
            AuiNodeKind::Slider,
            AuiRect::fixed_position(0., 60., 100., 20.),
        )
        .with_slider(1.0, 0.5, 2.0, 0.25);
        slider.parent = Some("root".into());
        let doc = AuiDocument::new(
            "settings",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![panel, label, button, slider],
        );
        let (_, cooked_document) = crate::neutral_assembler::cook_aui_document(
            "AUI/settings.json",
            serde_json::to_value(doc).unwrap(),
        )
        .unwrap();
        input.aui_documents.push(RuntimePackageSourceJson {
            id: "settings".into(),
            document: cooked_document,
        });
        input.aui_manifest = Some(RuntimeAuiManifest {
            schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.into(),
            documents: vec![RuntimeAuiManifestEntry {
                document_id: "settings".into(),
                path: "aui/documents/settings.aui.json".into(),
                canvas_count: 1,
                node_count: 4,
                binding_count: 0,
                action_count: 0,
                asset_refs: vec![],
            }],
        });
        let package_dir = root.join("package");
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&package_dir, "main"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        let package = load_runtime_package(&package_dir).value.unwrap();
        let world = World::new();
        let mut isolated = ProjectRuntimeBootstrap::bind(&package, &linked).unwrap();
        let old = isolated
            .project_runtime_session()
            .retained_ui()
            .unwrap()
            .resolve("settings", "label")
            .unwrap();
        for mode in 100..=104 {
            let mut bound = ProjectRuntimeBootstrap::bind(&package, &linked).unwrap();
            let session = bound.project_runtime_session_mut();
            let ui = session.retained_ui().unwrap();
            assert!(ui.node_type(old).is_err());
            let label = ui.resolve("settings", "label").unwrap();
            let button = ui.resolve("settings", "button").unwrap();
            let mut document = ui.document("settings").unwrap().source().clone();
            // This DLL fixture tests coordinates and transactions, not glyph production.
            // Supply the same fixed label geometry without requiring an unmounted font bundle.
            let geometry_label = document
                .nodes
                .iter_mut()
                .find(|n| n.node_id == "label")
                .unwrap();
            geometry_label.kind = AuiNodeKind::Panel;
            geometry_label.text = None;
            let geometry =
                engine_runtime::aui::AuiRuntimePresenter::present_package_smoke(&document, mode);
            assert_ne!(
                geometry.report.status,
                engine_runtime::aui::AuiRuntimePresentStatus::Failed,
                "{:?}",
                geometry.report.diagnostics
            );
            assert!(geometry.layout.diagnostics.is_empty());
            session.publish_ui_geometry(&geometry);
            let output = session.lifecycle(
                ProjectRuntimeSessionContext {
                    frame_index: mode,
                    time: TimeContext::from_delta(mode, 1. / 60., false),
                    world: WorldReadApi::new(&world),
                },
                project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged { focused: true },
            );
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(label, UiProperty::Text)
                    .unwrap(),
                UiValue::Text("人".into())
            );
            let accepted = output.status == ProjectRuntimeSessionStatus::Applied;
            assert_eq!(accepted, mode == 100, "mode {mode}: {output:?}");
            session.finish_ui_updates(accepted);
            for (property, expected, original) in [
                (
                    UiProperty::Rotation,
                    UiValue::Number(0.25),
                    UiValue::Number(0.),
                ),
                (
                    UiProperty::Scale,
                    UiValue::Vector([-1., 1.5]),
                    UiValue::Vector([1.; 2]),
                ),
                (
                    UiProperty::Pivot,
                    UiValue::Vector([0., 1.]),
                    UiValue::Vector([0.5; 2]),
                ),
                (
                    UiProperty::Opacity,
                    UiValue::Number(0.6),
                    UiValue::Number(1.),
                ),
                (
                    UiProperty::Tint,
                    UiValue::Color([0.8, 1., 0.5, 1.]),
                    UiValue::Color([1.; 4]),
                ),
            ] {
                assert_eq!(
                    session
                        .retained_ui()
                        .unwrap()
                        .read(label, property)
                        .unwrap(),
                    if accepted { expected } else { original },
                    "mode {mode}, {property:?}"
                );
            }
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(label, UiProperty::Text)
                    .unwrap(),
                UiValue::Text(if accepted { "骑兵" } else { "人" }.into())
            );
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(button, UiProperty::Interactable)
                    .unwrap(),
                UiValue::Bool(!accepted)
            );
        }
        for scenario in ["complete", "cancel", "rollback"] {
            let mut bound = ProjectRuntimeBootstrap::bind(&package, &linked).unwrap();
            let session = bound.project_runtime_session_mut();
            let label = session
                .retained_ui()
                .unwrap()
                .resolve("settings", "label")
                .unwrap();
            let mut document = session
                .retained_ui()
                .unwrap()
                .document("settings")
                .unwrap()
                .source()
                .clone();
            document.nodes[1].kind = AuiNodeKind::Panel;
            document.nodes[1].text = None;
            let geometry =
                engine_runtime::aui::AuiRuntimePresenter::present_package_smoke(&document, 200);
            assert!(geometry.layout.diagnostics.is_empty());
            session.publish_ui_geometry(&geometry);
            let initial = session
                .retained_ui()
                .unwrap()
                .local_to_screen(label, [0.; 2])
                .unwrap();
            let mut invoke = |mode: u64, accepted: bool| {
                let output = session.lifecycle(
                    ProjectRuntimeSessionContext {
                        frame_index: mode,
                        time: TimeContext::from_delta(mode, 1. / 60., false),
                        world: WorldReadApi::new(&world),
                    },
                    project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged {
                        focused: true,
                    },
                );
                assert_eq!(
                    output.status == ProjectRuntimeSessionStatus::Applied,
                    accepted,
                    "animation {scenario} mode {mode}: {output:?}"
                );
                session.finish_ui_updates(accepted);
            };
            invoke(
                if scenario == "rollback" { 204 } else { 200 },
                scenario != "rollback",
            );
            session.advance_ui_animations(200, 5_000).unwrap();
            let point = session
                .retained_ui()
                .unwrap()
                .local_to_screen(label, [0.; 2])
                .unwrap();
            assert_eq!(
                point,
                [
                    initial[0] + if scenario == "rollback" { 0. } else { 50. },
                    initial[1]
                ],
                "actual DLL halfway {scenario}"
            );
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(label, UiProperty::Translation)
                    .unwrap(),
                UiValue::Translation([0.; 2]),
                "animation never takes base ownership"
            );
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(label, UiProperty::Opacity)
                    .unwrap(),
                UiValue::Number(if scenario == "rollback" { 1. } else { 0.8 }),
                "setters before animation share callback acceptance"
            );
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(label, UiProperty::Tint)
                    .unwrap(),
                UiValue::Color(if scenario == "rollback" {
                    [1.; 4]
                } else {
                    [0.7, 1., 1., 1.]
                }),
                "setters after animation share callback acceptance"
            );
            if scenario == "complete" {
                for (mode, delta, offset) in [(201, 5_000, 50.), (202, 5_000, 100.)] {
                    let output = session.lifecycle(
                        ProjectRuntimeSessionContext {
                            frame_index: mode,
                            time: TimeContext::from_delta(mode, 1. / 60., false),
                            world: WorldReadApi::new(&world),
                        },
                        project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged {
                            focused: true,
                        },
                    );
                    assert_eq!(output.status, ProjectRuntimeSessionStatus::Applied);
                    session.finish_ui_updates(true);
                    session.advance_ui_animations(mode, delta).unwrap();
                    assert_eq!(
                        session
                            .retained_ui()
                            .unwrap()
                            .local_to_screen(label, [0.; 2])
                            .unwrap(),
                        [initial[0] + offset, initial[1]]
                    );
                }
                assert_eq!(
                    session
                        .retained_ui()
                        .unwrap()
                        .read(label, UiProperty::Text)
                        .unwrap(),
                    UiValue::Text("人".into()),
                    "paint cannot reenter project callback"
                );
            } else if scenario == "cancel" {
                let output = session.lifecycle(
                    ProjectRuntimeSessionContext {
                        frame_index: 203,
                        time: TimeContext::from_delta(203, 1. / 60., false),
                        world: WorldReadApi::new(&world),
                    },
                    project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged {
                        focused: true,
                    },
                );
                assert_eq!(output.status, ProjectRuntimeSessionStatus::Applied);
                session.finish_ui_updates(true);
                assert_eq!(
                    session
                        .retained_ui()
                        .unwrap()
                        .local_to_screen(label, [0.; 2])
                        .unwrap(),
                    initial
                );
            }
            let output = session.lifecycle(
                ProjectRuntimeSessionContext {
                    frame_index: 205,
                    time: TimeContext::from_delta(205, 1. / 60., false),
                    world: WorldReadApi::new(&world),
                },
                project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged { focused: true },
            );
            assert_eq!(output.status, ProjectRuntimeSessionStatus::Applied);
            session.finish_ui_updates(true);
            assert_eq!(
                session
                    .retained_ui()
                    .unwrap()
                    .read(label, UiProperty::Text)
                    .unwrap(),
                UiValue::Text(
                    match scenario {
                        "complete" => "动画完成",
                        "cancel" => "动画取消",
                        _ => "人",
                    }
                    .into()
                ),
                "actual DLL completion dispatch {scenario}"
            );
        }
        assert_eq!(
            isolated
                .project_runtime_session()
                .retained_ui()
                .unwrap()
                .read(old, UiProperty::Text)
                .unwrap(),
            UiValue::Text("人".into())
        );
        // A panic in another session must not prevent an already-live session from running.
        let session = isolated.project_runtime_session_mut();
        let mut document = session
            .retained_ui()
            .unwrap()
            .document("settings")
            .unwrap()
            .source()
            .clone();
        let geometry_label = document
            .nodes
            .iter_mut()
            .find(|n| n.node_id == "label")
            .unwrap();
        geometry_label.kind = AuiNodeKind::Panel;
        geometry_label.text = None;
        let geometry =
            engine_runtime::aui::AuiRuntimePresenter::present_package_smoke(&document, 100);
        assert_ne!(
            geometry.report.status,
            engine_runtime::aui::AuiRuntimePresentStatus::Failed,
            "{:?}",
            geometry.report.diagnostics
        );
        assert!(geometry.layout.diagnostics.is_empty());
        session.publish_ui_geometry(&geometry);
        let output = session.lifecycle(
            ProjectRuntimeSessionContext {
                frame_index: 100,
                time: TimeContext::from_delta(100, 1. / 60., false),
                world: WorldReadApi::new(&world),
            },
            project_runtime_sdk::ProjectRuntimeLifecycleEvent::FocusChanged { focused: true },
        );
        assert_eq!(output.status, ProjectRuntimeSessionStatus::Applied);
        session.finish_ui_updates(true);
        for (property, value) in [
            (UiProperty::Rotation, UiValue::Number(0.25)),
            (UiProperty::Scale, UiValue::Vector([-1.0, 1.5])),
            (UiProperty::Pivot, UiValue::Vector([0.0, 1.0])),
            (UiProperty::Opacity, UiValue::Number(0.6)),
            (UiProperty::Tint, UiValue::Color([0.8, 1.0, 0.5, 1.0])),
        ] {
            assert_eq!(
                session.retained_ui().unwrap().read(old, property).unwrap(),
                value
            );
        }
        assert_eq!(
            session
                .retained_ui()
                .unwrap()
                .read(old, UiProperty::Text)
                .unwrap(),
            UiValue::Text("骑兵".into())
        );
    }

    pub(super) fn temp_root(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aife-generated-runtime-glue-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn rule_manifest(artifact_suffix: &str) -> String {
        format!(
            r#"{{"schemaVersion":"runtime-rule-manifest.v1","mode":"rust-aot","rules":[{{"ruleId":"rule.fixture","phase":"Update","enabled":true,"executor":"rustAot","artifactId":"rule-artifact:rule.fixture:{artifact_suffix}"}}],"modules":[]}}"#
        )
    }

    pub(super) struct TestDirectoryGuard(pub(super) std::path::PathBuf);

    impl Drop for TestDirectoryGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
