use crate::assembly::{
    AssemblyDomainStatus, ComplexShooterProjectAssemblyReport, ComplexShooterProjectAssemblySpec,
    ComplexShooterProjectAssemblyValidator,
};
use crate::report::{
    ComplexProjectE2eArtifact, ComplexProjectE2eDiagnostic, ComplexProjectE2eGap,
    ComplexProjectE2eGateReport, ComplexProjectE2eStatus, ComplexProjectE2eStep,
};
use crate::sample_project::load_sample_project_summary;
use editor_core::{
    DesktopExportPipeline, DesktopExportRequest, DesktopExportStatus, ExplicitExportOutput,
};
use engine_runtime::runtime_package::load_runtime_package;
use runtime_player_winit::{
    run_headless_native_player_from_package_with_linked_modules, NativePlayerWindowRunRequest,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexProjectE2eGateRequest {
    pub project_path: PathBuf,
    pub output_root: PathBuf,
    pub frame_limit: u64,
    pub include_optional_real_window_step: bool,
    pub strict_assembly: bool,
}

impl ComplexProjectE2eGateRequest {
    pub fn new(project_path: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            output_root: output_root.into(),
            frame_limit: 6,
            include_optional_real_window_step: true,
            strict_assembly: true,
        }
    }

    pub fn sample_from_workspace(workspace_root: impl AsRef<Path>) -> Self {
        let workspace_root = workspace_root.as_ref();
        Self::new(
            workspace_root
                .join("samples")
                .join("complex_shooter_project"),
            workspace_root
                .join("target")
                .join("project_e2e_gate")
                .join("complex_shooter"),
        )
    }
}

pub fn run_complex_project_e2e_gate(
    request: ComplexProjectE2eGateRequest,
) -> ComplexProjectE2eGateReport {
    let mut report = ComplexProjectE2eGateReport::new(
        request.project_path.display().to_string(),
        request.output_root.display().to_string(),
    );

    if let Err(error) = fs::create_dir_all(&request.output_root) {
        report.diagnostics.push(
            ComplexProjectE2eDiagnostic::error(
                "GateOutputCreateFailed",
                format!("failed to create output directory: {error}"),
            )
            .with_path(request.output_root.display().to_string()),
        );
        report.recompute_status();
        return report;
    }

    match load_sample_project_summary(&request.project_path) {
        Ok(summary) => {
            report.metrics.scene_count = summary.scene_count;
            report.metrics.entity_count = summary.entity_count;
            report.metrics.prefab_count = summary.prefab_count;
            report.metrics.asset_count = summary.asset_count;
            report.metrics.rule_count = summary.rule_count;
            report.metrics.input_action_count = summary.input_action_count;
            report.metrics.aui_document_count = summary.aui_document_count;
            report.steps.push(ComplexProjectE2eStep::new(
                "sample-project-load",
                ComplexProjectE2eStatus::Passed,
                format!(
                    "loaded {} with {} scene(s), {} entity/entities, {} asset file(s)",
                    summary.project_name,
                    summary.scene_count,
                    summary.entity_count,
                    summary.asset_count
                ),
            ));
        }
        Err(diagnostics) => {
            report.diagnostics.extend(diagnostics);
            report.steps.push(ComplexProjectE2eStep::new(
                "sample-project-load",
                ComplexProjectE2eStatus::Failed,
                "sample project could not be loaded",
            ));
            finalize_report(&mut report);
            return report;
        }
    }

    if request.strict_assembly {
        let assembly_report = ComplexShooterProjectAssemblyValidator::validate(
            &ComplexShooterProjectAssemblySpec::for_project(&request.project_path),
        );
        merge_assembly_report(&mut report, &assembly_report);
        let assembly_report_path = request
            .output_root
            .join("reports")
            .join("complex-shooter-project-assembly-report.json");
        if let Err(error) = write_json(&assembly_report_path, &assembly_report) {
            report.diagnostics.push(
                ComplexProjectE2eDiagnostic::error(
                    "AssemblyReportWriteFailed",
                    format!("failed to write assembly report: {error}"),
                )
                .with_path(assembly_report_path.display().to_string()),
            );
        } else {
            report.artifacts.push(ComplexProjectE2eArtifact {
                artifact_id: "complex-shooter-project-assembly-report".to_string(),
                path: assembly_report_path.display().to_string(),
            });
        }
        if assembly_report.status == AssemblyDomainStatus::Failed {
            finalize_report(&mut report);
            return report;
        }
    }

    let mut export_request = DesktopExportRequest::windows_dev(&request.project_path)
        .with_explicit_output(ExplicitExportOutput::from_user_selected(
            &request.output_root,
        ));
    export_request.output_root = request.output_root.join("Build").join("Windows");
    export_request.frame_limit = request.frame_limit.max(1);
    let export_report = DesktopExportPipeline::export(export_request);
    report.exported_package_path = Some(export_report.package_dir.clone());
    report.artifacts.push(ComplexProjectE2eArtifact {
        artifact_id: "desktop-export-report".to_string(),
        path: export_report
            .package_dir
            .parse::<PathBuf>()
            .unwrap_or_else(|_| PathBuf::from(&export_report.package_dir))
            .join("reports")
            .join("desktop-export-report.json")
            .display()
            .to_string(),
    });
    report.artifacts.push(ComplexProjectE2eArtifact {
        artifact_id: "desktop-package".to_string(),
        path: export_report.package_dir.clone(),
    });
    report.steps.push(
        ComplexProjectE2eStep::new(
            "desktop-export",
            if export_report.status == DesktopExportStatus::Success {
                ComplexProjectE2eStatus::Passed
            } else {
                ComplexProjectE2eStatus::Failed
            },
            format!(
                "desktop export status={:?}, playerExit={:?}, runtimePackage={:?}",
                export_report.status,
                export_report.player_exit_code,
                export_report.runtime_package_status
            ),
        )
        .with_artifact_path(export_report.package_dir.clone()),
    );
    for diagnostic in &export_report.diagnostics {
        report.diagnostics.push(ComplexProjectE2eDiagnostic {
            severity: format!("{:?}", diagnostic.severity).to_ascii_lowercase(),
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            path: diagnostic.path.clone(),
        });
    }

    let runtime_package_path = PathBuf::from(&export_report.runtime_package_dir);
    let load = load_runtime_package(&runtime_package_path);
    if let Some(package) = load.value {
        report.metrics.runtime_package_entity_count = package.active_scene.entities.len();
        report.metrics.aui_package_document_count = package.aui_manifest.documents.len();
        report.metrics.aui_loaded_document_count = package.aui_documents.len();
        report.steps.push(
            ComplexProjectE2eStep::new(
                "runtime-package-load",
                ComplexProjectE2eStatus::Passed,
                format!(
                    "loaded runtime package active scene {} with {} entities",
                    package.active_scene.id,
                    package.active_scene.entities.len()
                ),
            )
            .with_artifact_path(runtime_package_path.display().to_string()),
        );
    } else {
        report.steps.push(ComplexProjectE2eStep::new(
            "runtime-package-load",
            ComplexProjectE2eStatus::Failed,
            "runtime package could not be loaded after export",
        ));
        for issue in load.diagnostics.issues {
            report.diagnostics.push(
                ComplexProjectE2eDiagnostic::error("RuntimePackageLoadFailed", issue.message)
                    .with_path(issue.path),
            );
        }
    }

    let mut player_request =
        NativePlayerWindowRunRequest::headless_surface_gate(&runtime_package_path);
    player_request.frame_limit = request.frame_limit.max(1);
    let linked_modules = crate::complex_shooter_linked_project_runtimes();
    let player_report = run_headless_native_player_from_package_with_linked_modules(
        player_request,
        linked_modules.as_ref(),
    );
    report.metrics.frames_run = player_report.frames_completed;
    report.metrics.draw_item_count = player_report.rhi_command_count;
    report.metrics.present_count = player_report.surface.presented_frame_count;
    report.metrics.aui_draw_item_count = player_report.aui.draw_item_count;
    report.metrics.aui_text_command_count = player_report.aui.text_command_count;
    report.metrics.aui_ui_pass_inserted = player_report.aui.ui_pass_inserted;
    report.metrics.aui_composition_stage_count = player_report.aui.ui_composition_stage_count;
    report.metrics.aui_before_world_item_count = player_report.aui.ui_before_world_item_count;
    report.metrics.aui_screen_overlay_item_count = player_report.aui.ui_screen_overlay_item_count;
    report.metrics.aui_modal_item_count = player_report.aui.ui_modal_item_count;
    report.metrics.aui_before_world_pass_present = player_report.aui.ui_before_world_pass_present;
    report.metrics.aui_screen_overlay_pass_present =
        player_report.aui.ui_screen_overlay_pass_present;
    report.metrics.aui_modal_pass_present = player_report.aui.ui_modal_pass_present;
    report.metrics.aui_before_world_skipped = player_report.aui.ui_before_world_skipped;
    report.metrics.aui_screen_overlay_skipped = player_report.aui.ui_screen_overlay_skipped;
    report.metrics.aui_modal_skipped = player_report.aui.ui_modal_skipped;
    report.metrics.aui_modal_rendering_only = player_report.aui.modal_rendering_only;
    report.metrics.aui_glyph_present = player_report.aui.glyph_present;
    report.metrics.aui_font_atlas_present = player_report.aui.font_atlas_present;
    report.metrics.aui_font_atlas_id = player_report.aui.font_atlas_id.clone();
    report.metrics.aui_font_source_kind = player_report.aui.font_source_kind.clone();
    report.metrics.aui_font_asset_id = player_report.aui.font_asset_id.clone();
    report.metrics.aui_font_asset_status = player_report.aui.font_asset_status.clone();
    report.metrics.aui_font_fallback_used = player_report.aui.font_fallback_used;
    report.metrics.aui_requested_glyph_count = player_report.aui.requested_glyph_count;
    report.metrics.aui_rendered_glyph_count = player_report.aui.rendered_glyph_count;
    report.metrics.aui_unsupported_glyph_count = player_report.aui.unsupported_glyph_count;
    report.metrics.aui_clipped_glyph_count = player_report.aui.clipped_glyph_count;
    report.metrics.aui_glyph_plan_hash = player_report.aui.glyph_plan_hash.clone();
    report.metrics.aui_snapshot_source = player_report.aui.snapshot_source.clone();
    report.metrics.aui_producer_id = player_report.aui.producer_id.clone();
    report.metrics.aui_snapshot_value_count = player_report.aui.snapshot_value_count;
    report.metrics.aui_produced_path_count = player_report.aui.produced_paths.len();
    report.metrics.aui_declared_binding_path_count = player_report.aui.declared_binding_paths.len();
    report.metrics.aui_missing_path_count = player_report.aui.missing_paths.len();
    report.metrics.aui_type_mismatch_path_count = player_report.aui.type_mismatch_paths.len();
    report.metrics.aui_status = player_report.aui.status.clone();
    report.metrics.aui_next_actions = player_report.aui.next_actions.clone();
    report.steps.push(ComplexProjectE2eStep::new(
        "headless-player-run",
        if player_report.exit_code == 0 {
            ComplexProjectE2eStatus::Passed
        } else {
            ComplexProjectE2eStatus::Failed
        },
        format!(
            "headless player frames={}, present={}, rhiCommands={}",
            player_report.frames_completed,
            player_report.surface.presented_frame_count,
            player_report.rhi_command_count
        ),
    ));
    for diagnostic in &player_report.diagnostics {
        report.diagnostics.push(ComplexProjectE2eDiagnostic {
            severity: format!("{:?}", diagnostic.severity).to_ascii_lowercase(),
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            path: diagnostic.path.clone(),
        });
    }
    report.steps.push(ComplexProjectE2eStep::new(
        "aui-runtime-present",
        if player_report.aui.ui_pass_inserted && player_report.aui.draw_item_count > 0 {
            ComplexProjectE2eStatus::Passed
        } else {
            ComplexProjectE2eStatus::Failed
        },
        format!(
            "aui status={}, snapshotSource={}, producerId={:?}, producedPaths={}, missingPaths={}, packageDocuments={}, loadedDocuments={}, drawItems={}, textCommands={}, compositionStages={}, beforeWorldPass={}, screenOverlayPass={}, modalPass={}, glyphPresent={}, fontAtlas={:?}, renderedGlyphs={}, glyphPlanHash={:?}",
            player_report.aui.status,
            player_report.aui.snapshot_source,
            player_report.aui.producer_id,
            player_report.aui.produced_paths.len(),
            player_report.aui.missing_paths.len(),
            player_report.aui.package_document_count,
            player_report.aui.loaded_document_count,
            player_report.aui.draw_item_count,
            player_report.aui.text_command_count,
            player_report.aui.ui_composition_stage_count,
            player_report.aui.ui_before_world_pass_present,
            player_report.aui.ui_screen_overlay_pass_present,
            player_report.aui.ui_modal_pass_present,
            player_report.aui.glyph_present,
            player_report.aui.font_atlas_id,
            player_report.aui.rendered_glyph_count,
            player_report.aui.glyph_plan_hash
        ),
    ));
    if !player_report.aui.glyph_present && player_report.aui.text_command_count > 0 {
        report.gaps.push(ComplexProjectE2eGap::new(
            "runtime-text-glyph-present",
            "warning",
            "AUI text commands reach the UI pass, but real glyph presentation is still not proven.",
            "runtime_text_glyph_present",
        ));
    }
    for next_action in &player_report.aui.next_actions {
        if next_action != "runtime_text_glyph_present" {
            report.gaps.push(ComplexProjectE2eGap::new(
                format!("aui-next-action-{}", sanitize_id(next_action)),
                "warning",
                "AUI present report requested a next action.",
                next_action.clone(),
            ));
        }
    }

    if request.include_optional_real_window_step {
        report.steps.push(ComplexProjectE2eStep::new(
            "optional-real-window-smoke",
            ComplexProjectE2eStatus::Skipped,
            "real OS window smoke is optional and requires a local real-window feature/GPU environment",
        ));
        report.gaps.push(ComplexProjectE2eGap::new(
            "optional-real-window-not-default",
            "info",
            "E2E gate validates the exported project through headless surface present by default.",
            "Run runtime_player_winit real-window smoke locally when checking real OS window integration.",
        ));
    }

    if request.project_path.join("project.afengine.json").exists() {
        report.gaps.push(ComplexProjectE2eGap::new(
            "legacy-manifest-name-present",
            "warning",
            "project.afengine.json exists but current editor/export pipeline reads project.aife.json.",
            "Keep project.aife.json as the active manifest until a formal manifest rename migration is approved.",
        ));
    }

    finalize_report(&mut report);
    report
}

fn finalize_report(report: &mut ComplexProjectE2eGateReport) {
    report.recompute_status();
    let report_path = PathBuf::from(&report.build_output_path)
        .join("reports")
        .join("complex-project-e2e-gate-report.json");
    if let Err(error) = write_json(&report_path, report) {
        report.diagnostics.push(
            ComplexProjectE2eDiagnostic::error(
                "GateReportWriteFailed",
                format!("failed to write gate report: {error}"),
            )
            .with_path(report_path.display().to_string()),
        );
        report.recompute_status();
    } else {
        report.artifacts.push(ComplexProjectE2eArtifact {
            artifact_id: "complex-project-e2e-gate-report".to_string(),
            path: report_path.display().to_string(),
        });
    }
}

fn merge_assembly_report(
    report: &mut ComplexProjectE2eGateReport,
    assembly_report: &ComplexShooterProjectAssemblyReport,
) {
    report.steps.push(ComplexProjectE2eStep::new(
        "complex-shooter-project-assembly",
        ComplexProjectE2eStatus::from(assembly_report.status),
        format!(
            "assembly status={:?}, domains passed={}/{}",
            assembly_report.status,
            assembly_report.metrics.passed_domain_count,
            assembly_report.metrics.domain_count
        ),
    ));
    for domain in &assembly_report.domains {
        report.steps.push(ComplexProjectE2eStep::new(
            format!("assembly-domain-{}", domain.domain_id),
            ComplexProjectE2eStatus::from(domain.status),
            domain.summary.clone(),
        ));
    }
    for diagnostic in &assembly_report.diagnostics {
        report.diagnostics.push(diagnostic.into());
    }
    for next_action in &assembly_report.next_actions {
        report.gaps.push(ComplexProjectE2eGap::new(
            format!("assembly-next-action-{}", sanitize_id(next_action)),
            if assembly_report.status == AssemblyDomainStatus::Failed {
                "error"
            } else {
                "warning"
            },
            "Assembly validator reported a required next action.",
            next_action.clone(),
        ));
    }
}

fn sanitize_id(value: &str) -> String {
    let id = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if id.is_empty() {
        "next-action".to_string()
    } else {
        id
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
