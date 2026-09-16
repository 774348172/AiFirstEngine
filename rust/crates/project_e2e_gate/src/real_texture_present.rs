use editor_core::{
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus,
};
use engine_runtime::render_command::apply_batch;
use engine_runtime::render_extract::RenderExtractContext;
use engine_runtime::render_resource::{RenderResourceHandle, RenderResourceKind};
use engine_runtime::render_state::RenderSceneState;
use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawKind, RhiDrawPayload};
use engine_runtime::runtime_package::{load_runtime_package, RuntimeAssetRef};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use engine_runtime::runtime_renderer::{
    QualityProfile, RenderTarget, RuntimeRenderer, RuntimeRendererInput,
};
use engine_runtime::runtime_scene_hydration::hydrate_active_scene_into_world;
use engine_runtime::runtime_texture::load_runtime_texture_payload;
use engine_runtime::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-real-texture-present-report.v1";
pub const COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_SCENARIO_ID: &str =
    "complex-shooter-real-texture-present-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterRealTexturePresentStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterRealTexturePresentMetrics {
    pub assembled_texture_payload_count: usize,
    pub package_texture_asset_count: usize,
    pub loaded_texture_payload_count: usize,
    pub loaded_texture_byte_count: usize,
    pub render_proxy_count: usize,
    pub sprite_draw_command_count: usize,
    pub non_fallback_sprite_draw_count: usize,
    pub rhi_textured_sprite_command_count: usize,
    pub rhi_non_fallback_texture_command_count: usize,
    pub sprite_texture_binding_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterRealTexturePresentReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterRealTexturePresentStatus,
    pub project_root: String,
    pub output_root: String,
    pub runtime_package_dir: Option<String>,
    pub metrics: ComplexShooterRealTexturePresentMetrics,
    pub diagnostics: Vec<String>,
    pub artifacts: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterRealTexturePresentRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterRealTexturePresentRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_real_texture_present_report(
    request: ComplexShooterRealTexturePresentRequest,
) -> ComplexShooterRealTexturePresentReport {
    let mut report = ComplexShooterRealTexturePresentReport {
        schema_version: COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_REAL_TEXTURE_PRESENT_SCENARIO_ID.to_string(),
        status: ComplexShooterRealTexturePresentStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        runtime_package_dir: None,
        metrics: ComplexShooterRealTexturePresentMetrics::default(),
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
        next_actions: Vec::new(),
    };

    if let Err(error) = fs::create_dir_all(&request.output_root) {
        report
            .diagnostics
            .push(format!("fail:output_root_create_failed:{error}"));
        return report;
    }

    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&request.project_root),
    );
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        report
            .diagnostics
            .push("fail:project_runtime_package_assembly_failed".to_string());
        for diagnostic in assembly.report.diagnostics {
            report.diagnostics.push(format!(
                "assembly:{:?}:{}:{}",
                diagnostic.domain, diagnostic.code, diagnostic.message
            ));
        }
        finalize_report(&request.output_root, report)
    } else {
        let input = assembly
            .build_input
            .expect("successful assembly should carry build input");
        report.metrics.assembled_texture_payload_count = input.texture_payloads.len();
        let active_scene_id = assembly
            .active_scene_id
            .clone()
            .unwrap_or_else(|| "scene-main".to_string());
        let package_dir = request.output_root.join("runtime_package");
        let build_request = RuntimePackageBuildRequest::dev_desktop(&package_dir, active_scene_id);
        let build_report = RuntimePackageBuilder::build(&build_request, &input);
        report.runtime_package_dir = Some(package_dir.display().to_string());
        report.artifacts.push(package_dir.display().to_string());
        if build_report.status != RuntimePackageBuildStatus::Success {
            report
                .diagnostics
                .push("fail:runtime_package_build_failed".to_string());
            for diagnostic in build_report.diagnostics {
                report
                    .diagnostics
                    .push(format!("build:{}:{}", diagnostic.code, diagnostic.message));
            }
            return finalize_report(&request.output_root, report);
        }

        let package_load = load_runtime_package(&package_dir);
        let Some(package) = package_load.value else {
            report
                .diagnostics
                .push("fail:runtime_package_load_failed".to_string());
            for issue in package_load.diagnostics.issues {
                report
                    .diagnostics
                    .push(format!("package:{}:{}", issue.path, issue.message));
            }
            return finalize_report(&request.output_root, report);
        };

        let texture_records = package
            .assets
            .runtime_asset_index
            .iter()
            .filter(|record| record.asset_type == "texture" || record.loader_kind == "texture")
            .collect::<Vec<_>>();
        report.metrics.package_texture_asset_count = texture_records.len();
        let mut bindings = Sprite2DTextureBindingContext::new();
        for (index, record) in texture_records.iter().enumerate() {
            match load_runtime_texture_payload(
                &package_dir,
                &package.runtime_asset_index,
                &RuntimeAssetRef {
                    id: record.asset_id.clone(),
                    asset_type: "texture".to_string(),
                    guid: None,
                    sub_asset: None,
                },
            ) {
                Ok(payload) => {
                    report.metrics.loaded_texture_payload_count += 1;
                    report.metrics.loaded_texture_byte_count += payload.rgba8.len();
                    bindings.insert_texture_handle(
                        payload.asset_id,
                        RenderResourceHandle {
                            kind: RenderResourceKind::Texture,
                            index: index as u64 + 1,
                            generation: 1,
                        },
                        payload.sampler,
                    );
                }
                Err(error) => report.diagnostics.push(format!(
                    "fail:texture_payload_load_failed:{:?}:{}",
                    error.code, error.message
                )),
            }
        }

        let world_result = hydrate_active_scene_into_world(&package);
        let Some((mut world, _hydration_report)) = world_result.value else {
            report
                .diagnostics
                .push("fail:runtime_scene_hydration_failed".to_string());
            for issue in world_result.diagnostics.issues {
                report
                    .diagnostics
                    .push(format!("hydrate:{}:{}", issue.path, issue.message));
            }
            return finalize_report(&request.output_root, report);
        };

        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let mut queue = extract.extract_world_dirty(1, &mut world, &render_scene);
        let commands = queue.normalize_merge(&render_scene);
        let apply_diagnostics = apply_batch(&mut render_scene, &commands);
        if !apply_diagnostics.is_empty() {
            report
                .diagnostics
                .push("fail:render_command_apply_failed".to_string());
        }
        report.metrics.render_proxy_count = render_scene.proxies_len();

        let renderer_output = RuntimeRenderer::new().build(RuntimeRendererInput {
            frame_index: 1,
            render_scene_state: &render_scene,
            render_view_state: None,
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: Some(&bindings),
            runtime_texture_bindings: None,
            game_view_presentation: None,
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::headless_texture("texture-present-gate", 640, 360),
        });

        report.metrics.sprite_texture_binding_ready = renderer_output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_texture_binding_ready");
        for command in &renderer_output.rhi_command_plan.commands {
            let RhiCommand::Draw {
                draw_kind, payload, ..
            } = command
            else {
                continue;
            };
            if *draw_kind != RhiDrawKind::SpriteTextured {
                continue;
            }
            report.metrics.rhi_textured_sprite_command_count += 1;
            if matches!(
                payload,
                RhiDrawPayload::SpriteTextured {
                    texture: Some(_),
                    fallback_used: false,
                    ..
                }
            ) {
                report.metrics.rhi_non_fallback_texture_command_count += 1;
            }
        }
        for pass in &renderer_output.render_graph.passes {
            for command in &pass.commands {
                if let engine_runtime::render_graph::RenderPassCommand::DrawSpriteTextured {
                    fallback_used,
                    ..
                } = command
                {
                    report.metrics.sprite_draw_command_count += 1;
                    if !fallback_used {
                        report.metrics.non_fallback_sprite_draw_count += 1;
                    }
                }
            }
        }

        if report.metrics.loaded_texture_payload_count == 0 {
            report
                .diagnostics
                .push("fail:no_runtime_texture_payload_loaded".to_string());
        }
        if report.metrics.non_fallback_sprite_draw_count == 0 {
            report
                .diagnostics
                .push("fail:no_non_fallback_sprite_draw".to_string());
        }
        if report.metrics.rhi_non_fallback_texture_command_count == 0 {
            report
                .diagnostics
                .push("fail:no_rhi_non_fallback_texture_command".to_string());
        }
        if !report.metrics.sprite_texture_binding_ready {
            report
                .diagnostics
                .push("fail:sprite_texture_binding_ready_missing".to_string());
        }

        finalize_report(&request.output_root, report)
    }
}

fn finalize_report(
    output_root: &Path,
    mut report: ComplexShooterRealTexturePresentReport,
) -> ComplexShooterRealTexturePresentReport {
    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("fail:"))
    {
        report
            .next_actions
            .push("inspect_complex_shooter_real_texture_present_report".to_string());
        ComplexShooterRealTexturePresentStatus::Failed
    } else {
        ComplexShooterRealTexturePresentStatus::Passed
    };

    let report_path = output_root
        .join("reports")
        .join("complex-shooter-real-texture-present-report.json");
    report.artifacts.push(report_path.display().to_string());
    if let Err(error) = write_json(&report_path, &report) {
        report
            .diagnostics
            .push(format!("fail:report_write_failed:{error}"));
        report.status = ComplexShooterRealTexturePresentStatus::Failed;
    }
    report
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
