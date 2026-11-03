//! World partition declarations and deterministic demand planning. Loading and
//! activation remain owned by RuntimeInstanceLoader's existing cell transaction.
use crate::spatial_contract::{ContractResult, SpatialAssetRef, SpatialContractError};
pub use project_runtime_sdk::ProjectRuntimeStreamingSource3D as StreamingSource3D;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(path: &str, message: &str) -> SpatialContractError {
    SpatialContractError::new("streaming.invalid", path, message)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldCell3D {
    pub id: String,
    pub prefab: SpatialAssetRef,
    pub min: [f64; 3],
    pub max: [f64; 3],
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default = "required_render")]
    pub render_required: bool,
}
fn required_render() -> bool {
    true
}
impl WorldCell3D {
    pub fn contains(&self, p: [f64; 3]) -> bool {
        (0..3).all(|i| p[i] >= self.min[i] && p[i] <= self.max[i])
    }
    fn distance(&self, p: [f64; 3]) -> f64 {
        (0..3)
            .map(|i| (self.min[i] - p[i]).max(0.).max(p[i] - self.max[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldStreaming3D {
    pub schema_version: String,
    pub world_id: String,
    pub revision: u64,
    pub cells: Vec<WorldCell3D>,
    #[serde(default)]
    pub sources: Vec<StreamingSource3D>,
    pub max_resident_cells: usize,
    pub max_loading_cells: usize,
    pub unload_margin: f64,
}
impl WorldStreaming3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "world-streaming3d.v1"
            || self.world_id.trim().is_empty()
            || self.world_id.len() > 128
            || self.revision == 0
            || self.cells.is_empty()
            || self.cells.len() > 4096
            || !(1..=64).contains(&self.max_resident_cells)
            || !(1..=8).contains(&self.max_loading_cells)
            || self.max_loading_cells > self.max_resident_cells
            || !self.unload_margin.is_finite()
            || !(0.0..=100_000.).contains(&self.unload_margin)
            || self.sources.len() > 16
        {
            return Err(invalid(
                "/",
                "Expected a bounded, versioned world partition and budgets.",
            ));
        }
        let mut ids = BTreeSet::new();
        for cell in &self.cells {
            if cell.id.trim().is_empty()
                || cell.id.len() > 128
                || !ids.insert(cell.id.as_str())
                || cell.prefab.0.asset_type != "prefab"
                || cell.prefab.0.id.trim().is_empty()
                || cell.prefab.0.guid.as_ref().is_none_or(|v| v.is_empty())
                || cell.dependencies.len() > 16
                || (0..3).any(|i| {
                    !cell.min[i].is_finite()
                        || !cell.max[i].is_finite()
                        || cell.min[i] >= cell.max[i]
                })
            {
                return Err(invalid("/cells", "Cell identities, prefab GUIDs, finite bounds and dependency budgets are required."));
            }
        }
        let map: BTreeMap<_, _> = self.cells.iter().map(|c| (c.id.as_str(), c)).collect();
        fn visit<'a>(
            id: &'a str,
            map: &BTreeMap<&'a str, &'a WorldCell3D>,
            pending: &mut BTreeSet<&'a str>,
            done: &mut BTreeMap<&'a str, usize>,
        ) -> ContractResult<usize> {
            if let Some(depth) = done.get(id) {
                return Ok(*depth);
            }
            if pending.len() >= 64 || !pending.insert(id) {
                return Err(invalid(
                    "/dependencies",
                    "Strong cell dependency cycle or depth greater than 64.",
                ));
            }
            let cell = map
                .get(id)
                .ok_or_else(|| invalid("/dependencies", "Unknown required cell."))?;
            let mut unique = BTreeSet::new();
            let mut depth = 1;
            for dep in &cell.dependencies {
                if !unique.insert(dep.as_str()) {
                    return Err(invalid("/dependencies", "Duplicate required cell."));
                }
                depth = depth.max(1 + visit(dep, map, pending, done)?);
                if depth > 64 {
                    return Err(invalid(
                        "/dependencies",
                        "Strong cell dependency depth greater than 64.",
                    ));
                }
            }
            pending.remove(id);
            done.insert(id, depth);
            Ok(depth)
        }
        let mut done = BTreeMap::new();
        for cell in &self.cells {
            visit(&cell.id, &map, &mut BTreeSet::new(), &mut done)?;
        }
        let mut sources = BTreeSet::new();
        if self
            .sources
            .iter()
            .any(|s| !s.validate() || !sources.insert(&s.id))
        {
            return Err(invalid(
                "/sources",
                "Sources need unique identities and finite activation/prefetch ranges.",
            ));
        }
        Ok(())
    }
    /// Pure snapshot planning; callers may execute this off the World thread.
    pub fn plan(&self, resident: &BTreeSet<String>) -> ContractResult<StreamingPlan3D> {
        self.validate()?;
        let map: BTreeMap<_, _> = self.cells.iter().map(|c| (c.id.clone(), c)).collect();
        let mut desired = BTreeMap::<String, StreamingDemand3D>::new();
        for cell in &self.cells {
            for source in &self.sources {
                let distance = cell.distance(source.position);
                let margin = if resident.contains(&cell.id) {
                    self.unload_margin
                } else {
                    0.
                };
                if distance <= source.prefetch_radius + margin {
                    let demand = desired.entry(cell.id.clone()).or_insert(StreamingDemand3D {
                        activate: false,
                        priority: source.priority,
                        distance,
                    });
                    demand.activate |= distance <= source.activation_radius;
                    demand.priority = demand.priority.max(source.priority);
                    demand.distance = demand.distance.min(distance);
                }
            }
        }
        let initial = desired.clone();
        fn require(
            id: &str,
            active: bool,
            priority: i32,
            distance: f64,
            map: &BTreeMap<String, &WorldCell3D>,
            out: &mut BTreeMap<String, StreamingDemand3D>,
            seen: &mut BTreeSet<String>,
        ) {
            if !seen.insert(id.to_owned()) {
                return;
            }
            for dep in &map[id].dependencies {
                let d = out.entry(dep.clone()).or_insert(StreamingDemand3D {
                    activate: active,
                    priority,
                    distance,
                });
                d.activate |= active;
                d.priority = d.priority.max(priority);
                d.distance = d.distance.min(distance);
                require(dep, active, priority, distance, map, out, seen);
            }
        }
        for (id, d) in initial {
            require(
                &id,
                d.activate,
                d.priority,
                d.distance,
                &map,
                &mut desired,
                &mut BTreeSet::new(),
            );
        }
        let mut order = Vec::new();
        let mut emitted = BTreeSet::new();
        fn emit(
            id: &str,
            map: &BTreeMap<String, &WorldCell3D>,
            wanted: &BTreeMap<String, StreamingDemand3D>,
            out: &mut Vec<String>,
            seen: &mut BTreeSet<String>,
        ) {
            if !seen.insert(id.to_owned()) {
                return;
            }
            for dep in &map[id].dependencies {
                if wanted.contains_key(dep) {
                    emit(dep, map, wanted, out, seen);
                }
            }
            out.push(id.to_owned());
        }
        let mut ranked: Vec<_> = desired.keys().cloned().collect();
        ranked.sort_by(|a, b| {
            desired[b]
                .activate
                .cmp(&desired[a].activate)
                .then(desired[b].priority.cmp(&desired[a].priority))
                .then(desired[a].distance.total_cmp(&desired[b].distance))
                .then(a.cmp(b))
        });
        for id in ranked {
            emit(&id, &map, &desired, &mut order, &mut emitted);
        }
        Ok(StreamingPlan3D { desired, order })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct StreamingDemand3D {
    pub activate: bool,
    pub priority: i32,
    pub distance: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct StreamingPlan3D {
    pub desired: BTreeMap<String, StreamingDemand3D>,
    pub order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StreamingGuard3D {
    pub owner: String,
}
impl StreamingGuard3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.owner.trim().is_empty() || self.owner.len() > 256 {
            return Err(invalid(
                "/owner",
                "Expected a world streaming owner identity.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReadyRegion {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
impl crate::world::World {
    /// The swept capsule envelope must remain covered by the union of ready
    /// partitions. Checking only its endpoint would permit tunnelling across gaps.
    pub(crate) fn streaming_guard_allows_path(
        &self,
        id: &crate::ids::EntityId,
        from: [f64; 3],
        to: [f64; 3],
        extent: [f64; 3],
    ) -> bool {
        let Some(crate::archetype::ComponentValue::Dynamic { value, .. }) = self.component_value(
            id,
            &crate::components::ComponentTypeId::new("engine.streaming_guard3d"),
        ) else {
            return true;
        };
        let Ok(g) = serde_json::from_value::<StreamingGuard3D>(
            crate::character_animation::dynamic_json(&value),
        ) else {
            return false;
        };
        let owner = crate::ids::EntityId::from(g.owner);
        let Some((generation, regions)) = self.streaming_ready.get(&owner) else {
            return false;
        };
        if self.entity_generation(&owner) != Some(*generation)
            || self
                .component_value(
                    &owner,
                    &crate::components::ComponentTypeId::new("engine.world_streaming3d"),
                )
                .is_none()
        {
            return false;
        }
        (0..9).all(|corner| {
            let offset: [f64; 3] = std::array::from_fn(|i| {
                if corner == 8 {
                    0.
                } else if corner & (1 << i) == 0 {
                    -extent[i]
                } else {
                    extent[i]
                }
            });
            let start: [f64; 3] = std::array::from_fn(|i| from[i] + offset[i]);
            let delta: [f64; 3] = std::array::from_fn(|i| to[i] - from[i]);
            let mut intervals: Vec<(f64, f64)> = regions
                .iter()
                .filter_map(|r| {
                    let mut lo: f64 = 0.;
                    let mut hi: f64 = 1.;
                    for i in 0..3 {
                        if delta[i].abs() < 1e-12 {
                            if start[i] < r.min[i] || start[i] > r.max[i] {
                                return None;
                            }
                        } else {
                            let a = (r.min[i] - start[i]) / delta[i];
                            let b = (r.max[i] - start[i]) / delta[i];
                            lo = lo.max(a.min(b));
                            hi = hi.min(a.max(b));
                        }
                    }
                    (hi >= lo).then_some((lo, hi))
                })
                .collect();
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut covered = 0.;
            for (lo, hi) in intervals {
                if lo > covered + 1e-9 {
                    return false;
                }
                covered = covered.max(hi);
                if covered >= 1. - 1e-9 {
                    return true;
                }
            }
            false
        })
    }
}

pub(crate) fn prepare_sources(
    world: &crate::world::World,
    overlay: &BTreeMap<
        (crate::ids::EntityId, crate::components::ComponentTypeId),
        crate::archetype::ComponentValue,
    >,
    id: &crate::ids::EntityId,
    sources: Vec<StreamingSource3D>,
) -> ContractResult<crate::archetype::ComponentValue> {
    use crate::{
        archetype::ComponentValue,
        character_animation::{dynamic_json, runtime_json},
        components::ComponentTypeId,
    };
    let ty = ComponentTypeId::new("engine.world_streaming3d");
    let value = overlay
        .get(&(id.clone(), ty.clone()))
        .cloned()
        .or_else(|| world.component_value(id, &ty))
        .ok_or_else(|| invalid("/owner", "World streaming binding is absent."))?;
    let ComponentValue::Dynamic { value, .. } = value else {
        return Err(invalid("/owner", "Expected world streaming binding."));
    };
    let mut binding: WorldStreaming3D = serde_json::from_value(dynamic_json(&value))
        .map_err(|_| invalid("/owner", "Malformed world streaming binding."))?;
    binding.sources = sources;
    binding.validate()?;
    Ok(ComponentValue::Dynamic {
        component_type: ty,
        value: runtime_json(serde_json::to_value(binding).unwrap()),
    })
}
