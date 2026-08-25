#[cfg(feature = "real-window")]
fn main() {
    if let Err(error) = authority_main() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(not(feature = "real-window"))]
fn main() {
    eprintln!("editor_ui_authority requires --features real-window");
    std::process::exit(2);
}

#[cfg(feature = "real-window")]
fn authority_main() -> Result<(), String> {
    use editor_window_winit::{
        run_real_native_editor_authority, EditorReachabilityDiagnosticSeverity,
        EditorReachabilityReportLevel, EditorReachabilityStatus, EditorScreenshotEvidence,
        EditorScreenshotEvidenceKind, EditorUiReachabilityReport, RealNativeEditorAuthorityOptions,
        EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION,
    };
    use engine_runtime::canonical_digest::sha256_prefixed;
    use std::path::PathBuf;

    let args = AuthorityArgs::parse()?;
    if matches!(
        args.scenario_id.as_str(),
        "258-main-to-floating" | "258-floating-redock-close"
    ) {
        return run_258_workspace_authority(&args);
    }
    std::env::set_var(
        editor_window_winit::EDITOR_WORKSPACE_LAYOUT_ROOT_OVERRIDE_ENV,
        &args.workspace_layout_store_root,
    );
    let outcome = run_real_native_editor_authority(RealNativeEditorAuthorityOptions {
        physical_width: args.width,
        physical_height: args.height,
        report_level: EditorReachabilityReportLevel::Trace,
        project_root: args.project_root.clone(),
        workspace_layout_store_root: Some(args.workspace_layout_store_root.clone()),
        click_widget_id: (!args.widget_id.is_empty()).then(|| args.widget_id.clone()),
        wheel_delta: (args.input_kind == "wheel").then_some(-120),
        drag_target_widget_id: args.drag_target_widget_id.clone(),
        drag_delta: args.drag_delta,
        scenario_path: args.production_authority_scenario.clone(),
    });
    let mut diagnostics = Vec::new();
    if args.production_authority_scenario.is_some() {
        match outcome.production_authority_report.as_ref() {
            Some(report) if report.status == "passed" => {}
            Some(report) => diagnostics.push(authority_error(
                "authority.production_scenario_failed",
                format!(
                    "Production authority scenario finished with status {}.",
                    report.status
                ),
                "production_authority",
                "Inspect the production authority report and its first failed step.",
            )),
            None => diagnostics.push(authority_error(
                "authority.production_scenario_report_missing",
                "Production authority scenario did not produce a terminal report.",
                "production_authority",
                "Inspect the authority event-loop termination path.",
            )),
        }
    }
    if (outcome.scale_factor - args.expected_scale).abs() > 0.01 {
        diagnostics.push(authority_error(
            "authority.scale_factor_mismatch",
            format!(
                "Expected OS scale factor {:.2}, but winit reported {:.4}.",
                args.expected_scale, outcome.scale_factor
            ),
            "windows.dpi",
            "Switch the monitor to the requested Windows scale and rerun this scenario.",
        ));
    }
    if outcome.physical_width != args.width || outcome.physical_height != args.height {
        diagnostics.push(authority_error(
            "authority.physical_size_mismatch",
            format!(
                "Requested {}x{}, but the actual client area is {}x{}.",
                args.width, args.height, outcome.physical_width, outcome.physical_height
            ),
            "winit.window",
            "Ensure the desktop can fit the requested client area and rerun.",
        ));
    }
    diagnostics.extend(
        outcome
            .window_report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity
                    == editor_window_winit::RealNativeEditorWindowDiagnosticSeverity::Error
            })
            .map(|diagnostic| {
                authority_error(
                    &diagnostic.code,
                    diagnostic.message.clone(),
                    &diagnostic.source_stage,
                    "Inspect the authority artifact and rerun after fixing the environment.",
                )
            }),
    );

    let scenario_dir = args.evidence_root.join(&args.scenario_id);
    std::fs::create_dir_all(&scenario_dir).map_err(|error| {
        format!(
            "authority.create_evidence_dir_failed:{}:{error}",
            scenario_dir.display()
        )
    })?;
    let screenshot_relative = PathBuf::from(&args.scenario_id).join("frame.png");
    let screenshot_path = args.evidence_root.join(&screenshot_relative);
    let screenshot = match outcome.capture.as_ref() {
        Some(capture) => {
            if capture.rgba8.len() != (capture.width * capture.height * 4) as usize
                || !capture
                    .rgba8
                    .chunks_exact(4)
                    .any(|pixel| pixel[3] != 0 && (pixel[0] != pixel[1] || pixel[1] != pixel[2]))
            {
                diagnostics.push(authority_error(
                    "authority.capture_blank_or_invalid",
                    "The captured RGBA payload is empty, malformed, or visually blank.",
                    "editor_wgpu_renderer.capture",
                    "Inspect the WGPU render pass and capture format.",
                ));
            } else {
                write_png(
                    &screenshot_path,
                    capture.width,
                    capture.height,
                    &capture.rgba8,
                )?;
            }
            let present = outcome.present_report.as_ref();
            Some(EditorScreenshotEvidence {
                kind: EditorScreenshotEvidenceKind::ActualWindowRgba,
                width: capture.width,
                height: capture.height,
                frame_index: outcome.window_report.frame_index,
                tree_revision: outcome
                    .snapshot
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.model_revision),
                rgba_sha256: Some(sha256_prefixed(&capture.rgba8)),
                artifact_path: Some(path_to_forward_slashes(&screenshot_relative)),
                backend: capture.backend.clone(),
                font: present.map_or_else(
                    || "unknown".to_string(),
                    |report| {
                        report
                            .font_source
                            .clone()
                            .unwrap_or_else(|| report.font_backend.clone())
                    },
                ),
                os: windows_version(),
                gpu: outcome.window_report.shared_gpu_backend.clone(),
            })
        }
        None => {
            diagnostics.push(authority_error(
                "authority.capture_missing",
                outcome
                    .capture_error
                    .clone()
                    .unwrap_or_else(|| "No RGBA capture was produced.".to_string()),
                "editor_wgpu_renderer.capture",
                "Run with a capture-capable WGPU backend.",
            ));
            None
        }
    };
    if outcome.snapshot.is_none() {
        diagnostics.push(authority_error(
            "authority.snapshot_missing",
            "No retained WidgetTree snapshot was produced.",
            "editor_ui_renderer.snapshot",
            "Ensure one retained frame is rendered before capture.",
        ));
    }
    let input_requested = !args.widget_id.is_empty() || args.input_kind != "click";
    if input_requested
        && outcome.input_replay.as_ref().is_none_or(|evidence| {
            evidence.route_status != EditorReachabilityStatus::Passed
                || if evidence.input_kind == "wheel" {
                    !evidence.wheel_observed
                } else {
                    !evidence.pointer_down_observed || !evidence.pointer_up_observed
                }
        })
    {
        diagnostics.push(authority_error(
            "authority.os_input_missing",
            "The target window did not produce complete foreground OS input evidence.",
            "windows.send_input",
            "Ensure the authority window can become foreground and retry.",
        ));
    }
    if args.input_kind == "drag" {
        let revision_advanced = outcome
            .workspace_layout_revision_before
            .zip(outcome.workspace_layout_revision_after)
            .is_some_and(|(before, after)| after > before);
        if !revision_advanced {
            diagnostics.push(authority_error(
                "authority.workspace_layout_revision_unchanged",
                "The real pointer drag did not commit a workspace layout revision.",
                "editor_workspace_docking",
                "Inspect the source/target geometry and retained pointer route.",
            ));
        }
        if args.drag_target_widget_id.is_some() && !outcome.workspace_drag_preview_observed {
            diagnostics.push(authority_error(
                "authority.workspace_drag_preview_missing",
                "The real tab drag did not expose a workspace drop preview.",
                "editor_workspace_docking",
                "Inspect the unified drop resolver and real pointer move route.",
            ));
        }
        if !outcome.workspace_diagnostics.is_empty() {
            diagnostics.push(authority_error(
                "authority.workspace_invariant_failed",
                format!(
                    "Workspace diagnostics after drag: {}",
                    outcome.workspace_diagnostics.join(",")
                ),
                "editor_workspace_docking",
                "Repair the workspace invariant before accepting the scenario.",
            ));
        }
    }
    let status = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == EditorReachabilityDiagnosticSeverity::Error)
    {
        EditorReachabilityStatus::Failed
    } else {
        EditorReachabilityStatus::Passed
    };
    let actual_rgba = screenshot.as_ref().map(|value| {
        serde_json::json!({
            "path": value.artifact_path,
            "sha256": value.rgba_sha256
        })
    });
    let report = EditorUiReachabilityReport {
        schema_version: EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION.to_string(),
        report_level: EditorReachabilityReportLevel::Trace,
        scenario_id: args.scenario_id.clone(),
        status,
        snapshot: outcome.snapshot,
        screenshot,
        input_replay: outcome.input_replay,
        diagnostics,
    };
    let report_path = scenario_dir.join(if args.production_authority_scenario.is_some() {
        "shell-report.json"
    } else {
        "report.json"
    });
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("authority.serialize_report_failed:{error}"))?;
    std::fs::write(&report_path, report_bytes)
        .map_err(|error| format!("authority.write_report_failed:{error}"))?;
    let workspace_path = scenario_dir.join("workspace.json");
    let screen_rect = outcome.screen_rect.map(|rect| {
        serde_json::json!({
            "x": rect.0,
            "y": rect.1,
            "width": rect.2,
            "height": rect.3
        })
    });
    let diagnostic_codes = report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();
    let workspace_bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": "editor-workspace-authority-evidence.v1",
        "source_commit": args.source_commit,
        "binary_sha256": current_binary_sha256()?,
        "scenario_id": args.scenario_id,
        "status": report.status,
        "input_kind": args.input_kind,
        "main_window_count": 1,
        "floating_window_count": 0,
        "workspace_native_lineage": [{
            "workspace_window_id": "main",
            "native_window_id": outcome.native_window_id,
            "surface_created": outcome.window_report.surface_created,
            "scale_factor": outcome.scale_factor,
            "screen_rect": screen_rect
        }],
        "proxy_created": false,
        "proxy_destroyed": false,
        "resolved_target_token": null,
        "layout_revision_before": outcome.workspace_layout_revision_before,
        "layout_revision_after": outcome.workspace_layout_revision_after,
        "drag_preview_observed": outcome.workspace_drag_preview_observed,
        "panel_unique": null,
        "actual_rgba": actual_rgba,
        "owned_process_cleanup": "event_loop_exited",
        "workspace_diagnostics": outcome.workspace_diagnostics,
        "diagnostics": diagnostic_codes,
    }))
    .map_err(|error| format!("authority.serialize_workspace_evidence_failed:{error}"))?;
    std::fs::write(&workspace_path, workspace_bytes)
        .map_err(|error| format!("authority.write_workspace_evidence_failed:{error}"))?;
    println!(
        "scenario={} status={:?} scale_factor={} report={}",
        args.scenario_id,
        report.status,
        outcome.scale_factor,
        report_path.display()
    );
    if report.status != EditorReachabilityStatus::Passed {
        return Err(format!("authority.scenario_failed:{}", args.scenario_id));
    }
    Ok(())
}

#[cfg(feature = "real-window")]
struct AuthorityArgs {
    scenario_id: String,
    width: u32,
    height: u32,
    expected_scale: f64,
    evidence_root: std::path::PathBuf,
    project_root: Option<std::path::PathBuf>,
    workspace_layout_store_root: std::path::PathBuf,
    widget_id: String,
    input_kind: String,
    drag_target_widget_id: Option<String>,
    drag_delta: Option<(i32, i32)>,
    production_authority_scenario: Option<std::path::PathBuf>,
    source_commit: String,
}

#[cfg(feature = "real-window")]
impl AuthorityArgs {
    fn parse() -> Result<Self, String> {
        let values = std::env::args().skip(1).collect::<Vec<_>>();
        let value = |name: &str| -> Result<String, String> {
            values
                .windows(2)
                .find(|pair| pair[0] == name)
                .map(|pair| pair[1].clone())
                .ok_or_else(|| format!("authority.argument_missing:{name}"))
        };
        let optional_value = |name: &str| -> Option<String> {
            values
                .windows(2)
                .find(|pair| pair[0] == name)
                .map(|pair| pair[1].clone())
        };
        let scenario_id = value("--scenario")?;
        if scenario_id.is_empty()
            || !scenario_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_.@".contains(character))
        {
            return Err("authority.invalid_scenario_id".to_string());
        }
        let input_kind = optional_value("--input-kind").unwrap_or_else(|| "click".to_string());
        if !matches!(input_kind.as_str(), "click" | "wheel" | "drag") {
            return Err("authority.invalid_input_kind".to_string());
        }
        let drag_target_widget_id = optional_value("--drag-target-widget-id");
        let drag_delta = match (
            optional_value("--drag-delta-x"),
            optional_value("--drag-delta-y"),
        ) {
            (Some(x), Some(y)) => Some((
                x.parse()
                    .map_err(|_| "authority.invalid_drag_delta_x".to_string())?,
                y.parse()
                    .map_err(|_| "authority.invalid_drag_delta_y".to_string())?,
            )),
            (None, None) => None,
            _ => return Err("authority.incomplete_drag_delta".to_string()),
        };
        if input_kind == "drag" && drag_target_widget_id.is_none() && drag_delta.is_none() {
            return Err("authority.drag_target_missing".to_string());
        }
        Ok(Self {
            scenario_id,
            width: value("--width")?
                .parse()
                .map_err(|_| "authority.invalid_width".to_string())?,
            height: value("--height")?
                .parse()
                .map_err(|_| "authority.invalid_height".to_string())?,
            expected_scale: value("--expected-scale")?
                .parse()
                .map_err(|_| "authority.invalid_expected_scale".to_string())?,
            evidence_root: std::path::PathBuf::from(value("--evidence-root")?),
            project_root: optional_value("--project-root").map(std::path::PathBuf::from),
            workspace_layout_store_root: std::path::PathBuf::from(value(
                "--workspace-layout-store-root",
            )?),
            widget_id: optional_value("--widget-id").unwrap_or_default(),
            input_kind,
            drag_target_widget_id,
            drag_delta,
            production_authority_scenario: optional_value("--production-authority-scenario")
                .map(std::path::PathBuf::from),
            source_commit: value("--source-commit")?,
        })
    }
}

#[cfg(feature = "real-window")]
fn run_258_workspace_authority(args: &AuthorityArgs) -> Result<(), String> {
    use editor_window_winit::{run_real_workspace_authority, RealWorkspaceAuthorityOptions};
    use engine_runtime::canonical_digest::sha256_prefixed;

    let project_root = args
        .project_root
        .clone()
        .ok_or_else(|| "authority.argument_missing:--project-root".to_string())?;
    let outcome = run_real_workspace_authority(RealWorkspaceAuthorityOptions {
        physical_width: args.width,
        physical_height: args.height,
        project_root,
        workspace_layout_store_root: args.workspace_layout_store_root.clone(),
        scenario_id: args.scenario_id.clone(),
    });
    let scenario_dir = args.evidence_root.join(&args.scenario_id);
    std::fs::create_dir_all(&scenario_dir).map_err(|error| {
        format!(
            "authority.create_evidence_dir_failed:{}:{error}",
            scenario_dir.display()
        )
    })?;
    let mut diagnostics = outcome
        .window_report
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity
                == editor_window_winit::RealNativeEditorWindowDiagnosticSeverity::Error
        })
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();
    let revision_advanced = outcome
        .layout_revision_before
        .zip(outcome.layout_revision_after)
        .is_some_and(|(before, after)| after > before);
    if !revision_advanced {
        diagnostics.push("authority.workspace_layout_revision_unchanged".to_string());
    }
    if !outcome.input_observed {
        diagnostics.push("authority.os_input_missing".to_string());
    }
    if args.scenario_id == "258-main-to-floating"
        && (!outcome.proxy_created || !outcome.proxy_destroyed)
    {
        diagnostics.push("authority.proxy_lifecycle_incomplete".to_string());
    }
    if !outcome.panel_unique {
        diagnostics.push("authority.panel_not_unique".to_string());
    }
    if !outcome.workspace_diagnostics.is_empty() {
        diagnostics.push("authority.workspace_invariant_failed".to_string());
    }
    match args.scenario_id.as_str() {
        "258-main-to-floating"
            if outcome.main_window_count != 1 || outcome.floating_window_count != 1 =>
        {
            diagnostics.push("authority.floating_window_not_created".to_string());
        }
        "258-floating-redock-close"
            if outcome.main_window_count != 1
                || outcome.floating_window_count != 0
                || outcome
                    .resolved_target_token
                    .as_deref()
                    .is_none_or(|token| !token.starts_with("main:")) =>
        {
            diagnostics.push("authority.floating_redock_cleanup_failed".to_string());
        }
        _ => {}
    }
    let mut window_evidence = Vec::new();
    for window in outcome.windows {
        let file_name = format!("frame-{}.png", window.workspace_window_id);
        let path = scenario_dir.join(&file_name);
        let rgba = match window.capture {
            Some(capture) => {
                write_png(&path, capture.width, capture.height, &capture.rgba8)?;
                Some(serde_json::json!({
                    "path": path_to_forward_slashes(
                        &std::path::PathBuf::from(&args.scenario_id).join(&file_name)
                    ),
                    "sha256": sha256_prefixed(&capture.rgba8),
                    "width": capture.width,
                    "height": capture.height,
                    "backend": capture.backend
                }))
            }
            None => {
                diagnostics.push(format!(
                    "authority.capture_missing:{}",
                    window.workspace_window_id
                ));
                None
            }
        };
        window_evidence.push(serde_json::json!({
            "workspace_window_id": window.workspace_window_id,
            "native_window_id": window.native_window_id,
            "surface_created": window.surface_created,
            "scale_factor": window.scale_factor,
            "screen_rect": {
                "x": window.screen_rect.0,
                "y": window.screen_rect.1,
                "width": window.screen_rect.2,
                "height": window.screen_rect.3
            },
            "actual_rgba": rgba
        }));
    }
    let status = if diagnostics.is_empty() {
        "passed"
    } else {
        "failed"
    };
    let evidence = serde_json::json!({
        "schema_version": "editor-workspace-authority-evidence.v2",
        "source_commit": args.source_commit,
        "binary_sha256": current_binary_sha256()?,
        "scenario_id": args.scenario_id,
        "status": status,
        "main_window_count": outcome.main_window_count,
        "floating_window_count": outcome.floating_window_count,
        "workspace_native_lineage": window_evidence,
        "proxy_created": outcome.proxy_created,
        "proxy_destroyed": outcome.proxy_destroyed,
        "resolved_target_token": outcome.resolved_target_token,
        "layout_revision_before": outcome.layout_revision_before,
        "layout_revision_after": outcome.layout_revision_after,
        "panel_unique": outcome.panel_unique,
        "workspace_diagnostics": outcome.workspace_diagnostics,
        "diagnostics": diagnostics,
        "owned_process_cleanup": "event_loop_exited"
    });
    let evidence_path = scenario_dir.join("workspace.json");
    std::fs::write(
        &evidence_path,
        serde_json::to_vec_pretty(&evidence)
            .map_err(|error| format!("authority.serialize_workspace_evidence_failed:{error}"))?,
    )
    .map_err(|error| format!("authority.write_workspace_evidence_failed:{error}"))?;
    println!(
        "scenario={} status={} report={}",
        args.scenario_id,
        status,
        evidence_path.display()
    );
    if status != "passed" {
        return Err(format!("authority.scenario_failed:{}", args.scenario_id));
    }
    Ok(())
}

#[cfg(feature = "real-window")]
fn current_binary_sha256() -> Result<String, String> {
    use engine_runtime::canonical_digest::sha256_prefixed;
    let path =
        std::env::current_exe().map_err(|error| format!("authority.current_exe_failed:{error}"))?;
    let bytes = std::fs::read(&path).map_err(|error| {
        format!(
            "authority.read_current_exe_failed:{}:{error}",
            path.display()
        )
    })?;
    Ok(sha256_prefixed(&bytes))
}

#[cfg(feature = "real-window")]
fn authority_error(
    code: &str,
    message: impl Into<String>,
    source_stage: &str,
    next_action: &str,
) -> editor_window_winit::EditorReachabilityDiagnostic {
    editor_window_winit::EditorReachabilityDiagnostic {
        severity: editor_window_winit::EditorReachabilityDiagnosticSeverity::Error,
        code: code.to_string(),
        message: message.into(),
        widget_id: None,
        source_stage: source_stage.to_string(),
        next_action: Some(next_action.to_string()),
    }
}

#[cfg(feature = "real-window")]
fn write_png(path: &std::path::Path, width: u32, height: u32, rgba8: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|error| format!("authority.create_png_failed:{error}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("authority.png_header_failed:{error}"))?;
    writer
        .write_image_data(rgba8)
        .map_err(|error| format!("authority.png_write_failed:{error}"))
}

#[cfg(feature = "real-window")]
fn path_to_forward_slashes(path: &std::path::Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(feature = "real-window")]
fn windows_version() -> String {
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
}
