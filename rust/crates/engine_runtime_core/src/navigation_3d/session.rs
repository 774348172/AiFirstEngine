use super::contract::invalid;
use super::control::{component, NavigationControl3D};
use super::*;
use crate::{
    components::ComponentTypeId, ids::EntityId, query::QuerySpec,
    runtime_asset_loader::RuntimeAssetLoader, spatial_contract::ContractResult, world::World,
};
use engine_navigation_native::{NativeNav, NativePath, NavError};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, Weak,
    },
    thread::JoinHandle,
};
fn backend(e: impl std::fmt::Debug) -> crate::spatial_contract::SpatialContractError {
    invalid("/backend", format!("{e:?}"))
}
#[path = "prepared.rs"]
mod prepared;
pub(crate) use prepared::{ActivatedNavigation3D, PreparedNavigation3D};
fn ids(world: &World, ty: &str) -> Vec<EntityId> {
    world
        .query_entities(&QuerySpec::all([ComponentTypeId::new(ty)]))
        .into_iter()
        .filter(|id| world.spatial_world(id).is_some_and(|s| s.enabled))
        .collect()
}
fn absolute(world: &World, id: &EntityId) -> ContractResult<[f64; 3]> {
    let s = world
        .spatial_world(id)
        .ok_or_else(|| invalid("/transform", "Missing spatial entity."))?;
    Ok(std::array::from_fn(|i| {
        s.anchor[i] + f64::from(s.world_matrix[12 + i])
    }))
}
fn local(p: [f64; 3], origin: [f64; 3]) -> ContractResult<[f32; 3]> {
    let p = std::array::from_fn::<_, 3, _>(|i| p[i] - origin[i]);
    if p.iter().any(|v| !v.is_finite() || v.abs() > 32768.) {
        return Err(invalid(
            "/origin",
            "Navigation query exceeds the local 32km envelope.",
        ));
    }
    Ok(p.map(|v| v as f32))
}
struct Surface {
    generation: u64,
    binding: NavigationSurface3D,
    base: [f64; 3],
    asset: Arc<CookedNavigation3D>,
    tiles: Vec<u64>,
}
struct Group {
    profile: AgentProfile3D,
    origin: [f64; 3],
    nav: Arc<Mutex<NativeNav>>,
    revision: u64,
    obstacles: Vec<[f32; 6]>,
}
struct Agent {
    generation: u64,
    descriptor: NavigationAgent3D,
    revision: u64,
    nav_revision: u64,
    deadline: u64,
    cancel: Arc<AtomicBool>,
    state: NavigationState3D,
    cursor: usize,
    motor_revision: u64,
    steering: [f32; 2],
    steering_elapsed: f64,
    steering_cursor: Option<usize>,
}
struct Job {
    entity: EntityId,
    generation: u64,
    revision: u64,
    nav_revision: u64,
    nav: Arc<Mutex<NativeNav>>,
    from: [f32; 3],
    to: [f32; 3],
    extent: [f32; 3],
    cancel: Arc<AtomicBool>,
}
struct Completed {
    entity: EntityId,
    generation: u64,
    revision: u64,
    nav_revision: u64,
    result: Result<NativePath, NavError>,
}
struct Worker {
    sender: Option<mpsc::SyncSender<Job>>,
    receiver: mpsc::Receiver<Completed>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    fn new() -> ContractResult<Self> {
        let (tx, rx) = mpsc::sync_channel::<Job>(64);
        let (done_tx, done_rx) = mpsc::sync_channel(64);
        let thread = std::thread::Builder::new()
            .name("engine-navigation3d".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    if job.cancel.load(Ordering::Acquire) {
                        continue;
                    }
                    let result = job
                        .nav
                        .lock()
                        .map_err(|_| NavError::Backend)
                        .and_then(|mut nav| nav.find_path(job.from, job.to, job.extent));
                    if !job.cancel.load(Ordering::Acquire) {
                        let _ = done_tx.try_send(Completed {
                            entity: job.entity,
                            generation: job.generation,
                            revision: job.revision,
                            nav_revision: job.nav_revision,
                            result,
                        });
                    }
                }
            })
            .map_err(backend)?;
        Ok(Self {
            sender: Some(tx),
            receiver: done_rx,
            thread: Some(thread),
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[derive(Default)]
pub struct NavigationModule {
    prepared_capacity: Arc<std::sync::atomic::AtomicUsize>,
    epoch: Option<u64>,
    surfaces: BTreeMap<EntityId, Surface>,
    groups: BTreeMap<String, Group>,
    assets: BTreeMap<String, Weak<CookedNavigation3D>>,
    agents: BTreeMap<EntityId, Agent>,
    worker: Option<Worker>,
    serial_revision: u64,
}
impl std::fmt::Debug for NavigationModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavigationModule")
            .field("surfaces", &self.surfaces.len())
            .field("groups", &self.groups.len())
            .field("agents", &self.agents.len())
            .finish()
    }
}
impl NavigationModule {
    pub fn resident_surface_count(&self) -> usize {
        self.surfaces.len()
    }
    pub fn pending_count(&self) -> usize {
        self.agents
            .values()
            .filter(|a| a.state.status == NavigationStatus3D::Pending)
            .count()
    }
    fn load(
        &mut self,
        loader: &mut RuntimeAssetLoader,
        binding: &NavigationSurface3D,
    ) -> ContractResult<Arc<CookedNavigation3D>> {
        binding.validate()?;
        let record = loader
            .asset_index()
            .resolve(&binding.navigation.0)
            .map_err(backend)?;
        if record.asset_id != binding.navigation.0.id {
            return Err(invalid("/asset", "Navigation id/GUID mismatch."));
        }
        let hash = record
            .hash
            .clone()
            .ok_or_else(|| invalid("/asset", "Navigation asset needs a hash."))?;
        if let Some(asset) = self.assets.get(&hash).and_then(Weak::upgrade) {
            return Ok(asset);
        }
        let handle = loader.load(&binding.navigation.0).map_err(backend)?;
        let result = loader
            .get_asset_bytes(&handle)
            .ok_or_else(|| invalid("/asset", "Navigation bytes unavailable."))
            .and_then(|bytes| {
                if crate::canonical_digest::sha256_prefixed(&bytes) != hash {
                    return Err(invalid("/asset", "Navigation hash mismatch."));
                }
                let asset: CookedNavigation3D = serde_json::from_slice(&bytes).map_err(backend)?;
                asset.validate()?;
                if asset.asset_id != binding.navigation.0.id {
                    return Err(invalid("/asset", "Navigation payload identity mismatch."));
                }
                for tile in &asset.tiles {
                    engine_navigation_native::validate_tile(&tile.data).map_err(backend)?;
                }
                Ok(Arc::new(asset))
            });
        loader.release(&handle).map_err(backend)?;
        let asset = result?;
        self.assets.insert(hash, Arc::downgrade(&asset));
        Ok(asset)
    }
    fn sync_surfaces(
        &mut self,
        world: &World,
        loader: &mut RuntimeAssetLoader,
    ) -> ContractResult<()> {
        let surface_ids = ids(world, "engine.navigation3d");
        if surface_ids.len() > 256 {
            return Err(invalid(
                "/capacity",
                "Resident navigation surface budget exceeded.",
            ));
        }
        let mut descriptors = BTreeMap::new();
        for id in surface_ids {
            let binding: NavigationSurface3D =
                serde_json::from_value(component(world, &id, "engine.navigation3d").unwrap())
                    .map_err(backend)?;
            binding.validate()?;
            let spatial = world.spatial_world(&id).unwrap();
            let matrix = spatial.world_matrix;
            for (i, value) in matrix.iter().enumerate().take(12) {
                let expected = if i == 0 || i == 5 || i == 10 { 1. } else { 0. };
                if (value - expected).abs() > 1e-5 {
                    return Err(invalid(
                        "/surface/transform",
                        "Cooked navigation supports translation only; bake rotation/scale offline.",
                    ));
                }
            }
            descriptors.insert(id.clone(), (binding, absolute(world, &id)?));
        }
        let retired = self
            .surfaces
            .iter()
            .filter(|(id, s)| {
                !descriptors.get(*id).is_some_and(|(b, p)| {
                    b == &s.binding
                        && p == &s.base
                        && world.entity_generation(id) == Some(s.generation)
                })
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in retired {
            let surface = self.surfaces.remove(&id).unwrap();
            let group = self.groups.get_mut(&surface.asset.profile.key).unwrap();
            let mut nav = group.nav.lock().map_err(backend)?;
            for tile in surface.tiles {
                nav.remove_tile(tile).map_err(backend)?;
            }
            self.serial_revision = self
                .serial_revision
                .checked_add(1)
                .ok_or_else(|| invalid("/revision", "Navigation revision exhausted."))?;
            group.revision = self.serial_revision;
        }
        self.groups
            .retain(|key, _| self.surfaces.values().any(|s| &s.asset.profile.key == key));
        for (id, (binding, base)) in descriptors {
            if self.surfaces.contains_key(&id) {
                continue;
            }
            let asset = self.load(loader, &binding)?;
            let key = asset.profile.key.clone();
            if let Some(group) = self.groups.get(&key) {
                if group.profile != asset.profile {
                    return Err(invalid(
                        "/profile",
                        "Same profile key cannot consume different bake parameters.",
                    ));
                }
            } else {
                self.groups.insert(
                    key.clone(),
                    Group {
                        profile: asset.profile.clone(),
                        origin: base,
                        nav: Arc::new(Mutex::new(
                            NativeNav::new(asset.profile.tile_width).map_err(backend)?,
                        )),
                        revision: 0,
                        obstacles: Vec::new(),
                    },
                );
            }
            let group = self.groups.get_mut(&key).unwrap();
            let delta = local(base, group.origin)?;
            let width = asset.profile.tile_width;
            let tile_delta = [
                (delta[0] / width).round() as i32,
                (delta[2] / width).round() as i32,
            ];
            if (delta[0] - tile_delta[0] as f32 * width).abs() > 0.001
                || (delta[2] - tile_delta[1] as f32 * width).abs() > 0.001
            {
                return Err(invalid(
                    "/surface/translation",
                    "Navigation X/Z placement must align with its tile grid.",
                ));
            }
            let mut nav = group.nav.lock().map_err(backend)?;
            let mut tiles = Vec::new();
            for tile in &asset.tiles {
                let bytes =
                    engine_navigation_native::translated_tile(&tile.data, delta, tile_delta)
                        .map_err(backend)?;
                match nav.add_tile(&bytes) {
                    Ok(tile) => tiles.push(tile),
                    Err(error) => {
                        for tile in tiles {
                            let _ = nav.remove_tile(tile);
                        }
                        return Err(backend(error));
                    }
                }
            }
            self.serial_revision = self
                .serial_revision
                .checked_add(1)
                .ok_or_else(|| invalid("/revision", "Navigation revision exhausted."))?;
            group.revision = self.serial_revision;
            nav.set_obstacles(&group.obstacles).map_err(backend)?;
            self.surfaces.insert(
                id.clone(),
                Surface {
                    generation: world.entity_generation(&id).unwrap(),
                    binding,
                    base,
                    asset,
                    tiles,
                },
            );
        }
        let obstacle_ids = ids(world, "engine.navigation_obstacle3d");
        if obstacle_ids.len() > 128 {
            return Err(invalid(
                "/capacity",
                "Dynamic navigation obstacle budget exceeded.",
            ));
        }
        for group in self.groups.values_mut() {
            let mut boxes = Vec::new();
            for id in &obstacle_ids {
                let obstacle: NavigationObstacle3D = serde_json::from_value(
                    component(world, id, "engine.navigation_obstacle3d").unwrap(),
                )
                .map_err(backend)?;
                obstacle.validate()?;
                let center = local(absolute(world, id)?, group.origin)?;
                let m = world.spatial_world(id).unwrap().world_matrix;
                let extent = std::array::from_fn::<_, 3, _>(|i| {
                    (0..3)
                        .map(|j| m[j * 4 + i].abs() * obstacle.half_extents[j])
                        .sum::<f32>()
                });
                boxes.push(std::array::from_fn(|i| {
                    if i < 3 {
                        center[i] - extent[i]
                    } else {
                        center[i - 3] + extent[i - 3]
                    }
                }));
            }
            if group.obstacles != boxes {
                group
                    .nav
                    .lock()
                    .map_err(backend)?
                    .set_obstacles(&boxes)
                    .map_err(backend)?;
                group.obstacles = boxes;
                self.serial_revision = self
                    .serial_revision
                    .checked_add(1)
                    .ok_or_else(|| invalid("/revision", "Navigation revision exhausted."))?;
                group.revision = self.serial_revision;
            }
        }
        self.assets.retain(|_, asset| asset.strong_count() > 0);
        Ok(())
    }
    pub fn advance(
        &mut self,
        world: &mut World,
        loader: &mut RuntimeAssetLoader,
        tick: u64,
        dt: f32,
    ) -> ContractResult<()> {
        if !dt.is_finite() || dt <= 0. || dt > 0.1 {
            return Err(invalid(
                "/tick",
                "Navigation requires a fixed delta in (0,0.1].",
            ));
        }
        if self.epoch != Some(world.epoch()) {
            for agent in self.agents.values() {
                agent.cancel.store(true, Ordering::Release);
            }
            self.worker = None;
            self.surfaces.clear();
            self.groups.clear();
            self.assets.clear();
            self.agents.clear();
            world.navigation3d_states.clear();
            self.epoch = Some(world.epoch());
        }
        world.propagate_spatial();
        self.sync_surfaces(world, loader)?;
        let agent_ids = ids(world, "engine.navigation_agent3d")
            .into_iter()
            .filter(|id| {
                component(world, id, "engine.motor3d").is_none_or(|m| m["enabled"] != false)
            })
            .collect::<Vec<_>>();
        if agent_ids.len() > 2048 {
            return Err(invalid("/capacity", "Navigation agent budget exceeded."));
        }
        self.agents.retain(|id, a| {
            let keep = agent_ids.binary_search(id).is_ok()
                && world.entity_generation(id) == Some(a.generation);
            if !keep {
                a.cancel.store(true, Ordering::Release);
            }
            keep
        });
        world
            .navigation3d_states
            .retain(|id, _| self.agents.contains_key(id));
        if let Some(worker) = &self.worker {
            for done in worker.receiver.try_iter() {
                if let Some(agent) = self.agents.get_mut(&done.entity).filter(|a| {
                    a.generation == done.generation
                        && a.revision == done.revision
                        && a.nav_revision == done.nav_revision
                        && a.state.status == NavigationStatus3D::Pending
                        && !a.cancel.load(Ordering::Acquire)
                }) {
                    if self
                        .groups
                        .get(&agent.descriptor.profile)
                        .is_none_or(|g| g.revision != done.nav_revision)
                    {
                        agent.state.status = NavigationStatus3D::Invalidated;
                        continue;
                    }
                    if tick > agent.deadline {
                        agent.state.status = NavigationStatus3D::TimedOut;
                        continue;
                    }
                    match done.result {
                        Ok(path) => {
                            let origin = self.groups[&agent.descriptor.profile].origin;
                            agent.state.points = path
                                .points
                                .iter()
                                .map(|p| std::array::from_fn(|i| origin[i] + f64::from(p[i])))
                                .collect();
                            agent.state.off_mesh = path.flags.iter().map(|f| f & 4 != 0).collect();
                            agent.state.status = NavigationStatus3D::Following;
                            agent.state.accepted_tick = tick;
                            agent.cursor = 0;
                            agent.steering_cursor = None;
                        }
                        Err(e) => {
                            agent.state.status = match e {
                                NavError::Unreachable => NavigationStatus3D::Unreachable,
                                NavError::NotResident => NavigationStatus3D::NotResident,
                                _ => NavigationStatus3D::Capacity,
                            }
                        }
                    }
                }
            }
        }
        for id in &agent_ids {
            let descriptor: NavigationAgent3D =
                serde_json::from_value(component(world, id, "engine.navigation_agent3d").unwrap())
                    .map_err(backend)?;
            descriptor.validate()?;
            if component(world, id, "engine.motor3d").is_none() {
                return Err(invalid(
                    "/agent/motor",
                    "Navigation requires CharacterMotor3D.",
                ));
            }
            let generation = world.entity_generation(id).unwrap();
            let agent = self.agents.entry(id.clone()).or_insert_with(|| Agent {
                generation,
                descriptor: descriptor.clone(),
                revision: 0,
                nav_revision: 0,
                deadline: 0,
                cancel: Arc::new(AtomicBool::new(false)),
                state: NavigationState3D {
                    revision: 0,
                    status: NavigationStatus3D::Cancelled,
                    points: Vec::new(),
                    off_mesh: Vec::new(),
                    accepted_tick: 0,
                },
                cursor: 0,
                motor_revision: 0,
                steering: [0.; 2],
                steering_elapsed: 0.,
                steering_cursor: None,
            });
            if agent.descriptor != descriptor {
                agent.cancel.store(true, Ordering::Release);
                agent.descriptor = descriptor;
                agent.revision = 0;
                agent.state.status = NavigationStatus3D::Invalidated;
            }
            let control = component(world, id, "engine.navigation3d_control")
                .map(serde_json::from_value::<NavigationControl3D>)
                .transpose()
                .map_err(backend)?;
            if let Some(control) = control.filter(|c| c.revision != agent.revision) {
                agent.cancel.store(true, Ordering::Release);
                agent.cancel = Arc::new(AtomicBool::new(false));
                agent.revision = control.revision;
                agent.state = NavigationState3D {
                    revision: control.revision,
                    status: NavigationStatus3D::Cancelled,
                    points: Vec::new(),
                    off_mesh: Vec::new(),
                    accepted_tick: 0,
                };
                agent.cursor = 0;
                if let Some(target) = control.target {
                    if !(1..=600).contains(&control.deadline_ticks)
                        || target.iter().any(|v| !v.is_finite())
                    {
                        return Err(invalid("/goal", "Invalid path deadline/target."));
                    }
                    if let Some(group) = self.groups.get(&agent.descriptor.profile) {
                        let from = local(absolute(world, id)?, group.origin)?;
                        let to = local(target, group.origin)?;
                        agent.nav_revision = group.revision;
                        agent.deadline = tick.saturating_add(u64::from(control.deadline_ticks));
                        if self.worker.is_none() {
                            self.worker = Some(Worker::new()?);
                        }
                        let extent = [
                            group.profile.radius.max(0.25),
                            group.profile.height,
                            group.profile.radius.max(0.25),
                        ];
                        let job = Job {
                            entity: id.clone(),
                            generation,
                            revision: agent.revision,
                            nav_revision: group.revision,
                            nav: group.nav.clone(),
                            from,
                            to,
                            extent,
                            cancel: agent.cancel.clone(),
                        };
                        agent.state.status = if self
                            .worker
                            .as_ref()
                            .unwrap()
                            .sender
                            .as_ref()
                            .unwrap()
                            .try_send(job)
                            .is_ok()
                        {
                            NavigationStatus3D::Pending
                        } else {
                            NavigationStatus3D::Capacity
                        };
                    } else {
                        agent.state.status = NavigationStatus3D::NotResident;
                    }
                }
            }
            if matches!(
                agent.state.status,
                NavigationStatus3D::Pending
                    | NavigationStatus3D::Following
                    | NavigationStatus3D::LinkRequired
            ) {
                if self
                    .groups
                    .get(&agent.descriptor.profile)
                    .is_none_or(|g| g.revision != agent.nav_revision)
                {
                    agent.cancel.store(true, Ordering::Release);
                    agent.state.status = NavigationStatus3D::Invalidated;
                } else if agent.state.status == NavigationStatus3D::Pending
                    && tick >= agent.deadline
                {
                    agent.cancel.store(true, Ordering::Release);
                    agent.state.status = NavigationStatus3D::TimedOut;
                }
            }
        }
        let positions = agent_ids
            .iter()
            .map(|id| Ok((id.clone(), absolute(world, id)?)))
            .collect::<ContractResult<BTreeMap<_, _>>>()?;
        for id in agent_ids {
            let agent = self.agents.get_mut(&id).unwrap();
            let position = positions[&id];
            if agent.state.status == NavigationStatus3D::Following {
                agent.steering_elapsed += f64::from(dt);
                while agent.cursor < agent.state.points.len() {
                    let target = agent.state.points[agent.cursor];
                    let distance = ((target[0] - position[0]).powi(2)
                        + (target[2] - position[2]).powi(2))
                    .sqrt();
                    if distance > f64::from(agent.descriptor.arrival_distance) {
                        break;
                    }
                    if agent.state.off_mesh.get(agent.cursor) == Some(&true) {
                        agent.state.status = NavigationStatus3D::LinkRequired;
                        break;
                    }
                    agent.cursor += 1;
                }
                if agent.cursor == agent.state.points.len() {
                    agent.state.status = NavigationStatus3D::Arrived;
                } else if agent.state.status == NavigationStatus3D::Following {
                    let target = agent.state.points[agent.cursor];
                    let offset = [
                        (target[0] - position[0]) as f32,
                        (target[2] - position[2]) as f32,
                    ];
                    let distance = offset[0].hypot(offset[1]);
                    if agent.steering_cursor != Some(agent.cursor) || agent.steering_elapsed >= 0.1
                    {
                        let mut direction = offset;
                        for d in &mut direction {
                            *d /= distance.max(1e-6);
                        }
                        let mut neighbours = positions
                            .iter()
                            .filter(|(other, _)| *other != &id)
                            .map(|(other, p)| {
                                let dx = (position[0] - p[0]) as f32;
                                let dz = (position[2] - p[2]) as f32;
                                (dx * dx + dz * dz, other, dx, dz)
                            })
                            .filter(|(d, _, _, _)| {
                                *d < agent.descriptor.avoidance_radius.powi(2) && *d > 1e-8
                            })
                            .collect::<Vec<_>>();
                        neighbours.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
                        for (d, _, dx, dz) in neighbours
                            .into_iter()
                            .take(agent.descriptor.max_neighbours as usize)
                        {
                            let distance = d.sqrt();
                            let strength = (1.
                                - distance / agent.descriptor.avoidance_radius.max(1e-6))
                                * 0.75;
                            direction[0] += dx / distance * strength;
                            direction[1] += dz / distance * strength;
                        }
                        let length = direction[0].hypot(direction[1]).max(1.);
                        agent.steering = direction.map(|d| d / length);
                        agent.steering_elapsed = if agent.steering_cursor == Some(agent.cursor) {
                            agent.steering_elapsed % 0.1
                        } else {
                            0.
                        };
                        agent.steering_cursor = Some(agent.cursor);
                    }
                    let step = (agent.descriptor.speed * dt).min(distance);
                    let displacement = [agent.steering[0] * step, 0., agent.steering[1] * step];
                    // A project motor request in this tick explicitly takes precedence over steering.
                    let external = component(world, &id, "engine.physics3d_control")
                        .and_then(|v| v["movement"][0].as_u64())
                        .unwrap_or(0);
                    if external <= agent.motor_revision {
                        // Avoidance may point across a blocked polygon even
                        // while the accepted path stays on the resident mesh.
                        let group = &self.groups[&agent.descriptor.profile];
                        let from = local(position, group.origin)?;
                        let to = std::array::from_fn(|i| from[i] + displacement[i]);
                        let extent = [
                            group.profile.radius.max(0.25),
                            group.profile.height,
                            group.profile.radius.max(0.25),
                        ];
                        let constrained = group
                            .nav
                            .lock()
                            .map_err(backend)?
                            .move_on_surface(from, to, extent);
                        let displacement = match constrained {
                            Ok(next) => [next[0] - from[0], 0., next[2] - from[2]],
                            Err(error) => {
                                agent.state.status = match error {
                                    NavError::NotResident => NavigationStatus3D::NotResident,
                                    _ => NavigationStatus3D::Capacity,
                                };
                                [0.; 3]
                            }
                        };
                        let value = crate::physics_3d::prepare_control(
                            world,
                            &BTreeMap::new(),
                            &id,
                            project_runtime_sdk::ProjectRuntimeSpatialIntent::MotorMove {
                                displacement,
                            },
                        )?;
                        let crate::archetype::ComponentValue::Dynamic { value, .. } = value else {
                            unreachable!()
                        };
                        let control = crate::character_animation::dynamic_json(&value);
                        agent.motor_revision = control["revision"].as_u64().unwrap();
                        world
                            .try_insert_dynamic_component(
                                id.clone(),
                                ComponentTypeId::new("engine.physics3d_control"),
                                value,
                            )
                            .map_err(|e| invalid("/motor", e.message))?;
                    } else {
                        agent.motor_revision = external;
                    }
                }
            } else {
                agent.steering_cursor = None;
                agent.steering_elapsed = 0.;
            }
            if world
                .navigation3d_states
                .get(&id)
                .is_none_or(|old| **old != agent.state)
            {
                let compact = serde_json::json!({"revision":agent.state.revision,"status":agent.state.status,"pointCount":agent.state.points.len(),"acceptedTick":agent.state.accepted_tick,"nextPoint":agent.state.points.get(agent.cursor),"linkExit":if agent.state.status==NavigationStatus3D::LinkRequired{agent.state.points.get(agent.cursor+1)}else{None}});
                world
                    .try_insert_dynamic_component(
                        id.clone(),
                        ComponentTypeId::new("engine.navigation3d_state"),
                        crate::character_animation::runtime_json(compact),
                    )
                    .map_err(|e| invalid("/state", e.message))?;
                world
                    .navigation3d_states
                    .insert(id, Arc::new(agent.state.clone()));
            }
        }
        Ok(())
    }
}
impl World {
    pub fn navigation3d_state(&self, id: &EntityId) -> Option<Arc<NavigationState3D>> {
        self.navigation3d_states.get(id).cloned()
    }
}
