use crate::config::NativeEditorWindowConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub created: bool,
    pub size: WindowSize,
    pub scale_factor: f64,
    pub close_requested: bool,
    pub redraw_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

pub struct HeadlessWindowBackend {
    state: WindowState,
}

impl HeadlessWindowBackend {
    pub fn create_window(config: &NativeEditorWindowConfig) -> Self {
        Self {
            state: WindowState {
                created: true,
                size: WindowSize {
                    width: config.width,
                    height: config.height,
                },
                scale_factor: config.scale_factor,
                close_requested: false,
                redraw_requested: false,
            },
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        self.state.size = WindowSize { width, height };
        self.state.scale_factor = scale_factor;
        self.state.redraw_requested = true;
    }

    pub fn request_redraw(&mut self) {
        self.state.redraw_requested = true;
    }

    pub fn close(&mut self) {
        self.state.close_requested = true;
    }

    pub fn snapshot(&self) -> WindowState {
        self.state.clone()
    }
}
