use serde::{Deserialize, Serialize};

pub const DEFAULT_FIXED_DELTA_TIME: f32 = 1.0 / 60.0;
pub const DEFAULT_MAXIMUM_DELTA_TIME: f32 = 0.333_333_34;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTime {
    pub time: f32,
    pub delta_time: f32,
    pub unscaled_time: f32,
    pub unscaled_delta_time: f32,
    pub fixed_time: f32,
    pub fixed_delta_time: f32,
    pub frame_count: u64,
    pub fixed_frame_count: u64,
    pub time_scale: f32,
    pub maximum_delta_time: f32,
    pub in_fixed_step: bool,
    clamped_by_maximum_delta_time: bool,
}

impl Default for RuntimeTime {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 0.0,
            unscaled_time: 0.0,
            unscaled_delta_time: 0.0,
            fixed_time: 0.0,
            fixed_delta_time: DEFAULT_FIXED_DELTA_TIME,
            frame_count: 0,
            fixed_frame_count: 0,
            time_scale: 1.0,
            maximum_delta_time: DEFAULT_MAXIMUM_DELTA_TIME,
            in_fixed_step: false,
            clamped_by_maximum_delta_time: false,
        }
    }
}

impl RuntimeTime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance_frame(&mut self, unscaled_delta_time: f32) -> TimeTraceSummary {
        let requested_delta = unscaled_delta_time.max(0.0);
        let clamped_unscaled_delta = requested_delta.min(self.maximum_delta_time);
        self.clamped_by_maximum_delta_time = requested_delta > clamped_unscaled_delta;
        self.unscaled_delta_time = clamped_unscaled_delta;
        self.delta_time = clamped_unscaled_delta * self.time_scale.max(0.0);
        self.unscaled_time += self.unscaled_delta_time;
        self.time += self.delta_time;
        self.frame_count += 1;
        self.in_fixed_step = false;
        self.trace_summary()
    }

    pub fn advance_fixed_step(&mut self) -> TimeTraceSummary {
        self.fixed_frame_count += 1;
        self.fixed_time += self.fixed_delta_time * self.time_scale.max(0.0);
        self.in_fixed_step = true;
        self.trace_summary()
    }

    pub fn leave_fixed_step(&mut self) {
        self.in_fixed_step = false;
    }

    pub fn set_time_scale(&mut self, time_scale: f32) {
        self.time_scale = time_scale.max(0.0);
    }

    pub fn set_maximum_delta_time(&mut self, maximum_delta_time: f32) {
        self.maximum_delta_time = maximum_delta_time.max(0.0);
    }

    pub fn set_fixed_delta_time(&mut self, fixed_delta_time: f32) {
        self.fixed_delta_time = fixed_delta_time.max(0.0);
    }

    pub fn context(&self) -> TimeContext {
        TimeContext {
            time: self.time,
            delta_time: if self.in_fixed_step {
                self.fixed_delta_time * self.time_scale.max(0.0)
            } else {
                self.delta_time
            },
            unscaled_time: self.unscaled_time,
            unscaled_delta_time: self.unscaled_delta_time,
            fixed_time: self.fixed_time,
            fixed_delta_time: self.fixed_delta_time,
            frame_count: self.frame_count,
            fixed_frame_count: self.fixed_frame_count,
            time_scale: self.time_scale,
            in_fixed_step: self.in_fixed_step,
        }
    }

    pub fn trace_summary(&self) -> TimeTraceSummary {
        TimeTraceSummary {
            frame_count: self.frame_count,
            fixed_frame_count: self.fixed_frame_count,
            delta_time: self.delta_time,
            unscaled_delta_time: self.unscaled_delta_time,
            time_scale: self.time_scale,
            in_fixed_step: self.in_fixed_step,
            clamped_by_maximum_delta_time: self.clamped_by_maximum_delta_time,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TimeContext {
    pub time: f32,
    pub delta_time: f32,
    pub unscaled_time: f32,
    pub unscaled_delta_time: f32,
    pub fixed_time: f32,
    pub fixed_delta_time: f32,
    pub frame_count: u64,
    pub fixed_frame_count: u64,
    pub time_scale: f32,
    pub in_fixed_step: bool,
}

impl TimeContext {
    pub fn from_delta(frame_count: u64, delta_time: f32, in_fixed_step: bool) -> Self {
        Self {
            time: delta_time * frame_count as f32,
            delta_time,
            unscaled_time: delta_time * frame_count as f32,
            unscaled_delta_time: delta_time,
            fixed_time: if in_fixed_step { delta_time } else { 0.0 },
            fixed_delta_time: delta_time,
            frame_count,
            fixed_frame_count: if in_fixed_step { 1 } else { 0 },
            time_scale: 1.0,
            in_fixed_step,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeTraceSummary {
    pub frame_count: u64,
    pub fixed_frame_count: u64,
    pub delta_time: f32,
    pub unscaled_delta_time: f32,
    pub time_scale: f32,
    pub in_fixed_step: bool,
    pub clamped_by_maximum_delta_time: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub duration: f32,
    pub elapsed: f32,
    pub repeat: bool,
    pub use_unscaled_time: bool,
    finished_this_frame: bool,
}

impl Timer {
    pub fn new(duration: f32, repeat: bool) -> Self {
        Self {
            duration: duration.max(0.0),
            elapsed: 0.0,
            repeat,
            use_unscaled_time: false,
            finished_this_frame: false,
        }
    }

    pub fn with_unscaled_time(mut self, use_unscaled_time: bool) -> Self {
        self.use_unscaled_time = use_unscaled_time;
        self
    }

    pub fn tick(&mut self, delta: f32) {
        self.finished_this_frame = false;
        self.elapsed += delta.max(0.0);
        if self.elapsed >= self.duration {
            self.finished_this_frame = true;
            if self.repeat && self.duration > 0.0 {
                self.elapsed %= self.duration;
            } else {
                self.elapsed = self.duration;
            }
        }
    }

    pub fn tick_context(&mut self, time: &TimeContext) {
        let delta = if self.use_unscaled_time {
            time.unscaled_delta_time
        } else {
            time.delta_time
        };
        self.tick(delta);
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.finished_this_frame = false;
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn finished_this_frame(&self) -> bool {
        self.finished_this_frame
    }

    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cooldown {
    pub duration: f32,
    pub remaining: f32,
}

impl Cooldown {
    pub fn new(duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            remaining: 0.0,
        }
    }

    pub fn tick(&mut self, delta: f32) {
        self.remaining = (self.remaining - delta.max(0.0)).max(0.0);
    }

    pub fn tick_context(&mut self, time: &TimeContext) {
        self.tick(time.delta_time);
    }

    pub fn ready(&self) -> bool {
        self.remaining <= 0.0
    }

    pub fn trigger(&mut self) -> bool {
        if self.ready() {
            self.remaining = self.duration;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.remaining = 0.0;
    }

    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    pub fn normalized(&self) -> f32 {
        if self.duration <= 0.0 {
            0.0
        } else {
            (self.remaining / self.duration).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_time_defaults_are_stable() {
        let time = RuntimeTime::new();

        assert_eq!(time.time, 0.0);
        assert_eq!(time.delta_time, 0.0);
        assert_eq!(time.fixed_delta_time, DEFAULT_FIXED_DELTA_TIME);
        assert_eq!(time.time_scale, 1.0);
        assert!(!time.in_fixed_step);
    }

    #[test]
    fn runtime_time_advance_applies_time_scale() {
        let mut time = RuntimeTime::new();
        time.set_time_scale(0.5);

        time.advance_frame(0.2);

        assert_eq!(time.unscaled_delta_time, 0.2);
        assert_eq!(time.delta_time, 0.1);
        assert_eq!(time.time, 0.1);
        assert_eq!(time.unscaled_time, 0.2);
    }

    #[test]
    fn runtime_time_clamps_maximum_delta_time() {
        let mut time = RuntimeTime::new();
        time.set_maximum_delta_time(0.1);

        let summary = time.advance_frame(0.5);

        assert_eq!(time.unscaled_delta_time, 0.1);
        assert!(summary.clamped_by_maximum_delta_time);
    }

    #[test]
    fn runtime_time_scale_zero_freezes_scaled_delta() {
        let mut time = RuntimeTime::new();
        time.set_time_scale(0.0);

        time.advance_frame(0.2);

        assert_eq!(time.delta_time, 0.0);
        assert_eq!(time.time, 0.0);
        assert_eq!(time.unscaled_delta_time, 0.2);
        assert_eq!(time.unscaled_time, 0.2);
    }

    #[test]
    fn runtime_time_fixed_step_sets_in_fixed_step() {
        let mut time = RuntimeTime::new();
        time.advance_frame(0.016);

        time.advance_fixed_step();

        assert!(time.context().in_fixed_step);
        assert_eq!(time.context().fixed_frame_count, 1);
        assert_eq!(time.context().delta_time, DEFAULT_FIXED_DELTA_TIME);
    }

    #[test]
    fn time_trace_summary_is_serializable() {
        let mut time = RuntimeTime::new();
        let summary = time.advance_frame(0.016);

        let json = serde_json::to_string(&summary).expect("serialize time trace summary");

        assert!(json.contains("frame_count"));
    }

    #[test]
    fn timer_helper_reports_finished_this_frame() {
        let mut timer = Timer::new(1.0, false);

        timer.tick(0.5);
        assert!(!timer.finished_this_frame());
        timer.tick(0.5);

        assert!(timer.finished());
        assert!(timer.finished_this_frame());
        assert_eq!(timer.progress(), 1.0);
    }

    #[test]
    fn timer_helper_repeat_is_stable() {
        let mut timer = Timer::new(1.0, true);

        timer.tick(1.25);

        assert!(timer.finished_this_frame());
        assert_eq!(timer.elapsed, 0.25);
        assert_eq!(timer.progress(), 0.25);
    }

    #[test]
    fn cooldown_helper_tick_and_trigger_are_stable() {
        let mut cooldown = Cooldown::new(1.0);

        assert!(cooldown.trigger());
        assert!(!cooldown.trigger());
        cooldown.tick(0.25);

        assert_eq!(cooldown.remaining(), 0.75);
        assert!(!cooldown.ready());
        assert_eq!(cooldown.normalized(), 0.75);
    }

    #[test]
    fn cooldown_helper_zero_duration_is_ready() {
        let mut cooldown = Cooldown::new(0.0);

        assert!(cooldown.ready());
        assert!(cooldown.trigger());
        assert!(cooldown.ready());
    }
}
