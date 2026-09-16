use super::*;
use std::collections::BTreeMap;

// This is an internal receipt for one engine build, not another project/tool cache.
#[derive(Serialize, Deserialize)]
struct PreparedEnginePlayer {
    identity: String,
    host_hash: String,
    dll_hash: String,
}

#[derive(Serialize, Deserialize)]
struct ProjectArtifactFiles {
    engine_player_identity: String,
    hashes: BTreeMap<String, String>,
}

fn artifact_hashes(root: &Path) -> Result<BTreeMap<String, String>, ProjectPlayerArtifactError> {
    [
        generated_host_executable(root),
        root.join("engine_runtime.dll"),
        root.join("project_runtime_module.dll"),
    ]
    .into_iter()
    .map(|path| {
        Ok((
            path.strip_prefix(root)
                .expect("artifact-owned path")
                .to_string_lossy()
                .into_owned(),
            file_hash(&path)?,
        ))
    })
    .collect()
}

pub(super) fn seal_artifact_files(
    root: &Path,
    identity: &str,
) -> Result<(), ProjectPlayerArtifactError> {
    let receipt = ProjectArtifactFiles {
        engine_player_identity: identity.into(),
        hashes: artifact_hashes(root)?,
    };
    fs::write(
        root.join("player-files.json"),
        serde_json::to_vec(&receipt).expect("artifact receipt serializes"),
    )
    .map_err(incremental::io_error)
}

pub(super) fn validate_artifact_files(root: &Path) -> Result<String, ProjectPlayerArtifactError> {
    let bytes = fs::read(root.join("player-files.json")).map_err(incremental::io_error)?;
    let receipt: ProjectArtifactFiles = serde_json::from_slice(&bytes).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_files_invalid",
            error.to_string(),
        )
    })?;
    if receipt.hashes != artifact_hashes(root)? || receipt.engine_player_identity.is_empty() {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_files_invalid",
            "Cached Host/Engine DLL/Project DLL differs from the successful build.",
        ));
    }
    Ok(receipt.engine_player_identity)
}

pub(super) fn sealed_artifact_module(
    executable: &Path,
) -> Result<Option<PathBuf>, ProjectPlayerArtifactError> {
    if let Some(root) = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    {
        if generated_host_executable(root) == executable && root.join("player-files.json").exists()
        {
            validate_artifact_files(root)?;
            return Ok(Some(root.join("project_runtime_module.dll")));
        }
    }
    Ok(None)
}

fn workspace_key(
    sdk: &Path,
    cargo: &Path,
    channel: &str,
    target: &str,
    environment: &BTreeMap<String, String>,
) -> String {
    sha256_prefixed(
        &serde_json::to_vec(&(
            "fixed-engine-player.v1:dev:debug=0:real-window",
            sdk,
            cargo,
            channel,
            target,
            environment,
        ))
        .expect("engine build identity is serializable"),
    )
}

fn artifact_identity(workspace: &str, source_digest: &str) -> String {
    sha256_prefixed(format!("{workspace}|{source_digest}").as_bytes())
}

fn file_hash(path: &Path) -> Result<String, ProjectPlayerArtifactError> {
    Ok(sha256_prefixed(
        &fs::read(path).map_err(incremental::io_error)?,
    ))
}

fn receipt_matches(receipt: &Path, identity: &str, host: &Path, dll: &Path) -> bool {
    let Ok(bytes) = fs::read(receipt) else {
        return false;
    };
    let Ok(prepared) = serde_json::from_slice::<PreparedEnginePlayer>(&bytes) else {
        return false;
    };
    prepared.identity == identity
        && file_hash(host).ok().as_ref() == Some(&prepared.host_hash)
        && file_hash(dll).ok().as_ref() == Some(&prepared.dll_hash)
}

pub(super) fn prepare_and_stage(
    report: &mut ProjectRuntimePlayerArtifactBuildReport,
    build_root: &Path,
    sdk: &Path,
    staging: &Path,
    cargo: &Path,
    channel: &str,
    target: &str,
    environment_identity: &BTreeMap<String, String>,
    timeout_ms: u64,
    capture_limit: usize,
) -> Result<(), ProjectPlayerArtifactError> {
    let key = workspace_key(sdk, cargo, channel, target, environment_identity);
    let identity = artifact_identity(&key, &engine_sdk_source_digest(sdk)?);
    // The typed engine key already isolates this workspace from project keys.
    // Avoid an extra directory level: MSVC build-script object paths are bounded.
    let workspace = incremental::CompileWorkspace::acquire(build_root, &key)?;
    let target_root = workspace.root.join("target");
    let output = target_root.join(target).join("debug");
    let host = output.join("ai_project_runtime_player.exe");
    let dll = output.join("engine_runtime.dll");
    let receipt = workspace.root.join("prepared-engine-player.json");
    report.engine_player_identity = Some(identity.clone());
    let reusable = receipt_matches(&receipt, &identity, &host, &dll)
        && validate_engine_runtime_dll(&dll).is_ok();
    report.engine_player_cache_status = Some(if reusable { "hit" } else { "rebuilt" }.into());
    if !reusable {
        // No success receipt survives a failed rebuild. Cargo keeps valid dependency objects.
        if receipt.exists() {
            fs::remove_file(&receipt).map_err(incremental::io_error)?;
        }
        // Cargo fingerprints do not authenticate externally modified final binaries.
        // Remove only these two build-owned outputs so a bad receipt cannot bless them.
        for file in [&host, &dll] {
            if file.is_file() {
                fs::remove_file(file).map_err(incremental::io_error)?;
            }
        }
        let environment = vec![
            (
                OsString::from("CARGO_TARGET_DIR"),
                target_root.into_os_string(),
            ),
            (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
            (OsString::from("CARGO_INCREMENTAL"), OsString::from("1")),
            (OsString::from("RUSTUP_AUTO_INSTALL"), OsString::from("0")),
            (OsString::from("RUSTUP_TOOLCHAIN"), OsString::from(channel)),
            (OsString::from("CARGO_BUILD_TARGET"), OsString::from(target)),
            (
                OsString::from("CARGO_PROFILE_DEV_DEBUG"),
                OsString::from("0"),
            ),
        ];
        let project_workspace = report
            .compile_workspace
            .replace(workspace.root.display().to_string());
        let result = run_required_cargo_step(
            report,
            "build_engine_player",
            cargo,
            [
                "build",
                "-p",
                "runtime_cli",
                "-p",
                "engine_runtime_host",
                "--features",
                "runtime_cli/real-window",
                "--locked",
                "--offline",
                "--message-format=json",
            ],
            sdk,
            &environment,
            timeout_ms,
            capture_limit,
        );
        report.compile_workspace = project_workspace;
        result?;
        validate_engine_runtime_dll(&dll)?;
        let prepared = PreparedEnginePlayer {
            identity,
            host_hash: file_hash(&host)?,
            dll_hash: file_hash(&dll)?,
        };
        fs::write(
            &receipt,
            serde_json::to_vec(&prepared).expect("engine receipt is serializable"),
        )
        .map_err(incremental::io_error)?;
    }
    // The workspace lock remains held until both files have been staged together.
    let staged_host = generated_host_executable(staging);
    fs::create_dir_all(staged_host.parent().expect("artifact host parent"))
        .map_err(incremental::io_error)?;
    fs::copy(&host, staged_host).map_err(incremental::io_error)?;
    fs::copy(&dll, staging.join("engine_runtime.dll")).map_err(incremental::io_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_artifact_hit_checks_each_binary_before_describing_or_exporting() {
        let root = std::env::temp_dir().join(format!(
            "aife-artifact-files-{}-{}",
            std::process::id(),
            ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let host = generated_host_executable(&root);
        fs::create_dir_all(host.parent().unwrap()).unwrap();
        let files = [
            host,
            root.join("engine_runtime.dll"),
            root.join("project_runtime_module.dll"),
        ];
        for file in &files {
            fs::write(file, b"original").unwrap();
        }
        assert!(validate_artifact_files(&root).is_err());
        seal_artifact_files(&root, "engine-id").unwrap();
        assert_eq!(validate_artifact_files(&root).unwrap(), "engine-id");
        assert_eq!(
            sealed_artifact_module(&generated_host_executable(&root)).unwrap(),
            Some(root.join("project_runtime_module.dll"))
        );
        assert_eq!(
            sealed_artifact_module(&root.join("Game.exe")).unwrap(),
            None
        );
        for file in &files {
            fs::write(file, b"same-ABI-different-bytes").unwrap();
            assert!(validate_artifact_files(&root).is_err());
            assert!(sealed_artifact_module(&generated_host_executable(&root)).is_err());
            fs::write(file, b"original").unwrap();
            assert!(validate_artifact_files(&root).is_ok());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn engine_identity_excludes_project_inputs_and_tracks_build_inputs() {
        let sdk = Path::new("sdk");
        let cargo = Path::new("cargo");
        let env = BTreeMap::new();
        let key = workspace_key(sdk, cargo, "1.96", "windows", &env);
        // Project paths/code/AOT are deliberately absent from this interface.
        assert_eq!(key, workspace_key(sdk, cargo, "1.96", "windows", &env));
        assert_ne!(key, workspace_key(sdk, cargo, "1.97", "windows", &env));
        assert_ne!(key, workspace_key(sdk, cargo, "1.96", "other", &env));
        assert_ne!(
            key,
            workspace_key(sdk, Path::new("other-cargo"), "1.96", "windows", &env)
        );
        let mut changed = env.clone();
        changed.insert("RUSTFLAGS".into(), "-C opt-level=2".into());
        assert_ne!(key, workspace_key(sdk, cargo, "1.96", "windows", &changed));
        assert_ne!(
            artifact_identity(&key, "source1"),
            artifact_identity(&key, "source2")
        );
    }

    #[test]
    fn engine_receipt_rejects_missing_corrupt_or_wrong_identity_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "aife-engine-receipt-{}-{}",
            std::process::id(),
            ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let host = root.join("Host.exe");
        let dll = root.join("Engine.dll");
        let receipt = root.join("receipt.json");
        fs::write(&host, b"host").unwrap();
        fs::write(&dll, b"dll").unwrap();
        assert!(!receipt_matches(&receipt, "identity", &host, &dll));
        let data = PreparedEnginePlayer {
            identity: "identity".into(),
            host_hash: file_hash(&host).unwrap(),
            dll_hash: file_hash(&dll).unwrap(),
        };
        fs::write(&receipt, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(receipt_matches(&receipt, "identity", &host, &dll));
        assert!(!receipt_matches(&receipt, "changed", &host, &dll));
        fs::write(&host, b"corrupted").unwrap();
        assert!(!receipt_matches(&receipt, "identity", &host, &dll));
        fs::write(&host, b"host").unwrap();
        fs::write(&dll, b"corrupted").unwrap();
        assert!(!receipt_matches(&receipt, "identity", &host, &dll));
        fs::remove_file(&dll).unwrap();
        assert!(!receipt_matches(&receipt, "identity", &host, &dll));
        fs::write(&receipt, b"invalid").unwrap();
        assert!(!receipt_matches(&receipt, "identity", &host, &dll));
        fs::remove_dir_all(root).unwrap();
    }
}
