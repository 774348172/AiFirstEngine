use super::*;
use crate::{navigation_3d::NavigationModule, physics_3d::PhysicsModule};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    navigation_3d::{ActivatedNavigation3D, PreparedNavigation3D},
    physics_3d::{ActivatedPhysics3D, PreparedPhysics3D},
};
#[derive(Debug)]
pub(super) struct PreparedSimulation {
    poses: BTreeMap<SourceEntityId, crate::world::SpatialWorldState>,
    #[cfg(not(target_arch = "wasm32"))]
    physics: Option<PreparedPhysics3D>,
    #[cfg(not(target_arch = "wasm32"))]
    navigation: Option<PreparedNavigation3D>,
}
impl PreparedSimulation {
    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        for id in self.poses.keys() {
            sample.cpu_arc(id.storage(), "world", &["queuePayload"]);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(physics) = &self.physics {
                physics.observe_memory(sample);
            }
            if let Some(navigation) = &self.navigation {
                navigation.observe_memory(sample);
            }
        }
        sample.partial(
            "cpu.owner.queues",
            "cell.nativePrepared.poseIds;StdPoseMapNodesAndNativeLibraryHeapNotMeasured",
        );
    }
}

pub(super) struct ActivatedSimulation {
    #[cfg(not(target_arch = "wasm32"))]
    physics: Option<ActivatedPhysics3D>,
    #[cfg(not(target_arch = "wasm32"))]
    navigation: Option<ActivatedNavigation3D>,
}
type Owners<'a> = Option<(&'a mut PhysicsModule, &'a mut NavigationModule)>;
pub(super) fn prepare(
    world: &World,
    prepared: &PreparedRuntimeEntities,
    loader: &mut RuntimeAssetLoader,
    domains: &BTreeSet<CellDomain>,
    owners: Owners<'_>,
) -> Result<Option<PreparedSimulation>, CellError> {
    let physics = domains.contains(&CellDomain::Physics);
    let navigation = domains.contains(&CellDomain::Navigation);
    if !physics && !navigation {
        return Ok(None);
    }
    let Some((p, n)) = owners else {
        return Err(CellError::UnsupportedDomain(if physics {
            CellDomain::Physics
        } else {
            CellDomain::Navigation
        }));
    };
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (p, n, world, prepared, loader);
        Err(CellError::UnsupportedDomain(CellDomain::Physics))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let poses = world
            .evaluate_prepared_spatial(&prepared.entities)
            .map_err(|e| CellError::Hydration(e.message))?;
        let physics = if physics {
            Some(
                p.prepare_cell_entities(world, &prepared.entities, &poses, loader)
                    .map_err(|e| CellError::Simulation(e.to_string()))?,
            )
        } else {
            None
        };
        let navigation = if navigation {
            Some(
                n.prepare_cell_entities(world, &prepared.entities, &poses, loader)
                    .map_err(|e| CellError::Simulation(e.to_string()))?,
            )
        } else {
            None
        };
        Ok(Some(PreparedSimulation {
            poses,
            physics,
            navigation,
        }))
    }
}
pub(super) fn activate(
    world: &World,
    prepared: &PreparedRuntimeEntities,
    ack: &PreparedSimulation,
    owners: Owners<'_>,
) -> Result<ActivatedSimulation, CellError> {
    let current = world
        .evaluate_prepared_spatial(&prepared.entities)
        .map_err(|e| CellError::Hydration(e.message))?;
    if current.len() != ack.poses.len()
        || current.iter().any(|(id, s)| {
            ack.poses.get(id).is_none_or(|p| {
                p.anchor != s.anchor || p.world_matrix != s.world_matrix || p.enabled != s.enabled
            })
        })
    {
        return Err(CellError::ParentChanged);
    }
    let Some((p, n)) = owners else {
        return Err(CellError::NotReady);
    };
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (p, n);
        Err(CellError::UnsupportedDomain(CellDomain::Physics))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let physics = ack
            .physics
            .as_ref()
            .map(|ack| p.activate_cell_entities(world, ack))
            .transpose()
            .map_err(|e| CellError::Simulation(e.to_string()))?;
        let navigation = match ack
            .navigation
            .as_ref()
            .map(|ack| n.activate_cell_entities(world, ack))
            .transpose()
        {
            Ok(n) => n,
            Err(error) => {
                if let Some(active) = physics {
                    p.rollback_cell_entities(active)
                        .map_err(|e| CellError::Simulation(e.to_string()))?;
                }
                return Err(CellError::Simulation(error.to_string()));
            }
        };
        Ok(ActivatedSimulation {
            physics,
            navigation,
        })
    }
}
pub(super) fn finish(
    world: &mut World,
    ack: PreparedSimulation,
    active: ActivatedSimulation,
    owners: Owners<'_>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (p, n) = owners.expect("Simulation owners validated before World commit");
        if let (Some(ack), Some(active)) = (ack.physics, active.physics) {
            p.finish_cell_entities(world, ack, active);
        }
        if let (Some(ack), Some(active)) = (ack.navigation, active.navigation) {
            n.finish_cell_entities(world, ack, active);
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (world, ack, active, owners);
}
pub(super) fn retire(
    world: &mut World,
    ids: &BTreeSet<SourceEntityId>,
    owners: Owners<'_>,
) -> Result<(), CellError> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((p, n)) = owners {
        p.retire_cell_entities(world, ids)
            .map_err(|e| CellError::Simulation(e.to_string()))?;
        n.retire_cell_entities(world, ids)
            .map_err(|e| CellError::Simulation(e.to_string()))?;
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (world, ids, owners);
    Ok(())
}
