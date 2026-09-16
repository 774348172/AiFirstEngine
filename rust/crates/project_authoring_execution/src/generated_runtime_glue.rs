use crate::game_project_compiler::{CompilerRuntimeModuleSpec, CompilerSourceView, TargetProfile};
use project_game_sdk::PROJECT_GAME_SDK_CONTRACT_ID;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

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
    let normalized_source = generated_source(
        runtime_module,
        &runtime_source_identity,
        &rule_artifact_match_arms(source_view)?,
    );
    let normalized_manifest = generated_manifest(runtime_module)?;
    let generation_digest = generation_digest(
        &normalized_source,
        &normalized_manifest,
        &runtime_source_identity,
        &runtime_contract_digest,
        target_profile,
    );
    let registration_path = registration_source_path(runtime_module);
    let source_map = generated_items()
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
    Ok(Some(PreparedRuntimeGlue {
        normalized_source,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn runtime_module() -> CompilerRuntimeModuleSpec {
        CompilerRuntimeModuleSpec {
            module_id: "fixture.game.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            cargo_manifest: "RuntimeModule/Cargo.toml".to_string(),
            cargo_package: "fixture_game_runtime".to_string(),
            player_binary: "fixture_game_player".to_string(),
            project_game_sdk: PROJECT_GAME_SDK_CONTRACT_ID.to_string(),
        }
    }

    fn source_view(gameplay_source: &str, scene_source: &str) -> CompilerSourceView {
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
        let project_source = r#"use project_game_sdk::{
    GameCallContext, GameResult, HandlerStatus, ProjectGameDefinition, ProjectGameSession,
    RuleOutput, RuleRequest, SessionCreateRequest,
};

pub struct Session;

impl ProjectGameSession for Session {
    fn session_id(&self) -> &str { "fixture.session" }
}

fn create(_: &SessionCreateRequest) -> GameResult<Session> { Ok(Session) }

fn rule(
    _: &mut Session,
    _: &mut GameCallContext<'_>,
    _: &RuleRequest,
) -> GameResult<RuleOutput> {
    Ok(RuleOutput {
        status: HandlerStatus::Applied,
        mutations: Vec::new(),
        diagnostics: Vec::new(),
    })
}

pub fn project_game() -> ProjectGameDefinition<Session> {
    ProjectGameDefinition::new(create).rule("rule.fixture", rule)
}
"#;
        fs::write(runtime_root.join("src/lib.rs"), project_source).unwrap();

        let source_view = CompilerSourceView {
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
                "[package]\nname='generated_glue_smoke'\nversion='0.1.0'\nedition='2021'\npublish=false\n\n[dependencies]\naife_generated_runtime_glue={{path='../RuntimeGlue'}}\nproject_runtime_abi={{path='{engine_sdk_path}/crates/project_runtime_abi'}}\nproject_runtime_sdk={{path='{engine_sdk_path}/crates/project_runtime_sdk'}}\n"
            ),
        )
        .unwrap();
        fs::write(
            smoke_root.join("src/main.rs"),
            r#"use project_runtime_abi::ProjectRuntimeOpaqueHandle;
use project_runtime_sdk::{call_json, ProjectRuntimeModuleDescriptor};

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
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
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

    struct TestDirectoryGuard(std::path::PathBuf);

    impl Drop for TestDirectoryGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
