//! Production-frame composition of existing Projection and Prepare products.
use super::{GeometryFrame, GeometryViewInput};
use crate::render_asset_production::{
    PreparedMeshRenderer, PreparedTextureAsset, RuntimeRenderAssetProducer,
};
use crate::render_contract::{ContractError, ContractErrorCode};
use crate::render_projection_adapter::MeshProjection;
use crate::render_resource::{RenderResourceManager, RenderResourceReleaseRequest};
use crate::render_state::{RenderProxyPayload, RenderSceneState};
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::runtime_renderer::RenderTarget;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

#[cfg(feature = "spine")]
#[path = "production/spine.rs"]
mod spine;
#[path = "production/vegetation.rs"]
mod vegetation;
#[path = "production/generated.rs"]
mod generated;

type PreparedProjection = (Arc<MeshProjection>, Arc<PreparedMeshRenderer>);
type BuiltGeometry = (RenderTarget, Arc<GeometryFrame>);

#[cfg(all(test, feature = "real-wgpu"))]
pub(crate) fn retained_command_test_graph() -> crate::render_graph::RenderGraph {
    unique_table_test_graph(1)
}
#[cfg(all(test, feature = "real-wgpu"))]
pub(crate) fn unique_table_test_graph(count: usize) -> crate::render_graph::RenderGraph {
    let (manager, mut snapshot) =
        output_binding_tests::mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
    let base = snapshot.instances[0].clone();
    snapshot.instances = Arc::new(
        (0..count)
            .map(|i| {
                let mut row = base.as_ref().clone();
                row.instance_id = i as u64 + 1;
                Arc::new(row)
            })
            .collect(),
    );
    let frame = snapshot
        .build_uncached(
            &RenderTarget::headless_texture("surface-main", 64, 64),
            &manager,
        )
        .unwrap();
    super::commands::tests::batch_test_graph(&frame)
}

/// No World, loader, mutable asset table or backend object crosses the worker boundary.
#[derive(Debug, Clone, Default)]
pub struct ProductionGeometrySnapshot {
    view_targets: Arc<BTreeMap<u64, RenderTarget>>,
    environments: BTreeMap<u64, Arc<crate::environment_lighting::EnvironmentLighting>>,
    _resource_lease: Arc<()>,
    resource_handles: Arc<Vec<crate::render_resource::RenderResourceHandle>>,
    built: Arc<std::sync::Mutex<Option<BuiltGeometry>>>,
    instance_reuse: Arc<std::sync::Mutex<super::build_inputs::InstanceReuse>>,
    worksets: Arc<std::sync::Mutex<Vec<Arc<PreparedWorkset>>>>,
    pub(super) meshes: Arc<Vec<PreparedProjection>>,
    instances: Arc<Vec<Arc<RetainedInstance>>>,
    pub content_version: u64,
    pub updated_instances: usize,
    pub views: Arc<Vec<RetainedGeometryView>>,
    pub(crate) identity_views: Arc<Vec<crate::render_thread::RenderViewHistoryIdentity>>,
    pub(crate) textures: Arc<Vec<PreparedTextureAsset>>,
    pub(super) error: Option<ContractError>,
}

/// Per-view immutable origin and camera history, frozen with the frame packet.
#[derive(Debug, Clone, PartialEq)]
pub struct RetainedGeometryView {
    pub camera: crate::render_projection_adapter::CameraProjection,
    pub previous_camera: crate::render_projection_adapter::CameraProjection,
    pub history_version: u64,
}

#[derive(Debug, Clone)]
pub(super) struct RetainedInstance {
    pub(super) projection: InstanceProjection,
    pub(super) dynamic_bounds: Option<crate::render_contract::GeometryBounds>,
    pub(super) prepared: Arc<PreparedMeshRenderer>,
    pub(super) submesh: usize,
    pub(super) instance_id: u64,
    pub(super) visibility: crate::render_contract::Visibility,
}

#[derive(Debug, Clone)]
pub(super) enum InstanceProjection {
    Mesh(Arc<MeshProjection>),
    Vegetation {
        owner: Arc<MeshProjection>,
        matrix: [f32; 16],
        source: Arc<str>,
    },
    #[cfg(feature = "spine")]
    Spine(Arc<spine::SpineProjection>),
}
impl InstanceProjection {
    fn presentation(&self) -> Option<&crate::model_lod3d::MeshPresentation3D> {
        match self {
            Self::Mesh(p) => p.presentation.as_deref(),
            Self::Vegetation { owner, .. } => owner.presentation.as_deref(),
            #[cfg(feature = "spine")]
            Self::Spine(_) => None,
        }
    }
    fn has_selection(&self) -> bool {
        self.presentation()
            .is_some_and(|p| p.range.is_some() || p.hlod.is_some())
    }
    fn visible(&self, camera: &crate::render_projection_adapter::CameraProjection) -> bool {
        let Some(p) = self.presentation() else {
            return true;
        };
        let matrix = self.matrix();
        let origin = self.origin();
        let absolute = std::array::from_fn(|i| origin[i] + f64::from(matrix[12 + i]));
        let view = std::array::from_fn(|i| camera.origin[i] + f64::from(camera.pose.position[i]));
        p.visible(view, absolute)
    }
    pub(super) fn deformation(
        &self,
    ) -> Option<&Arc<crate::character_animation::SkinDeformation3D>> {
        match self {
            Self::Mesh(p) => p.deformation.as_ref(),
            Self::Vegetation { .. } => None,
            #[cfg(feature = "spine")]
            Self::Spine(_) => None,
        }
    }
    fn matrix(&self) -> [f32; 16] {
        match self {
            Self::Mesh(p) => p.world.world_matrix,
            Self::Vegetation { matrix, .. } => *matrix,
            #[cfg(feature = "spine")]
            Self::Spine(p) => p.matrix,
        }
    }
    fn origin(&self) -> [f64; 3] {
        match self {
            Self::Mesh(p) => p.coordinate_origin,
            Self::Vegetation { owner, .. } => owner.coordinate_origin,
            #[cfg(feature = "spine")]
            Self::Spine(_) => [0.; 3],
        }
    }
    pub(super) fn layers(&self) -> &[String] {
        match self {
            Self::Mesh(p) => &p.component.layers,
            Self::Vegetation { owner, .. } => &owner.component.layers,
            #[cfg(feature = "spine")]
            Self::Spine(p) => &p.layers,
        }
    }
    pub(super) fn provenance(&self) -> &crate::render_contract::Provenance {
        match self {
            Self::Mesh(p) => &p.provenance,
            Self::Vegetation { owner, .. } => &owner.provenance,
            #[cfg(feature = "spine")]
            Self::Spine(p) => &p.provenance,
        }
    }
    fn visibility(&self) -> crate::render_contract::Visibility {
        match self {
            Self::Mesh(p) => p.visibility(),
            Self::Vegetation { owner, .. } => owner.visibility(),
            #[cfg(feature = "spine")]
            Self::Spine(p) => p.visibility.clone(),
        }
    }
    #[cfg(test)]
    fn mesh_mut(&mut self) -> &mut MeshProjection {
        match self {
            Self::Mesh(p) => Arc::make_mut(p),
            Self::Vegetation { .. } => panic!("expected mesh fixture"),
            #[cfg(feature = "spine")]
            Self::Spine(_) => panic!("expected mesh fixture"),
        }
    }
}

fn representation_projections(
    scene: &RenderSceneState,
) -> Result<
    Vec<(
        (crate::render_state::RenderProxyId, usize),
        Arc<MeshProjection>,
    )>,
    ContractError,
> {
    use crate::model_lod3d::HlodRole3D;
    let mut groups: BTreeMap<&str, (&crate::model_lod3d::Hlod3D, usize, usize)> = BTreeMap::new();
    for proxy in scene.proxies() {
        let RenderProxyPayload::Mesh3D(p) = &proxy.payload else {
            continue;
        };
        if let Some(h) = p.presentation.as_ref().and_then(|p| p.hlod.as_ref()) {
            let entry = groups.entry(&h.group).or_insert((h, 0, 0));
            if entry.0.center != h.center || entry.0.distance != h.distance {
                return Err(super::invalid("hlod.group_contract"));
            }
            match h.role {
                HlodRole3D::Detail => entry.1 += 1,
                HlodRole3D::Proxy => entry.2 += 1,
            }
        }
    }
    if groups
        .values()
        .any(|(_, details, proxies)| *details == 0 || *proxies != 1)
    {
        return Err(super::invalid("hlod.incomplete_group"));
    }
    let mut result = Vec::new();
    for proxy in scene.proxies() {
        let RenderProxyPayload::Mesh3D(p) = &proxy.payload else {
            continue;
        };
        if p.presentation
            .as_ref()
            .is_some_and(|m| m.vegetation.is_some())
        {
            continue;
        }
        if let Some(lod) = p.presentation.as_ref().and_then(|m| m.lod.as_ref()) {
            lod.validate().map_err(|_| super::invalid("lod.invalid"))?;
            for (level, item) in lod.levels.iter().enumerate() {
                let mut projection = p.as_ref().clone();
                projection.component = item.renderer.clone();
                let metadata = Arc::make_mut(projection.presentation.as_mut().unwrap());
                metadata.range = Some((
                    item.distance,
                    lod.levels.get(level + 1).map(|next| next.distance),
                ));
                metadata.source_instance = Some(format!("lod:{level}"));
                result.push(((proxy.common.proxy_id, level), Arc::new(projection)));
            }
        } else {
            result.push(((proxy.common.proxy_id, 0), Arc::new(p.as_ref().clone())));
        }
    }
    Ok(result)
}

#[derive(Debug)]
struct PreparedWorkset {
    source_instances: Arc<Vec<Arc<RetainedInstance>>>,
    instances: Arc<Vec<Arc<RetainedInstance>>>,
    origin: [u64; 3],
    selection: Option<[u64; 3]>,
    lease: Arc<()>,
    inputs: super::build_inputs::PreparedInputs,
}
#[derive(Debug)]
struct CachedMesh {
    projection: Arc<MeshProjection>,
    prepared: Arc<PreparedMeshRenderer>,
    binding_key: Arc<str>,
    instances: Vec<Arc<RetainedInstance>>,
}

impl ProductionGeometrySnapshot {
    fn output_base_extent(&self, target_id: Option<&str>) -> [u32; 2] {
        self.views
            .iter()
            .filter(|v| {
                self.view_targets
                    .get(&v.camera.view.view_id)
                    .map(|target| target.target_id.as_str())
                    == target_id
            })
            .fold([1, 1], |mut extent, v| {
                let [x, y, w, h] = v.camera.view.viewport;
                extent[0] = extent[0].max(x.saturating_add(w));
                extent[1] = extent[1].max(y.saturating_add(h));
                extent
            })
    }

    pub(super) fn observation_camera_for_target(
        &self,
        camera: &crate::render_projection_adapter::CameraProjection,
        target: &RenderTarget,
    ) -> Result<Option<(crate::render_projection_adapter::CameraProjection, [u32; 2])>, ContractError>
    {
        let bound = self.view_targets.get(&camera.view.view_id);
        let destination = bound.unwrap_or(target);
        let base_extent = self.output_base_extent(bound.map(|t| t.target_id.as_str()));
        output_view(camera, destination, base_extent).map(|mapped| {
            mapped.map(|(view, pose)| {
                let mut camera = camera.clone();
                camera.view = view;
                camera.pose = pose;
                (camera, [destination.width, destination.height])
            })
        })
    }

    /// Immutable Prepare products already owned by this snapshot.
    #[doc(hidden)]
    pub fn prepared_geometries(&self) -> impl Iterator<Item = &super::PreparedGeometry> {
        self.instances
            .iter()
            .map(|item| &item.prepared.geometry[item.submesh])
    }

    fn prepared_workset(
        &self,
        camera: &crate::render_projection_adapter::CameraProjection,
        manager: &RenderResourceManager,
    ) -> Result<Arc<PreparedWorkset>, ContractError> {
        let origin = camera.origin.map(f64::to_bits);
        let selection = self
            .instances
            .iter()
            .any(|item| item.projection.has_selection())
            .then(|| camera.pose.position.map(|v| f64::from(v).to_bits()));
        let mut worksets = self.worksets.lock().expect("geometry worksets poisoned");
        if let Some(workset) = worksets.iter().find(|w| {
            w.origin == origin
                && w.selection == selection
                && Arc::ptr_eq(&w.source_instances, &self.instances)
                && Arc::ptr_eq(&w.lease, &self._resource_lease)
        }) {
            return Ok(workset.clone());
        }
        let instances = self.build_instances(camera)?;
        let mut inputs = super::build_inputs::GeometryBuildInputs::borrowed(
            &instances,
            manager,
            Some(&self._resource_lease),
        )
        .map_err(|e| e.error)?
        .with_instance_reuse(&self.instance_reuse, origin);
        inputs.enable_compact().map_err(|e| e.error)?;
        let workset = Arc::new(PreparedWorkset {
            source_instances: self.instances.clone(),
            // PreparedInputs was frozen from this camera's selected rows. The
            // retained consumer must use the same row order, including when a
            // skipped LOD/HLOD representation precedes a selected row.
            instances: if selection.is_some() {
                Arc::new(
                    self.instances
                        .iter()
                        .filter(|item| item.projection.visible(camera))
                        .cloned()
                        .collect(),
                )
            } else {
                self.instances.clone()
            },
            origin,
            selection,
            lease: self._resource_lease.clone(),
            inputs: inputs.freeze(),
        });
        // Snapshot ownership is the lifetime. No global strong table keeps old
        // packet resources alive; exceeding reuse capacity only causes a miss.
        if worksets.len() < 16 {
            worksets.push(workset.clone());
        }
        Ok(workset)
    }

    fn build_instances(
        &self,
        camera: &crate::render_projection_adapter::CameraProjection,
    ) -> Result<Vec<super::build_inputs::BorrowedGeometryInstance<'_>>, ContractError> {
        self.instances
            .iter()
            .filter(|item| item.projection.visible(camera))
            .map(|item| {
                let mut matrix = item.projection.matrix();
                let relative = relative_position(
                    [matrix[12], matrix[13], matrix[14]],
                    item.projection.origin(),
                    camera.origin,
                )?;
                matrix[12..15].copy_from_slice(&relative);
                Ok(super::build_inputs::BorrowedGeometryInstance {
                    deformation: item.projection.deformation(),
                    dynamic_bounds: item.dynamic_bounds.as_ref(),
                    instance_id: item.instance_id,
                    source: &item.prepared.geometry[item.submesh],
                    world_transform: matrix,
                    visibility: item.projection.visibility(),
                    layers: item.projection.layers(),
                    provenance: item.projection.provenance(),
                })
            })
            .collect::<Result<Vec<_>, ContractError>>()
    }

    /// Frozen prepare result only; callers may defer an unready frame without
    /// accessing mutable loader state or treating a terminal failure as ready.
    pub fn preparation_error(&self) -> Option<ContractError> {
        self.error
    }

    /// Freeze host output bindings without touching the shared source snapshot.
    /// Unknown view IDs fail at frame construction rather than disappearing silently.
    pub(crate) fn with_view_targets(
        &self,
        targets: &BTreeMap<u64, RenderTarget>,
        revision: u64,
    ) -> Self {
        let mut snapshot = self.clone();
        snapshot.view_targets = Arc::new(targets.clone());
        snapshot.built = Default::default();
        // Source-content and output-binding changes both invalidate immutable
        // packet identity. Conservative all-view history invalidation is bounded
        // by the existing 16-view cap, without a second temporal-history owner.
        match snapshot.content_version.checked_add(revision) {
            Some(version) => snapshot.content_version = version,
            None => snapshot.error = Some(super::invalid("view.outputRevisionOverflow")),
        }
        let views = Arc::make_mut(&mut snapshot.views);
        let identities = Arc::make_mut(&mut snapshot.identity_views);
        for view in views {
            match view.history_version.checked_add(revision) {
                Some(version) => view.history_version = version,
                None => snapshot.error = Some(super::invalid("view.outputRevisionOverflow")),
            }
        }
        for view in identities {
            match view.history_version.checked_add(revision) {
                Some(version) => view.history_version = version,
                None => snapshot.error = Some(super::invalid("view.outputRevisionOverflow")),
            }
        }
        snapshot
    }

    pub fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_arc(&self.meshes, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.instances, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.views, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.identity_views, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.textures, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.resource_handles, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.view_targets, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.built, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.instance_reuse, "snapshots", &["snapshot"]);
        self.instance_reuse
            .lock()
            .expect("geometry instance reuse poisoned")
            .observe_memory(sample);
        for (projection, prepared) in self.meshes.iter() {
            sample.cpu_arc(projection, "snapshots", &["snapshot"]);
            sample.cpu_arc(prepared, "snapshots", &["snapshot"]);
            sample.cpu_string(
                &projection.component.schema_version,
                "snapshots",
                &["snapshot"],
            );
            super::memory::asset_reference(sample, &projection.component.mesh.0, "snapshot");
            super::memory::provenance_fields(sample, &projection.provenance, "snapshot");
            for (key, reference) in &projection.component.materials {
                sample.cpu_string(key, "snapshots", &["snapshot"]);
                super::memory::asset_reference(sample, &reference.0, "snapshot");
            }
            sample.cpu_vec(&projection.component.layers, "snapshots", &["snapshot"]);
            for layer in &projection.component.layers {
                sample.cpu_string(layer, "snapshots", &["snapshot"]);
            }
            sample.cpu_vec(&prepared.geometry, "snapshots", &["snapshot"]);
            sample.cpu_vec(&prepared.textures, "snapshots", &["snapshot"]);
            sample.cpu_vec(&prepared.retained_resources, "snapshots", &["snapshot"]);
            for geometry in &prepared.geometry {
                geometry.observe_memory(sample, "snapshot");
            }
        }
        for e in self.environments.values() {
            s_environment(e, sample);
        }
        for texture in self.textures.iter() {
            texture.observe_memory(sample);
        }
        sample.cpu_vec(&self.meshes, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.instances, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.views, "snapshots", &["snapshot"]);
        for target in self.view_targets.values() {
            sample.cpu_string(&target.target_id, "snapshots", &["snapshot"]);
            sample.cpu_string(&target.format, "snapshots", &["snapshot"]);
            sample.cpu_string(&target.color_space, "snapshots", &["snapshot"]);
        }
        for view in self.views.iter() {
            for camera in [&view.camera, &view.previous_camera] {
                if let Some(pipeline) = &camera.view.color_pipeline {
                    pipeline.observe_memory(sample, "snapshots", "snapshot");
                }
                if let Some(id) = &camera.view.source_camera_entity_id {
                    sample.cpu_string(id, "snapshots", &["snapshot"]);
                }
                if let crate::render_contract::LayerSelection::Named { names } = &camera.view.layers
                {
                    sample.cpu_vec(names, "snapshots", &["snapshot"]);
                    for name in names {
                        sample.cpu_string(name, "snapshots", &["snapshot"]);
                    }
                }
                super::memory::provenance_fields(sample, &camera.view.provenance, "snapshot");
                sample.cpu_arc(&camera.lights, "snapshots", &["snapshot"]);
                sample.cpu_vec(&camera.lights, "snapshots", &["snapshot"]);
                for light in camera.lights.iter() {
                    sample.cpu_vec(&light.component.layers, "snapshots", &["snapshot"]);
                    for layer in &light.component.layers {
                        sample.cpu_string(layer, "snapshots", &["snapshot"]);
                    }
                    sample.cpu_string(&light.component.schema_version, "snapshots", &["snapshot"]);
                }
            }
        }
        sample.cpu_vec(&self.identity_views, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.textures, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.resource_handles, "snapshots", &["snapshot"]);
        for item in self.instances.iter() {
            sample.cpu_arc(item, "snapshots", &["snapshot"]);
        }
        sample.cpu_arc(&self.worksets, "snapshots", &["snapshot"]);
        {
            let worksets = self.worksets.lock().expect("geometry worksets poisoned");
            sample.cpu_vec(&worksets, "snapshots", &["snapshot"]);
            for workset in worksets.iter() {
                sample.cpu_arc(workset, "snapshots", &["snapshot"]);
                sample.cpu_arc(&workset.source_instances, "snapshots", &["snapshot"]);
                sample.cpu_vec(&workset.source_instances, "snapshots", &["snapshot"]);
                sample.cpu_arc(&workset.instances, "snapshots", &["snapshot"]);
                sample.cpu_vec(&workset.instances, "snapshots", &["snapshot"]);
                workset.inputs.observe_memory(sample);
            }
        }
        if let Some((target, frame)) = self
            .built
            .lock()
            .expect("geometry snapshot cache poisoned")
            .as_ref()
        {
            sample.cpu_string(&target.target_id, "snapshots", &["snapshot"]);
            sample.cpu_arc(frame, "snapshots", &["snapshot"]);
            frame.observe_memory(sample);
        }
        sample.partial("cpu.owner.snapshots", "snapshot.descriptorArcs+vectors+bindings+draws+batchIdentityArcs+viewMetadata+textureDescriptors.capacity;stdNodes+instanceReuseWeakNodes+batchIdentityHashNodes+arcControlOpaque");
        sample.partial("cpu.role.snapshot", "snapshot.sharedBacking.capacity");
    }

    pub(crate) fn build(
        &self,
        target: &RenderTarget,
        manager: &RenderResourceManager,
    ) -> Result<Arc<GeometryFrame>, ContractError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if target.width == 0 || target.height == 0 {
            *self.built.lock().expect("geometry snapshot cache poisoned") = None;
            return self.build_uncached(target, manager).map(Arc::new);
        }
        // The snapshot freezes descriptors, never manager residency. A hit still
        // proves every distinct native resource's generation and lease in this owner.
        if !manager.native_leases_valid(&self.resource_handles, &self._resource_lease) {
            return Err(super::invalid("nativeLease.stale"));
        }
        let mut built = self.built.lock().expect("geometry snapshot cache poisoned");
        if let Some((previous_target, frame)) = &*built {
            if previous_target == target {
                return Ok(Arc::clone(frame));
            }
        }
        let frame = Arc::new(self.build_uncached(target, manager)?);
        *built = Some((target.clone(), Arc::clone(&frame)));
        Ok(frame)
    }

    fn build_uncached(
        &self,
        target: &RenderTarget,
        manager: &RenderResourceManager,
    ) -> Result<GeometryFrame, ContractError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let extent = [target.width, target.height];
        for (id, destination) in self.view_targets.iter() {
            if !self.views.iter().any(|v| v.camera.view.view_id == *id)
                || destination.target_id.is_empty()
                || destination.width == 0
                || destination.height == 0
                || destination.target_kind
                    == crate::runtime_renderer::RuntimeRenderTargetKind::Surface
                || destination.target_id == target.target_id
                || destination.format != target.format
                || destination.color_space != target.color_space
            {
                return Err(super::invalid("view.outputBinding"));
            }
        }
        for destination in self.view_targets.values() {
            if self
                .view_targets
                .values()
                .any(|other| other.target_id == destination.target_id && other != destination)
            {
                return Err(super::invalid("view.outputDescriptorConflict"));
            }
        }
        // A camera remains an output producer after its last mesh unloads.
        // Preserve the old no-camera/empty-world behavior, but keep view clear
        // and depth phases for live cameras on independent targets.
        if extent.contains(&0) || (self.meshes.is_empty() && self.views.is_empty()) {
            return Ok(GeometryFrame {
                target: target.target_id.clone(),
                extent,
                views: Vec::new(),
            });
        }
        if self.views.is_empty() {
            return Err(ContractError::new(
                ContractErrorCode::InvalidProjection,
                "mainCamera.missing",
            ));
        }
        let base_extent = self.output_base_extent(None);
        let mut output = GeometryFrame {
            target: target.target_id.clone(),
            extent,
            views: Vec::new(),
        };
        for retained in self.views.iter() {
            let camera = &retained.camera;
            let bound = self.view_targets.get(&camera.view.view_id);
            let destination = bound.unwrap_or(target);
            let extent = [destination.width, destination.height];
            // Independent outputs keep their own viewport coordinate basis.
            let base_extent = if bound.is_some() {
                self.output_base_extent(Some(&destination.target_id))
            } else {
                base_extent
            };
            let Some((view, pose)) = output_view(camera, destination, base_extent)? else {
                continue;
            };
            // IDs/prepared objects are retained; visibility and view-relative execution
            // descriptors are evaluated per view without touching simulation history.
            let workset = self.prepared_workset(camera, manager)?;
            let mut geometry_inputs = super::build_inputs::GeometryBuildInputs::retained(
                &workset.instances,
                &workset.inputs,
                manager,
                &self._resource_lease,
            )
            .map_err(|e| e.error)?
            .with_instance_reuse(&self.instance_reuse, camera.origin.map(f64::to_bits));
            let mut pending = super::prepare_tasks::select_color(
                &destination.target_id,
                extent,
                &GeometryViewInput { view: &view, pose },
                &mut geometry_inputs,
            )?;
            pending.frame.color_target = bound.cloned();
            pending.frame.coordinate_origin = camera.origin;
            let frame = super::shadow::build_scene_lighting(
                pending,
                &destination.target_id,
                GeometryViewInput { view: &view, pose },
                camera.origin,
                &mut geometry_inputs,
                &camera.lights,
                self.environments.get(&view.view_id).cloned(),
            )?;
            output.views.extend(frame.views);
        }
        Ok(output)
    }
}

/// Cache belongs to the existing frame/Prepare seam, keyed only by consumed package
/// and binding identity. Transform, visibility and camera changes reuse buffers.
#[derive(Debug, Default)]
pub(crate) struct ProductionGeometryCache {
    #[cfg(feature = "spine")]
    spine: spine::SpinePreparedFrame,
    prepared: BTreeMap<Arc<str>, Arc<PreparedMeshRenderer>>,
    generated_candidates: BTreeMap<(u64, u64, u64), Arc<PreparedMeshRenderer>>,
    generated_bindings: BTreeSet<Arc<str>>,
    generated_changed: bool,
    mesh_backings: std::collections::HashMap<
        crate::render_resource::RenderResourceHandle,
        std::sync::Weak<super::GeometryBuffers>,
    >,
    texture_sources: std::collections::HashMap<
        crate::render_resource::RenderResourceHandle,
        std::sync::Weak<crate::render_asset_production::PackageTexturePixels>,
    >,
    owned_handles: HashSet<crate::render_resource::RenderResourceHandle>,
    environment_requests: BTreeMap<
        String,
        (
            crate::runtime_asset::RuntimeAssetHandle,
            Option<Arc<crate::environment_lighting::EnvironmentMap>>,
        ),
    >,
    textures: Arc<Vec<PreparedTextureAsset>>,
    entries: BTreeMap<(crate::render_state::RenderProxyId, usize), CachedMesh>,
    vegetation: vegetation::VegetationPreparedFrame,
    package_identity: String,
    last: Option<Arc<ProductionGeometrySnapshot>>,
    last_proxy_identity: Option<Arc<()>>,
    instance_reuse: Arc<std::sync::Mutex<super::build_inputs::InstanceReuse>>,
    content_version: u64,
}

impl ProductionGeometryCache {
    fn share_mesh_backings(
        backings: &mut std::collections::HashMap<
            crate::render_resource::RenderResourceHandle,
            std::sync::Weak<super::GeometryBuffers>,
        >,
        prepared: &mut PreparedMeshRenderer,
        loader: Option<&RuntimeAssetLoader>,
    ) {
        for source in &mut prepared.geometry {
            let Some(handle) = source.geometry_binding.slots.iter().find_map(|slot| {
                if let crate::render_contract::BindingResource::Geometry(handle) = slot.resource {
                    Some(handle.raw())
                } else {
                    None
                }
            }) else {
                continue;
            };
            // A handle is only a candidate key. Conflicting content must retain
            // its own backing and reach the existing native conflict checks.
            if let Some(previous) = backings.get(&handle).and_then(std::sync::Weak::upgrade) {
                if previous == source.buffers {
                    source.buffers = previous;
                    continue;
                }
            }
            // Bounded preparation metadata, never resource admission or a lease.
            if backings.len() >= 65_536 {
                backings.clear();
            }
            if let Some(streamable) = loader.and_then(|loader| {
                super::GeometryBuffers::make_package_streamable(
                    &source.buffers,
                    loader,
                    &source.geometry_source,
                )
            }) {
                source.buffers = streamable;
            }
            backings.insert(handle, Arc::downgrade(&source.buffers));
        }
    }

    /// Finish only the original frame's pending environment requests. In
    /// particular, do not extract a new camera or advance previous-frame history.
    pub(crate) fn retry_pending_resources(
        &mut self,
        loader: &mut RuntimeAssetLoader,
    ) -> Option<Arc<ProductionGeometrySnapshot>> {
        let last = self.last.as_ref()?;
        if last.error.map(|e| e.code) != Some(ContractErrorCode::ResourceNotReady) {
            return Some(last.clone());
        }
        loader.poll_async();
        let mut environments = BTreeMap::new();
        let mut error = None;
        for retained in last.views.iter() {
            let camera = &retained.camera;
            let Some(config) = camera
                .view
                .color_pipeline
                .as_ref()
                .and_then(|p| p.ibl.as_ref())
            else {
                continue;
            };
            let key = serde_json::to_string(&(
                &self.package_identity,
                loader.asset_revision_epoch(),
                &config.environment,
            ))
            .expect("typed environment ref");
            // A frozen frame cannot recreate a released request or adopt a new
            // loader/index generation while waiting for its original resources.
            let Some((handle, prepared)) = self.environment_requests.get_mut(&key) else {
                error = Some(super::invalid("environment.pending_identity_changed"));
                break;
            };
            if loader
                .current_handle(handle)
                .is_none_or(|h| h.state == crate::runtime_asset::RuntimeAssetLoadState::Released)
            {
                error = Some(super::invalid("environment.pending_request_released"));
                break;
            }
            if prepared.is_none() {
                if let Some(payload) = loader.decoded_payload(handle) {
                    if let crate::runtime_asset::RuntimeDecodedPayload::Environment(map) =
                        payload.as_ref()
                    {
                        *prepared = Some(map.clone());
                    } else {
                        error = Some(super::invalid("environment.payload_kind"));
                        break;
                    }
                }
            }
            if let Some(map) = prepared {
                let previous = last
                    .environments
                    .get(&camera.view.view_id)
                    .filter(|e| Arc::ptr_eq(&e.map, map) && e.intensity == config.intensity);
                environments.insert(
                    camera.view.view_id,
                    previous.cloned().unwrap_or_else(|| {
                        Arc::new(crate::environment_lighting::EnvironmentLighting {
                            map: map.clone(),
                            intensity: config.intensity,
                        })
                    }),
                );
            } else {
                let code = if loader.streaming_error(handle).is_some() {
                    ContractErrorCode::MissingCookedPayload
                } else {
                    ContractErrorCode::ResourceNotReady
                };
                if code != ContractErrorCode::ResourceNotReady {
                    error = Some(ContractError::new(code, "environment.async"));
                    break;
                }
                error.get_or_insert(ContractError::new(code, "environment.async"));
            }
        }
        if last.error == error
            && last.environments.len() == environments.len()
            && environments.iter().all(|(id, e)| {
                last.environments
                    .get(id)
                    .is_some_and(|old| Arc::ptr_eq(old, e))
            })
        {
            return Some(last.clone());
        }
        let mut next = last.as_ref().clone();
        next.error = error;
        next.environments = environments;
        next.built = Default::default();
        self.content_version = self
            .content_version
            .checked_add(1)
            .expect("geometry content version");
        next.content_version = self.content_version;
        let next = Arc::new(next);
        self.last = Some(next.clone());
        Some(next)
    }

    pub(crate) fn reset_world(&mut self, frame: u64, manager: &mut RenderResourceManager) {
        #[cfg(feature = "spine")]
        self.spine.retire(frame, manager);
        for handle in &self.owned_handles {
            let _ = manager.release_resource_ownership(RenderResourceReleaseRequest {
                handle: *handle,
                frame_index: frame,
            });
        }
        // Requests remain owned until snapshot can release them through the current loader.
        let requests = std::mem::take(&mut self.environment_requests);
        self.vegetation.clear_resources();
        let vegetation = std::mem::take(&mut self.vegetation);
        *self = Self::default();
        self.vegetation = vegetation;
        self.environment_requests = requests;
    }

    pub(crate) fn snapshot(
        &mut self,
        frame: u64,
        scene: &RenderSceneState,
        package_identity: &str,
        mut loader: Option<&mut RuntimeAssetLoader>,
        manager: &mut RenderResourceManager,
        camera_error: Option<ContractError>,
    ) -> Arc<ProductionGeometrySnapshot> {
        self.project_vegetation(
            frame,
            scene,
            package_identity,
            loader.as_deref_mut(),
            manager,
        );
        let package_changed = self.package_identity != package_identity;
        if package_changed {
            self.entries.clear();
            self.package_identity = package_identity.into();
            self.instance_reuse = Default::default();
        }
        let mut active_proxies = BTreeSet::new();
        let mut updated_instances = 0;
        let mut entries_changed = self.vegetation.changed;
        let mut active = BTreeSet::new();
        let mut error = camera_error.or(self.vegetation.error);
        #[cfg(feature = "spine")]
        {
            error = error.or(self.spine.error);
        }
        let mut changed = std::mem::take(&mut self.generated_changed);
        let proxy_identity = scene.proxy_identity();
        let meshes_unchanged = !package_changed
            && self.last.as_ref().is_some_and(|last| last.error.is_none())
            && self
                .last_proxy_identity
                .as_ref()
                .is_some_and(|previous| Arc::ptr_eq(previous, &proxy_identity));
        if !meshes_unchanged {
            let representations = representation_projections(scene);
            if let Err(failure) = &representations {
                error.get_or_insert(*failure);
            }
            for (proxy_id, projection) in representations.unwrap_or_default() {
                let previous = self.entries.get(&proxy_id);
                let binding_unchanged = previous.is_some_and(|p| {
                    p.projection.component.mesh == projection.component.mesh
                        && p.projection.component.materials == projection.component.materials
                });
                let key: Arc<str> = if binding_unchanged {
                    previous.unwrap().binding_key.clone()
                } else {
                    serde_json::to_string(&(
                        package_identity,
                        &projection.component.mesh,
                        &projection.component.materials,
                    ))
                    .expect("typed asset bindings")
                    .into()
                };
                active.insert(key.clone());
                if !self.prepared.contains_key(&key) {
                    let result = loader
                        .as_deref_mut()
                        .ok_or_else(|| {
                            ContractError::new(
                                ContractErrorCode::MissingCookedPayload,
                                "runtimePackage",
                            )
                        })
                        .and_then(|loader| {
                            RuntimeRenderAssetProducer.prepare_mesh_renderer(
                                frame,
                                &projection.component,
                                loader,
                                manager,
                            )
                        });
                    match result {
                        Ok(mut prepared) => {
                            Self::share_mesh_backings(
                                &mut self.mesh_backings,
                                &mut prepared,
                                loader.as_deref(),
                            );
                            if let Some(loader) = loader.as_deref() {
                                for texture in &mut prepared.textures {
                                    texture
                                        .make_package_streamable(loader, &mut self.texture_sources);
                                }
                            }
                            self.prepared.insert(key.clone(), Arc::new(prepared));
                            changed = true;
                        }
                        Err(failure) => {
                            error.get_or_insert(failure);
                            continue;
                        }
                    }
                }
                let prepared = Arc::clone(&self.prepared[&key]);
                let unchanged = previous.is_some_and(|p| {
                    p.projection.as_ref() == projection.as_ref()
                        && Arc::ptr_eq(&p.prepared, &prepared)
                });
                if !unchanged {
                    let projection = Arc::new(projection.as_ref().clone());
                    let records = prepared
                        .geometry
                        .iter()
                        .enumerate()
                        .map(|(submesh, geometry)| {
                            Arc::new(RetainedInstance {
                                dynamic_bounds: projection
                                    .deformation
                                    .as_ref()
                                    .map(|d| d.conservative_bounds(&geometry.bounds)),
                                visibility: projection.visibility(),
                                instance_id: previous
                                    .filter(|old| {
                                        binding_unchanged
                                            && old.projection.runtime_entity_id
                                                == projection.runtime_entity_id
                                    })
                                    .and_then(|old| old.instances.get(submesh))
                                    .map_or_else(
                                        || projection.submesh_instance_id(geometry.material_slot()),
                                        |old| old.instance_id,
                                    ),
                                projection: InstanceProjection::Mesh(projection.clone()),
                                prepared: prepared.clone(),
                                submesh,
                            })
                        })
                        .collect::<Vec<_>>();
                    updated_instances += records.len();
                    entries_changed = true;
                    self.entries.insert(
                        proxy_id,
                        CachedMesh {
                            projection,
                            prepared,
                            binding_key: key,
                            instances: records,
                        },
                    );
                }
                active_proxies.insert(proxy_id);
            }
            self.entries.retain(|id, _| {
                let keep = active_proxies.contains(id);
                entries_changed |= !keep;
                keep
            });

            self.prepared.retain(|key, _| {
                let keep = active.contains(key) || self.generated_bindings.contains(key);
                changed |= !keep;
                keep
            });
            if changed {
                self.mesh_backings
                    .retain(|_, weak| weak.strong_count() != 0);
                self.texture_sources
                    .retain(|_, weak| weak.strong_count() != 0);
            }
            if changed {
                let mut handles = HashSet::new();
                self.textures = Arc::new(
                    self.prepared
                        .values()
                        .flat_map(|p| &p.textures)
                        .filter(|t| handles.insert(t.handle))
                        .cloned()
                        .collect(),
                );
                // All preparations in this batch shared the loader's decoded
                // pixels. Upload sources now own the pending CPU leases.
                if let Some(loader) = loader.as_deref_mut() {
                    for texture in self.textures.iter() {
                        texture.release_loader_pixels(loader);
                    }
                }
            }
        }
        if changed || self.vegetation.changed {
            self.refresh_owned_handles(frame, manager);
        }
        self.last_proxy_identity = Some(proxy_identity);
        #[cfg(feature = "spine")]
        {
            entries_changed |= self.spine.changed;
            updated_instances += self.spine.instances.len();
        }
        let mut views = Vec::new();
        for camera in scene.views().filter_map(|v| v.spatial.as_ref()) {
            let previous = self.last.as_ref().and_then(|last| {
                last.views
                    .iter()
                    .find(|v| v.camera.view.view_id == camera.view.view_id)
            });
            let reset = previous.is_none_or(|v| {
                v.camera.origin != camera.origin
                    || v.camera.view.viewport != camera.view.viewport
                    || v.camera.view.projection != camera.view.projection
                    || v.camera.view.source_camera_entity_id != camera.view.source_camera_entity_id
            });
            let previous_camera = if reset {
                camera.clone()
            } else {
                previous.unwrap().camera.clone()
            };
            let history_version = previous.map_or(1, |v| v.history_version + u64::from(reset));
            if let Some(old) = previous.filter(|v| v.camera == *camera) {
                views.push(RetainedGeometryView {
                    camera: old.camera.clone(),
                    previous_camera: old.camera.clone(),
                    history_version,
                });
            } else {
                views.push(RetainedGeometryView {
                    camera: camera.clone(),
                    previous_camera,
                    history_version,
                });
            }
        }
        let mut environments = BTreeMap::new();
        let mut active_environment_keys = BTreeSet::new();
        #[cfg(feature = "spine")]
        if !self.spine.instances.is_empty() {
            for source in scene.views().filter(|v| v.spatial.is_none()) {
                let height = source.projection_matrix[5];
                if height <= 0. || !height.is_finite() {
                    error.get_or_insert(super::invalid("spine.camera2d"));
                    continue;
                }
                let mut view = crate::render_contract::RenderView::orthographic(
                    source.view_id.0,
                    source.viewport.width,
                    source.viewport.height,
                    2. / height,
                    -1000.,
                    1000.,
                );
                view.layers = crate::render_contract::LayerSelection::Named {
                    names: vec!["main".into()],
                };
                view.clear_policy = crate::render_contract::ClearPolicy::Color {
                    color: source.clear_color,
                };
                view.source_camera_entity_id =
                    source.source_entity_id.as_ref().map(ToString::to_string);
                let camera = crate::render_projection_adapter::CameraProjection {
                    lights: Default::default(),
                    view,
                    pose: crate::render_contract::CameraPose {
                        position: [
                            -source.view_matrix[12],
                            -source.view_matrix[13],
                            -source.view_matrix[14],
                        ],
                        ..Default::default()
                    },
                    suspended: false,
                    origin: [0.; 3],
                    coordinate_origin: [0.; 3],
                };
                let previous = self.last.as_ref().and_then(|s| {
                    s.views
                        .iter()
                        .find(|v| v.camera.view.view_id == camera.view.view_id)
                });
                views.push(RetainedGeometryView {
                    previous_camera: previous.map_or_else(|| camera.clone(), |v| v.camera.clone()),
                    camera,
                    history_version: previous.map_or(1, |v| v.history_version),
                });
            }
        }
        if let Some(loader) = loader.as_deref_mut() {
            loader.poll_async();
            // RuntimeAssetLoader generations are process-wide unique. A replacement loader
            // cannot inherit stale pending work or release another loader's handle.
            self.environment_requests.retain(|_, (h, _)| {
                loader.current_handle(h).is_some_and(|h| {
                    h.state != crate::runtime_asset::RuntimeAssetLoadState::Released
                })
            });
            for retained in &views {
                let camera = &retained.camera;
                let Some(config) = camera
                    .view
                    .color_pipeline
                    .as_ref()
                    .and_then(|p| p.ibl.as_ref())
                else {
                    continue;
                };
                let key = serde_json::to_string(&(
                    package_identity,
                    loader.asset_revision_epoch(),
                    &config.environment,
                ))
                .expect("typed environment ref");
                active_environment_keys.insert(key.clone());
                let reference = &config.environment.0;
                let valid = loader.asset_index().resolve(reference).is_ok_and(|r| {
                    r.asset_id == reference.id
                        && reference.guid.as_deref() == Some(r.asset_guid.as_str())
                        && r.asset_type == "environment"
                });
                if !valid {
                    error.get_or_insert(super::invalid("environment.asset_identity"));
                    continue;
                }
                if !self.environment_requests.contains_key(&key) {
                    match loader.request_async(reference) {
                        Ok(handle) => {
                            self.environment_requests
                                .insert(key.clone(), (handle, None));
                        }
                        Err(_) => {
                            error.get_or_insert(super::invalid("environment.request"));
                            continue;
                        }
                    }
                }
                let (handle, prepared) = self.environment_requests.get_mut(&key).unwrap();
                if prepared.is_none() {
                    if let Some(payload) = loader.decoded_payload(handle) {
                        if let crate::runtime_asset::RuntimeDecodedPayload::Environment(map) =
                            payload.as_ref()
                        {
                            *prepared = Some(map.clone());
                        } else {
                            error.get_or_insert(super::invalid("environment.payload_kind"));
                        }
                    }
                }
                if let Some(map) = prepared {
                    let previous = self
                        .last
                        .as_ref()
                        .and_then(|last| last.environments.get(&camera.view.view_id))
                        .filter(|e| Arc::ptr_eq(&e.map, map) && e.intensity == config.intensity);
                    environments.insert(
                        camera.view.view_id,
                        previous.cloned().unwrap_or_else(|| {
                            Arc::new(crate::environment_lighting::EnvironmentLighting {
                                map: map.clone(),
                                intensity: config.intensity,
                            })
                        }),
                    );
                } else {
                    error.get_or_insert_with(|| {
                        ContractError::new(
                            if loader.streaming_error(handle).is_some() {
                                ContractErrorCode::MissingCookedPayload
                            } else {
                                ContractErrorCode::ResourceNotReady
                            },
                            "environment.async",
                        )
                    });
                }
            }
            let old: Vec<_> = self
                .environment_requests
                .keys()
                .filter(|k| !active_environment_keys.contains(*k))
                .cloned()
                .collect();
            for key in old {
                let (h, _) = self.environment_requests.remove(&key).unwrap();
                let _ = loader.release(&h);
            }
        } else if views.iter().any(|v| {
            v.camera
                .view
                .color_pipeline
                .as_ref()
                .is_some_and(|p| p.ibl.is_some())
        }) {
            error.get_or_insert(super::invalid("environment.runtime_package_required"));
        }
        if let Some(last) = &self.last {
            if last.environments.len() == environments.len()
                && environments.iter().all(|(id, e)| {
                    last.environments
                        .get(id)
                        .is_some_and(|old| Arc::ptr_eq(old, e))
                })
                && last.views.as_ref() == &views
                && last.error == error
                && !changed
                && !entries_changed
            {
                return last.clone();
            }
        }
        let stable = self
            .last
            .as_ref()
            .filter(|_| !entries_changed && !package_changed);
        let meshes = stable.map(|s| s.meshes.clone()).unwrap_or_else(|| {
            Arc::new(
                self.entries
                    .values()
                    .map(|entry| (entry.projection.clone(), entry.prepared.clone()))
                    .chain(self.vegetation.meshes.iter().cloned())
                    .collect(),
            )
        });
        let instances = stable.map(|s| s.instances.clone()).unwrap_or_else(|| {
            Arc::new(
                self.entries
                    .values()
                    .flat_map(|entry| entry.instances.iter().cloned())
                    .chain(self.spine_instances())
                    .chain(self.vegetation.instances.iter().cloned())
                    .collect(),
            )
        });
        self.content_version += 1;
        let identity_views = Arc::new(
            views
                .iter()
                .map(|v| crate::render_thread::RenderViewHistoryIdentity {
                    view_id: v.camera.view.view_id,
                    camera_entity_id: v.camera.view.source_camera_entity_id.clone(),
                    view_version: v.camera.view.version,
                    history_version: v.history_version,
                    origin_bits: v.camera.origin.map(f64::to_bits),
                })
                .collect(),
        );
        // Sort the frozen closure once so each live lease validation can use
        // the resource owner's bounded ordered-table walk, not per-key searches.
        let mut resource_handles = self.published_resource_handles();
        #[cfg(feature = "spine")]
        resource_handles.extend(self.spine.handles.iter().copied());
        resource_handles.sort_unstable();
        resource_handles.dedup();
        // Share a frozen closure only while it is exactly the same and the
        // original owner still validates its current generations and lease.
        // A changed closure must not keep the old whole-frame lease alive.
        let resource_lease = self
            .last
            .as_ref()
            .filter(|old| {
                old.resource_handles.as_ref() == &resource_handles
                    && manager.native_leases_valid(&resource_handles, &old._resource_lease)
            })
            .map(|old| old._resource_lease.clone())
            .unwrap_or_else(|| manager.lease_resources(resource_handles.iter().copied()));
        let snapshot = Arc::new(ProductionGeometrySnapshot {
            identity_views,
            environments,
            view_targets: Default::default(),
            worksets: self
                .last
                .as_ref()
                .filter(|old| {
                    Arc::ptr_eq(&old.instances, &instances)
                        && Arc::ptr_eq(&old._resource_lease, &resource_lease)
                })
                .map(|old| old.worksets.clone())
                .unwrap_or_default(),
            _resource_lease: resource_lease,
            resource_handles: Arc::new(resource_handles),
            built: Default::default(),
            instance_reuse: self.instance_reuse.clone(),
            meshes,
            instances,
            content_version: self.content_version,
            updated_instances,
            views: Arc::new(views),
            textures: self.frame_textures(),
            error,
        });
        self.last = Some(snapshot.clone());
        snapshot
    }

    fn spine_instances(&self) -> impl Iterator<Item = Arc<RetainedInstance>> + '_ {
        #[cfg(feature = "spine")]
        {
            self.spine.instances.iter().cloned()
        }
        #[cfg(not(feature = "spine"))]
        {
            std::iter::empty()
        }
    }
    fn frame_textures(&self) -> Arc<Vec<PreparedTextureAsset>> {
        let mut extra = self.vegetation.textures.iter();
        if self.vegetation.textures.is_empty() {
            #[cfg(not(feature = "spine"))]
            return self.textures.clone();
            #[cfg(feature = "spine")]
            if self.spine.textures.is_empty() {
                return self.textures.clone();
            }
        }
        let mut seen = HashSet::new();
        let textures = self.textures.iter().chain(&mut extra);
        #[cfg(feature = "spine")]
        let textures = textures.chain(self.spine.textures.iter());
        Arc::new(
            textures
                .filter(|t| seen.insert(t.handle))
                .cloned()
                .collect(),
        )
    }
}

// Rendering and explicit spatial observations use the same destination mapping.
fn output_view(
    camera: &crate::render_projection_adapter::CameraProjection,
    destination: &RenderTarget,
    base_extent: [u32; 2],
) -> Result<
    Option<(
        crate::render_contract::RenderView,
        crate::render_contract::CameraPose,
    )>,
    ContractError,
> {
    let extent = [destination.width, destination.height];
    let mut view = camera.view.clone();
    let [x, y, w, h] = view.viewport;
    view.viewport = [
        (u64::from(x) * u64::from(extent[0]) / u64::from(base_extent[0])) as u32,
        (u64::from(y) * u64::from(extent[1]) / u64::from(base_extent[1])) as u32,
        (u64::from(w) * u64::from(extent[0]) / u64::from(base_extent[0])) as u32,
        (u64::from(h) * u64::from(extent[1]) / u64::from(base_extent[1])) as u32,
    ];
    if view.viewport[2..].contains(&0) {
        return Ok(None);
    }
    view.target = match destination.target_kind {
        crate::runtime_renderer::RuntimeRenderTargetKind::Surface => {
            crate::render_contract::TargetKind::Window
        }
        _ => crate::render_contract::TargetKind::ViewportTexture,
    };
    let mut pose = camera.pose;
    pose.position = relative_position(pose.position, camera.coordinate_origin, camera.origin)?;
    Ok(Some((view, pose)))
}

pub(super) fn relative_position(
    local: [f32; 3],
    source_origin: [f64; 3],
    view_origin: [f64; 3],
) -> Result<[f32; 3], ContractError> {
    use crate::spatial_contract::{CoordinateDomain, WorldPosition};
    let result = (|| {
        WorldPosition::new(source_origin, local)?.relative_to(
            WorldPosition::new(view_origin, [0.; 3])?,
            CoordinateDomain::new(1e12, 32768., 32768., 0.005)?,
        )
    })();
    result.map_err(|_| {
        ContractError::new(
            ContractErrorCode::InvalidProjection,
            "view.coordinateOrigin",
        )
    })
}

#[cfg(test)]
mod output_binding_tests {
    use super::*;
    use crate::render_geometry::commands;
    #[test]
    fn spatial_observation_matches_the_actual_resized_surface_view() {
        use crate::render_contract::{DiagnosticLevel, MaterialBlend, Projection};
        use crate::render_state::{
            RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
        };
        let (manager, mut snapshot) = mesh_snapshot(MaterialBlend::Opaque);
        let camera = &mut Arc::make_mut(&mut snapshot.views)[0].camera;
        camera.view.viewport = [0, 0, 1920, 1080];
        camera.view.projection = Projection::Orthographic {
            size: 220.,
            near: 0.1,
            far: 2000.,
        };
        let main_view_id = camera.view.view_id;
        let mut scene = RenderSceneState::new();
        let mut state = RenderViewState::new(
            RenderViewId(camera.view.view_id),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        );
        state.spatial = Some(camera.clone());
        scene.register_view(state);
        let target = RenderTarget::surface("surface", 1765, 993);
        let expected = snapshot.build(&target, &manager).unwrap();
        let observed = snapshot
            .observe(
                1,
                super::super::observation::SpatialObservationContext {
                    world: &crate::world::World::new(),
                    scene: &scene,
                    target: Some(&target),
                },
                &manager,
                &[],
                DiagnosticLevel::Summary,
            )
            .unwrap();
        // This fixture isolates view derivation; World/entity correspondence
        // is covered by the existing spatial observation consumers.
        let expected_view = &expected
            .views
            .iter()
            .find(|v| v.view.view_id == main_view_id)
            .unwrap()
            .view;
        assert_eq!(observed.view.as_ref(), Some(expected_view));
    }
    #[test]
    fn independent_singleton_publication_matches_scalar_order_without_batch_wrappers() {
        use crate::render_contract::MaterialBlend;
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            for count in [32, 4096] {
                let (manager, mut snapshot) = mesh_snapshot(blend);
                let base = snapshot.instances[0].clone();
                snapshot.instances = Arc::new(
                    (0..count)
                        .map(|i| {
                            let mut row = base.as_ref().clone();
                            row.instance_id = i as u64 + 1;
                            let source = &mut Arc::make_mut(&mut row.prepared).geometry[0];
                            Arc::make_mut(&mut source.material_binding).binding_id =
                                format!("independent-{i}");
                            let world = &mut row.projection.mesh_mut().world.world_matrix;
                            world[12] = (i % 16) as f32 * 0.01;
                            world[14] -= (i / 16) as f32 * 0.01;
                            Arc::new(row)
                        })
                        .collect(),
                );
                Arc::make_mut(&mut snapshot.views)[0].camera.lights = Arc::new(Vec::new());
                let target = RenderTarget::headless_texture("main", 64, 64);
                commands::BATCH_CONSTRUCTIONS.store(0, std::sync::atomic::Ordering::Relaxed);
                let frame = snapshot.build_uncached(&target, &manager).unwrap();
                assert_eq!(commands::BATCH_CONSTRUCTIONS.load(std::sync::atomic::Ordering::Relaxed),
                    0, "{blend:?}/{count}: scalar publication still constructs temporary batch objects");
                let flatten = |frame: &GeometryFrame| {
                    frame
                        .views
                        .iter()
                        .flat_map(|v| v.selected_draws().map(|d| d.into_owned()))
                        .collect::<Vec<_>>()
                };
                assert_eq!(flatten(&frame), flatten(&legacy_frame(&snapshot, &manager)));
                assert_eq!(frame.views[0].draws.len(), count);
                assert!(
                    frame.views[0].batches.is_none(),
                    "{blend:?}/{count}: independent draws still allocate one batch wrapper each"
                );
                assert!(frame.views[0].draws.iter().all(|d| d.command.is_some()));
                let compile = |frame: &GeometryFrame| {
                    let mut graph = crate::render_graph::RenderGraph::new("singleton", 1);
                    graph.output_target = Some(frame.target.clone());
                    graph
                        .resources
                        .push(crate::render_graph::RenderResource::surface_backbuffer(
                            frame.target.clone(),
                            64,
                            64,
                        ));
                    frame.append_to_graph(&mut graph);
                    crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph)
                };
                assert!(
                    !compile(&frame).has_errors(),
                    "valid scalar publication failed graph/RHI composition"
                );
                let mut malformed = frame.clone();
                Arc::make_mut(malformed.views[0].draws.last_mut().unwrap()).camera_depth =
                    f32::NAN.into();
                assert!(
                    compile(&malformed).has_errors(),
                    "scalar publication hid a COW draw mutation"
                );
                Arc::make_mut(&mut snapshot.views)[0].camera.pose.position[0] = 0.125;
                let moved = snapshot.build_uncached(&target, &manager).unwrap();
                assert_eq!(flatten(&moved), flatten(&legacy_frame(&snapshot, &manager)));
            }
        }
    }
    #[test]
    fn prepared_material_variants_share_exact_mesh_backing_without_pinning_or_aliasing_changes() {
        use crate::render_contract::MaterialBlend;
        let (_, source, _, _) = super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let mut first = PreparedMeshRenderer {
            geometry: vec![source.clone()],
            textures: vec![],
            retained_resources: vec![],
        };
        let mut second = first.clone();
        // Model separate from_assets preparations, not an already shared clone.
        second.geometry[0].buffers = Arc::new(
            serde_json::from_value(serde_json::to_value(&source.buffers).unwrap()).unwrap(),
        );
        assert!(!Arc::ptr_eq(
            &first.geometry[0].buffers,
            &second.geometry[0].buffers
        ));
        let mut second_material = second.geometry[0].material_binding.as_ref().clone();
        second_material.binding_id.push_str("-variant");
        let second_material = Arc::new(second_material);
        second.geometry[0].material_binding = second_material.clone();
        let mut cache = ProductionGeometryCache::default();
        ProductionGeometryCache::share_mesh_backings(&mut cache.mesh_backings, &mut first, None);
        ProductionGeometryCache::share_mesh_backings(&mut cache.mesh_backings, &mut second, None);
        assert!(
            Arc::ptr_eq(&first.geometry[0].buffers, &second.geometry[0].buffers),
            "same immutable mesh was copied for a material variant"
        );
        assert!(Arc::ptr_eq(
            &second.geometry[0].material_binding,
            &second_material
        ));
        let old = Arc::downgrade(&first.geometry[0].buffers);
        let old_bytes = first.geometry[0].buffers.vertex_bytes().to_vec();
        let mut changed = first.clone();
        let mut wire = serde_json::to_value(&source.buffers).unwrap();
        let x = f32::from_le_bytes(old_bytes[..4].try_into().unwrap()) + 0.125;
        for (i, byte) in x.to_le_bytes().into_iter().enumerate() {
            wire["vertexBytes"][i] = byte.into();
        }
        changed.geometry[0].buffers = Arc::new(serde_json::from_value(wire).unwrap());
        ProductionGeometryCache::share_mesh_backings(&mut cache.mesh_backings, &mut changed, None);
        assert!(
            !Arc::ptr_eq(&first.geometry[0].buffers, &changed.geometry[0].buffers),
            "typed handle alone hid conflicting bytes"
        );
        assert_eq!(first.geometry[0].buffers.vertex_bytes(), old_bytes);
        drop(source);
        drop(first);
        drop(second);
        assert!(
            old.upgrade().is_none(),
            "prepare memo pins old mesh storage"
        );
        let current = Arc::downgrade(&changed.geometry[0].buffers);
        drop(changed);
        assert!(
            current.upgrade().is_none(),
            "prepare memo pins current mesh storage"
        );
        cache
            .mesh_backings
            .retain(|_, weak| weak.strong_count() != 0);
        assert!(cache.mesh_backings.is_empty());
    }
    #[test]
    fn retained_templates_outlive_dynamic_worksets_without_pinning_old_leases() {
        for count in [4, 32, 256] {
            let (manager, mut snapshot) =
                mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
            let target = RenderTarget::headless_texture("surface-main", 64, 64);
            let frame = snapshot.build_uncached(&target, &manager).unwrap();
            let first = frame
                .views
                .iter()
                .flat_map(|v| &v.draws)
                .find(|d| !d.view.is_shadow_map())
                .unwrap();
            let command = first.command.as_ref().unwrap().clone();
            let facts = first.retained_requirements("Rgba8Unorm").unwrap();
            let old_lease = Arc::downgrade(&snapshot._resource_lease);
            super::super::retained_commands::COMMANDS_BUILT.with(|n| n.set(0));
            for index in 0..count {
                // Actual Production rebuild: move an instance, replace the
                // immutable table and frame lease, and release the old workset.
                let instances = Arc::make_mut(&mut snapshot.instances);
                let item = Arc::make_mut(&mut instances[0]);
                item.projection.mesh_mut().world.world_matrix[12] = index as f32 * 0.001;
                snapshot._resource_lease =
                    manager.lease_resources(snapshot.resource_handles.iter().copied());
                snapshot.worksets = Default::default();
                let next = snapshot.build_uncached(&target, &manager).unwrap();
                let draw = next
                    .views
                    .iter()
                    .flat_map(|v| &v.draws)
                    .find(|d| !d.view.is_shadow_map())
                    .unwrap();
                assert!(
                    Arc::ptr_eq(draw.command.as_ref().unwrap(), &command),
                    "scale {count}, step {index}: workset rebuild discarded stable command"
                );
                assert!(Arc::ptr_eq(
                    &draw.retained_requirements("Rgba8Unorm").unwrap(),
                    &facts
                ));
            }
            assert_eq!(
                super::super::retained_commands::COMMANDS_BUILT.with(|n| n.get()),
                0,
                "scale {count}"
            );
            drop(frame);
            assert!(
                old_lease.upgrade().is_none(),
                "stable command pinned an old frame lease"
            );
        }
    }

    #[test]
    fn parallel_main_selection_matches_scalar_order_and_preserves_old_packets() {
        use crate::render_contract::MaterialBlend;
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let (manager, mut current) = mesh_snapshot(blend);
            let base = current.instances[0].clone();
            current.instances = Arc::new(
                (0..4096)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        row.visibility.authored = i % 11 != 0;
                        let world = &mut row.projection.mesh_mut().world.world_matrix;
                        world[12] = (i % 64) as f32 * 0.125 - 4.;
                        world[14] = -10. - (i / 64) as f32 * 2.;
                        let source = &mut Arc::make_mut(&mut row.prepared).geometry[0];
                        Arc::make_mut(&mut source.material_binding).binding_id =
                            format!("selection-{}", i % 7);
                        Arc::new(row)
                    })
                    .collect(),
            );
            Arc::make_mut(&mut current.views)[0].camera.lights = Arc::new(Vec::new());
            let target = RenderTarget::headless_texture("main", 64, 64);
            let old = current.build_uncached(&target, &manager).unwrap();
            let flatten = |frame: &GeometryFrame| {
                frame
                    .views
                    .iter()
                    .map(|v| {
                        (
                            v.view.clone(),
                            v.selected_draws()
                                .map(|d| d.into_owned())
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let old_values = flatten(&old);
            assert_eq!(old_values, flatten(&legacy_frame(&current, &manager)));
            let finish_threads = super::super::prepare_tasks::take_finish_threads()
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            if std::thread::available_parallelism().map_or(1, usize::from) > 1 {
                assert!(
                    finish_threads.len() > 1 && finish_threads.len() <= 4,
                    "large no-shadow publication still serial: {finish_threads:?}"
                );
            }
            let threads = super::super::prepare_tasks::take_color_threads();
            let unique = threads
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len();
            assert!(unique <= 4);
            if std::thread::available_parallelism().map_or(1, usize::from) > 1 {
                assert!(unique > 1);
            }
            Arc::make_mut(&mut current.views)[0].camera.pose.position[0] = 0.25;
            let moved = current.build_uncached(&target, &manager).unwrap();
            assert_eq!(flatten(&moved), flatten(&legacy_frame(&current, &manager)));
            assert_eq!(
                flatten(&old),
                old_values,
                "parallel selection rewrote a retained frame"
            );
        }
    }

    #[test]
    fn final_view_publication_does_not_rewrite_provisional_batch_heads() {
        use super::super::proof::HEAD_REWRITES;
        for count in [4, 32, 256] {
            let (manager, mut snapshot) =
                mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
            let base = snapshot.instances[0].clone();
            snapshot.instances = Arc::new(
                (0..count)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        let prepared = Arc::make_mut(&mut row.prepared);
                        Arc::make_mut(&mut prepared.geometry[0].material_binding).binding_id =
                            format!("material-{i}");
                        Arc::new(row)
                    })
                    .collect(),
            );
            let target = RenderTarget::headless_texture("main", 64, 64);
            snapshot.build_uncached(&target, &manager).unwrap();
            HEAD_REWRITES.with(|n| n.set(0));
            super::super::build_inputs::INSTANCE_INPUT_ROWS.with(|n| n.set(0));
            let frame = snapshot.build_uncached(&target, &manager).unwrap();
            let rewrites = HEAD_REWRITES.with(|n| n.get());
            assert_eq!(
                super::super::build_inputs::INSTANCE_INPUT_ROWS.with(|n| n.get()),
                0,
                "warm production expanded source descriptors at {count}"
            );
            eprintln!("final_publication scale={count} actual_head_rewrites={rewrites}");
            assert_eq!(
                rewrites, 0,
                "provisional batch heads reached production at {count}"
            );
            for view in &frame.views {
                for batch in view.batches.iter().flatten() {
                    batch.validate().unwrap();
                }
            }
            let reference = legacy_frame(&snapshot, &manager);
            let flatten = |frame: &GeometryFrame| {
                frame
                    .views
                    .iter()
                    .map(|v| {
                        (
                            v.view.clone(),
                            v.selected_draws()
                                .map(|d| d.into_owned())
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(flatten(&frame), flatten(&reference));
        }
    }

    #[test]
    fn shared_warm_command_attachment_does_not_take_exclusive_cache_access() {
        use super::super::build_inputs::COMMAND_WRITES;
        for count in [4, 32, 256] {
            let (manager, mut snapshot) =
                mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
            let base = snapshot.instances[0].clone();
            snapshot.instances = Arc::new(
                (0..count)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        let prepared = Arc::make_mut(&mut row.prepared);
                        Arc::make_mut(&mut prepared.geometry[0].material_binding).binding_id =
                            format!("shared-{i}");
                        Arc::new(row)
                    })
                    .collect(),
            );
            let target = RenderTarget::headless_texture("surface-main", 64, 64);
            let old = snapshot.build_uncached(&target, &manager).unwrap();
            let flatten = |frame: &super::GeometryFrame| {
                frame
                    .views
                    .iter()
                    .flat_map(|v| v.selected_draws().map(|d| d.into_owned()))
                    .collect::<Vec<_>>()
            };
            let old_wire = serde_json::to_value(flatten(&old)).unwrap();
            COMMAND_WRITES.with(|n| n.set(0));
            super::super::retained_commands::COMMAND_LOOKUPS.with(|n| n.set(0));
            let current = snapshot.build_uncached(&target, &manager).unwrap();
            let writes = COMMAND_WRITES.with(|n| n.get());
            eprintln!("shared warm commands scale={count} actual exclusive cache entries={writes}");
            assert_eq!(
                writes, 0,
                "unchanged templates required exclusive preparation"
            );
            assert_eq!(serde_json::to_value(flatten(&current)).unwrap(), old_wire);
            assert_eq!(
                super::super::retained_commands::COMMAND_LOOKUPS.with(|n| n.get()),
                0,
                "warm immutable table still searched source metadata per batch at {count}"
            );
        }
    }

    #[test]
    fn dynamic_phase_command_lock_work_is_per_view_not_per_material_batch() {
        use super::super::build_inputs::COMMAND_LOCKS;
        for count in [4, 32, 256] {
            let (manager, mut snapshot) =
                mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
            let base = snapshot.instances[0].clone();
            snapshot.instances = Arc::new(
                (0..count)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        let prepared = Arc::make_mut(&mut row.prepared);
                        Arc::make_mut(&mut prepared.geometry[0].material_binding).binding_id =
                            format!("material-{i}");
                        Arc::new(row)
                    })
                    .collect(),
            );
            let target = RenderTarget::headless_texture("surface-main", 64, 64);
            snapshot.build_uncached(&target, &manager).unwrap();
            COMMAND_LOCKS.with(|n| n.set(0));
            let frame = snapshot.build_uncached(&target, &manager).unwrap();
            let batches: usize = frame.views.iter().map(|v| v.draws.len()).sum();
            assert!(batches >= count);
            let locks = COMMAND_LOCKS.with(|n| n.get());
            eprintln!(
                "dynamic_phase scale={count} locks={locks} views={} batches={batches}",
                frame.views.len()
            );
            assert!(
                locks <= frame.views.len() + 1,
                "scale {count}: {locks} command locks for {} views / {batches} batches",
                frame.views.len()
            );
            for view in &frame.views {
                if let Some(batches) = &view.batches {
                    for batch in batches {
                        batch.validate().unwrap();
                    }
                } else {
                    for draw in &view.draws {
                        draw.validate().unwrap();
                    }
                }
            }
        }
    }

    pub(super) fn mesh_snapshot(
        blend: crate::render_contract::MaterialBlend,
    ) -> (RenderResourceManager, ProductionGeometrySnapshot) {
        use crate::render_contract::{CameraPose, IDENTITY_MATRIX};
        let (manager, source, view, light) = super::super::shadow::receiver_scene(blend, -100.);
        let mut world_matrix = IDENTITY_MATRIX;
        world_matrix[14] = -100.;
        let projection = Arc::new(MeshProjection {
            presentation: None,
            runtime_entity_id: crate::ids::RuntimeEntityId::new(1, 1),
            component: serde_json::from_value(serde_json::json!({
                "schemaVersion":"mesh-renderer3d.v1", "mesh":{"id":"mesh","type":"mesh"},
                "materials":{"surface":{"id":"material","type":"material"}},
                "visible":true,"layers":["main"]
            }))
            .unwrap(),
            world: crate::world::SpatialWorldState {
                anchor: [0.; 3],
                world_matrix,
                previous_world_matrix: world_matrix,
                pose: CameraPose::default(),
                enabled: true,
                authored_visible: true,
                inherited_visible: true,
                rigid: true,
                version: 1,
            },
            coordinate_origin: [0.; 3],
            deformation: None,
            provenance: source.geometry_source.as_ref().clone(),
        });
        let handles = source.resource_handles().collect::<Vec<_>>();
        let lease = manager.lease_resources(handles.iter().copied());
        let prepared = Arc::new(PreparedMeshRenderer {
            geometry: vec![source],
            textures: vec![],
            retained_resources: vec![],
        });
        let mut first = snapshot();
        first._resource_lease = lease;
        first.resource_handles = Arc::new(handles);
        first.meshes = Arc::new(vec![(projection.clone(), prepared.clone())]);
        first.instances = Arc::new(vec![Arc::new(RetainedInstance {
            dynamic_bounds: None,
            visibility: projection.visibility(),
            projection: InstanceProjection::Mesh(projection),
            prepared,
            submesh: 0,
            instance_id: 1,
        })]);
        let camera = &mut Arc::make_mut(&mut first.views)[0].camera;
        camera.view = view;
        camera.lights = Arc::new(vec![light]);
        (manager, first)
    }

    fn legacy_frame(
        snapshot: &ProductionGeometrySnapshot,
        manager: &RenderResourceManager,
    ) -> GeometryFrame {
        let camera = &snapshot.views[0].camera;
        let borrowed = snapshot.build_instances(camera).unwrap();
        let owned: Vec<_> = borrowed
            .iter()
            .map(|v| super::super::GeometryInstance {
                instance_id: v.instance_id,
                source: v.source,
                world_transform: v.world_transform,
                visibility: v.visibility.clone(),
                layers: crate::render_contract::LayerSelection::Named {
                    names: v.layers.to_vec(),
                },
                provenance: v.provenance.clone(),
            })
            .collect();
        let mut view = camera.view.clone();
        view.target = crate::render_contract::TargetKind::ViewportTexture;
        let mut pose = camera.pose;
        pose.position =
            relative_position(pose.position, camera.coordinate_origin, camera.origin).unwrap();
        let mut inputs = super::super::build_inputs::GeometryBuildInputs::new(
            &owned,
            manager,
            Some(&snapshot._resource_lease),
        )
        .unwrap();
        let mut frame = inputs
            .build(
                "main",
                [64, 64],
                &[GeometryViewInput { view: &view, pose }],
                crate::render_contract::DiagnosticLevel::Off,
            )
            .unwrap();
        super::super::shadow::append_scene_lighting(
            &mut frame,
            GeometryViewInput { view: &view, pose },
            camera.origin,
            &mut inputs,
            &camera.lights,
        )
        .unwrap();
        frame
    }

    #[test]
    fn parallel_shadow_preparation_matches_serial_reference_and_preserves_old_packets() {
        use crate::render_contract::MaterialBlend;
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let (manager, mut snapshot) = mesh_snapshot(blend);
            let base = snapshot.instances[0].clone();
            snapshot.instances = Arc::new(
                (0..512)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        let prepared = Arc::make_mut(&mut row.prepared);
                        Arc::make_mut(&mut prepared.geometry[0].material_binding).binding_id =
                            format!("publication-material-{}", i % 7);
                        let projection = row.projection.mesh_mut();
                        projection.world.world_matrix[12] = (i % 16) as f32 * 0.25 - 2.;
                        projection.world.world_matrix[14] -= (i / 16) as f32;
                        Arc::new(row)
                    })
                    .collect(),
            );
            let target = RenderTarget::headless_texture("main", 64, 64);
            let old = snapshot.build_uncached(&target, &manager).unwrap();
            let old_values = old
                .views
                .iter()
                .map(|v| {
                    v.selected_draws()
                        .map(|d| d.into_owned())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            Arc::make_mut(&mut snapshot.views)[0].camera.pose.position[0] += 0.25;
            super::super::prepare_tasks::take_test_threads();
            let parallel = snapshot.build_uncached(&target, &manager).unwrap();
            let task_threads = super::super::prepare_tasks::take_test_threads();
            if std::thread::available_parallelism().is_ok_and(|n| n.get() > 1) {
                assert!(
                    task_threads.len() > parallel.views.len(),
                    "large main publication remained one task: {} tasks / {} views",
                    task_threads.len(),
                    parallel.views.len()
                );
            }
            let threads = task_threads
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            if std::thread::available_parallelism().is_ok_and(|n| n.get() > 1) {
                assert!(
                    threads.len() > 1 && threads.len() <= 4,
                    "production preparation did not execute on bounded workers: {threads:?}"
                );
            }
            let serial = legacy_frame(&snapshot, &manager);
            assert_eq!(parallel.views.len(), serial.views.len());
            for (actual, expected) in parallel.views.iter().zip(&serial.views) {
                assert_eq!(actual.view, expected.view);
                assert_eq!(
                    actual.selected_draws().collect::<Vec<_>>(),
                    expected.selected_draws().collect::<Vec<_>>()
                );
            }
            assert_eq!(
                old_values,
                old.views
                    .iter()
                    .map(|v| v
                        .selected_draws()
                        .map(|d| d.into_owned())
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn retained_commands_cross_production_graph_and_rhi_without_reinterpretation() {
        use super::super::retained_commands::{COMMANDS_BUILT, INTERPRETATIONS};
        use crate::render_contract::MaterialBlend;
        for count in [4, 32, 256] {
            let (manager, mut snapshot) = mesh_snapshot(MaterialBlend::Opaque);
            let base = snapshot.instances[0].clone();
            snapshot.instances = Arc::new(
                (0..count)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        Arc::new(row)
                    })
                    .collect(),
            );
            let target = RenderTarget::headless_texture("surface-main", 64, 64);
            let first = snapshot.build_uncached(&target, &manager).unwrap();
            for draw in first.views.iter().flat_map(|v| &v.draws) {
                assert!(draw.command.is_some());
                draw.pipeline_key("Rgba8Unorm");
                draw.pipeline_key("Rgba16Float");
            }
            Arc::make_mut(&mut snapshot.views)[0].camera.pose.position[0] += 0.25;
            COMMANDS_BUILT.with(|n| n.set(0));
            INTERPRETATIONS.with(|n| n.set(0));
            let next = snapshot.build_uncached(&target, &manager).unwrap();
            let graph = super::super::commands::tests::batch_test_graph(&next);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let mut observed = 0;
            for draw in plan.commands.iter().filter_map(|c| c.geometry_draw()) {
                let source = next
                    .views
                    .iter()
                    .flat_map(|v| &v.draws)
                    .find(|d| Arc::ptr_eq(d, draw))
                    .unwrap();
                assert!(Arc::ptr_eq(
                    source.command.as_ref().unwrap(),
                    draw.command.as_ref().unwrap()
                ));
                for format in ["Rgba8Unorm", "Rgba16Float"] {
                    assert_eq!(
                        draw.pipeline_key(format),
                        draw.pipeline_key_uncached(format)
                    );
                }
                observed += 1;
            }
            assert!(observed >= 2, "color and shadow consumer missing");
            assert_eq!(
                COMMANDS_BUILT.with(|n| n.get()),
                0,
                "stable commands rebuilt for {count}"
            );
            assert_eq!(
                INTERPRETATIONS.with(|n| n.get()),
                0,
                "command facts reinterpreted for {count}"
            );
        }
    }

    #[test]
    fn moving_views_reuse_complete_static_compatibility_proof() {
        let (manager, mut snapshot) = mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
        let target = RenderTarget::headless_texture("main", 64, 64);
        let old = snapshot.build_uncached(&target, &manager).unwrap();
        Arc::make_mut(&mut snapshot.views)[0].camera.pose.position[0] += 0.25;
        super::super::commands::STATIC_COMPATIBILITY_CHECKS.with(|n| n.set(0));
        crate::render_contract::RenderBindingSet::take_validation_count_for_test();
        let current = snapshot.build_uncached(&target, &manager).unwrap();
        let checks = super::super::commands::STATIC_COMPATIBILITY_CHECKS.with(|n| n.get());
        assert_eq!(
            checks, 0,
            "new views reinterpreted unchanged complete static table"
        );
        assert_eq!(
            crate::render_contract::RenderBindingSet::take_validation_count_for_test(),
            0,
            "refreshed shared heads repeated proven static binding validation"
        );
        let reference = legacy_frame(&snapshot, &manager);
        for (actual, expected) in current.views.iter().zip(&reference.views) {
            assert_eq!(
                actual.selected_draws().collect::<Vec<_>>(),
                expected.selected_draws().collect::<Vec<_>>()
            );
        }
        assert!(!old.views.is_empty());
    }

    #[test]
    fn persistent_candidate_selection_borrows_hot_bounds_and_visibility() {
        let (manager, snapshot) = mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
        let workset = snapshot
            .prepared_workset(&snapshot.views[0].camera, &manager)
            .unwrap();
        let inputs = super::super::build_inputs::GeometryBuildInputs::retained(
            &workset.instances,
            &workset.inputs,
            &manager,
            &snapshot._resource_lease,
        )
        .unwrap();
        let candidate = inputs.candidate(0);
        assert!(
            workset.inputs.candidate_is_hot(0, &candidate),
            "selection still chases immutable cold Prepare/Projection metadata"
        );
    }

    #[test]
    fn persistent_worksets_match_flat_reference_under_camera_content_origin_and_visibility_changes()
    {
        use crate::render_contract::{LayerSelection, MaterialBlend, MaterialParameter};
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let (manager, mut current) = mesh_snapshot(blend);
            let base = &current.instances[0];
            current.instances = Arc::new(
                (0..48)
                    .map(|i| {
                        let mut row = base.as_ref().clone();
                        row.instance_id = (i * 197 % 97) as u64;
                        let projection = row.projection.mesh_mut();
                        projection.world.world_matrix[0] = if i % 3 == 0 { -1. } else { 1. };
                        projection.world.world_matrix[4] = if i % 5 == 0 { 0.25 } else { 0. };
                        projection.world.world_matrix[12] =
                            if i % 4 == 0 { 1000. } else { i as f32 - 24. };
                        projection.world.world_matrix[14] = -70. - i as f32;
                        projection.component.visible = i % 7 != 0;
                        projection.world.inherited_visible = i % 11 != 0;
                        projection.component.layers =
                            vec![if i % 6 == 0 { "other" } else { "main" }.into()];
                        row.visibility = projection.visibility();
                        row
                    })
                    .map(Arc::new)
                    .collect(),
            );
            Arc::make_mut(&mut current.views)[0].camera.view.layers = LayerSelection::Named {
                names: vec!["main".into()],
            };
            let target = RenderTarget::headless_texture("main", 64, 64);
            let mut retained = Vec::new();
            let mut original = None;
            for change in 0..6 {
                if change == 1 {
                    Arc::make_mut(&mut current.views)[0].camera.pose.position[0] = 2.;
                }
                if (2..=4).contains(&change) {
                    let row = Arc::make_mut(&mut Arc::make_mut(&mut current.instances)[1]);
                    if change == 2 {
                        row.projection.mesh_mut().world.world_matrix[12] += 3.;
                    }
                    if change == 3 {
                        Arc::make_mut(&mut row.prepared).geometry[0]
                            .material
                            .parameters
                            .insert("roughness".into(), MaterialParameter::Float(0.9));
                    }
                    if change == 4 {
                        row.projection.mesh_mut().component.visible = false;
                        row.visibility = row.projection.visibility();
                    }
                }
                if change == 5 {
                    Arc::make_mut(&mut current.views)[0].camera.origin = [128., 0., 0.];
                }
                let actual = current.build_uncached(&target, &manager).unwrap();
                let reference = legacy_frame(&current, &manager);
                assert_eq!(actual.views.len(), reference.views.len());
                for (a, b) in actual.views.iter().zip(&reference.views) {
                    assert_eq!(a.view, b.view, "blend={blend:?} change={change}");
                    assert_eq!(
                        a.selected_draws().collect::<Vec<_>>(),
                        b.selected_draws().collect::<Vec<_>>(),
                        "blend={blend:?} change={change} view={}",
                        a.view.view_id
                    );
                }
                assert!(actual.views.last().unwrap().selected_instances().count() > 0);
                if change == 0 {
                    original = Some(
                        actual
                            .views
                            .last()
                            .unwrap()
                            .selected_draws()
                            .map(|d| d.into_owned())
                            .collect::<Vec<_>>(),
                    );
                }
                retained.push(actual);
            }
            // A changed source table and material never mutate an older packet.
            assert_eq!(
                retained[0]
                    .views
                    .last()
                    .unwrap()
                    .selected_draws()
                    .map(|d| d.into_owned())
                    .collect::<Vec<_>>(),
                original.unwrap()
            );
            for frame in retained {
                for view in &frame.views {
                    for draw in view.selected_draws() {
                        draw.validate().unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn persistent_worksets_bound_origins_and_do_not_pin_retired_packets() {
        let (manager, first) = mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
        let mut camera = first.views[0].camera.clone();
        for origin in 0..18 {
            camera.origin = [origin as f64, 0., 0.];
            first.prepared_workset(&camera, &manager).unwrap();
        }
        assert_eq!(first.worksets.lock().unwrap().len(), 16);
        let overflow = first.prepared_workset(&camera, &manager).unwrap();
        let again = first.prepared_workset(&camera, &manager).unwrap();
        assert!(
            !Arc::ptr_eq(&overflow, &again),
            "origin cache overflow changed admission or retained an unbounded entry"
        );
        drop(overflow);
        drop(again);
        let frame = first
            .build(&RenderTarget::headless_texture("main", 64, 64), &manager)
            .unwrap();
        let workset = first
            .prepared_workset(&first.views[0].camera, &manager)
            .unwrap();
        let old_workset = Arc::downgrade(&workset);
        let old_instance = Arc::downgrade(&workset.inputs.table[0]);
        drop(workset);
        drop(first);
        assert!(
            old_workset.upgrade().is_none(),
            "global state retained a snapshot workset"
        );
        assert!(
            old_instance.upgrade().is_some(),
            "old packet lost its instance before retirement"
        );
        drop(frame);
        assert!(
            old_instance.upgrade().is_none(),
            "workset/reuse state pinned a retired packet"
        );
    }

    #[test]
    fn moving_production_snapshot_consumes_shared_instances_and_current_lease() {
        let (mut manager, first) = mesh_snapshot(crate::render_contract::MaterialBlend::Opaque);
        let inputs = first.build_instances(&first.views[0].camera).unwrap();
        let retained = &first.instances[0].projection;
        assert!(
            std::ptr::eq(inputs[0].provenance, retained.provenance()),
            "production input copied immutable provenance"
        );
        assert!(
            std::ptr::eq(inputs[0].layers, retained.layers()),
            "production input copied immutable layer backing"
        );
        drop(inputs);
        let target = RenderTarget::headless_texture("main", 64, 64);
        let a = first.build(&target, &manager).unwrap();
        let mut second = first.clone();
        second.built = Default::default();
        Arc::make_mut(&mut second.views)[0].camera.pose.position[0] = 1.;
        super::super::BUILD_SOURCE_VALIDATIONS.with(|n| n.set(0));
        super::super::commands::COMPACT_STATE_RESOLUTIONS.with(|n| n.set(0));
        let b = second.build(&target, &manager).unwrap();
        assert_eq!(
            (
                super::super::BUILD_SOURCE_VALIDATIONS.with(|n| n.get()),
                super::super::commands::COMPACT_STATE_RESOLUTIONS.with(|n| n.get()),
            ),
            (0, 0),
            "camera-only motion rebuilt immutable input validation or phase membership"
        );
        assert!(!Arc::ptr_eq(&a, &b));
        assert_ne!(a.views.last().unwrap().view, b.views.last().unwrap().view);
        assert!(
            Arc::ptr_eq(
                &a.views.last().unwrap().draws[0].instance,
                &b.views.last().unwrap().draws[0].instance
            ),
            "production build did not consume cross-snapshot immutable reuse"
        );
        assert!(b.views[1..4].iter().all(|v| v.draws.is_empty()));

        let mut rebased = first.clone();
        rebased.built = Default::default();
        Arc::make_mut(&mut rebased.views)[0].camera.origin = [128., 0., 0.];
        let c = rebased.build(&target, &manager).unwrap();
        assert_eq!(
            c.views.last().unwrap().draws[0].world_transform[12].to_f32(),
            -128.
        );
        assert_eq!(
            a.views.last().unwrap().draws[0].world_transform[12].to_f32(),
            0.
        );
        assert!(!Arc::ptr_eq(
            &a.views.last().unwrap().draws[0].instance,
            &c.views.last().unwrap().draws[0].instance
        ));

        // Exercise the real snapshot publication branch with a nonempty
        // resource closure. Camera changes may share its lease; unload may not.
        let mut scene = RenderSceneState::new();
        let mut moved = crate::render_state::RenderViewState::new(
            crate::render_state::RenderViewId(1),
            crate::render_state::RenderViewKind::Game,
            crate::render_state::RenderTargetKind::Window,
        );
        moved.spatial = Some(second.views[0].camera.clone());
        scene.register_view(moved.clone());
        let mut cache = ProductionGeometryCache {
            package_identity: "lease-test".into(),
            last: Some(Arc::new(first.clone())),
            last_proxy_identity: Some(scene.proxy_identity()),
            owned_handles: first.resource_handles.iter().copied().collect(),
            textures: first.textures.clone(),
            instance_reuse: first.instance_reuse.clone(),
            ..Default::default()
        };
        let shared = cache.snapshot(2, &scene, "lease-test", None, &mut manager, None);
        assert!(Arc::ptr_eq(&shared._resource_lease, &first._resource_lease));
        assert!(Arc::ptr_eq(&shared.instance_reuse, &first.instance_reuse));
        let removed = *cache.owned_handles.iter().next().unwrap();
        cache.owned_handles.remove(&removed);
        moved.spatial.as_mut().unwrap().pose.position[0] = 2.;
        scene.register_view(moved);
        let unloaded = cache.snapshot(3, &scene, "lease-test", None, &mut manager, None);
        assert!(!Arc::ptr_eq(
            &unloaded._resource_lease,
            &shared._resource_lease
        ));
        assert_eq!(
            unloaded.resource_handles.len() + 1,
            shared.resource_handles.len()
        );
        assert!(
            shared.resource_handles.contains(&removed),
            "old snapshot closure mutated"
        );
        manager.mark_device_lost(1);
        assert!(
            first.build(&target, &manager).is_err(),
            "cached frame bypassed current lease validation"
        );
        second.built = Default::default();
        assert!(
            second.build(&target, &manager).is_err(),
            "new snapshot bypassed current lease validation"
        );
    }
    #[test]
    fn ibl_same_package_patch_refreshes_snapshot_and_unmount_cannot_reuse_ready_backing() {
        use crate::render_state::*;
        use crate::runtime_asset::*;
        let root =
            std::env::temp_dir().join(format!("aife-371-ibl-revision-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let index = |version: u32| {
            let mut words = vec![0u32; 32 + 90 + 2048];
            words[..4].copy_from_slice(&[crate::environment_lighting::ENVIRONMENT_MAGIC, 2, 2, 32]);
            words[32] = (version as f32).to_bits();
            let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
            let path = format!("environment-{version}.ibl");
            std::fs::write(root.join(&path), &bytes).unwrap();
            let hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
            RuntimeAssetIndex::new(
                vec![RuntimeAssetRecord {
                    asset_guid: "guid-studio".into(),
                    asset_id: "studio".into(),
                    asset_type: "environment".into(),
                    sub_asset_id: None,
                    version: version.to_string(),
                    cooked_asset_id: format!("cooked-{version}"),
                    bundle_id: "startup".into(),
                    loader_kind: "environment".into(),
                    dependencies: vec![],
                    hash: hash.clone(),
                    size: Some(bytes.len() as u64),
                    flags: vec![],
                    source_map_debug: None,
                }],
                vec![CookedAssetRecord {
                    cooked_asset_id: format!("cooked-{version}"),
                    bundle_id: "startup".into(),
                    path: Some(path),
                    offset: None,
                    size: Some(bytes.len() as u64),
                    compression: None,
                    hash,
                }],
            )
        };
        let mut loader = RuntimeAssetLoader::new(
            &root,
            index(1),
            RuntimePackageMountTable::new(vec![BundleRecord {
                bundle_id: "startup".into(),
                mount_id: None,
                uri: "startup".into(),
                hash: None,
                version: None,
                mounted: true,
            }]),
        );
        let mut scene = RenderSceneState::new();
        let mut camera = snapshot().views[0].camera.clone();
        camera.view.color_pipeline=Some(serde_json::from_value(serde_json::json!({"schemaVersion":"view-color3d.v1","exposureEv":0,"toneMapping":"acesFitted","antiAliasing":"none","ambientDiffuse":[0,0,0],"ibl":{"environment":{"id":"studio","type":"environment","guid":"guid-studio"},"intensity":1}})).unwrap());
        let mut view = RenderViewState::new(
            RenderViewId(2),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        );
        view.spatial = Some(camera);
        scene.register_view(view);
        let mut cache = ProductionGeometryCache::default();
        let mut manager = RenderResourceManager::new();
        let wait = |cache: &mut ProductionGeometryCache,
                    loader: &mut RuntimeAssetLoader,
                    manager: &mut RenderResourceManager| {
            let frozen = cache.snapshot(1, &scene, "same-package", Some(loader), manager, None);
            assert_eq!(
                frozen.error.unwrap().code,
                ContractErrorCode::ResourceNotReady
            );
            let start = crate::platform_time::Instant::now();
            loop {
                let s = cache.retry_pending_resources(loader).unwrap();
                assert!(
                    Arc::ptr_eq(&s.views, &frozen.views),
                    "resource retry advanced camera history"
                );
                assert!(Arc::ptr_eq(&s.identity_views, &frozen.identity_views));
                assert!(Arc::ptr_eq(&s.instances, &frozen.instances));
                assert!(Arc::ptr_eq(&s.meshes, &frozen.meshes));
                assert!(Arc::ptr_eq(&s.resource_handles, &frozen.resource_handles));
                assert_eq!(
                    frozen.error.unwrap().code,
                    ContractErrorCode::ResourceNotReady,
                    "resource completion mutated the original packet"
                );
                if s.error.is_none() {
                    assert!(
                        Arc::ptr_eq(&s, &cache.retry_pending_resources(loader).unwrap()),
                        "ready retry must be a no-op"
                    );
                    break s;
                }
                assert!(
                    start.elapsed() < std::time::Duration::from_secs(3),
                    "{:?}",
                    s.error
                );
                std::thread::yield_now();
            }
        };
        let first = wait(&mut cache, &mut loader, &mut manager);
        let original = first.environments[&2].map.clone();
        loader.mount_patch_index(index(2));
        let patched = wait(&mut cache, &mut loader, &mut manager);
        assert_ne!(
            original.identity(),
            patched.environments[&2].map.identity(),
            "ready environment must not survive an index patch under the same package/ref key"
        );
        assert_eq!(
            f32::from_bits(original.words()[32]),
            1.,
            "old immutable snapshot changed"
        );
        loader.mount_patch_index(index(3));
        let pending = cache.snapshot(
            2,
            &scene,
            "same-package",
            Some(&mut loader),
            &mut manager,
            None,
        );
        assert_eq!(
            pending.error.unwrap().code,
            ContractErrorCode::ResourceNotReady
        );
        loader.mount_patch_index(index(4));
        let changed = cache.retry_pending_resources(&mut loader).unwrap();
        assert_ne!(
            changed.error.unwrap().code,
            ContractErrorCode::ResourceNotReady,
            "frozen frame adopted a new index generation"
        );
        assert_eq!(
            pending.error.unwrap().code,
            ContractErrorCode::ResourceNotReady
        );
        assert!(
            Arc::ptr_eq(
                &changed,
                &cache.retry_pending_resources(&mut loader).unwrap()
            ),
            "terminal failure was retried"
        );
        loader.mount_patch_index(index(5));
        let pending = cache.snapshot(
            3,
            &scene,
            "same-package",
            Some(&mut loader),
            &mut manager,
            None,
        );
        assert_eq!(
            pending.error.unwrap().code,
            ContractErrorCode::ResourceNotReady
        );
        let handle = cache
            .environment_requests
            .values()
            .next()
            .unwrap()
            .0
            .clone();
        loader.release(&handle).unwrap();
        let released = cache.retry_pending_resources(&mut loader).unwrap();
        assert_ne!(
            released.error.unwrap().code,
            ContractErrorCode::ResourceNotReady,
            "frozen frame recreated a released request"
        );
        assert!(Arc::ptr_eq(&released.views, &pending.views));
        loader.unmount_bundle("startup");
        let missing = cache.snapshot(
            2,
            &scene,
            "same-package",
            Some(&mut loader),
            &mut manager,
            None,
        );
        assert!(
            missing.error.is_some(),
            "unmounted ready environment was silently reused"
        );
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn publication_final_head_is_written_once_with_environment_lights_and_shadows() {
        use crate::environment_lighting::{EnvironmentLighting, EnvironmentMap, ENVIRONMENT_MAGIC};
        use crate::render_contract::{MaterialBlend, MaterialParameter};
        let mut words = vec![0u32; 32 + 6 * (4 + 1) * 3 + 32 * 32 * 2];
        words[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
        let bytes = words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>();
        let environment = Arc::new(EnvironmentLighting {
            map: EnvironmentMap::from_bytes(&bytes).unwrap(),
            intensity: 1f32.into(),
        });
        let target = RenderTarget::headless_texture("main", 64, 64);
        let mut rows = Vec::new();
        for shadows in [false, true] {
            for count in [4, 32, 256] {
                let (manager, mut current) = mesh_snapshot(MaterialBlend::Opaque);
                let base = current.instances[0].clone();
                current.instances = Arc::new(
                    (0..count)
                        .map(|i| {
                            let mut row = base.as_ref().clone();
                            row.instance_id = i as u64 + 1;
                            let prepared = Arc::make_mut(&mut row.prepared);
                            Arc::make_mut(&mut prepared.geometry[0].material_binding).binding_id =
                                format!("publication-material-{i}");
                            prepared.geometry[0]
                                .material
                                .parameters
                                .insert("roughness".into(), MaterialParameter::Float(0.5));
                            Arc::new(row)
                        })
                        .collect(),
                );
                let camera = &mut Arc::make_mut(&mut current.views)[0].camera;
                camera.view.color_pipeline=Some(serde_json::from_value(serde_json::json!({"schemaVersion":"view-color3d.v1","exposureEv":0,"toneMapping":"acesFitted","antiAliasing":"none","ambientDiffuse":[0,0,0],"ibl":{"environment":{"id":"studio","type":"environment","guid":"guid-studio"},"intensity":1}})).unwrap());
                for light in Arc::make_mut(&mut camera.lights) {
                    light.component.cast_shadows = shadows;
                }
                let view_id = camera.view.view_id;
                current.environments.insert(view_id, environment.clone());
                super::super::proof::HEAD_REWRITES.with(|n| n.set(0));
                let actual = current.build_uncached(&target, &manager).unwrap();
                let writes = super::super::proof::HEAD_REWRITES.with(|n| n.get());
                let main = actual
                    .views
                    .iter()
                    .find(|v| v.view.view_id == view_id)
                    .unwrap();
                assert!(
                    main.batches.is_none(),
                    "independent color draws need no batch wrappers"
                );
                assert_eq!(main.draws.len(), count);
                for draw in &main.draws {
                    assert!(Arc::ptr_eq(
                        draw.environment.as_ref().unwrap(),
                        &environment
                    ));
                    assert!(draw.lighting.is_some());
                    assert_eq!(draw.shadows.is_some(), shadows);
                    draw.validate().unwrap();
                }
                let mut reference = legacy_frame(&current, &manager);
                let flat = reference
                    .views
                    .iter_mut()
                    .find(|v| v.view.view_id == view_id)
                    .unwrap();
                for draw in &mut flat.draws {
                    Arc::make_mut(draw).environment = Some(environment.clone());
                }
                assert_eq!(actual.views.len(), reference.views.len());
                for (a, b) in actual.views.iter().zip(&reference.views) {
                    assert_eq!(a.view, b.view);
                    assert_eq!(
                        a.selected_draws().collect::<Vec<_>>(),
                        b.selected_draws().collect::<Vec<_>>()
                    );
                }
                let old = main
                    .selected_draws()
                    .map(|v| v.into_owned())
                    .collect::<Vec<_>>();
                let mut next = environment.as_ref().clone();
                next.intensity = 2f32.into();
                current.environments.insert(view_id, Arc::new(next));
                let camera = &mut Arc::make_mut(&mut current.views)[0].camera;
                camera
                    .view
                    .color_pipeline
                    .as_mut()
                    .unwrap()
                    .ibl
                    .as_mut()
                    .unwrap()
                    .intensity = 2f32.into();
                camera.pose.position[0] += 0.25;
                let changed = current.build_uncached(&target, &manager).unwrap();
                assert!(changed
                    .views
                    .iter()
                    .find(|v| v.view.view_id == view_id)
                    .unwrap()
                    .draws
                    .iter()
                    .all(|d| d.environment.as_ref().unwrap().intensity == 2f32.into()));
                assert_eq!(
                    main.selected_draws()
                        .map(|v| v.into_owned())
                        .collect::<Vec<_>>(),
                    old,
                    "old immutable frame changed"
                );
                rows.push((shadows, count, writes, main.draws.len()));
            }
        }
        eprintln!("publication actual rewritten heads {rows:?}");
        assert!(
            rows.iter().all(|&(_, _, writes, _)| writes == 0),
            "final attachments rewrote a published batch head: {rows:?}"
        );
    }

    fn snapshot() -> ProductionGeometrySnapshot {
        let camera = crate::render_projection_adapter::CameraProjection {
            lights: Default::default(),
            view: crate::render_contract::RenderView::perspective(2, 64, 64, 60., 0.1, 10.),
            pose: Default::default(),
            suspended: false,
            origin: [0.; 3],
            coordinate_origin: [0.; 3],
        };
        ProductionGeometrySnapshot {
            content_version: 7,
            views: Arc::new(vec![RetainedGeometryView {
                camera: camera.clone(),
                previous_camera: camera,
                history_version: 4,
            }]),
            identity_views: Arc::new(vec![crate::render_thread::RenderViewHistoryIdentity {
                view_id: 2,
                camera_entity_id: None,
                view_version: 1,
                history_version: 4,
                origin_bits: [0; 3],
            }]),
            ..Default::default()
        }
    }
    #[test]
    fn output_bindings_freeze_packet_and_invalidate_history_without_mutating_source() {
        let source = snapshot();
        let targets = [(2, RenderTarget::viewport_texture("second", 96, 64))].into();
        let changed = source.with_view_targets(&targets, 3);
        assert_eq!(source.content_version, 7);
        assert_eq!(changed.content_version, 10);
        assert!(source.view_targets.is_empty());
        assert_eq!(changed.view_targets[&2].width, 96);
        assert_eq!(source.views[0].history_version, 4);
        assert_eq!(source.identity_views[0].history_version, 4);
        assert_eq!(changed.views[0].history_version, 7);
        assert_eq!(changed.identity_views[0].history_version, 7);
        let removed = source.with_view_targets(&BTreeMap::new(), 4);
        assert_eq!(removed.content_version, 11);
        assert!(removed.view_targets.is_empty());
        assert_eq!(removed.identity_views[0].history_version, 8);
    }
    #[test]
    fn observation_target_mapping_preserves_split_views_independent_outputs_and_origins() {
        for independent in [false, true] {
            let mut source = snapshot();
            let views = Arc::make_mut(&mut source.views);
            views[0].camera.origin = [10_000., 0., 0.];
            views[0].camera.coordinate_origin = [9_999., 0., 0.];
            views[0].camera.pose.position = [1., 0., 0.];
            let mut second = views[0].clone();
            second.camera.view.view_id = 3;
            second.camera.view.viewport = [64, 0, 64, 64];
            views.push(second);
            let target = RenderTarget::surface("main", 1765, 993);
            if independent {
                let mut destination = RenderTarget::viewport_texture("second", 96, 64);
                destination.format = target.format.clone();
                destination.color_space = target.color_space.clone();
                source = source.with_view_targets(&[(2, destination)].into(), 1);
            }
            let camera = &source.views[0].camera;
            let (mapped, extent) = source
                .observation_camera_for_target(camera, &target)
                .unwrap()
                .unwrap();
            assert_eq!(mapped.pose.position, [0.; 3]);
            assert_eq!(
                mapped.view.viewport,
                if independent {
                    [0, 0, 96, 64]
                } else {
                    [0, 0, 882, 993]
                }
            );
            let mut expected = mapped.executable().unwrap().unwrap();
            expected.target_extent = extent;
            let actual = source
                .build(&target, &RenderResourceManager::new())
                .unwrap();
            let main = actual.views.iter().find(|v| v.view.view_id == 2).unwrap();
            assert_eq!(main.view, expected);
            if !independent {
                assert!(source
                    .observation_camera_for_target(camera, &RenderTarget::surface("zero", 0, 0))
                    .unwrap()
                    .is_none());
            }
        }
    }
    #[test]
    fn empty_scene_still_produces_the_bound_camera_clear() {
        let source = snapshot();
        let bound = source.with_view_targets(
            &[(2, RenderTarget::viewport_texture("second", 96, 64))].into(),
            1,
        );
        let frame = bound
            .build(
                &RenderTarget::headless_texture("main", 64, 64),
                &RenderResourceManager::new(),
            )
            .unwrap();
        assert_eq!(
            frame.views.len(),
            1,
            "unloading the last mesh must not remove the independent camera output"
        );
        assert!(frame.views[0].draws.is_empty());
        assert_eq!(
            frame.views[0].color_target.as_ref().unwrap().target_id,
            "second"
        );
        assert_eq!(frame.views[0].view.target_extent, [96, 64]);
    }
    #[test]
    fn invalid_output_binding_is_not_silently_ignored_even_without_draws() {
        let source = snapshot();
        let manager = RenderResourceManager::new();
        let main = RenderTarget::headless_texture("main", 64, 64);
        for fault in ["unknown", "zero", "surface", "primary", "format"] {
            let mut target = RenderTarget::viewport_texture("second", 64, 64);
            let mut view = 2;
            match fault {
                "unknown" => view = 999,
                "zero" => target.width = 0,
                "surface" => {
                    target.target_kind = crate::runtime_renderer::RuntimeRenderTargetKind::Surface
                }
                "primary" => target.target_id = "main".into(),
                "format" => target.format = "Bgra8UnormSrgb".into(),
                _ => unreachable!(),
            }
            let bound = source.with_view_targets(&[(view, target)].into(), 1);
            assert!(
                bound.build(&main, &manager).is_err(),
                "ignored {fault} output binding"
            );
        }
    }
}

fn s_environment(
    e: &Arc<crate::environment_lighting::EnvironmentLighting>,
    s: &mut crate::resource_memory::ResourceMemorySample,
) {
    s.cpu_arc(e, "snapshots", &["snapshot"]);
    s.cpu_arc(&e.map, "assets", &["snapshot"]);
    e.map.observe_memory(s, "snapshot");
}

#[cfg(test)]
#[path = "production/lod_tests.rs"]
mod lod_tests;
