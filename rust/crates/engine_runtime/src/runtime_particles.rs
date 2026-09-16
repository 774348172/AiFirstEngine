//! RenderProjectionAdapter<ParticleEffect>: per-source control and immutable package assets.
//! Particle records, simulation, and retirement remain in the renderer.
use crate::{
    components::ComponentTypeId,
    ids::{EntityId, RuntimeEntityId},
    particle_effect::*,
    particle_render_contract::{ParticleRenderStep, ParticleSourceFrame},
    query::QuerySpec,
    runtime_asset::RuntimeAssetHandle,
    runtime_asset_loader::RuntimeAssetLoader,
    runtime_package::RuntimeAssetRef,
    runtime_texture::RuntimeTexturePayload,
    world::World,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleEffect {
    pub effect_ref: RuntimeAssetRef,
    #[serde(default)]
    pub play_on_awake: bool,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub parameters: BTreeMap<String, ParticleValue>,
}
pub fn decode_particle_effect(value: &serde_json::Value) -> Result<ParticleEffect, String> {
    let source: ParticleEffect =
        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if source.effect_ref.asset_type != "particle-effect" || source.effect_ref.id.trim().is_empty() {
        return Err("particle_effect.invalid_effect_ref".into());
    }
    Ok(source)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticleAction {
    Play,
    Restart,
    StopEmitting,
    Clear,
    SetPaused(bool),
    SetParameter { name: String, value: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleCommand {
    pub entity_id: EntityId,
    pub runtime_id: RuntimeEntityId,
    pub action: ParticleAction,
}

/// Ordinary unlit material bytes. No arbitrary shader or material graph is accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleMaterialData {
    pub base_color: [f32; 4],
    #[serde(default)]
    pub texture: Option<RuntimeAssetRef>,
}
impl ParticleMaterialData {
    pub fn validate(&self) -> Result<(), String> {
        if !self
            .base_color
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
        {
            return Err("material.invalid_base_color".into());
        }
        if self
            .texture
            .as_ref()
            .is_some_and(|r| r.asset_type != "texture" || r.id.is_empty() || r.sub_asset.is_some())
        {
            return Err("material.invalid_texture_ref".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleMeshData {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}
impl ParticleMeshData {
    pub fn validate(&self) -> Result<(), String> {
        if self.positions.is_empty()
            || self.positions.len() != self.uvs.len()
            || self.indices.is_empty()
            || self.indices.len() % 3 != 0
            || self.indices.len() > 1_000_000
            || !self
                .positions
                .iter()
                .flatten()
                .chain(self.uvs.iter().flatten())
                .all(|v| v.is_finite())
            || self
                .indices
                .iter()
                .any(|i| *i as usize >= self.positions.len())
        {
            return Err("particle_render.invalid_mesh".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ParticlePreparedAssets {
    pub effect: CookedParticleEffect,
    pub textures: Vec<Option<RuntimeTexturePayload>>,
    pub meshes: Vec<Option<ParticleMeshData>>,
    pub tints: Vec<[f32; 4]>,
}
#[derive(Debug, Clone)]
struct Source {
    entity: EntityId,
    config: ParticleEffect,
    handles: Vec<RuntimeAssetHandle>,
    assets: Arc<str>,
    description: ParticleEffectDescription,
    instance: u64,
    epoch: u64,
    running: bool,
    emitting: bool,
    paused: bool,
    parameters: BTreeMap<String, ParticleValue>,
    parameter_json: Arc<str>,
    textures: Vec<Option<crate::render_resource::RenderResourceHandle>>,
    meshes: Vec<Option<crate::render_resource::RenderResourceHandle>>,
}
#[derive(Debug, Default)]
pub struct RuntimeParticles {
    sources: BTreeMap<RuntimeEntityId, Source>,
    pub diagnostics: Vec<String>,
}
impl RuntimeParticles {
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
    fn error(&mut self, message: String) {
        if self.diagnostics.len() < 16 {
            self.diagnostics.push(message);
        }
    }
    pub fn update(
        &mut self,
        world: &World,
        mut loader: Option<&mut RuntimeAssetLoader>,
        commands: Vec<ParticleCommand>,
        frame: u64,
        delta: f32,
    ) -> Vec<ParticleSourceFrame> {
        self.diagnostics.clear();
        let live = world
            .query_entities(&QuerySpec::all([ComponentTypeId::particle_effect()]))
            .into_iter()
            .filter_map(|entity| {
                Some((
                    world.runtime_id_for_source(&entity)?,
                    (entity.clone(), world.particle_effect(&entity)?.clone()),
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let retired = self
            .sources
            .iter()
            .filter_map(|(id, s)| {
                (!live.get(id).is_some_and(|(_, c)| c == &s.config)).then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in retired {
            let s = self.sources.remove(&id).unwrap();
            if let Some(loader) = loader.as_deref_mut() {
                for h in s.handles {
                    let _ = loader.release(&h);
                }
            }
        }
        for (id, (entity, config)) in &live {
            if self.sources.contains_key(id) {
                continue;
            }
            let Some(loader) = loader.as_deref_mut() else {
                self.error(format!("particle_effect.package_required:{entity}"));
                continue;
            };
            let mut handles = Vec::new();
            match prepare_assets(loader, config, &mut handles) {
                Ok(assets) => {
                    // The renderer may outlive a Host/session. Never reuse its simulation key.
                    static NEXT_INSTANCE: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(1);
                    let instance = NEXT_INSTANCE
                        .fetch_update(
                            std::sync::atomic::Ordering::Relaxed,
                            std::sync::atomic::Ordering::Relaxed,
                            |id| id.checked_add(1),
                        )
                        .expect("particle instance identity exhausted");
                    let mut parameters = assets
                        .effect
                        .description
                        .parameters
                        .iter()
                        .map(|p| (p.name.clone(), p.default.clone()))
                        .collect::<BTreeMap<_, _>>();
                    parameters.extend(config.parameters.clone());
                    let textures = assets
                        .textures
                        .iter()
                        .map(|t| {
                            t.as_ref().map(|t| {
                                crate::runtime_texture::runtime_texture_render_handle(&t.asset_id)
                            })
                        })
                        .collect();
                    let meshes = assets
                        .effect
                        .description
                        .emitters
                        .iter()
                        .map(|e| {
                            if let ParticleGeometry::Mesh { asset } = &e.draw.geometry {
                                let mut h = crate::runtime_texture::runtime_texture_render_handle(
                                    &asset.id,
                                );
                                h.kind = crate::render_resource::RenderResourceKind::MeshBuffer;
                                Some(h)
                            } else {
                                None
                            }
                        })
                        .collect();
                    self.sources.insert(
                        *id,
                        Source {
                            entity: entity.clone(),
                            config: config.clone(),
                            handles,
                            assets: serde_json::to_string(&assets).unwrap().into(),
                            description: assets.effect.description,
                            instance,
                            epoch: 0,
                            running: config.play_on_awake,
                            emitting: config.play_on_awake,
                            paused: config.paused,
                            parameter_json: serde_json::to_string(&parameters).unwrap().into(),
                            parameters,
                            textures,
                            meshes,
                        },
                    );
                }
                Err(e) => {
                    for h in handles {
                        let _ = loader.release(&h);
                    }
                    self.error(format!("particle_effect.asset_failed:{entity}:{e}"));
                }
            }
        }
        for command in commands {
            let Some(source) = self
                .sources
                .get_mut(&command.runtime_id)
                .filter(|s| s.entity == command.entity_id)
            else {
                self.error(format!(
                    "particle_effect.stale_target:{}",
                    command.entity_id
                ));
                continue;
            };
            match command.action {
                ParticleAction::Play => {
                    if !source.running {
                        source.epoch += 1;
                        source.running = true;
                    }
                    source.emitting = true;
                }
                ParticleAction::Restart => {
                    source.epoch += 1;
                    source.running = true;
                    source.emitting = true;
                }
                ParticleAction::StopEmitting => source.emitting = false,
                ParticleAction::Clear => {
                    source.epoch += 1;
                    source.running = false;
                    source.emitting = false;
                }
                ParticleAction::SetPaused(value) => source.paused = value,
                ParticleAction::SetParameter { name, value } => {
                    let value: ParticleValue = match serde_json::from_str(&value) {
                        Ok(v) => v,
                        Err(e) => {
                            self.error(format!("particle_effect.parameter_invalid:{e}"));
                            continue;
                        }
                    };
                    if let Err(error) = validate_parameter(&source.description, &name, &value) {
                        self.error(format!("{error}:{}", command.entity_id));
                        continue;
                    }
                    source.parameters.insert(name, value);
                    source.parameter_json =
                        serde_json::to_string(&source.parameters).unwrap().into();
                }
            }
        }
        self.sources
            .values()
            .map(|s| {
                let p = world
                    .transform(&s.entity)
                    .map(|t| t.local_position)
                    .unwrap_or(crate::math::Vec3::ZERO);
                ParticleSourceFrame {
                    instance: s.instance,
                    epoch: s.epoch,
                    assets: s.assets.clone(),
                    parameters: s.parameter_json.clone(),
                    textures: s.textures.clone(),
                    meshes: s.meshes.clone(),
                    step: ParticleRenderStep {
                        step_id: frame,
                        delta_seconds: delta.into(),
                        origin: [p.x.into(), p.y.into(), p.z.into()],
                        emitting: s.emitting,
                        paused: s.paused || !s.running,
                    },
                }
            })
            .collect()
    }
}
pub fn validate_parameter(
    description: &ParticleEffectDescription,
    name: &str,
    value: &ParticleValue,
) -> Result<(), String> {
    let p = description
        .parameters
        .iter()
        .find(|p| p.name == name)
        .ok_or("particle_effect.parameter_missing")?;
    if p.default.wgsl_type() != value.wgsl_type() {
        return Err("particle_effect.parameter_type".into());
    }
    let components: Vec<f64> = match value {
        ParticleValue::Float(v) => vec![*v as f64],
        ParticleValue::Uint(v) => vec![*v as f64],
        ParticleValue::Vec3(v) => v.iter().map(|v| *v as f64).collect(),
        ParticleValue::Vec4(v) => v.iter().map(|v| *v as f64).collect(),
    };
    if components.iter().any(|v| {
        !v.is_finite()
            || p.range
                .is_some_and(|r| *v < r[0] as f64 || *v > r[1] as f64)
    }) {
        return Err("particle_effect.parameter_range".into());
    }
    Ok(())
}
fn read_asset<T: serde::de::DeserializeOwned>(
    loader: &mut RuntimeAssetLoader,
    reference: &RuntimeAssetRef,
    handles: &mut Vec<RuntimeAssetHandle>,
) -> Result<T, String> {
    let handle = loader
        .load(reference)
        .map_err(|_| format!("asset_load:{}:{:?}", reference.id, loader.diagnostics()))?;
    handles.push(handle.clone());
    if handle.asset_id != reference.id || handle.asset_type != reference.asset_type {
        return Err(format!("asset_ref_mismatch:{}", reference.id));
    }
    let bytes = loader
        .get_asset_bytes(&handle)
        .ok_or("asset_bytes_missing")?;
    serde_json::from_slice(&bytes).map_err(|e| format!("asset_decode:{}:{e}", reference.id))
}
fn prepare_assets(
    loader: &mut RuntimeAssetLoader,
    source: &ParticleEffect,
    handles: &mut Vec<RuntimeAssetHandle>,
) -> Result<ParticlePreparedAssets, String> {
    let effect: CookedParticleEffect = read_asset(loader, &source.effect_ref, handles)?;
    effect
        .description
        .validate()
        .map_err(|e| format!("{e:?}"))?;
    for (name, value) in &source.parameters {
        validate_parameter(&effect.description, name, value)?;
    }
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut tints = Vec::new();
    for emitter in &effect.description.emitters {
        let material: Option<ParticleMaterialData> = emitter
            .draw
            .material
            .as_ref()
            .map(|r| read_asset(loader, r, handles))
            .transpose()?;
        let tint = material.as_ref().map_or([1.0; 4], |m| m.base_color);
        if let Some(material) = &material {
            material.validate()?;
        }
        if !tint.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0) {
            return Err("material.invalid_base_color".into());
        }
        let texture = emitter
            .draw
            .texture
            .as_ref()
            .or_else(|| material.as_ref().and_then(|m| m.texture.as_ref()));
        textures.push(
            texture
                .map(|r| loader.load_texture_payload(r))
                .transpose()?,
        );
        meshes.push(
            if let ParticleGeometry::Mesh { asset } = &emitter.draw.geometry {
                Some(read_asset(loader, asset, handles)?)
            } else {
                None
            },
        );
        tints.push(tint);
    }
    Ok(ParticlePreparedAssets {
        effect,
        textures,
        meshes,
        tints,
    })
}

#[cfg(test)]
#[path = "runtime_particles_tests.rs"]
mod tests;
