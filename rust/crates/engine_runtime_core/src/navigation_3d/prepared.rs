//! Cell ACK uses real Detour tiles in an isolated preparation owner. Only the
//! original resident group receives translated prepared bytes at commit.
use super::*;
use crate::{runtime_entity_hydration::PreparedRuntimeEntity, world::SpatialWorldState};
use std::sync::atomic::AtomicUsize;
struct Reservation {
    counter: Arc<AtomicUsize>,
    count: usize,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.counter.fetch_sub(self.count, Ordering::Relaxed);
    }
}
struct PreparedSurface {
    id: EntityId,
    binding: NavigationSurface3D,
    base: [f64; 3],
    asset: Arc<CookedNavigation3D>,
    bytes: Vec<Vec<u8>>,
}
pub(crate) struct PreparedNavigation3D {
    epoch: u64,
    reservation: Reservation,
    surfaces: Vec<PreparedSurface>,
    groups: BTreeMap<String, (Option<u64>, [f64; 3])>,
    _staged: Vec<NativeNav>,
}
impl PreparedNavigation3D {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        let roles = &["queuePayload"];
        sample.cpu_vec(&self.surfaces, "navigation", roles);
        sample.cpu_vec(&self._staged, "navigation", roles);
        sample.cpu_arc(&self.reservation.counter, "navigation", roles);
        for key in self.groups.keys() {
            sample.cpu_string(key, "navigation", roles);
        }
        for surface in &self.surfaces {
            sample.cpu_arc(surface.id.storage(), "world", roles);
            sample.cpu_string(&surface.binding.schema_version, "navigation", roles);
            surface
                .binding
                .navigation
                .0
                .observe_memory(sample, "navigation", "queuePayload");
            sample.cpu_vec(&surface.bytes, "navigation", roles);
            for bytes in &surface.bytes {
                sample.cpu_vec(bytes, "navigation", roles);
            }
            // Shared cooked data retains one allocation identity across surfaces.
            sample.cpu_arc(&surface.asset, "navigation", roles);
            let asset = surface.asset.as_ref();
            for text in [
                &asset.schema_version,
                &asset.asset_id,
                &asset.source_digest,
                &asset.recipe,
                &asset.profile.key,
            ] {
                sample.cpu_string(text, "navigation", roles);
            }
            sample.cpu_vec(&asset.tiles, "navigation", roles);
            for tile in &asset.tiles {
                sample.cpu_vec(&tile.data, "navigation", roles);
            }
        }
        sample.partial("cpu.owner.navigation", "cell.nativePrepared.navigation.RustSurfaces+TranslatedTileVecCapacity+SharedCookedAsset+NativeWrapperVec;DetourHeapAndStdNodesAndArcControlBlocksNotMeasured");
    }
}

impl std::fmt::Debug for PreparedNavigation3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedNavigation3D")
            .field("surfaces", &self.surfaces.len())
            .finish()
    }
}
pub(crate) struct ActivatedNavigation3D {
    surfaces: Vec<(EntityId, String, Vec<u64>)>,
    new_groups: Vec<String>,
    changed: std::collections::BTreeSet<String>,
}
fn translated(
    asset: &CookedNavigation3D,
    base: [f64; 3],
    origin: [f64; 3],
) -> ContractResult<Vec<Vec<u8>>> {
    let delta = local(base, origin)?;
    let width = asset.profile.tile_width;
    let tile_delta = [
        (delta[0] / width).round() as i32,
        (delta[2] / width).round() as i32,
    ];
    if (delta[0] - tile_delta[0] as f32 * width).abs() > 0.001
        || (delta[2] - tile_delta[1] as f32 * width).abs() > 0.001
    {
        return Err(invalid(
            "/prepare/grid",
            "Cell navigation placement must align with its tile grid.",
        ));
    }
    asset
        .tiles
        .iter()
        .map(|t| {
            engine_navigation_native::translated_tile(&t.data, delta, tile_delta).map_err(backend)
        })
        .collect()
}
impl NavigationModule {
    pub(crate) fn prepare_cell_entities(
        &mut self,
        world: &World,
        entities: &[PreparedRuntimeEntity],
        poses: &BTreeMap<EntityId, SpatialWorldState>,
        loader: &mut RuntimeAssetLoader,
    ) -> ContractResult<PreparedNavigation3D> {
        let mut surfaces = Vec::new();
        let mut groups = BTreeMap::new();
        let mut staged: BTreeMap<String, NativeNav> = BTreeMap::new();
        for e in entities {
            let Some(spatial) = poses.get(&e.world_id).filter(|s| s.enabled) else {
                continue;
            };
            let value = e.components.iter().find_map(|v| match v {
                crate::archetype::ComponentValue::Dynamic {
                    component_type,
                    value,
                } if component_type == &ComponentTypeId::new("engine.navigation3d") => {
                    Some(crate::character_animation::dynamic_json(value))
                }
                _ => None,
            });
            let Some(value) = value else {
                continue;
            };
            let binding: NavigationSurface3D = serde_json::from_value(value).map_err(backend)?;
            let asset = self.load(loader, &binding)?;
            for i in 0..12 {
                if (spatial.world_matrix[i] - if i == 0 || i == 5 || i == 10 { 1. } else { 0. })
                    .abs()
                    > 1e-5
                {
                    return Err(invalid(
                        "/prepare/transform",
                        "Navigation cell transform supports translation only.",
                    ));
                }
            }
            let base = std::array::from_fn(|i| {
                spatial.anchor[i] + f64::from(spatial.world_matrix[12 + i])
            });
            let key = asset.profile.key.clone();
            if let Some(group) = self.groups.get(&key) {
                if group.profile != asset.profile {
                    return Err(invalid(
                        "/prepare/profile",
                        "Bake profile differs from resident group.",
                    ));
                }
            }
            let entry = groups.entry(key.clone()).or_insert_with(|| {
                self.groups
                    .get(&key)
                    .map_or((None, base), |g| (Some(g.revision), g.origin))
            });
            let origin = entry.1;
            if !staged.contains_key(&key) {
                let mut nav = NativeNav::new(asset.profile.tile_width).map_err(backend)?;
                for old in self
                    .surfaces
                    .values()
                    .filter(|s| s.asset.profile.key == key)
                {
                    for bytes in translated(&old.asset, old.base, origin)? {
                        nav.add_tile(&bytes).map_err(backend)?;
                    }
                }
                staged.insert(key.clone(), nav);
            }
            let bytes = translated(&asset, base, origin)?;
            for b in &bytes {
                staged.get_mut(&key).unwrap().add_tile(b).map_err(backend)?;
            }
            surfaces.push(PreparedSurface {
                id: e.world_id.clone(),
                binding,
                base,
                asset,
                bytes,
            });
        }
        let count = surfaces.iter().map(|s| s.bytes.len()).sum::<usize>();
        let resident = self.surfaces.values().map(|s| s.tiles.len()).sum::<usize>();
        self.prepared_capacity
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |p| {
                p.checked_add(count).filter(|p| resident + *p <= 256)
            })
            .map_err(|_| {
                invalid(
                    "/prepare/capacity",
                    "Navigation tile reservation exhausted.",
                )
            })?;
        Ok(PreparedNavigation3D {
            epoch: world.epoch(),
            reservation: Reservation {
                counter: self.prepared_capacity.clone(),
                count,
            },
            surfaces,
            groups,
            _staged: staged.into_values().collect(),
        })
    }
    pub(crate) fn activate_cell_entities(
        &mut self,
        world: &World,
        p: &PreparedNavigation3D,
    ) -> ContractResult<ActivatedNavigation3D> {
        if p.epoch != world.epoch() || !Arc::ptr_eq(&p.reservation.counter, &self.prepared_capacity)
        {
            return Err(invalid(
                "/prepare/owner",
                "Navigation ACK belongs to a different World or owner.",
            ));
        }
        if self.epoch != Some(world.epoch()) {
            for a in self.agents.values() {
                a.cancel.store(true, Ordering::Release);
            }
            self.worker = None;
            self.surfaces.clear();
            self.groups.clear();
            self.assets.clear();
            self.agents.clear();
            self.epoch = Some(world.epoch());
        }
        for s in &p.surfaces {
            if self
                .groups
                .get(&s.asset.profile.key)
                .is_some_and(|g| g.profile != s.asset.profile)
            {
                return Err(invalid(
                    "/prepare/profile",
                    "Resident profile changed since preparation.",
                ));
            }
        }
        self.serial_revision
            .checked_add((p.surfaces.len() + p.groups.len()) as u64)
            .ok_or_else(|| {
                invalid(
                    "/revision",
                    "Navigation revision exhausted before activation.",
                )
            })?;
        let mut active = ActivatedNavigation3D {
            surfaces: Vec::new(),
            new_groups: Vec::new(),
            changed: Default::default(),
        };
        let result = (|| {
            for s in &p.surfaces {
                let key = s.asset.profile.key.clone();
                if !self.groups.contains_key(&key) {
                    self.groups.insert(
                        key.clone(),
                        Group {
                            profile: s.asset.profile.clone(),
                            origin: p.groups[&key].1,
                            nav: Arc::new(Mutex::new(
                                NativeNav::new(s.asset.profile.tile_width).map_err(backend)?,
                            )),
                            revision: 0,
                            obstacles: vec![],
                        },
                    );
                    active.new_groups.push(key.clone());
                }
                let group = self.groups.get_mut(&key).unwrap();
                let relocated = if group.origin != p.groups[&key].1 {
                    translated(&s.asset, s.base, group.origin)?
                } else {
                    Vec::new()
                };
                let bytes = if relocated.is_empty() {
                    &s.bytes
                } else {
                    &relocated
                };
                let mut nav = group.nav.lock().map_err(backend)?;
                active
                    .surfaces
                    .push((s.id.clone(), key.clone(), Vec::new()));
                for bytes in bytes {
                    let tile = nav.add_tile(bytes).map_err(backend)?;
                    active.surfaces.last_mut().unwrap().2.push(tile);
                }
                nav.set_obstacles(&group.obstacles).map_err(backend)?;
                active.changed.insert(key);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.rollback_cell_entities(active)?;
            return Err(error);
        }
        for key in &active.changed {
            self.serial_revision = self
                .serial_revision
                .checked_add(1)
                .ok_or_else(|| invalid("/revision", "Navigation revision exhausted."))?;
            self.groups.get_mut(key).unwrap().revision = self.serial_revision;
        }
        Ok(active)
    }
    pub(crate) fn rollback_cell_entities(
        &mut self,
        active: ActivatedNavigation3D,
    ) -> ContractResult<()> {
        for (_, key, tiles) in active.surfaces {
            if let Some(group) = self.groups.get_mut(&key) {
                let mut nav = group.nav.lock().map_err(backend)?;
                for tile in tiles {
                    nav.remove_tile(tile).map_err(backend)?;
                }
                self.serial_revision = self
                    .serial_revision
                    .checked_add(1)
                    .ok_or_else(|| invalid("/revision", "Navigation revision exhausted."))?;
                group.revision = self.serial_revision;
            }
        }
        for key in active.new_groups {
            self.groups.remove(&key);
        }
        Ok(())
    }
    fn invalidate_cell_groups(
        &mut self,
        world: &mut World,
        changed: &std::collections::BTreeSet<String>,
    ) {
        let mut writes = Vec::new();
        for (id, a) in &mut self.agents {
            if changed.contains(&a.descriptor.profile) {
                a.cancel.store(true, Ordering::Release);
                a.state.status = NavigationStatus3D::Invalidated;
                a.state.points.clear();
                a.state.off_mesh.clear();
                world
                    .navigation3d_states
                    .insert(id.clone(), Arc::new(a.state.clone()));
                writes.push((id.clone(),crate::archetype::ComponentValue::Dynamic{component_type:ComponentTypeId::new("engine.navigation3d_state"),value:crate::character_animation::runtime_json(serde_json::json!({"revision":a.state.revision,"status":a.state.status,"pointCount":0,"acceptedTick":a.state.accepted_tick}))}));
            }
        }
        world.commit_deferred_components(writes, None);
    }
    pub(crate) fn finish_cell_entities(
        &mut self,
        world: &mut World,
        p: PreparedNavigation3D,
        active: ActivatedNavigation3D,
    ) {
        for s in p.surfaces {
            let tiles = active
                .surfaces
                .iter()
                .find(|(id, _, _)| id == &s.id)
                .unwrap()
                .2
                .clone();
            self.surfaces.insert(
                s.id.clone(),
                Surface {
                    generation: world.entity_generation(&s.id).unwrap(),
                    binding: s.binding,
                    base: s.base,
                    asset: s.asset,
                    tiles,
                },
            );
        }
        self.invalidate_cell_groups(world, &active.changed);
    }
    pub(crate) fn retire_cell_entities(
        &mut self,
        world: &mut World,
        ids: &std::collections::BTreeSet<EntityId>,
    ) -> ContractResult<()> {
        let mut changed = std::collections::BTreeSet::new();
        for id in ids {
            if let Some(s) = self.surfaces.remove(id) {
                let key = s.asset.profile.key.clone();
                let group = self.groups.get_mut(&key).unwrap();
                let mut nav = group.nav.lock().map_err(backend)?;
                for tile in s.tiles {
                    nav.remove_tile(tile).map_err(backend)?;
                }
                self.serial_revision = self
                    .serial_revision
                    .checked_add(1)
                    .ok_or_else(|| invalid("/revision", "Navigation revision exhausted."))?;
                group.revision = self.serial_revision;
                changed.insert(key);
            }
            if let Some(a) = self.agents.remove(id) {
                a.cancel.store(true, Ordering::Release);
            }
            world.navigation3d_states.remove(id);
        }
        self.groups
            .retain(|key, _| self.surfaces.values().any(|s| &s.asset.profile.key == key));
        self.invalidate_cell_groups(world, &changed);
        Ok(())
    }
}

impl NavigationModule {
    pub(crate) fn validate_cell_migration(
        &self,
        world: &World,
        ids: &std::collections::BTreeSet<EntityId>,
    ) -> ContractResult<()> {
        if self.epoch.is_some_and(|e| e != world.epoch()) {
            return Err(invalid(
                "/migration",
                "Navigation owner belongs to another World.",
            ));
        }
        for id in ids {
            if self
                .surfaces
                .get(id)
                .is_some_and(|s| world.entity_generation(id) != Some(s.generation))
                || self
                    .agents
                    .get(id)
                    .is_some_and(|a| world.entity_generation(id) != Some(a.generation))
            {
                return Err(invalid("/migration", "Stale navigation incarnation."));
            }
        }
        Ok(())
    }
}
