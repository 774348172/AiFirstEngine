//! Gate C control owner. Typed tables are independent of author JSON/SDK/session
//! (Gate D). Official AnimationState is the only pose mixer.
use engine_spine_runtime::{
    Asset, BonePose, EntrySample, EventCursor, EventIndex, EventKey, Instance, Mask,
    import::{SourceAnimation, SourceEvent, ValidatedSource},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlError(pub &'static str);
type Result<T> = std::result::Result<T, ControlError>;
const INVALID: ControlError = ControlError("animator.runtime.invalid_phase");
const BACKEND: ControlError = ControlError("animator.runtime.backend_failure");
const CHAIN: ControlError = ControlError("animator.mix.chain_capacity");
const EVENT: ControlError = ControlError("animator.runtime.event_capacity");
const ACTION: ControlError = ControlError("animator.binding.unknown_action");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parameter {
    Bool(bool),
    Trigger,
}
#[derive(Debug, Clone, Copy)]
pub struct Layer {
    pub additive: bool,
    pub weight_permille: u16,
}
#[derive(Debug, Clone, Copy)]
pub struct Action {
    pub clip: u16,
    pub layer: u8,
    pub looping: bool,
    pub speed_permille: u16,
    pub mix_in_ticks: u32,
    pub fade_out_ticks: Option<u32>,
    pub mask: Option<u8>,
}
#[derive(Debug, Clone)]
pub struct Rule {
    pub all: Vec<(u8, bool)>,
    pub action: u16,
}
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub from: u16,
    pub to: u16,
    pub mix_ticks: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct Socket {
    pub bone: u16,
    pub offset: [f32; 3],
}
pub struct Definition {
    pub tick_rate: u32,
    pub layers: Vec<Layer>,
    pub actions: Vec<Action>,
    pub masks: Vec<Vec<String>>,
    pub parameters: Vec<Parameter>,
    pub rules: Vec<Rule>,
    pub default_action: u16,
    pub transitions: Vec<Transition>,
    pub sockets: Vec<Socket>,
    pub pixels_per_unit: f32,
    pub source_asset_id: String,
}
struct BoundAction {
    spec: Action,
    mask: Option<Mask>,
}
pub struct ControlTable {
    asset: Asset,
    tick_rate: u32,
    layers: Vec<Layer>,
    actions: Vec<BoundAction>,
    clips: Vec<SourceAnimation>,
    indexes: Vec<EventIndex>,
    bones: Vec<String>,
    parameters: Vec<Parameter>,
    rules: Vec<Rule>,
    default_action: u16,
    transitions: Vec<Transition>,
    sockets: Vec<Socket>,
    pixels_per_unit: f32,
    event_ids: Vec<Vec<String>>,
}
impl ControlTable {
    pub fn compile(source: &ValidatedSource, def: Definition) -> Result<Self> {
        if !(1..=1000).contains(&def.tick_rate) || def.layers.is_empty() || def.layers.len() > 4 {
            return Err(ControlError("animator.mix.layer_capacity"));
        }
        if def.parameters.len() > 16
            || def.masks.len() > 16
            || def.rules.len() > 64
            || def.actions.is_empty()
            || def.actions.len() > 128
        {
            return Err(INVALID);
        }
        if def.default_action as usize >= def.actions.len() {
            return Err(ACTION);
        }
        if def.actions[def.default_action as usize].layer != 0 {
            return Err(INVALID);
        }
        if def.source_asset_id.is_empty()
            || def.source_asset_id.len() > 1024
            || def.source_asset_id.contains('\0')
        {
            return Err(INVALID);
        }
        if def.sockets.len() > 32
            || !def.pixels_per_unit.is_finite()
            || def.pixels_per_unit <= 0.0
            || def.sockets.iter().any(|s| {
                s.bone as usize >= source.bones.len() || s.offset.iter().any(|v| !v.is_finite())
            })
        {
            return Err(ControlError("animator.binding.unknown_socket"));
        }
        if def.transitions.len() > 64
            || def.transitions.iter().enumerate().any(|(i, t)| {
                t.from as usize >= def.actions.len()
                    || t.to as usize >= def.actions.len()
                    || t.mix_ticks > 60 * def.tick_rate
                    || def.actions[t.from as usize].layer != def.actions[t.to as usize].layer
                    || def.transitions[..i]
                        .iter()
                        .any(|p| p.from == t.from && p.to == t.to)
            })
        {
            return Err(INVALID);
        }
        if def.layers.iter().any(|l| l.weight_permille > 1000) || def.layers[0].additive {
            return Err(INVALID);
        }
        for rule in &def.rules {
            if rule.action as usize >= def.actions.len()
                || rule.all.len() > 16
                || rule.all.iter().any(|(p, v)| {
                    *p as usize >= def.parameters.len()
                        || (def.parameters[*p as usize] == Parameter::Trigger && !v)
                })
            {
                return Err(INVALID);
            }
        }
        let asset = source.load().map_err(|_| BACKEND)?;
        let mut actions = Vec::with_capacity(def.actions.len());
        let mut masks = std::collections::BTreeMap::<(u16, u8), Mask>::new();
        for a in def.actions {
            if a.clip as usize >= source.animation_data.len()
                || a.layer as usize >= def.layers.len()
                || a.speed_permille > 4000
                || a.mix_in_ticks > 60 * def.tick_rate
                || a.fade_out_ticks.is_some_and(|t| t > 60 * def.tick_rate)
                || (a.looping && a.fade_out_ticks.is_some())
            {
                return Err(INVALID);
            }
            let clip = &source.animation_data[a.clip as usize];
            let mask = if let Some(index) = a.mask {
                if let Some(bound) = masks.get(&(a.clip, index)) {
                    Some(bound.clone())
                } else {
                    let names = def.masks.get(index as usize).ok_or(INVALID)?;
                    let names: Vec<_> = names.iter().map(String::as_str).collect();
                    let plan = source
                        .mask_plan(&clip.name, &names)
                        .map_err(|d| ControlError(d.code))?;
                    let bound = asset
                        .bind_mask(&plan)
                        .map_err(|_| ControlError("animator.mask.unsupported_timeline"))?;
                    masks.insert((a.clip, index), bound.clone());
                    Some(bound)
                }
            } else {
                None
            };
            if def.layers[a.layer as usize].additive {
                asset
                    .validate_additive(&clip.name, mask.as_ref())
                    .map_err(|_| ControlError("animator.mix.unsupported_additive"))?;
            }
            actions.push(BoundAction { spec: a, mask });
        }
        let indexes = source
            .animation_data
            .iter()
            .map(|c| EventIndex {
                duration_us: c.duration_us,
                keys: c
                    .events
                    .iter()
                    .map(|e| EventKey {
                        time_us: e.time_us,
                        ordinal: e.source_ordinal,
                    })
                    .collect(),
            })
            .collect();
        let event_ids = source
            .animation_data
            .iter()
            .map(|c| {
                c.events
                    .iter()
                    .map(|e| {
                        engine_spine_runtime::import::source_event_id(
                            &def.source_asset_id,
                            &c.name,
                            e.source_ordinal,
                        )
                    })
                    .collect()
            })
            .collect();
        Ok(Self {
            asset,
            tick_rate: def.tick_rate,
            layers: def.layers,
            actions,
            clips: source.animation_data.clone(),
            indexes,
            bones: source.bones.clone(),
            parameters: def.parameters,
            rules: def.rules,
            default_action: def.default_action,
            transitions: def.transitions,
            sockets: def.sockets,
            pixels_per_unit: def.pixels_per_unit,
            event_ids,
        })
    }
    pub fn instance(self: &std::rc::Rc<Self>) -> Result<Controller> {
        let backend = self.asset.instance().map_err(|_| BACKEND)?;
        let state = State {
            external: None,
            logical: 0,
            paused: false,
            next_id: 1,
            layers: vec![Vec::new(); self.layers.len()],
            manual: vec![false; self.layers.len()],
            automatic: vec![None; self.layers.len()],
            values: self
                .parameters
                .iter()
                .map(|p| matches!(p, Parameter::Bool(true)))
                .collect(),
        };
        let mut c = Controller {
            table: self.clone(),
            backend,
            state,
            committed: Pose {
                tick: 0,
                revision: 0,
                bones: Vec::new(),
                sockets: Vec::new(),
                events: Vec::new(),
            },
            isolated: false,
        };
        let (bones, sockets) = c.read_pose()?;
        c.committed.bones = bones;
        c.committed.sockets = sockets;
        Ok(c)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Intent {
    Play(u16),
    Restart(u16),
    SetPaused(bool),
    SetBool(u8, bool),
    SetTrigger(u8),
    Resume,
    FadeOutLayer { layer: u8, ticks: u32 },
    ClearLayer(u8),
}
#[derive(Debug, Clone)]
pub struct Event {
    pub play_id: u64,
    pub track: u8,
    pub loop_index: u64,
    pub source_ordinal: u32,
    pub tick: u64,
    pub crossing_us: u64,
    pub payload: SourceEvent,
    pub source_event_id: String,
}
#[derive(Debug, Clone)]
pub struct Pose {
    pub tick: u64,
    pub revision: u64,
    pub bones: Vec<BonePose>,
    pub sockets: Vec<BonePose>,
    pub events: Vec<Event>,
}
#[derive(Clone)]
struct Entry {
    id: u64,
    action: Option<u16>,
    start: u64,
    mix_start: u64,
    mix_ticks: u32,
    cursor: EventCursor,
}
#[derive(Clone)]
struct State {
    external: Option<u64>,
    logical: u64,
    paused: bool,
    next_id: u64,
    layers: Vec<Vec<Entry>>,
    manual: Vec<bool>,
    automatic: Vec<Option<u16>>,
    values: Vec<bool>,
}
enum Operation {
    Play {
        layer: usize,
        action: u16,
        id: u64,
        mix_ticks: u32,
    },
    Empty {
        layer: usize,
        ticks: u32,
        id: u64,
    },
    Clear(usize),
}
struct Prepared {
    state: State,
    ops: Vec<Operation>,
    events: Vec<Event>,
    samples: Vec<EntrySample>,
    previous_samples: Vec<EntrySample>,
    revision: u64,
}
pub struct Controller {
    table: std::rc::Rc<ControlTable>,
    backend: Instance,
    state: State,
    committed: Pose,
    isolated: bool,
}

fn ceil_div(n: u128, d: u128) -> u128 {
    n.div_ceil(d)
}
impl Controller {
    pub fn pose(&self) -> &Pose {
        &self.committed
    }
    pub fn bone(&self, name: &str) -> Result<BonePose> {
        self.table
            .bones
            .iter()
            .position(|n| n == name)
            .and_then(|i| self.committed.bones.get(i).copied())
            .ok_or(ControlError("animator.binding.unknown_socket"))
    }
    pub fn logical_tick(&self) -> u64 {
        self.state.logical
    }
    pub fn live_entry_count(&self, layer: usize) -> usize {
        self.state.layers.get(layer).map_or(0, Vec::len)
    }
    pub fn socket(&self, index: u8) -> Result<BonePose> {
        self.committed
            .sockets
            .get(index as usize)
            .copied()
            .ok_or(ControlError("animator.binding.unknown_socket"))
    }
    fn read_pose(&self) -> Result<(Vec<BonePose>, Vec<BonePose>)> {
        let transforms: Vec<_> = (0..self.table.bones.len())
            .map(|i| self.backend.bone_transform(i as u32).map_err(|_| BACKEND))
            .collect::<Result<_>>()?;
        let bones = transforms
            .iter()
            .map(|b| BonePose {
                x: b.x,
                y: b.y,
                rotation: b.c.atan2(b.a).to_degrees(),
            })
            .collect();
        let sockets = self
            .table
            .sockets
            .iter()
            .map(|s| {
                let b = transforms[s.bone as usize];
                let [x, y, r] = s.offset;
                let (sin, cos) = r.to_radians().sin_cos();
                let pose = BonePose {
                    x: b.x / self.table.pixels_per_unit + b.a * x + b.b * y,
                    y: b.y / self.table.pixels_per_unit + b.c * x + b.d * y,
                    rotation: (b.c * cos + b.d * sin)
                        .atan2(b.a * cos + b.b * sin)
                        .to_degrees(),
                };
                if [pose.x, pose.y, pose.rotation]
                    .iter()
                    .all(|v| v.is_finite())
                {
                    Ok(pose)
                } else {
                    Err(BACKEND)
                }
            })
            .collect::<Result<_>>()?;
        Ok((bones, sockets))
    }
    fn new_id(s: &mut State) -> Result<u64> {
        let id = s.next_id;
        s.next_id = id.checked_add(1).ok_or(INVALID)?;
        Ok(id)
    }
    fn play(
        &self,
        s: &mut State,
        ops: &mut Vec<Operation>,
        action: u16,
        restart: bool,
    ) -> Result<()> {
        let a = self.table.actions.get(action as usize).ok_or(ACTION)?.spec;
        let l = a.layer as usize;
        if !restart && s.layers[l].last().is_some_and(|e| e.action == Some(action)) {
            return Ok(());
        }
        // Reserve the one bounded empty-entry plan before accepting a once
        // action. Otherwise natural completion could require a fifth entry and
        // prevent every subsequent tick from making progress.
        let reserved = usize::from(a.speed_permille > 0 && a.fade_out_ticks.is_some_and(|t| t > 0));
        if s.layers[l].len() + 1 + reserved > 4 {
            return Err(CHAIN);
        }
        let previous = s.layers[l].last().and_then(|e| e.action);
        let mix_ticks = self
            .table
            .transitions
            .iter()
            .find(|t| Some(t.from) == previous && t.to == action)
            .map_or(a.mix_in_ticks, |t| t.mix_ticks);
        let id = Self::new_id(s)?;
        s.layers[l].push(Entry {
            id,
            action: Some(action),
            start: s.logical,
            mix_start: s.logical,
            mix_ticks,
            cursor: EventCursor::default(),
        });
        ops.push(Operation::Play {
            layer: l,
            action,
            id,
            mix_ticks,
        });
        Ok(())
    }
    fn fade(
        &self,
        s: &mut State,
        ops: &mut Vec<Operation>,
        layer: usize,
        ticks: u32,
        start: u64,
    ) -> Result<()> {
        if layer >= s.layers.len() || ticks > 60 * self.table.tick_rate {
            return Err(INVALID);
        }
        if s.layers[layer].is_empty() {
            return Ok(());
        }
        if ticks == 0 {
            s.layers[layer].clear();
            ops.push(Operation::Clear(layer));
            return Ok(());
        }
        if s.layers[layer].len() >= 4 {
            return Err(CHAIN);
        }
        let id = Self::new_id(s)?;
        s.layers[layer].push(Entry {
            id,
            action: None,
            start,
            mix_start: start,
            mix_ticks: ticks,
            cursor: EventCursor::default(),
        });
        ops.push(Operation::Empty { layer, ticks, id });
        Ok(())
    }
    /// All predictable errors, including the complete event budget, are checked
    /// on a compact staged control state before touching official AnimationState.
    fn prepare(&self, tick: u64, intents: &[Intent]) -> Result<Prepared> {
        if self.isolated {
            return Err(BACKEND);
        }
        if intents.len() > 64 || self.state.external.is_some_and(|t| tick < t) {
            return Err(INVALID);
        }
        let revision = self.committed.revision.checked_add(1).ok_or(INVALID)?;
        let mut s = self.state.clone();
        let mut ops = Vec::new();
        // Pausing freezes logical time, while external ticks continue to be seen.
        let paused = intents
            .iter()
            .filter_map(|i| {
                if let Intent::SetPaused(v) = i {
                    Some(*v)
                } else {
                    None
                }
            })
            .next_back()
            .unwrap_or(s.paused);
        let delta = if s.paused || paused {
            0
        } else {
            s.external.map_or(0, |old| tick - old)
        };
        s.logical = s.logical.checked_add(delta).ok_or(INVALID)?;
        s.external = Some(tick);
        for intent in intents {
            match *intent {
                Intent::Play(a) | Intent::Restart(a) => {
                    let l = self.table.actions.get(a as usize).ok_or(ACTION)?.spec.layer as usize;
                    self.play(&mut s, &mut ops, a, matches!(intent, Intent::Restart(_)))?;
                    s.manual[l] = true;
                }
                Intent::SetPaused(v) => s.paused = v,
                Intent::SetBool(p, v) => {
                    if !matches!(
                        self.table.parameters.get(p as usize),
                        Some(Parameter::Bool(_))
                    ) {
                        return Err(INVALID);
                    }
                    s.values[p as usize] = v;
                }
                Intent::SetTrigger(p) => {
                    if self.table.parameters.get(p as usize) != Some(&Parameter::Trigger) {
                        return Err(INVALID);
                    }
                    s.values[p as usize] = true;
                }
                Intent::Resume => {
                    s.manual.fill(false);
                    s.automatic = s
                        .layers
                        .iter()
                        .map(|l| l.last().and_then(|e| e.action))
                        .collect();
                }
                Intent::ClearLayer(l) => {
                    let layer = s.layers.get_mut(l as usize).ok_or(INVALID)?;
                    layer.clear();
                    s.manual[l as usize] = true;
                    ops.push(Operation::Clear(l as usize));
                }
                Intent::FadeOutLayer { layer, ticks } => {
                    let start = s.logical;
                    self.fade(&mut s, &mut ops, layer as usize, ticks, start)?;
                    s.manual[layer as usize] = true;
                }
            }
        }
        // A completed explicit base once returns to the current rule/default;
        // an explicit loop retains ownership until Resume.
        if let Some(e) = s.layers[0].last() {
            if let Some(a) = e.action {
                let a = self.table.actions[a as usize].spec;
                if !a.looping
                    && a.speed_permille > 0
                    && Self::elapsed_us(s.logical, e.start, a.speed_permille, self.table.tick_rate)?
                        >= self.table.clips[a.clip as usize].duration_us
                {
                    s.manual[0] = false;
                    s.automatic[0] = None;
                }
            }
        }
        let mut consumed = [false; 16];
        for layer in 0..s.layers.len() {
            if s.paused || s.manual[layer] {
                continue;
            }
            let rule = self.table.rules.iter().find(|r| {
                self.table.actions[r.action as usize].spec.layer as usize == layer
                    && r.all.iter().all(|(p, v)| s.values[*p as usize] == *v)
            });
            let desired = rule
                .map(|r| r.action)
                .or_else(|| (layer == 0).then_some(self.table.default_action));
            if s.automatic[layer] == desired {
                continue;
            }
            let prior_id = s.next_id;
            if let Some(desired) = desired {
                self.play(&mut s, &mut ops, desired, false)?;
            } else {
                let ticks = s.layers[layer]
                    .last()
                    .and_then(|e| e.action)
                    .map_or(0, |a| {
                        let a = self.table.actions[a as usize].spec;
                        a.fade_out_ticks.unwrap_or(a.mix_in_ticks)
                    });
                let start = s.logical;
                self.fade(&mut s, &mut ops, layer, ticks, start)?;
            }
            s.automatic[layer] = desired;
            if s.next_id != prior_id {
                if let Some(rule) = rule {
                    for (p, _) in &rule.all {
                        if self.table.parameters[*p as usize] == Parameter::Trigger {
                            consumed[*p as usize] = true;
                        }
                    }
                }
            }
        }
        for (p, used) in consumed.iter().enumerate() {
            if *used {
                s.values[p] = false;
            }
        }
        // Count the complete instance interval before enumerating any loops or
        // cloning payloads. A later layer cannot leave a partially advanced cursor.
        let mut total = 0;
        for (l, entries) in s.layers.iter().enumerate() {
            let Some(current) = entries.last() else {
                continue;
            };
            let Some(action) = current.action else {
                continue;
            };
            let a = self.table.actions[action as usize].spec;
            let end = Self::elapsed_us(
                s.logical,
                current.start,
                a.speed_permille,
                self.table.tick_rate,
            )?;
            let end = if a.looping {
                end
            } else {
                end.min(self.table.clips[a.clip as usize].duration_us)
            };
            if !s.paused
                && self.table.layers[l].weight_permille > 0
                && (entries.len() == 1 || current.mix_ticks == 0 || s.logical > current.mix_start)
            {
                total += current
                    .cursor
                    .count(
                        &self.table.indexes[a.clip as usize],
                        end,
                        a.looping,
                        64 - total,
                    )
                    .map_err(|_| EVENT)?;
            }
        }
        let mut events = Vec::with_capacity(total);
        for (l, entries) in s.layers.iter_mut().enumerate() {
            let mixing = entries.len() > 1;
            let Some(current) = entries.last_mut() else {
                continue;
            };
            let Some(action) = current.action else {
                continue;
            };
            let a = self.table.actions[action as usize].spec;
            let end = Self::elapsed_us(
                s.logical,
                current.start,
                a.speed_permille,
                self.table.tick_rate,
            )?;
            let end = if a.looping {
                end
            } else {
                end.min(self.table.clips[a.clip as usize].duration_us)
            };
            // Incoming at its exact zero-weight mix boundary emits nothing; the
            // cursor still advances, so suppressed keys cannot replay later.
            let effective = self.table.layers[l].weight_permille > 0
                && (!mixing || current.mix_ticks == 0 || s.logical > current.mix_start);
            let found = if effective && !s.paused {
                current
                    .cursor
                    .preview(
                        &self.table.indexes[a.clip as usize],
                        end,
                        a.looping,
                        64 - events.len(),
                    )
                    .map_err(|_| EVENT)?
            } else {
                Vec::new()
            };
            for e in found {
                let source_index = self.table.clips[a.clip as usize]
                    .events
                    .iter()
                    .position(|p| p.source_ordinal == e.ordinal)
                    .ok_or(INVALID)?;
                let payload = self.table.clips[a.clip as usize].events[source_index].clone();
                let source_event_id = self.table.event_ids[a.clip as usize][source_index].clone();
                let crossing_us = if a.speed_permille == 0 {
                    ceil_div(
                        current.start as u128 * 1_000_000,
                        self.table.tick_rate as u128,
                    )
                } else {
                    ceil_div(
                        current.start as u128 * 1_000_000 * a.speed_permille as u128
                            + e.absolute_us as u128 * 1000 * self.table.tick_rate as u128,
                        self.table.tick_rate as u128 * a.speed_permille as u128,
                    )
                };
                events.push(Event {
                    play_id: current.id,
                    track: l as u8,
                    loop_index: e.loop_index,
                    source_ordinal: e.ordinal,
                    tick,
                    crossing_us: u64::try_from(crossing_us).map_err(|_| INVALID)?,
                    payload,
                    source_event_id,
                });
            }
            if !s.paused {
                current.cursor.commit(end).map_err(|_| INVALID)?;
            }
        }
        events.sort_by_key(|e| (e.crossing_us, e.track, e.source_ordinal, e.play_id));
        // Natural completion retains the final content interval, then begins an
        // official empty fade at the exact integer completion tick. Explicit
        // interruption above suppresses the outgoing action immediately.
        for l in 0..s.layers.len() {
            let Some(current) = s.layers[l].last().cloned() else {
                continue;
            };
            if let Some(a) = current.action {
                let a = self.table.actions[a as usize].spec;
                if let Some(fade) = a.fade_out_ticks {
                    if a.speed_permille > 0 {
                        let duration = self.table.clips[a.clip as usize].duration_us;
                        let end = current
                            .start
                            .checked_add(
                                u64::try_from(ceil_div(
                                    duration as u128 * self.table.tick_rate as u128,
                                    a.speed_permille as u128 * 1000,
                                ))
                                .map_err(|_| INVALID)?,
                            )
                            .ok_or(INVALID)?;
                        if s.logical >= end {
                            self.fade(&mut s, &mut ops, l, fade, end)?;
                            if s.logical - end >= fade as u64 {
                                s.layers[l].clear();
                                ops.push(Operation::Clear(l));
                            }
                        }
                    }
                }
            } else if s.logical - current.start >= current.mix_ticks as u64 {
                s.layers[l].clear();
                ops.push(Operation::Clear(l));
            }
        }
        let samples = self.samples(&s)?;
        let mut previous = self.state.clone();
        previous.logical = s.logical;
        let previous_samples = self.samples(&previous)?;
        Ok(Prepared {
            state: s,
            ops,
            events,
            samples,
            previous_samples,
            revision,
        })
    }
    pub fn preflight(&self, tick: u64, intents: &[Intent]) -> Result<usize> {
        self.prepare(tick, intents).map(|p| p.events.len())
    }
    pub(crate) fn geometry_input_matches(
        &self,
        key: &engine_spine_runtime::GeometryInputKey,
    ) -> bool {
        !self.isolated
            && self
                .backend
                .geometry_input_matches(self.table.pixels_per_unit, key)
    }
    pub(crate) fn geometry_input_key(&self) -> Option<engine_spine_runtime::GeometryInputKey> {
        if self.isolated {
            return None;
        }
        self.backend.geometry_input_key(self.table.pixels_per_unit)
    }
    pub fn geometry(&self) -> Result<engine_spine_runtime::Geometry> {
        if self.isolated {
            return Err(BACKEND);
        }
        self.backend
            .geometry(self.table.pixels_per_unit)
            .map_err(|_| BACKEND)
    }
    pub fn advance(&mut self, tick: u64, intents: &[Intent]) -> Result<&Pose> {
        self.advance_with_skin(tick, intents, None)
    }
    pub fn advance_with_skin(
        &mut self,
        tick: u64,
        intents: &[Intent],
        skin: Option<&str>,
    ) -> Result<&Pose> {
        self.advance_with_skin_measured(tick, intents, skin, None)
    }
    pub(crate) fn advance_with_skin_measured(
        &mut self,
        tick: u64,
        intents: &[Intent],
        skin: Option<&str>,
        measurement: Option<&super::measurement::Measurement>,
    ) -> Result<&Pose> {
        let Prepared {
            state: mut s,
            ops,
            events,
            samples,
            previous_samples,
            revision,
        } = self.prepare(tick, intents)?;
        let evaluated = (|| {
            if let Some(name) = skin {
                self.backend.set_skin(name).map_err(|_| BACKEND)?;
            }
            if !ops.is_empty() {
                // At an exact completed-mix interruption, official update needs
                // the outgoing zero-weight application before it can retire
                // that entry. Otherwise the next transition retains a stale
                // third/fifth entry for another entire mix, every later cycle.
                let completed_mix = self.state.layers.iter().any(|entries| {
                    entries.len() > 1
                        && entries.last().is_some_and(|entry| {
                            s.logical.saturating_sub(entry.mix_start) >= u64::from(entry.mix_ticks)
                        })
                });
                let started = measurement.and_then(|m| m.start());
                let result = if completed_mix {
                    self.backend.sample(&previous_samples)
                } else {
                    self.backend.prepare_sample(&previous_samples)
                };
                if let Some(m) = measurement {
                    m.finish(super::measurement::Stage::Official, started);
                }
                result.map_err(|_| BACKEND)?;
            }
            for op in ops {
                match op {
                    Operation::Play {
                        layer,
                        action,
                        id,
                        mix_ticks,
                    } => {
                        let a = &self.table.actions[action as usize];
                        self.backend
                            .play(
                                layer as u32,
                                &self.table.clips[a.spec.clip as usize].name,
                                a.mask.as_ref(),
                                mix_ticks as f32 / self.table.tick_rate as f32,
                                a.spec.looping,
                                self.table.layers[layer].additive,
                            )
                            .map_err(|_| BACKEND)?;
                        self.backend
                            .tag_current(
                                layer as u32,
                                id,
                                self.table.layers[layer].weight_permille as f32 / 1000.0,
                            )
                            .map_err(|_| BACKEND)?;
                    }
                    Operation::Empty { layer, ticks, id } => {
                        self.backend
                            .fade_out(layer as u32, ticks as f32 / self.table.tick_rate as f32)
                            .map_err(|_| BACKEND)?;
                        self.backend
                            .tag_current(layer as u32, id, 1.0)
                            .map_err(|_| BACKEND)?;
                    }
                    Operation::Clear(l) => {
                        self.backend.clear_track(l as u32).map_err(|_| BACKEND)?
                    }
                }
            }
            let started = measurement.and_then(|m| m.start());
            let result = self.backend.sample(&samples);
            if let Some(m) = measurement {
                m.finish(super::measurement::Stage::Official, started);
            }
            result.map_err(|_| BACKEND)?;
            self.read_pose()
        })();
        let (bones, sockets) = match evaluated {
            Ok(v) => v,
            Err(e) => {
                self.isolated = true;
                return Err(e);
            }
        };
        let live = self.backend.live_entries().map_err(|_| BACKEND)?;
        for entries in &mut s.layers {
            entries.retain(|e| live.contains(&e.id));
        }
        if let Some(m) = measurement {
            m.update(|m| {
                m.evaluated_instances += 1;
                m.active_layers += s.layers.iter().filter(|l| !l.is_empty()).count() as u64;
                m.max_mixing_entries_per_layer = m
                    .max_mixing_entries_per_layer
                    .max(s.layers.iter().map(Vec::len).max().unwrap_or(0));
            });
        }
        self.state = s;
        self.committed = Pose {
            tick,
            revision,
            bones,
            sockets,
            events,
        };
        Ok(&self.committed)
    }
    fn elapsed_us(now: u64, start: u64, speed: u16, rate: u32) -> Result<u64> {
        u64::try_from((now - start) as u128 * speed as u128 * 1000 / rate as u128)
            .map_err(|_| INVALID)
    }
    fn samples(&self, s: &State) -> Result<Vec<EntrySample>> {
        let mut values = Vec::new();
        for entries in &s.layers {
            for e in entries {
                let phase = if let Some(a) = e.action {
                    let a = self.table.actions[a as usize].spec;
                    let us = Self::elapsed_us(
                        s.logical,
                        e.start,
                        a.speed_permille,
                        self.table.tick_rate,
                    )?;
                    let duration = self.table.clips[a.clip as usize].duration_us;
                    let local = if a.looping && duration > 0 {
                        us % duration
                    } else {
                        us.min(duration)
                    };
                    local as f32 / 1_000_000.0
                } else {
                    0.0
                };
                let mix = (s.logical - e.mix_start).min(e.mix_ticks as u64) as f32
                    / self.table.tick_rate as f32;
                values.push(EntrySample {
                    id: e.id,
                    phase,
                    mix,
                });
            }
        }
        Ok(values)
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
