use super::*;
use runtime_cli::{
    run_bounded_child_process, BoundedChildProcessExitReason, BoundedChildProcessRequest,
    BoundedChildProcessResult,
};
use serde::Serialize;
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub source_path: String,
    pub field_path: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub location: Option<SourceLocation>,
    pub next_action: String,
}

pub(super) fn error(
    code: &str,
    message: impl Into<String>,
    location: Option<SourceLocation>,
) -> GameProjectCompilerError {
    let message = message.into();
    let next_action =
        "Correct the reported source or toolchain condition, then check a fresh snapshot.";
    let code = format!("game_project_compiler.{code}");
    let mut result = compiler_error(
        &code,
        &message,
        GameProjectCompilerStage::Check,
        next_action,
    );
    result.source_location = location.clone();
    result.diagnostics.push(CheckDiagnostic {
        code,
        severity: "error".into(),
        message,
        location,
        next_action: next_action.into(),
    });
    result
}

fn location(path: &str, field: Option<&str>) -> SourceLocation {
    SourceLocation {
        source_path: path.into(),
        field_path: field.map(str::to_string),
        line: None,
        column: None,
        generated: false,
    }
}

fn json(path: &str, bytes: &[u8]) -> Result<Value, GameProjectCompilerError> {
    serde_json::from_slice(bytes).map_err(|cause| {
        let mut at = location(path, None);
        at.line = Some(cause.line() as u64);
        at.column = Some(cause.column() as u64);
        error("source_json_invalid", cause.to_string(), Some(at))
    })
}

pub(super) fn validate_source(
    source: &CompilerSourceView,
    snapshot: &ProjectSnapshot,
    target: TargetProfile,
    stage: GameProjectCompilerStage,
) -> Result<CompilerProjectManifest, GameProjectCompilerError> {
    validate_source_inner(source, snapshot, target).map_err(|mut failure| {
        failure.stage = stage;
        failure
    })
}

fn validate_source_inner(
    source: &CompilerSourceView,
    snapshot: &ProjectSnapshot,
    target: TargetProfile,
) -> Result<CompilerProjectManifest, GameProjectCompilerError> {
    let value = json(
        PROJECT_MANIFEST_PATH,
        source.bytes(PROJECT_MANIFEST_PATH).unwrap_or_default(),
    )?;
    let manifest: CompilerProjectManifest =
        serde_json::from_value(value.clone()).map_err(|cause| {
            error(
                "manifest_invalid",
                cause.to_string(),
                Some(location(PROJECT_MANIFEST_PATH, None)),
            )
        })?;
    if manifest.schema_version != crate::PROJECT_MANIFEST_SCHEMA_VERSION {
        return Err(error(
            "manifest_schema_unsupported",
            "Expected aife-project.v2.",
            Some(location(PROJECT_MANIFEST_PATH, Some("/schemaVersion"))),
        ));
    }
    if manifest.project_id != snapshot.portable_project_identity {
        return Err(error(
            "manifest_project_identity_mismatch",
            "Manifest projectId differs from the snapshot binding.",
            Some(location(PROJECT_MANIFEST_PATH, Some("/projectId"))),
        ));
    }
    for field in ["defaultScene", "observationContract"] {
        if let Some(path) = value.get(field).and_then(Value::as_str) {
            require_source(source, path, PROJECT_MANIFEST_PATH, &format!("/{field}"))?;
        }
    }
    if let Some(bytes) = source.bytes(target.source_path()) {
        let value = json(target.source_path(), bytes)?;
        let profile: crate::BuildProfile = serde_json::from_value(value).map_err(|cause| {
            error(
                "build_profile_invalid",
                cause.to_string(),
                Some(location(target.source_path(), None)),
            )
        })?;
        if let Some(issue) = profile.validation_issues().first() {
            return Err(error(
                "build_profile_invalid",
                &issue.message,
                Some(location(
                    target.source_path(),
                    Some(&format!("/{}", issue.field)),
                )),
            ));
        }
        let (expected_target, expected_profile) = target.expected_values();
        if profile.target != expected_target || profile.profile != expected_profile {
            return Err(error(
                "build_profile_target_invalid",
                "Build profile does not match the requested target/profile.",
                Some(location(target.source_path(), Some("/target"))),
            ));
        }
    }
    // These are canonical JSON authoring domains, not arbitrary project text files.
    for path in source.paths().filter(|path| {
        [
            "Scenes/",
            "Prefabs/",
            "AUI/",
            "Input/",
            "Rules/",
            "Assets/",
            "Observations/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
            && (path.ends_with(".json") || path.ends_with(".asset"))
    }) {
        let value = json(path, source.bytes(path).unwrap())?;
        if path.starts_with("Assets/") {
            if value.get("schemaVersion").and_then(Value::as_str) == Some("audio-asset.v1")
                && value.get("sourceImage").is_some()
            {
                return Err(error(
                    "audio_source_image_unsupported",
                    "Audio assets cannot contain sourceImage; use sourceAudio.",
                    Some(location(path, Some("/sourceImage"))),
                ));
            }
            if let Some(image) = value.get("sourceImage").and_then(Value::as_str) {
                require_source(source, image, path, "/sourceImage")?;
            }
            if value.get("schemaVersion").and_then(Value::as_str) == Some("audio-asset.v1") {
                let audio = value
                    .get("sourceAudio")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        error(
                            "audio_source_missing",
                            "Audio assets require sourceAudio.",
                            Some(location(path, Some("/sourceAudio"))),
                        )
                    })?;
                require_source(source, audio, path, "/sourceAudio")?;
                engine_runtime::audio::decode_audio_wav(source.bytes(audio).unwrap()).map_err(
                    |message| {
                        error(
                            "audio_decode_failed",
                            &message,
                            Some(location(path, Some("/sourceAudio"))),
                        )
                    },
                )?;
            }
        }
    }
    if snapshot.qualification != ProjectQualification::Ready {
        return Err(error(
            "project_invalid",
            "Snapshot qualification is invalid; source validation cannot qualify this input.",
            Some(location(PROJECT_MANIFEST_PATH, None)),
        ));
    }
    Ok(manifest)
}

fn require_source(
    source: &CompilerSourceView,
    reference: &str,
    origin: &str,
    field: &str,
) -> Result<(), GameProjectCompilerError> {
    if crate::ProjectRelativePath::parse(reference).is_err() || source.bytes(reference).is_none() {
        return Err(error(
            "source_reference_missing",
            format!("Referenced source '{reference}' is missing or outside the project."),
            Some(location(origin, Some(field))),
        ));
    }
    Ok(())
}

pub(super) fn check_rust(
    source: &CompilerSourceView,
    manifest: &CompilerProjectManifest,
    profile: &CheckProfile,
) -> Result<(Vec<CheckDiagnostic>, String), GameProjectCompilerError> {
    let Some(module) = manifest
        .runtime_module
        .as_ref()
        .filter(|module| module.module_id != "engine.empty.runtime")
    else {
        return Ok((Vec::new(), "not_applicable".into()));
    };
    if !profile.process_approved {
        return Err(error(
            "check_process_approval_required",
            "Project Rust checks require host process approval; no process was started.",
            None,
        ));
    }
    let glue = crate::generated_runtime_glue::generate_runtime_glue(
        source,
        Some(module),
        profile.target_profile,
    )
    .map_err(|cause| {
        error(
            "check_glue_invalid",
            cause.message,
            Some(location(PROJECT_MANIFEST_PATH, Some("/runtimeModule"))),
        )
    })?
    .ok_or_else(|| {
        error(
            "check_sdk_unsupported",
            "Only Project Game SDK Rust modules can be checked.",
            Some(location(
                PROJECT_MANIFEST_PATH,
                Some("/runtimeModule/projectGameSdk"),
            )),
        )
    })?;
    let root = scratch_root()?;
    let outcome = check_in_scratch(source, profile, &glue, &root);
    if outcome
        .as_ref()
        .err()
        .is_some_and(|failure| failure.code().ends_with("check_cleanup_unconfirmed"))
    {
        return Err(error(
            "check_cleanup_unconfirmed",
            format!(
                "Retained {} because child ownership was not closed: {outcome:?}",
                root.display()
            ),
            None,
        ));
    }
    // Only this invocation created root. Child ownership is closed before removal.
    if let Err(cause) = fs::remove_dir_all(&root) {
        return Err(error(
            "check_cleanup_failed",
            format!(
                "Check result: {outcome:?}; retained {}: {cause}",
                root.display()
            ),
            None,
        ));
    }
    outcome.map(|diagnostics| (diagnostics, "passed".into()))
}

fn scratch_root() -> Result<PathBuf, GameProjectCompilerError> {
    let parent = std::env::temp_dir().canonicalize().map_err(io_error)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|cause| error("check_clock_failed", cause.to_string(), None))?
        .as_nanos();
    let root = parent.join(format!(
        "aife-compiler-check-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir(&root).map_err(io_error)?;
    Ok(root)
}

fn io_error(cause: std::io::Error) -> GameProjectCompilerError {
    error("check_io_failed", cause.to_string(), None)
}

fn check_in_scratch(
    source: &CompilerSourceView,
    profile: &CheckProfile,
    glue: &crate::PreparedRuntimeGlue,
    root: &Path,
) -> Result<Vec<CheckDiagnostic>, GameProjectCompilerError> {
    let frozen = root.join("Source");
    fs::create_dir(&frozen).map_err(io_error)?;
    for (path, bytes) in &source.files {
        crate::ProjectRelativePath::parse(path).map_err(|cause| {
            error(
                "check_source_path_invalid",
                cause.to_string(),
                Some(location(path, None)),
            )
        })?;
        let destination = frozen.join(path);
        fs::create_dir_all(destination.parent().unwrap()).map_err(io_error)?;
        fs::write(destination, bytes).map_err(io_error)?;
    }
    let plan = crate::ProjectRuntimeProductionStaging::plan(&frozen, &profile.engine_sdk_root)
        .map_err(|cause| {
            error(
                "check_staging_rejected",
                cause.message,
                Some(location("RuntimeModule/Cargo.toml", None)),
            )
        })?;
    let staged = root.join("Staged");
    crate::ProjectRuntimeProductionStaging::stage(&frozen, &staged, &plan)
        .map_err(|cause| error("check_staging_failed", cause.message, None))?;
    glue.materialize(
        &staged.join("RuntimeGlue"),
        &plan.sdk_root,
        Path::new("../RuntimeModuleBuild"),
    )
    .map_err(|cause| error("check_glue_materialize_failed", cause.message, None))?;
    let cargo_dir = staged.join("RuntimeGlue");
    fs::write(cargo_dir.join("Cargo.lock"), &plan.dependency_lock_bytes).map_err(io_error)?;
    let toolchain: toml::Value = toml::from_str(
        &fs::read_to_string(plan.sdk_root.join("rust-toolchain.toml")).map_err(io_error)?,
    )
    .map_err(|cause| error("check_toolchain_invalid", cause.to_string(), None))?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            error(
                "check_toolchain_invalid",
                "SDK toolchain channel is missing.",
                None,
            )
        })?;
    let target = format!(
        "{}-pc-windows-{}",
        std::env::consts::ARCH,
        if cfg!(target_env = "msvc") {
            "msvc"
        } else {
            "gnu"
        }
    );
    let started = Instant::now();
    let run = |args: &[&str]| {
        let remaining = Duration::from_millis(profile.timeout_ms).saturating_sub(started.elapsed());
        run_bounded_child_process(BoundedChildProcessRequest {
            executable: profile.cargo_executable.clone(),
            args: args.iter().map(OsString::from).collect(),
            current_dir: cargo_dir.clone(),
            environment: vec![
                (
                    "CARGO_TARGET_DIR".into(),
                    root.join("target").into_os_string(),
                ),
                ("CARGO_NET_OFFLINE".into(), "true".into()),
                ("RUSTUP_AUTO_INSTALL".into(), "0".into()),
                ("RUSTUP_TOOLCHAIN".into(), channel.into()),
                ("CARGO_BUILD_TARGET".into(), target.clone().into()),
            ],
            timeout: remaining,
            stdout_capture_limit_bytes: 1024 * 1024,
            stderr_capture_limit_bytes: 1024 * 1024,
            priority: Default::default(),
        })
    };
    // Normalize only the derived lock; canonical project lock bytes are never changed.
    let lock = run(&["generate-lockfile", "--offline"]);
    validate_process(&lock)?;
    let result = run(&[
        "check",
        "--lib",
        "--locked",
        "--offline",
        "--message-format=json",
    ]);
    let parsed = cargo_diagnostics(&result.stdout_summary, &cargo_dir);
    if let Err(mut failure) = validate_process(&result) {
        let diagnostics = parsed.unwrap_or_default();
        if let Some(first) = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == "error")
        {
            failure.source_location = first.location.clone();
        }
        failure.diagnostics.extend(diagnostics);
        return Err(failure);
    }
    let diagnostics = parsed?;
    let finished = result
        .stdout_summary
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .any(|value| {
            value.get("reason").and_then(Value::as_str) == Some("build-finished")
                && value.get("success").and_then(Value::as_bool) == Some(true)
        });
    if !finished {
        return Err(error(
            "check_cargo_output_invalid",
            "Cargo did not emit a successful build-finished message.",
            None,
        ));
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err(error(
            "check_rust_failed",
            "Cargo emitted errors despite a successful process exit.",
            None,
        ));
    }
    Ok(diagnostics)
}

fn validate_process(result: &BoundedChildProcessResult) -> Result<(), GameProjectCompilerError> {
    if !result.owned_process_cleanup_confirmed() {
        return Err(error(
            "check_cleanup_unconfirmed",
            format!(
                "Cargo ownership not closed: {:?}; kill={:?}; wait={:?}; reader={:?}",
                result.ownership, result.kill_error, result.wait_error, result.reader_join_error
            ),
            None,
        ));
    }
    if result.exit_reason != BoundedChildProcessExitReason::Completed
        || result.exit_code != Some(0)
        || result.stdout_truncated
        || result.stderr_truncated
    {
        return Err(error("check_cargo_failed", format!("Cargo {:?} (exit {:?}); stdoutTruncated={}, stderrTruncated={}; spawn={:?}; stderr={}", result.exit_reason, result.exit_code, result.stdout_truncated, result.stderr_truncated, result.spawn_error, result.stderr_summary), None));
    }
    Ok(())
}

fn cargo_diagnostics(
    output: &str,
    cargo_dir: &Path,
) -> Result<Vec<CheckDiagnostic>, GameProjectCompilerError> {
    let messages = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .map_err(|cause| error("check_cargo_output_invalid", cause.to_string(), None))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut diagnostics = Vec::new();
    for value in messages {
        if value.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let message: CargoDiagnostic =
            serde_json::from_value(value.get("message").cloned().unwrap_or(Value::Null))
                .map_err(|cause| error("check_cargo_output_invalid", cause.to_string(), None))?;
        let at = message
            .spans
            .iter()
            .find(|span| span.is_primary)
            .and_then(|span| source_span(span, cargo_dir));
        diagnostics.push(CheckDiagnostic {
            code: message.code.map(|code| code.code).unwrap_or_else(|| "rustc".into()),
            severity: message.level,
            message: message.message,
            location: at,
            next_action: "Repair the reported project source; generated locations identify Compiler glue, not user-editable output.".into(),
        });
    }
    Ok(diagnostics)
}

#[derive(Deserialize)]
struct CargoDiagnostic {
    message: String,
    level: String,
    code: Option<CargoCode>,
    spans: Vec<CargoSpan>,
}

#[derive(Deserialize)]
struct CargoCode {
    code: String,
}

#[derive(Deserialize)]
struct CargoSpan {
    file_name: String,
    is_primary: bool,
    line_start: u64,
    column_start: u64,
}

fn source_span(span: &CargoSpan, cargo_dir: &Path) -> Option<SourceLocation> {
    let path = Path::new(&span.file_name);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cargo_dir.join(path)
    }
    .canonicalize()
    .ok()?;
    let staged = cargo_dir.parent()?;
    let (source_path, generated) = if let Ok(relative) =
        absolute.strip_prefix(staged.join("RuntimeModule").canonicalize().ok()?)
    {
        (
            format!(
                "RuntimeModule/{}",
                relative.to_string_lossy().replace('\\', "/")
            ),
            false,
        )
    } else if let Ok(relative) = absolute.strip_prefix(cargo_dir.canonicalize().ok()?) {
        (
            format!(
                "generated/RuntimeGlue/{}",
                relative.to_string_lossy().replace('\\', "/")
            ),
            true,
        )
    } else {
        (absolute.to_string_lossy().into_owned(), false)
    };
    Some(SourceLocation {
        source_path,
        field_path: None,
        line: Some(span.line_start),
        column: Some(span.column_start),
        generated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_source_check_rejects_source_image_with_source_location() {
        let source = CompilerSourceView {
            files: BTreeMap::from([
                (PROJECT_MANIFEST_PATH.into(), br#"{"schemaVersion":"aife-project.v2","projectId":"audio-test"}"#.to_vec()),
                ("Assets/clip.asset".into(), br#"{"schemaVersion":"audio-asset.v1","assetId":"clip","sourceAudio":"Assets/Audio/clip.wav","sourceImage":"Assets/Textures/clip.png"}"#.to_vec()),
                ("Assets/Audio/clip.wav".into(), b"RIFF\x2c\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x08\0\0\0\0\x80\0\0\0\x40\xff\x7f".to_vec()),
                ("Assets/Textures/clip.png".into(), b"not-a-png".to_vec()),
            ]),
        };
        let snapshot = ProjectSnapshot {
            schema_version: PROJECT_SNAPSHOT_SCHEMA_VERSION.into(),
            snapshot_id: "audio-snapshot".into(),
            portable_project_identity: "audio-test".into(),
            opened_project_binding: "audio-binding".into(),
            revision: authoring_project_context::ProjectRevision {
                schema_version: "project-revision.v1".into(),
                portable_project_identity: "audio-test".into(),
                source_policy_version: "project-source-policy.v1".into(),
                source_digest: "sha256:audio-source".into(),
                revision_id: "audio-revision".into(),
                qualification: ProjectQualification::Ready,
                diagnostics_digest: "sha256:diagnostics".into(),
            },
            qualification: ProjectQualification::Ready,
            files: Vec::new(),
        };
        let error =
            validate_source_inner(&source, &snapshot, TargetProfile::WindowsDev).unwrap_err();
        assert_eq!(
            error.code(),
            "game_project_compiler.audio_source_image_unsupported"
        );
        assert_eq!(
            error.source_location().unwrap().source_path,
            "Assets/clip.asset"
        );
        assert_eq!(
            error.source_location().unwrap().field_path.as_deref(),
            Some("/sourceImage")
        );
    }

    #[test]
    fn check_cargo_messages_are_structured_and_malformed_output_is_not_success() {
        assert!(cargo_diagnostics("not-json", Path::new(".")).is_err());
        assert!(cargo_diagnostics(
            r#"{"reason":"compiler-message","message":{}}"#,
            Path::new(".")
        )
        .is_err());
        let messages = cargo_diagnostics(r#"{"reason":"compiler-message","message":{"level":"error","message":"type mismatch","code":{"code":"E0308"},"spans":[]}}"#, Path::new(".")).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].code, "E0308");
        assert!(messages[0].location.is_none());
    }
}
