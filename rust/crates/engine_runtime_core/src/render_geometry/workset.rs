//! Snapshot-owned spatial facts. Uncertain interval tests keep the original culler.
use super::*;

#[cfg(test)]
thread_local! {
    static DYNAMIC_VIEW_FINITE_COMPONENTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static SHADOW_WORLD_TRANSFORMS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static RECEIVER_CAMERA_COMPONENTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SPATIAL_BOUND_DOTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SPATIAL_INTERVAL_PRODUCTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn world_corner(matrix: [f32; 16], point: [f32; 4]) -> [f32; 4] {
    #[cfg(test)]
    SHADOW_WORLD_TRANSFORMS.with(|n| n.set(n.get() + 1));
    transform_point(matrix, point)
}

#[cfg(test)]
pub(super) fn camera_corner(matrix: [f32; 16], point: [f32; 4]) -> [f32; 4] {
    #[cfg(test)]
    RECEIVER_CAMERA_COMPONENTS.with(|n| n.set(n.get() + 4));
    transform_point(matrix, point)
}

/// Pure world-space facts owned by the existing immutable snapshot workset.
/// No view result, native handle, resource admission, or lease is cached here.
#[derive(Debug)]
pub(super) struct ReceiverBounds {
    // Component-major corners permit the same ordered dot products to execute
    // across all eight corners without strided scalar traversal.
    corners: [[f32; 8]; 4],
    magnitude: [f32; 4],
}

impl ReceiverBounds {
    pub(super) fn new(world: [f32; 16], bounds: &GeometryBounds) -> Option<Self> {
        if world.iter().any(|v| !v.is_finite())
            || (0..3).any(|i| {
                !bounds.min[i].is_finite()
                    || !bounds.max[i].is_finite()
                    || bounds.min[i] > bounds.max[i]
            })
        {
            return None;
        }
        let local = [
            bounds.min[0].abs().max(bounds.max[0].abs()),
            bounds.min[1].abs().max(bounds.max[1].abs()),
            bounds.min[2].abs().max(bounds.max[2].abs()),
            1.,
        ];
        let magnitude = absolute_rows(world, local)?;
        let corners: [[f32; 4]; 8] = std::array::from_fn(|i| {
            let point: [f32; 4] = std::array::from_fn(|axis| {
                if axis == 3 {
                    1.
                } else if i & (1 << axis) == 0 {
                    bounds.min[axis]
                } else {
                    bounds.max[axis]
                }
            });
            world_corner(world, point)
        });
        if corners.iter().flatten().any(|v| !v.is_finite()) {
            return None;
        }
        Some(Self {
            corners: std::array::from_fn(|axis| corners.map(|p| p[axis])),
            magnitude,
        })
    }

    pub(super) fn shadow_corners(&self) -> [[f32; 3]; 8] {
        std::array::from_fn(|i| [self.corners[0][i], self.corners[1][i], self.corners[2][i]])
    }

    #[cfg(test)]
    pub(super) fn depth_range(&self, camera: [f32; 16]) -> Option<[f32; 2]> {
        self.depth_range_in(&ReceiverView::new(camera))
    }
    pub(super) fn depth_range_in(&self, current: &ReceiverView) -> Option<[f32; 2]> {
        let camera = current.camera;
        // All components of every camera corner are bounded, including those
        // not used as depth. Preserve the old overflow/error guard and f32 sum
        // order; an uncertain extreme returns unknown, never an empty receiver.
        if !current.finite {
            return None;
        }
        let camera_max = absolute_rows(camera, self.magnitude)?;
        let magnitude = self
            .magnitude
            .into_iter()
            .chain(camera_max)
            .fold(1f32, f32::max);
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let depths = receiver_depths(camera, &self.corners);
        for value in depths {
            #[cfg(test)]
            RECEIVER_CAMERA_COMPONENTS.with(|n| n.set(n.get() + 1));
            let depth = -value;
            if !depth.is_finite() {
                return None;
            }
            lo = lo.min(depth);
            hi = hi.max(depth);
        }
        // Positive absolute sums dominate all original signed corner sums
        // under monotone f32 addition. This is the same outward guard as before.
        let guard = 0.001 + 64. * f32::EPSILON * magnitude;
        ((lo - guard).is_finite() && (hi + guard).is_finite()).then_some([lo - guard, hi + guard])
    }
}

fn absolute_rows(matrix: [f32; 16], point: [f32; 4]) -> Option<[f32; 4]> {
    let rows = std::array::from_fn(|r| {
        (0..4)
            .map(|c| (matrix[c * 4 + r] * point[c]).abs())
            .sum::<f32>()
    });
    rows.iter().all(|v| v.is_finite()).then_some(rows)
}

#[derive(Debug)]
pub(super) struct SpatialBounds {
    min: [f64; 3],
    max: [f64; 3],
    magnitude: [f64; 4],
    matrix_magnitude: [f64; 4],
    valid: bool,
    receiver: Option<ReceiverBounds>,
}

pub(super) struct SpatialView {
    pub(super) matrix: [f32; 16],
    finite: bool,
    planes: [[f64; 4]; 6],
    upper_is_max: [[bool; 3]; 6],
    absolute: [[f64; 4]; 4],
    envelope: Option<ViewErrorBounds>,
}

#[derive(Clone, Copy)]
struct ViewErrorBounds {
    guards: [f64; 3],
    tolerance: f64,
}

#[derive(Debug, Default)]
pub(super) struct SpatialEnvelope {
    magnitude: [f64; 4],
    matrix_magnitude: [f64; 4],
}

impl SpatialEnvelope {
    pub(super) fn new(bounds: &[SpatialBounds]) -> Self {
        let mut result = Self::default();
        for bound in bounds {
            for axis in 0..4 {
                // Invalid rows cannot authorize an overflow shortcut.
                let upper = |v: f64| if v.is_finite() { v } else { f64::INFINITY };
                result.magnitude[axis] = result.magnitude[axis].max(upper(bound.magnitude[axis]));
                result.matrix_magnitude[axis] =
                    result.matrix_magnitude[axis].max(upper(bound.matrix_magnitude[axis]));
            }
        }
        result
    }
}

impl SpatialView {
    pub(super) fn with_envelope(matrix: [f32; 16], envelope: &SpatialEnvelope) -> Self {
        let mut view = Self::new(matrix);
        view.envelope = view.error_bounds(&envelope.magnitude, &envelope.matrix_magnitude);
        view
    }
    pub(super) fn new(matrix: [f32; 16]) -> Self {
        let row = |r: usize| std::array::from_fn::<_, 4, _>(|c| f64::from(matrix[c * 4 + r]));
        let rows = [row(0), row(1), row(2), row(3)];
        let planes: [[f64; 4]; 6] = std::array::from_fn(|p| {
            std::array::from_fn(|c| match p {
                0 => rows[3][c] + rows[0][c],
                1 => rows[3][c] - rows[0][c],
                2 => rows[3][c] + rows[1][c],
                3 => rows[3][c] - rows[1][c],
                4 => rows[2][c],
                _ => rows[3][c] - rows[2][c],
            })
        });
        Self {
            matrix,
            finite: current_matrix_finite(matrix),
            upper_is_max: planes.map(|plane| std::array::from_fn(|axis| plane[axis] >= 0.)),
            planes,
            absolute: rows.map(|r| r.map(f64::abs)),
            envelope: None,
        }
    }

    fn error_bounds(
        &self,
        magnitude: &[f64; 4],
        matrix_magnitude: &[f64; 4],
    ) -> Option<ViewErrorBounds> {
        if !self.finite {
            return None;
        }
        let dot_abs = |a: &[f64; 4], b: &[f64; 4]| {
            #[cfg(test)]
            SPATIAL_BOUND_DOTS.with(|n| n.set(n.get() + 1));
            (0..4).map(|i| a[i] * b[i]).sum::<f64>()
        };
        let magnitude = self.absolute.map(|r| dot_abs(&r, magnitude));
        let safe_limit = f64::from(f32::MAX) / 32.;
        if self
            .absolute
            .iter()
            .any(|r| dot_abs(r, matrix_magnitude) >= safe_limit)
            || magnitude.iter().any(|v| !v.is_finite() || *v >= safe_limit)
        {
            return None;
        }
        let error_factor = 128. * f64::from(f32::EPSILON);
        Some(ViewErrorBounds {
            guards: std::array::from_fn(|axis| {
                error_factor * (magnitude[axis] + magnitude[3] + 1.)
            }),
            tolerance: 1e-5 * (magnitude[3] * (1. + error_factor)).max(1.),
        })
    }
}

impl SpatialBounds {
    pub(super) fn new(world: [f32; 16], bounds: &GeometryBounds) -> Self {
        let bounds_valid = (0..3).all(|i| {
            bounds.min[i].is_finite() && bounds.max[i].is_finite() && bounds.min[i] <= bounds.max[i]
        });
        let local: [f64; 4] = [
            f64::from(bounds.min[0].abs().max(bounds.max[0].abs())),
            f64::from(bounds.min[1].abs().max(bounds.max[1].abs())),
            f64::from(bounds.min[2].abs().max(bounds.max[2].abs())),
            1.,
        ];
        let magnitude = std::array::from_fn(|r| {
            (0..4)
                .map(|c| f64::from(world[c * 4 + r]).abs() * local[c])
                .sum()
        });
        let matrix_magnitude = std::array::from_fn(|r| {
            (0..4)
                .map(|c| f64::from(world[c * 4 + r]).abs())
                .fold(0., f64::max)
        });
        let bound = |r: usize, upper: bool| {
            f64::from(world[12 + r])
                + (0..3)
                    .map(|c| {
                        let a = f64::from(world[c * 4 + r]) * f64::from(bounds.min[c]);
                        let b = f64::from(world[c * 4 + r]) * f64::from(bounds.max[c]);
                        if upper {
                            a.max(b)
                        } else {
                            a.min(b)
                        }
                    })
                    .sum::<f64>()
        };
        Self {
            min: std::array::from_fn(|r| bound(r, false)),
            max: std::array::from_fn(|r| bound(r, true)),
            magnitude,
            matrix_magnitude,
            valid: bounds_valid
                && world.iter().all(|v| v.is_finite())
                && world[3] == 0.
                && world[7] == 0.
                && world[11] == 0.
                && world[15] == 1.,
            receiver: ReceiverBounds::new(world, bounds),
        }
    }

    pub(super) fn receiver(&self) -> Option<&ReceiverBounds> {
        self.receiver.as_ref()
    }

    /// Some means a robust proof; None must execute the legacy eight-corner path.
    pub(super) fn classify(&self, view: &SpatialView) -> Option<bool> {
        if !self.valid || !view.finite {
            return None;
        }
        // Bound every intermediate of the original f32 matrix product and corner
        // evaluation, including unused matrix columns. Cancellation cannot hide overflow.
        // The snapshot envelope dominates every row by positive arithmetic.
        // If it is too large, keep the original per-row proof, then strict culler.
        let errors = view
            .envelope
            .or_else(|| view.error_bounds(&self.magnitude, &self.matrix_magnitude))?;
        // Dominates the original two f32 dot products, plane sum and f64 AABB
        // evaluation error, even with cancellation/shear. Near boundaries we fall back.
        let mut wholly_inside = true;
        for (p, plane) in view.planes.iter().enumerate() {
            let axis = match p {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => 2,
            };
            let guard = errors.guards[axis];
            let mut hi = plane[3];
            for (axis, coefficient) in plane[..3].iter().copied().enumerate() {
                #[cfg(test)]
                SPATIAL_INTERVAL_PRODUCTS.with(|n| n.set(n.get() + 1));
                hi += coefficient
                    * if view.upper_is_max[p][axis] {
                        self.max[axis]
                    } else {
                        self.min[axis]
                    };
            }
            if !hi.is_finite() {
                return None;
            }
            if hi < -guard - errors.tolerance {
                return Some(true);
            }
            if wholly_inside {
                // Preserve the original axis sum order. Finite f32 inputs bound
                // these f64 products below 1e117, so delaying the lower support
                // cannot hide an overflow. The existing guard still owns proof.
                let mut lo = plane[3];
                for (axis, coefficient) in plane[..3].iter().copied().enumerate() {
                    #[cfg(test)]
                    SPATIAL_INTERVAL_PRODUCTS.with(|n| n.set(n.get() + 1));
                    lo += coefficient
                        * if view.upper_is_max[p][axis] {
                            self.min[axis]
                        } else {
                            self.max[axis]
                        };
                }
                if !lo.is_finite() {
                    return None;
                }
                wholly_inside = lo >= guard;
            }
        }
        wholly_inside.then_some(false)
    }
}

#[cfg(test)]
impl SpatialBounds {
    fn original_classify(&self, view: &SpatialView) -> Option<bool> {
        if !self.valid || !view.finite {
            return None;
        }
        // Bound every intermediate of the original f32 matrix product and corner
        // evaluation, including unused matrix columns. Cancellation cannot hide overflow.
        // The snapshot envelope dominates every row by positive arithmetic.
        // If it is too large, keep the original per-row proof, then strict culler.
        let errors = view
            .envelope
            .or_else(|| view.error_bounds(&self.magnitude, &self.matrix_magnitude))?;
        // Dominates the original two f32 dot products, plane sum and f64 AABB
        // evaluation error, even with cancellation/shear. Near boundaries we fall back.
        let mut wholly_inside = true;
        for (p, plane) in view.planes.iter().enumerate() {
            let axis = match p {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => 2,
            };
            let guard = errors.guards[axis];
            let mut lo = plane[3];
            let mut hi = plane[3];
            for (axis, coefficient) in plane[..3].iter().copied().enumerate() {
                let a = coefficient * self.min[axis];
                let b = coefficient * self.max[axis];
                lo += a.min(b);
                hi += a.max(b);
            }
            if !lo.is_finite() || !hi.is_finite() {
                return None;
            }
            if hi < -guard - errors.tolerance {
                return Some(true);
            }
            wholly_inside &= lo >= guard;
        }
        wholly_inside.then_some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_support_matches_original_interval_decisions_including_extremes() {
        let (_, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let projection = view
            .evaluate(CameraPose::default())
            .unwrap()
            .view_projection();
        let mut seed = 371u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32
        };
        for i in 0..4096 {
            let mut world = IDENTITY_MATRIX;
            for c in [0, 1, 2, 4, 5, 6, 8, 9, 10] {
                world[c] = next() * 8.;
            }
            world[12] = next() * 500.;
            world[13] = next() * 200.;
            world[14] = next() * 2000.;
            if i % 7 == 0 {
                world[i % 3] = -0.;
            }
            if i % 13 == 0 {
                world[0] = f32::from_bits(1);
            }
            if i % 37 == 0 {
                world[0] = f32::MAX;
            }
            if i % 113 == 0 {
                world[3] = 0.01;
            }
            let bound = SpatialBounds::new(world, &source.bounds);
            for matrix in [IDENTITY_MATRIX, projection] {
                let current = SpatialView::new(matrix);
                assert_eq!(
                    bound.classify(&current),
                    bound.original_classify(&current),
                    "case={i}"
                );
                let with_envelope = SpatialView::with_envelope(
                    matrix,
                    &SpatialEnvelope::new(std::slice::from_ref(&bound)),
                );
                assert_eq!(
                    bound.classify(&with_envelope),
                    bound.original_classify(&with_envelope),
                    "envelope case={i}"
                );
            }
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, -0.] {
            let mut matrix = IDENTITY_MATRIX;
            matrix[0] = value;
            let current = SpatialView::new(matrix);
            let bound = SpatialBounds::new(IDENTITY_MATRIX, &source.bounds);
            assert_eq!(bound.classify(&current), bound.original_classify(&current));
        }
    }

    #[test]
    fn flow_spatial_rejection_does_not_compute_an_unused_lower_support() {
        let (_, source, _, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let view = SpatialView::new(IDENTITY_MATRIX);
        let mut world = IDENTITY_MATRIX;
        world[12] = -100.;
        let bound = SpatialBounds::new(world, &source.bounds);
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            SPATIAL_INTERVAL_PRODUCTS.with(|n| n.set(0));
            for _ in 0..count {
                assert_eq!(bound.classify(&view), Some(true));
            }
            actual.push((count, SPATIAL_INTERVAL_PRODUCTS.with(|n| n.get())));
        }
        eprintln!("actual support products: {actual:?}");
        assert!(actual.iter().all(|(rows, products)| *products == rows * 3));
    }

    #[test]
    fn persistent_bounds_match_legacy_culling_for_affine_views_and_boundary_fallbacks() {
        let (_, source, mut view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let mut seed = 371u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as u32 as f64 / f64::from(u32::MAX)) as f32
        };
        let mut fast = [0usize; 2];
        let mut fallback = 0;
        for orthographic in [false, true] {
            if orthographic {
                view.projection = Projection::Orthographic {
                    size: 400.,
                    near: 0.1,
                    far: 2000.,
                };
            }
            let evaluated = view.evaluate(CameraPose::default()).unwrap();
            let matrix = evaluated.view_projection();
            let planes = SpatialView::new(matrix);
            for i in 0..8192 {
                let mut world = IDENTITY_MATRIX;
                world[0] = (next() * 10. + 0.01) * if i & 1 == 0 { 1. } else { -1. };
                world[5] = next() * 4.;
                world[10] = next() * 6.;
                world[4] = next() * 3. - 1.5;
                world[9] = next() * 2. - 1.;
                world[12] = next() * 2000. - 1000.;
                world[13] = next() * 1000. - 500.;
                world[14] = -next() * 2200.;
                let expected = outside_frustum(matrix, world, &source.bounds).unwrap();
                let facts = SpatialBounds::new(world, &source.bounds);
                let actual = match facts.classify(&planes) {
                    Some(v) => {
                        fast[usize::from(v)] += 1;
                        v
                    }
                    None => {
                        fallback += 1;
                        outside_frustum(matrix, world, &source.bounds).unwrap()
                    }
                };
                assert_eq!(
                    actual, expected,
                    "view={orthographic} case={i} world={world:?}"
                );
            }
        }
        assert!(
            fast[0] > 100 && fast[1] > 100,
            "missing inside/outside fast paths: {fast:?}"
        );
        assert!(fallback > 0, "boundary cases must retain the old culler");
    }

    #[test]
    fn invalid_bounds_and_overflow_risk_cannot_bypass_original_error_path() {
        let (_, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let matrix = SpatialView::new(
            view.evaluate(CameraPose::default())
                .unwrap()
                .view_projection(),
        );
        let mut world = IDENTITY_MATRIX;
        world[0] = f32::MAX;
        assert!(SpatialBounds::new(world, &source.bounds)
            .classify(&matrix)
            .is_none());
        let mut bounds = source.bounds.clone();
        bounds.min[0] = f32::NAN;
        assert!(SpatialBounds::new(IDENTITY_MATRIX, &bounds)
            .classify(&matrix)
            .is_none());
        world = IDENTITY_MATRIX;
        world[3] = 0.01;
        assert!(SpatialBounds::new(world, &source.bounds)
            .classify(&matrix)
            .is_none());
    }
}

fn current_matrix_finite(matrix: [f32; 16]) -> bool {
    matrix.iter().all(|v| {
        #[cfg(test)]
        DYNAMIC_VIEW_FINITE_COMPONENTS.with(|n| n.set(n.get() + 1));
        v.is_finite()
    })
}
#[cfg(test)]
mod dynamic_consumption_tests {
    use super::*;
    #[test]
    fn execution_chain_production_consumes_envelope_and_matches_strict_selection() {
        let (manager, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        for count in [4, 32, 256] {
            let instances: Vec<_> = (0..count)
                .map(|i| GeometryInstance {
                    instance_id: i as u64,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[12] = i as f32 - count as f32 / 2.;
                        m[14] = -100.;
                        m
                    },
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(true),
                    },
                    layers: LayerSelection::Named { names: vec![] },
                    provenance: source.geometry_source.as_ref().clone(),
                })
                .collect();
            let prepared;
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs.enable_compact().unwrap();
            let views = [GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }];
            let strict = inputs
                .build("surface-main", [64, 64], &views, DiagnosticLevel::Off)
                .unwrap();
            prepared = inputs.freeze();
            inputs.prepared = Some(&prepared);
            SPATIAL_BOUND_DOTS.with(|n| n.set(0));
            let current = inputs
                .build("surface-main", [64, 64], &views, DiagnosticLevel::Off)
                .unwrap();
            assert_eq!(
                SPATIAL_BOUND_DOTS.with(|n| n.get()),
                8,
                "production did not consume current-view envelope"
            );
            assert_eq!(
                current.views[0].selected_draws().collect::<Vec<_>>(),
                strict.views[0].selected_draws().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn execution_chain_envelope_preserves_moving_view_boundary_and_extreme_fallbacks() {
        let (_, source, mut view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let worlds: Vec<_> = (0..1024)
            .map(|i| {
                let mut m = IDENTITY_MATRIX;
                m[0] = if i % 2 == 0 { -1. } else { 1. };
                m[4] = (i % 9) as f32 * 0.13;
                m[12] = (i % 37) as f32 * 17. - 300.;
                m[13] = (i % 23) as f32 * 13. - 150.;
                m[14] = -(i as f32 * 2. + 0.1);
                m
            })
            .collect();
        let bounds: Vec<_> = worlds
            .iter()
            .map(|w| SpatialBounds::new(*w, &source.bounds))
            .collect();
        let envelope = SpatialEnvelope::new(&bounds);
        let mut fast = [0; 2];
        let mut fallback = 0;
        for orthographic in [false, true] {
            if orthographic {
                view.projection = Projection::Orthographic {
                    size: 400.,
                    near: 0.1,
                    far: 2000.,
                };
            }
            for x in [0., 37., -120.] {
                let matrix = view
                    .evaluate(CameraPose {
                        position: [x, 0., 0.],
                        ..CameraPose::default()
                    })
                    .unwrap()
                    .view_projection();
                let current = SpatialView::with_envelope(matrix, &envelope);
                for (world, bound) in worlds.iter().zip(&bounds) {
                    let expected = outside_frustum(matrix, *world, &source.bounds).unwrap();
                    if let Some(actual) = bound.classify(&current) {
                        fast[usize::from(actual)] += 1;
                        assert_eq!(
                            actual, expected,
                            "view={orthographic} x={x} world={world:?}"
                        );
                    } else {
                        fallback += 1;
                    }
                }
            }
        }
        assert!(
            fast[0] > 100 && fast[1] > 100 && fallback > 0,
            "missing proof/fallback coverage {fast:?}/{fallback}"
        );
        let mut extreme = IDENTITY_MATRIX;
        extreme[0] = f32::MAX;
        let bounds = [
            SpatialBounds::new(extreme, &source.bounds),
            SpatialBounds::new(IDENTITY_MATRIX, &source.bounds),
        ];
        // Force an actual f32 product overflow. An orthographic projection's
        // small coefficient can safely scale even f32::MAX, so magnitude alone
        // is not a sufficient reason to require fallback.
        let mut matrix = IDENTITY_MATRIX;
        matrix[0] = 32.;
        let current = SpatialView::with_envelope(matrix, &SpatialEnvelope::new(&bounds));
        assert!(
            current.envelope.is_none(),
            "extreme global bound was admitted"
        );
        assert!(bounds[0].classify(&current).is_none());
        assert_eq!(
            bounds[1].classify(&current),
            bounds[1].classify(&SpatialView::new(matrix))
        );
    }
    #[test]
    fn execution_chain_spatial_envelope_bounds_current_view_work_at_multiple_scales() {
        let (_, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let matrix = view
            .evaluate(CameraPose::default())
            .unwrap()
            .view_projection();
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let bounds: Vec<_> = (0..count)
                .map(|i| {
                    let mut world = IDENTITY_MATRIX;
                    world[12] = i as f32 - count as f32 / 2.;
                    world[14] = -100.;
                    SpatialBounds::new(world, &source.bounds)
                })
                .collect();
            let envelope = SpatialEnvelope::new(&bounds);
            SPATIAL_BOUND_DOTS.with(|n| n.set(0));
            let current = SpatialView::with_envelope(matrix, &envelope);
            for bound in &bounds {
                let _ = bound.classify(&current);
            }
            actual.push((count, SPATIAL_BOUND_DOTS.with(|n| n.get())));
        }
        assert!(
            actual.iter().all(|&(_, n)| n <= 8),
            "per-row view bounds {actual:?}"
        );
    }
    #[test]
    fn dynamic_spatial_view_checks_current_matrix_once_at_multiple_scales() {
        let (_, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let matrix = view
            .evaluate(CameraPose::default())
            .unwrap()
            .view_projection();
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            DYNAMIC_VIEW_FINITE_COMPONENTS.with(|n| n.set(0));
            let spatial = SpatialView::new(matrix);
            for _ in 0..count {
                let mut world = IDENTITY_MATRIX;
                world[14] = -100.;
                let _ = SpatialBounds::new(world, &source.bounds).classify(&spatial);
            }
            rows.push((count, DYNAMIC_VIEW_FINITE_COMPONENTS.with(|n| n.get())));
        }
        eprintln!("dynamic spatial actual finite components {rows:?}");
        assert!(
            rows.iter().all(|&(_, n)| n == 16),
            "per-candidate view scan {rows:?}"
        );
    }
}

#[cfg(test)]
thread_local! {pub(super) static RECEIVER_FINITE_COMPONENTS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
fn receiver_matrix_finite(camera: [f32; 16]) -> bool {
    camera.iter().all(|v| {
        #[cfg(test)]
        RECEIVER_FINITE_COMPONENTS.with(|n| n.set(n.get() + 1));
        v.is_finite()
    })
}

/// Call-local current-camera facts, never a retained visible/depth result.
pub(super) struct ReceiverView {
    camera: [f32; 16],
    finite: bool,
}
impl ReceiverView {
    pub(super) fn new(camera: [f32; 16]) -> Self {
        Self {
            camera,
            finite: receiver_matrix_finite(camera),
        }
    }
}

// The standard float Iterator::sum starts at -0.0. Preserve its exact prefix
// order (including signed zero) while evaluating independent corner lanes.
fn receiver_depths(camera: [f32; 16], corners: &[[f32; 8]; 4]) -> [f32; 8] {
    let mut depths = [-0.0f32; 8];
    for (axis, values) in corners.iter().enumerate() {
        for (depth, value) in depths.iter_mut().zip(values) {
            *depth += camera[axis * 4 + 2] * value;
        }
    }
    depths
}
#[cfg(test)]
mod receiver_numeric_tests {
    use super::*;
    #[test]
    fn dynamic_consumption_receiver_lanes_preserve_scalar_sum_bits_and_signed_zero() {
        let mut seed = 371u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32
        };
        for case in 0..4096 {
            let scale = [0., 1e-30, 1., 1e10, 1e30, f32::MAX][case % 6];
            let mut camera = std::array::from_fn(|_| next() * scale);
            let mut corners = std::array::from_fn(|_| std::array::from_fn(|_| next() * scale));
            if case == 0 {
                camera = [1.; 16];
                corners = [[-0.0; 8]; 4];
            }
            if case == 1 {
                camera = [-0.0; 16];
                corners = [[1.; 8]; 4];
            }
            let actual = receiver_depths(camera, &corners);
            for lane in 0..8 {
                let expected = (0..4)
                    .map(|axis| camera[axis * 4 + 2] * corners[axis][lane])
                    .sum::<f32>();
                if expected.is_nan() {
                    assert!(actual[lane].is_nan());
                } else {
                    assert_eq!(
                        actual[lane].to_bits(),
                        expected.to_bits(),
                        "case={case} lane={lane}"
                    );
                }
            }
        }
    }
}
