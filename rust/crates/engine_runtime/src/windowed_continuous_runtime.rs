use serde::{Deserialize, Serialize};

use crate::components::{Hierarchy, Renderable, Transform};
use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use crate::ids::EntityId;
use crate::input_action::{ActionPhase, ActionSnapshot, InputActionState, InputTraceSummary};
use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
use crate::project_logic::{ProjectLogicRunner, RuleCall, RuleExecutionPlan};
use crate::render_state::{RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState};
use crate::world::World;

pub const MOVE_RIGHT_ACTION: &str = "action.move_right";
pub const MOVE_RIGHT_RULE: &str = "project.windowed_continuous.move_right";
pub const DEFAULT_MOVING_ENTITY_ID: &str = "entity-player";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousRuntimeRequest {
    pub frame_count: u64,
    pub fixed_dt: f32,
    pub input_script: Vec<WindowedContinuousInputFrame>,
    pub scene_id: String,
    pub viewport: WindowedContinuousViewport,
    pub backend_kind: WindowedContinuousBackendKind,
}

impl Default for WindowedContinuousRuntimeRequest {
    fn default() -> Self {
        Self {
            frame_count: 5,
            fixed_dt: 1.0 / 60.0,
            input_script: vec![
                WindowedContinuousInputFrame::move_right(1),
                WindowedContinuousInputFrame::move_right(2),
                WindowedContinuousInputFrame::move_right(3),
            ],
            scene_id: "scene-main".to_string(),
            viewport: WindowedContinuousViewport::default(),
            backend_kind: WindowedContinuousBackendKind::HeadlessSurface,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousInputFrame {
    pub frame_index: u64,
    pub actions: Vec<WindowedContinuousInputAction>,
}

impl WindowedContinuousInputFrame {
    pub fn move_right(frame_index: u64) -> Self {
        Self {
            frame_index,
            actions: vec![WindowedContinuousInputAction::MoveRight],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowedContinuousInputAction {
    MoveRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousViewport {
    pub viewport_id: String,
    pub target_id: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowedContinuousViewport {
    fn default() -> Self {
        Self {
            viewport_id: "game-view".to_string(),
            target_id: "main-surface".to_string(),
            width: 1280,
            height: 720,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowedContinuousBackendKind {
    HeadlessSurface,
    RealWindowSmoke,
}

impl WindowedContinuousBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HeadlessSurface => "headless-surface",
            Self::RealWindowSmoke => "real-window-smoke",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostic {
    pub system: String,
    pub stage: String,
    pub severity: RuntimeDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub frame_index: Option<u64>,
    pub entity_id: Option<String>,
    pub asset_id: Option<String>,
    pub command_id: Option<String>,
}

impl RuntimeDiagnostic {
    fn error(
        system: impl Into<String>,
        stage: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        frame_index: Option<u64>,
        entity_id: Option<String>,
    ) -> Self {
        Self {
            system: system.into(),
            stage: stage.into(),
            severity: RuntimeDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            frame_index,
            entity_id,
            asset_id: None,
            command_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousRuntimeReport {
    pub schema_version: String,
    pub ok: bool,
    pub frame_count: u64,
    pub backend_kind: String,
    pub frames: Vec<WindowedContinuousFrameReport>,
    pub final_ecs_position_x: f32,
    pub final_render_position_x: Option<f32>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousFrameReport {
    pub frame_index: u64,
    pub input: WindowedContinuousInputReport,
    pub runtime: WindowedContinuousStageReport,
    pub logic: WindowedContinuousLogicReport,
    pub ecs: WindowedContinuousEcsReport,
    pub render_extract: WindowedContinuousRenderExtractReport,
    pub render_thread: WindowedContinuousRenderThreadReport,
    pub renderer: WindowedContinuousRendererReport,
    pub present: WindowedContinuousPresentReport,
    pub frame_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousInputReport {
    pub action_count: usize,
    pub action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousStageReport {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousLogicReport {
    pub trace_event_count: usize,
    pub move_rule_applied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousEcsReport {
    pub entity_count: usize,
    pub moving_entity_position_x: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousRenderExtractReport {
    pub raw_command_count: usize,
    pub applied_command_count: usize,
    pub proxy_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousRenderThreadReport {
    pub status: String,
    pub report_schema: Option<String>,
    pub rdg_status: Option<String>,
    pub rhi_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousRendererReport {
    pub status: String,
    pub draw_item_count: Option<usize>,
    pub texture_lifetime_event_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedContinuousPresentReport {
    pub status: String,
    pub target_id: String,
    pub target_kind: Option<String>,
}

#[derive(Debug)]
pub struct WindowedContinuousRuntime {
    request: WindowedContinuousRuntimeRequest,
    host: EngineHostLoop,
    world: World,
}

impl WindowedContinuousRuntime {
    pub fn new(request: WindowedContinuousRuntimeRequest) -> Self {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(MOVE_RIGHT_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(MOVE_RIGHT_RULE, move_right_rule);

        let mut host = EngineHostLoop::with_project_logic(request.scene_id.clone(), runner);
        host.render_scene_mut().register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        ));

        Self {
            request,
            host,
            world: minimal_movable_world(),
        }
    }

    pub fn run(&mut self) -> WindowedContinuousRuntimeReport {
        let mut frames = Vec::new();
        let mut diagnostics = Vec::new();

        for frame_index in 1..=self.request.frame_count {
            let snapshot = self.action_snapshot_for_frame(frame_index);
            let input_summary = InputTraceSummary::from_snapshot(Some(&snapshot)).with_route(
                Some(self.request.viewport.viewport_id.clone()),
                Some("Game".to_string()),
                Some("RuntimeInputFrame".to_string()),
                Some("windowed_continuous_runtime_script".to_string()),
            );
            let engine_output = self.host.tick(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_action_snapshot(snapshot.clone())
                    .with_input_trace_summary(input_summary)
                    .with_unscaled_delta_time(self.request.fixed_dt),
                &mut self.world,
            );

            let ecs_position_x = self.moving_entity_position_x(&mut diagnostics, frame_index);
            let render_position_x = self.render_position_x();
            let render_thread_frame = engine_output.render_thread_frame.as_ref();
            let render_frame_report = engine_output.render_frame_report.as_ref();

            if (frame_index == self.request.frame_count)
                && render_position_x.is_some_and(|x| (x - ecs_position_x).abs() > f32::EPSILON)
            {
                diagnostics.push(RuntimeDiagnostic::error(
                    "engine.windowed_continuous_runtime",
                    "Verify",
                    "ecs_render_position_mismatch",
                    "ECS final Transform differs from RenderSceneState final Transform",
                    Some(frame_index),
                    Some(DEFAULT_MOVING_ENTITY_ID.to_string()),
                ));
            }

            frames.push(WindowedContinuousFrameReport {
                frame_index,
                input: WindowedContinuousInputReport {
                    action_count: snapshot.action_count(),
                    action_ids: snapshot.action_ids(),
                },
                runtime: WindowedContinuousStageReport {
                    status: if engine_output.runtime_advanced {
                        "advanced".to_string()
                    } else {
                        "skipped".to_string()
                    },
                },
                logic: WindowedContinuousLogicReport {
                    trace_event_count: engine_output.runtime_trace.events.len(),
                    move_rule_applied: engine_output.runtime_trace.events.iter().any(|event| {
                        event.system_id == format!("project.rule.{MOVE_RIGHT_RULE}")
                            && event.message.contains("applied")
                    }),
                },
                ecs: WindowedContinuousEcsReport {
                    entity_count: self.world.entity_count(),
                    moving_entity_position_x: ecs_position_x,
                },
                render_extract: WindowedContinuousRenderExtractReport {
                    raw_command_count: render_frame_report
                        .map(|report| report.counters.raw_command_count)
                        .unwrap_or_default(),
                    applied_command_count: render_frame_report
                        .map(|report| report.counters.applied_command_count)
                        .unwrap_or_default(),
                    proxy_count: self.host.render_scene().proxies_len(),
                },
                render_thread: WindowedContinuousRenderThreadReport {
                    status: if render_thread_frame.is_some() {
                        "rendered".to_string()
                    } else {
                        "not_rendered".to_string()
                    },
                    report_schema: render_thread_frame
                        .map(|frame| frame.report.schema_version.clone()),
                    rdg_status: render_thread_frame.map(|frame| frame.report.rdg_status.clone()),
                    rhi_status: render_thread_frame.map(|frame| frame.report.rhi_status.clone()),
                },
                renderer: WindowedContinuousRendererReport {
                    status: if engine_output.render_built {
                        "built".to_string()
                    } else {
                        "not_built".to_string()
                    },
                    draw_item_count: render_thread_frame
                        .map(|frame| frame.report.render_frame_report.draw_item_count),
                    texture_lifetime_event_count: render_thread_frame
                        .map(|frame| frame.report.texture_lifetime_report.events.len()),
                },
                present: WindowedContinuousPresentReport {
                    status: render_thread_frame
                        .map(|frame| frame.report.present_status.clone())
                        .unwrap_or_else(|| "not_submitted".to_string()),
                    target_id: render_thread_frame
                        .map(|frame| frame.report.target_id.clone())
                        .unwrap_or_else(|| self.request.viewport.target_id.clone()),
                    target_kind: render_thread_frame.map(|frame| frame.report.target_kind.clone()),
                },
                frame_hash: engine_output.frame_hash,
            });
        }

        let final_ecs_position_x = self.moving_entity_position_x(&mut diagnostics, None);
        let final_render_position_x = self.render_position_x();
        let ok = diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != RuntimeDiagnosticSeverity::Error)
            && frames.len() == self.request.frame_count as usize
            && final_render_position_x
                .is_some_and(|x| (x - final_ecs_position_x).abs() <= f32::EPSILON);

        WindowedContinuousRuntimeReport {
            schema_version: "windowed-continuous-runtime-report.v1".to_string(),
            ok,
            frame_count: self.request.frame_count,
            backend_kind: self.request.backend_kind.as_str().to_string(),
            frames,
            final_ecs_position_x,
            final_render_position_x,
            diagnostics,
        }
    }

    fn action_snapshot_for_frame(&self, frame_index: u64) -> ActionSnapshot {
        let actions = self
            .request
            .input_script
            .iter()
            .filter(|input_frame| input_frame.frame_index == frame_index)
            .flat_map(|input_frame| input_frame.actions.iter())
            .map(|action| match action {
                WindowedContinuousInputAction::MoveRight => {
                    InputActionState::button(MOVE_RIGHT_ACTION, ActionPhase::Pressed)
                }
            })
            .collect();
        ActionSnapshot::with_actions(frame_index, actions)
    }

    fn moving_entity_position_x(
        &self,
        diagnostics: &mut Vec<RuntimeDiagnostic>,
        frame_index: impl Into<Option<u64>>,
    ) -> f32 {
        let frame_index = frame_index.into();
        self.world
            .transform(&EntityId::from(DEFAULT_MOVING_ENTITY_ID))
            .map(|transform| transform.local_position.x)
            .unwrap_or_else(|| {
                diagnostics.push(RuntimeDiagnostic::error(
                    "engine.windowed_continuous_runtime",
                    "ECS",
                    "missing_moving_entity_transform",
                    "moving entity Transform is missing",
                    frame_index,
                    Some(DEFAULT_MOVING_ENTITY_ID.to_string()),
                ));
                0.0
            })
    }

    fn render_position_x(&self) -> Option<f32> {
        let source = EntityId::from(DEFAULT_MOVING_ENTITY_ID);
        self.host
            .render_scene()
            .proxy_for_source(&source)
            .and_then(|proxy_id| self.host.render_scene().proxy(proxy_id))
            .map(|proxy| proxy.common.transform.local_position.x)
    }
}

pub fn run_headless_windowed_continuous_runtime(
    request: WindowedContinuousRuntimeRequest,
) -> WindowedContinuousRuntimeReport {
    WindowedContinuousRuntime::new(request).run()
}

fn minimal_movable_world() -> World {
    let mut world = World::new();
    world.spawn_with_components(
        EntityId::from(DEFAULT_MOVING_ENTITY_ID),
        "Player",
        "actor",
        true,
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        },
        Some(Transform::identity()),
        Some(Renderable {
            mesh_ref: Some("mesh-player".to_string()),
            material_ref: Some("material-player".to_string()),
            visible: true,
            layer: "default".to_string(),
        }),
    );
    world.take_dirty_records();
    world
}

fn move_right_rule(context: &mut LogicContext<'_>) -> LogicResult {
    if !context.action_pressed(MOVE_RIGHT_ACTION) {
        return LogicResult::skipped(MOVE_RIGHT_RULE, ExecutorKind::RustAot);
    }
    let entity_id = EntityId::from(DEFAULT_MOVING_ENTITY_ID);
    let mut position = context
        .read_transform_local_position(&entity_id)
        .expect("moving entity Transform should exist");
    position.x += 1.0;
    let write = context
        .write_transform_local_position(entity_id, position)
        .expect("write should succeed");
    let mut result = LogicResult::applied(MOVE_RIGHT_RULE, ExecutorKind::RustAot);
    result.writes.push(write);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windowed_continuous_runtime_report_is_json_serializable() {
        let report =
            run_headless_windowed_continuous_runtime(WindowedContinuousRuntimeRequest::default());
        let json = serde_json::to_string(&report).expect("report should serialize");

        assert_eq!(
            report.schema_version,
            "windowed-continuous-runtime-report.v1"
        );
        assert!(json.contains("windowed-continuous-runtime-report.v1"));
        assert!(report.ok);
    }

    #[test]
    fn headless_windowed_continuous_runtime_runs_n_frames() {
        let report =
            run_headless_windowed_continuous_runtime(WindowedContinuousRuntimeRequest::default());

        assert!(report.ok);
        assert_eq!(report.frame_count, 5);
        assert_eq!(report.frames.len(), 5);
        assert!(report
            .frames
            .iter()
            .all(|frame| frame.runtime.status == "advanced"));
        assert!(report
            .frames
            .iter()
            .all(|frame| frame.render_thread.status == "rendered"));
    }

    #[test]
    fn input_script_drives_transform_and_render_scene_state() {
        let report =
            run_headless_windowed_continuous_runtime(WindowedContinuousRuntimeRequest::default());

        assert_eq!(report.frames[0].ecs.moving_entity_position_x, 1.0);
        assert_eq!(report.frames[1].ecs.moving_entity_position_x, 2.0);
        assert_eq!(report.frames[2].ecs.moving_entity_position_x, 3.0);
        assert_eq!(report.frames[3].ecs.moving_entity_position_x, 3.0);
        assert_eq!(report.final_ecs_position_x, 3.0);
        assert_eq!(report.final_render_position_x, Some(3.0));
    }

    #[test]
    fn first_frame_creates_proxy_and_later_frames_update_transform() {
        let report =
            run_headless_windowed_continuous_runtime(WindowedContinuousRuntimeRequest::default());

        assert_eq!(report.frames[0].render_extract.raw_command_count, 1);
        assert_eq!(report.frames[0].render_extract.applied_command_count, 1);
        assert_eq!(report.frames[0].render_extract.proxy_count, 1);
        assert_eq!(report.frames[1].render_extract.raw_command_count, 1);
        assert_eq!(report.frames[2].render_extract.raw_command_count, 1);
        assert_eq!(report.frames[3].render_extract.raw_command_count, 0);
    }

    #[test]
    fn report_exposes_input_logic_render_and_present_fields() {
        let report =
            run_headless_windowed_continuous_runtime(WindowedContinuousRuntimeRequest::default());
        let first = &report.frames[0];

        assert_eq!(first.input.action_ids, vec![MOVE_RIGHT_ACTION]);
        assert!(first.logic.move_rule_applied);
        assert_eq!(
            first.render_thread.report_schema.as_deref(),
            Some("render-thread-report.v1")
        );
        assert_eq!(first.render_thread.rdg_status.as_deref(), Some("ok"));
        assert_eq!(first.render_thread.rhi_status.as_deref(), Some("ok"));
        assert_eq!(first.present.status, "presented");
        assert!(first.renderer.draw_item_count.is_some());
        assert!(first.frame_hash.is_some());
    }
}
