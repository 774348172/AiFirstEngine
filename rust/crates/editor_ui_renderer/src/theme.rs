use crate::UiColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSurfaceTheme {
    pub root: UiColor,
    pub chrome: UiColor,
    pub toolbar: UiColor,
    pub panel: UiColor,
    pub panel_raised: UiColor,
    pub panel_recessed: UiColor,
    pub viewport: UiColor,
    pub field: UiColor,
    pub popup: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorBorderTheme {
    pub subtle: UiColor,
    pub normal: UiColor,
    pub focused: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorTextTheme {
    pub primary: UiColor,
    pub secondary: UiColor,
    pub disabled: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSelectionTheme {
    pub active: UiColor,
    pub inactive: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorStatusTheme {
    pub warning: UiColor,
    pub error: UiColor,
    pub success: UiColor,
    pub error_surface: UiColor,
    pub pending_surface: UiColor,
    pub ready_surface: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorOverlayTheme {
    pub scrim: UiColor,
    pub drop_preview: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorTheme {
    pub surface: EditorSurfaceTheme,
    pub border: EditorBorderTheme,
    pub text: EditorTextTheme,
    pub selection: EditorSelectionTheme,
    pub accent_primary: UiColor,
    pub status: EditorStatusTheme,
    pub overlay: EditorOverlayTheme,
}

impl EditorTheme {
    pub const DARK_NEUTRAL: Self = Self {
        surface: EditorSurfaceTheme {
            root: UiColor::rgba(24, 24, 24, 255),
            chrome: UiColor::rgba(20, 20, 20, 255),
            toolbar: UiColor::rgba(31, 31, 31, 255),
            panel: UiColor::rgba(38, 38, 38, 255),
            panel_raised: UiColor::rgba(48, 48, 48, 255),
            panel_recessed: UiColor::rgba(30, 30, 30, 255),
            viewport: UiColor::rgba(18, 18, 18, 255),
            field: UiColor::rgba(51, 51, 51, 255),
            popup: UiColor::rgba(34, 34, 34, 255),
        },
        border: EditorBorderTheme {
            subtle: UiColor::rgba(18, 18, 18, 255),
            normal: UiColor::rgba(58, 58, 58, 255),
            focused: UiColor::rgba(66, 137, 201, 255),
        },
        text: EditorTextTheme {
            primary: UiColor::rgba(224, 224, 224, 255),
            secondary: UiColor::rgba(162, 162, 162, 255),
            disabled: UiColor::rgba(102, 102, 102, 255),
        },
        selection: EditorSelectionTheme {
            active: UiColor::rgba(55, 91, 124, 255),
            inactive: UiColor::rgba(55, 55, 55, 255),
        },
        accent_primary: UiColor::rgba(66, 137, 201, 255),
        status: EditorStatusTheme {
            warning: UiColor::rgba(201, 161, 68, 255),
            error: UiColor::rgba(194, 72, 72, 255),
            success: UiColor::rgba(81, 154, 101, 255),
            error_surface: UiColor::rgba(58, 24, 28, 248),
            pending_surface: UiColor::rgba(37, 45, 52, 255),
            ready_surface: UiColor::rgba(28, 35, 40, 255),
        },
        overlay: EditorOverlayTheme {
            scrim: UiColor::rgba(0, 0, 0, 156),
            drop_preview: UiColor::rgba(66, 137, 201, 96),
        },
    };
}

impl UiColor {
    pub const IDENTITY_TINT: Self = Self::rgba(255, 255, 255, 255);
    pub const ROOT: Self = EditorTheme::DARK_NEUTRAL.surface.root;
    pub const MENU: Self = EditorTheme::DARK_NEUTRAL.surface.chrome;
    pub const TOOLBAR: Self = EditorTheme::DARK_NEUTRAL.surface.toolbar;
    pub const PANEL: Self = EditorTheme::DARK_NEUTRAL.surface.panel;
    pub const PANEL_DARK: Self = EditorTheme::DARK_NEUTRAL.surface.panel_recessed;
    pub const PANEL_LIGHT: Self = EditorTheme::DARK_NEUTRAL.surface.panel_raised;
    pub const TAB: Self = EditorTheme::DARK_NEUTRAL.surface.panel_recessed;
    pub const TAB_ACTIVE: Self = EditorTheme::DARK_NEUTRAL.selection.inactive;
    pub const FIELD: Self = EditorTheme::DARK_NEUTRAL.surface.field;
    pub const BORDER: Self = EditorTheme::DARK_NEUTRAL.border.subtle;
    pub const VIEWPORT_BG: Self = EditorTheme::DARK_NEUTRAL.surface.viewport;
    pub const TEXT: Self = EditorTheme::DARK_NEUTRAL.text.primary;
    pub const TEXT_MUTED: Self = EditorTheme::DARK_NEUTRAL.text.secondary;
    pub const ACCENT: Self = EditorTheme::DARK_NEUTRAL.accent_primary;
    pub const WARNING: Self = EditorTheme::DARK_NEUTRAL.status.warning;
    pub const ERROR: Self = EditorTheme::DARK_NEUTRAL.status.error;

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_dark_theme_has_complete_semantic_contrast() {
        let theme = EditorTheme::DARK_NEUTRAL;
        assert_ne!(theme.surface.root, theme.surface.panel);
        assert_ne!(theme.surface.panel, theme.surface.panel_raised);
        assert_ne!(theme.text.primary, theme.text.secondary);
        assert_ne!(theme.selection.active, theme.surface.panel);
        assert_ne!(theme.overlay.drop_preview, theme.overlay.scrim);
        assert_eq!(UiColor::PANEL, theme.surface.panel);
        assert_eq!(UiColor::ACCENT, theme.accent_primary);
    }

    #[test]
    fn editor_dark_theme_has_no_panel_local_rgba_literals() {
        let sources = [
            include_str!("renderer.rs"),
            include_str!("panels/launcher.rs"),
            include_str!("panels/project_browser.rs"),
        ];
        let count = sources
            .iter()
            .map(|source| source.matches("UiColor::rgba").count())
            .sum::<usize>();
        assert_eq!(count, 0, "panel-local colors must use EditorTheme tokens");
    }
}
