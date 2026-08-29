//! Historical module name for RuntimePackage -> World hydration.
//!
//! Current architecture reads this as `HydrationProjection` /
//! `HydrationProjectionAdapter<RuntimeScene>`. The public functions remain for
//! compatibility, while reports expose projection terminology.

use crate::diagnostics::{RuntimeDiagnostics, RuntimeLoadResult};
use crate::projection::{ProjectionDiagnostic, ProjectionDomain, ProjectionKind, ProjectionReport};
use crate::runtime_instance::{
    RuntimeInstanceState, RuntimeInstantiateReport, RuntimeSceneInstance, SceneInstantiateReport,
};
use crate::runtime_instance_diagnostics::InstanceDiagnosticSeverity;
use crate::runtime_instance_loader::RuntimeInstanceLoader;
use crate::runtime_package::{RuntimePackage, RuntimeScene};
use crate::world::{DirtyRecord, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSceneHydrationRequest {
    pub scene_id: String,
    pub mount_startup_bundle: bool,
}

impl RuntimeSceneHydrationRequest {
    pub fn active_scene(package: &RuntimePackage) -> Self {
        Self {
            scene_id: package.active_scene.id.clone(),
            mount_startup_bundle: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeSceneHydrationReport {
    pub scene_id: String,
    pub instance: Option<RuntimeSceneInstance>,
    pub instantiate_report: SceneInstantiateReport,
    pub initial_dirty_records: Vec<DirtyRecord>,
}

impl RuntimeSceneHydrationReport {
    pub fn has_errors(&self) -> bool {
        self.instantiate_report.has_errors()
    }

    pub fn created_entity_count(&self) -> usize {
        self.instantiate_report.created_entity_count
    }

    pub fn loaded_asset_count(&self) -> usize {
        self.instantiate_report.loaded_asset_count
    }

    pub fn projection_summary(&self) -> ProjectionReport {
        let diagnostics = self
            .instantiate_report
            .diagnostics
            .iter()
            .map(|diagnostic| {
                ProjectionDiagnostic::new(
                    match diagnostic.severity {
                        InstanceDiagnosticSeverity::Error => "error",
                        InstanceDiagnosticSeverity::Warning => "warning",
                    },
                    diagnostic.kind.clone(),
                    diagnostic.message.clone(),
                    Some("HydrationProjectionAdapter<RuntimeScene>".to_string()),
                )
            })
            .collect::<Vec<_>>();
        ProjectionReport::new(
            ProjectionKind::Hydration,
            ProjectionDomain::RuntimePackage,
            ProjectionDomain::World,
            "HydrationProjectionAdapter<RuntimeScene>",
        )
        .with_counts(
            self.created_entity_count(),
            0,
            self.instantiate_report
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == InstanceDiagnosticSeverity::Error)
                .count(),
        )
        .with_diagnostics(diagnostics)
    }
}

#[derive(Debug)]
pub struct RuntimeSceneHydrator {
    instance_loader: RuntimeInstanceLoader,
}

impl RuntimeSceneHydrator {
    pub fn from_package(package: &RuntimePackage) -> Self {
        Self {
            instance_loader: RuntimeInstanceLoader::from_package(package),
        }
    }

    pub fn instance_loader(&self) -> &RuntimeInstanceLoader {
        &self.instance_loader
    }

    pub fn instance_loader_mut(&mut self) -> &mut RuntimeInstanceLoader {
        &mut self.instance_loader
    }

    pub fn hydrate_active_scene(
        &mut self,
        package: &RuntimePackage,
        world: &mut World,
    ) -> RuntimeSceneHydrationReport {
        self.hydrate_scene(
            package,
            &package.active_scene,
            RuntimeSceneHydrationRequest::active_scene(package),
            world,
        )
    }

    pub fn hydrate_scene(
        &mut self,
        package: &RuntimePackage,
        scene: &RuntimeScene,
        request: RuntimeSceneHydrationRequest,
        world: &mut World,
    ) -> RuntimeSceneHydrationReport {
        if request.mount_startup_bundle {
            self.instance_loader
                .asset_loader_mut()
                .mount_bundle("startup");
        }
        let dirty_before = world.dirty_records().len();
        let (instance, instantiate_report) = if scene.id == package.active_scene.id {
            self.instance_loader
                .load_active_scene_instance(package, world)
        } else {
            self.instance_loader.load_scene_instance(
                scene,
                crate::runtime_package::RuntimeAssetRef {
                    id: request.scene_id.clone(),
                    asset_type: "scene".to_string(),
                    guid: None,
                    sub_asset: None,
                },
                request.scene_id.clone(),
                world,
            )
        };
        let initial_dirty_records = world.dirty_records()[dirty_before..].to_vec();
        RuntimeSceneHydrationReport {
            scene_id: request.scene_id,
            instance,
            instantiate_report,
            initial_dirty_records,
        }
    }
}

pub fn hydrate_active_scene_into_world(
    package: &RuntimePackage,
) -> RuntimeLoadResult<(World, RuntimeSceneHydrationReport)> {
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(package);
    let report = hydrator.hydrate_active_scene(package, &mut world);
    if report.has_errors() {
        RuntimeLoadResult::failed(diagnostics_from_instantiate_report(
            &report.instantiate_report,
        ))
    } else {
        RuntimeLoadResult::ok((world, report), RuntimeDiagnostics::new())
    }
}

fn diagnostics_from_instantiate_report(report: &RuntimeInstantiateReport) -> RuntimeDiagnostics {
    let mut diagnostics = RuntimeDiagnostics::new();
    for diagnostic in &report.diagnostics {
        let path = diagnostic
            .source_entity_id
            .as_ref()
            .map(|source| format!("scene.entity.{source}"))
            .unwrap_or_else(|| format!("scene.instantiate.{:?}", diagnostic.stage));
        diagnostics.error(path, diagnostic.message.clone());
    }
    diagnostics
}

pub fn hydration_instance_is_active(report: &RuntimeSceneHydrationReport) -> bool {
    report
        .instance
        .as_ref()
        .is_some_and(|instance| instance.state == RuntimeInstanceState::Active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::EntityId;
    use crate::runtime_asset::{
        BundleRecord, CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord,
        RuntimePackageMountTable,
    };
    use crate::runtime_package::{
        RuntimeAsset, RuntimeAssetManifest, RuntimeAssetRef, RuntimeInputManifest,
        RuntimeInputMappingManifestEntry, RuntimeManifestAssetIndex, RuntimeManifestInputIndex,
        RuntimeManifestRuleIndex, RuntimeMesh, RuntimePackageManifest, RuntimeProjectComponent,
        RuntimeProjectInfo, RuntimeRuleManifest, RuntimeSceneManifestEntry, RuntimeTransform,
        Vector3, RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION, RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION,
        RUNTIME_PACKAGE_MODE, RUNTIME_PACKAGE_SCHEMA_VERSION, RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
        RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use crate::world::DirtyType;
    use std::path::PathBuf;

    #[test]
    fn hydrator_loads_active_scene_into_world_with_instance_report() {
        let package = package_fixture();
        let mut world = World::new();
        let mut hydrator = RuntimeSceneHydrator::from_package(&package);

        let report = hydrator.hydrate_active_scene(&package, &mut world);

        assert!(
            !report.has_errors(),
            "{:?}",
            report.instantiate_report.diagnostics
        );
        assert!(hydration_instance_is_active(&report));
        assert_eq!(world.entity_count(), 2);
        assert_eq!(report.created_entity_count(), 2);
        assert!(world.transform(&EntityId::from("player")).is_some());
        assert!(world.renderable(&EntityId::from("player")).is_some());
        assert!(world
            .component_value(
                &EntityId::from("wingman"),
                &crate::components::ComponentTypeId::new("project.follow")
            )
            .is_some());
    }

    #[test]
    fn hydration_report_exposes_projection_summary() {
        let package = package_fixture();
        let mut world = World::new();
        let mut hydrator = RuntimeSceneHydrator::from_package(&package);

        let report = hydrator.hydrate_active_scene(&package, &mut world);
        let projection = report.projection_summary();

        assert_eq!(projection.kind, ProjectionKind::Hydration);
        assert_eq!(projection.source_domain, ProjectionDomain::RuntimePackage);
        assert_eq!(projection.target_domain, ProjectionDomain::World);
        assert_eq!(projection.projected_count, 2);
        assert_eq!(projection.error_count, 0);
    }

    #[test]
    fn hydrator_records_initial_dirty_records_for_render_and_dynamic_data() {
        let package = package_fixture();
        let mut world = World::new();
        let mut hydrator = RuntimeSceneHydrator::from_package(&package);

        let report = hydrator.hydrate_active_scene(&package, &mut world);

        let dirty_types = report
            .initial_dirty_records
            .iter()
            .map(|record| record.dirty_type.clone())
            .collect::<Vec<_>>();
        assert!(dirty_types.contains(&DirtyType::Transform));
        assert!(dirty_types.contains(&DirtyType::RenderState));
        assert!(dirty_types.contains(&DirtyType::DynamicData));
        assert_eq!(
            report.initial_dirty_records.len(),
            world.dirty_records().len()
        );
    }

    #[test]
    fn hydrate_active_scene_into_world_returns_world_and_report() {
        let package = package_fixture();

        let result = hydrate_active_scene_into_world(&package);

        assert!(
            result.diagnostics.is_ok(),
            "{:?}",
            result.diagnostics.issues
        );
        let (world, report) = result.value.expect("hydration should succeed");
        assert_eq!(world.entity_count(), 2);
        assert_eq!(report.loaded_asset_count(), 2);
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
                runtime_asset("scene-main", "scene"),
                runtime_asset("model-player", "model"),
            ],
            runtime_asset_index: vec![
                runtime_asset_record("scene-main", "scene"),
                runtime_asset_record("model-player", "model"),
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
                cooked_asset_record("scene-main"),
                cooked_asset_record("model-player"),
            ],
            dependency_table: Vec::new(),
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
                project: RuntimeProjectInfo::explicit_empty(
                    "project-hydration-fixture",
                    "Hydration Fixture",
                    "0.0.3",
                ),
                active_scene_id: "scene-main".to_string(),
                scenes: vec![RuntimeSceneManifestEntry {
                    id: "scene-main".to_string(),
                    name: "Main".to_string(),
                    path: "scenes/scene-main.json".to_string(),
                    entity_count: 2,
                }],
                assets: RuntimeManifestAssetIndex {
                    path: "assets/asset-manifest.json".to_string(),
                    asset_count: 2,
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
                background: "#000000".to_string(),
                sky_color: "#111111".to_string(),
                entities: vec![
                    runtime_entity("player", Some("model-player"), Vec::new()),
                    runtime_entity(
                        "wingman",
                        None,
                        vec![RuntimeProjectComponent {
                            component_type: "project.follow".to_string(),
                            data: serde_json::json!({ "entityRef": "player" }),
                        }],
                    ),
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

    fn runtime_asset(id: &str, asset_type: &str) -> RuntimeAsset {
        RuntimeAsset {
            id: id.to_string(),
            name: id.to_string(),
            asset_type: asset_type.to_string(),
            source: format!("{id}.asset"),
            state: "available".to_string(),
            bundle_id: "startup".to_string(),
            data: None,
        }
    }

    fn runtime_asset_record(id: &str, asset_type: &str) -> RuntimeAssetRecord {
        RuntimeAssetRecord {
            asset_guid: id.to_string(),
            asset_id: id.to_string(),
            asset_type: asset_type.to_string(),
            sub_asset_id: None,
            version: "1".to_string(),
            cooked_asset_id: format!("cooked-{id}"),
            bundle_id: "startup".to_string(),
            loader_kind: asset_type.to_string(),
            dependencies: Vec::new(),
            hash: None,
            size: Some(8),
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    fn cooked_asset_record(id: &str) -> CookedAssetRecord {
        CookedAssetRecord {
            cooked_asset_id: format!("cooked-{id}"),
            bundle_id: "startup".to_string(),
            path: None,
            offset: None,
            size: Some(8),
            compression: None,
            hash: None,
        }
    }

    fn runtime_entity(
        id: &str,
        model: Option<&str>,
        components: Vec<RuntimeProjectComponent>,
    ) -> crate::runtime_package::RuntimeEntity {
        crate::runtime_package::RuntimeEntity {
            schema_version: "runtime-entity.v1".to_string(),
            id: id.to_string(),
            name: id.to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: None,
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
                asset_ref: Some(RuntimeAssetRef {
                    id: model.to_string(),
                    asset_type: "model".to_string(),
                    guid: None,
                    sub_asset: None,
                }),
                material_ref: None,
                texture_ref: None,
                visible: true,
                layer: "default".to_string(),
                metalness: None,
                roughness: None,
            }),
            sprite_renderer2d: None,
            animator2d: None,
            components,
        }
    }
}
