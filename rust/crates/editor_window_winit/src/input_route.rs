use crate::viewport::{ViewportHost, ViewportKind};
use editor_input::{EditorInputEvent, EditorInputRouter, PointerButton};
use editor_ui_renderer::{UiDrawList, UiRect};
use engine_runtime::input_action::{ActionSnapshot, InputTraceSummary, PointerPosition};
use engine_runtime::input_mapping::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportInputRouteKind {
    UiConsumed,
    SceneCameraCommand,
    EditorToolCommand,
    RuntimeInputFrame,
    Ignored,
}

impl ViewportInputRouteKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UiConsumed => "UiConsumed",
            Self::SceneCameraCommand => "SceneCameraCommand",
            Self::EditorToolCommand => "EditorToolCommand",
            Self::RuntimeInputFrame => "RuntimeInputFrame",
            Self::Ignored => "Ignored",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportInputRoute {
    pub route_kind: ViewportInputRouteKind,
    pub viewport_id: Option<String>,
    pub viewport_kind: Option<ViewportKind>,
    pub focused: bool,
    pub hovered: bool,
    pub input_event_kind: String,
    pub reason: String,
    pub runtime_input_frame: Option<RuntimeInputFrame>,
}

impl ViewportInputRoute {
    fn ignored(event: &EditorInputEvent, reason: impl Into<String>) -> Self {
        Self {
            route_kind: ViewportInputRouteKind::Ignored,
            viewport_id: None,
            viewport_kind: None,
            focused: false,
            hovered: false,
            input_event_kind: event.kind().to_string(),
            reason: reason.into(),
            runtime_input_frame: None,
        }
    }

    pub fn input_trace_summary(&self, snapshot: Option<&ActionSnapshot>) -> InputTraceSummary {
        InputTraceSummary::from_snapshot(snapshot).with_route(
            self.viewport_id.clone(),
            self.viewport_kind
                .as_ref()
                .map(|kind| kind.as_str().to_string()),
            Some(self.route_kind.as_str().to_string()),
            Some(self.reason.clone()),
        )
    }
}

pub struct ViewportInputGateway {
    frame_counter: u64,
    game_pointer_capture_revision: Option<u64>,
}

impl Default for ViewportInputGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportInputGateway {
    pub fn new() -> Self {
        Self {
            frame_counter: 0,
            game_pointer_capture_revision: None,
        }
    }

    pub fn route_editor_input(
        &mut self,
        event: EditorInputEvent,
        ui_hit: bool,
        viewport_host: &mut ViewportHost,
    ) -> ViewportInputRoute {
        self.frame_counter += 1;
        if matches!(event, EditorInputEvent::FocusLost) {
            self.game_pointer_capture_revision = None;
            return ViewportInputRoute::ignored(&event, "window_focus_lost");
        }
        if ui_hit {
            return ViewportInputRoute {
                route_kind: ViewportInputRouteKind::UiConsumed,
                viewport_id: None,
                viewport_kind: None,
                focused: false,
                hovered: false,
                input_event_kind: event.kind().to_string(),
                reason: "ui_hit".to_string(),
                runtime_input_frame: None,
            };
        }

        if event_position(&event).is_none() {
            if let Some(viewport) = viewport_host.game_viewport() {
                if viewport.focused {
                    let runtime_event = runtime_input_event_from_editor(&event);
                    let mut frame =
                        RuntimeInputFrame::new(self.frame_counter, viewport.viewport_id.clone());
                    frame.events.push(runtime_event);
                    return ViewportInputRoute {
                        route_kind: ViewportInputRouteKind::RuntimeInputFrame,
                        viewport_id: Some(viewport.viewport_id.clone()),
                        viewport_kind: Some(ViewportKind::Game),
                        focused: true,
                        hovered: false,
                        input_event_kind: event.kind().to_string(),
                        reason: "game_view_focused".to_string(),
                        runtime_input_frame: Some(frame),
                    };
                }
                return ViewportInputRoute {
                    route_kind: ViewportInputRouteKind::Ignored,
                    viewport_id: Some(viewport.viewport_id.clone()),
                    viewport_kind: Some(ViewportKind::Game),
                    focused: false,
                    hovered: false,
                    input_event_kind: event.kind().to_string(),
                    reason: "viewport_not_focused".to_string(),
                    runtime_input_frame: None,
                };
            }
            if let Some(viewport) = viewport_host.scene_viewport() {
                if viewport.focused {
                    return ViewportInputRoute {
                        route_kind: ViewportInputRouteKind::EditorToolCommand,
                        viewport_id: Some(viewport.viewport_id.clone()),
                        viewport_kind: Some(ViewportKind::Scene),
                        focused: true,
                        hovered: false,
                        input_event_kind: event.kind().to_string(),
                        reason: "scene_view_editor_control".to_string(),
                        runtime_input_frame: None,
                    };
                }
            }
            return ViewportInputRoute::ignored(&event, "no_focused_viewport");
        }

        let hovered_scene = viewport_host
            .scene_viewport()
            .is_some_and(|viewport| event_hits_rect(&event, viewport.rect));
        if hovered_scene {
            let focused = viewport_host
                .scene_viewport()
                .is_some_and(|viewport| viewport.focused);
            let viewport = viewport_host
                .scene_viewport()
                .expect("checked scene viewport");
            return ViewportInputRoute {
                route_kind: ViewportInputRouteKind::SceneCameraCommand,
                viewport_id: Some(viewport.viewport_id.clone()),
                viewport_kind: Some(ViewportKind::Scene),
                focused,
                hovered: true,
                input_event_kind: event.kind().to_string(),
                reason: "scene_view_editor_control".to_string(),
                runtime_input_frame: None,
            };
        }

        let hovered_game = viewport_host
            .game_viewport()
            .is_some_and(|viewport| event_hits_rect(&event, viewport.rect));
        if hovered_game {
            if matches!(event, EditorInputEvent::PointerDown { .. }) {
                let _ = viewport_host.focus_game(true);
            }
            let viewport = viewport_host
                .game_viewport()
                .expect("checked game viewport");
            if !viewport.focused {
                return ViewportInputRoute {
                    route_kind: ViewportInputRouteKind::Ignored,
                    viewport_id: Some(viewport.viewport_id.clone()),
                    viewport_kind: Some(ViewportKind::Game),
                    focused: false,
                    hovered: true,
                    input_event_kind: event.kind().to_string(),
                    reason: "viewport_not_focused".to_string(),
                    runtime_input_frame: None,
                };
            }
            let presentation_revision = viewport_host.game_presentation_revision();
            if matches!(event, EditorInputEvent::PointerUp { .. })
                && self.game_pointer_capture_revision.is_some()
                && self.game_pointer_capture_revision != presentation_revision
            {
                self.game_pointer_capture_revision = None;
                return ViewportInputRoute {
                    route_kind: ViewportInputRouteKind::Ignored,
                    viewport_id: Some(viewport.viewport_id.clone()),
                    viewport_kind: Some(ViewportKind::Game),
                    focused: true,
                    hovered: true,
                    input_event_kind: event.kind().to_string(),
                    reason: "game_view_presentation_revision_changed".to_string(),
                    runtime_input_frame: None,
                };
            }
            let Some(local_event) = viewport_host.map_game_event_to_runtime(&event) else {
                if matches!(event, EditorInputEvent::PointerUp { .. }) {
                    self.game_pointer_capture_revision = None;
                }
                return ViewportInputRoute {
                    route_kind: ViewportInputRouteKind::Ignored,
                    viewport_id: Some(viewport.viewport_id.clone()),
                    viewport_kind: Some(ViewportKind::Game),
                    focused: true,
                    hovered: false,
                    input_event_kind: event.kind().to_string(),
                    reason: "game_view_display_gutter".to_string(),
                    runtime_input_frame: None,
                };
            };
            if matches!(event, EditorInputEvent::PointerDown { .. }) {
                self.game_pointer_capture_revision = presentation_revision;
            } else if matches!(event, EditorInputEvent::PointerUp { .. }) {
                self.game_pointer_capture_revision = None;
            }
            let runtime_event = runtime_input_event_from_editor(&local_event);
            let pointer_position = pointer_position_from_event(&local_event);
            let mut frame =
                RuntimeInputFrame::new(self.frame_counter, viewport.viewport_id.clone());
            frame.pointer_position = pointer_position;
            frame.events.push(runtime_event);
            return ViewportInputRoute {
                route_kind: ViewportInputRouteKind::RuntimeInputFrame,
                viewport_id: Some(viewport.viewport_id.clone()),
                viewport_kind: Some(ViewportKind::Game),
                focused: true,
                hovered: true,
                input_event_kind: event.kind().to_string(),
                reason: "game_view_focused_local_coordinates".to_string(),
                runtime_input_frame: Some(frame),
            };
        }

        if matches!(event, EditorInputEvent::PointerUp { .. }) {
            self.game_pointer_capture_revision = None;
        }
        ViewportInputRoute::ignored(&event, "no_viewport_hit")
    }
}

fn event_hits_rect(event: &EditorInputEvent, rect: UiRect) -> bool {
    let Some((x, y)) = event_position(event) else {
        return true;
    };
    x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
}

fn event_position(event: &EditorInputEvent) -> Option<(f32, f32)> {
    match event {
        EditorInputEvent::PointerDown { x, y, .. }
        | EditorInputEvent::PointerUp { x, y, .. }
        | EditorInputEvent::PointerMove { x, y } => Some((*x, *y)),
        EditorInputEvent::MouseWheel { .. }
        | EditorInputEvent::KeyDown { .. }
        | EditorInputEvent::KeyUp { .. }
        | EditorInputEvent::FocusLost => None,
    }
}

fn pointer_position_from_event(event: &EditorInputEvent) -> Option<PointerPosition> {
    event_position(event).map(|(x, y)| PointerPosition { x, y })
}

fn runtime_input_event_from_editor(event: &EditorInputEvent) -> RuntimeInputEvent {
    match event {
        EditorInputEvent::PointerDown { x, y, button } => RuntimeInputEvent::PointerDown {
            x: *x,
            y: *y,
            button: runtime_pointer_button(*button),
        },
        EditorInputEvent::PointerUp { x, y, button } => RuntimeInputEvent::PointerUp {
            x: *x,
            y: *y,
            button: runtime_pointer_button(*button),
        },
        EditorInputEvent::PointerMove { x, y } => RuntimeInputEvent::PointerMove { x: *x, y: *y },
        EditorInputEvent::MouseWheel { delta } => RuntimeInputEvent::MouseWheel { delta: *delta },
        EditorInputEvent::KeyDown { key } => RuntimeInputEvent::KeyDown { key: key.clone() },
        EditorInputEvent::KeyUp { key } => RuntimeInputEvent::KeyUp { key: key.clone() },
        EditorInputEvent::FocusLost => unreachable!("focus loss is filtered before runtime route"),
    }
}

fn runtime_pointer_button(button: PointerButton) -> RuntimePointerButton {
    match button {
        PointerButton::Primary => RuntimePointerButton::Primary,
        PointerButton::Secondary => RuntimePointerButton::Secondary,
        PointerButton::Middle => RuntimePointerButton::Middle,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CminInputRoute {
    Ui,
    Viewport { viewport_id: String },
    Window,
    None,
}

pub fn route_input_for_cmin(
    router: &mut EditorInputRouter,
    viewport_host: &mut ViewportHost,
    event: EditorInputEvent,
    draw_list: &UiDrawList,
) -> CminInputRoute {
    let cmin_route = viewport_host.route_input(&event, draw_list);
    if cmin_route == CminInputRoute::None {
        let result = router.route(event, draw_list);
        if result.command.is_some() {
            return CminInputRoute::Ui;
        }
    }
    cmin_route
}
