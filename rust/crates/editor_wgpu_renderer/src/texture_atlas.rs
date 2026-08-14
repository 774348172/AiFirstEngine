use crate::draw_plan::UiUvRect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub(crate) font_face_id: u16,
    pub(crate) ch: char,
    pub(crate) px_size: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GlyphAtlasEntry {
    pub(crate) uv: UiUvRect,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bearing_x: f32,
    pub(crate) bearing_y: f32,
    pub(crate) advance: f32,
}

pub(crate) struct CpuGlyphAtlas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
    cursor_x: u32,
    cursor_y: u32,
    row_h: u32,
}

impl CpuGlyphAtlas {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
            cursor_x: 1,
            cursor_y: 1,
            row_h: 0,
        }
    }

    pub(crate) fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let width = width.max(1) + 2;
        let height = height.max(1) + 2;
        if self.cursor_x + width >= self.width {
            self.cursor_x = 1;
            self.cursor_y += self.row_h + 1;
            self.row_h = 0;
        }
        if self.cursor_y + height >= self.height {
            return None;
        }
        let pos = (self.cursor_x, self.cursor_y);
        self.cursor_x += width;
        self.row_h = self.row_h.max(height);
        Some(pos)
    }

    pub(crate) fn write_alpha(&mut self, x: u32, y: u32, width: u32, height: u32, bitmap: &[u8]) {
        for row in 0..height {
            for col in 0..width {
                let src = (row * width + col) as usize;
                let dst = ((y + row) * self.width + x + col) as usize;
                if let Some(value) = bitmap.get(src) {
                    self.pixels[dst] = *value;
                }
            }
        }
    }
}
