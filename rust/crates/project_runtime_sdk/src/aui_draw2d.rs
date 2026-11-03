//! Bounded, retained 2D authoring descriptions. These are SDK values, not runtime
//! handles: a renderer/transport must validate them before consuming them.
//! Paint colors are linear normalized RGBA with straight alpha, matching custom
//! mesh vertices and NodePresentation::tint; they are not hexadecimal sRGB colors.

use crate::aui_presentation::validate_rgba;
use serde::{Deserialize, Serialize};

pub const GRAPHICS_2D_VERSION: u32 = 1;
pub const MAX_PATH_COMMANDS: usize = 4096;
pub const MAX_DRAW_ITEMS: usize = 256;
pub const GRAPHICS_ASSET_SCHEMA: &str = "graphics2d-asset.v1";

/// Project asset payload, consumed through the existing Asset Graph and package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Graphics2DAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub graphics: Graphics2D,
}

impl Graphics2DAsset {
    pub fn validate(&self) -> Result<(), Draw2DError> {
        if self.schema_version != GRAPHICS_ASSET_SCHEMA {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::UnsupportedVersion,
                index: 0,
            });
        }
        if self.asset_id.is_empty()
            || self.asset_guid.is_empty()
            || self.asset_id.len() > 128
            || self.asset_guid.len() > 128
        {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::InvalidValue,
                index: 0,
            });
        }
        self.graphics.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Draw2DErrorKind {
    UnsupportedVersion,
    CapacityExceeded,
    MissingSubpath,
    InvalidValue,
}

/// Index identifies the command or draw item at the failing validation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draw2DError {
    pub kind: Draw2DErrorKind,
    pub index: usize,
}

impl std::fmt::Display for Draw2DError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at index {}", self.kind, self.index)
    }
}
impl std::error::Error for Draw2DError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PathCommand {
    Move {
        to: [f32; 2],
    },
    Line {
        to: [f32; 2],
    },
    Quadratic {
        control: [f32; 2],
        to: [f32; 2],
    },
    Cubic {
        control1: [f32; 2],
        control2: [f32; 2],
        to: [f32; 2],
    },
    /// Connects the current point to the ellipse's start, then follows the arc.
    /// Positive sweep is clockwise on a Y-down screen; at most one turn per command.
    Arc {
        center: [f32; 2],
        radii: [f32; 2],
        start: f32,
        sweep: f32,
    },
    Close,
}

impl PathCommand {
    fn validate(&self, active: bool, index: usize) -> Result<bool, Draw2DError> {
        let point = |p: &[f32; 2]| p.iter().all(|v| v.is_finite());
        let valid = match self {
            Self::Move { to } | Self::Line { to } => point(to),
            Self::Quadratic { control, to } => point(control) && point(to),
            Self::Cubic {
                control1,
                control2,
                to,
            } => point(control1) && point(control2) && point(to),
            Self::Arc {
                center,
                radii,
                start,
                sweep,
            } => {
                point(center)
                    && point(radii)
                    && radii.iter().all(|r| *r >= 0.0)
                    && start.is_finite()
                    && sweep.is_finite()
                    && sweep.abs() <= std::f32::consts::TAU
                    && (0..2).all(|axis| {
                        f64::from(center[axis]).abs() + f64::from(radii[axis])
                            <= f64::from(f32::MAX)
                    })
            }
            Self::Close => true,
        };
        if !valid {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::InvalidValue,
                index,
            });
        }
        if matches!(self, Self::Move { .. }) {
            return Ok(true);
        }
        if !active {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::MissingSubpath,
                index,
            });
        }
        Ok(!matches!(self, Self::Close))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Path2D {
    commands: Vec<PathCommand>,
}

impl Path2D {
    pub fn line(from: [f32; 2], to: [f32; 2]) -> Result<Self, Draw2DError> {
        let mut builder = PathBuilder::new();
        builder.move_to(from)?.line_to(to)?;
        Ok(builder.finish())
    }

    /// Empty and degenerate polygons are valid; a filled area needs three points.
    pub fn polygon(points: &[[f32; 2]], closed: bool) -> Result<Self, Draw2DError> {
        let mut builder = PathBuilder::new();
        if let Some(first) = points.first() {
            builder.move_to(*first)?;
            for point in &points[1..] {
                builder.line_to(*point)?;
            }
            if closed {
                builder.close()?;
            }
        }
        Ok(builder.finish())
    }

    pub fn rectangle([x, y, width, height]: [f32; 4]) -> Result<Self, Draw2DError> {
        Self::rounded_rectangle([x, y, width, height], 0.0)
    }

    /// Radius clamps to half the smaller dimension; negative dimensions are errors.
    pub fn rounded_rectangle(
        [x, y, width, height]: [f32; 4],
        radius: f32,
    ) -> Result<Self, Draw2DError> {
        if ![x, y, width, height, radius].iter().all(|v| v.is_finite())
            || width < 0.0
            || height < 0.0
            || radius < 0.0
        {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::InvalidValue,
                index: 0,
            });
        }
        let r = radius.min(width.min(height) * 0.5);
        if r == 0.0 {
            return Self::polygon(
                &[
                    [x, y],
                    [x + width, y],
                    [x + width, y + height],
                    [x, y + height],
                ],
                true,
            );
        }
        let mut builder = PathBuilder::new();
        builder
            .move_to([x + r, y])?
            .line_to([x + width - r, y])?
            .arc(
                [x + width - r, y + r],
                [r, r],
                -std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_2,
            )?
            .line_to([x + width, y + height - r])?
            .arc(
                [x + width - r, y + height - r],
                [r, r],
                0.0,
                std::f32::consts::FRAC_PI_2,
            )?
            .line_to([x + r, y + height])?
            .arc(
                [x + r, y + height - r],
                [r, r],
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_2,
            )?
            .line_to([x, y + r])?
            .arc(
                [x + r, y + r],
                [r, r],
                std::f32::consts::PI,
                std::f32::consts::FRAC_PI_2,
            )?
            .close()?;
        Ok(builder.finish())
    }

    pub fn circle(center: [f32; 2], radius: f32) -> Result<Self, Draw2DError> {
        Self::ellipse(center, [radius; 2])
    }

    pub fn ellipse(center: [f32; 2], radii: [f32; 2]) -> Result<Self, Draw2DError> {
        let mut builder = PathBuilder::new();
        builder
            .move_to([center[0] + radii[0], center[1]])?
            .arc(center, radii, 0.0, std::f32::consts::TAU)?
            .close()?;
        Ok(builder.finish())
    }

    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// Required after deserialization: serde is not the trust boundary.
    pub fn validate(&self) -> Result<(), Draw2DError> {
        if self.commands.len() > MAX_PATH_COMMANDS {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::CapacityExceeded,
                index: MAX_PATH_COMMANDS,
            });
        }
        let mut active = false;
        for (index, command) in self.commands.iter().enumerate() {
            active = command.validate(active, index)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct PathBuilder {
    path: Path2D,
    active: bool,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Errors leave both commands and subpath state unchanged.
    pub fn push(&mut self, command: PathCommand) -> Result<&mut Self, Draw2DError> {
        let index = self.path.commands.len();
        if index >= MAX_PATH_COMMANDS {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::CapacityExceeded,
                index,
            });
        }
        let active = command.validate(self.active, index)?;
        self.path.commands.push(command);
        self.active = active;
        Ok(self)
    }

    pub fn move_to(&mut self, to: [f32; 2]) -> Result<&mut Self, Draw2DError> {
        self.push(PathCommand::Move { to })
    }
    pub fn line_to(&mut self, to: [f32; 2]) -> Result<&mut Self, Draw2DError> {
        self.push(PathCommand::Line { to })
    }
    pub fn quadratic_to(
        &mut self,
        control: [f32; 2],
        to: [f32; 2],
    ) -> Result<&mut Self, Draw2DError> {
        self.push(PathCommand::Quadratic { control, to })
    }
    pub fn cubic_to(
        &mut self,
        control1: [f32; 2],
        control2: [f32; 2],
        to: [f32; 2],
    ) -> Result<&mut Self, Draw2DError> {
        self.push(PathCommand::Cubic {
            control1,
            control2,
            to,
        })
    }
    pub fn arc(
        &mut self,
        center: [f32; 2],
        radii: [f32; 2],
        start: f32,
        sweep: f32,
    ) -> Result<&mut Self, Draw2DError> {
        self.push(PathCommand::Arc {
            center,
            radii,
            start,
            sweep,
        })
    }
    pub fn close(&mut self) -> Result<&mut Self, Draw2DError> {
        self.push(PathCommand::Close)
    }
    pub fn finish(self) -> Path2D {
        self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineJoin {
    Bevel,
    Round,
    Miter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrokeStyle {
    pub color: [f32; 4],
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: [1.0; 4],
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

impl StrokeStyle {
    pub fn validate(&self) -> Result<(), Draw2DError> {
        if validate_rgba(self.color).is_err()
            || !self.width.is_finite()
            || self.width < 0.0
            || !self.miter_limit.is_finite()
            || self.miter_limit < 1.0
        {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::InvalidValue,
                index: 0,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Draw2DItem {
    Stroke {
        path: Path2D,
        style: StrokeStyle,
    },
    Fill {
        path: Path2D,
        color: [f32; 4],
        rule: FillRule,
    },
}

impl Draw2DItem {
    fn path(&self) -> &Path2D {
        match self {
            Self::Stroke { path, .. } | Self::Fill { path, .. } => path,
        }
    }
    pub fn validate(&self) -> Result<(), Draw2DError> {
        self.path().validate()?;
        match self {
            Self::Stroke { style, .. } => style.validate(),
            Self::Fill { color, .. } => validate_rgba(*color).map_err(|_| Draw2DError {
                kind: Draw2DErrorKind::InvalidValue,
                index: 0,
            }),
        }
    }
}

/// Immutable when published. Cloning an authoring value does not create a runtime handle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Graphics2D {
    version: u32,
    items: Vec<Draw2DItem>,
}

impl Default for Graphics2D {
    fn default() -> Self {
        Self {
            version: GRAPHICS_2D_VERSION,
            items: Vec::new(),
        }
    }
}

impl Graphics2D {
    pub fn items(&self) -> &[Draw2DItem] {
        &self.items
    }

    pub fn validate(&self) -> Result<(), Draw2DError> {
        if self.version != GRAPHICS_2D_VERSION {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::UnsupportedVersion,
                index: 0,
            });
        }
        if self.items.len() > MAX_DRAW_ITEMS {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::CapacityExceeded,
                index: MAX_DRAW_ITEMS,
            });
        }
        let mut commands = 0;
        for (index, item) in self.items.iter().enumerate() {
            item.validate()?;
            commands += item.path().commands.len();
            if commands > MAX_PATH_COMMANDS {
                return Err(Draw2DError {
                    kind: Draw2DErrorKind::CapacityExceeded,
                    index,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Draw2D {
    graphics: Graphics2D,
    commands: usize,
}

impl Draw2D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, item: Draw2DItem) -> Result<&mut Self, Draw2DError> {
        let index = self.graphics.items.len();
        item.validate()?;
        let commands = self.commands + item.path().commands.len();
        if index >= MAX_DRAW_ITEMS || commands > MAX_PATH_COMMANDS {
            return Err(Draw2DError {
                kind: Draw2DErrorKind::CapacityExceeded,
                index,
            });
        }
        self.graphics.items.push(item);
        self.commands = commands;
        Ok(self)
    }

    pub fn stroke(&mut self, path: Path2D, style: StrokeStyle) -> Result<&mut Self, Draw2DError> {
        self.push(Draw2DItem::Stroke { path, style })
    }
    pub fn fill(
        &mut self,
        path: Path2D,
        color: [f32; 4],
        rule: FillRule,
    ) -> Result<&mut Self, Draw2DError> {
        self.push(Draw2DItem::Fill { path, color, rule })
    }
    pub fn finish(self) -> Graphics2D {
        self.graphics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_helpers_validate_dimensions_and_asset_contract() {
        for path in [
            Path2D::rectangle([0., 0., 20., 10.]).unwrap(),
            Path2D::rounded_rectangle([0., 0., 20., 10.], 100.).unwrap(),
            Path2D::ellipse([0., 0.], [20., 10.]).unwrap(),
        ] {
            path.validate().unwrap();
        }
        assert!(Path2D::rectangle([0., 0., -1., 10.]).is_err());
        assert!(Path2D::circle([0., 0.], -1.).is_err());
        assert!(Path2D::rectangle([f32::MAX, 0., f32::MAX, 1.]).is_err());
        let mut asset = Graphics2DAsset {
            schema_version: GRAPHICS_ASSET_SCHEMA.into(),
            asset_id: "shape".into(),
            asset_guid: "guid-shape".into(),
            graphics: Graphics2D::default(),
        };
        asset.validate().unwrap();
        asset.asset_guid.clear();
        assert!(asset.validate().is_err());
    }

    #[test]
    fn invalid_command_does_not_change_subpath_or_content() {
        let mut builder = PathBuilder::new();
        assert_eq!(
            builder.line_to([1.0; 2]).unwrap_err().kind,
            Draw2DErrorKind::MissingSubpath
        );
        builder.move_to([0.0; 2]).unwrap();
        assert!(builder.line_to([f32::NAN, 0.0]).is_err());
        builder.line_to([1.0; 2]).unwrap().close().unwrap();
        assert!(builder.line_to([2.0; 2]).is_err());
        assert_eq!(builder.finish().commands().len(), 3);
    }

    #[test]
    fn curves_multiple_subpaths_and_full_arc_round_trip() {
        let mut builder = PathBuilder::new();
        builder
            .move_to([1.0, 0.0])
            .unwrap()
            .arc([0.0; 2], [1.0; 2], 0.0, std::f32::consts::TAU)
            .unwrap()
            .close()
            .unwrap()
            .move_to([0.0; 2])
            .unwrap()
            .quadratic_to([1.0; 2], [2.0; 2])
            .unwrap()
            .cubic_to([3.0; 2], [4.0; 2], [5.0; 2])
            .unwrap();
        let mut draw = Draw2D::new();
        draw.fill(builder.finish(), [0.5; 4], FillRule::EvenOdd)
            .unwrap();
        let graphics = draw.finish();
        let decoded: Graphics2D =
            serde_json::from_slice(&serde_json::to_vec(&graphics).unwrap()).unwrap();
        decoded.validate().unwrap();
        assert_eq!(graphics, decoded);
    }

    #[test]
    fn deserialized_descriptions_require_state_and_version_validation() {
        let path: Path2D =
            serde_json::from_str(r#"{"commands":[{"kind":"line","to":[1,2]}]}"#).unwrap();
        assert_eq!(
            path.validate().unwrap_err().kind,
            Draw2DErrorKind::MissingSubpath
        );
        let graphics: Graphics2D = serde_json::from_str(r#"{"version":2,"items":[]}"#).unwrap();
        assert_eq!(
            graphics.validate().unwrap_err().kind,
            Draw2DErrorKind::UnsupportedVersion
        );
    }

    #[test]
    fn aggregate_command_and_item_budgets_cannot_be_bypassed() {
        let path = Path2D {
            commands: vec![PathCommand::Move { to: [0.0; 2] }; MAX_PATH_COMMANDS],
        };
        let mut draw = Draw2D::new();
        draw.stroke(path.clone(), StrokeStyle::default()).unwrap();
        assert_eq!(
            draw.stroke(path, StrokeStyle::default()).unwrap_err().kind,
            Draw2DErrorKind::CapacityExceeded
        );
        assert_eq!(draw.finish().items().len(), 1);
        let mut draw = Draw2D::new();
        for _ in 0..MAX_DRAW_ITEMS {
            draw.stroke(Path2D::default(), StrokeStyle::default())
                .unwrap();
        }
        assert!(draw
            .stroke(Path2D::default(), StrokeStyle::default())
            .is_err());
    }

    #[test]
    fn builder_rejects_command_overflow_without_losing_valid_prefix() {
        let mut builder = PathBuilder::new();
        builder.move_to([0.0; 2]).unwrap();
        for _ in 1..MAX_PATH_COMMANDS {
            builder.line_to([0.0; 2]).unwrap();
        }
        let error = builder.close().unwrap_err();
        assert_eq!(error.kind, Draw2DErrorKind::CapacityExceeded);
        assert_eq!(error.index, MAX_PATH_COMMANDS);
        let path = builder.finish();
        assert_eq!(path.commands().len(), MAX_PATH_COMMANDS);
        path.validate().unwrap();
    }

    #[test]
    fn degenerate_values_are_valid_but_negative_nonfinite_and_unbounded_arcs_are_not() {
        let mut path = PathBuilder::new();
        path.move_to([0.0; 2])
            .unwrap()
            .line_to([0.0; 2])
            .unwrap()
            .arc([0.0; 2], [0.0; 2], 0.0, 0.0)
            .unwrap();
        assert!(path.arc([0.0; 2], [-1.0, 1.0], 0.0, 1.0).is_err());
        assert!(path.arc([0.0; 2], [1.0; 2], 0.0, 7.0).is_err());
        assert!(StrokeStyle {
            width: -1.0,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(StrokeStyle {
            miter_limit: f32::INFINITY,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Path2D::default().validate().is_ok());
    }
}
