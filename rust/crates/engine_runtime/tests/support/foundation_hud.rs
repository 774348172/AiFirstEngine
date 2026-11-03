//! BM01 package/Host/HUD consumer. Keyframes are not a full K1 timing run.
use engine_runtime::{
    aui::*, engine_host_loop::*, frame_loop::RuntimeFrameContext, project_runtime_session::*,
    runtime_instance_loader::RuntimeInstanceLoader, scene_loader::load_scene_into_world,
    wgpu_backend::real::RealWgpuBackend,
};
use project_runtime_sdk::{ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D};
use std::sync::{Arc, Mutex};

struct Step {
    selected: Vec<usize>,
    round: usize,
    camera: ProjectRuntimeTransform3D,
}
impl Default for Step {
    fn default() -> Self {
        Self {
            selected: Vec::new(),
            round: 0,
            camera: ProjectRuntimeTransform3D::identity(),
        }
    }
}
struct Session(Arc<Mutex<Step>>);
impl ProjectRuntimeSession for Session {
    fn session_id(&self) -> &str {
        "foundation-hud"
    }
    fn handle_aui_actions(
        &mut self,
        _: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }
    fn fixed_update(
        &mut self,
        ctx: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let step = self.0.lock().unwrap();
        let mut batch = ProjectRuntimeMutationBuffer::new();
        for i in &step.selected {
            let id: engine_runtime::ids::SourceEntityId = format!("node-{i}").into();
            batch.spatial_command(
                id.clone(),
                ctx.world.entity_generation(&id).unwrap(),
                Intent::SetLocalTransform {
                    transform: ProjectRuntimeTransform3D {
                        translation: [step.round as f32 + 1., *i as f32, -4.],
                        ..ProjectRuntimeTransform3D::identity()
                    },
                },
            );
        }
        let id: engine_runtime::ids::SourceEntityId = "camera".into();
        batch.spatial_command(
            id.clone(),
            ctx.world.entity_generation(&id).unwrap(),
            Intent::SetLocalTransform {
                transform: step.camera.clone(),
            },
        );
        ProjectRuntimeSessionOutput::applied(batch)
    }
}

#[test]
#[ignore = "requires real GPU; exact-count consumer, not a full K0/K1 qualification"]
fn bm01_package_host_ten_thousand_spatial_entities_and_twenty_hud_nodes_reach_gpu() {
    let fixture = super::streamed_cells::build_pure_spatial_scene(10000);
    assert_eq!(fixture.package.active_scene.entities.len(), 10001);
    let mut world = load_scene_into_world(&fixture.package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let document = fixture.package.aui_documents.get("hud").unwrap();
    let step = Arc::new(Mutex::new(Step::default()));
    let mut host = EngineHostLoop::with_project_runtime_session(
        "main",
        Default::default(),
        Box::new(Session(step.clone())),
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    host.set_game_view_target(engine_runtime::game_view_presentation::GameViewTargetSpec {
        extent: engine_runtime::game_view_presentation::GameViewExtent::new(256, 256),
        scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
    });
    let mut backend = RealWgpuBackend::new_offscreen(256, 256).unwrap();
    let mut ui = AuiC2Runtime::default();
    let catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../samples/platform_3d_benchmark_project/Benchmark/target-recipes.json"
    ))
    .unwrap();
    // Repeatable non-prefix selection; this proves dirty cardinalities, not the
    // catalog's complete simulation/seed schedule or its 1920x1080 performance.
    let mut order: Vec<usize> = (0..10000).collect();
    let mut rng = catalog["baseSeed"].as_u64().unwrap() as u32;
    for i in (1..order.len()).rev() {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        order.swap(i, rng as usize % (i + 1));
    }
    let keys = catalog["tracks"]["K1"]["keys"].as_array().unwrap();
    let mut rows = Vec::new();
    let mut reference = None;
    let mut round = 0usize;
    for key in keys {
        let camera = ProjectRuntimeTransform3D {
            translation: std::array::from_fn(|i| key["position"][i].as_f64().unwrap() as f32),
            rotation_xyzw: std::array::from_fn(|i| key["rotationXyzw"][i].as_f64().unwrap() as f32),
            ..ProjectRuntimeTransform3D::identity()
        };
        for count in [0, 100, 1000, 10000] {
            *step.lock().unwrap() = Step {
                selected: order[..count].to_vec(),
                round,
                camera: camera.clone(),
            };
            let hud =
                AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
                    document,
                    ProjectUiStateSnapshotOutput::new(
                        "bm01",
                        AuiSnapshotSource::TestSnapshot,
                        ProjectUiStateSnapshot::new(round as u64),
                    ),
                    &fixture.package.font_atlases,
                    &fixture.package.font_bundles,
                    None,
                    &mut ui,
                );
            assert_eq!(hud.report.status, AuiRuntimePresentStatus::Success);
            assert_eq!(
                hud.overlay
                    .draw_items
                    .iter()
                    .filter(|i| i.node_id.starts_with("hud-"))
                    .count(),
                20
            );
            let output = host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_fixed_step_count(1)
                    .with_aui_overlay(hud.overlay)
                    .with_aui_composition(hud.composition),
                &mut world,
                RuntimeFrameContext {
                    package: &fixture.package,
                    instance_loader: &mut loader,
                    cell_resources: None,
                    cell_backend: Some(&mut backend),
                },
            );
            assert!(
                !output
                    .project_runtime_session_report
                    .as_ref()
                    .unwrap()
                    .terminal_fault,
                "{output:?}"
            );
            let changes = world.spatial_delta();
            assert_eq!(
                changes
                    .changes
                    .iter()
                    .filter(|c| c.entity_id.as_str().starts_with("node-"))
                    .count(),
                count
            );
            for i in &order[..count] {
                assert_eq!(
                    world
                        .transform3d(&format!("node-{i}").into())
                        .unwrap()
                        .translation,
                    [round as f32 + 1., *i as f32, -4.]
                );
            }
            let frame = output.render_thread_frame.unwrap();
            let pixels = backend
                .render_plan_to_rgba_bytes(&frame.renderer_output.rhi_command_plan, 256, 256)
                .unwrap();
            for i in 0..20 {
                let x = 20 + i % 5 * 48;
                let y = 20 + i / 5 * 48;
                let pixel = &pixels[(y * 256 + x) * 4..][..4];
                let expected = if i % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                };
                assert_eq!(pixel, expected, "HUD node {i}, round {round}");
            }
            if let Some(reference) = &reference {
                assert_eq!(
                    &pixels, reference,
                    "hidden spatial motion/camera must not change screen HUD"
                );
            } else {
                reference = Some(pixels);
            }
            rows.push(
                serde_json::json!({"keyTick":key["tick"],"dirty":count,"entities":10000,"hud":20}),
            );
            round += 1;
        }
    }
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    if let Ok(path) = std::env::var("ENGINE_FOUNDATION_HUD_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({"recipe":"BM-01/foundation-v1","claim":"package-host-dirty-HUD-GPU keyframe correctness only","viewport":[256,256],"fullK1":false,"formalQualified":false,"rows":rows})).unwrap()).unwrap();
    }
}
