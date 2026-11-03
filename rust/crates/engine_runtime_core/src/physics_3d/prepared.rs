//! Inactive cell preparation and reversible activation in the original Jolt owner.
use super::*;
use crate::{
    runtime_asset_loader::RuntimeAssetLoader, runtime_entity_hydration::PreparedRuntimeEntity,
    world::SpatialWorldState,
};
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(any(feature = "real-wgpu", test))]
#[path = "generated.rs"]
mod generated;
#[cfg(feature = "real-wgpu")]
pub(crate) use generated::{GeneratedStaticBinding, PreparedStaticCollision};
struct Reservation {
    counter: Arc<AtomicUsize>,
    count: usize,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.counter.fetch_sub(self.count, Ordering::Relaxed);
    }
}
struct PreparedBody {
    id: EntityId,
    descriptor: Body3D,
    pose: Pose,
    shape: Arc<Shape>,
}
struct PreparedMotor {
    id: EntityId,
    descriptor: CharacterMotor3D,
    pose: Pose,
    shape: Arc<Shape>,
}
pub(crate) struct PreparedPhysics3D {
    epoch: u64,
    reservation: Reservation,
    bodies: Vec<PreparedBody>,
    motors: Vec<PreparedMotor>,
    joints: Vec<(EntityId, Joint3D)>,
}
impl PreparedPhysics3D {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        let roles = &["queuePayload"];
        sample.cpu_vec(&self.bodies, "physics", roles);
        sample.cpu_vec(&self.motors, "physics", roles);
        sample.cpu_vec(&self.joints, "physics", roles);
        sample.cpu_arc(&self.reservation.counter, "physics", roles);
        for body in &self.bodies {
            sample.cpu_arc(body.id.storage(), "world", roles);
            sample.cpu_string(&body.descriptor.schema_version, "physics", roles);
            observe_prepared_collision(&body.descriptor.shape, sample);
            // This is the Rust Arc payload wrapper, never the native Jolt heap.
            sample.cpu_arc(&body.shape, "physics", roles);
        }
        for motor in &self.motors {
            sample.cpu_arc(motor.id.storage(), "world", roles);
            sample.cpu_string(&motor.descriptor.schema_version, "physics", roles);
            sample.cpu_arc(&motor.shape, "physics", roles);
        }
        for (id, joint) in &self.joints {
            sample.cpu_arc(id.storage(), "world", roles);
            sample.cpu_string(&joint.schema_version, "physics", roles);
            sample.cpu_string(&joint.other.entity_ref, "physics", roles);
        }
        sample.partial("cpu.owner.physics", "cell.nativePrepared.physics.RustVecCapacity+ids+descriptors+ArcPayloadWrappers;JoltShapeSolverHeapAndArcControlBlocksNotMeasured");
    }
}
fn observe_prepared_collision(
    shape: &CollisionShape3D,
    sample: &mut crate::resource_memory::ResourceMemorySample,
) {
    match shape {
        CollisionShape3D::Asset { source } => {
            source.0.observe_memory(sample, "physics", "queuePayload")
        }
        CollisionShape3D::Convex { vertices } => {
            sample.cpu_vec(vertices, "physics", &["queuePayload"])
        }
        CollisionShape3D::StaticMesh {
            vertices,
            triangles,
        } => {
            sample.cpu_vec(vertices, "physics", &["queuePayload"]);
            sample.cpu_vec(triangles, "physics", &["queuePayload"]);
        }
        CollisionShape3D::Box { .. }
        | CollisionShape3D::Sphere { .. }
        | CollisionShape3D::Capsule { .. } => (),
    }
}
#[cfg(test)]
#[test]
fn prepared_native_collision_memory_counts_reserved_capacity_without_retaining() {
    use crate::resource_memory::{memory_totals, ResourceMemorySample};
    let mut vertices = Vec::with_capacity(64);
    vertices.push([0.; 3]);
    let mut triangles = Vec::with_capacity(128);
    triangles.push([0u32; 3]);
    let expected = vertices.capacity() * std::mem::size_of::<[f32; 3]>()
        + triangles.capacity() * std::mem::size_of::<[u32; 3]>();
    let shape = CollisionShape3D::StaticMesh {
        vertices,
        triangles,
    };
    let mut sample = ResourceMemorySample::new(1, 0, 0);
    observe_prepared_collision(&shape, &mut sample);
    observe_prepared_collision(&shape, &mut sample);
    let totals = memory_totals(&sample.allocations).unwrap();
    assert_eq!(totals.unique_allocations, 2);
    assert_eq!(totals.by_owner["cpu.physics"], expected as u64);
    assert_eq!(totals.by_role["cpu.queuePayload"], expected as u64);
}

impl std::fmt::Debug for PreparedPhysics3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedPhysics3D")
            .field("bodies", &self.bodies.len())
            .field("motors", &self.motors.len())
            .finish()
    }
}
pub(crate) struct ActivatedPhysics3D {
    bodies: Vec<(EntityId, BodyId)>,
    motors: Vec<(EntityId, CharacterId)>,
    joints: Vec<(EntityId, JointEntry)>,
}
fn prepared_pose(s: &SpatialWorldState) -> ContractResult<Pose> {
    let matrix = Mat4::from_cols_array(&s.world_matrix);
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    if scale.min_element() <= 0.
        || !scale.is_finite()
        || Mat4::from_scale_rotation_translation(scale, rotation, translation)
            .to_cols_array()
            .iter()
            .zip(s.world_matrix)
            .any(|(a, b)| (a - b).abs() > 1e-4)
    {
        return Err(invalid(
            "/prepare/scale",
            "Physics requires positive shear-free world scale.",
        ));
    }
    Ok(Pose {
        position: std::array::from_fn(|i| s.anchor[i] + f64::from(translation[i])),
        rotation: rotation.to_array(),
        scale: scale.to_array(),
    })
}
fn data(e: &PreparedRuntimeEntity, ty: &str) -> Option<serde_json::Value> {
    e.components.iter().find_map(|v| match v {
        crate::archetype::ComponentValue::Dynamic {
            component_type,
            value,
        } if component_type == &ComponentTypeId::new(ty) => {
            Some(crate::character_animation::dynamic_json(value))
        }
        _ => None,
    })
}
impl PhysicsModule {
    pub(crate) fn prepare_cell_entities(
        &mut self,
        world: &World,
        entities: &[PreparedRuntimeEntity],
        poses: &BTreeMap<EntityId, SpatialWorldState>,
        loader: &mut RuntimeAssetLoader,
    ) -> ContractResult<PreparedPhysics3D> {
        let mut bodies = Vec::new();
        let mut motors = Vec::new();
        let mut joints = Vec::new();
        for e in entities {
            let Some(s) = poses.get(&e.world_id).filter(|s| s.enabled) else {
                continue;
            };
            let pose = prepared_pose(s)?;
            if let Some(v) = data(e, "engine.body3d") {
                let descriptor: Body3D = serde_json::from_value(v).map_err(backend)?;
                descriptor.validate()?;
                if descriptor.enabled {
                    let shape = self.resolve_shape(
                        &descriptor.shape,
                        pose.scale,
                        descriptor.motion,
                        descriptor.sensor,
                        &mut Some(&mut *loader),
                    )?;
                    bodies.push(PreparedBody {
                        id: e.world_id.clone(),
                        descriptor,
                        pose: pose.clone(),
                        shape,
                    });
                }
            }
            if let Some(v) = data(e, "engine.motor3d") {
                let descriptor: CharacterMotor3D = serde_json::from_value(v).map_err(backend)?;
                descriptor.validate()?;
                if descriptor.enabled {
                    if data(e, "engine.body3d").is_some_and(|v| v["enabled"] != false)
                        || pose.scale.iter().any(|v| (*v - 1.).abs() > 1e-5)
                    {
                        return Err(invalid(
                            "/prepare/motor",
                            "Motor must be the unique unit-scale transform owner.",
                        ));
                    }
                    let shape = Arc::new(shape(
                        &CollisionShape3D::Capsule {
                            half_height: descriptor.half_height,
                            radius: descriptor.radius,
                        },
                        [1.; 3],
                    )?);
                    motors.push(PreparedMotor {
                        id: e.world_id.clone(),
                        descriptor,
                        pose,
                        shape,
                    });
                }
            }
            if let Some(v) = data(e, "engine.joint3d") {
                let desc: Joint3D = serde_json::from_value(v).map_err(backend)?;
                desc.validate()?;
                if data(e, "engine.body3d").is_some_and(|v| v["enabled"] != false) {
                    joints.push((e.world_id.clone(), desc));
                }
            }
        }
        let count = bodies.len() + motors.len();
        let existing = self.bodies.len() + self.motors.len();
        self.prepared_capacity
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pending| {
                pending.checked_add(count).filter(|total| {
                    existing + *total <= 12000 && self.motors.len() + motors.len() <= 1024
                })
            })
            .map_err(|_| {
                invalid(
                    "/prepare/capacity",
                    "Physics cell capacity reservation failed.",
                )
            })?;
        let reservation = Reservation {
            counter: self.prepared_capacity.clone(),
            count,
        };
        for (id, joint) in &joints {
            let a = bodies.iter().find(|b| &b.id == id);
            let b = bodies
                .iter()
                .find(|b| b.id.as_str() == joint.other.entity_ref);
            if a.is_none()
                || b.is_none()
                || id.as_str() == joint.other.entity_ref
                || a.unwrap().descriptor.motion == BodyMotion3D::Static
                    && b.unwrap().descriptor.motion == BodyMotion3D::Static
            {
                return Err(invalid(
                    "/prepare/joint",
                    "Cell joints need distinct prepared active bodies and a moving endpoint.",
                ));
            }
        }
        Ok(PreparedPhysics3D {
            epoch: world.epoch(),
            reservation,
            bodies,
            motors,
            joints,
        })
    }
    pub(crate) fn activate_cell_entities(
        &mut self,
        world: &World,
        p: &PreparedPhysics3D,
    ) -> ContractResult<ActivatedPhysics3D> {
        if p.epoch != world.epoch() || !Arc::ptr_eq(&p.reservation.counter, &self.prepared_capacity)
        {
            return Err(invalid(
                "/prepare/owner",
                "Physics ACK belongs to a different World or owner.",
            ));
        }
        if self.epoch != Some(world.epoch()) {
            self.solver = None;
            self.bodies.clear();
            self.motors.clear();
            self.joints.clear();
            self.last_tick = None;
            self.epoch = Some(world.epoch());
        }
        if p.bodies.len() + self.bodies.len() > 12000 || p.motors.len() + self.motors.len() > 1024 {
            return Err(invalid(
                "/prepare/capacity",
                "Active physics capacity changed.",
            ));
        }
        if p.bodies.is_empty() && p.motors.is_empty() {
            return Ok(ActivatedPhysics3D {
                bodies: vec![],
                motors: vec![],
                joints: vec![],
            });
        }
        if self.solver.is_none() {
            self.initialize(world, &[])?;
            if world.main_camera().is_none() {
                self.origin = p
                    .bodies
                    .first()
                    .map(|b| b.pose.position)
                    .or_else(|| p.motors.first().map(|b| b.pose.position))
                    .unwrap_or([0.; 3]);
            }
        }
        let mut active = ActivatedPhysics3D {
            bodies: Vec::new(),
            motors: Vec::new(),
            joints: Vec::new(),
        };
        let result = (|| {
            for b in &p.bodies {
                if self.bodies.contains_key(&b.id) {
                    return Err(invalid("/prepare/identity", "Physics identity occupied."));
                }
                let local = self.local(b.pose.position)?;
                let d = &b.descriptor;
                let settings = match d.motion {
                    BodyMotion3D::Static => BodySettings::new_static(),
                    BodyMotion3D::Dynamic => BodySettings::new_dynamic(),
                    BodyMotion3D::Kinematic => BodySettings::new_kinematic(),
                }
                .position(point(local))
                .rotation(rotation(b.pose.rotation))
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
                    .create_body(&b.shape, &settings)
                    .map_err(backend)?;
                active.bodies.push((b.id.clone(), handle));
            }
            for m in &p.motors {
                let local = self.local(m.pose.position)?;
                let d = &m.descriptor;
                let settings = CharacterSettings::new(&m.shape)
                    .mass(d.mass)
                    .max_slope_angle(d.max_slope_degrees.to_radians())
                    .supporting_volume(Vec3::new(0., 1., 0.), -d.radius);
                let solver = self.solver.as_mut().unwrap();
                let handle = solver
                    .create_character(&settings, point(local), rotation(m.pose.rotation))
                    .map_err(backend)?;
                active.motors.push((m.id.clone(), handle));
                solver
                    .refresh_character_contacts(handle, &QueryFilter::default())
                    .map_err(backend)?;
            }
            for (id, d) in &p.joints {
                let a = active.bodies.iter().find(|(i, _)| i == id).unwrap().1;
                let b = active
                    .bodies
                    .iter()
                    .find(|(i, _)| i.as_str() == d.other.entity_ref)
                    .unwrap()
                    .1;
                let pose = &p.bodies.iter().find(|b| &b.id == id).unwrap().pose;
                let pivot = GVec3::from_array(self.local(pose.position)?)
                    + GQuat::from_array(pose.rotation) * GVec3::from_array(d.pivot);
                let solver = self.solver.as_mut().unwrap();
                let handle = match d.kind {
                    JointKind3D::Fixed => solver
                        .create_constraint(
                            a,
                            b,
                            &FixedConstraintSettings::default().auto_detect_point(true),
                        )
                        .map_err(backend)?
                        .into(),
                    JointKind3D::Point => solver
                        .create_constraint(
                            a,
                            b,
                            &PointConstraintSettings::new(point(pivot.to_array())),
                        )
                        .map_err(backend)?
                        .into(),
                };
                active.joints.push((
                    id.clone(),
                    JointEntry {
                        handle,
                        descriptor: d.clone(),
                        bodies: [a, b],
                    },
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.rollback_cell_entities(active)?;
            return Err(error);
        }
        Ok(active)
    }
    pub(crate) fn rollback_cell_entities(
        &mut self,
        active: ActivatedPhysics3D,
    ) -> ContractResult<()> {
        let Some(solver) = self.solver.as_mut() else {
            return Ok(());
        };
        for (_, j) in active.joints {
            solver.remove_constraint(j.handle).map_err(backend)?;
        }
        for (_, id) in active.motors {
            solver.remove_character(id).map_err(backend)?;
        }
        for (_, id) in active.bodies {
            solver.remove_body(id).map_err(backend)?;
        }
        Ok(())
    }
    pub(crate) fn finish_cell_entities(
        &mut self,
        world: &World,
        p: PreparedPhysics3D,
        active: ActivatedPhysics3D,
    ) {
        for b in p.bodies {
            let handle = active.bodies.iter().find(|(id, _)| id == &b.id).unwrap().1;
            self.bodies.insert(
                b.id.clone(),
                BodyEntry {
                    _shape: b.shape,
                    handle,
                    generation: world.entity_generation(&b.id).unwrap(),
                    descriptor: b.descriptor,
                    pose: b.pose,
                    revision: 0,
                    impulse: 0,
                },
            );
        }
        for m in p.motors {
            let handle = active.motors.iter().find(|(id, _)| id == &m.id).unwrap().1;
            self.motors.insert(
                m.id.clone(),
                MotorEntry {
                    handle,
                    generation: world.entity_generation(&m.id).unwrap(),
                    descriptor: m.descriptor,
                    pose: m.pose,
                    movement: 0,
                    jump: 0,
                    vertical: 0.,
                },
            );
        }
        for (id, j) in active.joints {
            self.joints.insert(id, j);
        }
    }
    pub(crate) fn retire_cell_entities(
        &mut self,
        world: &mut World,
        ids: &std::collections::BTreeSet<EntityId>,
    ) -> ContractResult<()> {
        if let Some(solver) = &mut self.solver {
            let joints = self
                .joints
                .iter()
                .filter(|(id, j)| {
                    ids.contains(*id)
                        || self
                            .bodies
                            .iter()
                            .any(|(id, b)| ids.contains(id) && j.bodies.contains(&b.handle))
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            for id in joints {
                solver
                    .remove_constraint(self.joints.remove(&id).unwrap().handle)
                    .map_err(backend)?;
            }
            for id in ids {
                if let Some(b) = self.bodies.remove(id) {
                    solver.remove_body(b.handle).map_err(backend)?;
                }
                if let Some(m) = self.motors.remove(id) {
                    solver.remove_character(m.handle).map_err(backend)?;
                }
            }
        }
        let mut frame = (*world.physics3d_frame).clone();
        frame
            .bodies
            .retain(|id, _| !ids.contains(&EntityId::from(id.clone())));
        frame
            .motors
            .retain(|id, _| !ids.contains(&EntityId::from(id.clone())));
        frame.contacts.retain(|c| {
            !ids.contains(&EntityId::from(c.entity1.clone()))
                && !ids.contains(&EntityId::from(c.entity2.clone()))
        });
        world.physics3d_frame = Arc::new(frame);
        Ok(())
    }
}

impl PhysicsModule {
    /// Bodies keep the same entity incarnation and absolute pose; only the cell's
    /// lifetime set moves. A joint may not be cut across the new lifetime boundary.
    pub(crate) fn validate_cell_migration(
        &self,
        world: &World,
        ids: &std::collections::BTreeSet<EntityId>,
    ) -> ContractResult<()> {
        if self.epoch.is_some_and(|e| e != world.epoch()) {
            return Err(invalid(
                "/migration",
                "Physics owner belongs to another World.",
            ));
        }
        let handles: Vec<_> = self
            .bodies
            .iter()
            .filter(|(id, _)| ids.contains(*id))
            .map(|(_, b)| b.handle)
            .collect();
        for id in ids {
            if self
                .bodies
                .get(id)
                .is_some_and(|b| world.entity_generation(id) != Some(b.generation))
                || self
                    .motors
                    .get(id)
                    .is_some_and(|m| world.entity_generation(id) != Some(m.generation))
            {
                return Err(invalid("/migration", "Stale simulation incarnation."));
            }
        }
        for (id, joint) in &self.joints {
            let moving = joint.bodies.iter().filter(|h| handles.contains(h)).count();
            if moving > 0 && (moving != joint.bodies.len() || !ids.contains(id))
                || moving == 0 && ids.contains(id)
            {
                return Err(invalid(
                    "/migration",
                    "Joint dependency crosses migration lifetime boundary.",
                ));
            }
        }
        Ok(())
    }
}
