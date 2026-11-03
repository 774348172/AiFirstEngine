//! Immutable resource facts and instance values; wire remains the strict flat contract.
use super::*;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

/// Stable CPU facts. Mutable compatibility access detaches and invalidates derived facts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DrawResourceData {
    #[serde(skip)]
    pub(crate) resource_lease: Arc<()>,
    pub geometry: Arc<GeometryBuffers>,
    pub geometry_binding: Arc<RenderBindingSet>,
    pub material_binding: Arc<RenderBindingSet>,
    pub geometry_source: Arc<Provenance>,
    pub material_source: Arc<Provenance>,
    pub first_index: u32,
    pub index_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DrawResources {
    pub(super) data: DrawResourceData,
    #[serde(skip)]
    textures: OnceLock<[Option<TextureHandle>; 5]>,
    #[serde(skip)]
    geometry_handle: OnceLock<GeometryHandle>,
}
impl DrawResources {
    pub(super) fn new(data: DrawResourceData) -> Self {
        Self {
            data,
            textures: OnceLock::new(),
            geometry_handle: OnceLock::new(),
        }
    }
    pub(super) fn geometry_handle(&self) -> GeometryHandle {
        // Pure interpretation of this immutable binding, never current GPU
        // admission. Every mutable resource access below clears this fact.
        *self.geometry_handle.get_or_init(|| {
            match &self
                .data
                .geometry_binding
                .slots
                .iter()
                .find(|slot| {
                    #[cfg(test)]
                    GEOMETRY_SLOT_VISITS.with(|n| n.set(n.get() + 1));
                    slot.name == "geometry"
                })
                .unwrap()
                .resource
            {
                BindingResource::Geometry(handle) => *handle,
                _ => unreachable!("validated geometry binding"),
            }
        })
    }
    pub(super) fn textures(&self) -> [Option<TextureHandle>; 5] {
        *self.textures.get_or_init(|| {
            self.data
                .material_binding
                .texture_slots(STANDARD_LIT_TEXTURE_SLOTS)
        })
    }
}
impl PartialEq for DrawResources {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}
impl Eq for DrawResources {}
impl Hash for DrawResources {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

/// Per-instance values shared across this camera/origin's color and shadow views.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DrawInstance {
    pub(super) resources: Arc<DrawResources>,
    pub deformation: Option<Arc<crate::character_animation::SkinDeformation3D>>,
    pub decal_bias: Option<i32>,
    pub world_transform: [OrderedF32; 16],
    pub base_color: [OrderedF32; 4],
    pub alpha_cutoff: Option<OrderedF32>,
    pub standard_lit: Option<StandardLitParameters>,
    pub opaque_alpha: bool,
    pub ignore_vertex_color: bool,
    pub blend: MaterialBlend,
    pub cull: MaterialCull,
    pub instance_id: u64,
    pub provenance: Arc<Provenance>,
}
impl Deref for DrawInstance {
    type Target = DrawResourceData;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.resources.data
    }
}
impl DerefMut for DrawInstance {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let resources = Arc::make_mut(&mut self.resources);
        resources.textures.take();
        resources.geometry_handle.take();
        &mut resources.data
    }
}

/// View-local execution facts. Legacy field access uses safe copy on write.
#[derive(Debug, Clone)]
pub struct DrawGeometry {
    pub(crate) instance: Arc<DrawInstance>,
    pub(super) success: proof::BoundSuccess<DrawGeometry>,
    pub(super) command: Option<Arc<retained_commands::RetainedDrawCommand>>,
    // A complete immutable instance's static success is independent of shared
    // resource/pipeline interpretation. COW and detached instances must miss.
    pub(super) static_instance: Option<std::sync::Weak<DrawInstance>>,
    pub environment: Option<Arc<crate::environment_lighting::EnvironmentLighting>>,
    pub view: Arc<ExecutableRenderView>,
    pub target: Arc<str>,
    pub depth_target: Option<Arc<str>>,
    pub lighting: Option<Arc<ClusteredLights>>,
    pub shadows: Option<Arc<ShadowMaps>>,
    pub depth: MaterialDepth,
    pub camera_depth: OrderedF32,
}
impl Deref for DrawGeometry {
    type Target = DrawInstance;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}
impl DerefMut for DrawGeometry {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.command = None;
        self.static_instance = None;
        self.success = Default::default();
        Arc::make_mut(&mut self.instance)
    }
}
impl DrawGeometry {
    pub(super) fn static_instance_proven(&self) -> bool {
        self.static_instance
            .as_ref()
            .is_some_and(|source| source.as_ptr() == Arc::as_ptr(&self.instance))
    }
    #[doc(hidden)]
    #[inline]
    pub fn retained_requirements(&self, format: &str) -> Option<Arc<CommandRequirements>> {
        self.command.as_ref()?.requirements(self, format)
    }
    /// Pure binding interpretation only; never a residency or completion proof.
    #[doc(hidden)]
    #[inline]
    pub fn material_textures(&self) -> [Option<TextureHandle>; 5] {
        self.instance.resources.textures()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DrawGeometryWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deformation: Option<Arc<crate::character_animation::SkinDeformation3D>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decal_bias: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<Arc<crate::environment_lighting::EnvironmentLighting>>,
    #[serde(skip)]
    pub(crate) resource_lease: Arc<()>,
    pub view: Arc<ExecutableRenderView>,
    pub target: Arc<str>,
    pub depth_target: Option<Arc<str>>,
    pub geometry: Arc<GeometryBuffers>,
    pub geometry_binding: Arc<RenderBindingSet>,
    pub material_binding: Arc<RenderBindingSet>,
    pub geometry_source: Arc<Provenance>,
    pub material_source: Arc<Provenance>,
    pub first_index: u32,
    pub index_count: u32,
    pub world_transform: [OrderedF32; 16],
    pub base_color: [OrderedF32; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_cutoff: Option<OrderedF32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard_lit: Option<StandardLitParameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Arc<ClusteredLights>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<Arc<ShadowMaps>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub opaque_alpha: bool,
    /// Native UnlitFlat variant; default preserves vertex-color multiplication.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignore_vertex_color: bool,
    pub blend: MaterialBlend,
    pub cull: MaterialCull,
    pub depth: MaterialDepth,
    /// Actual camera-space distance; never Sprite's quantized sort-z.
    pub camera_depth: OrderedF32,
    pub instance_id: u64,
    pub provenance: Arc<Provenance>,
}

impl From<DrawGeometryWire> for DrawGeometry {
    fn from(w: DrawGeometryWire) -> Self {
        let resources = Arc::new(DrawResources::new(DrawResourceData {
            resource_lease: w.resource_lease,
            geometry: w.geometry,
            geometry_binding: w.geometry_binding,
            material_binding: w.material_binding,
            geometry_source: w.geometry_source,
            material_source: w.material_source,
            first_index: w.first_index,
            index_count: w.index_count,
        }));
        let instance = Arc::new(DrawInstance {
            resources,
            deformation: w.deformation,
            decal_bias: w.decal_bias,
            world_transform: w.world_transform,
            base_color: w.base_color,
            alpha_cutoff: w.alpha_cutoff,
            standard_lit: w.standard_lit,
            opaque_alpha: w.opaque_alpha,
            ignore_vertex_color: w.ignore_vertex_color,
            blend: w.blend,
            cull: w.cull,
            instance_id: w.instance_id,
            provenance: w.provenance,
        });
        Self {
            instance,
            success: Default::default(),
            command: None,
            static_instance: None,
            environment: w.environment,
            view: w.view,
            target: w.target,
            depth_target: w.depth_target,
            lighting: w.lighting,
            shadows: w.shadows,
            depth: w.depth,
            camera_depth: w.camera_depth,
        }
    }
}
// The private command is derived CPU state, excluded from wire/value identity.
impl PartialEq for DrawGeometry {
    fn eq(&self, other: &Self) -> bool {
        self.instance == other.instance
            && self.environment == other.environment
            && self.view == other.view
            && self.target == other.target
            && self.depth_target == other.depth_target
            && self.lighting == other.lighting
            && self.shadows == other.shadows
            && self.depth == other.depth
            && self.camera_depth == other.camera_depth
    }
}
impl Eq for DrawGeometry {}
impl Hash for DrawGeometry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.instance.hash(state);
        self.environment.hash(state);
        self.view.hash(state);
        self.target.hash(state);
        self.depth_target.hash(state);
        self.lighting.hash(state);
        self.shadows.hash(state);
        self.depth.hash(state);
        self.camera_depth.hash(state);
    }
}
impl From<&DrawGeometry> for DrawGeometryWire {
    fn from(draw: &DrawGeometry) -> Self {
        Self {
            deformation: draw.deformation.clone(),
            decal_bias: draw.decal_bias,
            environment: draw.environment.clone(),
            resource_lease: draw.resource_lease.clone(),
            view: draw.view.clone(),
            target: draw.target.clone(),
            depth_target: draw.depth_target.clone(),
            geometry: draw.geometry.clone(),
            geometry_binding: draw.geometry_binding.clone(),
            material_binding: draw.material_binding.clone(),
            geometry_source: draw.geometry_source.clone(),
            material_source: draw.material_source.clone(),
            first_index: draw.first_index,
            index_count: draw.index_count,
            world_transform: draw.world_transform,
            base_color: draw.base_color,
            alpha_cutoff: draw.alpha_cutoff,
            standard_lit: draw.standard_lit.clone(),
            lighting: draw.lighting.clone(),
            shadows: draw.shadows.clone(),
            opaque_alpha: draw.opaque_alpha,
            ignore_vertex_color: draw.ignore_vertex_color,
            blend: draw.blend,
            cull: draw.cull,
            depth: draw.depth,
            camera_depth: draw.camera_depth,
            instance_id: draw.instance_id,
            provenance: draw.provenance.clone(),
        }
    }
}
impl Serialize for DrawGeometry {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        DrawGeometryWire::from(self).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for DrawGeometry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DrawGeometryWire::deserialize(deserializer).map(Into::into)
    }
}

#[cfg(test)]
thread_local! { pub(super) static GEOMETRY_SLOT_VISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)}; }

#[cfg(test)]
mod tests {
    use super::*;
    fn draw() -> DrawGeometry {
        crate::rhi_command_plan::tests::retained_geometry_graph(1)
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| match c {
                crate::render_graph::RenderPassCommand::DrawGeometry { draw } => {
                    Some(draw.as_ref().clone())
                }
                _ => None,
            })
            .unwrap()
    }
    #[test]
    fn shared_geometry_handle_interpretation_visits_named_slot_once_at_multiple_scales() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let mut original = draw();
            // Exercise the same mutable resource boundary as a replacement,
            // without changing its value; the first read must interpret afresh.
            let _ = &mut original.geometry_binding;
            let expected = original
                .geometry_binding
                .slots
                .iter()
                .find_map(|s| match (&*s.name, &s.resource) {
                    ("geometry", BindingResource::Geometry(h)) => Some(*h),
                    _ => None,
                })
                .unwrap();
            GEOMETRY_SLOT_VISITS.with(|n| n.set(0));
            for _ in 0..count {
                assert_eq!(original.clone().geometry_handle(), expected);
            }
            actual.push((count, GEOMETRY_SLOT_VISITS.with(|n| n.get())));
        }
        eprintln!("actual named geometry slot visits: {actual:?}");
        assert!(
            actual.iter().all(|(_, visits)| *visits == 1),
            "shared resource binding reinterpreted: {actual:?}"
        );
    }
    #[test]
    fn interpreted_geometry_handle_keeps_generation_cow_wire_and_invalid_binding_boundaries() {
        use crate::render_resource::RenderResourceHandle;
        let original = draw();
        let before = serde_json::to_value(&original).unwrap();
        let hash = |v: &DrawGeometry| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            v.hash(&mut h);
            h.finish()
        };
        let before_hash = hash(&original);
        let first = original.geometry_handle();
        assert_eq!(serde_json::to_value(&original).unwrap(), before);
        assert_eq!(hash(&original), before_hash);
        let mut changed = original.clone();
        let next = GeometryHandle::try_from(RenderResourceHandle {
            generation: first.raw().generation + 1,
            ..first.raw()
        })
        .unwrap();
        let binding = Arc::make_mut(&mut changed.geometry_binding);
        binding
            .slots
            .iter_mut()
            .find(|s| s.name == "geometry")
            .unwrap()
            .resource = BindingResource::Geometry(next);
        assert_eq!(changed.geometry_handle(), next);
        assert_eq!(original.geometry_handle(), first);
        let decoded: DrawGeometry =
            serde_json::from_value(serde_json::to_value(&changed).unwrap()).unwrap();
        assert_eq!(decoded.geometry_handle(), next);
        assert_eq!(decoded, changed);
        let mut missing = original.clone();
        Arc::make_mut(&mut missing.geometry_binding)
            .slots
            .iter_mut()
            .find(|s| s.name == "geometry")
            .unwrap()
            .name = "renamed".into();
        assert!(missing.validate().is_err());
        assert!(std::panic::catch_unwind(|| missing.geometry_handle()).is_err());
        let mut wrong = original.clone();
        let texture = TextureHandle::try_from(RenderResourceHandle {
            kind: crate::render_resource::RenderResourceKind::Texture,
            ..first.raw()
        })
        .unwrap();
        Arc::make_mut(&mut wrong.geometry_binding)
            .slots
            .iter_mut()
            .find(|s| s.name == "geometry")
            .unwrap()
            .resource = BindingResource::Texture(texture);
        assert!(wrong.validate().is_err());
        assert!(std::panic::catch_unwind(|| wrong.geometry_handle()).is_err());
        assert_eq!(original.geometry_handle(), first);
    }
    #[test]
    fn legacy_flat_wire_is_strict_and_cow_preserves_old_packet_values() {
        let original = draw();
        let expected = serde_json::to_value(DrawGeometryWire::from(&original)).unwrap();
        let wire = serde_json::to_value(&original).unwrap();
        assert_eq!(wire, expected);
        assert!(wire.get("instance").is_none() && wire.get("resources").is_none());
        assert!(wire.get("worldTransform").is_some() && wire.get("geometryBinding").is_some());
        let decoded: DrawGeometry = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(decoded, original);
        let mut unknown = wire;
        unknown["instance"] = serde_json::json!({});
        assert!(serde_json::from_value::<DrawGeometry>(unknown).is_err());
        let mut changed = original.clone();
        assert!(Arc::ptr_eq(&original.instance, &changed.instance));
        changed.world_transform[12] = 3f32.into();
        assert!(!Arc::ptr_eq(&original.instance, &changed.instance));
        assert!(Arc::ptr_eq(
            &original.instance.resources,
            &changed.instance.resources
        ));
        assert_eq!(original.world_transform[12].to_f32(), 0.);
        Arc::make_mut(&mut changed.material_binding).binding_id = "reimported-material".into();
        assert!(!Arc::ptr_eq(
            &original.instance.resources,
            &changed.instance.resources
        ));
        assert_ne!(
            original.material_binding.binding_id,
            changed.material_binding.binding_id
        );
        assert_eq!(serde_json::to_value(&original).unwrap(), expected);
    }
    #[test]
    fn derived_texture_facts_invalidate_on_resource_mutation_and_do_not_affect_value_identity() {
        use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
        let mut original = draw();
        let texture = TextureHandle::try_from(RenderResourceHandle {
            kind: RenderResourceKind::Texture,
            index: 70,
            generation: 1,
        })
        .unwrap();
        Arc::make_mut(&mut original.material_binding)
            .slots
            .push(NamedRenderBinding {
                name: "baseColorTexture".into(),
                resource: BindingResource::Texture(texture),
            });
        let cold = original.clone();
        let hash = |draw: &DrawGeometry| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            draw.hash(&mut h);
            h.finish()
        };
        let before = hash(&original);
        assert_eq!(original.material_textures()[0], Some(texture));
        assert_eq!(original, cold);
        assert_eq!(hash(&original), before);
        let mut changed = original.clone();
        let next = TextureHandle::try_from(RenderResourceHandle {
            generation: 2,
            ..texture.raw()
        })
        .unwrap();
        Arc::make_mut(&mut changed.material_binding).slots[1].resource =
            BindingResource::Texture(next);
        assert_eq!(changed.material_textures()[0], Some(next));
        assert_eq!(original.material_textures()[0], Some(texture));
        Arc::make_mut(&mut changed.material_binding)
            .binding_id
            .clear();
        assert_eq!(changed.material_textures(), [None; 5]);
        assert!(changed.validate().is_err());
    }
    #[test]
    fn shared_cpu_backing_is_observed_once_per_allocation_with_both_roles() {
        let original = draw();
        let alias = original.clone();
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        original.observe_memory(&mut sample, "snapshot");
        alias.observe_memory(&mut sample, "retiring");
        let totals = crate::resource_memory::memory_totals(&sample.allocations).unwrap();
        for (address, bytes) in [
            (
                Arc::as_ptr(&original.instance) as usize,
                std::mem::size_of::<DrawInstance>(),
            ),
            (
                Arc::as_ptr(&original.instance.resources) as usize,
                std::mem::size_of::<DrawResources>(),
            ),
        ] {
            let id = format!("cpu-storage:{address:x}");
            let observed: Vec<_> = sample
                .allocations
                .iter()
                .filter(|a| a.allocation_id == id)
                .collect();
            assert_eq!(observed.len(), 2);
            assert!(observed.iter().all(|a| a.bytes == bytes as u64));
            assert!(observed.iter().any(|a| a.roles.contains("snapshot")));
            assert!(observed.iter().any(|a| a.roles.contains("retiring")));
        }
        assert!(totals.cpu_allocated > 0);
    }
}

impl DrawResourceData {
    #[doc(hidden)]
    pub fn native_resource_lease(&self) -> &Arc<()> {
        &self.resource_lease
    }
}
impl DrawGeometry {
    #[doc(hidden)]
    #[inline]
    pub fn native_instance(&self) -> &Arc<DrawInstance> {
        &self.instance
    }
}
