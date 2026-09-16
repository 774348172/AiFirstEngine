use super::*;

pub(super) fn compatibility_key(
    project_root: &Path,
    source: &ProjectRuntimePlayerStagingPlan,
    cargo: &Option<PathBuf>,
    channel: &str,
    target: &str,
    environment: &std::collections::BTreeMap<String, String>,
) -> Result<String, ProjectPlayerArtifactError> {
    Ok(sha256_prefixed(
        &serde_json::to_vec(&(
            "windows-dev.v1",
            project_root.display().to_string(),
            // Source freshness belongs to Cargo and the immutable artifact key.
            // Including it here discards unchanged dependencies on every SDK edit.
            source.sdk_root.display().to_string(),
            &source.normalized_manifest_digest,
            &source.trusted_lock_digest,
            cargo,
            channel,
            target,
            environment,
        ))
        .unwrap(),
    ))
}

pub(super) struct FrozenSource(pub PathBuf);

impl Drop for FrozenSource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) struct CompileWorkspace {
    pub root: PathBuf,
    _lock: fs::File,
}

impl CompileWorkspace {
    pub fn acquire(build_root: &Path, key: &str) -> Result<Self, ProjectPlayerArtifactError> {
        let digest = key.trim_start_matches("sha256:");
        // Keep MSVC object paths short; verify the full identity under the lock.
        let root = build_root.join(format!("c-{}", &digest[..digest.len().min(16)]));
        fs::create_dir_all(&root).map_err(io_error)?;
        reject_links(&root)?;
        let path = root.join("workspace.lock");
        if path.exists() {
            reject_links(&path)?;
        }
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(io_error)?;
        lock.try_lock().map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.compile_workspace_busy",
                format!("Compile workspace is locked or unavailable: {error}"),
            )
        })?;
        let identity = root.join("compatibility-key");
        if identity.exists() {
            reject_links(&identity)?;
            if fs::read(&identity).map_err(io_error)? != key.as_bytes() {
                return Err(ProjectPlayerArtifactError::new("project_runtime.compile_workspace_identity_collision", "Short directory identity differs from the full compatibility key; use another build root."));
            }
        } else {
            fs::write(identity, key).map_err(io_error)?;
        }
        if root.join("ownership-unclosed").exists() {
            return Err(ProjectPlayerArtifactError::new("project_runtime.compile_workspace_recovery_required", "Previous child ownership was not closed; recover this workspace before reusing it."));
        }
        Ok(Self { root, _lock: lock })
    }

    pub fn sync(&self, staged: &Path) -> Result<(), ProjectPlayerArtifactError> {
        for name in ["RuntimeModule", "RuntimeModuleBuild", "RuntimeGlue", "Host"] {
            sync_tree(&staged.join(name), &self.root.join(name))?;
        }
        Ok(())
    }
}

fn reject_links(path: &Path) -> Result<(), ProjectPlayerArtifactError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(io_error(std::io::Error::other(
                "reparse point in compile workspace",
            )));
        }
    }
    if metadata.file_type().is_symlink() {
        return Err(io_error(std::io::Error::other(
            "symlink in compile workspace",
        )));
    }
    Ok(())
}

fn sync_tree(source: &Path, target: &Path) -> Result<(), ProjectPlayerArtifactError> {
    if !source.exists() {
        if target.exists() {
            remove_tree(target)?;
        }
        return Ok(());
    }
    reject_links(source)?;
    if target.exists() {
        reject_links(target)?;
    }
    if source.is_file() {
        let bytes = fs::read(source).map_err(io_error)?;
        if fs::read(target).ok().as_deref() != Some(bytes.as_slice()) {
            if target.is_dir() {
                remove_tree(target)?;
            }
            fs::write(target, bytes).map_err(io_error)?;
        }
        return Ok(());
    }
    if target.is_file() {
        fs::remove_file(target).map_err(io_error)?;
    }
    fs::create_dir_all(target).map_err(io_error)?;
    for entry in fs::read_dir(target).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        if !source.join(entry.file_name()).exists() {
            remove_tree(&entry.path())?;
        }
    }
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        sync_tree(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
}

fn remove_tree(path: &Path) -> Result<(), ProjectPlayerArtifactError> {
    reject_links(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path).map_err(io_error)? {
            remove_tree(&entry.map_err(io_error)?.path())?;
        }
        fs::remove_dir(path).map_err(io_error)
    } else {
        fs::remove_file(path).map_err(io_error)
    }
}

pub(super) fn io_error(error: std::io::Error) -> ProjectPlayerArtifactError {
    ProjectPlayerArtifactError::new("project_runtime.compile_workspace_io", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_source_edit_preserves_compile_workspace_but_changes_artifact_input() {
        let project = default_engine_sdk_root()
            .canonicalize()
            .unwrap()
            .parent()
            .unwrap()
            .join("samples/complex_shooter_project");
        let mut source =
            ProjectRuntimeProductionStaging::plan(&project, &default_engine_sdk_root()).unwrap();
        let root = std::env::temp_dir().join(format!(
            "aife-cache-key-{}-{}",
            std::process::id(),
            ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ENGINE_PLAYER_RUNTIME_CRATES {
            fs::create_dir_all(root.join("crates").join(name)).unwrap();
        }
        let file = root.join("crates/engine_input/lib.rs");
        fs::write(&file, "pub fn value() -> u32 { 1 }").unwrap();
        source.sdk_root = root.clone();
        let env = std::collections::BTreeMap::new();
        let key = compatibility_key(
            &project,
            &source,
            &None,
            "1.96.0",
            "x86_64-pc-windows-msvc",
            &env,
        )
        .unwrap();
        let digest = engine_sdk_source_digest(&root).unwrap();
        fs::write(&file, "pub fn value() -> u32 { 2 }").unwrap();
        let changed = compatibility_key(
            &project,
            &source,
            &None,
            "1.96.0",
            "x86_64-pc-windows-msvc",
            &env,
        )
        .unwrap();
        assert_ne!(digest, engine_sdk_source_digest(&root).unwrap());
        assert_ne!(
            key,
            compatibility_key(
                &project,
                &source,
                &None,
                "other-toolchain",
                "x86_64-pc-windows-msvc",
                &env
            )
            .unwrap()
        );
        assert_ne!(
            key,
            compatibility_key(&project, &source, &None, "1.96.0", "other-target", &env).unwrap()
        );
        let changed_env =
            std::collections::BTreeMap::from([("CARGO_PROFILE_DEV_OPT_LEVEL".into(), "2".into())]);
        assert_ne!(
            key,
            compatibility_key(
                &project,
                &source,
                &None,
                "1.96.0",
                "x86_64-pc-windows-msvc",
                &changed_env
            )
            .unwrap()
        );
        source.trusted_lock_digest.push_str("changed");
        assert_ne!(
            key,
            compatibility_key(
                &project,
                &source,
                &None,
                "1.96.0",
                "x86_64-pc-windows-msvc",
                &env
            )
            .unwrap()
        );
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            key, changed,
            "Engine source edits must not discard unrelated Cargo artifacts"
        );
    }

    #[test]
    fn real_cargo_reuses_stable_sdk_project_and_rebuilds_only_changed_unit() {
        let root = std::env::temp_dir().join(format!(
            "aife-ci-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let staged = root.join("staged");
        fs::create_dir_all(staged.join("RuntimeModule/src")).unwrap();
        fs::create_dir_all(staged.join("RuntimeModuleBuild")).unwrap();
        let sdk = default_engine_sdk_root().join("crates/project_game_sdk");
        // An editable engine dependency above the unchanged real SDK exercises
        // Cargo's reverse dependency rebuild without mutating workspace sources.
        for name in ENGINE_PLAYER_RUNTIME_CRATES {
            fs::create_dir_all(root.join("crates").join(name)).unwrap();
        }
        let engine = root.join("crates/engine_input");
        fs::write(engine.join("Cargo.toml"), format!("[package]\nname='incremental_engine'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='lib.rs'\n[dependencies]\nproject_game_sdk={{path={:?}}}\n", sdk.to_string_lossy().replace('\\', "/"))).unwrap();
        let engine_source = engine.join("lib.rs");
        fs::write(
            &engine_source,
            "pub fn engine_value() -> u32 { 1 }\npub use project_game_sdk::SessionCreateRequest;\n",
        )
        .unwrap();
        let manifest = format!("[package]\nname='incremental_fixture'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='../RuntimeModule/src/lib.rs'\n[dependencies]\nincremental_engine={{path={:?}}}\n", engine.to_string_lossy().replace('\\', "/"));
        fs::write(staged.join("RuntimeModuleBuild/Cargo.toml"), manifest).unwrap();
        let source = staged.join("RuntimeModule/src/lib.rs");
        fs::write(
            &source,
            "pub fn value() -> u32 { 1 + incremental_engine::engine_value() }\npub use incremental_engine::SessionCreateRequest;\n",
        )
        .unwrap();
        let build_root = root.join("player-build");
        fs::create_dir_all(&build_root).unwrap();
        let project = default_engine_sdk_root()
            .canonicalize()
            .unwrap()
            .parent()
            .unwrap()
            .join("samples/complex_shooter_project");
        let mut plan =
            ProjectRuntimeProductionStaging::plan(&project, &default_engine_sdk_root()).unwrap();
        plan.sdk_root = root.clone();
        let mut first_workspace = None;
        for round in 0..4 {
            if round == 2 {
                fs::write(&source, "pub fn value() -> u32 { 2 + incremental_engine::engine_value() }\npub use incremental_engine::SessionCreateRequest;\n").unwrap();
            }
            if round == 3 {
                fs::write(&engine_source, "pub fn engine_value() -> u32 { 2 }\npub use project_game_sdk::SessionCreateRequest;\n").unwrap();
            }
            let key = compatibility_key(
                &project,
                &plan,
                &None,
                "fixture",
                "host",
                &Default::default(),
            )
            .unwrap();
            let workspace =
                CompileWorkspace::acquire(&build_root.canonicalize().unwrap(), &key).unwrap();
            if let Some(first) = &first_workspace {
                assert_eq!(first, &workspace.root);
            } else {
                first_workspace = Some(workspace.root.clone());
            }
            workspace.sync(&staged).unwrap();
            let started = std::time::Instant::now();
            let result = run_bounded_child_process(BoundedChildProcessRequest {
                executable: std::env::var_os("CARGO")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| "cargo".into()),
                args: ["build", "--offline", "--message-format=json"]
                    .iter()
                    .map(OsString::from)
                    .collect(),
                current_dir: workspace.root.join("RuntimeModuleBuild"),
                environment: vec![
                    (
                        "CARGO_TARGET_DIR".into(),
                        workspace.root.join("target").into_os_string(),
                    ),
                    ("RUSTUP_AUTO_INSTALL".into(), "0".into()),
                ],
                timeout: Duration::from_secs(90),
                stdout_capture_limit_bytes: 1024 * 1024,
                stderr_capture_limit_bytes: 1024 * 1024,
                priority: Default::default(),
            });
            assert!(result.owned_process_cleanup_confirmed(), "{result:?}");
            assert_eq!(result.exit_code, Some(0), "{result:?}");
            let artifacts: Vec<serde_json::Value> = result
                .stdout_summary
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .filter(|value: &serde_json::Value| value["reason"] == "compiler-artifact")
                .collect();
            let fixture = artifacts
                .iter()
                .find(|value| value["target"]["name"] == "incremental_fixture")
                .unwrap();
            assert_eq!(fixture["fresh"], round == 1, "round={round}: {artifacts:?}");
            if round > 0 {
                assert!(artifacts
                    .iter()
                    .filter(|v| v["target"]["name"] != "incremental_fixture"
                        && v["target"]["name"] != "incremental_engine")
                    .all(|v| v["fresh"] == true));
                assert_eq!(
                    artifacts
                        .iter()
                        .find(|v| v["target"]["name"] == "incremental_engine")
                        .unwrap()["fresh"],
                    round != 3
                );
            }
            eprintln!(
                "incremental SDK round={round}, elapsed_ms={}, project_fresh={}",
                started.elapsed().as_millis(),
                fixture["fresh"]
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stable_workspace_preserves_unchanged_files_removes_deleted_sources_and_excludes_competitors()
    {
        let root = std::env::temp_dir().join(format!(
            "aife-incremental-{}-{}",
            std::process::id(),
            ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let staged = root.join("staged");
        fs::create_dir_all(staged.join("RuntimeModule/src")).unwrap();
        fs::write(staged.join("RuntimeModule/src/lib.rs"), "pub fn f() {}\n").unwrap();
        let workspace = CompileWorkspace::acquire(&root, "test").unwrap();
        workspace.sync(&staged).unwrap();
        let file = workspace.root.join("RuntimeModule/src/lib.rs");
        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        assert!(CompileWorkspace::acquire(&root, "test").is_err());
        workspace.sync(&staged).unwrap();
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), modified);
        fs::remove_file(staged.join("RuntimeModule/src/lib.rs")).unwrap();
        workspace.sync(&staged).unwrap();
        assert!(!file.exists());
        drop(workspace);
        assert!(CompileWorkspace::acquire(&root, "test").is_ok());
        let first_key = "0123456789abcdef-first";
        let second_key = "0123456789abcdef-second";
        drop(CompileWorkspace::acquire(&root, first_key).unwrap());
        assert!(CompileWorkspace::acquire(&root, second_key).is_err());
        let recovery = CompileWorkspace::acquire(&root, "recovery").unwrap();
        fs::write(recovery.root.join("ownership-unclosed"), "unclosed").unwrap();
        drop(recovery);
        assert!(CompileWorkspace::acquire(&root, "recovery").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
