use editor_core::{
    ProjectEditorCompositionArtifact, ProjectEditorCompositionBuildRequest,
    ProjectEditorCompositionBuildStatus, ProjectEditorCompositionCachePolicy,
    ProjectEditorCompositionIdentity, ProjectManifest, ProjectRuntimeTrustDecisionKind,
    ProjectRuntimeTrustInspection, ProjectRuntimeTrustModule,
    PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::{
    project_runtime_aot_digest, ProjectRuntimeAotDigestSource,
};
use runtime_cli::{
    run_bounded_child_process, BoundedChildProcessExitReason, BoundedChildProcessRequest,
};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RUN_ROOT_ENV: &str = "AIFE_262_WINDOW4_RUN_ROOT";
const PROJECT_FILTER_ENV: &str = "AIFE_262_WINDOW4_PROJECT_FILTER";

#[test]
#[ignore = "builds and launches four real release project Editor compositions"]
fn project_editor_composition_production_project_matrix() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root must resolve");
    let sdk_root = repository_root.join("rust").canonicalize().unwrap();
    let run_root = explicit_or_temporary_run_root();
    fs::create_dir_all(run_root.join("evidence")).unwrap();
    fs::create_dir_all(run_root.join("trust")).unwrap();
    fs::create_dir_all(run_root.join("build")).unwrap();

    let tower_defense_source = repository_root.join("samples/tower_defense_project");
    let tower_candidate_destination = run_root.join("projects/tower-defense-candidate");
    let tower_candidate = if tower_candidate_destination.exists() {
        validate_project_fixture(&tower_candidate_destination, "project-4966952341520437268")
    } else {
        create_tower_defense_candidate(
            &tower_defense_source,
            &tower_candidate_destination,
            &sdk_root,
        )
    };
    let external_destination = run_root.join("external-projects/tower-defense-external-v2");
    let external_project = if external_destination.exists() {
        validate_project_fixture(&external_destination, "fixture.external.tower-defense")
    } else {
        create_external_project_fixture(&tower_defense_source, &external_destination, &sdk_root)
    };
    let projects = [
        (
            "complex-shooter",
            repository_root.join("samples/complex_shooter_project"),
        ),
        (
            "switch-puzzle",
            repository_root.join("samples/switch_puzzle_project"),
        ),
        ("tower-defense", tower_candidate),
        ("repository-external", external_project),
    ];

    let editor_build_identity = source_identity(&sdk_root);
    let engine_sdk_digest = sha256_prefixed(&fs::read(sdk_root.join("Cargo.lock")).unwrap());
    let toolchain_identity = rustc_identity();
    let trust = ProjectRuntimeTrustModule::open(run_root.join("trust")).unwrap();
    let mut matrix = Vec::new();

    let project_filter = std::env::var(PROJECT_FILTER_ENV).ok();
    for (label, project_root) in projects {
        if project_filter
            .as_deref()
            .is_some_and(|filter| filter != label)
        {
            continue;
        }
        let project_root = project_root.canonicalize().unwrap();
        let inspection = ProjectRuntimeTrustInspection::inspect(
            &project_root,
            &sdk_root,
            editor_build_identity.clone(),
        )
        .unwrap_or_else(|error| panic!("{label}: trust inspection failed: {error}"));
        trust
            .record_explicit(
                &inspection.request,
                ProjectRuntimeTrustDecisionKind::Trusted,
                now_epoch_seconds(),
            )
            .unwrap_or_else(|error| panic!("{label}: trust record failed: {error}"));
        let evaluation = trust
            .evaluate(&inspection.request, None)
            .unwrap_or_else(|error| panic!("{label}: trust evaluation failed: {error}"));
        assert_eq!(
            evaluation.status,
            editor_core::ProjectRuntimeTrustStatus::Trusted,
            "{label}: explicit run-owned trust receipt was not accepted"
        );

        let manifest: ProjectManifest =
            serde_json::from_slice(&fs::read(project_root.join("project.aife.json")).unwrap())
                .unwrap();
        let aot_content_digest = project_aot_digest(&project_root, &manifest);
        let identity = ProjectEditorCompositionIdentity {
            schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: manifest.project_id.clone(),
            module_id: manifest.runtime_module.module_id.clone(),
            interface_version: manifest.runtime_module.interface_version.clone(),
            aot_content_digest: aot_content_digest.clone(),
            editor_build_identity: editor_build_identity.clone(),
            engine_sdk_digest: engine_sdk_digest.clone(),
            toolchain_identity: toolchain_identity.clone(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: inspection.request.normalized_manifest_digest.clone(),
            normalized_dependency_digest: inspection.request.normalized_dependency_digest.clone(),
            dependency_lock_digest: engine_sdk_digest.clone(),
        };
        identity.validate().unwrap();
        let expected_identity_digest = identity.digest().unwrap();
        let build = ProjectEditorCompositionArtifact::prepare(
            ProjectEditorCompositionBuildRequest {
                schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
                project_root: project_root.clone(),
                engine_sdk_root: sdk_root.clone(),
                build_root: run_root.join("build").join(label),
                expected_identity: identity,
                cache_policy: ProjectEditorCompositionCachePolicy::default(),
                qos_policy: editor_core::ProjectEditorCompositionBuildQosPolicy::default(),
                deadline_policy: editor_core::ProjectEditorCompositionBuildDeadlinePolicy::default(
                ),
                cargo_executable: None,
                cargo_identity: "cargo-current".to_string(),
                capture_limit_bytes: 256 * 1024,
                prepared_runtime_glue: None,
            },
            editor_core::ProjectEditorCompositionPreparationControl::default(),
        );
        assert_eq!(
            build.status,
            ProjectEditorCompositionBuildStatus::Success,
            "{label}: composition build failed: {:#?}",
            build.diagnostics
        );
        let artifact = build
            .artifact
            .expect("successful build must return artifact");
        assert_eq!(
            artifact.descriptor.identity_digest,
            expected_identity_digest
        );
        assert_eq!(artifact.descriptor.identity.project_id, manifest.project_id);
        assert_eq!(
            artifact.descriptor.identity.module_id,
            manifest.runtime_module.module_id
        );
        assert_eq!(
            artifact.descriptor.identity.aot_content_digest,
            aot_content_digest
        );

        let process = run_bounded_child_process(BoundedChildProcessRequest {
            executable: artifact.executable_path.clone(),
            args: vec![
                OsString::from("--qualify-project-runtime"),
                project_root.as_os_str().to_os_string(),
            ],
            current_dir: project_root.clone(),
            environment: vec![(
                OsString::from("AIFE_PROJECT_RUNTIME_PLAYER_BUILD_ROOT"),
                run_root
                    .join("player-artifacts")
                    .join(label)
                    .as_os_str()
                    .to_os_string(),
            )],
            timeout: Duration::from_secs(180),
            stdout_capture_limit_bytes: 256 * 1024,
            stderr_capture_limit_bytes: 256 * 1024,
            priority: runtime_cli::BoundedChildProcessPriority::Normal,
        });
        assert_eq!(
            process.exit_reason,
            BoundedChildProcessExitReason::Completed,
            "{label}: qualification process did not complete: {process:#?}"
        );
        assert_eq!(
            process.exit_code,
            Some(0),
            "{label}: qualification process failed: {process:#?}"
        );
        let report: Value =
            serde_json::from_str(process.stdout_summary.trim()).unwrap_or_else(|error| {
                panic!("{label}: invalid qualification JSON: {error}; {process:#?}")
            });
        assert_eq!(report["status"], "passed", "{label}: {report:#}");
        assert_eq!(report["projectId"], manifest.project_id);
        assert_eq!(report["moduleId"], manifest.runtime_module.module_id);
        assert_eq!(
            report["compositionIdentityDigest"],
            expected_identity_digest
        );
        assert_eq!(report["linkedAotContentDigest"], aot_content_digest);
        assert_eq!(report["stopped"], true);

        matrix.push(json!({
            "label": label,
            "projectRoot": project_root,
            "trustStatus": "trusted",
            "cacheStatus": build.cache_status,
            "executablePath": artifact.executable_path,
            "executableHash": artifact.descriptor.executable_hash,
            "identityDigest": expected_identity_digest,
            "qualification": report,
            "qualificationProgress": process.stderr_summary,
            "processOwnership": process.ownership,
        }));
    }

    assert!(
        !matrix.is_empty(),
        "project matrix filter selected no project"
    );

    let evidence = json!({
        "schemaVersion": "project-editor-composition-gate-g-report.v1",
        "status": "passed",
        "runRoot": run_root,
        "editorBuildIdentity": editor_build_identity,
        "engineSdkDigest": engine_sdk_digest,
        "toolchainIdentity": toolchain_identity,
        "projects": matrix,
    });
    fs::write(
        run_root.join("evidence/gate-g-report.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&evidence).unwrap());
}

fn project_aot_digest(project_root: &Path, manifest: &ProjectManifest) -> String {
    let mut source_paths = vec![
        PathBuf::from("RuntimeModule/Cargo.toml"),
        PathBuf::from("RuntimeModule/Cargo.lock"),
    ];
    collect_rust_sources(
        project_root,
        &project_root.join("RuntimeModule/src"),
        &mut source_paths,
    );
    source_paths.sort();
    let sources = source_paths
        .into_iter()
        .map(|relative_path| {
            let canonical_name = relative_path.to_string_lossy().replace('\\', "/");
            let bytes = fs::read(project_root.join(&relative_path)).unwrap();
            (canonical_name, bytes)
        })
        .collect::<Vec<_>>();
    project_runtime_aot_digest(
        &manifest.runtime_module.module_id,
        &manifest.runtime_module.interface_version,
        &manifest.runtime_module.cargo_manifest,
        &manifest.runtime_module.cargo_package,
        &manifest.runtime_module.player_binary,
        sources
            .iter()
            .map(|(relative_path, bytes)| ProjectRuntimeAotDigestSource {
                relative_path,
                bytes,
            }),
    )
    .unwrap()
}

fn collect_rust_sources(project_root: &Path, directory: &Path, output: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "source link is not allowed: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_rust_sources(project_root, &path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path.strip_prefix(project_root).unwrap().to_path_buf());
        }
    }
}

fn create_external_project_fixture(source: &Path, destination: &Path, sdk_root: &Path) -> PathBuf {
    assert!(
        !destination.exists(),
        "external fixture destination must be fresh"
    );
    copy_regular_tree(source, destination);
    let manifest_path = destination.join("project.aife.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["projectId"] = Value::String("fixture.external.tower-defense".to_string());
    manifest["projectName"] = Value::String("External Tower Defense Fixture".to_string());
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    rewrite_project_identity_references(
        destination,
        "project-4966952341520437268",
        "fixture.external.tower-defense",
    );

    normalize_engine_dependency_paths(destination, sdk_root);
    destination.canonicalize().unwrap()
}

fn create_tower_defense_candidate(source: &Path, destination: &Path, sdk_root: &Path) -> PathBuf {
    assert!(
        !destination.exists(),
        "Tower Defense candidate destination must be fresh"
    );
    copy_regular_tree(source, destination);
    normalize_engine_dependency_paths(destination, sdk_root);
    validate_project_fixture(destination, "project-4966952341520437268")
}

fn normalize_engine_dependency_paths(project_root: &Path, sdk_root: &Path) {
    let runtime_manifest_path = project_root.join("RuntimeModule/Cargo.toml");
    let mut runtime_manifest: toml::Value =
        toml::from_str(&fs::read_to_string(&runtime_manifest_path).unwrap()).unwrap();
    let dependencies = runtime_manifest
        .get_mut("dependencies")
        .and_then(toml::Value::as_table_mut)
        .unwrap();
    for dependency_name in [
        "engine_runtime",
        "project_runtime_abi",
        "project_runtime_sdk",
    ] {
        let Some(dependency) = dependencies
            .get_mut(dependency_name)
            .and_then(toml::Value::as_table_mut)
        else {
            continue;
        };
        dependency.insert(
            "path".to_string(),
            toml::Value::String(
                sdk_root
                    .join("crates")
                    .join(dependency_name)
                    .canonicalize()
                    .unwrap()
                    .display()
                    .to_string(),
            ),
        );
    }
    fs::write(
        runtime_manifest_path,
        toml::to_string_pretty(&runtime_manifest).unwrap(),
    )
    .unwrap();
}

fn validate_project_fixture(destination: &Path, expected_project_id: &str) -> PathBuf {
    let canonical = destination.canonicalize().unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(canonical.join("project.aife.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["projectId"], expected_project_id,
        "existing run-owned external fixture has an unexpected identity"
    );
    canonical
}

fn rewrite_project_identity_references(
    directory: &Path,
    old_project_id: &str,
    new_project_id: &str,
) {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "fixture identity input cannot be a link: {}",
            path.display()
        );
        if metadata.is_dir() {
            rewrite_project_identity_references(&path, old_project_id, new_project_id);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let Ok(mut value) = serde_json::from_slice::<Value>(&fs::read(&path).unwrap()) else {
                continue;
            };
            if replace_json_string(&mut value, old_project_id, new_project_id) {
                fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
            }
        }
    }
}

fn replace_json_string(value: &mut Value, old_value: &str, new_value: &str) -> bool {
    match value {
        Value::String(current) if current == old_value => {
            *current = new_value.to_string();
            true
        }
        Value::Array(values) => values.iter_mut().fold(false, |changed, value| {
            replace_json_string(value, old_value, new_value) || changed
        }),
        Value::Object(values) => values.values_mut().fold(false, |changed, value| {
            replace_json_string(value, old_value, new_value) || changed
        }),
        _ => false,
    }
}

fn copy_regular_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    let mut entries = fs::read_dir(source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "fixture source link is not allowed: {}",
            path.display()
        );
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if metadata.is_dir() && matches!(name.as_str(), ".aife" | "target" | "tests") {
            continue;
        }
        let target = destination.join(path.file_name().unwrap());
        if metadata.is_dir() {
            copy_regular_tree(&path, &target);
        } else if metadata.is_file() {
            fs::copy(&path, target).unwrap();
        }
    }
}

fn source_identity(sdk_root: &Path) -> String {
    let mut paths = vec![sdk_root.join("Cargo.toml"), sdk_root.join("Cargo.lock")];
    for crate_name in [
        "editor_core",
        "editor_input",
        "editor_ui_backend_egui",
        "editor_ui_model",
        "editor_ui_renderer",
        "editor_window_winit",
        "engine_input",
        "engine_runtime",
        "runtime_cli",
    ] {
        collect_identity_inputs(&sdk_root.join("crates").join(crate_name), &mut paths);
    }
    paths.sort();
    let mut bytes = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(sdk_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read(&path).unwrap();
        bytes.extend_from_slice(&(relative.len() as u64).to_le_bytes());
        bytes.extend_from_slice(relative.as_bytes());
        bytes.extend_from_slice(&(content.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&content);
    }
    sha256_prefixed(&bytes)
}

fn collect_identity_inputs(directory: &Path, output: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "Editor source identity input cannot be a link: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_identity_inputs(&path, output);
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml")
            || path.extension().is_some_and(|extension| extension == "rs")
        {
            output.push(path);
        }
    }
}

fn rustc_identity() -> String {
    let output = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()
        .expect("rustc identity command must start");
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn explicit_or_temporary_run_root() -> PathBuf {
    std::env::var_os(RUN_ROOT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "aife-262-window4-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ))
        })
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
