//! Existing FrameLoop animation stage. Mutable clocks stay private; World receives committed poses.
use super::binding::{dynamic_json, runtime_json, CharacterControl3D};
use super::*;
use crate::{
    archetype::ComponentValue, components::ComponentTypeId, ids::EntityId, query::QuerySpec,
    runtime_asset_loader::RuntimeAssetLoader, world::World,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CommittedCharacterPose {
    pub tick: u64,
    pub asset: Arc<CharacterAsset>,
    pub current: SampledCharacterPose,
    pub previous: SampledCharacterPose,
    pub visual: SampledCharacterPose,
    pub previous_visual: SampledCharacterPose,
    pub visual_updated: bool,
    pub skin_sources: Vec<Arc<SkinVertexData3D>>,
    pub history_reset: bool,
    pub mode: CharacterMode3D,
    pub joints: Vec<EntityId>,
    pub events: Vec<AnimationEventOccurrence>,
    pub state: String,
}
#[derive(Debug)]
struct Live {
    generation: u64,
    binding: CharacterBinding3D,
    asset: Arc<CharacterAsset>,
    animator: Option<Animator3D>,
    target_generations: Vec<u64>,
    skin_sources: Vec<Arc<SkinVertexData3D>>,
    revision: u64,
    seek_revision: u64,
    play_revision: u64,
    mode: CharacterMode3D,
    mode_initialized: bool,
}
#[derive(Debug)]
struct Pending {
    tick: u64,
    pose: SampledCharacterPose,
    events: Vec<AnimationEventOccurrence>,
    state: String,
    history_reset: bool,
}
#[derive(Debug, Default)]
pub struct CharacterModule {
    pending: BTreeMap<EntityId, Pending>,
    epoch: Option<u64>,
    live: BTreeMap<EntityId, Live>,
    assets: BTreeMap<String, Weak<CharacterAsset>>,
}

fn data(world: &World, id: &EntityId, ty: &str) -> Option<serde_json::Value> {
    match world.component_value(id, &ComponentTypeId::new(ty)) {
        Some(ComponentValue::Dynamic { value, .. }) => Some(dynamic_json(&value)),
        _ => None,
    }
}
impl CharacterModule {
    fn load_asset(
        &mut self,
        loader: &mut RuntimeAssetLoader,
        reference: &crate::spatial_contract::SpatialAssetRef,
    ) -> ContractResult<Arc<CharacterAsset>> {
        let record = loader
            .asset_index()
            .resolve(&reference.0)
            .map_err(|_| invalid("/character", "Character asset reference is unresolved."))?;
        if record.asset_id != reference.0.id {
            return Err(invalid("/character", "Character id and GUID disagree."));
        }
        let identity = record
            .hash
            .clone()
            .ok_or_else(|| invalid("/character", "Cooked character requires a content hash."))?;
        if let Some(asset) = self.assets.get(&identity).and_then(Weak::upgrade) {
            return Ok(asset);
        }
        let handle = loader.load(&reference.0).map_err(|_| {
            invalid(
                "/character",
                "Character asset load failed; see asset diagnostics.",
            )
        })?;
        let decoded = loader
            .get_asset_bytes(&handle)
            .ok_or_else(|| invalid("/character", "Character bytes are not ready."))
            .and_then(|bytes| {
                if crate::canonical_digest::sha256_prefixed(&bytes) != identity {
                    return Err(invalid("/character", "Character cooked hash mismatch."));
                }
                let asset: CharacterAsset = serde_json::from_slice(&bytes)
                    .map_err(|e| invalid("/character", e.to_string()))?;
                asset.validate()?;
                if asset.asset_id != reference.0.id {
                    return Err(invalid("/character", "Payload asset identity mismatch."));
                }
                Ok(Arc::new(asset))
            });
        loader
            .release(&handle)
            .map_err(|_| invalid("/character", "Failed releasing character load lease."))?;
        let asset = decoded?;
        self.assets.insert(identity, Arc::downgrade(&asset));
        Ok(asset)
    }
    fn bind(
        &mut self,
        world: &World,
        loader: &mut RuntimeAssetLoader,
        id: &EntityId,
        generation: u64,
    ) -> ContractResult<Live> {
        let binding: CharacterBinding3D = serde_json::from_value(
            data(world, id, "engine.character3d")
                .ok_or_else(|| invalid("/binding", "Missing character binding."))?,
        )
        .map_err(|e| invalid("/binding", e.to_string()))?;
        binding.validate()?;
        let asset = self.load_asset(loader, &binding.character)?;
        let animation_asset = if let Some(retarget) = &binding.retarget {
            let source = self.load_asset(loader, &retarget.source)?;
            let pose = SampledCharacterPose {
                joints: source.joints.iter().map(|j| j.bind.clone()).collect(),
                morph_weights: Vec::new(),
            };
            retarget.mapping.apply(&source, &asset, &pose)?;
            source
        } else {
            asset.clone()
        };
        for ik in binding.simulation_ik.iter().chain(&binding.visual_ik) {
            ik.validate(&asset)?;
        }
        if binding.joints.len() != asset.joints.len()
            || binding.joints.iter().any(|j| {
                world
                    .transform3d(&EntityId::from(j.entity_ref.clone()))
                    .is_none()
            })
        {
            return Err(invalid(
                "/joints",
                "Every ordered joint requires a live Transform3D target.",
            ));
        }
        for (i, joint) in asset.joints.iter().enumerate() {
            if let Some(parent) = joint.parent {
                if world
                    .hierarchy(&EntityId::from(binding.joints[i].entity_ref.clone()))
                    .and_then(|h| h.parent_id)
                    != Some(EntityId::from(
                        binding.joints[parent as usize].entity_ref.clone(),
                    ))
                {
                    return Err(invalid(
                        "/joints/parent",
                        "Bound joint hierarchy disagrees with the cooked skeleton.",
                    ));
                }
            }
        }
        let graph = binding.graph.clone().or_else(|| {
            animation_asset.clips.first().map(|first| AnimationGraph3D {
                initial: first.key.clone(),
                parameters: BTreeMap::new(),
                states: animation_asset
                    .clips
                    .iter()
                    .map(|clip| AnimationState3D {
                        key: clip.key.clone(),
                        clip: clip.key.clone(),
                        looping: true,
                    })
                    .collect(),
                transitions: Vec::new(),
                layers: Vec::new(),
            })
        });
        super::modes::validate_modes(world, id, &binding, &asset)?;
        let mut animator = graph
            .map(|graph| Animator3D::new(animation_asset, graph))
            .transpose()?
            .map(|mut a| {
                a.set_paused(!binding.play_on_awake);
                a
            });
        if let (Some(root), Some(animator)) = (&binding.root_motion, &mut animator) {
            let key = if let Some(retarget) = &binding.retarget {
                &retarget
                    .mapping
                    .joints
                    .iter()
                    .find(|m| m.target == root.joint)
                    .ok_or_else(|| {
                        invalid(
                            "/rootMotion/retarget",
                            "Root requires an explicit source map.",
                        )
                    })?
                    .source
            } else {
                &root.joint
            };
            let index = animator
                .asset()
                .joints
                .iter()
                .position(|j| &j.key == key)
                .ok_or_else(|| invalid("/rootMotion/joint", "Unknown source root."))?;
            animator.track_root_motion(Some(index))?;
        }
        let target_generations = binding
            .joints
            .iter()
            .map(|j| {
                world
                    .entity_generation(&EntityId::from(j.entity_ref.clone()))
                    .unwrap()
            })
            .collect();
        let skin_sources = asset
            .skins
            .iter()
            .map(|s| Arc::new(SkinVertexData3D::from_skin(s)))
            .collect();
        let mode = binding.initial_mode;
        Ok(Live {
            generation,
            binding,
            asset,
            animator,
            target_generations,
            skin_sources,
            revision: 0,
            seek_revision: 0,
            play_revision: 0,
            mode,
            mode_initialized: false,
        })
    }
    /// Called after project intents and before spatial tick publication, including offscreen rigs.
    pub fn advance(
        &mut self,
        world: &mut World,
        loader: &mut RuntimeAssetLoader,
        tick: u64,
        dt: f32,
    ) -> ContractResult<()> {
        self.prepare(world, loader, tick, dt)?;
        self.publish(world, tick)
    }
    /// Sample once and submit motor requests before physics consumes the tick.
    pub fn prepare(
        &mut self,
        world: &mut World,
        loader: &mut RuntimeAssetLoader,
        tick: u64,
        dt: f32,
    ) -> ContractResult<()> {
        if !self.pending.is_empty() {
            return Err(invalid(
                "/stage",
                "Previous character tick was not published.",
            ));
        }
        if self.epoch != Some(world.epoch()) {
            self.live.clear();
            self.pending.clear();
            self.assets.clear();
            world.character_poses.clear();
            world.character_skins.clear();
            self.epoch = Some(world.epoch());
        }
        let ids = world
            .query_entities(&QuerySpec::all([ComponentTypeId::new(
                "engine.character3d",
            )]))
            .into_iter()
            .filter(|id| world.entity(id).is_some_and(|e| e.alive && e.enabled))
            .collect::<Vec<_>>();
        if ids.len() > 1024 {
            return Err(invalid(
                "/characters",
                "Character instance budget exceeded.",
            ));
        }
        self.live.retain(|id, live| {
            ids.binary_search(id).is_ok() && world.entity_generation(id) == Some(live.generation)
        });
        world
            .character_poses
            .retain(|id, _| self.live.contains_key(id));
        for id in ids {
            let generation = world
                .entity_generation(&id)
                .ok_or_else(|| invalid("/entity", "Stale character entity."))?;
            if !self.live.contains_key(&id) {
                let live = self.bind(world, loader, &id, generation)?;
                self.live.insert(id.clone(), live);
            }
            let live = self.live.get_mut(&id).unwrap();
            let mut history_reset = !world.character_poses.contains_key(&id)
                || world.spatial_delta().changes.iter().any(|change| {
                    (change.entity_id == id
                        || live
                            .binding
                            .joints
                            .iter()
                            .any(|j| j.entity_ref == change.entity_id.as_str()))
                        && change
                            .field_mask
                            .contains(crate::world::SpatialFieldMask::HISTORY)
                });
            // Targets may despawn independently. Fail before any animation writes.
            if live
                .binding
                .joints
                .iter()
                .zip(&live.target_generations)
                .any(|(j, g)| {
                    world.entity_generation(&EntityId::from(j.entity_ref.clone())) != Some(*g)
                })
            {
                return Err(invalid(
                    "/joint",
                    "Character joint target retired; a replacement cannot inherit its pose.",
                ));
            }
            let old_mode = live.mode;
            if let Some(value) = data(world, &id, "engine.character3d_control") {
                let control: CharacterControl3D = serde_json::from_value(value)
                    .map_err(|e| invalid("/control", e.to_string()))?;
                if control.revision != live.revision {
                    if let Some(mode) = control.mode {
                        live.mode = mode;
                    }
                    if let Some(animator) = &mut live.animator {
                        for (name, value) in &control.parameters {
                            animator.set_parameter(name, *value)?;
                        }
                        if let Some((revision, state, reset)) = control
                            .play
                            .filter(|(revision, _, _)| *revision > live.play_revision)
                        {
                            animator.play(&state, reset)?;
                            live.play_revision = revision;
                            history_reset = true;
                        }
                        if let Some((revision, time)) = control
                            .seek
                            .filter(|(revision, _)| *revision > live.seek_revision)
                        {
                            animator.seek(time)?;
                            live.seek_revision = revision;
                            history_reset = true;
                        }
                        if let Some(paused) = control.paused {
                            animator.set_paused(paused);
                        }
                    } else if !control.parameters.is_empty()
                        || control.play.is_some()
                        || control.seek.is_some()
                    {
                        return Err(invalid("/control", "Static rig has no graph."));
                    }
                    live.revision = control.revision;
                }
            }
            if !live.mode_initialized || live.mode != old_mode {
                super::modes::apply_mode(world, &id, &live.binding, &live.asset, live.mode)?;
                live.mode_initialized = true;
                history_reset = true;
            }
            if live.mode != CharacterMode3D::Ragdoll
                && live.binding.joints.iter().any(|joint| {
                    data(
                        world,
                        &EntityId::from(joint.entity_ref.clone()),
                        "engine.body3d",
                    )
                    .is_some_and(|value| value["motion"] == "dynamic" && value["enabled"] != false)
                })
            {
                return Err(invalid(
                    "/ownership",
                    "Animation cannot write joints owned by dynamic bodies.",
                ));
            }
            let (mut pose, events, state) = if let Some(animator) = &mut live.animator {
                animator.tick(
                    tick,
                    if live.mode == CharacterMode3D::Ragdoll {
                        0.
                    } else {
                        dt
                    },
                )?;
                let pose = if let Some(retarget) = &live.binding.retarget {
                    retarget
                        .mapping
                        .apply(animator.asset(), &live.asset, &animator.pose)?
                } else {
                    animator.pose.clone()
                };
                (pose, animator.events.clone(), animator.state().to_string())
            } else {
                (
                    SampledCharacterPose {
                        joints: live.asset.joints.iter().map(|j| j.bind.clone()).collect(),
                        morph_weights: live
                            .asset
                            .skins
                            .iter()
                            .map(|s| (s.mesh_joint, s.morph_weights.clone()))
                            .collect(),
                    },
                    Vec::new(),
                    String::new(),
                )
            };
            if live.mode == CharacterMode3D::RootMotion {
                let config = live.binding.root_motion.as_ref().unwrap();
                let index = live
                    .asset
                    .joints
                    .iter()
                    .position(|j| j.key == config.joint)
                    .unwrap();
                let scale = live.binding.retarget.as_ref().map_or(1., |r| {
                    r.mapping
                        .joints
                        .iter()
                        .find(|m| m.target == config.joint)
                        .unwrap()
                        .translation_scale
                });
                let delta = live
                    .animator
                    .as_ref()
                    .ok_or_else(|| {
                        invalid("/rootMotion", "Root motion requires an animation graph.")
                    })?
                    .root_displacement;
                let delta =
                    std::array::from_fn(|i| if config.axes[i] { delta[i] * scale } else { 0. });
                for i in 0..3 {
                    if config.axes[i] {
                        pose.joints[index].translation[i] =
                            live.asset.joints[index].bind.translation[i];
                    }
                }
                let matrix = world
                    .spatial_world(&id)
                    .ok_or_else(|| invalid("/rootMotion", "Motor has no committed pose."))?
                    .world_matrix;
                let displacement = glam::Mat4::from_cols_array(&matrix)
                    .transform_vector3(glam::Vec3::from_array(delta))
                    .to_array();
                let control = crate::physics_3d::prepare_control(
                    world,
                    &BTreeMap::new(),
                    &id,
                    project_runtime_sdk::ProjectRuntimeSpatialIntent::MotorMove { displacement },
                )?;
                let ComponentValue::Dynamic {
                    component_type,
                    value,
                } = control
                else {
                    unreachable!()
                };
                world
                    .try_insert_dynamic_component(id.clone(), component_type, value)
                    .map_err(|e| invalid("/rootMotion", e.message))?;
            }
            self.pending.insert(
                id,
                Pending {
                    tick,
                    pose,
                    events,
                    state,
                    history_reset,
                },
            );
        }
        self.assets.retain(|_, asset| asset.strong_count() > 0);
        Ok(())
    }
    /// Called after the motor/rigid-body World commit. No clock advances here.
    pub fn publish(&mut self, world: &mut World, tick: u64) -> ContractResult<()> {
        if self.pending.len() <= 1 {
            return self.publish_sequential(world, tick);
        }
        world.with_deferred_spatial_delta(|world| self.publish_sequential(world, tick))
    }

    fn publish_sequential(&mut self, world: &mut World, tick: u64) -> ContractResult<()> {
        world.character_skins.clear();
        for (id, pending) in std::mem::take(&mut self.pending) {
            if pending.tick != tick {
                return Err(invalid("/stage", "Character publish tick mismatch."));
            }
            let live = self
                .live
                .get(&id)
                .ok_or_else(|| invalid("/stage", "Character owner retired before publication."))?;
            let Pending {
                mut pose,
                events,
                state,
                history_reset,
                ..
            } = pending;
            if live.mode == CharacterMode3D::Ragdoll {
                for (target, joint) in live.binding.joints.iter().zip(&mut pose.joints) {
                    let t = world
                        .transform3d(&EntityId::from(target.entity_ref.clone()))
                        .ok_or_else(|| invalid("/ragdoll", "Ragdoll target retired."))?;
                    joint.translation = t.translation;
                    joint.rotation = t.rotation_xyzw;
                    joint.scale = t.scale;
                }
            }
            if live.mode != CharacterMode3D::Ragdoll {
                for ik in &live.binding.simulation_ik {
                    ik.apply(&live.asset, &mut pose)?;
                }
            }
            let visible = world.visibility3d(&id).is_none_or(|v| v.visible)
                && live.binding.visual_max_distance.is_none_or(|max| {
                    world
                        .main_camera()
                        .and_then(|camera| world.spatial_world(camera))
                        .zip(world.spatial_world(&id))
                        .is_none_or(|(camera, owner)| {
                            let distance = (0..3)
                                .map(|i| {
                                    let d = (camera.anchor[i]
                                        + f64::from(camera.world_matrix[12 + i]))
                                        - (owner.anchor[i] + f64::from(owner.world_matrix[12 + i]));
                                    d * d
                                })
                                .sum::<f64>()
                                .sqrt();
                            distance <= max
                        })
                });
            let visual_updated = history_reset
                || AnimationLod3D {
                    visual_interval: live.binding.visual_interval,
                }
                .evaluate_visual(tick, visible);
            let mut visual = if visual_updated {
                pose.clone()
            } else {
                world
                    .character_poses
                    .get(&id)
                    .map_or_else(|| pose.clone(), |p| p.visual.clone())
            };
            if visual_updated && live.mode != CharacterMode3D::Ragdoll {
                for ik in &live.binding.visual_ik {
                    ik.apply(&live.asset, &mut visual)?;
                }
            }
            let mut writes = Vec::with_capacity(pose.joints.len());
            let mut targets = Vec::with_capacity(pose.joints.len());
            for (target, pose) in live.binding.joints.iter().zip(&pose.joints) {
                let target = EntityId::from(target.entity_ref.clone());
                let runtime = world.runtime_id_for_source(&target).unwrap();
                targets.push(target);
                writes.push((
                    project_runtime_sdk::spatial_bulk::SpatialEntityKey {
                        world_epoch: world.epoch(),
                        index: runtime.index,
                        generation: runtime.generation,
                    },
                    project_runtime_sdk::ProjectRuntimeTransform3D {
                        translation: pose.translation,
                        rotation_xyzw: pose.rotation,
                        scale: pose.scale,
                    },
                ));
            }
            if live.mode != CharacterMode3D::Ragdoll {
                world
                    .commit_spatial_local_batch(writes)
                    .map_err(|e| invalid("/commit", e.message))?;
            }
            let previous = world
                .character_poses
                .get(&id)
                .filter(|_| !history_reset)
                .map_or_else(|| pose.clone(), |old| old.current.clone());
            let previous_visual = world
                .character_poses
                .get(&id)
                .filter(|_| !history_reset)
                .map_or_else(|| visual.clone(), |old| old.visual.clone());
            let frame = Arc::new(CommittedCharacterPose {
                tick,
                asset: live.asset.clone(),
                current: pose,
                previous,
                visual,
                previous_visual,
                visual_updated,
                skin_sources: live.skin_sources.clone(),
                history_reset,
                mode: live.mode,
                joints: targets,
                events,
                state,
            });
            for (index, skin) in live.asset.skins.iter().enumerate() {
                let current = live.asset.skin_matrices(index, &frame.visual)?;
                let previous = live.asset.skin_matrices(index, &frame.previous_visual)?;
                let weights = frame
                    .visual
                    .morph_weights
                    .iter()
                    .find(|(j, _)| *j == skin.mesh_joint)
                    .map(|(_, w)| w.as_slice())
                    .unwrap_or(&skin.morph_weights);
                let previous_weights = frame
                    .previous_visual
                    .morph_weights
                    .iter()
                    .find(|(j, _)| *j == skin.mesh_joint)
                    .map(|(_, w)| w.as_slice())
                    .unwrap_or(&skin.morph_weights);
                let deformation = Arc::new(SkinDeformation3D::new(
                    live.skin_sources[index].clone(),
                    &current,
                    &previous,
                    weights,
                    previous_weights,
                    history_reset,
                )?);
                let target = frame.joints[skin.mesh_joint as usize].clone();
                if world
                    .mesh_renderer3d(&target)
                    .is_none_or(|m| m.mesh.0.id != skin.mesh_asset_id)
                {
                    return Err(invalid(
                        "/skin/mesh",
                        "Bound skin mesh disagrees with the cooked mesh identity.",
                    ));
                }
                if world
                    .character_skins
                    .insert(target.clone(), deformation)
                    .is_some()
                {
                    return Err(invalid(
                        "/skin/mesh",
                        "Two character owners cannot deform the same mesh.",
                    ));
                }
                world.mark_render_payload_dirty(target);
            }
            let status = serde_json::json!({"tick":tick,"state":frame.state,"mode":frame.mode,"events":frame.events.iter().map(|e|serde_json::json!({"name":e.name,"cycle":e.cycle,"time":e.time})).collect::<Vec<_>>()});
            world
                .try_insert_dynamic_component(
                    id.clone(),
                    ComponentTypeId::new("engine.character3d_state"),
                    runtime_json(status),
                )
                .map_err(|e| invalid("/state", e.message))?;
            world.character_poses.insert(id, frame);
        }
        Ok(())
    }
}

impl World {
    pub fn character_pose(&self, id: &EntityId) -> Option<Arc<CommittedCharacterPose>> {
        self.character_poses.get(id).cloned()
    }
    pub fn character_skin(&self, id: &EntityId) -> Option<Arc<SkinDeformation3D>> {
        self.character_skins.get(id).cloned()
    }
    /// Gameplay attachment reads committed simulation pose, never a visual IK result.
    pub fn character_joint_matrix(&self, id: &EntityId, key: &str) -> ContractResult<[f32; 16]> {
        let frame = self
            .character_poses
            .get(id)
            .ok_or_else(|| invalid("/character", "No committed pose."))?;
        let index = frame
            .asset
            .joints
            .iter()
            .position(|j| j.key == key)
            .ok_or_else(|| invalid("/joint", "Unknown stable joint key."))?;
        Ok(frame.asset.global_matrices(&frame.current)?[index])
    }
    pub fn character_visual_joint_matrix(
        &self,
        id: &EntityId,
        key: &str,
    ) -> ContractResult<[f32; 16]> {
        let frame = self
            .character_poses
            .get(id)
            .ok_or_else(|| invalid("/character", "No committed pose."))?;
        let index = frame
            .asset
            .joints
            .iter()
            .position(|j| j.key == key)
            .ok_or_else(|| invalid("/joint", "Unknown stable joint key."))?;
        Ok(frame.asset.global_matrices(&frame.visual)?[index])
    }
}
