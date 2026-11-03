use super::*;

// Resolve before forming either workspace key: the command and its identity must
// name the same installed tool, including callers that execute a test binary directly.
pub(super) fn resolve_cargo(
    requested: Option<&Path>,
    ambient: Option<&OsStr>,
    search_path: Option<&OsStr>,
    channel: &str,
    current_dir: &Path,
) -> Result<PathBuf, ProjectPlayerArtifactError> {
    let selected = requested
        .map(PathBuf::from)
        .or_else(|| ambient.map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let executable = resolve_executable(&selected, search_path, current_dir)?;
    let rustup = executable.with_file_name(if cfg!(windows) {
        "rustup.exe"
    } else {
        "rustup"
    });
    // Rustup's cargo proxy is the same executable as rustup. Do not replace a
    // caller's custom Cargo merely because a rustup executable is next to it.
    if rustup.is_file()
        && executable != rustup
        && fs::read(&executable).map_err(io_error)? == fs::read(&rustup).map_err(io_error)?
    {
        let process = run_bounded_child_process(BoundedChildProcessRequest {
            executable: rustup,
            args: ["which", "--toolchain", channel, "cargo"]
                .map(OsString::from)
                .to_vec(),
            current_dir: current_dir.into(),
            environment: vec![("RUSTUP_AUTO_INSTALL".into(), "0".into())],
            timeout: Duration::from_secs(10),
            stdout_capture_limit_bytes: 16 * 1024,
            stderr_capture_limit_bytes: 16 * 1024,
            priority: runtime_cli::BoundedChildProcessPriority::Normal,
        });
        if process.exit_code != Some(0)
            || process.exit_reason != BoundedChildProcessExitReason::Completed
        {
            return Err(ProjectPlayerArtifactError::new(
                "project_runtime.toolchain_invalid",
                format!(
                    "Installed Cargo for SDK toolchain {channel} cannot be resolved: {}",
                    process.stderr_summary
                ),
            ));
        }
        return resolve_executable(
            Path::new(process.stdout_summary.trim()),
            search_path,
            current_dir,
        );
    }
    Ok(executable)
}

fn resolve_executable(
    selected: &Path,
    search_path: Option<&OsStr>,
    current_dir: &Path,
) -> Result<PathBuf, ProjectPlayerArtifactError> {
    let candidates = if selected.is_absolute() || selected.components().count() > 1 {
        vec![current_dir.join(selected)]
    } else {
        search_path
            .map(std::env::split_paths)
            .into_iter()
            .flatten()
            .map(|directory| current_dir.join(directory).join(selected))
            .collect()
    };
    for candidate in candidates {
        let candidate = if cfg!(windows) && candidate.extension().is_none() {
            candidate.with_extension("exe")
        } else {
            candidate
        };
        if candidate.is_file() {
            return candidate.canonicalize().map_err(io_error);
        }
    }
    Err(ProjectPlayerArtifactError::new(
        "project_runtime.toolchain_invalid",
        format!(
            "Cargo executable cannot be resolved: {}",
            selected.display()
        ),
    ))
}

pub(super) fn effective_environment_identity(
    environment: &std::collections::BTreeMap<String, String>,
    channel: &str,
    target: &str,
    profile: ProjectRuntimePlayerArtifactBuildProfile,
) -> std::collections::BTreeMap<String, String> {
    let mut effective = environment.clone();
    for (key, value) in child_build_environment(Path::new("owner-output"), channel, target, profile)
    {
        effective.insert(
            key.into_string().expect("known environment key"),
            value.into_string().expect("known build value"),
        );
    }
    // The owner selects its target directory after computing the workspace key.
    // Caller-owned scratch paths are never consumed by either Cargo child.
    effective.remove("CARGO_TARGET_DIR");
    effective
}

pub(super) fn child_build_environment(
    target_root: &Path,
    channel: &str,
    target: &str,
    profile: ProjectRuntimePlayerArtifactBuildProfile,
) -> Vec<(OsString, OsString)> {
    vec![
        (
            OsString::from("CARGO_TARGET_DIR"),
            cargo_target_directory(target_root),
        ),
        (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
        (
            OsString::from("CARGO_INCREMENTAL"),
            OsString::from(profile.cargo_incremental()),
        ),
        (OsString::from("RUSTUP_AUTO_INSTALL"), OsString::from("0")),
        (OsString::from("RUSTUP_TOOLCHAIN"), OsString::from(channel)),
        (OsString::from("CARGO_BUILD_TARGET"), OsString::from(target)),
        (
            OsString::from("CARGO_PROFILE_DEV_DEBUG"),
            OsString::from("0"),
        ),
        (
            OsString::from("CARGO_PROFILE_RELEASE_OPT_LEVEL"),
            OsString::from("3"),
        ),
        (
            OsString::from("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS"),
            OsString::from("false"),
        ),
        (
            OsString::from("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS"),
            OsString::from("false"),
        ),
    ]
}

pub(crate) fn cargo_target_directory(path: &Path) -> OsString {
    // Keep canonical paths for ownership checks. MSVC and Android NDK LLVM
    // object paths from a verbatim target directory can fail to link on Windows.
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            if let Prefix::VerbatimDisk(drive) = prefix.kind() {
                let mut target = PathBuf::from(format!("{}:", char::from(drive)));
                target.extend(components);
                return target.into_os_string();
            }
        }
    }
    path.as_os_str().to_owned()
}

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
    fn cargo_resolution_unifies_alias_ambient_and_explicit_but_preserves_custom_tool() {
        let root = std::env::temp_dir().join(format!(
            "aife-cargo-resolution-{}-{}",
            std::process::id(),
            ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let name = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let tool = root.join(name);
        fs::write(&tool, b"custom-cargo").unwrap();
        let path = std::env::join_paths([&root]).unwrap();
        let resolve = |requested: Option<&Path>, ambient: Option<&OsStr>| {
            resolve_cargo(requested, ambient, Some(&path), "1.96.0", &root).unwrap()
        };
        let expected = tool.canonicalize().unwrap();
        assert_eq!(resolve(None, None), expected);
        assert_eq!(resolve(None, Some(OsStr::new("cargo"))), expected);
        assert_eq!(resolve(Some(&tool), Some(OsStr::new("missing"))), expected);
        let other = root.join("custom.exe");
        fs::write(&other, b"different-cargo").unwrap();
        assert_ne!(resolve(Some(&other), None), expected);
        assert!(resolve_cargo(
            Some(Path::new("missing")),
            None,
            Some(&path),
            "1.96.0",
            &root
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_cargo_alias_resolves_to_sdk_toolchain_without_building() {
        let root = default_engine_sdk_root();
        let actual = resolve_cargo(
            None,
            std::env::var_os("CARGO").as_deref(),
            std::env::var_os("PATH").as_deref(),
            "1.96.0",
            &root,
        )
        .unwrap();
        let explicit = resolve_cargo(Some(&actual), None, None, "1.96.0", &root).unwrap();
        assert_eq!(actual, explicit);
        let proxy = resolve_cargo(
            Some(Path::new("cargo")),
            None,
            std::env::var_os("PATH").as_deref(),
            "1.96.0",
            &root,
        )
        .unwrap();
        assert_eq!(actual, proxy);
    }

    #[cfg(windows)]
    #[test]
    fn cargo_target_preserves_the_owned_directory_without_verbatim_drive_prefix() {
        let suffix = format!(r"\{}\target", "long-directory".repeat(20));
        let normal = PathBuf::from(format!("D:{suffix}"));
        let canonical = PathBuf::from(format!(r"\\?\D:{suffix}"));
        let environment = child_build_environment(
            &canonical,
            "1.96.0",
            "x86_64-pc-windows-msvc",
            ProjectRuntimePlayerArtifactBuildProfile::Release,
        );
        let target = environment
            .iter()
            .find(|(key, _)| key == "CARGO_TARGET_DIR")
            .unwrap();
        assert_eq!(Path::new(&target.1), normal);
        assert_eq!(cargo_target_directory(&normal), normal.as_os_str());
        for unchanged in [
            r"\\server\share\target",
            r"\\?\UNC\server\share\target",
            "target",
        ] {
            assert_eq!(cargo_target_directory(Path::new(unchanged)), unchanged);
        }
        assert!(canonical.starts_with(r"\\?\D:\"));
    }

    #[test]
    fn overridden_child_environment_does_not_invalidate_cache_but_real_inputs_do() {
        let raw = std::collections::BTreeMap::from([
            ("CARGO_TARGET_DIR".into(), "first-temporary-target".into()),
            ("CARGO_INCREMENTAL".into(), "1".into()),
            ("CARGO_BUILD_TARGET".into(), "ambient-target".into()),
            ("CARGO_PROFILE_RELEASE_OPT_LEVEL".into(), "1".into()),
            ("PATH".into(), "toolchain-path".into()),
        ]);
        let normalize = |env: &std::collections::BTreeMap<String, String>| {
            effective_environment_identity(
                env,
                "1.96.0",
                "x86_64-pc-windows-msvc",
                ProjectRuntimePlayerArtifactBuildProfile::Release,
            )
        };
        let expected = normalize(&raw);
        for name in [
            "CARGO_TARGET_DIR",
            "CARGO_INCREMENTAL",
            "CARGO_BUILD_TARGET",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL",
        ] {
            let mut changed = raw.clone();
            changed.insert(name.into(), "different-ambient-value".into());
            assert_eq!(expected, normalize(&changed), "child overrides {name}");
        }
        for name in [
            "PATH",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
            "RUSTC_WRAPPER",
            "CARGO_HOME",
        ] {
            let mut changed = raw.clone();
            changed.insert(name.into(), "changed-real-input".into());
            assert_ne!(expected, normalize(&changed), "keep real input {name}");
        }
        assert_ne!(
            expected,
            effective_environment_identity(
                &raw,
                "other-toolchain",
                "x86_64-pc-windows-msvc",
                ProjectRuntimePlayerArtifactBuildProfile::Release
            )
        );
        assert_ne!(
            expected,
            effective_environment_identity(
                &raw,
                "1.96.0",
                "other-target",
                ProjectRuntimePlayerArtifactBuildProfile::Release
            )
        );
        assert_ne!(
            expected,
            effective_environment_identity(
                &raw,
                "1.96.0",
                "x86_64-pc-windows-msvc",
                ProjectRuntimePlayerArtifactBuildProfile::Dev
            )
        );
    }

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
