//! Pure axis-aligned kinematic motion; no world ownership, gravity, or gameplay policy.
//! Callers supply world-space rectangles and requested displacement. X is resolved before Y.

use crate::{GameError, GameErrorCode, GameResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb2 {
    pub center: [f32; 2],
    pub half: [f32; 2],
}

impl Aabb2 {
    pub fn overlaps(self, other: Self) -> bool {
        (0..2).all(|axis| {
            (self.center[axis] - other.center[axis]).abs() < self.half[axis] + other.half[axis]
        })
    }

    fn valid(self) -> bool {
        self.center.iter().all(|v| v.is_finite())
            && self.half.iter().all(|v| v.is_finite() && *v > 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion2 {
    pub center: [f32; 2],
    pub blocked: [bool; 2],
    /// Index in the supplied solid slice, when downward motion met a supporting surface.
    pub floor: Option<usize>,
}

/// Sweeps a non-penetrating body along each axis, then slides on the blocked surface.
/// Initial penetration recovery, slopes, and dynamic rigid-body impulses are outside this helper.
pub fn slide_aabb(mut body: Aabb2, delta: [f32; 2], solids: &[Aabb2]) -> GameResult<Motion2> {
    if !body.valid() || delta.iter().any(|v| !v.is_finite()) || solids.iter().any(|s| !s.valid()) {
        return Err(GameError::new(
            GameErrorCode::InvalidArgument,
            "Invalid kinematic rectangle or displacement",
        ));
    }
    let mut result = Motion2 {
        center: body.center,
        blocked: [false; 2],
        floor: None,
    };
    for axis in 0..2 {
        let other = 1 - axis;
        let mut distance = delta[axis];
        let mut contact = None;
        for (index, solid) in solids.iter().enumerate() {
            if (body.center[other] - solid.center[other]).abs()
                >= body.half[other] + solid.half[other] - 0.00001
            {
                continue;
            }
            let gap = if delta[axis] > 0.0 {
                solid.center[axis] - solid.half[axis] - body.center[axis] - body.half[axis]
            } else {
                solid.center[axis] + solid.half[axis] - body.center[axis] + body.half[axis]
            };
            let hit = if delta[axis] > 0.0 {
                gap >= -0.00001 && gap <= distance
            } else {
                delta[axis] < 0.0 && gap <= 0.00001 && gap >= distance
            };
            if hit {
                distance = if delta[axis] > 0.0 {
                    gap.max(0.0)
                } else {
                    gap.min(0.0)
                };
                result.blocked[axis] = true;
                contact = Some(
                    solid.center[axis]
                        - delta[axis].signum() * (solid.half[axis] + body.half[axis]),
                );
                if axis == 1 && delta[axis] < 0.0 {
                    result.floor = Some(index);
                }
            }
        }
        body.center[axis] = contact.unwrap_or(body.center[axis] + distance);
    }
    result.center = body.center;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(x: f32, y: f32, hx: f32, hy: f32) -> Aabb2 {
        Aabb2 {
            center: [x, y],
            half: [hx, hy],
        }
    }

    #[test]
    fn fast_motion_stops_at_nearest_wall_and_slides() {
        let result = slide_aabb(
            rect(0.0, 0.0, 0.5, 0.5),
            [50.0, 1.0],
            &[rect(8.0, 0.0, 0.5, 5.0), rect(3.0, 0.0, 0.5, 5.0)],
        )
        .unwrap();
        assert_eq!(result.center, [2.0, 1.0]);
        assert_eq!(result.blocked, [true, false]);
    }

    #[test]
    fn repeated_gravity_is_stable_and_ceiling_is_not_a_floor() {
        let solids = [rect(0.0, -1.0, 5.0, 0.5), rect(0.0, 3.0, 5.0, 0.5)];
        let mut body = rect(0.0, 1.0, 0.5, 0.5);
        for _ in 0..600 {
            let step = slide_aabb(body, [0.0, -0.2], &solids).unwrap();
            body.center = step.center;
        }
        assert_eq!(body.center, [0.0, 0.0]);
        assert_eq!(
            slide_aabb(body, [0.0, -0.1], &solids).unwrap().floor,
            Some(0)
        );
        let upward = slide_aabb(body, [0.0, 20.0], &solids).unwrap();
        assert_eq!(upward.center, [0.0, 2.0]);
        assert_eq!(upward.floor, None);
    }

    #[test]
    fn mirrored_motion_and_invalid_inputs() {
        assert_eq!(
            slide_aabb(
                rect(0.0, 0.0, 0.5, 0.5),
                [-10.0, 0.0],
                &[rect(-3.0, 0.0, 0.5, 2.0)]
            )
            .unwrap()
            .center,
            [-2.0, 0.0]
        );
        assert!(slide_aabb(rect(0.0, 0.0, 0.5, 0.5), [f32::NAN, 0.0], &[]).is_err());
        assert!(slide_aabb(rect(0.0, 0.0, -1.0, 0.5), [1.0, 0.0], &[]).is_err());
    }
}
