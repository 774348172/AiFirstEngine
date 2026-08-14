use std::collections::{BTreeMap, BTreeSet};

use editor_ui_renderer::{WorkspaceWindowId, WorkspaceWindowPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NativeWorkspaceWindowKey(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NativeWorkspaceWindowMetrics {
    pub(crate) physical_width: u32,
    pub(crate) physical_height: u32,
    pub(crate) scale_factor: f64,
    pub(crate) focused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeWorkspaceWindowState {
    pub(crate) workspace_window_id: WorkspaceWindowId,
    pub(crate) native_key: NativeWorkspaceWindowKey,
    pub(crate) renderer_owner: u64,
    pub(crate) surface_owner: u64,
    pub(crate) metrics: NativeWorkspaceWindowMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeWorkspaceWindowCreateStage {
    Window,
    Surface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeWorkspaceWindowDiagnostic {
    pub(crate) code: &'static str,
    pub(crate) window_id: WorkspaceWindowId,
    pub(crate) stage: NativeWorkspaceWindowCreateStage,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NativeWorkspaceWindowEvent {
    CloseRequested,
    Resized {
        physical_width: u32,
        physical_height: u32,
    },
    ScaleFactorChanged {
        scale_factor: f64,
    },
    Focused(bool),
    RedrawRequested,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NativeWorkspaceWindowAction {
    ShutdownEditor,
    CloseFloating(WorkspaceWindowId),
    Resized {
        window_id: WorkspaceWindowId,
        logical_width: u32,
        logical_height: u32,
    },
    ScaleFactorChanged {
        window_id: WorkspaceWindowId,
        scale_factor: f64,
        logical_width: u32,
        logical_height: u32,
    },
    FocusChanged {
        window_id: WorkspaceWindowId,
        focused: bool,
    },
    Redraw(WorkspaceWindowId),
}

pub(crate) trait NativeWorkspaceWindowFactory {
    fn create(
        &mut self,
        window_id: &WorkspaceWindowId,
    ) -> Result<NativeWorkspaceWindowState, NativeWorkspaceWindowCreateStage>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeDragProxyKey(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeDragProxyStage {
    Create,
    Move,
    Destroy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeDragProxyDiagnostic {
    pub(crate) code: &'static str,
    pub(crate) stage: NativeDragProxyStage,
}

pub(crate) trait NativeDragProxyFactory {
    fn create(&mut self) -> Result<NativeDragProxyKey, NativeDragProxyStage>;
    fn move_to(
        &mut self,
        key: NativeDragProxyKey,
        screen_x: i32,
        screen_y: i32,
    ) -> Result<(), NativeDragProxyStage>;
    fn destroy(&mut self, key: NativeDragProxyKey) -> Result<(), NativeDragProxyStage>;
}

#[derive(Debug, Default)]
pub(crate) struct NativeWorkspaceWindowHost {
    windows: BTreeMap<WorkspaceWindowId, NativeWorkspaceWindowState>,
    native_to_workspace: BTreeMap<NativeWorkspaceWindowKey, WorkspaceWindowId>,
    drag_proxy: Option<NativeDragProxyKey>,
}

impl NativeWorkspaceWindowHost {
    pub(crate) fn reconcile(
        &mut self,
        plan: &WorkspaceWindowPlan,
        factory: &mut impl NativeWorkspaceWindowFactory,
    ) -> Vec<NativeWorkspaceWindowDiagnostic> {
        let desired = plan
            .windows
            .iter()
            .map(|entry| entry.window_id.clone())
            .collect::<BTreeSet<_>>();
        let removed = self
            .windows
            .keys()
            .filter(|window_id| !window_id.is_main() && !desired.contains(*window_id))
            .cloned()
            .collect::<Vec<_>>();
        for window_id in removed {
            if let Some(state) = self.windows.remove(&window_id) {
                self.native_to_workspace.remove(&state.native_key);
            }
        }

        let mut diagnostics = Vec::new();
        for entry in &plan.windows {
            if self.windows.contains_key(&entry.window_id) {
                continue;
            }
            match factory.create(&entry.window_id) {
                Ok(mut state) => {
                    state.workspace_window_id = entry.window_id.clone();
                    self.native_to_workspace
                        .insert(state.native_key, entry.window_id.clone());
                    self.windows.insert(entry.window_id.clone(), state);
                }
                Err(stage) => diagnostics.push(NativeWorkspaceWindowDiagnostic {
                    code: match stage {
                        NativeWorkspaceWindowCreateStage::Window => {
                            "native_workspace_window_create_failed"
                        }
                        NativeWorkspaceWindowCreateStage::Surface => {
                            "native_workspace_surface_create_failed"
                        }
                    },
                    window_id: entry.window_id.clone(),
                    stage,
                }),
            }
        }
        diagnostics
    }

    pub(crate) fn windows(&self) -> &BTreeMap<WorkspaceWindowId, NativeWorkspaceWindowState> {
        &self.windows
    }

    pub(crate) fn workspace_window_id(
        &self,
        native_key: NativeWorkspaceWindowKey,
    ) -> Option<&WorkspaceWindowId> {
        self.native_to_workspace.get(&native_key)
    }

    pub(crate) fn route_event(
        &mut self,
        native_key: NativeWorkspaceWindowKey,
        event: NativeWorkspaceWindowEvent,
    ) -> Option<NativeWorkspaceWindowAction> {
        let window_id = self.native_to_workspace.get(&native_key)?.clone();
        let state = self.windows.get_mut(&window_id)?;
        let logical_size = |metrics: NativeWorkspaceWindowMetrics| {
            (
                (f64::from(metrics.physical_width) / metrics.scale_factor).round() as u32,
                (f64::from(metrics.physical_height) / metrics.scale_factor).round() as u32,
            )
        };
        Some(match event {
            NativeWorkspaceWindowEvent::CloseRequested if window_id.is_main() => {
                NativeWorkspaceWindowAction::ShutdownEditor
            }
            NativeWorkspaceWindowEvent::CloseRequested => {
                NativeWorkspaceWindowAction::CloseFloating(window_id)
            }
            NativeWorkspaceWindowEvent::Resized {
                physical_width,
                physical_height,
            } => {
                state.metrics.physical_width = physical_width;
                state.metrics.physical_height = physical_height;
                let (logical_width, logical_height) = logical_size(state.metrics);
                NativeWorkspaceWindowAction::Resized {
                    window_id,
                    logical_width,
                    logical_height,
                }
            }
            NativeWorkspaceWindowEvent::ScaleFactorChanged { scale_factor } => {
                state.metrics.scale_factor = scale_factor;
                let (logical_width, logical_height) = logical_size(state.metrics);
                NativeWorkspaceWindowAction::ScaleFactorChanged {
                    window_id,
                    scale_factor,
                    logical_width,
                    logical_height,
                }
            }
            NativeWorkspaceWindowEvent::Focused(focused) => {
                state.metrics.focused = focused;
                NativeWorkspaceWindowAction::FocusChanged { window_id, focused }
            }
            NativeWorkspaceWindowEvent::RedrawRequested => {
                NativeWorkspaceWindowAction::Redraw(window_id)
            }
        })
    }

    pub(crate) fn reconcile_drag_proxy(
        &mut self,
        screen_position: Option<(i32, i32)>,
        factory: &mut impl NativeDragProxyFactory,
    ) -> Vec<NativeDragProxyDiagnostic> {
        let Some((screen_x, screen_y)) = screen_position else {
            let Some(key) = self.drag_proxy.take() else {
                return Vec::new();
            };
            return match factory.destroy(key) {
                Ok(()) => Vec::new(),
                Err(stage) => vec![NativeDragProxyDiagnostic {
                    code: "native_drag_proxy_destroy_failed",
                    stage,
                }],
            };
        };
        let key = match self.drag_proxy {
            Some(key) => key,
            None => match factory.create() {
                Ok(key) => {
                    self.drag_proxy = Some(key);
                    key
                }
                Err(stage) => {
                    return vec![NativeDragProxyDiagnostic {
                        code: "native_drag_proxy_create_failed",
                        stage,
                    }];
                }
            },
        };
        if let Err(stage) = factory.move_to(key, screen_x, screen_y) {
            self.drag_proxy = None;
            let _ = factory.destroy(key);
            return vec![NativeDragProxyDiagnostic {
                code: "native_drag_proxy_move_failed",
                stage,
            }];
        }
        Vec::new()
    }

    pub(crate) fn drag_proxy_is_live(&self) -> bool {
        self.drag_proxy.is_some()
    }
}
