//! Shared 2D presentation math. This module does not submit runtime UI updates.
//! Coordinates are logical pixels, angles are radians and positive rotation is
//! clockwise on a screen whose Y axis points down. Composition is parent * local.

use serde::{Deserialize, Serialize};

pub const INVERSE_RELATIVE_EPSILON: f64 = 1.0e-8;
pub const DEFAULT_CURVE_ERROR_PIXELS: f32 = 0.25;

/// Base presentation values; animation and feedback compose after this layer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct NodePresentation {
    pub transform: Transform2D,
    pub opacity: f32,
    /// Linear normalized RGBA, multiplied after existing paint/texture decoding.
    pub tint: [f32; 4],
}

impl Default for NodePresentation {
    fn default() -> Self {
        Self {
            transform: Transform2D::default(),
            opacity: 1.0,
            tint: [1.0; 4],
        }
    }
}

impl NodePresentation {
    pub fn validate(&self) -> Result<(), PresentationError> {
        self.transform.validate()?;
        validate_rgba(self.tint)?;
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(PresentationError::InvalidValue("opacity"));
        }
        Ok(())
    }

    pub fn apply(&mut self, property: crate::aui::UiProperty, value: &crate::aui::UiValue) {
        use crate::aui::{UiProperty as P, UiValue as V};
        match (property, value) {
            (P::Translation, V::Translation(v)) => self.transform.translation = *v,
            (P::Rotation, V::Number(v)) => self.transform.rotation = *v,
            (P::Scale, V::Vector(v)) => self.transform.scale = *v,
            (P::Pivot, V::Vector(v)) => self.transform.pivot = *v,
            (P::Opacity, V::Number(v)) => self.opacity = *v,
            (P::Tint, V::Color(v)) => self.tint = *v,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationError {
    InvalidValue(&'static str),
    NonInvertibleTransform,
    UnrepresentableTransform,
}

impl std::fmt::Display for PresentationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PresentationError {}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transform2D {
    pub translation: [f32; 2],
    pub rotation: f32,
    pub scale: [f32; 2],
    /// Normalized within the layout size; finite values outside 0..1 are allowed.
    pub pivot: [f32; 2],
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            translation: [0.0; 2],
            rotation: 0.0,
            scale: [1.0; 2],
            pivot: [0.5; 2],
        }
    }
}

impl Transform2D {
    pub fn validate(&self) -> Result<(), PresentationError> {
        if !self
            .translation
            .iter()
            .chain(self.scale.iter())
            .chain(self.pivot.iter())
            .chain(std::iter::once(&self.rotation))
            .all(|value| value.is_finite())
        {
            return Err(PresentationError::InvalidValue("transform"));
        }
        Ok(())
    }

    /// Layout origin is relative to the parent, never an already accumulated screen origin.
    pub fn local_matrix(
        &self,
        layout_origin: [f32; 2],
        layout_size: [f32; 2],
    ) -> Result<Affine2D, PresentationError> {
        self.validate()?;
        if !layout_origin.iter().all(|v| v.is_finite())
            || !layout_size.iter().all(|v| v.is_finite() && *v >= 0.0)
        {
            return Err(PresentationError::InvalidValue("layout"));
        }
        let (sin, cos) = f64::from(self.rotation).sin_cos();
        let a = cos * f64::from(self.scale[0]);
        let b = sin * f64::from(self.scale[0]);
        let c = -sin * f64::from(self.scale[1]);
        let d = cos * f64::from(self.scale[1]);
        let px = f64::from(self.pivot[0]) * f64::from(layout_size[0]);
        let py = f64::from(self.pivot[1]) * f64::from(layout_size[1]);
        Affine2D::checked([
            a,
            b,
            c,
            d,
            f64::from(layout_origin[0]) + f64::from(self.translation[0]) + px - a * px - c * py,
            f64::from(layout_origin[1]) + f64::from(self.translation[1]) + py - b * px - d * py,
        ])
    }
}

/// Matrix [a,b,c,d,tx,ty]: x'=a*x+c*y+tx, y'=b*x+d*y+ty.
/// Higher precision intermediates avoid underflow in small-scale inverse queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2D([f64; 6]);

impl Default for Affine2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine2D {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn from_coefficients(values: [f32; 6]) -> Result<Self, PresentationError> {
        Self::checked(values.map(f64::from))
    }

    fn checked(values: [f64; 6]) -> Result<Self, PresentationError> {
        if values
            .iter()
            .all(|v| v.is_finite() && v.abs() <= f64::from(f32::MAX))
        {
            // Queries and rendering must use the same published f32 coefficients.
            Ok(Self(values.map(|value| f64::from(value as f32))))
        } else {
            Err(PresentationError::UnrepresentableTransform)
        }
    }

    pub fn coefficients(self) -> [f32; 6] {
        self.0.map(|v| v as f32)
    }

    /// Returns self * local. Changing this order changes nested layout and pointer semantics.
    pub fn compose(self, local: Self) -> Result<Self, PresentationError> {
        let [a, b, c, d, tx, ty] = self.0;
        let [la, lb, lc, ld, lx, ly] = local.0;
        Self::checked([
            a * la + c * lb,
            b * la + d * lb,
            a * lc + c * ld,
            b * lc + d * ld,
            a * lx + c * ly + tx,
            b * lx + d * ly + ty,
        ])
    }

    pub fn try_inverse(self) -> Result<Self, PresentationError> {
        let [a, b, c, d, tx, ty] = self.0;
        let norm = a.abs().max(b.abs()).max(c.abs()).max(d.abs());
        if norm == 0.0 {
            return Err(PresentationError::NonInvertibleTransform);
        }
        let determinant = a * d - b * c;
        if (determinant / norm / norm).abs() <= INVERSE_RELATIVE_EPSILON {
            return Err(PresentationError::NonInvertibleTransform);
        }
        Self::checked([
            d / determinant,
            -b / determinant,
            -c / determinant,
            a / determinant,
            (c * ty - d * tx) / determinant,
            (b * tx - a * ty) / determinant,
        ])
    }

    pub fn transform_point(self, point: [f32; 2]) -> Result<[f32; 2], PresentationError> {
        if !point.iter().all(|v| v.is_finite()) {
            return Err(PresentationError::InvalidValue("point"));
        }
        let [a, b, c, d, tx, ty] = self.0;
        let x = a * f64::from(point[0]) + c * f64::from(point[1]) + tx;
        let y = b * f64::from(point[0]) + d * f64::from(point[1]) + ty;
        if [x, y]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= f64::from(f32::MAX))
        {
            Ok([x as f32, y as f32])
        } else {
            Err(PresentationError::UnrepresentableTransform)
        }
    }

    /// Upper bound on scale, including shear from nested non-uniform transforms.
    pub fn maximum_scale(self) -> f64 {
        let [a, b, c, d, _, _] = self.0;
        let s1 = a * a + b * b;
        let s2 = c * c + d * d;
        let cross = a * c + b * d;
        ((s1 + s2 + ((s1 - s2).powi(2) + 4.0 * cross * cross).sqrt()) * 0.5).sqrt()
    }
}

/// Converts a screen-space error to local-space tolerance. A degenerate transform
/// is not tessellated; its consumer should cull it rather than invent a tolerance.
pub fn curve_tolerance(
    maximum_scale: f64,
    dpi: f32,
    error_pixels: f32,
) -> Result<f32, PresentationError> {
    if !maximum_scale.is_finite() || maximum_scale <= 0.0 {
        return Err(PresentationError::InvalidValue("maximum_scale"));
    }
    if !dpi.is_finite() || dpi <= 0.0 || !error_pixels.is_finite() || error_pixels <= 0.0 {
        return Err(PresentationError::InvalidValue("curve_precision"));
    }
    let tolerance = (f64::from(error_pixels) / maximum_scale / f64::from(dpi)) as f32;
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(PresentationError::InvalidValue("curve_tolerance"));
    }
    Ok(tolerance)
}

/// Decode one normalized sRGB paint channel to linear light. Alpha is not sRGB.
pub fn srgb_channel_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode one normalized linear channel for an sRGB authoring value or target.
pub fn linear_channel_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    }
}

/// Validate normalized RGBA without changing its space. Numeric paint and Tint
/// are linear; hexadecimal AUI style RGB is decoded by the renderer once.
pub fn validate_rgba(rgba: [f32; 4]) -> Result<(), PresentationError> {
    if rgba
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    {
        Ok(())
    } else {
        Err(PresentationError::InvalidValue("rgba"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(actual: [f32; 2], expected: [f32; 2]) {
        for i in 0..2 {
            assert!(
                (actual[i] - expected[i]).abs() < 0.0001,
                "{actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn pivot_rotation_and_mirror_round_trip() {
        let transform = Transform2D {
            translation: [3.0, 4.0],
            rotation: std::f32::consts::FRAC_PI_2,
            scale: [-2.0, 1.0],
            pivot: [0.5, 0.5],
        };
        let matrix = transform.local_matrix([10.0, 20.0], [8.0, 6.0]).unwrap();
        near(matrix.transform_point([4.0, 3.0]).unwrap(), [17.0, 27.0]);
        let point = [1.0, 5.0];
        near(
            matrix
                .try_inverse()
                .unwrap()
                .transform_point(matrix.transform_point(point).unwrap())
                .unwrap(),
            point,
        );
    }

    #[test]
    fn nested_parent_rotation_rotates_child_origin_once() {
        let parent = Transform2D {
            rotation: std::f32::consts::FRAC_PI_2,
            pivot: [0.0; 2],
            ..Default::default()
        };
        let parent = parent.local_matrix([100.0, 50.0], [20.0; 2]).unwrap();
        let child = Transform2D::default()
            .local_matrix([10.0, 0.0], [5.0; 2])
            .unwrap();
        near(
            parent
                .compose(child)
                .unwrap()
                .transform_point([0.0; 2])
                .unwrap(),
            [100.0, 60.0],
        );
    }

    #[test]
    fn small_uniform_scale_is_invertible_but_zero_and_ill_conditioned_are_not() {
        let matrix = Affine2D::from_coefficients([1e-20, 0.0, 0.0, 1e-20, 0.0, 0.0]).unwrap();
        assert!(matrix.try_inverse().is_ok());
        for scale in [[0.0, 1.0], [1.0, 1e-10]] {
            let matrix = Transform2D {
                scale,
                ..Default::default()
            }
            .local_matrix([0.0; 2], [0.0; 2])
            .unwrap();
            assert_eq!(
                matrix.try_inverse(),
                Err(PresentationError::NonInvertibleTransform)
            );
        }
    }

    #[test]
    fn matrix_and_point_overflow_are_errors() {
        let huge = Affine2D::from_coefficients([f32::MAX, 0.0, 0.0, 1.0, 0.0, 0.0]).unwrap();
        assert_eq!(
            huge.compose(huge),
            Err(PresentationError::UnrepresentableTransform)
        );
        assert_eq!(
            huge.transform_point([2.0, 0.0]),
            Err(PresentationError::UnrepresentableTransform)
        );
        assert!(Transform2D {
            rotation: f32::NAN,
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn shear_precision_uses_largest_singular_value_and_dpi() {
        let matrix = Affine2D::from_coefficients([1.0, 0.0, 1.0, 1.0, 0.0, 0.0]).unwrap();
        assert!((matrix.maximum_scale() - 1.61803398875).abs() < 1e-9);
        assert_eq!(curve_tolerance(2.0, 2.0, 0.25).unwrap(), 0.0625);
        assert!(curve_tolerance(0.0, 1.0, 0.25).is_err());
        assert!(validate_rgba([0.5, 0.5, 0.5, 2.0]).is_err());
    }
}
