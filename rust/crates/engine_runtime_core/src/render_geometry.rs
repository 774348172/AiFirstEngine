//! Projection/Prepare -> neutral geometry commands. No ECS or project-source access.
#[cfg_attr(test, path = "render_geometry/batch.rs")]
mod batch;
#[cfg_attr(test, path = "render_geometry/build_inputs.rs")]
mod build_inputs;
#[cfg_attr(test, path = "render_geometry/commands.rs")]
mod commands;
#[cfg(test)]
pub(crate) use commands::tests::{batch_test_frame, batch_test_graph};
pub use commands::{GeometryBatch, GeometryVisibleInstance};
#[cfg_attr(test, path = "render_geometry/visible_instances.rs")]
mod visible_instances;
pub use visible_instances::GeometryVisibleInstances;
#[cfg_attr(test, path = "render_geometry/data.rs")]
pub(crate) mod data;
pub use data::{DrawGeometry, DrawInstance, DrawResourceData};
#[cfg_attr(test, path = "render_geometry/lighting.rs")]
mod lighting;
#[cfg_attr(test, path = "render_geometry/shadow.rs")]
mod shadow;
pub use lighting::{ClusteredLights, LIGHT_GRID, MAX_VIEW_LIGHTS};
#[doc(hidden)]
pub use shadow::DepthAttachmentValidation;
pub use shadow::{ShadowMaps, ShadowProjection};
#[cfg_attr(test, path = "render_geometry/buffers.rs")]
mod buffers;
#[cfg_attr(test, path = "render_geometry/generated.rs")]
pub(crate) mod generated;
#[cfg_attr(test, path = "render_geometry/graph.rs")]
mod graph;
#[cfg_attr(test, path = "render_geometry/memory.rs")]
mod memory;
#[cfg_attr(test, path = "render_geometry/mesh_residency.rs")]
mod mesh_residency;
#[cfg_attr(test, path = "render_geometry/observation.rs")]
mod observation;
#[cfg_attr(test, path = "render_geometry/pipeline.rs")]
mod pipeline;
#[cfg_attr(test, path = "render_geometry/prepare_tasks.rs")]
mod prepare_tasks;
#[cfg(feature = "real-wgpu")]
#[doc(hidden)]
pub use prepare_tasks::{join_validation, read_native_partitions};
#[cfg_attr(test, path = "render_geometry/prepared_material.rs")]
mod prepared_material;
#[cfg_attr(test, path = "render_geometry/production.rs")]
mod production;
#[cfg_attr(test, path = "render_geometry/proof.rs")]
mod proof;
pub(crate) use proof::ContentValidation;
#[cfg_attr(test, path = "render_geometry/retained_commands.rs")]
mod retained_commands;
#[doc(hidden)]
pub use retained_commands::CommandRequirements;

#[cfg(feature = "real-wgpu")]
#[doc(hidden)]
pub fn observe_success_proof_memory(sample: &mut crate::resource_memory::ResourceMemorySample) {
    proof::observe_memory(sample);
}
#[cfg_attr(test, path = "render_geometry/workset.rs")]
mod workset;
#[cfg(test)]
pub(crate) use buffers::package_streamable_for_test;
pub use buffers::GeometryBuffers;
pub(crate) use graph::{validate_geometry_graph, validate_geometry_graph_with_cache};
pub use observation::*;
pub(crate) use production::ProductionGeometryCache;
#[cfg(all(test, feature = "real-wgpu"))]
pub(crate) use production::{retained_command_test_graph, unique_table_test_graph};
pub use production::{ProductionGeometrySnapshot, RetainedGeometryView};

use crate::render_asset_production::{PreparedCookedRenderAsset, RenderAssetPreparationState};
use crate::render_contract::*;
use crate::render_graph::OrderedF32;
use crate::render_resource::RenderResourceManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// U1's unlit recipe maps to this executable shader owner for neutral geometry.
pub const UNLIT_GEOMETRY_SHADER_SOURCE: &str =
    "engine_runtime/src/wgpu_backend/real/geometry.rs#geometry_pipeline";

fn invalid(field: &'static str) -> ContractError {
    ContractError::new(ContractErrorCode::InvalidBinding, field)
        .at_stage(ProducerStage::AssetBinding)
}

/// Validated once at Prepare, shared by instances and frames. Not a second asset registry.
#[derive(Debug, Clone)]
pub struct PreparedGeometry {
    buffers: Arc<GeometryBuffers>,
    geometry_binding: Arc<RenderBindingSet>,
    material_binding: Arc<RenderBindingSet>,
    geometry_source: Arc<Provenance>,
    material_source: Arc<Provenance>,
    material: prepared_material::PreparedMaterial,
    bounds: GeometryBounds,
    submesh: SubmeshRange,
    convention: CoordinateConvention,
}

impl PreparedGeometry {
    #[cfg(feature = "real-wgpu")]
    #[doc(hidden)]
    pub fn upload_source(
        &self,
    ) -> (
        crate::render_resource::RenderResourceHandle,
        Arc<GeometryBuffers>,
    ) {
        let handle = self
            .geometry_binding
            .slots
            .iter()
            .find_map(|slot| match slot.resource {
                BindingResource::Geometry(handle) => Some(handle.raw()),
                _ => None,
            })
            .expect("validated geometry binding");
        (handle, self.buffers.clone())
    }
    pub(crate) fn resource_handles(
        &self,
    ) -> impl Iterator<Item = crate::render_resource::RenderResourceHandle> + '_ {
        self.geometry_binding
            .slots
            .iter()
            .chain(&self.material_binding.slots)
            .map(|slot| slot.resource.raw())
    }
    pub fn material_slot(&self) -> &str {
        &self.submesh.material_slot
    }

    pub fn from_assets(
        mesh: &PreparedCookedRenderAsset,
        material: &PreparedCookedRenderAsset,
        submesh: usize,
        manager: &RenderResourceManager,
    ) -> Result<Self, ContractError> {
        for asset in [mesh, material] {
            if asset.state != RenderAssetPreparationState::Prepared {
                return Err(ContractError::new(
                    ContractErrorCode::ResourceNotReady,
                    "preparedAsset",
                ));
            }
            asset.binding.validate_residency(manager)?;
        }
        let RenderAssetData::Mesh(geometry) = &mesh.asset.asset else {
            return Err(invalid("geometry"));
        };
        let RenderAssetData::Material(params) = &material.asset.asset else {
            return Err(invalid("material"));
        };
        if mesh.binding.binding_kind != RenderBindingKind::Mesh
            || material.binding.binding_kind != RenderBindingKind::Material
        {
            return Err(invalid("bindingKind"));
        }
        params.validate()?;
        for slot in STANDARD_LIT_TEXTURE_SLOTS {
            if params.textures.contains_key(slot) != material.binding.texture(slot).is_some() {
                return Err(invalid("material.textures"));
            }
        }
        let submesh = geometry
            .submeshes
            .get(submesh)
            .ok_or_else(|| invalid("submesh"))?
            .clone();
        if params.preset == MaterialPreset::StandardLit {
            geometry.validate_standard_lit_inputs(
                submesh.first_index..submesh.first_index + submesh.index_count,
                params.textures.contains_key("normalTexture"),
                !params.textures.is_empty(),
            )?;
        }
        let prepared_material = prepared_material::PreparedMaterial::from(params.clone());
        prepared_material.values()?;
        Ok(Self {
            buffers: Arc::new(GeometryBuffers::from_cooked(geometry)?),
            geometry_binding: Arc::new(mesh.binding.clone()),
            material_binding: Arc::new(material.binding.clone()),
            geometry_source: asset_provenance(mesh),
            material_source: asset_provenance(material),
            material: prepared_material,
            bounds: geometry.bounds.clone(),
            submesh,
            convention: geometry.coordinate_convention,
        })
    }

    /// Old string-only MeshBasic commands need actual prepared assets to migrate.
    pub fn from_legacy_mesh(
        command: &crate::render_graph::RenderPassCommand,
        mesh: &PreparedCookedRenderAsset,
        material: &PreparedCookedRenderAsset,
        manager: &RenderResourceManager,
    ) -> Result<Self, ContractError> {
        let crate::render_graph::RenderPassCommand::DrawMeshBasic {
            mesh_ref,
            material_ref,
            ..
        } = command
        else {
            return Err(invalid("legacy.command"));
        };
        if mesh_ref != &mesh.asset_guid || material_ref.as_deref() != Some(&material.asset_guid) {
            return Err(ContractError::new(
                ContractErrorCode::AssetIdentityConflict,
                "legacy.assetRef",
            ));
        }
        Self::from_assets(mesh, material, 0, manager)
    }
}

fn asset_provenance(asset: &PreparedCookedRenderAsset) -> Arc<Provenance> {
    Arc::new(Provenance {
        asset_id: Some(asset.asset_guid.clone()),
        content_digest: Some(asset.cooked_digest.clone()),
        source_path: asset.source_path.clone(),
        availability: SourceAvailability::SuppliedContext,
        ..Provenance::unspecified(ProducerStage::AssetBinding)
    })
}

pub struct GeometryInstance<'a> {
    pub instance_id: u64,
    pub source: &'a PreparedGeometry,
    /// Evaluated world matrix from Projection, not an authoring interface.
    pub world_transform: [f32; 16],
    pub visibility: Visibility,
    pub layers: LayerSelection,
    pub provenance: Provenance,
}

#[cfg(test)]
thread_local! {
    static BUILD_SOURCE_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DRAW_VIEW_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DRAW_BINDING_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub struct GeometryViewInput<'a> {
    pub view: &'a RenderView,
    pub pose: CameraPose,
}

// Scratch state for one immutable graph pass. Never survives a validation call
// or owns resources; per-instance matrix/range/color checks still run per draw.
#[derive(Default)]
#[doc(hidden)]
pub struct DrawViewValidation<'a> {
    view: Option<&'a ExecutableRenderView>,
    lighting: Option<(&'a ClusteredLights, &'a ExecutableRenderView)>,
    shadows: Option<(
        &'a ShadowMaps,
        &'a ExecutableRenderView,
        &'a ClusteredLights,
    )>,
    environment: Option<&'a crate::environment_lighting::EnvironmentLighting>,
    geometry_binding: Option<&'a RenderBindingSet>,
    material_binding: Option<&'a RenderBindingSet>,
    // Call-local successful identities, borrowed for the entire immutable pass.
    // Keep the last-binding fast path; material ordering need not be contiguous.
    bindings: std::collections::HashSet<*const RenderBindingSet>,
    projection: [f32; 16],
    valid: bool,
    rigid: bool,
    selection: commands::SelectionScratch,
    product_bound: Option<(u64, bool)>,
    #[cfg(test)]
    validations: usize,
    #[cfg(test)]
    binding_validations: usize,
}

impl<'a> DrawViewValidation<'a> {
    fn validate_binding(&mut self, binding: &'a RenderBindingSet) -> Result<(), ContractError> {
        // Borrowed inputs cannot change during this pass. Only a successful
        // validation admits pointer reuse; this never proves native residency.
        let previous = if binding.binding_kind == RenderBindingKind::Mesh {
            &mut self.geometry_binding
        } else {
            &mut self.material_binding
        };
        if previous.is_some_and(|old| std::ptr::eq(old, binding)) {
            return Ok(());
        }
        if self
            .bindings
            .contains(&(binding as *const RenderBindingSet))
        {
            *previous = Some(binding);
            return Ok(());
        }
        #[cfg(test)]
        {
            self.binding_validations += 1;
            DRAW_BINDING_VALIDATIONS.with(|n| n.set(n.get() + 1));
        }
        binding.validate()?;
        self.bindings.insert(binding as *const RenderBindingSet);
        *previous = Some(binding);
        Ok(())
    }

    fn projection(&mut self, view: &'a ExecutableRenderView) -> [f32; 16] {
        if !self.view.is_some_and(|old| std::ptr::eq(old, view)) {
            self.projection = view.view_projection();
            self.view = Some(view);
            self.valid = false;
            self.rigid = false;
            self.product_bound = None;
        }
        self.projection
    }

    fn validate_rigid(&mut self, view: &'a ExecutableRenderView) -> Result<(), ContractError> {
        self.projection(view);
        if self.rigid {
            return Ok(());
        }
        let v = view.view_matrix.map(OrderedF32::to_f32);
        if [v[3], v[7], v[11], v[15] - 1.]
            .iter()
            .any(|x| x.abs() > 1e-5)
            || (0..3).any(|a| {
                (0..3).any(|b| {
                    ((0..3).map(|i| v[a * 4 + i] * v[b * 4 + i]).sum::<f32>()
                        - if a == b { 1. } else { 0. })
                    .abs()
                        > 1e-4
                })
            })
        {
            return Err(invalid("drawGeometry.standardLitRigidView"));
        }
        self.rigid = true;
        Ok(())
    }

    fn validate(&mut self, view: &'a ExecutableRenderView) -> Result<(), ContractError> {
        self.projection(view);
        if self.valid {
            return Ok(());
        }
        #[cfg(test)]
        {
            self.validations += 1;
            DRAW_VIEW_VALIDATIONS.with(|n| n.set(n.get() + 1));
        }
        view.validate_extent(u32::MAX, u32::MAX)?;
        self.valid = true;
        Ok(())
    }
}

impl DrawGeometry {
    #[doc(hidden)]
    pub fn validate_shared_with_view<'a>(
        self: &'a Arc<Self>,
        validation: &mut DrawViewValidation<'a>,
    ) -> Result<(), ContractError> {
        proof::draw(self, validation)
    }

    /// Derived from the full affine matrix, including parent shear and reflections.
    /// No independently authored winding flag can disagree with the transform.
    pub fn mirrored(&self) -> bool {
        let m = self.world_transform.map(|v| f64::from(v.to_f32()));
        let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
            + m[8] * (m[1] * m[6] - m[5] * m[2]);
        det < 0.
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        self.validate_with_view(&mut DrawViewValidation::default())
    }

    pub(super) fn validate_with_view<'a>(
        &'a self,
        view_validation: &mut DrawViewValidation<'a>,
    ) -> Result<(), ContractError> {
        self.validate_with_static_facts(view_validation, false)
    }

    // Only the shared immutable proof owner supplies this flag. Value/wire
    // validation above always keeps the original strict static interpretation.
    fn validate_with_static_facts<'a>(
        &'a self,
        view_validation: &mut DrawViewValidation<'a>,
        static_proven: bool,
    ) -> Result<(), ContractError> {
        self.validate_with_selection_facts(view_validation, static_proven, false)
    }

    // Only CompactView's private publication path supplies selection_proven:
    // the complete selection already checked this exact instance/view product,
    // and finalization changes only illumination and retained command metadata.
    fn validate_with_selection_facts<'a>(
        &'a self,
        view_validation: &mut DrawViewValidation<'a>,
        static_proven: bool,
        selection_proven: bool,
    ) -> Result<(), ContractError> {
        if self.blend.is_premultiplied()
            && (self.standard_lit.is_some()
                || self.depth != MaterialDepth::Disabled
                || self.decal_bias.is_some()
                || self.view.color_pipeline.is_some())
        {
            return Err(invalid("drawGeometry.premultipliedPolicy"));
        }
        if self.decal_bias.is_some_and(|v| {
            !(-16..=-1).contains(&v)
                || self.standard_lit.is_some()
                || self.blend != MaterialBlend::AlphaBlend
                || self.depth != MaterialDepth::TestOnly
                || self.view.is_shadow_map()
        }) {
            return Err(invalid("drawGeometry.decalBias"));
        }
        if (!static_proven
            && ((self.blend == MaterialBlend::AlphaMask) != self.alpha_cutoff.is_some()
                || self.alpha_cutoff.is_some_and(|v| {
                    !v.to_f32().is_finite() || !(0.0..=1.0).contains(&v.to_f32())
                })))
            || (self.blend == MaterialBlend::AlphaMask && self.depth != MaterialDepth::TestAndWrite)
        {
            return Err(invalid("drawGeometry.alphaCutoff"));
        }
        if self.view.is_shadow_map()
            && (self.depth_target.as_ref() != Some(&self.target)
                || self.lighting.is_some()
                || self.shadows.is_some()
                || self.blend.is_transparent()
                || self.depth != MaterialDepth::TestAndWrite)
        {
            return Err(invalid("shadow.draw"));
        }
        if let Some(shadows) = self.shadows.as_deref() {
            let lights = self
                .lighting
                .as_deref()
                .ok_or_else(|| invalid("shadow.scene_lights_required"))?;
            if view_validation
                .shadows
                .is_none_or(|(old, view, old_lights)| {
                    !std::ptr::eq(old, shadows)
                        || !std::ptr::eq(view, self.view.as_ref())
                        || !std::ptr::eq(old_lights, lights)
                })
            {
                shadows.validate(&self.view, lights)?;
                // The proof depends on all three immutably borrowed inputs.
                // Same maps may be paired with a different receiver or list later.
                view_validation.shadows = Some((shadows, self.view.as_ref(), lights));
            }
        }
        if let Some(lights) = self.lighting.as_ref() {
            if self.standard_lit.is_none() {
                return Err(invalid("light.unlit"));
            }
            if view_validation.lighting.is_none_or(|(old, view)| {
                !std::ptr::eq(old, lights.as_ref()) || !std::ptr::eq(view, self.view.as_ref())
            }) {
                lights.validate_shared(&self.view)?;
                // The current receiver is a separate fact from immutable list
                // content. Reusing a list with another view must still reject.
                view_validation.lighting = Some((lights.as_ref(), self.view.as_ref()));
            }
        }
        if let Some(env) = self.environment.as_deref() {
            let config = self
                .view
                .color_pipeline
                .as_ref()
                .and_then(|p| p.ibl.as_ref())
                .ok_or_else(|| invalid("environment.camera_required"))?;
            if self.standard_lit.is_none()
                || self.view.is_shadow_map()
                || config.intensity != env.intensity
            {
                return Err(invalid("environment.draw"));
            }
            if view_validation
                .environment
                .is_none_or(|old| !std::ptr::eq(old, env))
            {
                env.map.validate_shared()?;
                view_validation.environment = Some(env);
            }
        }
        if let Some(lit) = &self.standard_lit {
            if !static_proven {
                lit.validate()?;
            }
            if self.view.domain != RenderDomain::World3d {
                return Err(invalid("drawGeometry.standardLitDomain"));
            }
            view_validation.validate_rigid(&self.view)?;

            if !static_proven
                && self
                    .base_color
                    .iter()
                    .any(|v| !(0.0..=1.0).contains(&v.to_f32()))
            {
                return Err(invalid("drawGeometry.baseColor"));
            }
            if !static_proven
                && self.texture_mask() != 0
                && !self
                    .geometry
                    .attributes()
                    .iter()
                    .any(|a| a.semantic == VertexSemantic::TexCoord0)
            {
                return Err(invalid("drawGeometry.uvRequired"));
            }
            if !static_proven
                && !self
                    .geometry
                    .attributes()
                    .iter()
                    .any(|a| a.semantic == VertexSemantic::Normal)
            {
                return Err(invalid("drawGeometry.normal"));
            }
        }
        if !static_proven && self.opaque_alpha && self.blend != MaterialBlend::Opaque {
            return Err(invalid("drawGeometry.opaqueAlpha"));
        }
        if !static_proven {
            view_validation.validate_binding(&self.geometry_binding)?;
            view_validation.validate_binding(&self.material_binding)?;
        }
        if (!static_proven
            && (self.geometry_binding.binding_kind != RenderBindingKind::Mesh
                || self.material_binding.binding_kind != RenderBindingKind::Material
                || self.index_count == 0
                || self.index_count % 3 != 0
                || self.first_index % 3 != 0
                || self
                    .first_index
                    .checked_add(self.index_count)
                    .is_none_or(|n| n > self.geometry.index_count())))
            || self.target.is_empty()
            || self.depth_target.is_some() != (self.view.depth_policy != DepthPolicy::Disabled)
            || (self.depth != MaterialDepth::Disabled && self.depth_target.is_none())
        {
            return Err(invalid("drawGeometry"));
        }
        if !static_proven {
            proof::instance(&self.instance)?;
        }
        if !selection_proven {
            #[cfg(test)]
            HEAD_MATRIX_PRODUCTS.with(|n| n.set(n.get() + 1));
            finite_draw(&multiply_matrix(
                view_validation.projection(&self.view),
                self.world_transform.map(OrderedF32::to_f32),
            ))?;
        }
        finite_draw(&[self.camera_depth.to_f32()])?;
        view_validation.validate(&self.view)
    }

    pub fn lit_binding_mask(&self) -> u8 {
        self.texture_mask() | (u8::from(self.environment.is_some()) << 5)
    }
    pub fn texture_mask(&self) -> u8 {
        self.material_textures()
            .into_iter()
            .enumerate()
            .fold(0, |mask, (i, texture)| {
                mask | (u8::from(texture.is_some()) << i)
            })
    }
    #[inline]
    pub fn geometry_handle(&self) -> GeometryHandle {
        self.instance.resources.geometry_handle()
    }

    #[doc(hidden)]
    pub fn is_opaque_shadow_depth_writer(&self) -> bool {
        self.view.is_shadow_map()
            && self.blend == MaterialBlend::Opaque
            && self.depth == MaterialDepth::TestAndWrite
    }

    /// CPU opaque phase key; uniform values and camera depth do not split a batch.
    pub fn batch_identity(&self) -> String {
        crate::canonical_digest::sha256_prefixed(
            &serde_json::to_vec(&(
                self.pipeline_identity("neutral"),
                self.geometry_handle(),
                (!self.is_opaque_shadow_depth_writer()).then_some(&self.material_binding),
                self.first_index,
                self.index_count,
                self.deformation.as_ref().map(|pose| Arc::as_ptr(pose) as usize),
            ))
            .expect("typed batch identity"),
        )
    }

    /// Complete execution state. Camera matrices/material values remain uniforms.
    pub fn pipeline_identity(&self, color_format: &str) -> String {
        self.pipeline_key(color_format).as_ref().to_owned()
    }

    // Backend consumers retain the same bounded memo's immutable digest. Public
    // identity strings keep their existing type; recipe changes version their bytes.
    #[doc(hidden)]
    pub fn pipeline_key(&self, color_format: &str) -> Arc<str> {
        if let Some(facts) = self.retained_requirements(color_format) {
            return facts.pipeline.clone();
        }
        self.pipeline_key_uncached(color_format)
    }

    fn pipeline_key_uncached(&self, color_format: &str) -> Arc<str> {
        let position_only = self.is_opaque_shadow_depth_writer();
        // The opaque depth recipe reads only group 0 and positions. Its native
        // layout must use the same normalization; mask/color remain unchanged.
        let texture_mask = if position_only {
            0
        } else {
            self.texture_mask()
        };
        let lit_binding_mask =
            texture_mask | (u8::from(!position_only && self.environment.is_some()) << 5);
        let standard_lit = !position_only && self.standard_lit.is_some();
        let scene_lights = !position_only && self.lighting.is_some();
        let color_pipeline = !position_only && self.view.color_pipeline.is_some();
        let shadow_mask = if position_only {
            0
        } else {
            self.shadows.as_ref().map_or(0, |v| v.texture_mask())
        };
        let decal_bias = if position_only { None } else { self.decal_bias };
        let flat = !position_only && self.ignore_vertex_color;
        let opaque = !position_only && self.opaque_alpha;
        let inputs = pipeline::PipelineInputs {
            shadow: self.view.is_shadow_map(),
            shadow_mask,
            scene_lights,
            color_pipeline,
            lit_mask: standard_lit.then_some(lit_binding_mask),
            stride: self.geometry.vertex_stride(),
            attributes: std::borrow::Cow::Borrowed(self.geometry.attributes()),
            index_format: self.geometry.index_format(),
            color_format: std::borrow::Cow::Borrowed(color_format),
            depth_target: self.depth_target.is_some(),
            blend: self.blend,
            cull: self.cull,
            mirrored: self.mirrored(),
            depth: self.depth,
            decal_bias,
            domain: self.view.domain,
            textured: texture_mask & 1 != 0,
            flat,
            opaque,
        };
        pipeline::identity(inputs, || {
            let original = crate::canonical_digest::sha256_prefixed(
                &serde_json::to_vec(&(
                    if position_only {
                        "draw-geometry.v11.position-only-shadow"
                    } else {
                        "draw-geometry.v10.layered-world-position"
                    },
                    (
                        standard_lit,
                        scene_lights,
                        color_pipeline,
                        self.view.is_shadow_map(),
                        shadow_mask,
                    ),
                    lit_binding_mask,
                    UNLIT_SHADER_SOURCE,
                    UNLIT_GEOMETRY_SHADER_SOURCE,
                    self.geometry.vertex_stride(),
                    self.geometry.attributes(),
                    self.geometry.index_format(),
                    color_format,
                    self.depth_target.as_ref().map(|_| "depth32Float"),
                    self.blend,
                    self.cull,
                    self.mirrored(),
                    self.depth,
                    self.view.domain,
                    texture_mask & 1 != 0,
                ))
                .expect("typed pipeline identity"),
            );
            let original = if let Some(bias) = decal_bias {
                crate::canonical_digest::sha256_prefixed(
                    format!("{original}|mesh-decal:{bias}").as_bytes(),
                )
            } else {
                original
            };
            let original = if flat {
                crate::canonical_digest::sha256_prefixed(
                    format!("{original}|unlit-flat").as_bytes(),
                )
            } else {
                original
            };
            if opaque {
                crate::canonical_digest::sha256_prefixed(
                    format!("{original}|opaque-alpha-one").as_bytes(),
                )
            } else {
                original
            }
        })
    }
}

#[cfg(test)]
thread_local! {static HEAD_MATRIX_PRODUCTS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCullingSummary {
    pub view_id: u64,
    pub candidates: usize,
    pub included: usize,
    pub hidden: usize,
    pub layer_excluded: usize,
    pub domain_excluded: usize,
    pub frustum_excluded: usize,
}

#[derive(Debug, Clone)]
pub struct GeometryViewFrame {
    /// Coordinate anchor of this view's already-rebased matrices.
    pub coordinate_origin: [f64; 3],
    /// Explicit independent output; None preserves the frame primary target.
    pub color_target: Option<crate::runtime_renderer::RenderTarget>,
    pub view: ExecutableRenderView,
    pub depth_target: Option<String>,
    pub draws: Vec<Arc<DrawGeometry>>,
    /// Production stores one descriptor per compatible ordered batch.
    /// None is the legacy flat-input adapter, never the production hot path.
    pub batches: Option<Vec<Arc<GeometryBatch>>>,
    pub summary: Option<ViewCullingSummary>,
    pub trace: Vec<ViewVisibility>,
}

#[derive(Debug, Clone)]
pub struct GeometryFrame {
    pub target: String,
    pub extent: [u32; 2],
    pub views: Vec<GeometryViewFrame>,
}

/// Compact failure keeps the failing pair identifiable; source mapping remains in
/// the caller's existing instance/view Provenance. No strings are allocated in Off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryBuildError {
    pub error: ContractError,
    pub view_id: Option<u64>,
    pub instance_id: Option<u64>,
}
impl From<ContractError> for GeometryBuildError {
    fn from(error: ContractError) -> Self {
        Self {
            error,
            view_id: None,
            instance_id: None,
        }
    }
}
impl std::fmt::Display for GeometryBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; view={:?}, instance={:?}",
            self.error, self.view_id, self.instance_id
        )
    }
}
impl std::error::Error for GeometryBuildError {}
fn build_error(
    error: ContractError,
    view_id: Option<u64>,
    instance_id: Option<u64>,
) -> GeometryBuildError {
    GeometryBuildError {
        error,
        view_id,
        instance_id,
    }
}

impl GeometryFrame {
    pub fn build(
        target: &str,
        extent: [u32; 2],
        views: &[GeometryViewInput<'_>],
        instances: &[GeometryInstance<'_>],
        manager: &RenderResourceManager,
        diagnostics: DiagnosticLevel,
    ) -> Result<Self, GeometryBuildError> {
        Self::build_retained(target, extent, views, instances, manager, diagnostics, None)
    }
    pub(crate) fn build_retained(
        target: &str,
        extent: [u32; 2],
        views: &[GeometryViewInput<'_>],
        instances: &[GeometryInstance<'_>],
        manager: &RenderResourceManager,
        diagnostics: DiagnosticLevel,
        native_lease: Option<&Arc<()>>,
    ) -> Result<Self, GeometryBuildError> {
        if target.trim().is_empty() {
            return Err(invalid("target").into());
        }
        build_inputs::GeometryBuildInputs::new(instances, manager, native_lease)?.build(
            target,
            extent,
            views,
            diagnostics,
        )
    }

    fn build_validated(
        target: &str,
        extent: [u32; 2],
        views: &[GeometryViewInput<'_>],
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        diagnostics: DiagnosticLevel,
    ) -> Result<Self, GeometryBuildError> {
        if target.trim().is_empty() {
            return Err(invalid("target").into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut output = Self {
            target: target.into(),
            extent,
            views: Vec::new(),
        };
        for input in views {
            if !ids.insert(input.view.view_id) {
                return Err(build_error(
                    invalid("viewId"),
                    Some(input.view.view_id),
                    None,
                ));
            }
            let pending = Self::select_view(target, extent, input, inputs, diagnostics)?;
            output
                .views
                .push(pending.finish(inputs, &FinalViewState::default())?);
        }
        Ok(output)
    }
}

/// Private same-frame selection. No provisional draw/batch Arc escapes to
/// consumers; lighting/shadow dependencies are installed before publication.
struct PendingGeometryView {
    frame: GeometryViewFrame,
    compact: Option<commands::CompactView>,
}
#[derive(Default)]
struct FinalViewState {
    environment: Option<Arc<crate::environment_lighting::EnvironmentLighting>>,
    lighting: Option<Arc<ClusteredLights>>,
    shadows: Option<Arc<ShadowMaps>>,
}
impl FinalViewState {
    fn apply(&self, draw: &mut DrawGeometry) {
        if draw.standard_lit.is_some() {
            draw.environment = self.environment.clone();
            draw.lighting = self.lighting.clone();
            draw.shadows = self.shadows.clone();
        }
    }
}
impl PendingGeometryView {
    fn finish(
        mut self,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        state: &FinalViewState,
    ) -> Result<GeometryViewFrame, GeometryBuildError> {
        if let Some(compact) = self.compact {
            let batches = prepare_tasks::finish_color(compact, inputs, state)
                .map_err(|e| build_error(e, Some(self.frame.view.view_id), None))?;
            commands::publish_color(&mut self.frame, batches, inputs.prepared.is_some());
        } else {
            for draw in &mut self.frame.draws {
                state.apply(Arc::make_mut(draw));
            }
        }
        Ok(self.frame)
    }
    fn receiver_ranges(
        &self,
        prepared: Option<&build_inputs::PreparedInputs>,
    ) -> Option<Vec<[f32; 2]>> {
        let compact = self.compact.as_ref()?;
        let prepared = prepared?;
        let receiver_view =
            workset::ReceiverView::new(self.frame.view.view_matrix.map(OrderedF32::to_f32));
        let mut ranges = Vec::new();
        for (index, instance) in compact.receiver_rows() {
            if instance.standard_lit.is_some() {
                ranges.push(
                    prepared
                        .receiver_bounds(index as usize, instance)?
                        .depth_range_in(&receiver_view)?,
                );
            }
        }
        Some(ranges)
    }
}
impl GeometryFrame {
    fn select_view(
        target: &str,
        extent: [u32; 2],
        input: &GeometryViewInput<'_>,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        diagnostics: DiagnosticLevel,
    ) -> Result<PendingGeometryView, GeometryBuildError> {
        let count = inputs.instances.len();
        Self::select_view_range(target, extent, input, inputs, diagnostics, 0..count)
    }
    fn select_view_range(
        target: &str,
        extent: [u32; 2],
        input: &GeometryViewInput<'_>,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        diagnostics: DiagnosticLevel,
        range: std::ops::Range<usize>,
    ) -> Result<PendingGeometryView, GeometryBuildError> {
        if target.trim().is_empty() {
            return Err(invalid("target").into());
        }
        let instances = inputs.instances;
        let mut view = input
            .view
            .evaluate(input.pose)
            .map_err(|e| build_error(e, Some(input.view.view_id), None))?;
        view.target_extent = extent;
        view.validate_extent(extent[0], extent[1])
            .map_err(|e| build_error(e, Some(view.view_id), None))?;
        let depth_target = (view.depth_policy != DepthPolicy::Disabled).then(|| {
            if view.is_shadow_map() {
                target.to_owned()
            } else {
                format!("{target}:depth:view-{}", view.view_id)
            }
        });
        let view_projection = view.view_projection();
        let spatial_view = inputs.spatial_view(view_projection);
        let draw_view = Arc::new(view.clone());
        let mut frame = GeometryViewFrame {
            coordinate_origin: [0.; 3],
            color_target: None,
            view: view.clone(),
            depth_target,
            draws: Vec::new(),
            batches: None,
            summary: None,
            trace: Vec::new(),
        };
        let mut compact = inputs
            .compact_table
            .clone()
            .map(|table| commands::CompactView::new(table).with_prepared(inputs.prepared));
        let draw_target: Arc<str> = if view.color_pipeline.is_some() {
            format!("color3d-{}-hdr", view.view_id).into()
        } else {
            target.into()
        };
        let draw_depth: Option<Arc<str>> = frame.depth_target.as_deref().map(Into::into);
        let mut summary = ViewCullingSummary {
            view_id: view.view_id,
            ..Default::default()
        };
        let mut layers = inputs.layer_matcher(&input.view.layers);
        for instance_index in range {
            let instance = inputs.candidate(instance_index);
            summary.candidates += 1;
            let inherited = match instance.visibility.inherited {
                Evaluation::Known(v) => v,
                Evaluation::NotEvaluated if !instance.visibility.authored => false,
                Evaluation::NotEvaluated => {
                    return Err(build_error(
                        invalid("visibility.inherited"),
                        Some(view.view_id),
                        Some(instance.instance_id),
                    ));
                }
            };
            // Blended surfaces receive shadows but do not cast opaque silhouettes.
            if view.is_shadow_map() && instance.blend.is_transparent() {
                continue;
            }
            let (visible, decision) = if !instance.visibility.authored || !inherited {
                summary.hidden += 1;
                (false, CullingDecision::NotEvaluated)
            } else if instance.convention != input.view.coordinate_convention {
                summary.domain_excluded += 1;
                (false, CullingDecision::DomainExcluded)
            } else if !layers
                .matches(instance_index, &instance)
                .map_err(|e| build_error(e, Some(view.view_id), Some(instance.instance_id)))?
            {
                summary.layer_excluded += 1;
                (false, CullingDecision::LayerExcluded)
            } else if inputs
                .outside_frustum(instance_index, &spatial_view, &instance)
                .map_err(|e| build_error(e, Some(view.view_id), Some(instance.instance_id)))?
            {
                summary.frustum_excluded += 1;
                (false, CullingDecision::FrustumExcluded)
            } else {
                summary.included += 1;
                (true, CullingDecision::Included)
            };
            if diagnostics == DiagnosticLevel::Trace {
                frame.trace.push(ViewVisibility {
                    view_id: view.view_id,
                    instance_id: instance.instance_id,
                    visibility: Evaluation::Known(visible),
                    culling: decision,
                });
            }
            if !visible {
                continue;
            }
            let center = instance.bounds.sphere_center;
            let camera_depth = selection_camera_depth(
                view.view_matrix.map(OrderedF32::to_f32),
                instance.world_transform,
                [center[0], center[1], center[2], 1.],
            );
            let depth = if view.is_shadow_map() {
                MaterialDepth::TestAndWrite
            } else {
                instance.depth
            };
            let camera_depth = OrderedF32::from(camera_depth);
            let make_draw = |data| DrawGeometry {
                instance: data,
                success: Default::default(),
                command: None,
                static_instance: None,
                view: Arc::clone(&draw_view),
                target: draw_target.clone(),
                depth_target: draw_depth.clone(),
                environment: None,
                lighting: None,
                shadows: None,
                depth: if view.is_shadow_map() {
                    MaterialDepth::TestAndWrite
                } else {
                    instance.depth
                },
                camera_depth,
            };
            if let Some(compact) = &mut compact {
                // enable_compact freezes rows in this exact input order.
                // Visibility can select a sparse subset, never reorder the
                // table. Clone only the row needed for a final head.
                let table = inputs.compact_table.as_ref().expect("compact table");
                compact
                    .push(instance_index as u32, depth, camera_depth, || {
                        make_draw(table[instance_index].clone())
                    })
                    .map_err(|e| build_error(e, Some(view.view_id), Some(instance.instance_id)))?;
            } else {
                let legacy = instances.get(instance_index).expect("legacy row index");
                let data = inputs
                    .instance_data(&legacy)
                    .map_err(|e| build_error(e, Some(view.view_id), Some(instance.instance_id)))?;
                frame.draws.push(Arc::new(make_draw(data)));
            }
        }
        // Borrow the completed immutable draw set so shared pass inputs can
        // use the same validator as graph/native preparation. No partial
        // frame escapes, and all instance-specific checks still run.
        let mut validation = DrawViewValidation::default();
        for draw in &frame.draws {
            draw.validate_shared_with_view(&mut validation)
                .map_err(|e| build_error(e, Some(view.view_id), Some(draw.instance_id)))?;
        }
        frame.draws.sort_by(|a, b| {
            let alpha_a = a.blend.is_transparent();
            let alpha_b = b.blend.is_transparent();
            alpha_a
                .cmp(&alpha_b)
                .then_with(|| {
                    let order = a.camera_depth.to_f32().total_cmp(&b.camera_depth.to_f32());
                    if alpha_a {
                        order.reverse()
                    } else {
                        order
                    }
                })
                .then(a.instance_id.cmp(&b.instance_id))
        });
        // PBR opaque/mask depth writers are binned phases. Legacy Unlit mask
        // and non-writing depth policies retain their original order barriers.
        let sortable = |d: &DrawGeometry| {
            d.depth == MaterialDepth::TestAndWrite
                && (d.blend == MaterialBlend::Opaque
                    || ((d.view.is_shadow_map() || d.standard_lit.is_some())
                        && d.blend == MaterialBlend::AlphaMask))
        };
        let mut start = 0;
        while start < frame.draws.len() {
            if !sortable(&frame.draws[start]) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < frame.draws.len() && sortable(&frame.draws[end]) {
                end += 1;
            }
            inputs.sort_opaque_segment(&mut frame.draws[start..end]);
            start = end;
        }
        if diagnostics != DiagnosticLevel::Off {
            frame.summary = Some(summary);
        }

        Ok(PendingGeometryView { frame, compact })
    }
}

#[cfg(test)]
thread_local! {static LAYER_MATCH_EVALUATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };}

fn layers_match(view: &LayerSelection, instance: &LayerSelection) -> Result<bool, ContractError> {
    #[cfg(test)]
    LAYER_MATCH_EVALUATIONS.with(|n| n.set(n.get() + 1));
    match (view, instance) {
        (LayerSelection::Named { names: a }, LayerSelection::Named { names: b }) => {
            Ok(a.is_empty() || a.iter().any(|n| b.contains(n)))
        }
        (LayerSelection::Legacy { mask: a }, LayerSelection::Legacy { mask: b }) => {
            Ok(a.is_none() || a == b)
        }
        _ => Err(invalid("layers.representation")),
    }
}

fn finite_draw(values: &[f32]) -> Result<(), ContractError> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(ContractError::new(
            ContractErrorCode::NonFinite,
            "drawGeometry",
        ))
    }
}

fn validate_world(m: [f32; 16]) -> Result<(), ContractError> {
    finite_draw(&m)?;
    if m[3] != 0. || m[7] != 0. || m[11] != 0. || m[15] != 1. {
        return Err(invalid("worldTransform.affine"));
    }
    Ok(())
}

#[cfg(test)]
mod view_validation_tests {
    use super::*;

    #[test]
    fn build_shared_prepared_source_validates_once_without_hiding_instance_or_lease_changes() {
        use crate::render_resource::*;
        let mut manager = RenderResourceManager::new();
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let template = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = c {
                    Some(draw)
                } else {
                    None
                }
            })
            .unwrap();
        let mut binding = |id: &str, kind, binding_kind, source| {
            let handle = manager.request_resource(
                0,
                RenderResourceRequest {
                    key: RenderAssetKey::new(id, "1", kind),
                    source,
                    byte_len: 64,
                    reason: "build-source-test".into(),
                },
            );
            Arc::new(
                RenderBindingSet::single(
                    id.into(),
                    binding_kind,
                    handle,
                    "nearestClamp".into(),
                    false,
                    "source-test".into(),
                )
                .unwrap(),
            )
        };
        let geometry_binding = binding(
            "mesh",
            RenderResourceKind::MeshBuffer,
            RenderBindingKind::Mesh,
            RenderResourceSource::MeshBufferDescriptor {
                vertex_count: 3,
                index_count: 3,
            },
        );
        let material_binding = binding(
            "material",
            RenderResourceKind::MaterialParams,
            RenderBindingKind::Material,
            RenderResourceSource::MaterialParamsDescriptor { param_count: 1 },
        );
        let source = PreparedGeometry {
            buffers: template.geometry.clone(),
            geometry_binding,
            material_binding,
            geometry_source: template.geometry_source.clone(),
            material_source: template.material_source.clone(),
            material: CookedMaterial {
                decal_bias: None,
                base_color_texture_sampling: None,
                opaque_alpha: false,
                ignore_vertex_color: false,
                preset: MaterialPreset::Unlit,
                texture_sampling: Default::default(),
                parameters: [("baseColor".into(), MaterialParameter::Vector4([1.; 4]))].into(),
                textures: Default::default(),
                blend: MaterialBlend::Opaque,
                cull: MaterialCull::None,
                depth: MaterialDepth::Disabled,
                shader_source: UNLIT_SHADER_SOURCE.into(),
            }
            .into(),
            bounds: GeometryBounds {
                min: [-0.5; 3],
                max: [0.5; 3],
                sphere_center: [0.; 3],
                sphere_radius: 1.,
            },
            submesh: SubmeshRange {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
            },
            convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        };
        let lease = manager.lease_resources(source.resource_handles());
        let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        view.depth_policy = DepthPolicy::Disabled;
        let views = [GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }];
        let mut instances: Vec<_> = (0..128)
            .map(|instance_id| {
                let mut matrix = IDENTITY_MATRIX;
                matrix[14] = -2.;
                GeometryInstance {
                    instance_id,
                    source: &source,
                    world_transform: matrix,
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(true),
                    },
                    layers: LayerSelection::Named {
                        names: vec!["main".into()],
                    },
                    provenance: template.provenance.as_ref().clone(),
                }
            })
            .collect();
        BUILD_SOURCE_VALIDATIONS.with(|n| n.set(0));
        DRAW_VIEW_VALIDATIONS.with(|n| n.set(0));
        let frame = GeometryFrame::build_retained(
            "main",
            [64, 64],
            &views,
            &instances,
            &manager,
            DiagnosticLevel::Off,
            Some(&lease),
        )
        .unwrap();
        assert_eq!(frame.views[0].draws.len(), 128);
        assert_eq!(
            DRAW_VIEW_VALIDATIONS.with(|n| n.get()),
            1,
            "build revalidates the same immutable draw view per instance"
        );
        assert_eq!(
            BUILD_SOURCE_VALIDATIONS.with(|n| n.get()),
            1,
            "same Prepare resource revalidated per instance"
        );
        BUILD_SOURCE_VALIDATIONS.with(|n| n.set(0));
        let mut shared =
            build_inputs::GeometryBuildInputs::new(&instances, &manager, Some(&lease)).unwrap();
        let geometry_holders = Arc::strong_count(&source.buffers);
        let mut retained_views = Vec::new();
        for index in 0..13 {
            let result = shared
                .build(
                    &format!("view-target-{index}"),
                    [64, 64],
                    &views,
                    DiagnosticLevel::Off,
                )
                .unwrap();
            assert_eq!(result.views[0].draws.len(), 128);
            retained_views.push(result);
        }
        assert!(
            Arc::strong_count(&source.buffers) <= geometry_holders + 1,
            "128 instances x 13 views duplicate source holders: {} added",
            Arc::strong_count(&source.buffers) - geometry_holders
        );
        for view in &retained_views[1..] {
            for (first, next) in retained_views[0].views[0]
                .draws
                .iter()
                .zip(&view.views[0].draws)
            {
                assert_eq!(first.instance_id, next.instance_id);
                assert!(
                    Arc::ptr_eq(&first.instance, &next.instance),
                    "instance backing copied per view"
                );
            }
        }
        let first = retained_views[0].views[0].draws[0].clone();
        let mut next = first.as_ref().clone();
        next.world_transform[12] = 7f32.into();
        assert_eq!(first.world_transform[12].to_f32(), 0.);
        assert_eq!(
            retained_views[12].views[0].draws[0].world_transform[12].to_f32(),
            0.
        );
        println!(
            "shared_sources=1 instances=128 views=13 view_draw_bytes={} instance_bytes={}",
            std::mem::size_of::<DrawGeometry>(),
            std::mem::size_of::<DrawInstance>()
        );
        drop(shared);
        assert_eq!(
            BUILD_SOURCE_VALIDATIONS.with(|n| n.get()),
            1,
            "same immutable input batch revalidated for each color/shadow build"
        );
        instances[127].world_transform[15] = 0.;
        assert!(
            GeometryFrame::build_retained(
                "main",
                [64, 64],
                &views,
                &instances,
                &manager,
                DiagnosticLevel::Off,
                Some(&lease)
            )
            .is_err(),
            "per-instance matrix check skipped"
        );
        instances[127].world_transform[15] = 1.;
        let mut invalid = source.clone();
        Arc::make_mut(&mut invalid.geometry_binding)
            .slots
            .push(source.geometry_binding.slots[0].clone());
        instances[127].source = &invalid;
        assert!(
            GeometryFrame::build_retained(
                "main",
                [64, 64],
                &views,
                &instances,
                &manager,
                DiagnosticLevel::Off,
                Some(&lease)
            )
            .is_err(),
            "late changed binding skipped"
        );
        instances[127].source = &source;
        let mut invalid_range = source.clone();
        invalid_range.submesh.index_count = 6;
        instances[127].source = &invalid_range;
        assert!(
            GeometryFrame::build_retained(
                "main",
                [64, 64],
                &views,
                &instances,
                &manager,
                DiagnosticLevel::Off,
                Some(&lease)
            )
            .is_err(),
            "shared view validation hid a late invalid draw range"
        );
        instances[127].source = &source;
        manager.mark_device_lost(1);
        assert!(
            GeometryFrame::build_retained(
                "main",
                [64, 64],
                &views,
                &instances,
                &manager,
                DiagnosticLevel::Off,
                Some(&lease)
            )
            .is_err(),
            "proof crossed calls and hid device loss"
        );
    }

    #[test]
    fn alternating_pass_bindings_validate_once_and_late_errors_remain_visible() {
        use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
        let bindings: Vec<_> = (0..4)
            .map(|index| {
                RenderBindingSet::single(
                    format!("material-{index}"),
                    RenderBindingKind::Material,
                    RenderResourceHandle {
                        kind: RenderResourceKind::MaterialParams,
                        index,
                        generation: 1,
                    },
                    "nearest".into(),
                    false,
                    "owner".into(),
                )
                .unwrap()
            })
            .collect();
        let mut context = DrawViewValidation::default();
        for index in 0..4096 {
            context.validate_binding(&bindings[index % 4]).unwrap();
        }
        assert_eq!(
            context.binding_validations, 4,
            "alternating immutable bindings revalidated per draw"
        );
        let mut invalid = bindings[0].clone();
        invalid.slots.push(invalid.slots[0].clone());
        for _ in 0..2 {
            assert!(context.validate_binding(&invalid).is_err());
        }
        assert_eq!(
            context.binding_validations, 6,
            "invalid binding cached as success"
        );
        context.validate_binding(&bindings[0]).unwrap();
        assert_eq!(context.binding_validations, 6);
        let mut fresh = DrawViewValidation::default();
        fresh.validate_binding(&bindings[0]).unwrap();
        assert_eq!(
            fresh.binding_validations, 1,
            "proof escaped its immutable call"
        );
    }

    #[test]
    fn shared_pass_bindings_validate_once_and_invalid_bindings_never_reuse() {
        use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
        let binding = RenderBindingSet::single(
            "mesh".into(),
            RenderBindingKind::Mesh,
            RenderResourceHandle {
                kind: RenderResourceKind::MeshBuffer,
                index: 1,
                generation: 1,
            },
            "nearest".into(),
            false,
            "owner".into(),
        )
        .unwrap();
        let mut context = DrawViewValidation::default();
        for _ in 0..128 {
            context.validate_binding(&binding).unwrap();
        }
        assert_eq!(
            context.binding_validations, 1,
            "immutable binding revalidated per draw"
        );
        let mut changed = binding.clone();
        changed.binding_id.clear();
        for _ in 0..2 {
            assert!(context.validate_binding(&changed).is_err());
        }
        assert_eq!(
            context.binding_validations, 3,
            "invalid binding admitted to success reuse"
        );
        context.validate_binding(&binding).unwrap();
        assert_eq!(
            context.binding_validations, 3,
            "failed binding displaced a proven input"
        );
        let mut distinct = binding.clone();
        distinct.slots.push(distinct.slots[0].clone());
        assert!(
            context.validate_binding(&distinct).is_err(),
            "duplicate slots bypassed validation"
        );
        assert_eq!(context.binding_validations, 4);
        let material = RenderBindingSet::single(
            "material".into(),
            RenderBindingKind::Material,
            RenderResourceHandle {
                kind: RenderResourceKind::MaterialParams,
                index: 2,
                generation: 1,
            },
            "nearest".into(),
            false,
            "owner".into(),
        )
        .unwrap();
        let mut paired = DrawViewValidation::default();
        for _ in 0..128 {
            paired.validate_binding(&binding).unwrap();
            paired.validate_binding(&material).unwrap();
        }
        assert_eq!(
            paired.binding_validations, 2,
            "mesh/material pair revalidated per draw"
        );
        let mut distinct_valid = binding.clone();
        distinct_valid.binding_id = "other-mesh".into();
        paired.validate_binding(&distinct_valid).unwrap();
        assert_eq!(paired.binding_validations, 3, "changed binding skipped");
        paired.validate_binding(&binding).unwrap();
        assert_eq!(
            paired.binding_validations, 3,
            "successful immutable binding was revalidated after an intervening binding"
        );
        let mut fresh = DrawViewValidation::default();
        fresh.validate_binding(&binding).unwrap();
        assert_eq!(fresh.binding_validations, 1, "proof leaked across passes");
    }

    #[test]
    fn rigid_view_fact_is_local_and_changed_same_id_view_cannot_reuse_it() {
        let original = RenderView::perspective(1, 64, 64, 60., 0.1, 100.)
            .evaluate(CameraPose::default())
            .unwrap();
        let mut validation = DrawViewValidation::default();
        for _ in 0..128 {
            validation.validate_rigid(&original).unwrap();
        }
        let mut changed = original.clone();
        changed.view_matrix[0] = 2f32.into();
        assert!(validation.validate_rigid(&changed).is_err());
        assert!(validation.validate_rigid(&changed).is_err());
        validation.validate_rigid(&original).unwrap();
        assert!(DrawViewValidation::default()
            .validate_rigid(&changed)
            .is_err());
    }

    #[test]
    fn unchanged_pass_view_is_validated_once_and_changed_views_are_checked() {
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.)
            .evaluate(CameraPose::default())
            .unwrap();
        let mut validation = DrawViewValidation::default();
        for _ in 0..128 {
            validation.validate(&view).unwrap();
        }
        assert_eq!(
            validation.validations, 1,
            "immutable pass view revalidated per instance"
        );
        let mut changed = view.clone();
        changed.viewport[2] = 0;
        assert!(validation.validate(&changed).is_err());
        assert!(validation.validate(&changed).is_err());
        assert_eq!(
            validation.validations, 3,
            "invalid views must not be admitted into successful reuse"
        );
        validation.validate(&view).unwrap();
        assert_eq!(validation.validations, 4);
    }
}

fn outside_frustum(
    vp: [f32; 16],
    world: [f32; 16],
    bounds: &GeometryBounds,
) -> Result<bool, ContractError> {
    let m = multiply_matrix(vp, world);
    finite_draw(&m)?;
    let mut corners = [[0.; 4]; 8];
    for (i, p) in corners.iter_mut().enumerate() {
        *p = transform_point(
            m,
            [
                if i & 1 == 0 {
                    bounds.min[0]
                } else {
                    bounds.max[0]
                },
                if i & 2 == 0 {
                    bounds.min[1]
                } else {
                    bounds.max[1]
                },
                if i & 4 == 0 {
                    bounds.min[2]
                } else {
                    bounds.max[2]
                },
                1.,
            ],
        );
        finite_draw(p)?;
    }
    // Conservative homogeneous plane rejection; never divide by w near the eye.
    Ok((0..6).any(|plane| {
        corners.iter().all(|p| {
            let distance = match plane {
                0 => p[0] + p[3],
                1 => p[3] - p[0],
                2 => p[1] + p[3],
                3 => p[3] - p[1],
                4 => p[2],
                _ => p[3] - p[2],
            };
            distance < -1e-5 * p[3].abs().max(1.)
        })
    }))
}

#[cfg(test)]
thread_local! {static SELECTION_DEPTH_COMPONENTS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

fn selection_camera_depth(camera: [f32; 16], world: [f32; 16], point: [f32; 4]) -> f32 {
    // Only z is consumed. Preserve the exact original (camera * world) *
    // point f32 sum order; camera * (world * point) would change depth ties.
    let row: [f32; 4] = std::array::from_fn(|col| {
        #[cfg(test)]
        SELECTION_DEPTH_COMPONENTS.with(|n| n.set(n.get() + 1));
        (0..4).map(|k| camera[2 + k * 4] * world[col * 4 + k]).sum()
    });
    #[cfg(test)]
    SELECTION_DEPTH_COMPONENTS.with(|n| n.set(n.get() + 1));
    -(0..4).map(|col| row[col] * point[col]).sum::<f32>()
}

#[cfg(test)]
mod residual_depth_tests {
    use super::*;
    #[test]
    fn residual_selection_evaluates_only_depth_components_at_multiple_scales() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            SELECTION_DEPTH_COMPONENTS.with(|n| n.set(0));
            let f = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let selected = f
                .views
                .iter()
                .map(|v| v.selected_instances().count())
                .sum::<usize>();
            actual.push((
                count,
                selected,
                SELECTION_DEPTH_COMPONENTS.with(|n| n.get()),
            ));
        }
        eprintln!("selection depth scalar components {actual:?}");
        assert!(
            actual
                .iter()
                .all(|(_, selected, components)| *selected > 0 && *components <= selected * 5),
            "actual scalar components {actual:?}"
        );
    }
    #[test]
    fn residual_depth_keeps_original_f32_order_for_cancellation_shear_and_extremes() {
        let mut seed = 0x38275a3fu32;
        for i in 0..4096 {
            let mut value = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed as i32 as f32) / 1048576.
            };
            let mut camera = std::array::from_fn(|_| value());
            let mut world = std::array::from_fn(|_| value());
            let point = std::array::from_fn(|_| value());
            if i % 11 == 0 {
                world[0] = f32::MAX;
                camera[2] = -f32::MAX;
            }
            if i % 7 == 0 {
                camera[6] = -camera[2];
                world[4] = world[0];
            }
            let expected = -transform_point(multiply_matrix(camera, world), point)[2];
            let actual = selection_camera_depth(camera, world, point);
            assert!(
                actual.to_bits() == expected.to_bits() || actual.is_nan() && expected.is_nan(),
                "order {i}: {actual:?} {expected:?}"
            );
        }
        for zero in [0., -0.] {
            let camera = [zero; 16];
            assert_eq!(
                selection_camera_depth(camera, IDENTITY_MATRIX, [1.; 4]).to_bits(),
                (-transform_point(multiply_matrix(camera, IDENTITY_MATRIX), [1.; 4])[2]).to_bits()
            );
        }
    }
}
