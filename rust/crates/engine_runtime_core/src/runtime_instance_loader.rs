use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::runtime_asset::{RuntimeAssetHandle, RuntimeAssetLoadState};
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::runtime_entity_hydration::PreparedRuntimeEntities;
use crate::runtime_instance::{
    PrefabInstantiateReport, RuntimeInstanceId, RuntimeInstanceState, RuntimePrefabInstance,
    RuntimeReportLevel, RuntimeSceneInstance, SceneInstantiateReport,
};
use crate::runtime_instance_diagnostics::{InstanceDiagnostic, InstanceStage};
use crate::runtime_package::{
    RuntimeAsset, RuntimeAssetRef, RuntimeEntity, RuntimePackage, RuntimePrefabData, RuntimeScene,
};
use crate::world::World;
use std::collections::BTreeMap;
#[cfg_attr(test, path = "runtime_instance_loader/memory.rs")]
mod memory;
#[cfg_attr(test, path = "runtime_instance_loader/project_cells.rs")]
mod project_cells;
#[cfg_attr(test, path = "runtime_instance_loader/generated_content.rs")]
mod generated_content;
#[cfg_attr(test, path = "runtime_instance_loader/project_storage.rs")]
mod project_storage;
#[cfg(feature = "real-wgpu")]
pub(crate) use generated_content::native::GeneratedContentNativeFrame;
#[cfg_attr(test, path = "runtime_instance_loader/vegetation_gameplay.rs")]
mod vegetation_gameplay;
#[cfg_attr(test, path = "runtime_instance_loader/world_persistence.rs")]
mod world_persistence;
#[cfg_attr(test, path = "runtime_instance_loader/world_streaming.rs")]
mod world_streaming;
pub use project_cells::ProjectCellSnapshot;
pub use vegetation_gameplay::VegetationPromotionFields3D;
#[cfg_attr(test, path = "runtime_instance_loader/cells.rs")]
mod cells;
pub use cells::{
    CellActivationRequest, CellDomain, CellError, CellMigrationRequest, CellStatus, CellTicket,
    SoftCellEntityRef,
};

#[derive(Debug)]
pub struct RuntimeInstanceLoader {
    cells: cells::CellState,
    project_cells: project_cells::ProjectCells,
    generated_content: generated_content::GeneratedContentState,
    project_storage: std::sync::Mutex<project_storage::StorageState>,
    world_streaming: world_streaming::WorldStreamingState,
    persistence: world_persistence::PersistenceState,
    asset_loader: RuntimeAssetLoader,
    scene_instances: BTreeMap<RuntimeInstanceId, RuntimeSceneInstance>,
    prefab_instances: BTreeMap<RuntimeInstanceId, RuntimePrefabInstance>,
    next_instance_id: u64,
    next_request_id: u64,
    report_level: RuntimeReportLevel,
}

impl RuntimeInstanceLoader {
    pub fn from_package(package: &RuntimePackage) -> Self {
        Self::new(
            RuntimeAssetLoader::new(
                package.package_dir.clone(),
                package.runtime_asset_index.clone(),
                package.runtime_asset_mount_table.clone(),
            )
            .with_package_source(package.source.clone()),
        )
    }

    pub fn new(asset_loader: RuntimeAssetLoader) -> Self {
        Self {
            asset_loader,
            cells: cells::CellState::default(),
            project_cells: project_cells::ProjectCells::default(),
            generated_content: generated_content::GeneratedContentState::default(),
            project_storage: std::sync::Mutex::new(project_storage::StorageState::default()),
            world_streaming: world_streaming::WorldStreamingState::default(),
            persistence: world_persistence::PersistenceState::default(),
            scene_instances: BTreeMap::new(),
            prefab_instances: BTreeMap::new(),
            next_instance_id: 1,
            next_request_id: 1,
            report_level: RuntimeReportLevel::Off,
        }
    }

    pub fn asset_loader(&self) -> &RuntimeAssetLoader {
        &self.asset_loader
    }

    pub fn asset_loader_mut(&mut self) -> &mut RuntimeAssetLoader {
        &mut self.asset_loader
    }

    pub fn report_level(&self) -> RuntimeReportLevel {
        self.report_level
    }

    pub fn set_report_level(&mut self, report_level: RuntimeReportLevel) {
        self.report_level = report_level;
    }

    pub fn scene_instance(&self, id: RuntimeInstanceId) -> Option<&RuntimeSceneInstance> {
        self.scene_instances.get(&id)
    }

    pub fn prefab_instance(&self, id: RuntimeInstanceId) -> Option<&RuntimePrefabInstance> {
        self.prefab_instances.get(&id)
    }

    pub fn load_active_scene_instance(
        &mut self,
        package: &RuntimePackage,
        world: &mut World,
    ) -> (Option<RuntimeSceneInstance>, SceneInstantiateReport) {
        let scene_ref = RuntimeAssetRef {
            id: package.active_scene.id.clone(),
            asset_type: "scene".to_string(),
            guid: None,
            sub_asset: None,
        };
        self.load_scene_instance(
            &package.active_scene,
            scene_ref,
            package.active_scene.id.clone(),
            world,
        )
    }

    pub fn load_scene_instance(
        &mut self,
        scene: &RuntimeScene,
        scene_ref: RuntimeAssetRef,
        scene_asset_guid: impl Into<String>,
        world: &mut World,
    ) -> (Option<RuntimeSceneInstance>, SceneInstantiateReport) {
        let request_id = self.next_request_id();
        let mut report = SceneInstantiateReport::new(request_id, scene_ref.id.clone())
            .with_report_level(self.report_level);
        let mut handles = Vec::new();

        if let Err(error) = scene.validate_spatial() {
            report.diagnostics.push(InstanceDiagnostic::error(
                error.code.clone(),
                error.to_string(),
                InstanceStage::PrepareEntities,
            ));
            return (None, report);
        }
        report.stage = InstanceStage::ResolveAssets;
        match self.asset_loader.load(&scene_ref) {
            Ok(handle) => handles.push(handle),
            Err(_) => {
                let mut diagnostic = InstanceDiagnostic::error(
                    "scene asset missing",
                    format!("scene asset could not be loaded: {}", scene_ref.id),
                    InstanceStage::ResolveAssets,
                )
                .with_suggested_fix("Check RuntimeAssetIndex and mounted bundle for scene asset.");
                // Loading the scene also loads its dependencies. Preserve the
                // current failure rather than blaming the scene or an older
                // retained failure from a previous request.
                if let Some(cause) =
                    self.asset_loader
                        .diagnostics()
                        .entries()
                        .iter()
                        .rev()
                        .find(|entry| {
                            entry.state == crate::runtime_asset_diagnostics::AssetLoadState::Failed
                        })
                {
                    let path = cause
                        .cooked_asset_id
                        .as_deref()
                        .and_then(|id| self.asset_loader.asset_index().cooked_asset(id))
                        .and_then(|asset| asset.path.as_deref());
                    diagnostic.message = format!(
                        "scene asset could not be loaded: {}; asset {} failed at {:?}: {:?}; cooked {:?}, path {:?}",
                        scene_ref.id, cause.asset_ref_id, cause.stage, cause.error_code,
                        cause.cooked_asset_id, path,
                    );
                    diagnostic.asset_guid = cause.asset_guid.clone();
                    if let Some(action) = &cause.recommended_action {
                        diagnostic.suggested_fix = Some(action.clone());
                    }
                }
                report.diagnostics.push(diagnostic);
                return (None, report);
            }
        }
        if !self.collect_entity_asset_refs(&scene.entities, &mut handles, &mut report) {
            self.release_handles(handles);
            return (None, report);
        }

        let instance_id = self.next_instance_id();
        report.instance_id = Some(instance_id);
        report.stage = InstanceStage::PrepareEntities;
        let prepared = match PreparedRuntimeEntities::prepare_scene_instance(&scene.entities, world)
        {
            Ok(prepared) => prepared,
            Err(diagnostics) => {
                report.diagnostics.extend(diagnostics);
                self.release_handles(handles);
                return (None, report);
            }
        };
        let source_to_world = prepared.source_to_world.clone();
        let root_sources = prepared.root_sources.clone();
        report.remapped_reference_count = prepared.remapped_reference_count;
        report.stage = InstanceStage::CommitEntities;
        let source_to_runtime = prepared.commit(world).source_to_runtime;
        if let Some(camera) = &scene.main_camera {
            world
                .select_main_camera(Some(SourceEntityId::from(camera.clone())))
                .expect("scene validated before commit");
        }
        report.committed = true;
        report.world_changed = !source_to_runtime.is_empty();

        let root_entities = root_sources
            .iter()
            .filter_map(|source| source_to_runtime.get(source))
            .copied()
            .collect::<Vec<_>>();
        report.stage = InstanceStage::Activate;
        report.created_entity_count = source_to_runtime.len();
        report.loaded_asset_count = handles.len();
        report.source_to_runtime_entity_debug =
            debug_entity_map_for_level(&source_to_runtime, report.report_level);

        let instance = RuntimeSceneInstance {
            instance_id,
            scene_asset_guid: scene_asset_guid.into(),
            scene_id: scene.id.clone(),
            root_entities,
            source_to_runtime_entity: source_to_runtime,
            source_to_world_entity: source_to_world,
            owned_asset_handles: handles,
            state: RuntimeInstanceState::Active,
        };
        self.scene_instances.insert(instance_id, instance.clone());
        (Some(instance), report)
    }

    pub fn instantiate_prefab_from_package(
        &mut self,
        package: &RuntimePackage,
        prefab_ref: RuntimeAssetRef,
        parent_entity: Option<SourceEntityId>,
        target_scene_instance: Option<RuntimeInstanceId>,
        world: &mut World,
    ) -> (Option<RuntimePrefabInstance>, PrefabInstantiateReport) {
        let (prepared, report) = self.prepare_prefab_from_package(
            package,
            prefab_ref,
            parent_entity,
            target_scene_instance,
            world,
        );
        match prepared {
            Some(prepared) => {
                let (instance, report) = self.commit_prepared_prefab(prepared, report, world);
                (Some(instance), report)
            }
            None => (None, report),
        }
    }

    pub(crate) fn deferred_prefabs(&mut self) -> DeferredPrefabs<'_> {
        DeferredPrefabs {
            loader: self,
            prepared: Vec::new(),
        }
    }

    pub(crate) fn prefab_instance_for_root(
        &self,
        root: RuntimeEntityId,
    ) -> Option<RuntimeInstanceId> {
        self.prefab_instances
            .values()
            .find(|v| {
                v.root_entity == Some(root)
                    && v.state == RuntimeInstanceState::Active
                    && !self.cells.owns_active(v.instance_id)
            })
            .map(|v| v.instance_id)
    }

    fn prepare_prefab_from_package(
        &mut self,
        package: &RuntimePackage,
        prefab_ref: RuntimeAssetRef,
        parent_entity: Option<SourceEntityId>,
        target_scene_instance: Option<RuntimeInstanceId>,
        world: &World,
    ) -> (Option<PreparedPrefab>, PrefabInstantiateReport) {
        let request_id = self.next_request_id();
        let mut report = PrefabInstantiateReport::new(request_id, prefab_ref.id.clone())
            .with_report_level(self.report_level);
        report.stage = InstanceStage::ResolveAssets;
        let mut handles = Vec::new();
        match self.asset_loader.load(&prefab_ref) {
            Ok(handle) => handles.push(handle),
            Err(_) => {
                report.diagnostics.push(
                    InstanceDiagnostic::error(
                        "prefab asset missing",
                        format!("prefab asset could not be loaded: {}", prefab_ref.id),
                        InstanceStage::ResolveAssets,
                    )
                    .with_suggested_fix(
                        "Check RuntimeAssetIndex and mounted bundle for prefab asset.",
                    ),
                );
                return (None, report);
            }
        }

        let Some(prefab_data) = prefab_data_from_package(package, &prefab_ref) else {
            report.diagnostics.push(
                InstanceDiagnostic::error(
                    "prefab data missing",
                    format!(
                        "prefab asset has no inline RuntimePrefabData: {}",
                        prefab_ref.id
                    ),
                    InstanceStage::ResolveAssets,
                )
                .with_suggested_fix("Cook prefab assets with runtime asset data."),
            );
            self.release_handles(handles);
            return (None, report);
        };

        if let Some(target) = target_scene_instance {
            if !self
                .scene_instances
                .get(&target)
                .is_some_and(|scene| scene.state == RuntimeInstanceState::Active)
            {
                report.diagnostics.push(
                    InstanceDiagnostic::error(
                        "world.instance.target_scene_missing",
                        format!("target scene instance is not active: {target}"),
                        InstanceStage::ValidateInput,
                    )
                    .with_suggested_fix("Use an active RuntimeSceneInstanceId or omit the target."),
                );
                self.release_handles(handles);
                return (None, report);
            }
        }

        if !self.collect_entity_asset_refs(&prefab_data.entities, &mut handles, &mut report) {
            self.release_handles(handles);
            return (None, report);
        }

        let instance_id = self.next_instance_id();
        report.instance_id = Some(instance_id);
        report.stage = InstanceStage::PrepareEntities;
        let prepared = match PreparedRuntimeEntities::prepare_prefab_instance(
            &prefab_data.entities,
            instance_id,
            prefab_data.root_entity_id.as_deref(),
            parent_entity.clone(),
            world,
        ) {
            Ok(prepared) => prepared,
            Err(diagnostics) => {
                report.diagnostics.extend(diagnostics);
                self.release_handles(handles);
                return (None, report);
            }
        };
        (
            Some(PreparedPrefab {
                prepared,
                prefab_ref,
                parent_entity,
                target_scene_instance,
                root_source: prefab_root_source(&prefab_data),
                instance_id,
                handles,
            }),
            report,
        )
    }

    fn commit_prepared_prefab(
        &mut self,
        prepared: PreparedPrefab,
        mut report: PrefabInstantiateReport,
        world: &mut World,
    ) -> (RuntimePrefabInstance, PrefabInstantiateReport) {
        let PreparedPrefab {
            prepared,
            prefab_ref,
            parent_entity,
            target_scene_instance,
            root_source,
            instance_id,
            handles,
        } = prepared;
        let source_to_world = prepared.source_to_world.clone();
        report.remapped_reference_count = prepared.remapped_reference_count;
        report.stage = InstanceStage::CommitEntities;
        let source_to_runtime = prepared.commit(world).source_to_runtime;
        report.committed = true;
        report.world_changed = !source_to_runtime.is_empty();

        let root_entity = root_source.and_then(|source| source_to_runtime.get(&source).copied());
        report.stage = InstanceStage::Activate;
        report.created_entity_count = source_to_runtime.len();
        report.loaded_asset_count = handles.len();
        report.source_to_runtime_entity_debug =
            debug_entity_map_for_level(&source_to_runtime, report.report_level);

        let instance = RuntimePrefabInstance {
            instance_id,
            prefab_asset_guid: prefab_ref.guid.clone().unwrap_or(prefab_ref.id.clone()),
            root_entity,
            parent_entity,
            target_scene_instance,
            source_to_runtime_entity: source_to_runtime,
            source_to_world_entity: source_to_world,
            owned_asset_handles: handles,
            state: RuntimeInstanceState::Active,
        };
        self.prefab_instances.insert(instance_id, instance.clone());
        (instance, report)
    }

    pub fn unload_scene_instance(
        &mut self,
        instance_id: RuntimeInstanceId,
        world: &mut World,
    ) -> SceneInstantiateReport {
        let request_id = self.next_request_id();
        let mut report =
            SceneInstantiateReport::new(request_id, format!("scene-instance:{}", instance_id))
                .with_report_level(self.report_level);
        report.stage = InstanceStage::Release;
        let Some(mut instance) = self.scene_instances.remove(&instance_id) else {
            report.diagnostics.push(
                InstanceDiagnostic::error(
                    "unload unknown scene instance",
                    format!("scene instance does not exist: {}", instance_id),
                    InstanceStage::Release,
                )
                .with_suggested_fix("Keep RuntimeSceneInstanceId returned by load_scene_instance."),
            );
            return report;
        };
        if world.main_camera().is_some_and(|camera| {
            instance
                .source_to_world_entity
                .values()
                .any(|id| id == camera)
        }) {
            world
                .select_main_camera(None)
                .expect("clearing selection is valid");
        }
        for world_entity in instance.source_to_world_entity.values().rev() {
            world.despawn_entity(world_entity);
        }
        self.release_handles(instance.owned_asset_handles.clone());
        instance.state = RuntimeInstanceState::Released;
        report.instance_id = Some(instance_id);
        report.created_entity_count = instance.source_to_world_entity.len();
        report.loaded_asset_count = instance.owned_asset_handles.len();
        report.source_to_runtime_entity_debug =
            debug_entity_map_for_level(&instance.source_to_runtime_entity, report.report_level);
        report
    }

    pub fn despawn_prefab_instance(
        &mut self,
        instance_id: RuntimeInstanceId,
        world: &mut World,
    ) -> PrefabInstantiateReport {
        if self.cells.owns_active(instance_id) {
            let mut report = PrefabInstantiateReport::new(
                self.next_request_id(),
                format!("cell:{}", instance_id),
            )
            .with_report_level(self.report_level);
            report.diagnostics.push(InstanceDiagnostic::error(
                "cell.release_owner_required",
                "Release the cell through its resource and GPU owners",
                InstanceStage::Release,
            ));
            return report;
        }
        self.despawn_prefab_instance_unchecked(instance_id, world)
    }

    fn despawn_prefab_instance_unchecked(
        &mut self,
        instance_id: RuntimeInstanceId,
        world: &mut World,
    ) -> PrefabInstantiateReport {
        let request_id = self.next_request_id();
        let mut report =
            PrefabInstantiateReport::new(request_id, format!("prefab-instance:{}", instance_id))
                .with_report_level(self.report_level);
        report.stage = InstanceStage::Release;
        let Some(mut instance) = self.prefab_instances.remove(&instance_id) else {
            report.diagnostics.push(
                InstanceDiagnostic::error(
                    "despawn unknown prefab instance",
                    format!("prefab instance does not exist: {}", instance_id),
                    InstanceStage::Release,
                )
                .with_suggested_fix("Keep RuntimePrefabInstanceId returned by instantiate_prefab."),
            );
            return report;
        };
        // Unload publishes one immutable spatial delta for the existing instance
        // transaction. Entity mutations retain their original order and visibility.
        world.with_deferred_spatial_delta(|world| {
            for world_entity in instance.source_to_world_entity.values().rev() {
                world.despawn_entity(world_entity);
            }
        });
        self.release_handles(instance.owned_asset_handles.clone());
        instance.state = RuntimeInstanceState::Released;
        report.instance_id = Some(instance_id);
        report.created_entity_count = instance.source_to_world_entity.len();
        report.loaded_asset_count = instance.owned_asset_handles.len();
        report.source_to_runtime_entity_debug =
            debug_entity_map_for_level(&instance.source_to_runtime_entity, report.report_level);
        report
    }

    fn collect_entity_asset_refs(
        &mut self,
        entities: &[RuntimeEntity],
        handles: &mut Vec<RuntimeAssetHandle>,
        report: &mut crate::runtime_instance::RuntimeInstantiateReport,
    ) -> bool {
        report.stage = InstanceStage::ResolveAssets;
        for entity in entities {
            if let Some(crate::spatial_contract::RuntimeSpatial::World3D {
                mesh_renderer: Some(mesh),
                ..
            }) = &entity.spatial
            {
                for reference in std::iter::once(&mesh.mesh).chain(mesh.materials.values()) {
                    match self.asset_loader.load(&reference.0) {
                        Ok(handle) => handles.push(handle),
                        Err(()) => {
                            report.diagnostics.push(InstanceDiagnostic::error(
                                "three_d.asset_load_failed",
                                format!("Unable to resolve {} for {}", reference.0.id, entity.id),
                                InstanceStage::ResolveAssets,
                            ));
                            return false;
                        }
                    }
                }
            }
            for component in entity
                .components
                .iter()
                .filter(|component| component.component_type == "engine.audio_source")
            {
                let source = match crate::audio::decode_audio_source(&component.data) {
                    Ok(source) => source,
                    Err(message) => {
                        report.diagnostics.push(
                            InstanceDiagnostic::error(
                                "audio.source_invalid",
                                message,
                                InstanceStage::ResolveAssets,
                            )
                            .with_source_entity_id(SourceEntityId::from(entity.id.clone()))
                            .with_suggested_fix(
                                "Fix AudioSource clipRef/volume before loading this instance.",
                            ),
                        );
                        return false;
                    }
                };
                match self.asset_loader.load(&source.clip_ref) {
                    Ok(handle) => handles.push(handle),
                    Err(()) => {
                        report.diagnostics.push(
                            InstanceDiagnostic::error(
                                "audio.asset_load_failed",
                                format!(
                                    "AudioSource on {} cannot load audio {}: {:?}",
                                    entity.id,
                                    source.clip_ref.id,
                                    self.asset_loader.diagnostics()
                                ),
                                InstanceStage::ResolveAssets,
                            )
                            .with_source_entity_id(SourceEntityId::from(entity.id.clone()))
                            .with_suggested_fix(
                                "Check the clipRef id/guid/type and cooked PCM WAV payload.",
                            ),
                        );
                        return false;
                    }
                }
            }
            for component in entity
                .components
                .iter()
                .filter(|component| component.component_type == "engine.particle_effect")
            {
                let source = match crate::runtime_particles::decode_particle_effect(&component.data)
                {
                    Ok(source) => source,
                    Err(message) => {
                        report.diagnostics.push(
                            InstanceDiagnostic::error(
                                "particle_effect.source_invalid",
                                message,
                                InstanceStage::ResolveAssets,
                            )
                            .with_source_entity_id(SourceEntityId::from(entity.id.clone()))
                            .with_suggested_fix(
                                "Fix ParticleEffect effectRef/volume before loading this instance.",
                            ),
                        );
                        return false;
                    }
                };
                match self.asset_loader.load(&source.effect_ref) {
                    Ok(handle) => handles.push(handle),
                    Err(()) => {
                        report.diagnostics.push(
                            InstanceDiagnostic::error(
                                "particle_effect.asset_load_failed",
                                format!("ParticleEffect on {} cannot load audio {}: {:?}", entity.id, source.effect_ref.id, self.asset_loader.diagnostics()),
                                InstanceStage::ResolveAssets,
                            )
                            .with_source_entity_id(SourceEntityId::from(entity.id.clone()))
                            .with_suggested_fix("Check the effectRef id/guid/type and cooked particle-effect payload."),
                        );
                        return false;
                    }
                }
            }

            if let Some(mesh) = &entity.mesh {
                for asset_ref in [
                    mesh.asset_ref.as_ref(),
                    mesh.material_ref.as_ref(),
                    mesh.texture_ref.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    match self.asset_loader.load(asset_ref) {
                        Ok(handle) => handles.push(handle),
                        Err(_) => {
                            report.diagnostics.push(
                                InstanceDiagnostic::error(
                                    "asset dependency missing",
                                    format!(
                                        "entity {} points to missing asset: {}",
                                        entity.id, asset_ref.id
                                    ),
                                    InstanceStage::ResolveAssets,
                                )
                                .with_source_entity_id(SourceEntityId::from(entity.id.clone()))
                                .with_suggested_fix(
                                    "Check RuntimeAssetIndex and bundle mount state.",
                                ),
                            );
                            return false;
                        }
                    }
                }
            }
            if let Some(sprite) = &entity.sprite_renderer2d {
                for asset_ref in [sprite.sprite_ref.as_ref(), sprite.material_ref.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    match self.asset_loader.load(asset_ref) {
                        Ok(handle) => handles.push(handle),
                        Err(_) => {
                            report.diagnostics.push(
                                InstanceDiagnostic::error(
                                    "asset dependency missing",
                                    format!(
                                        "entity {} points to missing asset: {}",
                                        entity.id, asset_ref.id
                                    ),
                                    InstanceStage::ResolveAssets,
                                )
                                .with_source_entity_id(SourceEntityId::from(entity.id.clone()))
                                .with_suggested_fix(
                                    "Check RuntimeAssetIndex and bundle mount state.",
                                ),
                            );
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn release_handles(&mut self, handles: Vec<RuntimeAssetHandle>) {
        for handle in handles {
            if handle.state != RuntimeAssetLoadState::Released {
                let _ = self.asset_loader.release(&handle);
            }
        }
    }

    fn next_instance_id(&mut self) -> RuntimeInstanceId {
        let id = RuntimeInstanceId(self.next_instance_id);
        self.next_instance_id += 1;
        id
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }
}

fn prefab_data_from_package(
    package: &RuntimePackage,
    prefab_ref: &RuntimeAssetRef,
) -> Option<RuntimePrefabData> {
    let asset = package.assets.assets.iter().find(|asset| {
        asset.asset_type == "prefab"
            && (asset.id == prefab_ref.id
                || prefab_ref
                    .guid
                    .as_ref()
                    .is_some_and(|guid| guid == &asset.id))
    })?;
    runtime_prefab_data(asset)
}

fn runtime_prefab_data(asset: &RuntimeAsset) -> Option<RuntimePrefabData> {
    let data = asset.data.as_ref()?;
    let document = if data.get("schemaVersion").is_some() {
        data
    } else {
        data.get("prefab")?
    };
    match crate::spatial_contract::decode_runtime_prefab(document.clone()).ok()? {
        crate::spatial_contract::RuntimePrefabDocument::Legacy(prefab) => Some(prefab),
        crate::spatial_contract::RuntimePrefabDocument::Spatial(prefab) => Some(prefab.into()),
    }
}

fn prefab_root_source(prefab: &RuntimePrefabData) -> Option<SourceEntityId> {
    if let Some(root) = &prefab.root_entity_id {
        return Some(SourceEntityId::from(root.clone()));
    }
    prefab
        .entities
        .iter()
        .find(|entity| entity.parent_id.is_none())
        .map(|entity| SourceEntityId::from(entity.id.clone()))
}

fn debug_entity_map_for_level(
    source_to_runtime: &BTreeMap<SourceEntityId, RuntimeEntityId>,
    report_level: RuntimeReportLevel,
) -> Vec<(String, String)> {
    if report_level != RuntimeReportLevel::Trace {
        return Vec::new();
    }
    source_to_runtime
        .iter()
        .map(|(source, runtime)| (source.to_string(), runtime.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archetype::ComponentValue;
    use crate::component_value::RuntimeValue;
    use crate::components::ComponentTypeId;
    use crate::ids::EntityId;
    use crate::math::Vec2;
    use crate::physics2d::Shape2D;
    use crate::runtime_asset::{
        BundleRecord, CookedAssetRecord, RuntimeAssetDependencyRecord, RuntimeAssetIndex,
        RuntimeAssetRecord, RuntimePackageMountTable,
    };
    use crate::runtime_package::{
        RuntimeAssetManifest, RuntimeInputManifest, RuntimeInputMappingManifestEntry,
        RuntimeManifestAssetIndex, RuntimeManifestInputIndex, RuntimeManifestRuleIndex,
        RuntimeMesh, RuntimePackageManifest, RuntimeProjectComponent, RuntimeProjectInfo,
        RuntimeRuleManifest, RuntimeSceneManifestEntry, RuntimeTransform, Vector3,
        RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION, RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION,
        RUNTIME_PACKAGE_MODE, RUNTIME_PACKAGE_SCHEMA_VERSION, RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
        RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use crate::{render_extract::RenderExtractContext, render_state::RenderSceneState};
    use std::path::PathBuf;

    #[test]
    fn scene_dependency_read_failure_keeps_current_asset_cause_before_commit() {
        let mut package = package_fixture();
        package.source = crate::runtime_package_source::RuntimePackageSource::memory(
            std::iter::empty::<(String, Vec<u8>)>(),
        )
        .unwrap();
        package.assets.runtime_asset_index[0].dependencies = vec!["model-player".into()];
        package.assets.cooked_asset_table[1].path =
            Some("missing-cooked/model-player.asset".into());
        package.runtime_asset_index = RuntimeAssetIndex::from_manifest(
            &package.assets,
            &package.assets.runtime_asset_index,
            &package.assets.cooked_asset_table,
            &package.assets.dependency_table,
        );
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        assert!(loader
            .asset_loader_mut()
            .load_by_id("older-unrelated-failure", "texture")
            .is_err());
        let mut world = World::new();

        let (instance, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(loader
            .asset_loader()
            .diagnostics()
            .has_error(crate::runtime_asset_diagnostics::AssetLoadErrorCode::CookedAssetMissing));
        assert!(instance.is_none());
        assert!(!report.committed);
        assert_eq!(world.entity_count(), 0);
        let diagnostic = &report.diagnostics[0];
        assert_eq!(diagnostic.stage, InstanceStage::ResolveAssets);
        assert_eq!(diagnostic.asset_guid.as_deref(), Some("model-player"));
        for detail in [
            "model-player",
            "CookedAssetMissing",
            "Read",
            "cooked-model-player",
            "missing-cooked/model-player.asset",
        ] {
            assert!(diagnostic.message.contains(detail), "{diagnostic:?}");
        }
        assert!(!diagnostic.message.contains("older-unrelated-failure"));
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    }

    #[test]
    fn scene_instance_creates_entities_and_remaps_entity_refs() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (instance, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        let instance = instance.expect("scene instance should load");
        assert_eq!(instance.source_to_runtime_entity.len(), 2);
        assert_eq!(world.entity_count(), 2);
        assert!(world.transform(&EntityId::from("player")).is_some());
        assert_eq!(report.remapped_reference_count, 1);
        assert_eq!(
            world.component_value(
                &EntityId::from("target"),
                &ComponentTypeId::from("game.target_ref"),
            ),
            Some(ComponentValue::Dynamic {
                component_type: ComponentTypeId::from("game.target_ref"),
                value: RuntimeValue::object([(
                    "entityRef",
                    RuntimeValue::EntityRef(EntityId::from("player"))
                )]),
            })
        );
    }

    #[test]
    fn prefab_command_position_is_applied_before_projection_and_preserves_prefab_shape() {
        use crate::gameplay_command::{
            apply_gameplay_commands_with_runtime, GameplayCommand, GameplayCommandId,
            RuntimeCommandContext,
        };
        use crate::math::Vec3;
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let mut roots = Vec::new();
        for (index, position) in [
            None,
            Some(Vec3 {
                x: 3.5,
                y: -3.25,
                z: 0.0,
            }),
        ]
        .into_iter()
        .enumerate()
        {
            let records = apply_gameplay_commands_with_runtime(
                &mut world,
                vec![(
                    GameplayCommandId(index as u64),
                    GameplayCommand::InstantiatePrefab {
                        prefab_ref: asset_ref("prefab-ship", "prefab"),
                        parent_entity: None,
                        target_scene_instance: None,
                        position,
                    },
                )],
                RuntimeCommandContext {
                    package: &package,
                    instance_loader: &mut loader,
                },
            );
            assert_eq!(records[0].result, "ok");
            let root = records[0].root_entity_id.unwrap();
            let id = world
                .try_resolve_runtime_entity(root)
                .unwrap()
                .source_id
                .clone();
            roots.push(id.clone());
            let transform = world.transform(&id).unwrap();
            assert_eq!(
                transform.local_position,
                position.unwrap_or(Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                })
            );
            assert_eq!(
                transform.local_scale,
                Vec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0
                }
            );
        }
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let commands = extract
            .extract_world_dirty(1, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(crate::render_command::apply_batch(&mut scene, &commands).is_empty());
        let proxy = scene
            .proxy(scene.proxy_for_source(&roots[1]).unwrap())
            .unwrap();
        assert_eq!(
            proxy.common.transform.local_position,
            Vec3 {
                x: 3.5,
                y: -3.25,
                z: 0.0
            }
        );
    }

    #[test]
    fn prefab_instance_under_parent_creates_unique_entities() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (scene, _) = loader.load_active_scene_instance(&package, &mut world);
        let scene = scene.unwrap();

        let (first, first_report) = loader.instantiate_prefab_from_package(
            &package,
            asset_ref("prefab-ship", "prefab"),
            Some(EntityId::from("player")),
            Some(scene.instance_id),
            &mut world,
        );
        let (second, second_report) = loader.instantiate_prefab_from_package(
            &package,
            asset_ref("prefab-ship", "prefab"),
            Some(EntityId::from("player")),
            Some(scene.instance_id),
            &mut world,
        );

        assert!(!first_report.has_errors(), "{:?}", first_report.diagnostics);
        assert!(
            !second_report.has_errors(),
            "{:?}",
            second_report.diagnostics
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first.root_entity, second.root_entity);
        assert_eq!(world.entity_count(), 6);
        assert_eq!(first_report.remapped_reference_count, 3);
    }

    #[test]
    fn unload_scene_and_despawn_prefab_release_entities_and_handles() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (scene, _) = loader.load_active_scene_instance(&package, &mut world);
        let scene = scene.unwrap();
        let (prefab, _) = loader.instantiate_prefab_from_package(
            &package,
            asset_ref("prefab-ship", "prefab"),
            Some(EntityId::from("player")),
            Some(scene.instance_id),
            &mut world,
        );
        let prefab = prefab.unwrap();

        let despawn_report = loader.despawn_prefab_instance(prefab.instance_id, &mut world);
        assert!(
            !despawn_report.has_errors(),
            "{:?}",
            despawn_report.diagnostics
        );
        assert_eq!(world.entity_count(), 2);

        let unload_report = loader.unload_scene_instance(scene.instance_id, &mut world);
        assert!(
            !unload_report.has_errors(),
            "{:?}",
            unload_report.diagnostics
        );
        assert_eq!(world.entity_count(), 0);
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    }

    #[test]
    fn missing_prefab_asset_reports_diagnostic() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (instance, report) = loader.instantiate_prefab_from_package(
            &package,
            asset_ref("missing-prefab", "prefab"),
            None,
            None,
            &mut world,
        );

        assert!(instance.is_none());
        assert!(report.has_errors());
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == "prefab asset missing"));
    }

    #[test]
    fn scene_preflight_errors_leave_world_instances_and_handles_unchanged() {
        let mut duplicate = package_fixture();
        duplicate.active_scene.entities[1].id = "player".to_string();
        assert_scene_failure_is_clean(duplicate, "world.entity.duplicate_id");

        let mut missing_parent = package_fixture();
        missing_parent.active_scene.entities[0].parent_id = Some("missing".to_string());
        assert_scene_failure_is_clean(missing_parent, "world.parent.missing");

        let mut cycle = package_fixture();
        cycle.active_scene.entities[0].parent_id = Some("target".to_string());
        cycle.active_scene.entities[1].parent_id = Some("player".to_string());
        assert_scene_failure_is_clean(cycle, "world.parent.cycle");

        let mut missing_transform = package_fixture();
        missing_transform.active_scene.entities[0].transform = None;
        assert_scene_failure_is_clean(missing_transform, "world.component.missing");

        let mut invalid_collider = package_fixture();
        invalid_collider.active_scene.entities[0]
            .components
            .push(RuntimeProjectComponent {
                component_type: ComponentTypeId::collider2d().to_string(),
                data: serde_json::json!({ "shape": "triangle" }),
            });
        assert_scene_failure_is_clean(invalid_collider, "world.component.decode_failed");

        let mut missing_ref = package_fixture();
        missing_ref.active_scene.entities[1].components[0].data =
            serde_json::json!({ "entityRef": "missing" });
        assert_scene_failure_is_clean(missing_ref, "world.entity_ref.missing_target");
    }

    #[test]
    fn existing_world_collision_does_not_replace_entity_or_register_instance() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        world
            .try_spawn_entity(
                EntityId::from("player"),
                "Existing",
                "actor",
                true,
                crate::components::Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
            )
            .expect("fixture entity should spawn");

        let (instance, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(instance.is_none());
        assert!(!report.committed);
        assert!(!report.world_changed);
        assert_eq!(world.entity_count(), 1);
        assert_eq!(
            world.entity(&EntityId::from("player")).unwrap().name,
            "Existing"
        );
        assert!(loader.scene_instances.is_empty());
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    }

    #[test]
    fn prefab_preflight_errors_do_not_activate_or_leak() {
        let mut duplicate = package_fixture();
        let prefab = duplicate
            .assets
            .assets
            .iter_mut()
            .find(|asset| asset.id == "prefab-ship")
            .expect("prefab fixture");
        prefab.data.as_mut().unwrap()["entities"][1]["id"] = serde_json::json!("wingman-root");
        let mut loader = RuntimeInstanceLoader::from_package(&duplicate);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (instance, report) = loader.instantiate_prefab_from_package(
            &duplicate,
            asset_ref("prefab-ship", "prefab"),
            None,
            None,
            &mut world,
        );

        assert!(instance.is_none());
        assert!(report
            .diagnostics
            .iter()
            .any(|issue| issue.kind == "world.entity.duplicate_id"));
        assert!(!report.committed);
        assert!(!report.world_changed);
        assert_eq!(world.entity_count(), 0);
        assert!(loader.prefab_instances.is_empty());
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);

        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (instance, report) = loader.instantiate_prefab_from_package(
            &package,
            asset_ref("prefab-ship", "prefab"),
            Some(EntityId::from("missing-parent")),
            None,
            &mut world,
        );
        assert!(instance.is_none());
        assert!(report
            .diagnostics
            .iter()
            .any(|issue| issue.kind == "world.parent.missing"));
        assert_eq!(world.entity_count(), 0);
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);

        let (instance, report) = loader.instantiate_prefab_from_package(
            &package,
            asset_ref("prefab-ship", "prefab"),
            None,
            Some(RuntimeInstanceId(999)),
            &mut world,
        );
        assert!(instance.is_none());
        assert!(report
            .diagnostics
            .iter()
            .any(|issue| issue.kind == "world.instance.target_scene_missing"));
        assert_eq!(world.entity_count(), 0);
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    }

    #[test]
    fn runtime_report_defaults_off_and_debug_map_requires_trace() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (_, report) = loader.load_active_scene_instance(&package, &mut world);
        assert_eq!(report.report_level, RuntimeReportLevel::Off);
        assert!(report.source_to_runtime_entity_debug.is_empty());
        assert!(report.committed);
        assert!(report.world_changed);

        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.set_report_level(RuntimeReportLevel::Trace);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (_, report) = loader.load_active_scene_instance(&package, &mut world);
        assert_eq!(report.source_to_runtime_entity_debug.len(), 2);
    }

    #[test]
    fn render_extract_sees_renderable_after_scene_instance() {
        let package = package_fixture();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (_, report) = loader.load_active_scene_instance(&package, &mut world);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);

        let scene_state = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let queue = extract.extract_world_dirty(1, &mut world, &scene_state);

        assert!(!queue.pending_commands.is_empty());
    }

    fn assert_scene_failure_is_clean(package: RuntimePackage, expected_kind: &str) {
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (instance, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(instance.is_none());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|issue| issue.kind == expected_kind),
            "{:?}",
            report.diagnostics
        );
        assert!(!report.committed);
        assert!(!report.world_changed);
        assert_eq!(world.entity_count(), 0);
        assert!(loader.scene_instances.is_empty());
        assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    }

    #[test]
    fn scene_instance_hydrates_sprite_renderer2d_and_render_extract_projects_sprite() {
        let mut package = package_fixture();
        package
            .active_scene
            .entities
            .push(runtime_sprite_entity("sprite-ship", Some("texture-ship")));
        package
            .assets
            .assets
            .push(runtime_asset("texture-ship", "texture", None));
        package
            .assets
            .runtime_asset_index
            .push(record("texture-ship", "texture"));
        package
            .assets
            .cooked_asset_table
            .push(cooked("texture-ship"));
        package.runtime_asset_index = RuntimeAssetIndex::from_manifest(
            &package.assets,
            &package.assets.runtime_asset_index,
            &package.assets.cooked_asset_table,
            &package.assets.dependency_table,
        );
        package.runtime_asset_mount_table =
            RuntimePackageMountTable::from_manifest(&package.assets);
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (_, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        let sprite = world
            .sprite_renderer2d(&EntityId::from("sprite-ship"))
            .expect("sprite component should be hydrated");
        assert_eq!(sprite.sprite_ref.as_deref(), Some("texture-ship"));
        assert_eq!(sprite.order_in_layer, 9);

        let scene_state = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let mut queue = extract.extract_world_dirty(1, &mut world, &scene_state);
        let merged = queue.normalize_merge(&scene_state);

        assert!(merged.iter().any(|command| {
            matches!(
                &command.payload,
                crate::render_command::RenderCommandPayload::AddProxy { descriptor, .. }
                    if descriptor.payload_kind() == crate::render_state::RenderPayloadKind::Sprite
            )
        }));
    }

    #[test]
    fn scene_instance_hydrates_engine_collider2d_as_typed_component() {
        let mut package = package_fixture();
        package.active_scene.entities[0]
            .components
            .push(RuntimeProjectComponent {
                component_type: ComponentTypeId::collider2d().to_string(),
                data: serde_json::json!({
                    "shape": "aabb",
                    "halfExtents": { "x": 0.25, "y": 0.5 },
                    "offset": { "x": 0.1, "y": -0.2 },
                    "layer": 1,
                    "mask": 4294967295u64,
                    "enabled": true,
                    "isSensor": true
                }),
            });
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (_, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        let collider = world
            .collider2d(&EntityId::from("player"))
            .expect("typed collider should hydrate");
        assert_eq!(
            collider.shape,
            Shape2D::Aabb {
                half_extents: Vec2 { x: 0.25, y: 0.5 }
            }
        );
        assert_eq!(collider.offset, Vec2 { x: 0.1, y: -0.2 });
        assert!(collider.is_sensor);
    }

    #[test]
    fn missing_sprite_asset_stops_hydration_with_diagnostic() {
        let mut package = package_fixture();
        package.active_scene.entities.push(runtime_sprite_entity(
            "sprite-ship",
            Some("missing-texture"),
        ));
        package.runtime_asset_index = RuntimeAssetIndex::from_manifest(
            &package.assets,
            &package.assets.runtime_asset_index,
            &package.assets.cooked_asset_table,
            &package.assets.dependency_table,
        );
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();

        let (instance, report) = loader.load_active_scene_instance(&package, &mut world);

        assert!(instance.is_none());
        assert!(report.has_errors());
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == "asset dependency missing"
                && diagnostic.message.contains("missing-texture")
        }));
    }

    fn package_fixture() -> RuntimePackage {
        let default_input_mapping = engine_input::InputMappingAsset::gameplay_default();
        let input_manifest = RuntimeInputManifest {
            schema_version: RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION.to_string(),
            default_mapping_id: default_input_mapping.asset_id.clone(),
            mappings: vec![RuntimeInputMappingManifestEntry {
                id: default_input_mapping.asset_id.clone(),
                path: "input/input.default.json".to_string(),
                enabled: true,
            }],
        };
        let assets = RuntimeAssetManifest {
            schema_version: RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            assets: vec![
                runtime_asset("scene-main", "scene", None),
                runtime_asset("model-player", "model", None),
                runtime_asset("model-wingman", "model", None),
                runtime_asset("prefab-ship", "prefab", Some(prefab_json())),
            ],
            runtime_asset_index: vec![
                record("scene-main", "scene"),
                record("model-player", "model"),
                record("model-wingman", "model"),
                record("prefab-ship", "prefab"),
            ],
            bundle_table: vec![BundleRecord {
                bundle_id: "startup".to_string(),
                mount_id: None,
                uri: "bundles/startup".to_string(),
                hash: None,
                version: None,
                mounted: false,
            }],
            cooked_asset_table: vec![
                cooked("scene-main"),
                cooked("model-player"),
                cooked("model-wingman"),
                cooked("prefab-ship"),
            ],
            dependency_table: Vec::<RuntimeAssetDependencyRecord>::new(),
        };
        let runtime_asset_index = RuntimeAssetIndex::from_manifest(
            &assets,
            &assets.runtime_asset_index,
            &assets.cooked_asset_table,
            &assets.dependency_table,
        );
        let runtime_asset_mount_table = RuntimePackageMountTable::from_manifest(&assets);
        RuntimePackage {
            package_dir: PathBuf::new(),
            source: crate::runtime_package_source::RuntimePackageSource::directory(PathBuf::new()),
            manifest: RuntimePackageManifest {
                schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION.to_string(),
                package_mode: RUNTIME_PACKAGE_MODE.to_string(),
                project: RuntimeProjectInfo::explicit_empty("project-fixture", "Fixture", "0.1.0"),
                active_scene_id: "scene-main".to_string(),
                scenes: vec![RuntimeSceneManifestEntry {
                    id: "scene-main".to_string(),
                    name: "Main".to_string(),
                    path: "scenes/scene-main.json".to_string(),
                    entity_count: 2,
                }],
                assets: RuntimeManifestAssetIndex {
                    path: "assets/asset-manifest.json".to_string(),
                    asset_count: 4,
                },
                rules: RuntimeManifestRuleIndex {
                    path: "rules/rule-manifest.json".to_string(),
                    mode: "none".to_string(),
                },
                input: RuntimeManifestInputIndex {
                    path: "input/input-manifest.json".to_string(),
                    default_mapping_id: default_input_mapping.asset_id.clone(),
                    mapping_count: 1,
                },
                aui: Some(crate::runtime_package::RuntimeManifestAuiIndex {
                    path: "aui/aui-manifest.json".to_string(),
                    document_count: 0,
                }),
                font_atlases: None,
                font_bundles: None,
                animator2d: None,
                spine_animator: None,
                observation_contract: None,
                content_hash: None,
            },
            active_scene: RuntimeScene {
                main_camera: None,
                schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
                id: "scene-main".to_string(),
                name: "Main".to_string(),
                gravity: 0.0,
                background: "#000".to_string(),
                sky_color: "#111".to_string(),
                entities: vec![
                    runtime_entity("player", None, Some("model-player")),
                    runtime_entity_with_component("target", None, None, "player"),
                ],
            },
            assets,
            runtime_asset_index,
            runtime_asset_mount_table,
            rules: RuntimeRuleManifest {
                schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
                mode: "none".to_string(),
                rules: Vec::new(),
                modules: Vec::new(),
            },
            aui_manifest: crate::runtime_package::RuntimeAuiManifest::empty(),
            aui_documents: crate::runtime_package::RuntimeAuiDocumentRegistry::empty("fixture"),
            aui_themes: crate::aui_theme::AuiThemeRegistry::builtin(),
            font_atlas_manifest: crate::runtime_package::RuntimeFontAtlasManifest::empty(),
            font_atlases: crate::runtime_package::RuntimeAuiFontAtlasRegistry::empty("fixture"),
            font_bundle_manifest: crate::font_bundle::RuntimeFontBundleManifest::empty(),
            font_bundles: crate::font_bundle::RuntimeFontBundleRegistry::default(),
            animator2d_registry: crate::animator2d::CookedAnimator2DRegistry::empty(),
            spine_animator_registry: None,
            input_manifest,
            input_mappings: vec![default_input_mapping.clone()],
            default_input_mapping: Some(default_input_mapping),
        }
    }

    fn runtime_asset(id: &str, asset_type: &str, data: Option<serde_json::Value>) -> RuntimeAsset {
        RuntimeAsset {
            id: id.to_string(),
            name: id.to_string(),
            asset_type: asset_type.to_string(),
            source: format!("{}.asset", id),
            state: "available".to_string(),
            bundle_id: "startup".to_string(),
            data,
        }
    }

    fn record(id: &str, asset_type: &str) -> RuntimeAssetRecord {
        RuntimeAssetRecord {
            asset_guid: id.to_string(),
            asset_id: id.to_string(),
            asset_type: asset_type.to_string(),
            sub_asset_id: None,
            version: "1".to_string(),
            cooked_asset_id: format!("cooked-{}", id),
            bundle_id: "startup".to_string(),
            loader_kind: asset_type.to_string(),
            dependencies: Vec::new(),
            hash: None,
            size: Some(8),
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    fn cooked(id: &str) -> CookedAssetRecord {
        CookedAssetRecord {
            cooked_asset_id: format!("cooked-{}", id),
            bundle_id: "startup".to_string(),
            path: None,
            offset: None,
            size: Some(8),
            compression: None,
            hash: None,
        }
    }

    fn runtime_entity(id: &str, parent_id: Option<&str>, model: Option<&str>) -> RuntimeEntity {
        RuntimeEntity {
            spatial: None,
            schema_version: "runtime-entity.v1".to_string(),
            id: id.to_string(),
            name: id.to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: parent_id.map(str::to_string),
            sibling_order: 0,
            transform: Some(RuntimeTransform {
                local_position: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                local_rotation: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                local_scale: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }),
            mesh: model.map(|model| RuntimeMesh {
                primitive: Some("model".to_string()),
                color: None,
                label: None,
                asset_ref: Some(asset_ref(model, "model")),
                material_ref: None,
                texture_ref: None,
                visible: true,
                layer: "default".to_string(),
                metalness: None,
                roughness: None,
            }),
            sprite_renderer2d: None,
            animator2d: None,
            components: Vec::new(),
        }
    }

    fn runtime_entity_with_component(
        id: &str,
        parent_id: Option<&str>,
        model: Option<&str>,
        entity_ref: &str,
    ) -> RuntimeEntity {
        let mut entity = runtime_entity(id, parent_id, model);
        entity.components.push(RuntimeProjectComponent {
            component_type: "game.target_ref".to_string(),
            data: serde_json::json!({ "entityRef": entity_ref }),
        });
        entity
    }

    fn runtime_sprite_entity(id: &str, texture_asset_id: Option<&str>) -> RuntimeEntity {
        let mut entity = runtime_entity(id, None, None);
        entity.sprite_renderer2d = Some(crate::runtime_package::RuntimeSpriteRenderer2D {
            sprite_ref: texture_asset_id.map(|id| asset_ref(id, "texture")),
            material_ref: None,
            color: Some([1.0, 0.5, 0.25, 1.0]),
            flip_x: Some(false),
            flip_y: Some(true),
            sorting_layer: Some(3),
            order_in_layer: Some(9),
            sort_z: Some(0.25),
            visible: Some(true),
        });
        entity
    }

    fn prefab_json() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": "runtime-prefab.v1",
            "id": "prefab-ship",
            "name": "Wingman",
            "rootEntityId": "wingman-root",
            "entities": [
                entity_json("wingman-root", serde_json::Value::Null, Some("model-wingman"), "wingman-child"),
                entity_json("wingman-child", serde_json::json!("wingman-root"), None, "wingman-root")
            ]
        })
    }

    fn entity_json(
        id: &str,
        parent_id: serde_json::Value,
        model: Option<&str>,
        entity_ref: &str,
    ) -> serde_json::Value {
        let mut value = serde_json::json!({
            "schemaVersion": "runtime-entity.v1",
            "id": id,
            "name": id,
            "kind": "actor",
            "enabled": true,
            "parentId": parent_id,
            "siblingOrder": 0,
            "transform": {
                "localPosition": { "x": 0.0, "y": 0.0, "z": 0.0 },
                "localRotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
                "localScale": { "x": 1.0, "y": 1.0, "z": 1.0 }
            },
            "components": [{
                "componentType": "game.target_ref",
                "data": { "entityRef": entity_ref }
            }]
        });
        if let Some(model) = model {
            value["mesh"] = serde_json::json!({
                "primitive": "model",
                "assetRef": { "id": model, "type": "model" },
                "visible": true,
                "layer": "default"
            });
        }
        value
    }

    fn asset_ref(id: &str, asset_type: &str) -> RuntimeAssetRef {
        RuntimeAssetRef {
            id: id.to_string(),
            asset_type: asset_type.to_string(),
            guid: None,
            sub_asset: None,
        }
    }
}

pub(crate) fn prefab_has_spatial_root(
    package: &RuntimePackage,
    reference: &RuntimeAssetRef,
) -> bool {
    prefab_data_from_package(package, reference).is_some_and(|p| {
        p.entities
            .iter()
            .any(|e| Some(&e.id) == p.root_entity_id.as_ref() && e.spatial.is_some())
    })
}

// Owned by the existing loader. Dropping an uncommitted batch releases every asset lease.
#[derive(Debug)]
struct PreparedPrefab {
    prepared: PreparedRuntimeEntities,
    prefab_ref: RuntimeAssetRef,
    parent_entity: Option<SourceEntityId>,
    target_scene_instance: Option<RuntimeInstanceId>,
    root_source: Option<SourceEntityId>,
    instance_id: RuntimeInstanceId,
    handles: Vec<RuntimeAssetHandle>,
}
pub(crate) struct DeferredPrefabs<'a> {
    loader: &'a mut RuntimeInstanceLoader,
    prepared: Vec<(u64, PreparedPrefab, PrefabInstantiateReport)>,
}
impl DeferredPrefabs<'_> {
    pub(crate) fn prepare(
        &mut self,
        request_id: u64,
        reference: RuntimeAssetRef,
        package: &RuntimePackage,
        world: &World,
    ) -> Result<(), PrefabInstantiateReport> {
        if !package
            .runtime_asset_index
            .resolve(&reference)
            .is_ok_and(|record| {
                record.asset_id == reference.id && record.sub_asset_id == reference.sub_asset
            })
        {
            let mut report = PrefabInstantiateReport::new(request_id, reference.id.clone())
                .with_report_level(self.loader.report_level);
            report.diagnostics.push(InstanceDiagnostic::error(
                "three_d.prefab_reference_invalid",
                "Prefab id/GUID/sub_asset must resolve to the same package entry.",
                InstanceStage::ResolveAssets,
            ));
            return Err(report);
        }
        let (prepared, mut report) = self
            .loader
            .prepare_prefab_from_package(package, reference, None, None, world);
        match prepared {
            Some(prepared) => {
                let spatial_root = prepared.root_source.as_ref().is_some_and(|root| {
                    prepared.prepared.entities.iter().any(|entity| {
                        &entity.source_id == root
                            && entity.components.iter().any(|c| {
                                matches!(c, crate::archetype::ComponentValue::Transform3D(_))
                            })
                    })
                });
                if !spatial_root {
                    self.loader.release_handles(prepared.handles);
                    report.diagnostics.push(InstanceDiagnostic::error(
                        "three_d.prefab_root_required",
                        "Spatial prefab needs an actual World3D root.",
                        InstanceStage::PrepareEntities,
                    ));
                    return Err(report);
                }
                self.prepared.push((request_id, prepared, report));
                Ok(())
            }
            None => Err(report),
        }
    }
    pub(crate) fn commit(mut self, world: &mut World) -> BTreeMap<u64, (SourceEntityId, u64)> {
        let mut result = BTreeMap::new();
        for (request, prepared, report) in std::mem::take(&mut self.prepared) {
            let (instance, _) = self.loader.commit_prepared_prefab(prepared, report, world);
            if let Some(root) = instance.root_entity {
                let id = world
                    .try_resolve_runtime_entity(root)
                    .expect("committed root")
                    .source_id
                    .clone();
                let generation = world
                    .entity_generation(&id)
                    .expect("committed root generation");
                result.insert(request, (id, generation));
            }
        }
        result
    }
}
impl Drop for DeferredPrefabs<'_> {
    fn drop(&mut self) {
        for (_, prepared, _) in std::mem::take(&mut self.prepared) {
            self.loader.release_handles(prepared.handles);
        }
    }
}
