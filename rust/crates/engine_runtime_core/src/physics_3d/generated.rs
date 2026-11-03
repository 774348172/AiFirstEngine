//! Reversible static replacement inside the original prepared/Jolt owner.
use super::*;
use crate::archetype::ComponentValue;
#[cfg(test)]
#[path = "generated_tests.rs"]
mod tests;

pub(crate) struct PreparedStaticCollision {
    epoch: u64,
    id: EntityId,
    generation: u64,
    before: Option<ComponentValue>,
    old_handle: Option<BodyId>,
    pose: Pose,
    body: Option<PreparedBody>,
    value: Option<ComponentValue>,
    reservation: Reservation,
}
impl std::fmt::Debug for PreparedStaticCollision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedStaticCollision")
            .field("id", &self.id)
            .finish()
    }
}
#[derive(Debug)]
pub(crate) struct GeneratedStaticBinding {
    epoch: u64,
    id: EntityId,
    generation: u64,
    owner: Arc<AtomicUsize>,
    handle: Option<BodyId>,
    // Enabled bodies retain their descriptor in the original BodyEntry, not here.
    disabled: Option<Body3D>,
}
struct ActivatedStaticCollision {
    handle: Option<BodyId>,
}

fn body_value(body: &Body3D) -> ComponentValue {
    ComponentValue::Dynamic {
        component_type: ComponentTypeId::new("engine.body3d"),
        value: crate::character_animation::runtime_json(
            serde_json::to_value(body).expect("typed body"),
        ),
    }
}
impl PhysicsModule {
    fn validate_static_target(&self, world: &World, id: &EntityId) -> ContractResult<()> {
        let before: Option<Body3D> = component(world, id, "engine.body3d")
            .map(serde_json::from_value)
            .transpose()
            .map_err(backend)?;
        let native = (self.epoch == Some(world.epoch()))
            .then(|| self.bodies.get(id))
            .flatten();
        if before
            .as_ref()
            .is_some_and(|b| b.motion != BodyMotion3D::Static)
            || native.is_some_and(|b| b.descriptor.motion != BodyMotion3D::Static)
            || component(world, id, "engine.motor3d").is_some()
            || (self.epoch == Some(world.epoch()) && self.motors.contains_key(id))
            || component(world, id, "engine.joint3d").is_some()
            || world
                .query_entities(&crate::query::QuerySpec::all([ComponentTypeId::new(
                    "engine.joint3d",
                )]))
                .iter()
                .any(|joint| {
                    component(world, joint, "engine.joint3d")
                        .is_some_and(|v| v["other"]["entityRef"].as_str() == Some(id.as_str()))
                })
            || native.is_some_and(|b| self.joints.values().any(|j| j.bodies.contains(&b.handle)))
        {
            return Err(invalid("/generated/motion", "Generated collision changes require a static target without a motor or joint endpoint."));
        }
        if native.is_some_and(|b| world.entity_generation(id) != Some(b.generation)) {
            return Err(invalid(
                "/generated/identity",
                "Native target incarnation has not retired.",
            ));
        }
        Ok(())
    }

    pub(crate) fn prepare_static_collision(
        &mut self,
        world: &World,
        id: &EntityId,
        descriptor: Option<Body3D>,
        loader: &mut RuntimeAssetLoader,
    ) -> ContractResult<PreparedStaticCollision> {
        self.validate_static_target(world, id)?;
        let generation = world
            .entity_generation(id)
            .ok_or_else(|| invalid("/generated/identity", "Missing target."))?;
        let pose = world_pose(world, id)?;
        let before = world.component_value(id, &ComponentTypeId::new("engine.body3d"));
        let old_handle = (self.epoch == Some(world.epoch()))
            .then(|| self.bodies.get(id).map(|b| b.handle))
            .flatten();
        let body = if let Some(descriptor) = descriptor {
            descriptor.validate()?;
            if descriptor.motion != BodyMotion3D::Static {
                return Err(invalid(
                    "/generated/motion",
                    "Only static collision is supported.",
                ));
            }
            let shape = self.resolve_shape(
                &descriptor.shape,
                pose.scale,
                descriptor.motion,
                descriptor.sensor,
                &mut Some(loader),
            )?;
            Some(PreparedBody {
                id: id.clone(),
                descriptor,
                pose: pose.clone(),
                shape,
            })
        } else {
            None
        };
        let count = usize::from(body.as_ref().is_some_and(|b| b.descriptor.enabled));
        let existing = if self.epoch == Some(world.epoch()) {
            self.bodies.len() + self.motors.len()
        } else {
            0
        };
        self.prepared_capacity
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pending| {
                pending
                    .checked_add(count)
                    .filter(|n| existing + *n <= 12000)
            })
            .map_err(|_| {
                invalid(
                    "/generated/capacity",
                    "Static candidate exceeds original physics capacity.",
                )
            })?;
        let value = body.as_ref().map(|b| body_value(&b.descriptor));
        Ok(PreparedStaticCollision {
            epoch: world.epoch(),
            id: id.clone(),
            generation,
            before,
            old_handle,
            pose,
            body,
            value,
            reservation: Reservation {
                counter: self.prepared_capacity.clone(),
                count,
            },
        })
    }

    fn validate_static_collision(
        &self,
        world: &World,
        p: &PreparedStaticCollision,
    ) -> ContractResult<()> {
        if world.epoch() != p.epoch
            || world.entity_generation(&p.id) != Some(p.generation)
            || !Arc::ptr_eq(&p.reservation.counter, &self.prepared_capacity)
            || world.component_value(&p.id, &ComponentTypeId::new("engine.body3d")) != p.before
            || (self.epoch == Some(p.epoch))
                .then(|| self.bodies.get(&p.id).map(|b| b.handle))
                .flatten()
                != p.old_handle
        {
            return Err(invalid(
                "/generated/identity",
                "Static candidate owner or binding changed.",
            ));
        }
        self.validate_static_target(world, &p.id)?;
        let pose = world_pose(world, &p.id)?;
        if pose_changed(&pose, &p.pose) || pose.scale != p.pose.scale {
            return Err(invalid(
                "/generated/pose",
                "Static target pose changed after preparation.",
            ));
        }
        Ok(())
    }

    fn activate_static_collision(
        &mut self,
        world: &World,
        p: &PreparedStaticCollision,
    ) -> ContractResult<ActivatedStaticCollision> {
        self.validate_static_collision(world, p)?;
        if self.epoch != Some(world.epoch()) {
            self.solver = None;
            self.bodies.clear();
            self.motors.clear();
            self.joints.clear();
            self.last_tick = None;
            self.epoch = Some(world.epoch());
        }
        let Some(body) = p.body.as_ref().filter(|b| b.descriptor.enabled) else {
            return Ok(ActivatedStaticCollision { handle: None });
        };
        self.initialize(world, std::slice::from_ref(&p.id))?;
        let local = self.local(body.pose.position)?;
        let d = &body.descriptor;
        let settings = BodySettings::new_static()
            .position(point(local))
            .rotation(rotation(body.pose.rotation))
            .friction(d.friction)
            .restitution(d.restitution)
            .mass(d.mass)
            .sensor(d.sensor)
            .allow_sleeping(d.allow_sleeping)
            .motion_quality(if d.ccd {
                MotionQuality::LinearCast
            } else {
                MotionQuality::Discrete
            });
        let handle = self
            .solver
            .as_mut()
            .unwrap()
            .create_body(&body.shape, &settings)
            .map_err(backend)?;
        Ok(ActivatedStaticCollision {
            handle: Some(handle),
        })
    }
    fn rollback_static_collision(
        &mut self,
        active: ActivatedStaticCollision,
    ) -> ContractResult<()> {
        if let Some(handle) = active.handle {
            self.solver
                .as_mut()
                .unwrap()
                .remove_body(handle)
                .map_err(backend)?;
        }
        Ok(())
    }

    /// All fallible native work precedes World publication. The old body remains live
    /// until the new body has actually activated; failed activation/identity keeps it.
    pub(crate) fn commit_static_collision(
        &mut self,
        world: &mut World,
        mut p: PreparedStaticCollision,
    ) -> ContractResult<Option<GeneratedStaticBinding>> {
        // The original first simulation may materialize the unchanged authored
        // static body while this candidate waits for GPU readiness. Capture that
        // handle only for the exact original descriptor, pose, incarnation and
        // owner; an existing captured handle can never be silently replaced.
        if p.old_handle.is_none()
            && p.before.is_some()
            && self.epoch == Some(p.epoch)
            && Arc::ptr_eq(&p.reservation.counter, &self.prepared_capacity)
        {
            if let Some(native) = self.bodies.get(&p.id) {
                let authored = component(world, &p.id, "engine.body3d")
                    .and_then(|v| serde_json::from_value::<Body3D>(v).ok());
                if native.generation == p.generation
                    && authored.as_ref() == Some(&native.descriptor)
                    && !pose_changed(&native.pose, &p.pose)
                    && native.pose.scale == p.pose.scale
                {
                    p.old_handle = Some(native.handle);
                }
            }
        }
        let active = self.activate_static_collision(world, &p)?;
        self.finish_static_collision(world, p, active)
    }
    fn finish_static_collision(
        &mut self,
        world: &mut World,
        mut p: PreparedStaticCollision,
        active: ActivatedStaticCollision,
    ) -> ContractResult<Option<GeneratedStaticBinding>> {
        if let Err(error) = self.validate_static_collision(world, &p) {
            self.rollback_static_collision(active)?;
            return Err(error);
        }
        if let Some(old) = p.old_handle {
            if let Err(error) = self
                .solver
                .as_mut()
                .unwrap()
                .remove_body(old)
                .map_err(backend)
            {
                self.rollback_static_collision(active)?;
                return Err(error);
            }
            self.bodies.remove(&p.id);
        }
        let binding = p.body.take().map(|b| {
            let disabled = if let Some(handle) = active.handle {
                self.bodies.insert(
                    p.id.clone(),
                    BodyEntry {
                        _shape: b.shape,
                        handle,
                        generation: p.generation,
                        descriptor: b.descriptor,
                        pose: b.pose,
                        revision: 0,
                        impulse: 0,
                    },
                );
                None
            } else {
                Some(b.descriptor)
            };
            GeneratedStaticBinding {
                epoch: p.epoch,
                id: p.id.clone(),
                generation: p.generation,
                owner: self.prepared_capacity.clone(),
                handle: active.handle,
                disabled,
            }
        });
        if let Some(value) = p.value.take() {
            world.insert_component_value(p.id.clone(), value);
        } else {
            world.remove_component_value(&p.id, &ComponentTypeId::new("engine.body3d"));
        }
        // Pending old-handle contact events cannot map onto the new native incarnation.
        let frame = Arc::make_mut(&mut world.physics3d_frame);
        frame.bodies.remove(p.id.as_str());
        frame
            .contacts
            .retain(|c| c.entity1 != p.id.as_str() && c.entity2 != p.id.as_str());
        Ok(binding)
    }

    pub(crate) fn generated_static_native_live(&self, binding: &GeneratedStaticBinding) -> bool {
        self.epoch == Some(binding.epoch)
            && Arc::ptr_eq(&self.prepared_capacity, &binding.owner)
            && binding.handle.is_none_or(|handle| {
                self.bodies
                    .get(&binding.id)
                    .is_some_and(|b| b.handle == handle && b.generation == binding.generation)
            })
    }
    pub(crate) fn retire_generated_static(
        &mut self,
        world: &mut World,
        binding: &GeneratedStaticBinding,
    ) -> ContractResult<()> {
        if self.epoch != Some(binding.epoch)
            || !Arc::ptr_eq(&self.prepared_capacity, &binding.owner)
        {
            return Ok(());
        }
        let native = self
            .bodies
            .get(&binding.id)
            .filter(|b| Some(b.handle) == binding.handle && b.generation == binding.generation);
        if binding.handle.is_some() && native.is_none() {
            return Ok(());
        }
        let descriptor = native.map(|b| &b.descriptor).or(binding.disabled.as_ref());
        let owns_world = world.epoch() == binding.epoch
            && world.entity_generation(&binding.id) == Some(binding.generation)
            && descriptor.is_some_and(|d| {
                world.component_value(&binding.id, &ComponentTypeId::new("engine.body3d"))
                    == Some(body_value(d))
            });
        if let Some(handle) = binding.handle {
            self.solver
                .as_mut()
                .unwrap()
                .remove_body(handle)
                .map_err(backend)?;
            self.bodies.remove(&binding.id);
        }
        if owns_world {
            world.remove_component_value(&binding.id, &ComponentTypeId::new("engine.body3d"));
        }
        if world.epoch() == binding.epoch {
            let frame = Arc::make_mut(&mut world.physics3d_frame);
            frame.bodies.remove(binding.id.as_str());
            frame
                .contacts
                .retain(|c| c.entity1 != binding.id.as_str() && c.entity2 != binding.id.as_str());
        }
        Ok(())
    }
}
