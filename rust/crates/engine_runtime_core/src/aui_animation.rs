//! Bounded session playback. Only live compositions and their indexed property overrides are retained.
use crate::aui_instance::AuiInstance;
use project_runtime_sdk::aui::*;
use project_runtime_sdk::aui_animation::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_PLAYBACKS: usize = 4096;
pub const MAX_ACTIVE_TRACKS: usize = 4096;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
type Key = (UiNodeHandle, UiProperty);

fn invalid(kind: UiErrorKind) -> UiError {
    UiError {
        kind,
        handle: None,
        property: None,
    }
}

#[derive(Clone)]
struct Track {
    track: TweenTrack,
    start: u64,
    end: u64,
}

#[derive(Clone)]
struct Playback {
    handle: AnimationHandle,
    request: AnimationPlay,
    timing: AnimationTiming,
    tracks: Vec<Track>,
    stamps: BTreeMap<Key, (UiPropertyOwner, u64)>,
    elapsed: u64,
    paused: bool,
    held: bool,
    notified: bool,
}

#[derive(Clone)]
pub(crate) struct AnimationEvent {
    pub callback_id: UiCallbackId,
    pub target: UiNodeHandle,
    pub handle: AnimationHandle,
    pub outcome: AnimationOutcome,
}

#[derive(Clone)]
pub(crate) struct AuiAnimations {
    session: u64,
    plays: BTreeMap<u32, Playback>,
    ui_clock: PresentationClock,
    game_clock: PresentationClock,
    values: BTreeMap<Key, UiValue>,
    active_owners: BTreeMap<Key, u32>,
    pub events: VecDeque<AnimationEvent>,
}

impl Default for AuiAnimations {
    fn default() -> Self {
        Self {
            session: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            plays: BTreeMap::new(),
            ui_clock: Default::default(),
            game_clock: Default::default(),
            values: BTreeMap::new(),
            active_owners: BTreeMap::new(),
            events: VecDeque::new(),
        }
    }
}

fn compile(step: &AnimationStep, origin: u64, result: &mut Vec<Track>) -> u64 {
    match step {
        AnimationStep::Tween { track } => {
            let start = origin + track.delay_us;
            let end = start + track.duration_us;
            result.push(Track {
                track: track.clone(),
                start,
                end,
            });
            end
        }
        AnimationStep::Delay { duration_us } => origin + duration_us,
        AnimationStep::Sequence { steps } => steps
            .iter()
            .fold(origin, |time, step| compile(step, time, result)),
        AnimationStep::Parallel { steps } => steps
            .iter()
            .map(|step| compile(step, origin, result))
            .max()
            .unwrap_or(origin),
    }
}

impl Playback {
    fn position(&self) -> u64 {
        let cycle = self.timing.cycle_us;
        let terminal = self
            .timing
            .total_us
            .is_some_and(|total| self.elapsed >= total);
        let t = if terminal {
            cycle
        } else if cycle == 0 {
            0
        } else {
            self.elapsed % cycle
        };
        match self.request.animation.direction {
            PlaybackDirection::Forward => t,
            PlaybackDirection::Reverse => self.timing.traversal_us.saturating_sub(t),
            PlaybackDirection::Yoyo => {
                if t <= self.timing.traversal_us {
                    t
                } else {
                    cycle - t
                }
            }
        }
    }

    fn samples(&self, terminal: bool) -> BTreeMap<Key, UiValue> {
        let t = self.position();
        let mut values = BTreeMap::new();
        if terminal {
            // Serial reuse has a defined terminal value. Reverse/Yoyo ends at the first value.
            let reverse = self.request.animation.direction != PlaybackDirection::Forward;
            let mut tracks: Vec<_> = self.tracks.iter().collect();
            tracks.sort_by_key(|track| (track.start, track.end));
            if reverse {
                tracks.reverse();
            }
            for track in tracks {
                let value = if reverse {
                    track.track.from
                } else {
                    track.track.to
                };
                values.insert(
                    (track.track.target, track.track.property.ui_property()),
                    value.ui_value(track.track.property),
                );
            }
            return values;
        }
        for track in &self.tracks {
            let reverse = match self.request.animation.direction {
                PlaybackDirection::Reverse => true,
                PlaybackDirection::Yoyo => {
                    self.timing.cycle_us != 0
                        && self.elapsed % self.timing.cycle_us >= self.timing.traversal_us
                }
                PlaybackDirection::Forward => false,
            };
            let outside = if reverse {
                t <= track.start || t > track.end
            } else {
                t < track.start || t >= track.end
            };
            if (track.start != track.end && outside)
                || (track.start == track.end && t != track.start)
            {
                continue;
            }
            let progress = if track.start == track.end {
                1.0
            } else {
                (t - track.start) as f64 / (track.end - track.start) as f64
            } as f32;
            let value = track
                .track
                .from
                .interpolate(
                    track.track.to,
                    track
                        .track
                        .easing
                        .sample(progress)
                        .expect("validated progress"),
                )
                .expect("validated track");
            values.insert(
                (track.track.target, track.track.property.ui_property()),
                value.ui_value(track.track.property),
            );
        }
        values
    }
}

impl AuiAnimations {
    pub(crate) fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        owner: &str,
        role: &str,
    ) {
        fn composition(
            step: &AnimationStep,
            sample: &mut crate::resource_memory::ResourceMemorySample,
            owner: &str,
            role: &str,
        ) {
            if let AnimationStep::Sequence { steps } | AnimationStep::Parallel { steps } = step {
                sample.cpu_vec(steps, owner, &[role]);
                for step in steps {
                    composition(step, sample, owner, role);
                }
            }
        }
        for play in self.plays.values() {
            composition(&play.request.animation.root, sample, owner, role);
            sample.cpu_vec(&play.tracks, owner, &[role]);
        }
        sample.cpu_deque(&self.events, "queues", &["queuePayload"]);
        // Tracks/events and map stamps/keys are scalar. Samples come only from
        // numeric TweenValue::ui_value; std tree node storage is not exposed.
        sample.partial(
            &format!("cpu.owner.{owner}"),
            "sdk.ui.animationComposition+compiledTracks.actualBacking;stdTreeNodesOpaque",
        );
        sample.partial(
            "cpu.owner.queues",
            "sdk.ui.animationEvents.actualDequeCapacity",
        );
    }

    pub fn request(
        &mut self,
        request: AnimationRequest,
        instances: &BTreeMap<u64, AuiInstance>,
        drafts: &BTreeMap<u64, AuiInstance>,
    ) -> Result<AnimationReply, UiError> {
        match request {
            AnimationRequest::Play { request } => {
                // Existing playback controls consume their reserved event, even at capacity.
                if self.events.len()
                    + self
                        .plays
                        .values()
                        .filter(|p| !p.notified && p.request.callback_id.is_some())
                        .count()
                    >= MAX_PLAYBACKS
                {
                    return Err(invalid(UiErrorKind::CapacityExceeded));
                }
                let timing = request
                    .animation
                    .validate()
                    .map_err(|_| invalid(UiErrorKind::InvalidValue))?;
                if request.callback_id.is_some_and(|id| id.0 == 0) {
                    return Err(invalid(UiErrorKind::InvalidValue));
                }
                let mut tracks = Vec::new();
                compile(&request.animation.root, 0, &mut tracks);
                if self.plays.len() >= MAX_PLAYBACKS
                    || self.plays.values().map(|p| p.tracks.len()).sum::<usize>() + tracks.len()
                        > MAX_ACTIVE_TRACKS
                {
                    return Err(invalid(UiErrorKind::CapacityExceeded));
                }
                let mut stamps = BTreeMap::new();
                drafts
                    .get(&request.owner.instance)
                    .or_else(|| instances.get(&request.owner.instance))
                    .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
                    .is_effectively_active(request.owner)?;
                for track in &tracks {
                    let key = (track.track.target, track.track.property.ui_property());
                    let stamp = drafts
                        .get(&key.0.instance)
                        .or_else(|| instances.get(&key.0.instance))
                        .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
                        .animation_stamp(key.0, key.1)?;
                    stamps.insert(key, stamp);
                }
                let slot = (0..MAX_PLAYBACKS as u32)
                    .find(|slot| !self.plays.contains_key(slot))
                    .expect("bounded playback slot");
                let handle = AnimationHandle {
                    session: self.session,
                    slot,
                    generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
                };
                let play = Playback {
                    handle,
                    request,
                    timing,
                    tracks,
                    stamps,
                    elapsed: 0,
                    paused: false,
                    held: false,
                    notified: false,
                };
                let keys: BTreeSet<_> = play.samples(false).into_keys().collect();
                let conflicts: BTreeSet<_> = keys
                    .iter()
                    .filter_map(|key| self.active_owners.get(key).copied())
                    .collect();
                if !conflicts.is_empty() && play.request.conflict == TrackConflictPolicy::Reject {
                    return Err(invalid(UiErrorKind::AnimationConflict));
                }
                for slot in conflicts {
                    self.stop(slot, AnimationOutcome::Replaced);
                }
                for key in keys {
                    self.active_owners.insert(key, slot);
                }
                self.plays.insert(slot, play);
                Ok(AnimationReply::Handle(handle))
            }
            AnimationRequest::SetPaused { handle, paused } => {
                self.live_mut(handle)?.paused = paused;
                Ok(AnimationReply::Done)
            }
            AnimationRequest::Cancel { handle } => {
                self.live_mut(handle)?;
                self.stop(handle.slot, AnimationOutcome::Cancelled);
                Ok(AnimationReply::Done)
            }
            AnimationRequest::SetGameClock { paused, speed } => {
                self.game_clock
                    .set_speed(speed)
                    .map_err(|_| invalid(UiErrorKind::InvalidValue))?;
                self.game_clock.set_paused(paused);
                Ok(AnimationReply::Done)
            }
        }
    }

    fn live_mut(&mut self, handle: AnimationHandle) -> Result<&mut Playback, UiError> {
        self.plays
            .get_mut(&handle.slot)
            .filter(|p| p.handle == handle)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))
    }

    fn notify(&mut self, slot: u32, outcome: AnimationOutcome) {
        let play = self.plays.get_mut(&slot).expect("live playback");
        if !play.notified {
            if let Some(callback_id) = play.request.callback_id {
                self.events.push_back(AnimationEvent {
                    callback_id,
                    target: play.request.owner,
                    handle: play.handle,
                    outcome,
                });
            }
            play.notified = true;
        }
    }

    fn stop(&mut self, slot: u32, outcome: AnimationOutcome) {
        self.notify(slot, outcome);
        self.plays.remove(&slot);
        self.active_owners.retain(|_, owner| *owner != slot);
    }

    pub fn capture_new_stamps(&mut self, previous: &Self, instances: &BTreeMap<u64, AuiInstance>) {
        for play in self.plays.values_mut().filter(|p| {
            previous
                .plays
                .get(&p.handle.slot)
                .is_none_or(|old| old.handle != p.handle)
        }) {
            for (key, stamp) in &mut play.stamps {
                if let Some(instance) = instances.get(&key.0.instance) {
                    if let Ok(current) = instance.animation_stamp(key.0, key.1) {
                        *stamp = current;
                    }
                }
            }
        }
    }

    pub fn advance(
        &mut self,
        frame: u64,
        delta_us: u64,
        instances: &mut BTreeMap<u64, AuiInstance>,
    ) -> Result<(), UiError> {
        // Clock changes are atomic even if one clock overflows.
        let mut ui = self.ui_clock.clone();
        let mut game = self.game_clock.clone();
        let ui_step = ui
            .advance(frame, delta_us)
            .map_err(|_| invalid(UiErrorKind::InvalidValue))?;
        let game_step = game
            .advance(frame, delta_us)
            .map_err(|_| invalid(UiErrorKind::InvalidValue))?;
        self.ui_clock = ui;
        self.game_clock = game;
        if !ui_step.advanced {
            return Ok(());
        }
        let slots: Vec<_> = self.plays.keys().copied().collect();
        for slot in slots {
            let play = self.plays.get_mut(&slot).expect("listed playback");
            let targets = std::iter::once(play.request.owner)
                .chain(play.tracks.iter().map(|t| t.track.target));
            let mut active = true;
            let mut disposed = false;
            for target in targets {
                match instances
                    .get(&target.instance)
                    .and_then(|i| i.is_effectively_active(target).ok())
                {
                    Some(enabled) => active &= enabled,
                    None => disposed = true,
                }
            }
            if disposed {
                self.stop(slot, AnimationOutcome::TargetDisposed);
                continue;
            }
            if !play.paused
                && !play.held
                && (active || play.request.scope == UiSubscriptionScope::Instance)
            {
                let delta = if play.request.clock == AnimationClock::Ui {
                    ui_step.delta_us
                } else {
                    game_step.delta_us
                };
                let Some(elapsed) = play.elapsed.checked_add(delta) else {
                    self.stop(slot, AnimationOutcome::Error);
                    continue;
                };
                play.elapsed = play
                    .timing
                    .total_us
                    .map_or(elapsed, |total| elapsed.min(total));
            }
        }
        self.resolve(instances)
    }

    /// Also used after accepted cancellation/structure edits; never advances either clock.
    pub fn resolve(&mut self, instances: &mut BTreeMap<u64, AuiInstance>) -> Result<(), UiError> {
        let mut next: BTreeMap<Key, (u32, UiValue)> = BTreeMap::new();
        // Creation order, rather than reused slot order, determines the existing writer.
        let mut slots: Vec<_> = self
            .plays
            .values()
            .map(|p| (p.handle.generation, p.handle.slot))
            .collect();
        slots.sort_unstable();
        for (_, slot) in slots {
            if !self.plays.contains_key(&slot) {
                continue;
            }
            let play = &self.plays[&slot];
            if std::iter::once(play.request.owner)
                .chain(play.tracks.iter().map(|t| t.track.target))
                .any(|h| {
                    instances
                        .get(&h.instance)
                        .is_none_or(|i| i.node_type(h).is_err())
                })
            {
                self.stop(slot, AnimationOutcome::TargetDisposed);
                continue;
            }
            let terminal = play.held
                || play
                    .timing
                    .total_us
                    .is_some_and(|total| play.elapsed >= total);
            if terminal
                && !play.held
                && play.request.animation.completion == CompletionPolicy::Release
            {
                self.stop(slot, AnimationOutcome::Completed);
                continue;
            }
            let samples = play.samples(terminal);
            let conflicts: BTreeSet<_> = samples
                .keys()
                .filter_map(|key| next.get(key).map(|(slot, _)| *slot))
                .collect();
            if !conflicts.is_empty() {
                if play.request.conflict == TrackConflictPolicy::Reject {
                    self.stop(slot, AnimationOutcome::Conflict);
                    continue;
                }
                for other in conflicts {
                    self.stop(other, AnimationOutcome::Replaced);
                    next.retain(|_, (owner, _)| *owner != other);
                }
            }
            let play = &self.plays[&slot];
            if terminal && !play.held {
                match play.request.animation.completion {
                    CompletionPolicy::Release => {
                        self.stop(slot, AnimationOutcome::Completed);
                        continue;
                    }
                    CompletionPolicy::Hold => {
                        self.notify(slot, AnimationOutcome::Completed);
                        self.plays.get_mut(&slot).unwrap().held = true;
                    }
                    CompletionPolicy::CommitFinal => {
                        let valid = samples.keys().all(|key| {
                            instances
                                .get(&key.0.instance)
                                .and_then(|i| i.animation_stamp(key.0, key.1).ok())
                                == self.plays[&slot].stamps.get(key).copied()
                                && self.plays[&slot].stamps[key].0 == UiPropertyOwner::Controller
                        });
                        if valid {
                            // All values/handles/owners have been checked; per-document commits cannot fail.
                            let ids: BTreeSet<_> =
                                samples.keys().map(|key| key.0.instance).collect();
                            for id in ids {
                                let mut edit = instances.get_mut(&id).unwrap().edit();
                                for (key, value) in
                                    samples.iter().filter(|(key, _)| key.0.instance == id)
                                {
                                    edit.write_without_notify(key.0, key.1, value.clone())?;
                                }
                                edit.commit()?;
                            }
                        }
                        self.stop(
                            slot,
                            if valid {
                                AnimationOutcome::Completed
                            } else {
                                AnimationOutcome::CommitConflict
                            },
                        );
                        continue;
                    }
                }
            }
            for (key, value) in samples {
                next.insert(key, (slot, value));
            }
        }
        let keys: BTreeSet<_> = self.values.keys().chain(next.keys()).copied().collect();
        for key in keys {
            let value = next.get(&key).map(|(_, value)| value.clone());
            if self.values.get(&key) != value.as_ref() {
                if let Some(instance) = instances.get_mut(&key.0.instance) {
                    // Deleted targets are intentionally discarded; generation prevents slot reuse writes.
                    if instance.node_type(key.0).is_ok() {
                        instance.set_animation_value(key.0, key.1, value)?;
                    }
                }
            }
        }
        self.active_owners = next.iter().map(|(key, (slot, _))| (*key, *slot)).collect();
        self.values = next
            .into_iter()
            .map(|(key, (_, value))| (key, value))
            .collect();
        Ok(())
    }

    pub fn cancel_translation(
        &mut self,
        target: UiNodeHandle,
        instances: &mut BTreeMap<u64, AuiInstance>,
    ) {
        let slots: Vec<_> = self
            .plays
            .iter()
            .filter(|(_, p)| {
                p.tracks.iter().any(|t| {
                    t.track.target == target && t.track.property == TweenProperty::Translation
                })
            })
            .map(|(slot, _)| *slot)
            .collect();
        for slot in slots {
            self.stop(slot, AnimationOutcome::Cancelled);
        }
        let _ = self.resolve(instances);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect};
    use std::sync::Arc;

    fn fixture() -> (AuiAnimations, BTreeMap<u64, AuiInstance>, UiNodeHandle) {
        let document = AuiDocument::new(
            "animation",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![AuiNode::new(
                "root",
                AuiNodeKind::Panel,
                AuiRect::stretch_full(),
            )],
        );
        let instance = AuiInstance::mount(Arc::new(document)).unwrap();
        let target = instance.node("root").unwrap();
        (
            AuiAnimations::default(),
            BTreeMap::from([(target.instance, instance)]),
            target,
        )
    }
    fn tween(target: UiNodeHandle, delay: u64, duration: u64, from: f32, to: f32) -> AnimationStep {
        AnimationStep::Tween {
            track: TweenTrack {
                target,
                property: TweenProperty::Opacity,
                from: TweenValue::Scalar(from),
                to: TweenValue::Scalar(to),
                delay_us: delay,
                duration_us: duration,
                easing: Easing::Linear,
            },
        }
    }
    fn play(
        target: UiNodeHandle,
        root: AnimationStep,
        completion: CompletionPolicy,
    ) -> AnimationPlay {
        AnimationPlay {
            animation: Animation2D {
                version: ANIMATION_2D_VERSION,
                root,
                repeat: Repeat::Once,
                direction: PlaybackDirection::Forward,
                completion,
            },
            owner: target,
            scope: UiSubscriptionScope::Active,
            clock: AnimationClock::Ui,
            conflict: TrackConflictPolicy::Reject,
            callback_id: Some(UiCallbackId(7)),
        }
    }
    fn start(
        a: &mut AuiAnimations,
        i: &BTreeMap<u64, AuiInstance>,
        p: AnimationPlay,
    ) -> Result<AnimationHandle, UiError> {
        match a.request(AnimationRequest::Play { request: p }, i, &BTreeMap::new())? {
            AnimationReply::Handle(h) => Ok(h),
            _ => panic!("play handle required"),
        }
    }
    fn opacity(a: &AuiAnimations, target: UiNodeHandle) -> Option<f32> {
        a.values
            .get(&(target, UiProperty::Opacity))
            .map(|v| match v {
                UiValue::Number(v) => *v,
                _ => panic!("scalar required"),
            })
    }

    #[test]
    fn animation_memory_observes_original_composition_compiled_tracks_and_retained_events() {
        use crate::resource_memory::{memory_totals, ResourceMemorySample};
        for capacity in [8, 64] {
            let (mut animations, instances, target) = fixture();
            animations.events = VecDeque::with_capacity(capacity);
            let mut inner = Vec::with_capacity(capacity);
            inner.push(tween(target, 0, 100, 0.0, 1.0));
            let inner_backing = (
                format!("cpu-storage:{:x}", inner.as_ptr() as usize),
                (inner.capacity() * std::mem::size_of::<AnimationStep>()) as u64,
            );
            let mut outer = Vec::with_capacity(capacity);
            outer.push(AnimationStep::Parallel { steps: inner });
            outer.push(AnimationStep::Delay { duration_us: 10 });
            let outer_backing = (
                format!("cpu-storage:{:x}", outer.as_ptr() as usize),
                (outer.capacity() * std::mem::size_of::<AnimationStep>()) as u64,
            );
            let handle = start(
                &mut animations,
                &instances,
                play(
                    target,
                    AnimationStep::Sequence { steps: outer },
                    CompletionPolicy::Release,
                ),
            )
            .unwrap();
            let tracks = &animations.plays[&handle.slot].tracks;
            let tracks_backing = (
                format!("cpu-storage:{:x}", tracks.as_ptr() as usize),
                (tracks.capacity() * std::mem::size_of::<Track>()) as u64,
            );
            let event_backing = (
                format!(
                    "cpu-owned-deque:{:x}",
                    &animations.events as *const _ as usize
                ),
                (animations.events.capacity() * std::mem::size_of::<AnimationEvent>()) as u64,
            );
            let retained = [inner_backing, outer_backing, tracks_backing];
            let mut sample = ResourceMemorySample::new(1, 1, 0);
            animations.observe_memory(&mut sample, "other", "resident");
            for (id, bytes) in &retained {
                let row = sample
                    .allocations
                    .iter()
                    .find(|row| &row.allocation_id == id)
                    .unwrap_or_else(|| panic!("missing animation backing {id} ({bytes})"));
                assert_eq!(row.bytes, *bytes);
                assert_eq!(row.owner, "other");
                assert_eq!(row.roles, ["resident".to_string()].into());
            }
            let total = retained.iter().map(|(_, bytes)| bytes).sum::<u64>() + event_backing.1;
            assert_eq!(
                memory_totals(&sample.allocations).unwrap().cpu_allocated,
                total
            );
            animations.observe_memory(&mut sample, "other", "resident");
            assert_eq!(
                memory_totals(&sample.allocations).unwrap().cpu_allocated,
                total
            );
            assert_eq!(animations.plays.len(), 1);
            assert!(animations.events.is_empty());

            let pending = animations.clone();
            let mut pending_sample = ResourceMemorySample::new(2, 1, 0);
            pending.observe_memory(&mut pending_sample, "queues", "queuePayload");
            let pending_bytes = memory_totals(&pending_sample.allocations)
                .unwrap()
                .cpu_allocated;
            pending.observe_memory(&mut sample, "queues", "queuePayload");
            assert_eq!(
                memory_totals(&sample.allocations).unwrap().cpu_allocated,
                total + pending_bytes
            );
            drop(pending);

            animations
                .request(
                    AnimationRequest::Cancel { handle },
                    &instances,
                    &BTreeMap::new(),
                )
                .unwrap();
            let mut cancelled = ResourceMemorySample::new(3, 1, 0);
            animations.observe_memory(&mut cancelled, "other", "resident");
            assert_eq!(cancelled.allocations.len(), 1);
            assert_eq!(cancelled.allocations[0].allocation_id, event_backing.0);
            assert_eq!(cancelled.allocations[0].bytes, event_backing.1);
            assert_eq!(cancelled.allocations[0].owner, "queues");
            assert_eq!(
                cancelled.allocations[0].roles,
                ["queuePayload".to_string()].into()
            );
            assert_eq!(
                animations.events.pop_front().unwrap().outcome,
                AnimationOutcome::Cancelled
            );
            let mut drained = ResourceMemorySample::new(4, 1, 0);
            animations.observe_memory(&mut drained, "other", "resident");
            assert_eq!(
                memory_totals(&drained.allocations).unwrap().cpu_allocated,
                event_backing.1
            );
            animations.events = VecDeque::new();
            let mut released = ResourceMemorySample::new(5, 1, 0);
            animations.observe_memory(&mut released, "other", "resident");
            assert!(released.allocations.is_empty());
        }
    }

    #[test]
    fn full_playback_capacity_still_allows_pause_cancel_and_clock_control() {
        let (mut animations, instances, target) = fixture();
        let mut first = None;
        for _ in 0..MAX_PLAYBACKS {
            let request = play(
                target,
                AnimationStep::Delay { duration_us: 100 },
                CompletionPolicy::Release,
            );
            let handle = start(&mut animations, &instances, request).unwrap();
            first.get_or_insert(handle);
        }
        let request = play(
            target,
            AnimationStep::Delay { duration_us: 100 },
            CompletionPolicy::Release,
        );
        assert_eq!(
            animations.request(
                AnimationRequest::Play { request },
                &instances,
                &BTreeMap::new()
            ),
            Err(invalid(UiErrorKind::CapacityExceeded))
        );
        let handle = first.unwrap();
        animations
            .request(
                AnimationRequest::SetPaused {
                    handle,
                    paused: true,
                },
                &instances,
                &BTreeMap::new(),
            )
            .unwrap();
        animations
            .request(
                AnimationRequest::SetGameClock {
                    paused: true,
                    speed: 0.5,
                },
                &instances,
                &BTreeMap::new(),
            )
            .unwrap();
        animations
            .request(
                AnimationRequest::Cancel { handle },
                &instances,
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(animations.events.len(), 1);
        assert_eq!(animations.events[0].outcome, AnimationOutcome::Cancelled);
        assert_eq!(animations.plays.len(), MAX_PLAYBACKS - 1);
    }

    #[test]
    fn once_per_present_frame_and_cancel_restore_current_controller_base() {
        let (mut a, mut i, target) = fixture();
        let h = start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 100, 0., 1.),
                CompletionPolicy::Release,
            ),
        )
        .unwrap();
        a.advance(1, 25, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.25));
        for _ in 0..8 {
            a.advance(1, 25, &mut i).unwrap();
        }
        a.advance(0, 100, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.25));
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(target, UiProperty::Opacity, UiValue::Number(0.6))
            .unwrap();
        edit.commit().unwrap();
        assert_eq!(
            i[&target.instance]
                .read(target, UiProperty::Opacity)
                .unwrap(),
            UiValue::Number(0.6)
        );
        a.request(AnimationRequest::Cancel { handle: h }, &i, &BTreeMap::new())
            .unwrap();
        a.resolve(&mut i).unwrap();
        assert_eq!(opacity(&a, target), None);
        assert_eq!(
            i[&target.instance]
                .presentation_property_at(target.slot, UiProperty::Opacity)
                .unwrap()
                .1,
            UiValue::Number(0.6)
        );
        assert_eq!(
            a.events.pop_front().unwrap().outcome,
            AnimationOutcome::Cancelled
        );
        assert_eq!(
            a.request(AnimationRequest::Cancel { handle: h }, &i, &BTreeMap::new())
                .unwrap_err()
                .kind,
            UiErrorKind::InvalidHandle
        );
        assert!(a.events.is_empty());
    }

    #[test]
    fn sequence_gaps_release_override_and_parallel_can_reuse_property_in_gap() {
        let (mut a, mut i, target) = fixture();
        let root = AnimationStep::Parallel {
            steps: vec![
                AnimationStep::Sequence {
                    steps: vec![
                        tween(target, 0, 20, 0., 0.2),
                        AnimationStep::Delay { duration_us: 60 },
                        tween(target, 0, 20, 0.8, 1.),
                    ],
                },
                tween(target, 30, 20, 0.3, 0.5),
            ],
        };
        start(&mut a, &i, play(target, root, CompletionPolicy::Hold)).unwrap();
        a.advance(1, 10, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.1));
        a.advance(2, 15, &mut i).unwrap();
        assert_eq!(opacity(&a, target), None);
        a.advance(3, 15, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.4));
        a.advance(4, 50, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.9));
        a.advance(5, 10, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(1.));
        for f in 6..12 {
            a.advance(f, 1000, &mut i).unwrap();
        }
        assert_eq!(a.events.len(), 1);
        assert_eq!(a.events[0].outcome, AnimationOutcome::Completed);
    }

    #[test]
    fn immediate_conflicts_reject_and_replace_cancel_existing_once() {
        let (mut a, mut i, target) = fixture();
        let request = play(
            target,
            tween(target, 0, 100, 0., 1.),
            CompletionPolicy::Hold,
        );
        let old = start(&mut a, &i, request.clone()).unwrap();
        assert_eq!(
            start(&mut a, &i, request.clone()).unwrap_err().kind,
            UiErrorKind::AnimationConflict
        );
        let mut replacement = request;
        replacement.conflict = TrackConflictPolicy::Replace;
        let new = start(&mut a, &i, replacement).unwrap();
        assert_ne!(old.generation, new.generation);
        a.advance(1, 50, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.5));
        assert_eq!(a.events.len(), 1);
        assert_eq!(a.events[0].handle, old);
        assert_eq!(a.events[0].outcome, AnimationOutcome::Replaced);
    }

    #[test]
    fn delayed_overlap_is_checked_on_activation_and_reused_slot_does_not_change_priority() {
        let (mut a, mut i, target) = fixture();
        start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 200, 0., 1.),
                CompletionPolicy::Release,
            ),
        )
        .unwrap();
        let later = start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 50, 100, 1., 0.),
                CompletionPolicy::Release,
            ),
        )
        .unwrap();
        a.advance(1, 25, &mut i).unwrap();
        assert!(a.events.is_empty());
        a.advance(2, 30, &mut i).unwrap();
        assert_eq!(a.events[0].handle, later);
        assert_eq!(a.events[0].outcome, AnimationOutcome::Conflict);
        assert_eq!(a.plays.len(), 1);
    }

    #[test]
    fn ui_clock_continues_when_game_clock_paused_and_resume_never_catches_up() {
        let (mut a, mut i, target) = fixture();
        let mut request = play(
            target,
            tween(target, 0, 100, 0., 1.),
            CompletionPolicy::Hold,
        );
        request.clock = AnimationClock::Game;
        let handle = start(&mut a, &i, request).unwrap();
        a.request(
            AnimationRequest::SetGameClock {
                paused: true,
                speed: 2.,
            },
            &i,
            &BTreeMap::new(),
        )
        .unwrap();
        a.advance(1, 100, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.));
        assert_eq!(a.ui_clock.elapsed_us(), 100);
        a.request(
            AnimationRequest::SetGameClock {
                paused: false,
                speed: 2.,
            },
            &i,
            &BTreeMap::new(),
        )
        .unwrap();
        a.advance(2, 20, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.4));
        a.request(
            AnimationRequest::SetPaused {
                handle,
                paused: true,
            },
            &i,
            &BTreeMap::new(),
        )
        .unwrap();
        a.advance(3, 1000, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.4));
        a.request(
            AnimationRequest::SetPaused {
                handle,
                paused: false,
            },
            &i,
            &BTreeMap::new(),
        )
        .unwrap();
        a.advance(4, 10, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.6));
        assert!(a
            .request(
                AnimationRequest::SetGameClock {
                    paused: false,
                    speed: f64::NAN
                },
                &i,
                &BTreeMap::new()
            )
            .is_err());
    }

    #[test]
    fn active_scope_pauses_and_hidden_remains_alive() {
        let (mut a, mut i, target) = fixture();
        start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 100, 0., 1.),
                CompletionPolicy::Hold,
            ),
        )
        .unwrap();
        a.advance(1, 20, &mut i).unwrap();
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(target, UiProperty::Active, UiValue::Bool(false))
            .unwrap();
        edit.commit().unwrap();
        a.advance(2, 5000, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.2));
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(target, UiProperty::Active, UiValue::Bool(true))
            .unwrap();
        edit.write_without_notify(
            target,
            UiProperty::Visibility,
            UiValue::Visibility(UiVisibility::Hidden),
        )
        .unwrap();
        edit.commit().unwrap();
        a.advance(3, 10, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.3));
    }

    #[test]
    fn release_complete_is_single_and_hold_is_lifecycle_bound() {
        let (mut a, mut i, target) = fixture();
        start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 10, 0., 1.),
                CompletionPolicy::Release,
            ),
        )
        .unwrap();
        a.advance(1, 10, &mut i).unwrap();
        assert!(a.values.is_empty());
        assert!(a.plays.is_empty());
        a.advance(2, 100, &mut i).unwrap();
        assert_eq!(a.events.len(), 1);
        start(
            &mut a,
            &i,
            play(target, tween(target, 0, 10, 0., 1.), CompletionPolicy::Hold),
        )
        .unwrap();
        a.advance(3, 10, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(1.));
        i.get_mut(&target.instance).unwrap().dispose();
        a.advance(4, 1, &mut i).unwrap();
        assert!(a.plays.is_empty());
        assert!(a.values.is_empty());
        assert_eq!(a.events.len(), 2);
    }

    #[test]
    fn dispose_before_completion_reports_distinct_outcome_and_old_session_handle_is_invalid() {
        let (mut a, mut i, target) = fixture();
        let h = start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 100, 0., 1.),
                CompletionPolicy::Hold,
            ),
        )
        .unwrap();
        i.get_mut(&target.instance).unwrap().dispose();
        a.advance(1, 10, &mut i).unwrap();
        assert_eq!(a.events[0].outcome, AnimationOutcome::TargetDisposed);
        let (mut b, j, next) = fixture();
        start(
            &mut b,
            &j,
            play(next, tween(next, 0, 100, 0., 1.), CompletionPolicy::Hold),
        )
        .unwrap();
        assert_eq!(
            b.request(
                AnimationRequest::SetPaused {
                    handle: h,
                    paused: true
                },
                &j,
                &BTreeMap::new()
            )
            .unwrap_err()
            .kind,
            UiErrorKind::InvalidHandle
        );
    }

    #[test]
    fn commit_final_checks_property_revision_and_does_not_restore_start_snapshot() {
        let (mut a, mut i, target) = fixture();
        start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 10, 0., 0.4),
                CompletionPolicy::CommitFinal,
            ),
        )
        .unwrap();
        a.advance(1, 10, &mut i).unwrap();
        assert_eq!(
            i[&target.instance]
                .read(target, UiProperty::Opacity)
                .unwrap(),
            UiValue::Number(0.4)
        );
        assert!(a.values.is_empty());
        start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 10, 0., 0.8),
                CompletionPolicy::CommitFinal,
            ),
        )
        .unwrap();
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(target, UiProperty::Opacity, UiValue::Number(0.6))
            .unwrap();
        edit.commit().unwrap();
        // Even changing back is an ownership revision change.
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(target, UiProperty::Opacity, UiValue::Number(0.4))
            .unwrap();
        edit.commit().unwrap();
        a.advance(2, 10, &mut i).unwrap();
        assert_eq!(a.events[1].outcome, AnimationOutcome::CommitConflict);
        assert_eq!(
            i[&target.instance]
                .read(target, UiProperty::Opacity)
                .unwrap(),
            UiValue::Number(0.4)
        );
    }

    #[test]
    fn unrelated_base_write_does_not_block_commit_and_slot_reuse_rejects_old_handle() {
        let (mut a, mut i, target) = fixture();
        let old = start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 10, 0., 0.4),
                CompletionPolicy::CommitFinal,
            ),
        )
        .unwrap();
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(target, UiProperty::Rotation, UiValue::Number(0.5))
            .unwrap();
        edit.commit().unwrap();
        a.advance(1, 10, &mut i).unwrap();
        assert_eq!(a.events[0].outcome, AnimationOutcome::Completed);
        let new = start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 100, 0., 1.),
                CompletionPolicy::Hold,
            ),
        )
        .unwrap();
        assert_eq!(old.slot, new.slot);
        assert_ne!(old.generation, new.generation);
        assert!(a
            .request(
                AnimationRequest::Cancel { handle: old },
                &i,
                &BTreeMap::new()
            )
            .is_err());
        assert_eq!(a.plays.len(), 1);
    }

    #[test]
    fn reverse_yoyo_loop_and_zero_duration_commit_have_defined_endpoints() {
        for (direction, terminal) in [
            (PlaybackDirection::Forward, 1.),
            (PlaybackDirection::Reverse, 0.),
            (PlaybackDirection::Yoyo, 0.),
        ] {
            let (mut a, mut i, target) = fixture();
            let mut request = play(
                target,
                tween(target, 0, 100, 0., 1.),
                CompletionPolicy::Hold,
            );
            request.animation.direction = direction;
            request.animation.repeat = Repeat::Count { cycles: 2 };
            start(&mut a, &i, request).unwrap();
            a.resolve(&mut i).unwrap();
            assert_eq!(
                opacity(&a, target),
                Some(if direction == PlaybackDirection::Reverse {
                    1.
                } else {
                    0.
                })
            );
            a.advance(1, 150, &mut i).unwrap();
            assert_eq!(opacity(&a, target), Some(0.5));
            a.advance(2, 250, &mut i).unwrap();
            assert_eq!(opacity(&a, target), Some(terminal));
            assert_eq!(a.events.len(), 1);
        }
        let (mut a, mut i, target) = fixture();
        start(
            &mut a,
            &i,
            play(
                target,
                tween(target, 0, 0, 0., 0.7),
                CompletionPolicy::CommitFinal,
            ),
        )
        .unwrap();
        a.resolve(&mut i).unwrap();
        assert_eq!(
            i[&target.instance]
                .read(target, UiProperty::Opacity)
                .unwrap(),
            UiValue::Number(0.7)
        );
    }

    #[test]
    fn yoyo_turning_point_preserves_the_peak_instead_of_flashing_base() {
        let (mut a, mut i, target) = fixture();
        let mut request = play(
            target,
            tween(target, 0, 100, 0., 1.),
            CompletionPolicy::Hold,
        );
        request.animation.direction = PlaybackDirection::Yoyo;
        start(&mut a, &i, request).unwrap();
        a.advance(1, 100, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(1.));
        a.advance(2, 25, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.75));
        a.advance(3, 75, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.));
    }

    #[test]
    fn all_six_properties_can_run_in_parallel_with_typed_values_and_easing() {
        let (mut a, mut i, target) = fixture();
        let values = [
            (
                TweenProperty::Translation,
                TweenValue::Vec2([0.; 2]),
                TweenValue::Vec2([100., 40.]),
                UiValue::Translation([25., 10.]),
            ),
            (
                TweenProperty::Rotation,
                TweenValue::Scalar(0.),
                TweenValue::Scalar(4.),
                UiValue::Number(1.),
            ),
            (
                TweenProperty::Scale,
                TweenValue::Vec2([1.; 2]),
                TweenValue::Vec2([-1., 3.]),
                UiValue::Vector([0.5, 1.5]),
            ),
            (
                TweenProperty::Pivot,
                TweenValue::Vec2([0.; 2]),
                TweenValue::Vec2([1.; 2]),
                UiValue::Vector([0.25; 2]),
            ),
            (
                TweenProperty::Opacity,
                TweenValue::Scalar(0.),
                TweenValue::Scalar(1.),
                UiValue::Number(0.25),
            ),
            (
                TweenProperty::Tint,
                TweenValue::Color([0.; 4]),
                TweenValue::Color([1.; 4]),
                UiValue::Color([0.25; 4]),
            ),
        ];
        let root = AnimationStep::Parallel {
            steps: values
                .iter()
                .map(|(property, from, to, _)| AnimationStep::Tween {
                    track: TweenTrack {
                        target,
                        property: *property,
                        from: *from,
                        to: *to,
                        delay_us: 0,
                        duration_us: 100,
                        easing: Easing::EaseInQuad,
                    },
                })
                .collect(),
        };
        start(&mut a, &i, play(target, root, CompletionPolicy::Hold)).unwrap();
        a.advance(1, 50, &mut i).unwrap();
        assert_eq!(a.active_owners.len(), 6);
        for (property, _, _, expected) in values {
            assert_eq!(a.values[&(target, property.ui_property())], expected);
        }
    }

    #[test]
    fn infinite_playback_clamps_long_frames_and_drag_cancels_translation_only() {
        let (mut a, mut i, target) = fixture();
        let mut request = play(
            target,
            tween(target, 0, 1_000_000, 0., 1.),
            CompletionPolicy::Hold,
        );
        request.animation.repeat = Repeat::Forever;
        start(&mut a, &i, request).unwrap();
        a.advance(1, u64::MAX, &mut i).unwrap();
        assert_eq!(opacity(&a, target), Some(0.25));
        assert!(a.events.is_empty());
        let translation = AnimationStep::Tween {
            track: TweenTrack {
                target,
                property: TweenProperty::Translation,
                from: TweenValue::Vec2([0.; 2]),
                to: TweenValue::Vec2([100.; 2]),
                duration_us: 100,
                delay_us: 0,
                easing: Easing::Linear,
            },
        };
        start(
            &mut a,
            &i,
            play(target, translation, CompletionPolicy::Hold),
        )
        .unwrap();
        a.resolve(&mut i).unwrap();
        a.cancel_translation(target, &mut i);
        assert!(!a.values.contains_key(&(target, UiProperty::Translation)));
        assert!(a.values.contains_key(&(target, UiProperty::Opacity)));
        assert_eq!(a.events[0].outcome, AnimationOutcome::Cancelled);
        let mut edit = i.get_mut(&target.instance).unwrap().edit();
        edit.write_without_notify(
            target,
            UiProperty::Translation,
            UiValue::Translation([42., 17.]),
        )
        .unwrap();
        edit.commit().unwrap();
        assert_eq!(
            i[&target.instance]
                .presentation_property_at(target.slot, UiProperty::Translation)
                .unwrap()
                .1,
            UiValue::Translation([42., 17.])
        );
    }
}
