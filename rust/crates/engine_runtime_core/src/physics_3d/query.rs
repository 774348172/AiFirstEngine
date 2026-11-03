//! Borrow the existing solver; no query state or native handles escape this call.
use super::*;
use project_runtime_abi::ProjectRuntimeAbiStatus as Status;
use project_runtime_sdk::physics_query3d::*;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "query/tests.rs"]
mod tests;

impl PhysicsModule {
    pub(crate) fn query_batch3d(
        &self,
        world: &crate::world::World,
        batch: &PhysicsQueryBatch3D,
    ) -> Result<PhysicsQueryResult3D, Status> {
        if !batch.is_valid() {
            return Err(Status::INVALID_ARGUMENT);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = world;
            Err(Status::UNSUPPORTED)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(tick) = self.query_tick_for_world(world.epoch())? else {
                return Ok(PhysicsQueryResult3D {
                    tick: None,
                    hits: Vec::new(),
                });
            };
            for query in &batch.queries {
                let (PhysicsQuery3D::Raycast { origin, .. } | PhysicsQuery3D::Sweep { origin, .. }) =
                    query;
                self.local(*origin).map_err(|_| Status::INVALID_ARGUMENT)?;
            }
            let mut hits = Vec::with_capacity(batch.queries.len());
            for query in &batch.queries {
                let hit = match query {
                    PhysicsQuery3D::Raycast {
                        origin,
                        displacement,
                    } => self.raycast(&PhysicsRay3D {
                        origin: *origin,
                        displacement: *displacement,
                    }),
                    PhysicsQuery3D::Sweep {
                        origin,
                        displacement,
                        shape,
                        rotation_xyzw,
                    } => {
                        let shape = match shape {
                            PhysicsQueryShape3D::Box { half_extents } => CollisionShape3D::Box {
                                half_extents: *half_extents,
                            },
                            PhysicsQueryShape3D::Sphere { radius } => {
                                CollisionShape3D::Sphere { radius: *radius }
                            }
                            PhysicsQueryShape3D::Capsule {
                                half_height,
                                radius,
                            } => CollisionShape3D::Capsule {
                                half_height: *half_height,
                                radius: *radius,
                            },
                        };
                        self.sweep(
                            &shape,
                            &PhysicsRay3D {
                                origin: *origin,
                                displacement: *displacement,
                            },
                            *rotation_xyzw,
                        )
                    }
                }
                .map_err(|_| Status::FAILED)?;
                hits.push(
                    hit.filter(|hit| {
                        world.entity_generation(&hit.entity.as_str().into()) == Some(hit.generation)
                    })
                    .map(|hit| PhysicsQueryHit3D {
                        entity: hit.entity,
                        generation: hit.generation,
                        fraction: hit.fraction,
                        distance: hit.distance,
                        position: hit.position,
                        normal: hit.normal,
                    }),
                );
            }
            Ok(PhysicsQueryResult3D {
                tick: Some(tick),
                hits,
            })
        }
    }
}
