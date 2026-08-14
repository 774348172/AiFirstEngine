use crate::input_route::CminInputRoute;
use editor_input::{EditorInputEvent, PointerButton};
use editor_ui_renderer::{HitTarget, UiDrawList, UiPoint, UiRect};
use engine_runtime::game_view_presentation::{
    CanvasReferenceFact, GameViewExtent, GameViewPoint, GameViewPresentationModule,
    GameViewPresentationSpec, GameViewRect, GameViewScalePolicy, ResolvedGameViewPresentation,
};
use engine_runtime::runtime_renderer::ViewportTextureDescriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportKind {
    Scene,
    Game,
}

impl ViewportKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scene => "SceneView",
            Self::Game => "GameView",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportOutputKind {
    Clear,
    TestTriangle,
    TestTexture,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneCameraState {
    pub mode: SceneCameraMode,
    pub position: Vec3,
    pub target: Vec3,
    pub orbit_enabled: bool,
    pub pan_enabled: bool,
}

impl Default for SceneCameraState {
    fn default() -> Self {
        Self {
            mode: SceneCameraMode::Fixed,
            position: Vec3 {
                x: 0.0,
                y: 2.0,
                z: 6.0,
            },
            target: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            orbit_enabled: false,
            pan_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneCameraMode {
    Fixed,
    OrbitPanPlaceholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionOutlineState {
    pub enabled: bool,
    pub selected_entity_id: Option<String>,
    pub descriptor: String,
}

impl Default for SelectionOutlineState {
    fn default() -> Self {
        Self {
            enabled: false,
            selected_entity_id: None,
            descriptor: "selection-outline.placeholder".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GizmoState {
    pub mode: GizmoMode,
    pub descriptor: String,
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            mode: GizmoMode::Disabled,
            descriptor: "gizmo.disabled".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GizmoMode {
    Disabled,
    Placeholder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportState {
    pub viewport_id: String,
    pub kind: ViewportKind,
    pub rect: UiRect,
    pub focused: bool,
    pub output_kind: ViewportOutputKind,
    pub camera_state: SceneCameraState,
    pub selection_outline_state: SelectionOutlineState,
    pub gizmo_state: GizmoState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeViewportFrameSummary {
    pub viewport_id: String,
    pub target_id: String,
    pub texture_id: String,
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
}

impl RuntimeViewportFrameSummary {
    pub fn from_descriptor(
        viewport_id: impl Into<String>,
        descriptor: &ViewportTextureDescriptor,
    ) -> Self {
        Self {
            viewport_id: viewport_id.into(),
            target_id: descriptor.target_id.clone(),
            texture_id: descriptor.texture_id.clone(),
            frame_index: descriptor.frame_index,
            width: descriptor.width,
            height: descriptor.height,
        }
    }
}

pub struct ViewportHost {
    scene_viewport: Option<ViewportState>,
    game_viewport: Option<ViewportState>,
    latest_runtime_frame: Option<RuntimeViewportFrameSummary>,
    game_runtime_extent: Option<(u32, u32)>,
    game_presentation: Option<ResolvedGameViewPresentation>,
    game_canvas_references: Vec<CanvasReferenceFact>,
    game_presentation_revision: u64,
}

impl Default for ViewportHost {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportHost {
    pub fn new() -> Self {
        Self {
            scene_viewport: None,
            game_viewport: None,
            latest_runtime_frame: None,
            game_runtime_extent: None,
            game_presentation: None,
            game_canvas_references: Vec::new(),
            game_presentation_revision: 0,
        }
    }

    pub fn register_scene_viewport(
        &mut self,
        viewport_id: impl Into<String>,
        rect: UiRect,
    ) -> Result<(), String> {
        if self.scene_viewport.is_some() {
            return Err("scene_viewport_already_registered".to_string());
        }
        self.scene_viewport = Some(ViewportState {
            viewport_id: viewport_id.into(),
            kind: ViewportKind::Scene,
            rect,
            focused: false,
            output_kind: ViewportOutputKind::TestTriangle,
            camera_state: SceneCameraState::default(),
            selection_outline_state: SelectionOutlineState::default(),
            gizmo_state: GizmoState::default(),
        });
        Ok(())
    }

    pub fn register_game_viewport(
        &mut self,
        viewport_id: impl Into<String>,
        rect: UiRect,
    ) -> Result<(), String> {
        if self.game_viewport.is_some() {
            return Err("game_viewport_already_registered".to_string());
        }
        self.game_viewport = Some(ViewportState {
            viewport_id: viewport_id.into(),
            kind: ViewportKind::Game,
            rect,
            focused: false,
            output_kind: ViewportOutputKind::TestTexture,
            camera_state: SceneCameraState::default(),
            selection_outline_state: SelectionOutlineState::default(),
            gizmo_state: GizmoState::default(),
        });
        Ok(())
    }

    pub fn update_scene_rect(&mut self, rect: UiRect) -> Result<(), String> {
        let viewport = self.scene_viewport_mut()?;
        viewport.rect = rect;
        Ok(())
    }

    pub fn focus_scene(&mut self, focused: bool) -> Result<(), String> {
        let viewport = self.scene_viewport_mut()?;
        viewport.focused = focused;
        if focused {
            if let Some(game) = &mut self.game_viewport {
                game.focused = false;
            }
        }
        Ok(())
    }

    pub fn focus_game(&mut self, focused: bool) -> Result<(), String> {
        let viewport = self.game_viewport_mut()?;
        viewport.focused = focused;
        if focused {
            if let Some(scene) = &mut self.scene_viewport {
                scene.focused = false;
            }
        }
        Ok(())
    }

    pub fn update_game_rect(&mut self, rect: UiRect) -> Result<(), String> {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return Err("game_viewport_display_rect_invalid".to_string());
        }
        if self
            .game_viewport()
            .is_some_and(|viewport| viewport.rect == rect)
        {
            return Ok(());
        }
        self.game_viewport_mut()?.rect = rect;
        if let Some(presentation) = self.game_presentation.as_ref() {
            let extent = presentation.target_extent;
            let policy = presentation.scale_policy;
            self.resolve_game_presentation(extent.width, extent.height, policy)?;
        }
        Ok(())
    }

    pub fn clear_game_viewport(&mut self) {
        self.game_viewport = None;
        self.game_runtime_extent = None;
        self.game_presentation = None;
        self.game_canvas_references.clear();
    }

    pub fn update_game_runtime_extent(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.update_game_presentation(width, height, GameViewScalePolicy::Stretch)
    }

    pub fn update_game_presentation(
        &mut self,
        width: u32,
        height: u32,
        scale_policy: GameViewScalePolicy,
    ) -> Result<(), String> {
        self.update_game_presentation_with_canvases(width, height, scale_policy, Vec::new())
    }

    pub fn update_game_presentation_with_canvases(
        &mut self,
        width: u32,
        height: u32,
        scale_policy: GameViewScalePolicy,
        canvas_references: Vec<CanvasReferenceFact>,
    ) -> Result<(), String> {
        if self.game_viewport.is_none() {
            return Err("game_viewport_missing".to_string());
        }
        if width == 0 || height == 0 {
            return Err("game_viewport_runtime_extent_invalid".to_string());
        }
        let unchanged = self.game_presentation.as_ref().is_some_and(|presentation| {
            presentation.target_extent == GameViewExtent::new(width, height)
                && presentation.scale_policy == scale_policy
                && self.game_canvas_references == canvas_references
        });
        self.game_runtime_extent = Some((width, height));
        self.game_canvas_references = canvas_references;
        if unchanged {
            return Ok(());
        }
        self.resolve_game_presentation(width, height, scale_policy)
    }

    pub fn game_runtime_extent(&self) -> Option<(u32, u32)> {
        self.game_runtime_extent
    }

    pub fn game_display_content_rect(&self) -> Option<UiRect> {
        self.game_presentation.as_ref().map(|presentation| UiRect {
            x: presentation.display_content_rect.x,
            y: presentation.display_content_rect.y,
            width: presentation.display_content_rect.width,
            height: presentation.display_content_rect.height,
        })
    }

    pub fn game_presentation(&self) -> Option<&ResolvedGameViewPresentation> {
        self.game_presentation.as_ref()
    }

    pub fn game_presentation_revision(&self) -> Option<u64> {
        self.game_presentation
            .as_ref()
            .map(|presentation| presentation.identity.presentation_revision)
    }

    pub fn map_game_event_to_runtime(&self, event: &EditorInputEvent) -> Option<EditorInputEvent> {
        let Some(viewport) = self.game_viewport() else {
            return Some(event.clone());
        };
        if let Some(presentation) = self.game_presentation.as_ref() {
            return map_pointer_event(event, |x, y| {
                presentation
                    .display_to_target(GameViewPoint::new(x, y))
                    .ok()
                    .map(|point| (point.x, point.y))
            });
        }
        let local = editor_event_in_rect_local_space(event, viewport.rect);
        let Some((runtime_width, runtime_height)) = self.game_runtime_extent else {
            return Some(local);
        };
        let scale_x = runtime_width as f32 / viewport.rect.width;
        let scale_y = runtime_height as f32 / viewport.rect.height;
        Some(scale_pointer_event(
            local,
            scale_x,
            scale_y,
            runtime_width,
            runtime_height,
        ))
    }

    fn resolve_game_presentation(
        &mut self,
        width: u32,
        height: u32,
        scale_policy: GameViewScalePolicy,
    ) -> Result<(), String> {
        let (viewport_id, rect) = self
            .game_viewport()
            .map(|viewport| (viewport.viewport_id.clone(), viewport.rect))
            .ok_or_else(|| "game_viewport_missing".to_string())?;
        self.game_presentation_revision = self.game_presentation_revision.saturating_add(1);
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "editor-game-view-input".to_string(),
            target_id: viewport_id,
            target_extent: GameViewExtent::new(width, height),
            display_rect: GameViewRect::new(rect.x, rect.y, rect.width, rect.height),
            scale_policy,
            surface_generation: 1,
            presentation_revision: self.game_presentation_revision,
            canvas_references: self.game_canvas_references.clone(),
        })
        .map_err(|error| error.code.to_string())?;
        self.game_presentation = Some(presentation);
        Ok(())
    }

    pub fn set_output_kind(&mut self, output_kind: ViewportOutputKind) -> Result<(), String> {
        let viewport = self.scene_viewport_mut()?;
        viewport.output_kind = output_kind;
        Ok(())
    }

    pub fn set_camera_state(&mut self, camera_state: SceneCameraState) -> Result<(), String> {
        let viewport = self.scene_viewport_mut()?;
        viewport.camera_state = camera_state;
        Ok(())
    }

    pub fn set_selection_outline_state(
        &mut self,
        state: SelectionOutlineState,
    ) -> Result<(), String> {
        let viewport = self.scene_viewport_mut()?;
        viewport.selection_outline_state = state;
        Ok(())
    }

    pub fn set_gizmo_state(&mut self, state: GizmoState) -> Result<(), String> {
        let viewport = self.scene_viewport_mut()?;
        viewport.gizmo_state = state;
        Ok(())
    }

    pub fn scene_viewport(&self) -> Option<&ViewportState> {
        self.scene_viewport.as_ref()
    }

    pub fn game_viewport(&self) -> Option<&ViewportState> {
        self.game_viewport.as_ref()
    }

    pub fn latest_runtime_frame(&self) -> Option<&RuntimeViewportFrameSummary> {
        self.latest_runtime_frame.as_ref()
    }

    pub fn ingest_runtime_frame(
        &mut self,
        summary: RuntimeViewportFrameSummary,
    ) -> Result<(), String> {
        let viewport = self
            .scene_viewport
            .as_ref()
            .ok_or_else(|| "scene_viewport_missing".to_string())?;
        if viewport.viewport_id != summary.viewport_id {
            return Err("runtime_frame_viewport_mismatch".to_string());
        }
        self.latest_runtime_frame = Some(summary);
        Ok(())
    }

    pub fn route_input(
        &mut self,
        event: &EditorInputEvent,
        draw_list: &UiDrawList,
    ) -> CminInputRoute {
        match event {
            EditorInputEvent::PointerDown {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                let hit = editor_ui_renderer::hit_test(draw_list, UiPoint { x: *x, y: *y });
                match hit.map(|region| &region.target) {
                    Some(HitTarget::Viewport) => {
                        let _ = self.focus_scene(true);
                        CminInputRoute::Viewport {
                            viewport_id: self
                                .scene_viewport
                                .as_ref()
                                .map(|viewport| viewport.viewport_id.clone())
                                .unwrap_or_else(|| "missing".to_string()),
                        }
                    }
                    Some(_) => CminInputRoute::Ui,
                    None => CminInputRoute::None,
                }
            }
            _ => CminInputRoute::None,
        }
    }

    fn scene_viewport_mut(&mut self) -> Result<&mut ViewportState, String> {
        self.scene_viewport
            .as_mut()
            .ok_or_else(|| "scene_viewport_missing".to_string())
    }

    fn game_viewport_mut(&mut self) -> Result<&mut ViewportState, String> {
        self.game_viewport
            .as_mut()
            .ok_or_else(|| "game_viewport_missing".to_string())
    }
}

fn map_pointer_event(
    event: &EditorInputEvent,
    map: impl Fn(f32, f32) -> Option<(f32, f32)>,
) -> Option<EditorInputEvent> {
    match event {
        EditorInputEvent::PointerDown { x, y, button } => {
            let (x, y) = map(*x, *y)?;
            Some(EditorInputEvent::PointerDown {
                x,
                y,
                button: *button,
            })
        }
        EditorInputEvent::PointerUp { x, y, button } => {
            let (x, y) = map(*x, *y)?;
            Some(EditorInputEvent::PointerUp {
                x,
                y,
                button: *button,
            })
        }
        EditorInputEvent::PointerMove { x, y } => {
            let (x, y) = map(*x, *y)?;
            Some(EditorInputEvent::PointerMove { x, y })
        }
        other => Some(other.clone()),
    }
}

fn editor_event_in_rect_local_space(event: &EditorInputEvent, rect: UiRect) -> EditorInputEvent {
    match event {
        EditorInputEvent::PointerDown { x, y, button } => EditorInputEvent::PointerDown {
            x: x - rect.x,
            y: y - rect.y,
            button: *button,
        },
        EditorInputEvent::PointerUp { x, y, button } => EditorInputEvent::PointerUp {
            x: x - rect.x,
            y: y - rect.y,
            button: *button,
        },
        EditorInputEvent::PointerMove { x, y } => EditorInputEvent::PointerMove {
            x: x - rect.x,
            y: y - rect.y,
        },
        other => other.clone(),
    }
}

fn scale_pointer_event(
    event: EditorInputEvent,
    scale_x: f32,
    scale_y: f32,
    runtime_width: u32,
    runtime_height: u32,
) -> EditorInputEvent {
    let map = |x: f32, y: f32| {
        (
            (x * scale_x).clamp(0.0, runtime_width as f32),
            (y * scale_y).clamp(0.0, runtime_height as f32),
        )
    };
    match event {
        EditorInputEvent::PointerDown { x, y, button } => {
            let (x, y) = map(x, y);
            EditorInputEvent::PointerDown { x, y, button }
        }
        EditorInputEvent::PointerUp { x, y, button } => {
            let (x, y) = map(x, y);
            EditorInputEvent::PointerUp { x, y, button }
        }
        EditorInputEvent::PointerMove { x, y } => {
            let (x, y) = map(x, y);
            EditorInputEvent::PointerMove { x, y }
        }
        other => other,
    }
}
