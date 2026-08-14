use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceState {
    pub created: bool,
    pub configured: bool,
    pub format: String,
    pub present_mode: String,
    pub width: u32,
    pub height: u32,
    pub acquired_frame: u64,
    pub presented_frame: u64,
    pub last_error: Option<String>,
}

pub struct HeadlessSurfaceBackend {
    state: SurfaceState,
}

impl HeadlessSurfaceBackend {
    pub fn create_surface() -> Self {
        Self {
            state: SurfaceState {
                created: true,
                configured: false,
                format: "Bgra8UnormSrgb".to_string(),
                present_mode: "Fifo".to_string(),
                width: 0,
                height: 0,
                acquired_frame: 0,
                presented_frame: 0,
                last_error: None,
            },
        }
    }

    pub fn configure(
        &mut self,
        width: u32,
        height: u32,
        format: impl Into<String>,
        present_mode: impl Into<String>,
    ) {
        self.state.configured = true;
        self.state.width = width;
        self.state.height = height;
        self.state.format = format.into();
        self.state.present_mode = present_mode.into();
        self.state.last_error = None;
    }

    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        format: impl Into<String>,
        present_mode: impl Into<String>,
    ) {
        self.configure(width, height, format, present_mode);
        self.state.acquired_frame = 0;
        self.state.presented_frame = 0;
    }

    pub fn acquire(&mut self) {
        if self.state.created && self.state.configured {
            self.state.acquired_frame += 1;
            self.state.last_error = None;
        } else {
            self.state.last_error = Some("surface_not_configured".to_string());
        }
    }

    pub fn present(&mut self) {
        if self.state.acquired_frame > self.state.presented_frame {
            self.state.presented_frame = self.state.acquired_frame;
            self.state.last_error = None;
        } else {
            self.state.last_error = Some("surface_frame_not_acquired".to_string());
        }
    }

    pub fn lose_surface(&mut self, error: impl Into<String>) {
        self.state.configured = false;
        self.state.last_error = Some(error.into());
    }

    pub fn snapshot(&self) -> SurfaceState {
        self.state.clone()
    }
}
