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

#[derive(Debug)]
pub struct RuntimeInstanceLoader {
    asset_loader: RuntimeAssetLoader,
    scene_instances: BTreeMap<RuntimeInstanceId, RuntimeSceneInstance>,
    prefab_instances: BTreeMap<RuntimeInstanceId, RuntimePrefabInstance>,
    next_instance_id: u64,
    next_request_id: u64,
    report_level: RuntimeReportLevel,
}

impl RuntimeInstanceLoader {
    pub fn from_package(package: &RuntimePackage) -> Self {
        Self {
            asset_loader: RuntimeAssetLoader::new(
                package.package_dir.clone(),
                package.runtime_asset_index.clone(),
                package.runtime_asset_mount_table.clone(),
            ),
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

        report.stage = InstanceStage::ResolveAssets;
        match self.asset_loader.load(&scene_ref) {
            Ok(handle) => handles.push(handle),
            Err(_) => {
                report.diagnostics.push(
                    InstanceDiagnostic::error(
                        "scene asset missing",
                        format!("scene asset could not be loaded: {}", scene_ref.id),
                        InstanceStage::ResolveAssets,
                    )
                    .with_suggested_fix(
                        "Check RuntimeAssetIndex and mounted bundle for scene asset.",
                    ),
                );
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
        let source_to_world = prepared.source_to_world.clone();
        report.remapped_reference_count = prepared.remapped_reference_count;
        report.stage = InstanceStage::CommitEntities;
        let source_to_runtime = prepared.commit(world).source_to_runtime;
        report.committed = true;
        report.world_changed = !source_to_runtime.is_empty();

        let root_entity = prefab_root_source(&prefab_data)
            .and_then(|source| source_to_runtime.get(&source).copied());
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
        (Some(instance), report)
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

    fn collect_entity_asset_refs(
        &mut self,
        entities: &[RuntimeEntity],
        handles: &mut Vec<RuntimeAssetHandle>,
        report: &mut crate::runtime_instance::RuntimeInstantiateReport,
    ) -> bool {
        report.stage = InstanceStage::ResolveAssets;
        for entity in entities {
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
    if data.get("schemaVersion").is_some() {
        serde_json::from_value::<RuntimePrefabData>(data.clone()).ok()
    } else {
        serde_json::from_value::<RuntimePrefabData>(data.get("prefab")?.clone()).ok()
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
            manifest: RuntimePackageManifest {
                schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION.to_string(),
                package_mode: RUNTIME_PACKAGE_MODE.to_string(),
                project: RuntimeProjectInfo::explicit_empty("project-fixture", "Fixture", "0.0.3"),
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
                observation_contract: None,
                content_hash: None,
            },
            active_scene: RuntimeScene {
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
            font_atlas_manifest: crate::runtime_package::RuntimeFontAtlasManifest::empty(),
            font_atlases: crate::runtime_package::RuntimeAuiFontAtlasRegistry::empty("fixture"),
            font_bundle_manifest: crate::font_bundle::RuntimeFontBundleManifest::empty(),
            font_bundles: crate::font_bundle::RuntimeFontBundleRegistry::default(),
            animator2d_registry: crate::animator2d::CookedAnimator2DRegistry::empty(),
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
