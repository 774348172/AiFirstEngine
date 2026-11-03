use super::control::{component, PhysicsControl3D};
use super::*;
use crate::{
    components::ComponentTypeId, ids::EntityId, query::QuerySpec, spatial_contract::ContractResult,
    world::World,
};
use glam::{Mat4, Quat as GQuat, Vec3 as GVec3};
use oxijolt::prelude::math::*;
use oxijolt::prelude::*;
use oxijolt::{FixedConstraintSettings, GroundState, PointConstraintSettings};
#[path = "prepared.rs"]
mod prepared;
pub(crate) use prepared::{ActivatedPhysics3D, PreparedPhysics3D};
#[cfg(feature = "real-wgpu")]
pub(crate) use prepared::{GeneratedStaticBinding, PreparedStaticCollision};

fn backend(error: impl std::fmt::Display) -> crate::spatial_contract::SpatialContractError {
    invalid("/backend", error.to_string())
}
fn vector(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}
fn array(v: Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}
fn rotation(q: [f32; 4]) -> Quat {
    Quat::from_xyzw(q[0], q[1], q[2], q[3])
}
fn quat(q: Quat) -> [f32; 4] {
    [q.x, q.y, q.z, q.w]
}
fn point(v: [f32; 3]) -> RVec3 {
    RVec3::new(v[0], v[1], v[2])
}
fn position(v: RVec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}
fn shape(source: &CollisionShape3D, scale: [f32; 3]) -> ContractResult<Shape> {
    source.validate()?;
    let base = match source {
        CollisionShape3D::Asset { .. } => {
            return Err(invalid(
                "/shape",
                "Asset geometry requires the RuntimePackage loader.",
            ))
        }
        CollisionShape3D::Box { half_extents } => Shape::new_box(vector(*half_extents)),
        CollisionShape3D::Sphere { radius } => Shape::new_sphere(*radius),
        CollisionShape3D::Capsule {
            half_height,
            radius,
        } => Shape::new_capsule(*half_height, *radius),
        CollisionShape3D::Convex { vertices } => {
            Shape::new_convex_hull(&vertices.iter().copied().map(vector).collect::<Vec<_>>())
        }
        CollisionShape3D::StaticMesh {
            vertices,
            triangles,
        } => Shape::new_mesh(
            &vertices.iter().copied().map(vector).collect::<Vec<_>>(),
            triangles,
        )
        .map(|(s, _)| s),
    }
    .map_err(backend)?;
    if scale == [1.; 3] {
        Ok(base)
    } else {
        Shape::new_scaled(&base, vector(scale)).map_err(backend)
    }
}

#[derive(Clone)]
struct Pose {
    position: [f64; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}
fn pose_changed(a: &Pose, b: &Pose) -> bool {
    a.position
        .iter()
        .zip(b.position)
        .any(|(a, b)| (a - b).abs() > 0.00001)
        || GQuat::from_array(a.rotation)
            .dot(GQuat::from_array(b.rotation))
            .abs()
            < 1. - 1e-6
}
fn translation_only(matrix: Mat4) -> bool {
    matrix.x_axis == Mat4::IDENTITY.x_axis
        && matrix.y_axis == Mat4::IDENTITY.y_axis
        && matrix.z_axis == Mat4::IDENTITY.z_axis
}
fn world_pose(world: &World, id: &EntityId) -> ContractResult<Pose> {
    let state = world
        .spatial_world(id)
        .ok_or_else(|| invalid("/transform", "Missing committed spatial state."))?;
    let matrix = Mat4::from_cols_array(&state.world_matrix);
    let (mut scale, rotation, translation) = matrix.to_scale_rotation_translation();
    if scale.min_element() <= 0.
        || !scale.is_finite()
        || Mat4::from_scale_rotation_translation(scale, rotation, translation)
            .to_cols_array()
            .iter()
            .zip(state.world_matrix)
            .any(|(a, b)| (a - b).abs() > 1e-4)
    {
        return Err(invalid(
            "/scale",
            "Physics requires positive shear-free world scale.",
        ));
    }
    if world
        .hierarchy(id)
        .and_then(|h| h.parent_id)
        .is_none_or(|parent| {
            world
                .spatial_world(&parent)
                .is_some_and(|p| translation_only(Mat4::from_cols_array(&p.world_matrix)))
        })
    {
        // Translation preserves canonical scale, including cell placements.
        // Solver rotation roundoff must not become a new authored shape scale.
        scale = GVec3::from_array(world.transform3d(id).unwrap().scale);
    }
    Ok(Pose {
        position: std::array::from_fn(|i| state.anchor[i] + f64::from(translation[i])),
        rotation: rotation.to_array(),
        scale: scale.to_array(),
    })
}
struct JointEntry {
    handle: AnyConstraintId,
    descriptor: Joint3D,
    bodies: [BodyId; 2],
}
struct BodyEntry {
    _shape: Arc<Shape>,
    handle: BodyId,
    generation: u64,
    descriptor: Body3D,
    pose: Pose,
    revision: u64,
    impulse: u64,
}
struct MotorEntry {
    handle: CharacterId,
    generation: u64,
    descriptor: CharacterMotor3D,
    pose: Pose,
    movement: u64,
    jump: u64,
    vertical: f32,
}

#[derive(Default)]
pub struct PhysicsModule {
    prepared_capacity: Arc<std::sync::atomic::AtomicUsize>,
    solver: Option<PhysicsWorld>,
    epoch: Option<u64>,
    origin: [f64; 3],
    last_tick: Option<u64>,
    bodies: BTreeMap<EntityId, BodyEntry>,
    motors: BTreeMap<EntityId, MotorEntry>,
    shapes: BTreeMap<String, std::sync::Weak<Shape>>,
    solver_creations: u64,
    joints: BTreeMap<EntityId, JointEntry>,
}
impl std::fmt::Debug for PhysicsModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhysicsModule")
            .field("bodies", &self.bodies.len())
            .field("motors", &self.motors.len())
            .field("solver_creations", &self.solver_creations)
            .finish()
    }
}
impl PhysicsModule {
    pub(super) fn query_tick_for_world(
        &self,
        epoch: u64,
    ) -> Result<Option<u64>, project_runtime_abi::ProjectRuntimeAbiStatus> {
        if self.epoch.is_some_and(|bound| bound != epoch) {
            return Err(project_runtime_abi::ProjectRuntimeAbiStatus::INVALID_HANDLE);
        }
        Ok(self.last_tick.filter(|_| self.solver.is_some()))
    }
    pub fn solver_creation_count(&self) -> u64 {
        self.solver_creations
    }
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }
    pub fn motor_count(&self) -> usize {
        self.motors.len()
    }
    pub fn origin(&self) -> [f64; 3] {
        self.origin
    }
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }
    pub(super) fn local(&self, p: [f64; 3]) -> ContractResult<[f32; 3]> {
        let relative = std::array::from_fn::<_, 3, _>(|i| p[i] - self.origin[i]);
        if relative.iter().any(|v| !v.is_finite() || v.abs() > 32768.) {
            return Err(invalid(
                "/origin",
                "Physics local position exceeds the declared 32km precision envelope.",
            ));
        }
        Ok(relative.map(|v| v as f32))
    }
    fn absolute(&self, p: RVec3) -> [f64; 3] {
        let p = position(p);
        std::array::from_fn(|i| self.origin[i] + f64::from(p[i]))
    }
    fn initialize(&mut self, world: &World, ids: &[EntityId]) -> ContractResult<()> {
        if self.solver.is_none() {
            self.origin = world
                .main_camera()
                .and_then(|id| world.spatial_world(id))
                .or_else(|| ids.first().and_then(|id| world.spatial_world(id)))
                .map_or([0.; 3], |s| s.anchor);
            let mut solver =
                PhysicsWorld::new(WorldSettings::default().max_bodies(16384).worker_threads(1))
                    .map_err(backend)?;
            solver.set_event_settings(EventSettings::default().persisted_contacts(true));
            self.solver = Some(solver);
            self.solver_creations += 1;
        }
        Ok(())
    }
    pub fn rebase(&mut self, origin: [f64; 3]) -> ContractResult<()> {
        if origin.iter().any(|v| !v.is_finite()) {
            return Err(invalid("/origin", "Non-finite physics origin."));
        }
        if origin == self.origin {
            return Ok(());
        }
        let shift = std::array::from_fn::<_, 3, _>(|i| (self.origin[i] - origin[i]) as f32);
        if shift.iter().any(|v| !v.is_finite() || v.abs() > 32768.) {
            return Err(invalid(
                "/origin",
                "Incremental rebase exceeds local precision envelope.",
            ));
        }
        if let Some(solver) = &mut self.solver {
            let ids = self.bodies.values().map(|b| b.handle).collect::<Vec<_>>();
            solver
                .rebase(&ids, Quat::IDENTITY, point(shift))
                .map_err(backend)?;
            for entry in self.motors.values() {
                // PhysicsWorld::rebase also moves its characters. Applying
                // the shift again here would move every motor in world space.
                solver
                    .refresh_character_contacts(entry.handle, &QueryFilter::default())
                    .map_err(backend)?;
            }
        }
        self.origin = origin;
        Ok(())
    }
    fn resolve_shape(
        &mut self,
        source: &CollisionShape3D,
        scale: [f32; 3],
        motion: BodyMotion3D,
        sensor: bool,
        loader: &mut Option<&mut crate::runtime_asset_loader::RuntimeAssetLoader>,
    ) -> ContractResult<Arc<Shape>> {
        let CollisionShape3D::Asset { source } = source else {
            return shape(source, scale).map(Arc::new);
        };
        let loader = loader.as_deref_mut().ok_or_else(|| {
            invalid(
                "/shape",
                "Collision AssetRef requires RuntimePackage context.",
            )
        })?;
        let record = loader
            .asset_index()
            .resolve(&source.0)
            .map_err(|e| backend(format!("{e:?}")))?;
        if record.asset_id != source.0.id {
            return Err(invalid("/shape", "Collision id/GUID mismatch."));
        }
        let hash = record
            .hash
            .clone()
            .ok_or_else(|| invalid("/shape", "Collision asset needs a hash."))?;
        let key = format!("{hash}:{:?}:{motion:?}:{sensor}", scale.map(f32::to_bits));
        if let Some(shape) = self.shapes.get(&key).and_then(std::sync::Weak::upgrade) {
            return Ok(shape);
        }
        let handle = loader
            .load(&source.0)
            .map_err(|e| backend(format!("{e:?}")))?;
        let result = loader
            .get_asset_bytes(&handle)
            .ok_or_else(|| invalid("/shape", "Collision bytes unavailable."))
            .and_then(|bytes| {
                if crate::canonical_digest::sha256_prefixed(&bytes) != hash {
                    return Err(invalid("/shape", "Collision payload hash mismatch."));
                }
                let cooked: CookedCollision3D = serde_json::from_slice(&bytes).map_err(backend)?;
                cooked.validate()?;
                if matches!(cooked.shape, CollisionShape3D::StaticMesh { .. })
                    && (motion != BodyMotion3D::Static || sensor)
                {
                    return Err(invalid(
                        "/shape",
                        "Triangle collision assets require static nonsensor bodies.",
                    ));
                }
                shape(&cooked.shape, scale).map(Arc::new)
            });
        loader
            .release(&handle)
            .map_err(|e| backend(format!("{e:?}")))?;
        let shape = result?;
        self.shapes.insert(key, Arc::downgrade(&shape));
        Ok(shape)
    }
    pub fn advance_with_assets(
        &mut self,
        world: &mut World,
        loader: &mut crate::runtime_asset_loader::RuntimeAssetLoader,
        tick: u64,
        dt: f32,
    ) -> ContractResult<()> {
        self.advance_internal(world, tick, dt, Some(loader))
    }
    pub fn advance(&mut self, world: &mut World, tick: u64, dt: f32) -> ContractResult<()> {
        self.advance_internal(world, tick, dt, None)
    }
    /// One fixed simulation step. The solver and its contact/sleep state survive ticks.
    fn advance_internal(
        &mut self,
        world: &mut World,
        tick: u64,
        dt: f32,
        mut loader: Option<&mut crate::runtime_asset_loader::RuntimeAssetLoader>,
    ) -> ContractResult<()> {
        if !dt.is_finite()
            || dt <= 0.
            || dt > 0.1
            || self
                .last_tick
                .is_some_and(|last| self.epoch == Some(world.epoch()) && tick <= last)
        {
            return Err(invalid(
                "/tick",
                "Physics requires increasing fixed ticks and delta in (0,0.1].",
            ));
        }
        if self.epoch != Some(world.epoch()) {
            self.solver = None;
            self.bodies.clear();
            self.motors.clear();
            self.joints.clear();
            self.epoch = Some(world.epoch());
            self.last_tick = None;
            world.physics3d_frame = Arc::new(PhysicsFrame3D::default());
        }
        world.propagate_spatial();
        let body_ids = world
            .query_entities(&QuerySpec::all([ComponentTypeId::new("engine.body3d")]))
            .into_iter()
            .filter(|id| {
                world.spatial_world(id).is_some_and(|s| s.enabled)
                    && component(world, id, "engine.body3d").is_none_or(|v| v["enabled"] != false)
            })
            .collect::<Vec<_>>();
        let motor_ids = world
            .query_entities(&QuerySpec::all([ComponentTypeId::new("engine.motor3d")]))
            .into_iter()
            .filter(|id| {
                world.spatial_world(id).is_some_and(|s| s.enabled)
                    && component(world, id, "engine.motor3d").is_none_or(|v| v["enabled"] != false)
            })
            .collect::<Vec<_>>();
        if body_ids.len() > 12000 || motor_ids.len() > 1024 {
            return Err(invalid("/capacity", "Physics body/motor budget exceeded."));
        }
        let mut descriptors = BTreeMap::new();
        let mut motor_descriptors = BTreeMap::new();
        for id in &body_ids {
            let desc: Body3D =
                serde_json::from_value(component(world, id, "engine.body3d").unwrap())
                    .map_err(backend)?;
            desc.validate()?;
            if motor_ids.binary_search(id).is_ok() {
                return Err(invalid(
                    "/ownership",
                    "A motor and rigid body cannot own the same transform.",
                ));
            }
            descriptors.insert(id.clone(), (desc, world_pose(world, id)?));
        }
        for id in &motor_ids {
            let desc: CharacterMotor3D =
                serde_json::from_value(component(world, id, "engine.motor3d").unwrap())
                    .map_err(backend)?;
            desc.validate()?;
            let pose = world_pose(world, id)?;
            if pose.scale.iter().any(|v| (*v - 1.).abs() > 1e-5) {
                return Err(invalid(
                    "/motor/scale",
                    "Motor dimensions are authored in world metres; scale must be one.",
                ));
            }
            motor_descriptors.insert(id.clone(), (desc, pose));
        }
        let mut all = body_ids.clone();
        all.extend(motor_ids.iter().cloned());
        if all.is_empty() && self.solver.is_none() {
            self.last_tick = Some(tick);
            return Ok(());
        }
        self.initialize(world, &all)?;
        if let Some(origin) = world
            .main_camera()
            .and_then(|id| world.spatial_world(id))
            .map(|s| s.anchor)
        {
            self.rebase(origin)?;
        }
        let retired = self
            .bodies
            .iter()
            .filter(|(id, e)| {
                !descriptors.get(*id).is_some_and(|(d, p)| {
                    world.entity_generation(id) == Some(e.generation)
                        && d == &e.descriptor
                        && p.scale == e.pose.scale
                })
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        let retired_joints = self
            .joints
            .iter()
            .filter(|(id, e)| {
                retired.contains(id)
                    || retired.iter().any(|id| {
                        self.bodies
                            .get(id)
                            .is_some_and(|b| e.bodies.contains(&b.handle))
                    })
                    || component(world, id, "engine.joint3d").is_none_or(|v| {
                        serde_json::from_value::<Joint3D>(v).ok().as_ref() != Some(&e.descriptor)
                    })
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in retired_joints {
            let joint = self.joints.remove(&id).unwrap();
            self.solver
                .as_mut()
                .unwrap()
                .remove_constraint(joint.handle)
                .map_err(backend)?;
        }
        for id in retired {
            let e = self.bodies.remove(&id).unwrap();
            self.solver
                .as_mut()
                .unwrap()
                .remove_body(e.handle)
                .map_err(backend)?;
        }
        let retired = self
            .motors
            .iter()
            .filter(|(id, e)| {
                !motor_descriptors.get(*id).is_some_and(|(d, _)| {
                    world.entity_generation(id) == Some(e.generation) && d == &e.descriptor
                })
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in retired {
            let e = self.motors.remove(&id).unwrap();
            self.solver
                .as_mut()
                .unwrap()
                .remove_character(e.handle)
                .map_err(backend)?;
        }
        for (id, (desc, pose)) in descriptors {
            let local = self.local(pose.position)?;
            if !self.bodies.contains_key(&id) {
                let shape = self.resolve_shape(
                    &desc.shape,
                    pose.scale,
                    desc.motion,
                    desc.sensor,
                    &mut loader,
                )?;
                let settings = match desc.motion {
                    BodyMotion3D::Static => BodySettings::new_static(),
                    BodyMotion3D::Dynamic => BodySettings::new_dynamic(),
                    BodyMotion3D::Kinematic => BodySettings::new_kinematic(),
                }
                .position(point(local))
                .rotation(rotation(pose.rotation))
                .friction(desc.friction)
                .restitution(desc.restitution)
                .mass(desc.mass)
                .sensor(desc.sensor)
                .allow_sleeping(desc.allow_sleeping)
                .motion_quality(if desc.ccd {
                    MotionQuality::LinearCast
                } else {
                    MotionQuality::Discrete
                });
                let handle = self
                    .solver
                    .as_mut()
                    .unwrap()
                    .create_body(&shape, &settings)
                    .map_err(backend)?;
                self.bodies.insert(
                    id.clone(),
                    BodyEntry {
                        _shape: shape,
                        handle,
                        generation: world.entity_generation(&id).unwrap(),
                        descriptor: desc.clone(),
                        pose: pose.clone(),
                        revision: 0,
                        impulse: 0,
                    },
                );
            }
            let e = self.bodies.get_mut(&id).unwrap();
            let solver = self.solver.as_mut().unwrap();
            if pose_changed(&pose, &e.pose) {
                if desc.motion == BodyMotion3D::Kinematic {
                    solver
                        .body_mut(e.handle)
                        .map_err(backend)?
                        .move_kinematic(point(local), rotation(pose.rotation), dt)
                        .map_err(backend)?;
                } else {
                    let teleport = world.spatial_delta().changes.iter().any(|c| {
                        c.entity_id == id
                            && c.field_mask
                                .contains(crate::world::SpatialFieldMask::HISTORY)
                    });
                    if desc.motion == BodyMotion3D::Dynamic && !teleport {
                        return Err(invalid("/ownership","A dynamic body's transform changed outside the physics/teleport owner."));
                    }
                    solver
                        .body_mut(e.handle)
                        .map_err(backend)?
                        .set_position_and_rotation(
                            point(local),
                            rotation(pose.rotation),
                            Activation::Activate,
                        )
                        .map_err(backend)?;
                }
            }
            if let Some(control) = component(world, &id, "engine.physics3d_control") {
                let control: PhysicsControl3D = serde_json::from_value(control).map_err(backend)?;
                if control.revision != e.revision {
                    if let Some(v) = control.velocity {
                        solver
                            .body_mut(e.handle)
                            .map_err(backend)?
                            .set_linear_velocity(vector(v))
                            .map_err(backend)?;
                    }
                    if let Some((revision, impulse)) =
                        control.impulse.filter(|(r, _)| *r > e.impulse)
                    {
                        solver
                            .body_mut(e.handle)
                            .map_err(backend)?
                            .add_impulse(vector(impulse))
                            .map_err(backend)?;
                        e.impulse = revision;
                    }
                    e.revision = control.revision;
                }
            }
            e.pose = pose;
        }
        for (id, (desc, pose)) in motor_descriptors {
            let local = self.local(pose.position)?;
            if !self.motors.contains_key(&id) {
                let capsule = shape(
                    &CollisionShape3D::Capsule {
                        half_height: desc.half_height,
                        radius: desc.radius,
                    },
                    [1.; 3],
                )?;
                let settings = CharacterSettings::new(&capsule)
                    .mass(desc.mass)
                    .max_slope_angle(desc.max_slope_degrees.to_radians())
                    .supporting_volume(Vec3::new(0., 1., 0.), -desc.radius);
                let solver = self.solver.as_mut().unwrap();
                let handle = solver
                    .create_character(&settings, point(local), rotation(pose.rotation))
                    .map_err(backend)?;
                solver
                    .refresh_character_contacts(handle, &QueryFilter::default())
                    .map_err(backend)?;
                self.motors.insert(
                    id.clone(),
                    MotorEntry {
                        handle,
                        generation: world.entity_generation(&id).unwrap(),
                        descriptor: desc,
                        pose,
                        movement: 0,
                        jump: 0,
                        vertical: 0.,
                    },
                );
            }
        }
        for id in world.query_entities(&QuerySpec::all([ComponentTypeId::new("engine.joint3d")])) {
            if self.joints.contains_key(&id) {
                continue;
            }
            let descriptor: Joint3D =
                serde_json::from_value(component(world, &id, "engine.joint3d").unwrap())
                    .map_err(backend)?;
            descriptor.validate()?;
            let other = EntityId::from(descriptor.other.entity_ref.clone());
            // Disabled bodies are retired together with their constraints.
            if !world.spatial_world(&id).is_some_and(|s| s.enabled)
                || !world.spatial_world(&other).is_some_and(|s| s.enabled)
                || component(world, &id, "engine.body3d").is_some_and(|v| v["enabled"] == false)
                || component(world, &other, "engine.body3d").is_some_and(|v| v["enabled"] == false)
            {
                continue;
            }
            let a = self
                .bodies
                .get(&id)
                .ok_or_else(|| invalid("/joint", "Joint owner needs an active body."))?;
            let b = self
                .bodies
                .get(&other)
                .ok_or_else(|| invalid("/joint/other", "Joint target needs an active body."))?;
            if id == other
                || a.descriptor.motion == BodyMotion3D::Static
                    && b.descriptor.motion == BodyMotion3D::Static
            {
                return Err(invalid(
                    "/joint",
                    "Distinct bodies and at least one moving body required.",
                ));
            }
            let bodies = [a.handle, b.handle];
            let local = self.local(a.pose.position)?;
            let pivot = GVec3::from_array(local)
                + GQuat::from_array(a.pose.rotation) * GVec3::from_array(descriptor.pivot);
            let solver = self.solver.as_mut().unwrap();
            let handle = match descriptor.kind {
                JointKind3D::Fixed => solver
                    .create_constraint(
                        bodies[0],
                        bodies[1],
                        &FixedConstraintSettings::default().auto_detect_point(true),
                    )
                    .map_err(|error| {
                        backend(format!(
                            "joint owner={id}, other={other}, owner_position={:?}, other_position={:?}: {error}",
                            a.pose.position, b.pose.position
                        ))
                    })?
                    .into(),
                JointKind3D::Point => solver
                    .create_constraint(
                        bodies[0],
                        bodies[1],
                        &PointConstraintSettings::new(point(pivot.to_array())),
                    )
                    .map_err(|error| {
                        backend(format!(
                            "joint owner={id}, other={other}, owner_position={:?}, other_position={:?}: {error}",
                            a.pose.position, b.pose.position
                        ))
                    })?
                    .into(),
            };
            self.joints.insert(
                id,
                JointEntry {
                    handle,
                    descriptor,
                    bodies,
                },
            );
        }
        let outcome = self.solver.as_mut().unwrap().step(dt).map_err(backend)?;
        if !outcome.is_complete() {
            return Err(invalid(
                "/step",
                "Jolt reported an incomplete step; capacity may be exhausted.",
            ));
        }
        let mut frame = PhysicsFrame3D {
            tick,
            ..Default::default()
        };
        let mut output = BTreeMap::new();
        for (id, e) in &self.bodies {
            let b = self
                .solver
                .as_ref()
                .unwrap()
                .body(e.handle)
                .map_err(backend)?;
            let p = self.absolute(b.position());
            let q = quat(b.rotation());
            frame.bodies.insert(
                id.to_string(),
                PhysicsBodyState3D {
                    generation: e.generation,
                    position: p,
                    rotation: q,
                    velocity: array(b.linear_velocity()),
                    sleeping: !b.is_active(),
                },
            );
            if e.descriptor.motion != BodyMotion3D::Static {
                output.insert(
                    id.clone(),
                    Pose {
                        position: p,
                        rotation: q,
                        scale: e.pose.scale,
                    },
                );
            }
        }
        for (id, e) in &mut self.motors {
            let solver = self.solver.as_mut().unwrap();
            let actual = world_pose(world, id)?;
            if pose_changed(&actual, &e.pose) {
                let local = std::array::from_fn(|i| (actual.position[i] - self.origin[i]) as f32);
                solver
                    .character_mut(e.handle)
                    .map_err(backend)?
                    .set_position(point(local))
                    .map_err(backend)?;
                solver
                    .character_mut(e.handle)
                    .map_err(backend)?
                    .set_rotation(rotation(actual.rotation))
                    .map_err(backend)?;
                solver
                    .refresh_character_contacts(e.handle, &QueryFilter::default())
                    .map_err(backend)?;
                e.vertical = 0.;
            }
            let c = solver.character(e.handle).map_err(backend)?;
            let old = position(c.position());
            let grounded = c.ground_state() == GroundState::OnGround;
            let ground_velocity = if grounded {
                array(c.ground_velocity())
            } else {
                [0.; 3]
            };
            if grounded && e.vertical < 0. {
                e.vertical = 0.;
            }
            let mut requested = [0.; 3];
            if let Some(control) = component(world, id, "engine.physics3d_control") {
                let control: PhysicsControl3D = serde_json::from_value(control).map_err(backend)?;
                if let Some((revision, movement)) =
                    control.movement.filter(|(r, _)| *r > e.movement)
                {
                    requested = movement;
                    e.movement = revision;
                }
                if let Some((revision, speed)) = control.jump.filter(|(r, _)| *r > e.jump) {
                    if grounded {
                        e.vertical = speed;
                    }
                    e.jump = revision;
                }
            }
            e.vertical -= e.descriptor.gravity * dt;
            let mut velocity = std::array::from_fn(|i| {
                requested[i] / dt + ground_velocity[i] + if i == 1 { e.vertical } else { 0. }
            });
            let from = std::array::from_fn(|i| self.origin[i] + f64::from(old[i]));
            let extent = [
                f64::from(e.descriptor.radius),
                f64::from(e.descriptor.half_height + e.descriptor.radius),
                f64::from(e.descriptor.radius),
            ];
            let predicted =
                std::array::from_fn(|i| self.origin[i] + f64::from(old[i] + velocity[i] * dt));
            if !world.streaming_guard_allows_path(id, from, predicted, extent) {
                velocity = [0.; 3];
                e.vertical = 0.;
            }
            solver
                .character_mut(e.handle)
                .map_err(backend)?
                .set_linear_velocity(vector(velocity))
                .map_err(backend)?;
            let settings = ExtendedUpdateSettings::default()
                .walk_stairs_step_up(Vec3::new(0., e.descriptor.step_height, 0.))
                .stick_to_floor_step_down(if e.vertical > 0. {
                    Vec3::ZERO
                } else {
                    Vec3::new(0., -e.descriptor.step_height.max(0.05), 0.)
                });
            solver
                .update_character(
                    e.handle,
                    dt,
                    Vec3::new(0., -e.descriptor.gravity, 0.),
                    &settings,
                    &QueryFilter::default(),
                )
                .map_err(backend)?;
            let c = solver.character(e.handle).map_err(backend)?;
            let mut p = position(c.position());
            let absolute = std::array::from_fn(|i| self.origin[i] + f64::from(p[i]));
            if !world.streaming_guard_allows_path(id, from, absolute, extent) {
                solver
                    .character_mut(e.handle)
                    .map_err(backend)?
                    .set_position(point(old))
                    .map_err(backend)?;
                solver
                    .character_mut(e.handle)
                    .map_err(backend)?
                    .set_linear_velocity(Vec3::ZERO)
                    .map_err(backend)?;
                solver
                    .refresh_character_contacts(e.handle, &QueryFilter::default())
                    .map_err(backend)?;
                p = old;
                e.vertical = 0.;
            }
            let c = solver.character(e.handle).map_err(backend)?;
            let accepted = std::array::from_fn(|i| p[i] - old[i]);
            let grounded = c.ground_state() == GroundState::OnGround;
            if grounded && e.vertical < 0. {
                e.vertical = 0.;
            } else if !grounded && accepted[1] / dt < e.vertical {
                e.vertical = accepted[1] / dt;
            }
            frame.motors.insert(
                id.to_string(),
                MotorResult3D {
                    generation: e.generation,
                    tick,
                    requested,
                    accepted,
                    grounded,
                    velocity: array(c.linear_velocity()),
                },
            );
            output.insert(
                id.clone(),
                Pose {
                    position: std::array::from_fn(|i| self.origin[i] + f64::from(p[i])),
                    rotation: quat(c.rotation()),
                    scale: e.pose.scale,
                },
            );
        }
        let lookup = self
            .bodies
            .iter()
            .map(|(id, e)| (e.handle, (id, e)))
            .collect::<BTreeMap<_, _>>();
        for event in self.solver.as_mut().unwrap().take_events().contacts {
            let pair = event.pair();
            if let (Some((a, ea)), Some((b, eb))) =
                (lookup.get(&pair.body1), lookup.get(&pair.body2))
            {
                let phase = match event {
                    ContactEvent::Added { .. } => "added",
                    ContactEvent::Persisted { .. } => "persisted",
                    ContactEvent::Removed(_) => "removed",
                };
                frame.contacts.push(PhysicsContact3D {
                    entity1: a.to_string(),
                    generation1: ea.generation,
                    entity2: b.to_string(),
                    generation2: eb.generation,
                    phase: phase.into(),
                    sensor: ea.descriptor.sensor || eb.descriptor.sensor,
                });
            }
        }
        frame.contacts.sort_by(|a, b| {
            (&a.entity1, &a.entity2, &a.phase).cmp(&(&b.entity1, &b.entity2, &b.phase))
        });
        frame.contacts.dedup();
        let mut writes = Vec::with_capacity(output.len());
        for (id, pose) in &output {
            let state = world.spatial_world(id).unwrap();
            let runtime = world.runtime_id_for_source(id).unwrap();
            if world.hierarchy(id).and_then(|h| h.parent_id).is_none() {
                writes.push((
                    project_runtime_sdk::spatial_bulk::SpatialEntityKey {
                        world_epoch: world.epoch(),
                        index: runtime.index,
                        generation: runtime.generation,
                    },
                    project_runtime_sdk::ProjectRuntimeTransform3D {
                        translation: std::array::from_fn(|i| {
                            (pose.position[i] - state.anchor[i]) as f32
                        }),
                        rotation_xyzw: GQuat::from_array(pose.rotation).normalize().to_array(),
                        scale: pose.scale,
                    },
                ));
                continue;
            }
            let mut desired = Mat4::from_scale_rotation_translation(
                GVec3::from_array(pose.scale),
                GQuat::from_array(pose.rotation),
                GVec3::from_array(std::array::from_fn(|i| {
                    (pose.position[i] - state.anchor[i]) as f32
                })),
            );
            let mut parent_preserves_scale = true;
            if let Some(parent) = world.hierarchy(id).and_then(|h| h.parent_id) {
                let ps = world
                    .spatial_world(&parent)
                    .ok_or_else(|| invalid("/parent", "Missing parent spatial state."))?;
                let parent_matrix = if let Some(p) = output.get(&parent) {
                    Mat4::from_scale_rotation_translation(
                        GVec3::from_array(p.scale),
                        GQuat::from_array(p.rotation),
                        GVec3::from_array(std::array::from_fn(|i| {
                            (p.position[i] - state.anchor[i]) as f32
                        })),
                    )
                } else {
                    let mut m = ps.world_matrix;
                    for i in 0..3 {
                        m[12 + i] += (ps.anchor[i] - state.anchor[i]) as f32;
                    }
                    Mat4::from_cols_array(&m)
                };
                parent_preserves_scale = translation_only(parent_matrix);
                desired = parent_matrix.inverse() * desired;
            }
            let (s, r, t) = desired.to_scale_rotation_translation();
            writes.push((
                project_runtime_sdk::spatial_bulk::SpatialEntityKey {
                    world_epoch: world.epoch(),
                    index: runtime.index,
                    generation: runtime.generation,
                },
                project_runtime_sdk::ProjectRuntimeTransform3D {
                    translation: t.to_array(),
                    rotation_xyzw: r.normalize().to_array(),
                    scale: if parent_preserves_scale {
                        pose.scale
                    } else {
                        s.to_array()
                    },
                },
            ));
        }
        world
            .commit_spatial_local_batch(writes)
            .map_err(|e| invalid("/commit", e.message))?;
        for (id, pose) in output {
            if let Some(e) = self.bodies.get_mut(&id) {
                e.pose = pose.clone();
            }
            if let Some(e) = self.motors.get_mut(&id) {
                e.pose = pose;
            }
        }
        world.physics3d_frame = Arc::new(frame);
        self.shapes.retain(|_, shape| shape.strong_count() > 0);
        self.last_tick = Some(tick);
        Ok(())
    }
    pub fn overlap(
        &self,
        geometry: &CollisionShape3D,
        origin: [f64; 3],
        orientation: [f32; 4],
    ) -> ContractResult<Vec<PhysicsOverlap3D>> {
        let local = self.local(origin)?;
        let Some(solver) = &self.solver else {
            return Ok(Vec::new());
        };
        let shape = shape(geometry, [1.; 3])?;
        let hits = solver
            .collide_shape(
                &CollideShape::new(&shape, point(local), rotation(orientation)),
                &QueryFilter::default(),
            )
            .map_err(backend)?;
        let mut results = hits
            .into_iter()
            .filter_map(|hit| {
                self.bodies
                    .iter()
                    .find(|(_, e)| e.handle == hit.body)
                    .map(|(id, e)| PhysicsOverlap3D {
                        entity: id.to_string(),
                        generation: e.generation,
                        penetration: hit.penetration_depth,
                        position: self.absolute(hit.point_on_body),
                        normal: array(hit.normal),
                    })
            })
            .collect::<Vec<_>>();
        results.sort_by(|a, b| {
            a.entity
                .cmp(&b.entity)
                .then_with(|| b.penetration.total_cmp(&a.penetration))
        });
        results.dedup_by(|a, b| a.entity == b.entity);
        Ok(results)
    }
    pub fn raycast(&self, ray: &PhysicsRay3D) -> ContractResult<Option<PhysicsHit3D>> {
        ray.validate()?;
        let local = self.local(ray.origin)?;
        let Some(solver) = &self.solver else {
            return Ok(None);
        };
        let hit = solver
            .cast_ray(
                &RayCast::new(point(local), vector(ray.displacement)),
                &QueryFilter::default(),
            )
            .map_err(backend)?;
        Ok(hit.and_then(|hit| {
            self.bodies
                .iter()
                .find(|(_, e)| e.handle == hit.body)
                .map(|(id, e)| PhysicsHit3D {
                    entity: id.to_string(),
                    generation: e.generation,
                    fraction: hit.fraction,
                    distance: hit.distance,
                    position: std::array::from_fn(|i| {
                        ray.origin[i] + f64::from(ray.displacement[i] * hit.fraction)
                    }),
                    normal: array(hit.normal),
                })
        }))
    }
    pub fn sweep(
        &self,
        geometry: &CollisionShape3D,
        ray: &PhysicsRay3D,
        orientation: [f32; 4],
    ) -> ContractResult<Option<PhysicsHit3D>> {
        ray.validate()?;
        let local = self.local(ray.origin)?;
        let Some(solver) = &self.solver else {
            return Ok(None);
        };
        let shape = shape(geometry, [1.; 3])?;
        let hit = solver
            .cast_shape(
                &ShapeCast::new(
                    &shape,
                    point(local),
                    rotation(orientation),
                    vector(ray.displacement),
                ),
                &QueryFilter::default(),
            )
            .map_err(backend)?;
        Ok(hit.and_then(|hit| {
            self.bodies
                .iter()
                .find(|(_, e)| e.handle == hit.body)
                .map(|(id, e)| PhysicsHit3D {
                    entity: id.to_string(),
                    generation: e.generation,
                    fraction: hit.fraction,
                    distance: hit.distance,
                    position: self.absolute(hit.point),
                    normal: array(hit.normal),
                })
        }))
    }
}
