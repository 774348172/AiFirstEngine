//! 371 coordinate/identity kernels. Not a Package schema or a live World adapter.
use super::{ContractResult, SpatialContractError};

#[cfg(test)]
std::thread_local! { static RELATIVE_PATH_FORMATS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
fn relative_path(axis: usize) -> String {
    #[cfg(test)]
    RELATIVE_PATH_FORMATS.with(|count| count.set(count.get() + 1));
    format!("relative[{axis}]")
}

/// Metres in the existing right-handed, Y-up world. Local displacement is kept
/// separate so adding it to a large global anchor cannot erase small motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPosition {
    anchor: [f64; 3],
    local: [f32; 3],
}

/// Explicit per-axis envelope for a conversion consumer. This bounds numeric
/// conversion only; a physics/navigation backend must qualify its own envelope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateDomain {
    max_anchor: f64,
    max_local: f64,
    max_relative: f64,
    max_error: f64,
}

impl CoordinateDomain {
    pub fn new(
        max_anchor: f64,
        max_local: f64,
        max_relative: f64,
        max_error: f64,
    ) -> ContractResult<Self> {
        for (path, value) in [
            ("domain.maxAnchor", max_anchor),
            ("domain.maxLocal", max_local),
            ("domain.maxRelative", max_relative),
            ("domain.maxError", max_error),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(SpatialContractError::new(
                    "coordinate.invalid_domain",
                    path,
                    "limit must be finite and positive",
                ));
            }
        }
        if max_relative > f64::from(f32::MAX) || max_local > f64::from(f32::MAX) {
            return Err(SpatialContractError::new(
                "coordinate.invalid_domain",
                "domain",
                "local/relative limit exceeds f32 range",
            ));
        }
        Ok(Self {
            max_anchor,
            max_local,
            max_relative,
            max_error,
        })
    }
}

impl WorldPosition {
    pub fn new(anchor: [f64; 3], local: [f32; 3]) -> ContractResult<Self> {
        for axis in 0..3 {
            if !anchor[axis].is_finite() || !local[axis].is_finite() {
                return Err(SpatialContractError::new(
                    "coordinate.non_finite",
                    &format!("position[{axis}]"),
                    "position must be finite",
                ));
            }
        }
        Ok(Self { anchor, local })
    }

    /// Subtract anchors in f64 FIRST, then local displacements, then narrow once.
    /// Returns an error instead of clamping, overflowing or silently losing precision.
    /// The budget includes conservative anchor representation/arithmetic error;
    /// it excludes upstream authoring and backend integration error.
    pub fn relative_to(self, origin: Self, domain: CoordinateDomain) -> ContractResult<[f32; 3]> {
        let mut result = [0.0; 3];
        for (axis, output) in result.iter_mut().enumerate() {
            if self.anchor[axis].abs() > domain.max_anchor
                || origin.anchor[axis].abs() > domain.max_anchor
                || f64::from(self.local[axis]).abs() > domain.max_local
                || f64::from(origin.local[axis]).abs() > domain.max_local
            {
                return Err(SpatialContractError::new(
                    "coordinate.out_of_range",
                    &relative_path(axis),
                    "anchor/local exceeds consumer envelope",
                ));
            }
            let anchor_delta = self.anchor[axis] - origin.anchor[axis];
            let local_delta = f64::from(self.local[axis]) - f64::from(origin.local[axis]);
            let relative = anchor_delta + local_delta;
            if !relative.is_finite() || relative.abs() > domain.max_relative {
                return Err(SpatialContractError::new(
                    "coordinate.out_of_range",
                    &relative_path(axis),
                    "relative position exceeds consumer envelope",
                ));
            }
            let narrowed = relative as f32;
            let arithmetic_bound = f64::EPSILON
                * (self.anchor[axis].abs()
                    + origin.anchor[axis].abs()
                    + anchor_delta.abs()
                    + f64::from(self.local[axis]).abs()
                    + f64::from(origin.local[axis]).abs()
                    + local_delta.abs()
                    + relative.abs());
            let error = (f64::from(narrowed) - relative).abs() + arithmetic_bound;
            if !error.is_finite() || error > domain.max_error {
                return Err(SpatialContractError::new(
                    "coordinate.precision_budget",
                    &relative_path(axis),
                    "conversion exceeds absolute error budget",
                ));
            }
            *output = if narrowed == 0.0 { 0.0 } else { narrowed };
        }
        Ok(result)
    }
}

/// Persistent identity is a tuple, never a bare Prefab asset/local entity id.
/// The placement/spawn key must be allocated by its owner and persisted for replay.
/// It does not resolve a runtime handle or bypass its generation/alive checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersistentEntityIdentity {
    world: String,
    instance: String,
    entity_local: String,
}

impl PersistentEntityIdentity {
    pub fn new(world: String, instance: String, entity_local: String) -> ContractResult<Self> {
        for (path, value) in [
            ("identity.world", &world),
            ("identity.instance", &instance),
            ("identity.entityLocal", &entity_local),
        ] {
            if value.trim().is_empty() {
                return Err(SpatialContractError::new(
                    "identity.empty",
                    path,
                    "identity segment must not be empty",
                ));
            }
        }
        Ok(Self {
            world,
            instance,
            entity_local,
        })
    }
}

#[cfg(test)]
mod allocation_regressions {
    use super::*;
    #[test]
    fn successful_coordinate_batches_do_not_format_error_paths() {
        let origin = WorldPosition::new([1e9, 0., 0.], [0.; 3]).unwrap();
        let domain = CoordinateDomain::new(1e12, 32768., 32768., 0.005).unwrap();
        let mut observed = Vec::new();
        for count in [1usize, 100, 1000, 10000] {
            RELATIVE_PATH_FORMATS.with(|n| n.set(0));
            for i in 0..count {
                let x = (i % 4096) as f32;
                let point = WorldPosition::new([1e9, 0., 0.], [x, 0., 0.]).unwrap();
                assert_eq!(point.relative_to(origin, domain).unwrap(), [x, 0., 0.]);
            }
            observed.push((count, RELATIVE_PATH_FORMATS.with(|n| n.get())));
        }
        eprintln!("actual successful coordinate error-path formats: {observed:?}");
        assert!(
            observed.iter().all(|(_, formats)| *formats == 0),
            "{observed:?}"
        );
    }
    #[test]
    fn coordinate_errors_keep_the_exact_failing_axis_and_precision_code() {
        let domain = CoordinateDomain::new(1e12, 32768., 32768., 0.005).unwrap();
        let origin = WorldPosition::new([0.; 3], [0.; 3]).unwrap();
        for axis in 0..3 {
            let mut p = [0.; 3];
            p[axis] = 32769.;
            let error = WorldPosition::new([0.; 3], p)
                .unwrap()
                .relative_to(origin, domain)
                .unwrap_err();
            assert_eq!(error.field_path, format!("relative[{axis}]"));
            assert_eq!(error.code, "three_d.coordinate.out_of_range");
        }
    }
}
