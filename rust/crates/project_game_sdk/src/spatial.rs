use crate::{
    DeferredMutation, DeferredMutationWriter, EntityHandle, GameError, GameErrorCode, GameResult,
    Value,
};
pub use project_runtime_sdk::ProjectRuntimeCharacterMode3D as CharacterMode3D;
use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;
pub use project_runtime_sdk::{
    ProjectRuntimeSpatialAssetRef as SpatialAssetRef, ProjectRuntimeTransform3D as Transform3D,
};

pub(crate) fn read_transform(value: Value) -> GameResult<Transform3D> {
    let Value::Object(fields) = value else {
        return Err(invalid("Expected typed Transform3D"));
    };
    let vec3 = |name| match fields.get(name) {
        Some(Value::Vec3(v)) => Ok(*v),
        _ => Err(invalid("Transform3D vector is absent")),
    };
    let Some(Value::Array(values)) = fields.get("rotationXyzw") else {
        return Err(invalid("Transform3D quaternion is absent"));
    };
    let mut rotation_xyzw = [0.0; 4];
    if values.len() != 4 {
        return Err(invalid("Expected xyzw quaternion"));
    }
    for (out, v) in rotation_xyzw.iter_mut().zip(values) {
        *out = match v {
            Value::Number(n) => *n as f32,
            _ => return Err(invalid("Expected quaternion numbers")),
        };
    }
    Ok(Transform3D {
        translation: vec3("translation")?,
        rotation_xyzw,
        scale: vec3("scale")?,
    })
}
fn invalid(message: &str) -> GameError {
    GameError::new(GameErrorCode::InvalidArgument, message)
}
impl DeferredMutationWriter {
    pub fn capture_world_logical3d(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
    ) -> GameResult<()> {
        if request_id == 0 {
            return Err(invalid("Persistence request must be nonzero"));
        }
        self.spatial_intent(entity, Intent::WorldLogicalCapture { request_id })
    }
    pub fn restore_world_logical3d(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
        snapshot: project_runtime_sdk::WorldLogicalSave3D,
    ) -> GameResult<()> {
        if request_id == 0 {
            return Err(invalid("Persistence request must be nonzero"));
        }
        self.spatial_intent(
            entity,
            Intent::WorldLogicalRestore {
                request_id,
                snapshot,
            },
        )
    }
    pub fn world_streaming_sources3d(
        &mut self,
        entity: &EntityHandle,
        sources: Vec<project_runtime_sdk::ProjectRuntimeStreamingSource3D>,
    ) -> GameResult<()> {
        let mut ids = std::collections::BTreeSet::new();
        if sources.len() > 16 || sources.iter().any(|s| !s.validate() || !ids.insert(&s.id)) {
            return Err(invalid(
                "Expected at most 16 uniquely identified, finite streaming sources",
            ));
        }
        self.spatial_intent(entity, Intent::WorldStreamingSources { sources })
    }
    pub fn character_mode3d(
        &mut self,
        entity: &EntityHandle,
        mode: CharacterMode3D,
    ) -> GameResult<()> {
        self.spatial_intent(entity, Intent::CharacterMode { mode })
    }
    pub fn navigation_target3d(
        &mut self,
        entity: &EntityHandle,
        target: [f64; 3],
        deadline_ticks: u32,
    ) -> GameResult<()> {
        if target.iter().any(|v| !v.is_finite()) || !(1..=600).contains(&deadline_ticks) {
            return Err(invalid(
                "Finite navigation target and deadline 1..600 ticks required",
            ));
        }
        self.spatial_intent(
            entity,
            Intent::NavigationTarget {
                target,
                deadline_ticks,
            },
        )
    }
    pub fn navigation_cancel3d(&mut self, entity: &EntityHandle) -> GameResult<()> {
        self.spatial_intent(entity, Intent::NavigationCancel {})
    }

    pub fn body_velocity3d(&mut self, entity: &EntityHandle, velocity: [f32; 3]) -> GameResult<()> {
        if velocity.iter().any(|v| !v.is_finite()) {
            return Err(invalid("Body velocity must be finite"));
        }
        self.spatial_intent(entity, Intent::BodyVelocity { velocity })
    }
    pub fn body_impulse3d(&mut self, entity: &EntityHandle, impulse: [f32; 3]) -> GameResult<()> {
        if impulse.iter().any(|v| !v.is_finite()) {
            return Err(invalid("Body impulse must be finite"));
        }
        self.spatial_intent(entity, Intent::BodyImpulse { impulse })
    }
    pub fn motor_move3d(
        &mut self,
        entity: &EntityHandle,
        displacement: [f32; 3],
    ) -> GameResult<()> {
        if displacement.iter().any(|v| !v.is_finite()) {
            return Err(invalid("Motor displacement must be finite"));
        }
        self.spatial_intent(entity, Intent::MotorMove { displacement })
    }
    pub fn motor_jump3d(&mut self, entity: &EntityHandle, speed: f32) -> GameResult<()> {
        if !speed.is_finite() || speed < 0. {
            return Err(invalid("Jump speed must be finite and nonnegative"));
        }
        self.spatial_intent(entity, Intent::MotorJump { speed })
    }
    pub fn character_parameter3d(
        &mut self,
        entity: &EntityHandle,
        name: impl Into<String>,
        value: f32,
    ) -> GameResult<()> {
        if !value.is_finite() {
            return Err(invalid("Character parameter must be finite"));
        }
        self.spatial_intent(
            entity,
            Intent::CharacterParameter {
                name: name.into(),
                value,
            },
        )
    }
    pub fn character_play3d(
        &mut self,
        entity: &EntityHandle,
        state: impl Into<String>,
        reset: bool,
    ) -> GameResult<()> {
        self.spatial_intent(
            entity,
            Intent::CharacterPlay {
                state: state.into(),
                reset,
            },
        )
    }
    pub fn character_seek3d(&mut self, entity: &EntityHandle, seconds: f64) -> GameResult<()> {
        if !seconds.is_finite() || seconds < 0. {
            return Err(invalid("Character seek must be finite and nonnegative"));
        }
        self.spatial_intent(entity, Intent::CharacterSeek { seconds })
    }
    pub fn character_pause3d(&mut self, entity: &EntityHandle, paused: bool) -> GameResult<()> {
        self.spatial_intent(entity, Intent::CharacterPaused { paused })
    }
    fn spatial_intent(&mut self, entity: &EntityHandle, intent: Intent) -> GameResult<()> {
        if entity.generation() == 0 {
            return Err(invalid(
                "Resolve a live EntityHandle through WorldRead before spatial mutation",
            ));
        }
        self.mutations.push(DeferredMutation::Spatial {
            entity_id: entity.identity().into(),
            generation: entity.generation(),
            intent,
        });
        Ok(())
    }
    /// Metres and quaternion xyzw; committed atomically after the session callback.
    pub fn write_local_transform3d(
        &mut self,
        entity: &EntityHandle,
        transform: Transform3D,
    ) -> GameResult<()> {
        self.spatial_intent(entity, Intent::SetLocalTransform { transform })
    }
    pub fn set_mesh_visible(&mut self, entity: &EntityHandle, visible: bool) -> GameResult<()> {
        self.spatial_intent(entity, Intent::SetMeshVisible { visible })
    }
    pub fn set_material(
        &mut self,
        entity: &EntityHandle,
        slot: impl Into<String>,
        material: SpatialAssetRef,
    ) -> GameResult<()> {
        self.spatial_intent(
            entity,
            Intent::SetMaterial {
                slot: slot.into(),
                material,
            },
        )
    }
    /// Change an existing scene light through the same generation-checked transaction.
    pub fn set_light_enabled(&mut self, entity: &EntityHandle, enabled: bool) -> GameResult<()> {
        self.spatial_intent(entity, Intent::SetLightEnabled { enabled })
    }
    /// Linear RGB, not sRGB. Rejected locally without staging an invalid command.
    pub fn set_light_color(&mut self, entity: &EntityHandle, color: [f32; 3]) -> GameResult<()> {
        if color.iter().any(|v| !v.is_finite() || *v < 0.) {
            return Err(invalid("Light color must be finite nonnegative linear RGB"));
        }
        self.spatial_intent(entity, Intent::SetLightColor { color })
    }
    /// Lux for authored directional lights; candela for authored point/spot lights.
    pub fn set_light_intensity(&mut self, entity: &EntityHandle, intensity: f32) -> GameResult<()> {
        if !intensity.is_finite() || intensity < 0. {
            return Err(invalid("Light intensity must be finite and nonnegative"));
        }
        self.spatial_intent(entity, Intent::SetLightIntensity { intensity })
    }
    pub fn select_main_camera(&mut self, entity: &EntityHandle) -> GameResult<()> {
        self.spatial_intent(entity, Intent::SelectMainCamera {})
    }
}

/// A request correlation, not an entity. Resolve after a successful deferred commit.
/// Valid until the next successful batch containing prefab spawns in this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnTicket {
    pub(crate) request_id: u64,
}
impl SpawnTicket {
    pub fn request_id(self) -> u64 {
        self.request_id
    }
}
impl DeferredMutationWriter {
    pub fn instantiate_spatial_prefab(
        &mut self,
        request_id: u64,
        prefab: SpatialAssetRef,
    ) -> GameResult<SpawnTicket> {
        if request_id == 0 {
            return Err(invalid(
                "Spawn request ID must be nonzero and unique within the batch",
            ));
        }
        self.mutations
            .push(DeferredMutation::InstantiateSpatialPrefab { request_id, prefab });
        Ok(SpawnTicket { request_id })
    }
    pub fn despawn_spatial_prefab(&mut self, root: &EntityHandle) -> GameResult<()> {
        if root.generation() == 0 {
            return Err(invalid(
                "Resolve the actual prefab root handle before despawn",
            ));
        }
        self.mutations.push(DeferredMutation::DespawnSpatialPrefab {
            entity_id: root.identity().into(),
            generation: root.generation(),
        });
        Ok(())
    }
}

impl DeferredMutationWriter {
    /// One callback transaction; validation failure leaves the writer unchanged.
    /// The engine checks the configurable combined budget at the tick safe point.
    pub fn write_local_transforms3d(
        &mut self,
        updates: &[(EntityHandle, Transform3D)],
    ) -> GameResult<()> {
        if updates.iter().any(|(entity, _)| entity.generation() == 0) {
            return Err(invalid(
                "Resolve every handle before submitting a spatial batch",
            ));
        }
        for (_, t) in updates {
            if t.translation
                .iter()
                .chain(t.rotation_xyzw.iter())
                .chain(t.scale.iter())
                .any(|v| !v.is_finite())
                || t.scale.contains(&0.)
                || !matches!(t.rotation_xyzw.iter().map(|v|v*v).sum::<f32>(),norm if norm.is_finite() && norm>0.)
            {
                return Err(invalid("Spatial batch contains an invalid transform"));
            }
        }
        self.mutations
            .extend(
                updates
                    .iter()
                    .map(|(entity, transform)| DeferredMutation::Spatial {
                        entity_id: entity.identity().into(),
                        generation: entity.generation(),
                        intent: Intent::SetLocalTransform {
                            transform: transform.clone(),
                        },
                    }),
            );
        Ok(())
    }
    pub fn teleport3d(&mut self, entity: &EntityHandle, transform: Transform3D) -> GameResult<()> {
        self.spatial_intent(entity, Intent::Teleport { transform })
    }
    pub fn set_spatial_interpolation(
        &mut self,
        entity: &EntityHandle,
        enabled: bool,
    ) -> GameResult<()> {
        self.spatial_intent(entity, Intent::SetInterpolation { enabled })
    }
}

impl DeferredMutationWriter {
    pub fn reparent3d(
        &mut self,
        entity: &EntityHandle,
        parent: Option<&EntityHandle>,
        preserve_world: bool,
    ) -> GameResult<()> {
        if parent.is_some_and(|p| p.generation() == 0) {
            return Err(invalid("Resolve the current parent handle"));
        }
        self.spatial_intent(
            entity,
            Intent::Reparent {
                parent_entity_id: parent.map(|p| p.identity().into()),
                parent_generation: parent.map_or(0, |p| p.generation()),
                preserve_world,
            },
        )
    }
    /// Root anchor in metres. Keep-world rebases local displacement; unrepresentable results fail atomically.
    pub fn set_world_anchor3d(
        &mut self,
        entity: &EntityHandle,
        anchor: [f64; 3],
        preserve_world: bool,
    ) -> GameResult<()> {
        if anchor.iter().any(|v| !v.is_finite()) {
            return Err(invalid("Anchor must be finite"));
        }
        self.spatial_intent(
            entity,
            Intent::SetWorldAnchor {
                anchor,
                preserve_world,
            },
        )
    }
}

/// A non-owning World-scoped runtime identity. Every commit rechecks epoch and generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialEntityHandle(project_runtime_sdk::spatial_bulk::SpatialEntityKey);
impl SpatialEntityHandle {
    pub fn from_runtime_key(
        key: project_runtime_sdk::spatial_bulk::SpatialEntityKey,
    ) -> GameResult<Self> {
        if !key.is_valid() {
            return Err(invalid("Invalid spatial runtime identity"));
        }
        Ok(Self(key))
    }
    pub fn runtime_key(self) -> project_runtime_sdk::spatial_bulk::SpatialEntityKey {
        self.0
    }
}
impl DeferredMutationWriter {
    /// Validates the complete tail before staging any item. The host owns final World validation.
    pub fn write_spatial_transforms3d(
        &mut self,
        updates: &[(SpatialEntityHandle, Transform3D)],
    ) -> GameResult<()> {
        if updates.len() > project_runtime_sdk::spatial_bulk::MAX_SPATIAL_ITEMS
            || updates.iter().any(|(h, t)| {
                !h.0.is_valid() || !project_runtime_sdk::spatial_bulk::valid_transform(t)
            })
        {
            return Err(invalid("Invalid or oversized typed spatial batch"));
        }
        self.mutations
            .extend(updates.iter().map(|(entity, transform)| {
                DeferredMutation::SpatialLocalTransform {
                    entity: entity.0,
                    transform: transform.clone(),
                }
            }));
        Ok(())
    }
}
