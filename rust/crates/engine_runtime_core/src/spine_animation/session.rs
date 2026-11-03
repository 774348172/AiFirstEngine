//! The original FrameLoop's Spine state. Native state is thread confined; callbacks receive copies.
use super::*;
use crate::{ids::EntityId, runtime_package::RuntimePackage, world::World};
use project_runtime_sdk::{ProjectRuntimeAnimatorIntent as Wire, animator::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatorCommand {
    pub entity_id: EntityId,
    pub generation: u64,
    pub intent: Wire,
}
pub type CallbackSnapshots = BTreeMap<String, AnimatorSnapshot>;
#[cfg(feature = "spine")]
type SpineRenderSource = (
    EntityId,
    std::sync::Arc<engine_spine_runtime::Geometry>,
    Vec<SpinePage>,
);
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineObservation {
    pub registry_digest: String,
    pub tick: Option<u64>,
    pub instance_count: usize,
    #[serde(default)]
    pub compiled_table_count: usize,
    pub failed_instance_count: usize,
    pub queued_command_count: usize,
    pub queued_event_count: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub instances: CallbackSnapshots,
}

pub struct SpineModule {
    pub(crate) measurement: super::measurement::Measurement,
    pub registry_digest: String,
    #[cfg(feature = "spine")]
    world_epoch: u64,
    #[cfg(feature = "spine")]
    registry: CookedSpineRegistry,
    #[cfg(feature = "spine")]
    bindings: Vec<(EntityId, SpineAnimatorBinding)>,
    #[cfg(feature = "spine")]
    tables: Vec<std::rc::Rc<control::ControlTable>>,
    #[cfg(feature = "spine")]
    live: BTreeMap<EntityId, Live>,
    pending: Vec<AnimatorCommand>,
    tick: Option<u64>,
    snapshots: CallbackSnapshots,
    events: Vec<AnimatorEvent>,
    pub failed_instances: usize,
}
impl std::fmt::Debug for SpineModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpineModule")
            .field("registry_digest", &self.registry_digest)
            .field("tick", &self.tick)
            .field("instances", &self.snapshots.len())
            .finish()
    }
}
#[cfg(feature = "spine")]
struct Live {
    generation: u64,
    controller_index: usize,
    controller: control::Controller,
    isolated: bool,
}

impl SpineModule {
    pub fn begin_measurement_frame(&self, enabled: bool) {
        self.measurement.begin_frame(enabled);
    }
    pub fn cpu_measurement(&self) -> Option<super::measurement::SpineCpuFrameSample> {
        self.measurement.sample()
    }

    #[cfg(feature = "spine")]
    pub fn load(package: &RuntimePackage, world: &World) -> Result<Self, String> {
        let registry = package
            .spine_animator_registry
            .clone()
            .ok_or("animator.runtime.registry_uninitialized")?;
        registry.validate()?;
        let mut tables = Vec::new();
        let mut compiled = BTreeMap::<Vec<u8>, std::rc::Rc<control::ControlTable>>::new();
        for c in &registry.controllers {
            // Author bindings have distinct controller asset names. That name
            // is resolved by this module, not consumed by ControlTable::compile.
            // Equal definitions in this registry can share all immutable native
            // data while each binding still creates its own mutable instance.
            let mut definition = c.clone();
            definition.asset_id.clear();
            let identity = serde_json::to_vec(&definition).map_err(|e| e.to_string())?;
            if let Some(table) = compiled.get(&identity) {
                tables.push(table.clone());
                continue;
            }
            let source = registry.validated_asset(&package.package_dir, &c.skeleton_asset_id)?;
            let a = registry
                .assets
                .iter()
                .find(|a| a.asset_id == c.skeleton_asset_id)
                .ok_or("animator.binding.unknown_skeleton")?;
            let table = c.compile(&source, registry.tick_rate, a.pixels_per_unit)?;
            compiled.insert(identity, table.clone());
            tables.push(table);
        }
        let mut bindings = Vec::new();
        for e in &package.active_scene.entities {
            if let Some(b) = SpineAnimatorBinding::from_entity(e)? {
                if b.registry_digest != registry.registry_digest
                    || registry
                        .controllers
                        .get(b.controller_index as usize)
                        .is_none_or(|c| c.asset_id != b.controller_id)
                {
                    return Err("animator.binding.registry_identity".into());
                }
                bindings.push((e.id.as_str().into(), b));
            }
        }
        if bindings.len() > 1024 {
            return Err("animator.runtime.instance_capacity".into());
        }
        let mut module = Self {
            registry_digest: registry.registry_digest.clone(),
            world_epoch: world.epoch(),
            registry,
            bindings,
            tables,
            live: BTreeMap::new(),
            pending: Vec::new(),
            tick: None,
            snapshots: BTreeMap::new(),
            events: Vec::new(),
            failed_instances: 0,
            measurement: Default::default(),
        };
        module.refresh(world)?;
        Ok(module)
    }
    #[cfg(not(feature = "spine"))]
    pub fn load(_package: &RuntimePackage, _world: &World) -> Result<Self, String> {
        Err("spine.package.backend_not_compiled".into())
    }

    #[cfg(feature = "spine")]
    pub fn refresh(&mut self, world: &World) -> Result<(), String> {
        if self.world_epoch != world.epoch() {
            self.live.clear();
            self.snapshots.clear();
            self.events.clear();
            self.pending.clear();
            self.tick = None;
            self.failed_instances = 0;
            self.world_epoch = world.epoch();
        }
        self.live.retain(|id, l| {
            world.entity_generation(id) == Some(l.generation)
                && world.entity(id).is_some_and(|m| m.alive && m.enabled)
        });
        self.snapshots.retain(|id, s| {
            world.entity_generation(&id.as_str().into()) == Some(s.entity_generation)
                && self.live.contains_key(&id.as_str().into())
        });
        self.events.retain(|e| {
            world.entity_generation(&e.entity_id.as_str().into()) == Some(e.generation)
                && self.live.contains_key(&e.entity_id.as_str().into())
        });
        self.pending.retain(|e| {
            world.entity_generation(&e.entity_id) == Some(e.generation)
                && self.live.contains_key(&e.entity_id)
        });
        for (id, b) in &self.bindings {
            let Some(generation) = world
                .entity_generation(id)
                .filter(|_| world.entity(id).is_some_and(|m| m.alive && m.enabled))
            else {
                continue;
            };
            if !self.live.contains_key(id) {
                let index = b.controller_index as usize;
                let controller = self.tables[index].instance().map_err(|e| e.0.to_string())?;
                self.snapshots.insert(
                    id.to_string(),
                    snapshot(
                        id,
                        generation,
                        &self.registry.controllers[index],
                        controller.pose(),
                    ),
                );
                self.live.insert(
                    id.clone(),
                    Live {
                        generation,
                        controller_index: index,
                        controller,
                        isolated: false,
                    },
                );
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "spine"))]
    pub fn refresh(&mut self, _world: &World) -> Result<(), String> {
        Ok(())
    }
    pub fn callback_snapshots(&mut self, world: &World) -> Result<CallbackSnapshots, String> {
        let started = self.measurement.start();
        let result = self.callback_snapshots_inner(world);
        self.measurement
            .finish(super::measurement::Stage::Control, started);
        result
    }
    fn callback_snapshots_inner(&mut self, world: &World) -> Result<CallbackSnapshots, String> {
        self.refresh(world)?;
        let mut snapshots = self.snapshots.clone();
        for event in self.events.drain(..) {
            if let Some(s) = snapshots.get_mut(&event.entity_id) {
                s.events.push(event);
            }
        }
        Ok(snapshots)
    }
    pub fn snapshot(&self, id: &str) -> Option<&AnimatorSnapshot> {
        self.snapshots.get(id)
    }
    pub fn tick_index(&self) -> Option<u64> {
        self.tick
    }
    pub fn observe(
        &self,
        level: crate::render_contract::DiagnosticLevel,
    ) -> Option<SpineObservation> {
        use crate::render_contract::DiagnosticLevel;
        (level != DiagnosticLevel::Off).then(|| SpineObservation {
            registry_digest: self.registry_digest.clone(),
            tick: self.tick,
            instance_count: self.snapshots.len(),
            compiled_table_count: {
                #[cfg(feature = "spine")]
                {
                    self.tables
                        .iter()
                        .map(std::rc::Rc::as_ptr)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                }
                #[cfg(not(feature = "spine"))]
                {
                    0
                }
            },
            failed_instance_count: self.failed_instances,
            queued_command_count: self.pending.len(),
            queued_event_count: self.events.len(),
            instances: if level == DiagnosticLevel::Trace {
                self.snapshots.clone()
            } else {
                BTreeMap::new()
            },
        })
    }
    pub(crate) fn queue(&mut self, commands: Vec<AnimatorCommand>) {
        self.pending.extend(commands);
    }

    #[cfg(feature = "spine")]
    pub fn preflight(
        &self,
        world: &World,
        tick: u64,
        commands: &[AnimatorCommand],
    ) -> Result<(), String> {
        let started = self.measurement.start();
        let result = self.preflight_inner(world, tick, commands);
        self.measurement
            .finish(super::measurement::Stage::Control, started);
        result
    }
    #[cfg(feature = "spine")]
    fn preflight_inner(
        &self,
        world: &World,
        tick: u64,
        commands: &[AnimatorCommand],
    ) -> Result<(), String> {
        if self.world_epoch != world.epoch() {
            return Err("animator.runtime.stale_handle".into());
        }
        if self.pending.len() + commands.len() > 4096 {
            return Err("animator.runtime.command_capacity".into());
        }
        let tick = tick.max(self.tick.map_or(tick, |t| t.saturating_add(1)));
        for cmd in self.pending.iter().chain(commands) {
            if cmd.generation == 0
                || world.entity_generation(&cmd.entity_id) != Some(cmd.generation)
                || self
                    .live
                    .get(&cmd.entity_id)
                    .is_none_or(|l| l.generation != cmd.generation || l.isolated)
                || !world
                    .entity(&cmd.entity_id)
                    .is_some_and(|m| m.alive && m.enabled)
            {
                return Err("animator.runtime.stale_handle".into());
            }
        }
        let mut count = self.events.len();
        if count > 4096 {
            return Err("animator.runtime.event_capacity".into());
        }
        for (id, l) in &self.live {
            if l.isolated {
                continue;
            }
            let (intents, _) = self.resolve(
                l,
                self.pending
                    .iter()
                    .chain(commands)
                    .filter(|c| &c.entity_id == id),
            )?;
            count += l
                .controller
                .preflight(tick, &intents)
                .map_err(|e| e.0.to_string())?;
            if count > 4096 {
                return Err("animator.runtime.event_capacity".into());
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "spine"))]
    pub fn preflight(
        &self,
        _world: &World,
        _tick: u64,
        _commands: &[AnimatorCommand],
    ) -> Result<(), String> {
        Err("spine.package.backend_not_compiled".into())
    }
    #[cfg(feature = "spine")]
    fn resolve<'a>(
        &self,
        l: &Live,
        commands: impl Iterator<Item = &'a AnimatorCommand>,
    ) -> Result<(Vec<control::Intent>, Option<String>), String> {
        use control::Intent as I;
        let c = &self.registry.controllers[l.controller_index];
        let mut intents = Vec::new();
        let mut skin = None;
        for (command_index, cmd) in commands.enumerate() {
            if command_index >= 64 {
                return Err("animator.runtime.command_capacity".into());
            }
            let index = |n: &str| {
                c.actions
                    .iter()
                    .position(|a| a.name == n)
                    .map(|i| i as u16)
                    .ok_or("animator.binding.unknown_action".to_string())
            };
            let layer = |n: &str| {
                c.layers
                    .iter()
                    .position(|l| l.name == n)
                    .map(|i| i as u8)
                    .ok_or("animator.binding.unknown_layer".to_string())
            };
            let intent = match &cmd.intent {
                Wire::Play { name } => I::Play(index(name)?),
                Wire::Restart { name } => I::Restart(index(name)?),
                Wire::SetPaused { paused } => I::SetPaused(*paused),
                Wire::Resume {} => I::Resume,
                Wire::SetBool { name, value } => I::SetBool(
                    c.parameters
                        .iter()
                        .position(|p| matches!(p,schema::Parameter::Bool{name:n,..} if n==name))
                        .ok_or("animator.binding.unknown_parameter")? as u8,
                    *value,
                ),
                Wire::SetTrigger { name } => I::SetTrigger(
                    c.parameters
                        .iter()
                        .position(|p| matches!(p,schema::Parameter::Trigger{name:n} if n==name))
                        .ok_or("animator.binding.unknown_parameter")? as u8,
                ),
                Wire::ClearLayer { layer: n } => I::ClearLayer(layer(n)?),
                Wire::FadeOutLayer {
                    layer: n,
                    duration_ticks,
                } => I::FadeOutLayer {
                    layer: layer(n)?,
                    ticks: *duration_ticks,
                },
                Wire::SetSkin { name } => {
                    let a = self
                        .registry
                        .assets
                        .iter()
                        .find(|a| a.asset_id == c.skeleton_asset_id)
                        .ok_or("animator.binding.unknown_skeleton")?;
                    if !a.skins.contains(name) {
                        return Err("animator.binding.unknown_skin".into());
                    }
                    skin = Some(name.clone());
                    continue;
                }
            };
            intents.push(intent);
        }
        if intents.len() > 64 {
            return Err("animator.runtime.command_capacity".into());
        }
        Ok((intents, skin))
    }
    #[cfg(feature = "spine")]
    pub fn advance(&mut self, world: &World, tick: u64) -> Result<(), String> {
        let started = self.measurement.start();
        let result = self.advance_inner(world, tick);
        self.measurement
            .finish(super::measurement::Stage::Control, started);
        result
    }
    #[cfg(feature = "spine")]
    fn advance_inner(&mut self, world: &World, tick: u64) -> Result<(), String> {
        self.refresh(world)?;
        if self.tick == Some(tick) {
            return if self.pending.is_empty() {
                Ok(())
            } else {
                Err("animator.runtime.invalid_phase".into())
            };
        }
        if self.tick.is_some_and(|last| tick < last) {
            return Err("animator.runtime.invalid_phase".into());
        }
        self.preflight(world, tick, &[])?;
        let mut resolved = BTreeMap::new();
        for (id, l) in &self.live {
            if !l.isolated {
                resolved.insert(
                    id.clone(),
                    self.resolve(l, self.pending.iter().filter(|c| &c.entity_id == id))?,
                );
            }
        }
        for (id, (intents, skin)) in resolved {
            let l = self.live.get_mut(&id).unwrap();
            match l.controller.advance_with_skin_measured(
                tick,
                &intents,
                skin.as_deref(),
                Some(&self.measurement),
            ) {
                Ok(pose) => {
                    let c = &self.registry.controllers[l.controller_index];
                    self.snapshots
                        .insert(id.to_string(), snapshot(&id, l.generation, c, pose));
                    for e in &pose.events {
                        self.events.push(AnimatorEvent {
                            entity_id: id.to_string(),
                            generation: l.generation,
                            play_id: e.play_id,
                            track: e.track,
                            loop_index: e.loop_index,
                            source_event_id: e.source_event_id.clone(),
                            source_ordinal: e.source_ordinal,
                            tick: e.tick,
                            crossing_us: e.crossing_us,
                            payload: AnimatorEventPayload {
                                name: e.payload.name.clone(),
                                int_value: e.payload.int_value,
                                float_value: e.payload.float_value,
                                string_value: e.payload.string_value.clone(),
                                volume: e.payload.volume,
                                balance: e.payload.balance,
                            },
                        });
                    }
                }
                Err(_) => {
                    l.isolated = true;
                    self.failed_instances += 1;
                }
            }
        }
        self.events.sort_by_key(|e| {
            (
                e.tick,
                e.crossing_us,
                e.track,
                e.source_ordinal,
                e.play_id,
                e.entity_id.clone(),
            )
        });
        self.pending.clear();
        self.tick = Some(tick);
        self.measurement.update(|sample| {
            sample.first_fixed_tick.get_or_insert(tick);
            sample.last_fixed_tick = Some(tick);
        });
        Ok(())
    }
    #[cfg(not(feature = "spine"))]
    pub fn advance(&mut self, _world: &World, _tick: u64) -> Result<(), String> {
        Err("spine.package.backend_not_compiled".into())
    }
    #[cfg(feature = "spine")]
    pub(crate) fn render_sources(&self) -> Result<Vec<SpineRenderSource>, String> {
        let started = self.measurement.start();
        let result = self.render_sources_inner();
        self.measurement
            .finish(super::measurement::Stage::Vertices, started);
        result
    }
    #[cfg(feature = "spine")]
    fn render_sources_inner(&self) -> Result<Vec<SpineRenderSource>, String> {
        let mut geometries = std::collections::HashMap::new();
        // Common identical poses can compare their complete input directly.
        // Keep the original hash table for other keys; both caches end here.
        let mut first_geometry: Option<(
            engine_spine_runtime::GeometryInputKey,
            std::sync::Arc<engine_spine_runtime::Geometry>,
        )> = None;
        self.live
            .iter()
            .filter(|(_, live)| !live.isolated)
            .map(|(id, live)| {
                let controller = &self.registry.controllers[live.controller_index];
                let asset = self
                    .registry
                    .assets
                    .iter()
                    .find(|a| a.asset_id == controller.skeleton_asset_id)
                    .ok_or("animator.binding.unknown_skeleton")?;
                let geometry = if let Some((_, geometry)) = first_geometry
                    .as_ref()
                    .filter(|(key, _)| live.controller.geometry_input_matches(key))
                {
                    std::sync::Arc::clone(geometry)
                } else {
                    let key = live.controller.geometry_input_key();
                    if let Some(g) = key.as_ref().and_then(|k| {
                        first_geometry
                            .as_ref()
                            .filter(|(first, _)| first == k)
                            .map(|(_, geometry)| geometry)
                            .or_else(|| geometries.get(k))
                    }) {
                        std::sync::Arc::clone(g)
                    } else {
                        let g = std::sync::Arc::new(
                            live.controller.geometry().map_err(|e| e.0.to_string())?,
                        );
                        if let Some(key) = key {
                            if first_geometry.is_none() {
                                first_geometry = Some((key, std::sync::Arc::clone(&g)));
                            } else {
                                geometries.insert(key, std::sync::Arc::clone(&g));
                            }
                        }
                        g
                    }
                };
                self.measurement.update(|m| {
                    m.geometry_instances += 1;
                    m.vertices += geometry.vertices.len() as u64;
                    m.indices += geometry.indices.len() as u64;
                });
                Ok((id.clone(), geometry, asset.pages.clone()))
            })
            .collect()
    }
    #[cfg(feature = "spine")]
    pub fn geometry(&self, id: &str) -> Result<engine_spine_runtime::Geometry, String> {
        self.live
            .get(&id.into())
            .ok_or("animator.runtime.stale_handle")?
            .controller
            .geometry()
            .map_err(|e| e.0.into())
    }
}
#[cfg(feature = "spine")]
fn snapshot(
    _id: &EntityId,
    generation: u64,
    c: &schema::CookedSpineController,
    pose: &control::Pose,
) -> AnimatorSnapshot {
    AnimatorSnapshot {
        entity_generation: generation,
        tick: pose.tick,
        pose_revision: pose.revision,
        sockets: c
            .sockets
            .iter()
            .zip(&pose.sockets)
            .map(|(s, p)| {
                (
                    s.name.clone(),
                    AnimatorSocket {
                        entity_generation: generation,
                        tick: pose.tick,
                        pose_revision: pose.revision,
                        position: [p.x, p.y],
                        rotation_degrees: p.rotation,
                        source_asset_id: c.skeleton_asset_id.clone(),
                    },
                )
            })
            .collect(),
        events: Vec::new(),
    }
}

#[cfg(all(test, feature = "spine"))]
mod tests {
    use super::*;
    pub(super) fn fixture() -> (SpineModule, World) {
        let source = engine_spine_runtime::import::validate(
            include_str!("../../../engine_spine_runtime/tests/fixtures/probe.json"),
            include_str!("../../../engine_spine_runtime/tests/fixtures/probe.atlas"),
            &BTreeMap::from([("probe.png".into(), "texture-probe".into())]),
        )
        .unwrap();
        let c = schema::CookedSpineController {
            asset_id: "controller".into(),
            skeleton_asset_id: "hero".into(),
            default_action_index: 0,
            parameters: Vec::new(),
            layers: vec![schema::Layer {
                name: "base".into(),
                additive: false,
                mask_index: None,
            }],
            masks: Vec::new(),
            actions: vec![schema::Action {
                name: "idle".into(),
                source_animation_index: source
                    .animations
                    .iter()
                    .position(|n| n == "motion")
                    .unwrap() as u16,
                layer_index: 0,
                playback: schema::Playback::Loop,
                speed_permille: 1000,
                requested_mix_in_seconds: 0.,
                mix_in_ticks: 0,
                requested_fade_out_seconds: None,
                fade_out_ticks: None,
                on_complete: schema::Completion::Hold,
                mask_index: None,
            }],
            rules: Vec::new(),
            sockets: Vec::new(),
        };
        let table = c.compile(&source, 60, 100.).unwrap();
        let mut registry = CookedSpineRegistry::empty(60);
        registry.controllers.push(c);
        // Runtime owner tests already have a validated source/table. Only fields used
        // for named skin resolution are needed in this non-public in-memory fixture.
        registry.assets.push(CookedSpineAsset {
            asset_id: "hero".into(),
            source_digest: String::new(),
            payload_path: String::new(),
            payload_digest: String::new(),
            atlas_path: String::new(),
            atlas_digest: String::new(),
            pages: Vec::new(),
            animations: source.animations,
            skins: source.skins,
            bones: source.bones,
            slots: source.slots,
            events: Vec::new(),
            limits: BTreeMap::new(),
            pixels_per_unit: 100.,
        });
        let mut world = World::new();
        world.spawn_entity(
            "robot".into(),
            "Robot",
            "entity",
            true,
            crate::components::Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        );
        let binding = SpineAnimatorBinding {
            controller_id: "controller".into(),
            controller_index: 0,
            registry_digest: String::new(),
        };
        let mut module = SpineModule {
            registry_digest: String::new(),
            world_epoch: world.epoch(),
            registry,
            bindings: vec![("robot".into(), binding)],
            tables: vec![table],
            live: BTreeMap::new(),
            pending: Vec::new(),
            tick: None,
            snapshots: BTreeMap::new(),
            events: Vec::new(),
            failed_instances: 0,
            measurement: Default::default(),
        };
        module.refresh(&world).unwrap();
        (module, world)
    }
    #[test]
    fn spine374_d_pending_events_and_skin_commands_share_preflight_budgets() {
        let (mut module, world) = fixture();
        module.advance(&world, 1).unwrap();
        assert_eq!(module.events.len(), 1);
        module.events = vec![module.events[0].clone(); 4096];
        let before = module.snapshot("robot").unwrap().clone();
        assert_eq!(
            module.preflight(&world, 16, &[]).unwrap_err(),
            "animator.runtime.event_capacity"
        );
        assert_eq!(
            module.advance(&world, 16).unwrap_err(),
            "animator.runtime.event_capacity"
        );
        assert_eq!(module.snapshot("robot"), Some(&before));
        assert_eq!(module.tick_index(), Some(1));
        module.callback_snapshots(&world).unwrap();
        let cmd = AnimatorCommand {
            entity_id: "robot".into(),
            generation: world.entity_generation(&"robot".into()).unwrap(),
            intent: Wire::SetSkin {
                name: "default".into(),
            },
        };
        assert!(
            module
                .preflight(&world, 2, &vec![cmd; 65])
                .unwrap_err()
                .contains("command_capacity")
        );
    }
    #[test]
    fn spine374_d_duplicate_tick_is_noop_and_reverse_tick_preserves_pending() {
        let (mut module, world) = fixture();
        module.advance(&world, 1).unwrap();
        let before = module.snapshot("robot").unwrap().clone();
        module.advance(&world, 1).unwrap();
        assert_eq!(module.snapshot("robot"), Some(&before));
        assert_eq!(module.events.len(), 1);
        module.queue(vec![AnimatorCommand {
            entity_id: "robot".into(),
            generation: world.entity_generation(&"robot".into()).unwrap(),
            intent: Wire::Resume {},
        }]);
        assert!(module.advance(&world, 0).is_err());
        assert!(module.advance(&world, 1).is_err());
        assert_eq!(module.pending.len(), 1);
        module.advance(&world, 2).unwrap();
        assert_eq!(module.pending.len(), 0);
        assert_eq!(module.snapshot("robot").unwrap().pose_revision, 2);
    }
    #[test]
    fn spine374_d_focus_callback_cannot_submit_animator_or_world_writes() {
        use crate::project_runtime_session::*;
        struct Session;
        impl ProjectRuntimeSession for Session {
            fn session_id(&self) -> &str {
                "phase"
            }
            fn handle_aui_actions(
                &mut self,
                _: ProjectRuntimeSessionContext<'_>,
                _: ProjectAuiActionBatch<'_>,
            ) -> ProjectRuntimeSessionOutput {
                ProjectRuntimeSessionOutput::no_op()
            }
            fn fixed_update(
                &mut self,
                _: ProjectRuntimeSessionContext<'_>,
            ) -> ProjectRuntimeSessionOutput {
                ProjectRuntimeSessionOutput::no_op()
            }
            fn lifecycle(
                &mut self,
                c: ProjectRuntimeSessionContext<'_>,
                _: ProjectRuntimeLifecycleEvent,
            ) -> ProjectRuntimeSessionOutput {
                let mut b = ProjectRuntimeMutationBuffer::new();
                let mut t = crate::components::Transform::identity();
                t.local_position.x = 77.;
                b.write_transform("robot".into(), t);
                b.animator_command(AnimatorCommand {
                    entity_id: "robot".into(),
                    generation: c.world.entity_generation(&"robot".into()).unwrap(),
                    intent: Wire::Resume {},
                });
                ProjectRuntimeSessionOutput::applied(b)
            }
        }
        let (mut module, mut world) = fixture();
        world.insert_transform("robot".into(), crate::components::Transform::identity());
        let r = execute_project_runtime_session_stage_with_spine(
            &mut Session,
            ProjectRuntimeSessionStage::FocusChanged { focused: false },
            1,
            crate::runtime_time::TimeContext::from_delta(1, 1. / 60., false),
            &mut world,
            &[],
            ProjectRuntimeSessionReportLevel::Summary,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut Vec::new(),
            &mut None,
            None,
            Some(&mut module),
            None,
        );
        assert!(r.terminal_fault);
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d == "animator.runtime.invalid_phase")
        );
        assert_eq!(
            world.transform(&"robot".into()).unwrap().local_position.x,
            0.
        );
        assert!(module.pending.is_empty());
    }
}

#[cfg(all(test, feature = "spine"))]
#[path = "session/gate_f_tests.rs"]
mod gate_f_tests;
