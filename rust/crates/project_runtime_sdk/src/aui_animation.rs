//! Typed animation descriptions and clock math. Runtime playback, property ownership
//! and completion dispatch are supplied by the AUI host, not by these authoring values.

use crate::aui::UiNodeHandle;
use crate::aui_presentation::validate_rgba;
use serde::{Deserialize, Serialize};

pub const ANIMATION_2D_VERSION: u32 = 1;
pub const MAX_ANIMATION_STEPS: usize = 256;
pub const MAX_ANIMATION_DEPTH: usize = 32;
pub const MAX_PRESENTATION_DELTA_US: u64 = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationErrorKind {
    UnsupportedVersion,
    InvalidValue,
    CapacityExceeded,
    ConflictingTracks,
    TimeOverflow,
    EmptyInfiniteLoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationError {
    pub kind: AnimationErrorKind,
    pub step: usize,
}

impl std::fmt::Display for AnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at step {}", self.kind, self.step)
    }
}
impl std::error::Error for AnimationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TweenProperty {
    Translation,
    Rotation,
    Scale,
    Pivot,
    Opacity,
    Tint,
}

impl TweenProperty {
    pub fn ui_property(self) -> crate::aui::UiProperty {
        use crate::aui::UiProperty as P;
        match self {
            Self::Translation => P::Translation,
            Self::Rotation => P::Rotation,
            Self::Scale => P::Scale,
            Self::Pivot => P::Pivot,
            Self::Opacity => P::Opacity,
            Self::Tint => P::Tint,
        }
    }
}

impl TweenValue {
    pub fn ui_value(self, property: TweenProperty) -> crate::aui::UiValue {
        use crate::aui::UiValue as V;
        match self {
            Self::Scalar(v) => V::Number(v),
            Self::Color(v) => V::Color(v),
            Self::Vec2(v) if property == TweenProperty::Translation => V::Translation(v),
            Self::Vec2(v) => V::Vector(v),
        }
    }
}

/// A session identity plus a reusable slot and generation; rolled-back handles never become live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationHandle {
    pub session: u64,
    pub slot: u32,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationClock {
    Ui,
    Game,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationPlay {
    pub animation: Animation2D,
    /// All playback, including infinite loops and Hold, is tied to this live node.
    pub owner: UiNodeHandle,
    pub scope: crate::aui::UiSubscriptionScope,
    pub clock: AnimationClock,
    #[serde(default)]
    pub conflict: TrackConflictPolicy,
    pub callback_id: Option<crate::aui::UiCallbackId>,
}

impl AnimationPlay {
    /// Active-scoped UI playback, rejecting concurrent writers and releasing with the
    /// description's completion policy. Add a callback ID only when an outcome is needed.
    pub fn new(animation: Animation2D, owner: UiNodeHandle) -> Self {
        Self {
            animation,
            owner,
            scope: crate::aui::UiSubscriptionScope::Active,
            clock: AnimationClock::Ui,
            conflict: TrackConflictPolicy::Reject,
            callback_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnimationRequest {
    Play {
        request: AnimationPlay,
    },
    SetPaused {
        handle: AnimationHandle,
        paused: bool,
    },
    Cancel {
        handle: AnimationHandle,
    },
    SetGameClock {
        paused: bool,
        speed: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationReply {
    Handle(AnimationHandle),
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationOutcome {
    Completed,
    Cancelled,
    Replaced,
    TargetDisposed,
    Conflict,
    CommitConflict,
    Error,
}

pub trait UiAnimationAccess {
    fn animation_request(
        &self,
        request: AnimationRequest,
    ) -> Result<AnimationReply, crate::aui::UiError> {
        let _ = request;
        Err(crate::aui_host::unavailable())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TweenValue {
    Scalar(f32),
    Vec2([f32; 2]),
    Color([f32; 4]),
}

impl TweenValue {
    fn valid_for(self, property: TweenProperty) -> bool {
        match (property, self) {
            (
                TweenProperty::Translation | TweenProperty::Scale | TweenProperty::Pivot,
                Self::Vec2(v),
            ) => v.iter().all(|v| v.is_finite()),
            (TweenProperty::Rotation, Self::Scalar(v)) => v.is_finite(),
            (TweenProperty::Opacity, Self::Scalar(v)) => v.is_finite() && (0.0..=1.0).contains(&v),
            (TweenProperty::Tint, Self::Color(v)) => validate_rgba(v).is_ok(),
            _ => false,
        }
    }

    /// Linear interpolation in the property's declared numeric space. Tint conversion
    /// belongs to the consumer; rotations use unwrapped radians, not shortest-arc magic.
    pub fn interpolate(self, to: Self, progress: f32) -> Result<Self, AnimationError> {
        let error = AnimationError {
            kind: AnimationErrorKind::InvalidValue,
            step: 0,
        };
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(error);
        }
        let lerp = |a: f32, b: f32| -> Result<f32, AnimationError> {
            if !a.is_finite() || !b.is_finite() {
                return Err(error);
            }
            Ok(
                (f64::from(a) * (1.0 - f64::from(progress)) + f64::from(b) * f64::from(progress))
                    as f32,
            )
        };
        match (self, to) {
            (Self::Scalar(a), Self::Scalar(b)) => Ok(Self::Scalar(lerp(a, b)?)),
            (Self::Vec2(a), Self::Vec2(b)) => {
                Ok(Self::Vec2([lerp(a[0], b[0])?, lerp(a[1], b[1])?]))
            }
            (Self::Color(a), Self::Color(b)) => Ok(Self::Color([
                lerp(a[0], b[0])?,
                lerp(a[1], b[1])?,
                lerp(a[2], b[2])?,
                lerp(a[3], b[3])?,
            ])),
            _ => Err(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutCubic,
}

impl Easing {
    pub fn sample(self, progress: f32) -> Result<f32, AnimationError> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(AnimationError {
                kind: AnimationErrorKind::InvalidValue,
                step: 0,
            });
        }
        Ok(match self {
            Self::Linear => progress,
            Self::EaseInQuad => progress * progress,
            Self::EaseOutQuad => 1.0 - (1.0 - progress).powi(2),
            Self::EaseInOutCubic if progress < 0.5 => 4.0 * progress.powi(3),
            Self::EaseInOutCubic => 1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TweenTrack {
    pub target: UiNodeHandle,
    pub property: TweenProperty,
    pub from: TweenValue,
    pub to: TweenValue,
    pub delay_us: u64,
    pub duration_us: u64,
    pub easing: Easing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnimationStep {
    Tween { track: TweenTrack },
    Delay { duration_us: u64 },
    Sequence { steps: Vec<AnimationStep> },
    Parallel { steps: Vec<AnimationStep> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Repeat {
    Once,
    Count { cycles: u32 },
    Forever,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackDirection {
    Forward,
    Reverse,
    Yoyo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionPolicy {
    Release,
    Hold,
    CommitFinal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackConflictPolicy {
    #[default]
    Reject,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Animation2D {
    pub version: u32,
    pub root: AnimationStep,
    /// Repetition applies to the whole composition; Count is total cycles, not extra repeats.
    pub repeat: Repeat,
    /// Yoyo is one forward plus one reverse traversal per cycle.
    pub direction: PlaybackDirection,
    pub completion: CompletionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationTiming {
    pub traversal_us: u64,
    pub cycle_us: u64,
    pub total_us: Option<u64>,
    pub leaf_steps: usize,
}

#[derive(Clone)]
struct Interval {
    target: UiNodeHandle,
    property: TweenProperty,
    start: u64,
    end: u64,
    step: usize,
}

impl Interval {
    fn overlaps(&self, other: &Self) -> bool {
        if self.target != other.target || self.property != other.property {
            return false;
        }
        match (self.start == self.end, other.start == other.end) {
            (true, true) => self.start == other.start,
            (true, false) => other.start <= self.start && self.start < other.end,
            (false, true) => self.start <= other.start && other.start < self.end,
            (false, false) => self.start < other.end && other.start < self.end,
        }
    }
}

struct Validator {
    nodes: usize,
    leaves: usize,
}

impl Validator {
    fn add(a: u64, b: u64, step: usize) -> Result<u64, AnimationError> {
        a.checked_add(b).ok_or(AnimationError {
            kind: AnimationErrorKind::TimeOverflow,
            step,
        })
    }

    fn visit(
        &mut self,
        node: &AnimationStep,
        origin: u64,
        depth: usize,
    ) -> Result<(u64, Vec<Interval>), AnimationError> {
        let step = self.nodes;
        // Containers are also bounded; empty containers cannot bypass the leaf budget.
        if depth > MAX_ANIMATION_DEPTH || self.nodes >= MAX_ANIMATION_STEPS * 2 {
            return Err(AnimationError {
                kind: AnimationErrorKind::CapacityExceeded,
                step,
            });
        }
        self.nodes += 1;
        match node {
            AnimationStep::Tween { track } => {
                self.leaf(step)?;
                if !track.from.valid_for(track.property) || !track.to.valid_for(track.property) {
                    return Err(AnimationError {
                        kind: AnimationErrorKind::InvalidValue,
                        step,
                    });
                }
                let start = Self::add(origin, track.delay_us, step)?;
                let end = Self::add(start, track.duration_us, step)?;
                Ok((
                    end,
                    vec![Interval {
                        target: track.target,
                        property: track.property,
                        start,
                        end,
                        step,
                    }],
                ))
            }
            AnimationStep::Delay { duration_us } => {
                self.leaf(step)?;
                Ok((Self::add(origin, *duration_us, step)?, Vec::new()))
            }
            AnimationStep::Sequence { steps } => {
                let mut end = origin;
                let mut intervals = Vec::new();
                for child in steps {
                    let (next, child_intervals) = self.visit(child, end, depth + 1)?;
                    end = next;
                    intervals.extend(child_intervals);
                }
                Ok((end, intervals))
            }
            AnimationStep::Parallel { steps } => {
                let mut end = origin;
                let mut intervals: Vec<Interval> = Vec::new();
                for child in steps {
                    let (next, child_intervals) = self.visit(child, origin, depth + 1)?;
                    for interval in &child_intervals {
                        if intervals.iter().any(|existing| existing.overlaps(interval)) {
                            return Err(AnimationError {
                                kind: AnimationErrorKind::ConflictingTracks,
                                step: interval.step,
                            });
                        }
                    }
                    end = end.max(next);
                    intervals.extend(child_intervals);
                }
                Ok((end, intervals))
            }
        }
    }

    fn leaf(&mut self, step: usize) -> Result<(), AnimationError> {
        if self.leaves >= MAX_ANIMATION_STEPS {
            return Err(AnimationError {
                kind: AnimationErrorKind::CapacityExceeded,
                step,
            });
        }
        self.leaves += 1;
        Ok(())
    }
}

impl Animation2D {
    /// Does not validate node liveness or CommitFinal ownership: only a runtime can do that.
    pub fn validate(&self) -> Result<AnimationTiming, AnimationError> {
        let fail = |kind| AnimationError { kind, step: 0 };
        if self.version != ANIMATION_2D_VERSION {
            return Err(fail(AnimationErrorKind::UnsupportedVersion));
        }
        let mut validator = Validator {
            nodes: 0,
            leaves: 0,
        };
        let (traversal_us, _) = validator.visit(&self.root, 0, 1)?;
        let cycle_us = traversal_us
            .checked_mul(if self.direction == PlaybackDirection::Yoyo {
                2
            } else {
                1
            })
            .ok_or_else(|| fail(AnimationErrorKind::TimeOverflow))?;
        let total_us = match self.repeat {
            Repeat::Once => Some(cycle_us),
            Repeat::Count { cycles: 0 } => return Err(fail(AnimationErrorKind::InvalidValue)),
            Repeat::Count { cycles } => Some(
                cycle_us
                    .checked_mul(u64::from(cycles))
                    .ok_or_else(|| fail(AnimationErrorKind::TimeOverflow))?,
            ),
            Repeat::Forever if cycle_us == 0 => {
                return Err(fail(AnimationErrorKind::EmptyInfiniteLoop))
            }
            Repeat::Forever => None,
        };
        Ok(AnimationTiming {
            traversal_us,
            cycle_us,
            total_us,
            leaf_steps: validator.leaves,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockStep {
    pub advanced: bool,
    pub delta_us: u64,
    pub elapsed_us: u64,
}

/// One clock per scope, called once by the presentation frame owner. Calling it from
/// multiple fixed updates with the same frame index cannot count a delta twice.
#[derive(Debug, Clone)]
pub struct PresentationClock {
    last_frame: Option<u64>,
    elapsed_us: u64,
    fractional_us: f64,
    paused: bool,
    speed: f64,
}

impl Default for PresentationClock {
    fn default() -> Self {
        Self {
            last_frame: None,
            elapsed_us: 0,
            fractional_us: 0.0,
            paused: false,
            speed: 1.0,
        }
    }
}

impl PresentationClock {
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn set_speed(&mut self, speed: f64) -> Result<(), AnimationError> {
        if !speed.is_finite() || speed < 0.0 {
            return Err(AnimationError {
                kind: AnimationErrorKind::InvalidValue,
                step: 0,
            });
        }
        self.speed = speed;
        Ok(())
    }
    pub fn elapsed_us(&self) -> u64 {
        self.elapsed_us
    }

    pub fn advance(&mut self, frame: u64, delta_us: u64) -> Result<ClockStep, AnimationError> {
        if self.last_frame.is_some_and(|last| frame <= last) {
            return Ok(ClockStep {
                advanced: false,
                delta_us: 0,
                elapsed_us: self.elapsed_us,
            });
        }
        let scaled = if self.paused {
            self.fractional_us
        } else {
            delta_us.min(MAX_PRESENTATION_DELTA_US) as f64 * self.speed + self.fractional_us
        };
        // u64::MAX rounds up to 2^64 in f64. Reject that boundary, never saturate a cast.
        if !scaled.is_finite() || scaled >= u64::MAX as f64 {
            return Err(AnimationError {
                kind: AnimationErrorKind::TimeOverflow,
                step: 0,
            });
        }
        let delta = scaled.floor() as u64;
        let elapsed = self.elapsed_us.checked_add(delta).ok_or(AnimationError {
            kind: AnimationErrorKind::TimeOverflow,
            step: 0,
        })?;
        self.last_frame = Some(frame);
        self.elapsed_us = elapsed;
        self.fractional_us = scaled - delta as f64;
        Ok(ClockStep {
            advanced: true,
            delta_us: delta,
            elapsed_us: elapsed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(delay: u64, duration: u64) -> AnimationStep {
        AnimationStep::Tween {
            track: TweenTrack {
                target: UiNodeHandle {
                    instance: 1,
                    slot: 0,
                    generation: 1,
                },
                property: TweenProperty::Opacity,
                from: TweenValue::Scalar(0.0),
                to: TweenValue::Scalar(1.0),
                delay_us: delay,
                duration_us: duration,
                easing: Easing::Linear,
            },
        }
    }
    fn animation(root: AnimationStep) -> Animation2D {
        Animation2D {
            version: ANIMATION_2D_VERSION,
            root,
            repeat: Repeat::Once,
            direction: PlaybackDirection::Forward,
            completion: CompletionPolicy::Release,
        }
    }

    #[test]
    fn sequential_same_property_is_valid_parallel_overlap_is_not() {
        assert_eq!(
            animation(AnimationStep::Sequence {
                steps: vec![track(0, 100), track(0, 100)]
            })
            .validate()
            .unwrap()
            .total_us,
            Some(200)
        );
        let error = animation(AnimationStep::Parallel {
            steps: vec![track(0, 100), track(50, 100)],
        })
        .validate()
        .unwrap_err();
        assert_eq!(error.kind, AnimationErrorKind::ConflictingTracks);
        assert_eq!(error.step, 2);
        assert!(animation(AnimationStep::Parallel {
            steps: vec![track(0, 100), track(100, 100)]
        })
        .validate()
        .is_ok());
    }

    #[test]
    fn parallel_conflict_check_preserves_gaps_in_nested_sequences() {
        let sequence = AnimationStep::Sequence {
            steps: vec![
                track(0, 20),
                AnimationStep::Delay { duration_us: 60 },
                track(0, 20),
            ],
        };
        assert!(animation(AnimationStep::Parallel {
            steps: vec![sequence.clone(), track(30, 20)]
        })
        .validate()
        .is_ok());
        assert!(animation(AnimationStep::Parallel {
            steps: vec![sequence, track(90, 20)]
        })
        .validate()
        .is_err());
    }

    #[test]
    fn instant_parallel_writes_conflict_but_serial_order_is_defined() {
        assert!(animation(AnimationStep::Sequence {
            steps: vec![track(0, 0), track(0, 0)]
        })
        .validate()
        .is_ok());
        assert!(animation(AnimationStep::Parallel {
            steps: vec![track(0, 0), track(0, 0)]
        })
        .validate()
        .is_err());
    }

    #[test]
    fn looping_yoyo_and_overflow_have_checked_timing() {
        let mut description = animation(track(10, 40));
        description.direction = PlaybackDirection::Yoyo;
        description.repeat = Repeat::Count { cycles: 3 };
        assert_eq!(description.validate().unwrap().total_us, Some(300));
        description.root = track(0, 0);
        description.repeat = Repeat::Forever;
        assert_eq!(
            description.validate().unwrap_err().kind,
            AnimationErrorKind::EmptyInfiniteLoop
        );
        description.root = track(1, u64::MAX);
        assert_eq!(
            description.validate().unwrap_err().kind,
            AnimationErrorKind::TimeOverflow
        );
    }

    #[test]
    fn whole_cycle_overflow_and_zero_repeat_are_rejected() {
        let mut description = animation(track(0, u64::MAX));
        description.direction = PlaybackDirection::Yoyo;
        assert_eq!(
            description.validate().unwrap_err().kind,
            AnimationErrorKind::TimeOverflow
        );
        description.direction = PlaybackDirection::Forward;
        description.repeat = Repeat::Count { cycles: 2 };
        assert_eq!(
            description.validate().unwrap_err().kind,
            AnimationErrorKind::TimeOverflow
        );
        description.repeat = Repeat::Count { cycles: 0 };
        assert_eq!(
            description.validate().unwrap_err().kind,
            AnimationErrorKind::InvalidValue
        );
    }

    #[test]
    fn generation_and_property_distinguish_parallel_targets_and_description_round_trips() {
        let first = track(0, 100);
        let mut second = track(0, 100);
        if let AnimationStep::Tween { track } = &mut second {
            track.target.generation += 1;
        }
        let description = animation(AnimationStep::Parallel {
            steps: vec![first, second],
        });
        let decoded: Animation2D =
            serde_json::from_slice(&serde_json::to_vec(&description).unwrap()).unwrap();
        assert_eq!(decoded.validate().unwrap().total_us, Some(100));
        assert_eq!(decoded, description);
    }

    #[test]
    fn deep_empty_containers_and_leaf_count_are_bounded() {
        let mut root = track(0, 1);
        for _ in 0..MAX_ANIMATION_DEPTH {
            root = AnimationStep::Sequence { steps: vec![root] };
        }
        assert_eq!(
            animation(root).validate().unwrap_err().kind,
            AnimationErrorKind::CapacityExceeded
        );
        assert!(animation(AnimationStep::Sequence {
            steps: vec![track(0, 1); MAX_ANIMATION_STEPS + 1]
        })
        .validate()
        .is_err());
        assert!(animation(AnimationStep::Sequence {
            steps: vec![AnimationStep::Sequence { steps: vec![] }; MAX_ANIMATION_STEPS * 2]
        })
        .validate()
        .is_err());
    }

    #[test]
    fn mismatched_property_values_and_protocol_versions_are_rejected() {
        let mut root = track(0, 1);
        if let AnimationStep::Tween { track } = &mut root {
            track.to = TweenValue::Vec2([1.0; 2]);
        }
        assert_eq!(
            animation(root).validate().unwrap_err().kind,
            AnimationErrorKind::InvalidValue
        );
        let mut description = animation(track(0, 1));
        description.version += 1;
        assert_eq!(
            description.validate().unwrap_err().kind,
            AnimationErrorKind::UnsupportedVersion
        );
    }

    #[test]
    fn clock_counts_frame_once_clamps_and_pauses_without_catchup() {
        let mut clock = PresentationClock::default();
        assert_eq!(
            clock.advance(10, 1_000_000).unwrap().delta_us,
            MAX_PRESENTATION_DELTA_US
        );
        assert!(!clock.advance(10, 1_000_000).unwrap().advanced);
        assert!(!clock.advance(9, 1_000_000).unwrap().advanced);
        clock.set_paused(true);
        assert_eq!(clock.advance(11, 100_000).unwrap().delta_us, 0);
        clock.set_paused(false);
        assert_eq!(clock.advance(12, 10).unwrap().delta_us, 10);
        assert_eq!(clock.elapsed_us(), MAX_PRESENTATION_DELTA_US + 10);
    }

    #[test]
    fn fractional_speed_retains_submicroseconds_and_error_is_atomic() {
        let mut clock = PresentationClock::default();
        clock.set_speed(0.5).unwrap();
        assert_eq!(clock.advance(0, 1).unwrap().delta_us, 0);
        assert_eq!(clock.advance(1, 1).unwrap().delta_us, 1);
        clock.set_speed(f64::MAX).unwrap();
        assert_eq!(
            clock.advance(2, 2).unwrap_err().kind,
            AnimationErrorKind::TimeOverflow
        );
        clock.set_speed(1.0).unwrap();
        assert_eq!(clock.advance(2, 1).unwrap().delta_us, 1);
        assert!(clock.set_speed(-1.0).is_err());
    }

    #[test]
    fn interpolation_does_not_overflow_finite_endpoints() {
        assert_eq!(
            TweenValue::Scalar(-f32::MAX)
                .interpolate(TweenValue::Scalar(f32::MAX), 0.5)
                .unwrap(),
            TweenValue::Scalar(0.0)
        );
        for easing in [
            Easing::Linear,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutCubic,
        ] {
            assert_eq!(easing.sample(0.0).unwrap(), 0.0);
            assert_eq!(easing.sample(1.0).unwrap(), 1.0);
            assert!(easing.sample(f32::NAN).is_err());
        }
    }
}
