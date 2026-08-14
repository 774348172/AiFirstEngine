use crate::runtime_asset::{
    prepared_kind, DecodedAsset, PreparedRuntimeResource, RuntimeAssetHandle, RuntimeAssetIndex,
    RuntimeAssetLoadState, RuntimeAssetResolveError, RuntimePackageMountTable,
};
use crate::runtime_asset_diagnostics::{
    AssetLoadDiagnostic, AssetLoadDiagnostics, AssetLoadErrorCode, AssetLoadStage, AssetLoadState,
};
use crate::runtime_package::RuntimeAssetRef;
use crate::runtime_package_path::safe_join_runtime_package;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct RuntimeAssetLoader {
    package_dir: PathBuf,
    index: RuntimeAssetIndex,
    mount_table: RuntimePackageMountTable,
    decoded_cache: HashMap<String, DecodedAsset>,
    handles: HashMap<u64, RuntimeAssetHandle>,
    dependency_handles_by_owner: HashMap<u64, Vec<RuntimeAssetHandle>>,
    diagnostics: AssetLoadDiagnostics,
    next_request_id: u64,
    next_handle_id: u64,
    next_generation: u64,
}

impl RuntimeAssetLoader {
    pub fn new(
        package_dir: impl Into<PathBuf>,
        index: RuntimeAssetIndex,
        mount_table: RuntimePackageMountTable,
    ) -> Self {
        Self {
            package_dir: package_dir.into(),
            index,
            mount_table,
            decoded_cache: HashMap::new(),
            handles: HashMap::new(),
            dependency_handles_by_owner: HashMap::new(),
            diagnostics: AssetLoadDiagnostics::new(),
            next_request_id: 1,
            next_handle_id: 1,
            next_generation: 1,
        }
    }

    pub fn mount_bundle(&mut self, bundle_id: &str) -> bool {
        self.mount_table.mount_bundle(bundle_id)
    }

    pub fn unmount_bundle(&mut self, bundle_id: &str) {
        self.mount_table.unmount_bundle(bundle_id);
    }

    pub fn mount_patch_index(&mut self, patch: RuntimeAssetIndex) {
        self.index.merge_patch(patch);
    }

    pub fn load(&mut self, asset_ref: &RuntimeAssetRef) -> Result<RuntimeAssetHandle, ()> {
        let request_id = self.next_request_id();
        self.load_internal(
            asset_ref,
            request_id,
            true,
            &mut Vec::new(),
            &mut HashSet::new(),
        )
    }

    pub fn load_async(&mut self, asset_ref: &RuntimeAssetRef) -> Result<RuntimeAssetHandle, ()> {
        let request_id = self.next_request_id();
        self.load_internal(
            asset_ref,
            request_id,
            false,
            &mut Vec::new(),
            &mut HashSet::new(),
        )
    }

    pub fn load_by_id(
        &mut self,
        asset_id: impl Into<String>,
        asset_type: impl Into<String>,
    ) -> Result<RuntimeAssetHandle, ()> {
        self.load(&RuntimeAssetRef {
            id: asset_id.into(),
            asset_type: asset_type.into(),
            guid: None,
            sub_asset: None,
        })
    }

    pub fn release(&mut self, handle: &RuntimeAssetHandle) -> Result<(), ()> {
        let request_id = self.next_request_id();
        let Some(stored) = self.handles.get(&handle.handle_id) else {
            self.push_error(
                request_id,
                &handle.asset_id,
                Some(handle.asset_guid.clone()),
                &handle.asset_type,
                AssetLoadStage::Release,
                AssetLoadErrorCode::ReleaseGenerationMismatch,
                Some(handle.bundle_id.clone()),
                Some(handle.cooked_asset_id.clone()),
                Some(handle.loader_kind.clone()),
                Vec::new(),
                Some(handle.handle_id),
                Some(handle.generation),
                true,
                Some("Use the latest RuntimeAssetHandle returned by load before release."),
            );
            return Err(());
        };

        if stored.generation != handle.generation {
            self.push_error(
                request_id,
                &handle.asset_id,
                Some(handle.asset_guid.clone()),
                &handle.asset_type,
                AssetLoadStage::Release,
                AssetLoadErrorCode::ReleaseGenerationMismatch,
                Some(handle.bundle_id.clone()),
                Some(handle.cooked_asset_id.clone()),
                Some(handle.loader_kind.clone()),
                Vec::new(),
                Some(handle.handle_id),
                Some(handle.generation),
                true,
                Some("The handle generation is stale. Release the current handle only."),
            );
            return Err(());
        }

        let released = {
            let stored = self
                .handles
                .get_mut(&handle.handle_id)
                .expect("handle was checked above");
            stored.state = RuntimeAssetLoadState::Released;
            stored.ref_count = 0;
            stored.clone()
        };
        if let Some(decoded) = self.decoded_cache.get_mut(&released.cooked_asset_id) {
            decoded.ref_count = decoded.ref_count.saturating_sub(1);
        }
        self.decoded_cache
            .retain(|_, decoded| decoded.ref_count > 0);
        let dependency_handles = self
            .dependency_handles_by_owner
            .remove(&released.handle_id)
            .unwrap_or_default();
        for dependency_handle in dependency_handles {
            let _ = self.release(&dependency_handle);
        }
        self.diagnostics.push(AssetLoadDiagnostic {
            request_id,
            asset_ref_id: released.asset_id.clone(),
            asset_guid: Some(released.asset_guid.clone()),
            asset_type: released.asset_type.clone(),
            stage: AssetLoadStage::Release,
            state: AssetLoadState::Ok,
            error_code: None,
            bundle_id: Some(released.bundle_id.clone()),
            cooked_asset_id: Some(released.cooked_asset_id.clone()),
            loader_kind: Some(released.loader_kind.clone()),
            dependency_chain: Vec::new(),
            handle_id: Some(released.handle_id),
            generation: Some(released.generation),
            sync_load: true,
            recommended_action: None,
        });
        Ok(())
    }

    pub fn get_handle_state(&self, handle: &RuntimeAssetHandle) -> Option<RuntimeAssetLoadState> {
        self.handles
            .get(&handle.handle_id)
            .map(|stored| stored.state.clone())
    }

    pub fn diagnostics(&self) -> &AssetLoadDiagnostics {
        &self.diagnostics
    }

    pub fn decoded_cache_len(&self) -> usize {
        self.decoded_cache.len()
    }

    fn load_internal(
        &mut self,
        asset_ref: &RuntimeAssetRef,
        request_id: u64,
        sync_load: bool,
        dependency_chain: &mut Vec<String>,
        visiting: &mut HashSet<String>,
    ) -> Result<RuntimeAssetHandle, ()> {
        let record = match self.index.resolve(asset_ref).cloned() {
            Ok(record) => record,
            Err(error) => {
                let code = match error {
                    RuntimeAssetResolveError::MissingAssetRef => {
                        AssetLoadErrorCode::MissingAssetRef
                    }
                    RuntimeAssetResolveError::TypeMismatch { .. } => {
                        AssetLoadErrorCode::TypeMismatch
                    }
                    RuntimeAssetResolveError::SubAssetMissing => {
                        AssetLoadErrorCode::SubAssetMissing
                    }
                };
                self.push_error(
                    request_id,
                    &asset_ref.id,
                    asset_ref.guid.clone(),
                    &asset_ref.asset_type,
                    AssetLoadStage::Resolve,
                    code,
                    None,
                    None,
                    None,
                    dependency_chain.clone(),
                    None,
                    None,
                    sync_load,
                    Some("Check AssetRef id/guid/type against RuntimeAssetIndex."),
                );
                return Err(());
            }
        };

        if !visiting.insert(record.asset_guid.clone()) {
            dependency_chain.push(record.asset_guid.clone());
            self.push_error(
                request_id,
                &record.asset_id,
                Some(record.asset_guid.clone()),
                &record.asset_type,
                AssetLoadStage::Dependency,
                AssetLoadErrorCode::CyclicDependency,
                Some(record.bundle_id.clone()),
                Some(record.cooked_asset_id.clone()),
                Some(record.loader_kind.clone()),
                dependency_chain.clone(),
                None,
                None,
                sync_load,
                Some("Break the dependency cycle in build-time dependency data."),
            );
            dependency_chain.pop();
            return Err(());
        }

        dependency_chain.push(record.asset_guid.clone());
        let mut loaded_dependencies = Vec::new();
        for dependency_guid in &record.dependencies {
            let Some(dependency) = self.index.record_by_guid(dependency_guid).cloned() else {
                self.release_loaded_dependencies(loaded_dependencies);
                self.push_error(
                    request_id,
                    &record.asset_id,
                    Some(record.asset_guid.clone()),
                    &record.asset_type,
                    AssetLoadStage::Dependency,
                    AssetLoadErrorCode::DependencyMissing,
                    Some(record.bundle_id.clone()),
                    Some(record.cooked_asset_id.clone()),
                    Some(record.loader_kind.clone()),
                    dependency_chain.clone(),
                    None,
                    None,
                    sync_load,
                    Some("Rebuild Runtime Package dependency_table."),
                );
                visiting.remove(&record.asset_guid);
                dependency_chain.pop();
                return Err(());
            };
            let dependency_ref = RuntimeAssetRef {
                id: dependency.asset_id.clone(),
                asset_type: dependency.asset_type.clone(),
                guid: Some(dependency.asset_guid.clone()),
                sub_asset: dependency.sub_asset_id.clone(),
            };
            let dependency_handle = self.load_internal(
                &dependency_ref,
                request_id,
                sync_load,
                dependency_chain,
                visiting,
            )?;
            loaded_dependencies.push(dependency_handle);
        }
        dependency_chain.pop();
        visiting.remove(&record.asset_guid);

        if !self.mount_table.is_bundle_mounted(&record.bundle_id) {
            self.release_loaded_dependencies(loaded_dependencies);
            self.push_error(
                request_id,
                &record.asset_id,
                Some(record.asset_guid.clone()),
                &record.asset_type,
                AssetLoadStage::BundleMount,
                AssetLoadErrorCode::BundleNotMounted,
                Some(record.bundle_id.clone()),
                Some(record.cooked_asset_id.clone()),
                Some(record.loader_kind.clone()),
                Vec::new(),
                None,
                None,
                sync_load,
                Some("Mount the bundle before loading this asset."),
            );
            return Err(());
        }

        if self.ensure_decoded(request_id, &record, sync_load).is_err() {
            self.release_loaded_dependencies(loaded_dependencies);
            return Err(());
        }
        let prepared = PreparedRuntimeResource {
            resource_id: format!("prepared:{}:{}", record.loader_kind, record.cooked_asset_id),
            kind: prepared_kind(&record.loader_kind),
        };
        let handle = RuntimeAssetHandle {
            handle_id: self.next_handle_id(),
            asset_guid: record.asset_guid.clone(),
            asset_id: record.asset_id.clone(),
            asset_type: record.asset_type.clone(),
            sub_asset_id: record.sub_asset_id.clone(),
            cooked_asset_id: record.cooked_asset_id.clone(),
            bundle_id: record.bundle_id.clone(),
            runtime_resource_id: Some(prepared.resource_id),
            state: RuntimeAssetLoadState::Ready,
            generation: self.next_generation(),
            ref_count: 1,
            loader_kind: record.loader_kind.clone(),
            version: record.version.clone(),
        };
        self.handles.insert(handle.handle_id, handle.clone());
        if !loaded_dependencies.is_empty() {
            self.dependency_handles_by_owner
                .insert(handle.handle_id, loaded_dependencies);
        }
        if let Some(decoded) = self.decoded_cache.get_mut(&record.cooked_asset_id) {
            decoded.ref_count = decoded.ref_count.saturating_add(1);
        }
        self.diagnostics.push(AssetLoadDiagnostic {
            request_id,
            asset_ref_id: record.asset_id.clone(),
            asset_guid: Some(record.asset_guid.clone()),
            asset_type: record.asset_type.clone(),
            stage: AssetLoadStage::Apply,
            state: AssetLoadState::Ok,
            error_code: None,
            bundle_id: Some(record.bundle_id.clone()),
            cooked_asset_id: Some(record.cooked_asset_id.clone()),
            loader_kind: Some(record.loader_kind.clone()),
            dependency_chain: Vec::new(),
            handle_id: Some(handle.handle_id),
            generation: Some(handle.generation),
            sync_load,
            recommended_action: None,
        });
        Ok(handle)
    }

    fn release_loaded_dependencies(&mut self, handles: Vec<RuntimeAssetHandle>) {
        for handle in handles {
            let _ = self.release(&handle);
        }
    }

    fn ensure_decoded(
        &mut self,
        request_id: u64,
        record: &crate::runtime_asset::RuntimeAssetRecord,
        sync_load: bool,
    ) -> Result<(), ()> {
        if self.decoded_cache.contains_key(&record.cooked_asset_id) {
            return Ok(());
        }

        let Some(cooked) = self.index.cooked_asset(&record.cooked_asset_id).cloned() else {
            self.push_error(
                request_id,
                &record.asset_id,
                Some(record.asset_guid.clone()),
                &record.asset_type,
                AssetLoadStage::Read,
                AssetLoadErrorCode::CookedAssetMissing,
                Some(record.bundle_id.clone()),
                Some(record.cooked_asset_id.clone()),
                Some(record.loader_kind.clone()),
                Vec::new(),
                None,
                None,
                sync_load,
                Some("RuntimeAssetIndex points to a missing cooked_asset_table entry."),
            );
            return Err(());
        };

        let (bytes_len, source_debug) = if let Some(path) = cooked.path {
            let full_path = match safe_join_runtime_package(&self.package_dir, &path) {
                Ok(path) => path,
                Err(_) => {
                    self.push_error(
                        request_id,
                        &record.asset_id,
                        Some(record.asset_guid.clone()),
                        &record.asset_type,
                        AssetLoadStage::Read,
                        AssetLoadErrorCode::UnsafePackagePath,
                        Some(record.bundle_id.clone()),
                        Some(record.cooked_asset_id.clone()),
                        Some(record.loader_kind.clone()),
                        Vec::new(),
                        None,
                        None,
                        sync_load,
                        Some("Use a normalized package-relative cooked asset path."),
                    );
                    return Err(());
                }
            };
            match fs::read(&full_path) {
                Ok(bytes) => (bytes.len(), full_path.display().to_string()),
                Err(_) => {
                    self.push_error(
                        request_id,
                        &record.asset_id,
                        Some(record.asset_guid.clone()),
                        &record.asset_type,
                        AssetLoadStage::Read,
                        AssetLoadErrorCode::CookedAssetMissing,
                        Some(record.bundle_id.clone()),
                        Some(record.cooked_asset_id.clone()),
                        Some(record.loader_kind.clone()),
                        Vec::new(),
                        None,
                        None,
                        sync_load,
                        Some("Check cooked asset path inside Runtime Package."),
                    );
                    return Err(());
                }
            }
        } else {
            (
                record.size.unwrap_or(0) as usize,
                "compatibility_fake_decoded_asset".to_string(),
            )
        };

        self.decoded_cache.insert(
            record.cooked_asset_id.clone(),
            DecodedAsset {
                cooked_asset_id: record.cooked_asset_id.clone(),
                asset_type: record.asset_type.clone(),
                bytes_len,
                source_debug,
                ref_count: 0,
            },
        );
        Ok(())
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    fn next_handle_id(&mut self) -> u64 {
        let id = self.next_handle_id;
        self.next_handle_id += 1;
        id
    }

    fn next_generation(&mut self) -> u64 {
        let id = self.next_generation;
        self.next_generation += 1;
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn push_error(
        &mut self,
        request_id: u64,
        asset_ref_id: &str,
        asset_guid: Option<String>,
        asset_type: &str,
        stage: AssetLoadStage,
        error_code: AssetLoadErrorCode,
        bundle_id: Option<String>,
        cooked_asset_id: Option<String>,
        loader_kind: Option<String>,
        dependency_chain: Vec<String>,
        handle_id: Option<u64>,
        generation: Option<u64>,
        sync_load: bool,
        recommended_action: Option<&str>,
    ) {
        self.diagnostics.push(AssetLoadDiagnostic {
            request_id,
            asset_ref_id: asset_ref_id.to_string(),
            asset_guid,
            asset_type: asset_type.to_string(),
            stage,
            state: AssetLoadState::Failed,
            error_code: Some(error_code),
            bundle_id,
            cooked_asset_id,
            loader_kind,
            dependency_chain,
            handle_id,
            generation,
            sync_load,
            recommended_action: recommended_action.map(str::to_string),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_asset::{
        BundleRecord, CookedAssetRecord, RuntimeAssetRecord, RuntimePackageMountTable,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn bundle_must_be_mounted_before_load() {
        let mut loader = loader_with_records(vec![record(
            "guid-texture",
            "texture-main",
            "texture",
            "cooked-texture",
            vec![],
            "1",
        )]);
        let result = loader.load(&asset_ref("texture-main", "texture"));
        assert!(result.is_err());
        assert!(loader
            .diagnostics()
            .has_error(AssetLoadErrorCode::BundleNotMounted));
    }

    #[test]
    fn load_prefers_guid_when_id_is_stale() {
        let mut loader = loader_with_records(vec![record(
            "guid-texture",
            "texture-main",
            "texture",
            "cooked-texture",
            vec![],
            "1",
        )]);
        assert!(loader.mount_bundle("startup"));
        let handle = loader
            .load(&RuntimeAssetRef {
                id: "old-texture-id".to_string(),
                asset_type: "texture".to_string(),
                guid: Some("guid-texture".to_string()),
                sub_asset: None,
            })
            .unwrap();
        assert_eq!(handle.asset_id, "texture-main");
        assert_eq!(handle.asset_guid, "guid-texture");
    }

    #[test]
    fn load_reads_cooked_asset_file_from_package_dir() {
        let package_dir = temp_dir();
        fs::create_dir_all(package_dir.join("cooked")).unwrap();
        fs::write(
            package_dir.join("cooked").join("texture.bin"),
            [1_u8, 2, 3, 4],
        )
        .unwrap();
        let index = RuntimeAssetIndex::new(
            vec![record(
                "guid-texture",
                "texture-main",
                "texture",
                "cooked-texture",
                vec![],
                "1",
            )],
            vec![CookedAssetRecord {
                cooked_asset_id: "cooked-texture".to_string(),
                bundle_id: "startup".to_string(),
                path: Some("cooked/texture.bin".to_string()),
                offset: None,
                size: Some(4),
                compression: None,
                hash: None,
            }],
        );
        let mut loader = RuntimeAssetLoader::new(
            &package_dir,
            index,
            RuntimePackageMountTable::new(vec![BundleRecord {
                bundle_id: "startup".to_string(),
                mount_id: None,
                uri: "bundles/startup".to_string(),
                hash: None,
                version: None,
                mounted: false,
            }]),
        );
        assert!(loader.mount_bundle("startup"));
        let handle = loader.load(&asset_ref("texture-main", "texture")).unwrap();
        let decoded = loader
            .decoded_cache
            .get(&handle.cooked_asset_id)
            .expect("decoded asset should be cached");
        assert_eq!(decoded.bytes_len, 4);
        assert!(
            decoded.source_debug.ends_with("cooked\\texture.bin")
                || decoded.source_debug.ends_with("cooked/texture.bin")
        );
    }

    #[test]
    fn dependency_bundle_must_be_mounted_too() {
        let records = vec![
            RuntimeAssetRecord {
                bundle_id: "dependency-bundle".to_string(),
                ..record(
                    "guid-texture",
                    "texture-main",
                    "texture",
                    "cooked-texture",
                    vec![],
                    "1",
                )
            },
            record(
                "guid-material",
                "material-main",
                "material",
                "cooked-material",
                vec!["guid-texture".to_string()],
                "1",
            ),
        ];
        let cooked_assets = vec![
            CookedAssetRecord {
                bundle_id: "dependency-bundle".to_string(),
                ..cooked("cooked-texture")
            },
            cooked("cooked-material"),
        ];
        let index = RuntimeAssetIndex::new(records, cooked_assets);
        let mut loader = RuntimeAssetLoader::new(
            temp_dir(),
            index,
            RuntimePackageMountTable::new(vec![
                BundleRecord {
                    bundle_id: "startup".to_string(),
                    mount_id: None,
                    uri: "bundles/startup".to_string(),
                    hash: None,
                    version: None,
                    mounted: false,
                },
                BundleRecord {
                    bundle_id: "dependency-bundle".to_string(),
                    mount_id: None,
                    uri: "bundles/dependency-bundle".to_string(),
                    hash: None,
                    version: None,
                    mounted: false,
                },
            ]),
        );
        assert!(loader.mount_bundle("startup"));
        assert!(loader
            .load(&asset_ref("material-main", "material"))
            .is_err());
        assert!(loader
            .diagnostics()
            .has_error(AssetLoadErrorCode::BundleNotMounted));
    }

    #[test]
    fn loads_dependency_chain_after_mount() {
        let mut loader = loader_with_records(vec![
            record(
                "guid-texture",
                "texture-main",
                "texture",
                "cooked-texture",
                vec![],
                "1",
            ),
            record(
                "guid-material",
                "material-main",
                "material",
                "cooked-material",
                vec!["guid-texture".to_string()],
                "1",
            ),
            record(
                "guid-prefab",
                "prefab-ship",
                "prefab",
                "cooked-prefab",
                vec!["guid-material".to_string()],
                "1",
            ),
        ]);
        assert!(loader.mount_bundle("startup"));
        let handle = loader.load(&asset_ref("prefab-ship", "prefab")).unwrap();
        assert_eq!(handle.state, RuntimeAssetLoadState::Ready);
        assert_eq!(loader.decoded_cache_len(), 3);
    }

    #[test]
    fn missing_dependency_is_diagnostic() {
        let mut loader = loader_with_records(vec![record(
            "guid-material",
            "material-main",
            "material",
            "cooked-material",
            vec!["guid-missing".to_string()],
            "1",
        )]);
        assert!(loader.mount_bundle("startup"));
        assert!(loader
            .load(&asset_ref("material-main", "material"))
            .is_err());
        assert!(loader
            .diagnostics()
            .has_error(AssetLoadErrorCode::DependencyMissing));
    }

    #[test]
    fn cyclic_dependency_is_diagnostic() {
        let mut loader = loader_with_records(vec![
            record(
                "guid-a",
                "asset-a",
                "material",
                "cooked-a",
                vec!["guid-b".to_string()],
                "1",
            ),
            record(
                "guid-b",
                "asset-b",
                "material",
                "cooked-b",
                vec!["guid-a".to_string()],
                "1",
            ),
        ]);
        assert!(loader.mount_bundle("startup"));
        assert!(loader.load(&asset_ref("asset-a", "material")).is_err());
        assert!(loader
            .diagnostics()
            .has_error(AssetLoadErrorCode::CyclicDependency));
    }

    #[test]
    fn release_rejects_stale_generation() {
        let mut loader = loader_with_records(vec![record(
            "guid-texture",
            "texture-main",
            "texture",
            "cooked-texture",
            vec![],
            "1",
        )]);
        assert!(loader.mount_bundle("startup"));
        let mut handle = loader.load(&asset_ref("texture-main", "texture")).unwrap();
        handle.generation += 1;
        assert!(loader.release(&handle).is_err());
        assert!(loader
            .diagnostics()
            .has_error(AssetLoadErrorCode::ReleaseGenerationMismatch));
    }

    #[test]
    fn patch_index_applies_to_next_load_without_mutating_old_handle() {
        let mut loader = loader_with_records(vec![record(
            "guid-texture",
            "texture-main",
            "texture",
            "cooked-texture-v1",
            vec![],
            "1",
        )]);
        assert!(loader.mount_bundle("startup"));
        let v1 = loader.load(&asset_ref("texture-main", "texture")).unwrap();
        assert_eq!(v1.version, "1");

        loader.mount_patch_index(RuntimeAssetIndex::new(
            vec![record(
                "guid-texture",
                "texture-main",
                "texture",
                "cooked-texture-v2",
                vec![],
                "2",
            )],
            vec![cooked("cooked-texture-v2")],
        ));
        let v2 = loader.load(&asset_ref("texture-main", "texture")).unwrap();
        assert_eq!(v2.version, "2");
        assert_eq!(v1.version, "1");
        assert_ne!(v1.cooked_asset_id, v2.cooked_asset_id);
    }

    #[test]
    fn releasing_root_handle_releases_loaded_dependency_cache() {
        let mut loader = loader_with_records(vec![
            record(
                "guid-texture",
                "texture-main",
                "texture",
                "cooked-texture",
                vec![],
                "1",
            ),
            record(
                "guid-material",
                "material-main",
                "material",
                "cooked-material",
                vec!["guid-texture".to_string()],
                "1",
            ),
            record(
                "guid-prefab",
                "prefab-ship",
                "prefab",
                "cooked-prefab",
                vec!["guid-material".to_string()],
                "1",
            ),
        ]);
        assert!(loader.mount_bundle("startup"));
        let handle = loader.load(&asset_ref("prefab-ship", "prefab")).unwrap();
        assert_eq!(loader.decoded_cache_len(), 3);
        assert!(loader.release(&handle).is_ok());
        assert_eq!(loader.decoded_cache_len(), 0);
    }

    fn loader_with_records(records: Vec<RuntimeAssetRecord>) -> RuntimeAssetLoader {
        let cooked_assets = records
            .iter()
            .map(|record| cooked(&record.cooked_asset_id))
            .collect();
        let index = RuntimeAssetIndex::new(records, cooked_assets);
        let mount_table = RuntimePackageMountTable::new(vec![BundleRecord {
            bundle_id: "startup".to_string(),
            mount_id: None,
            uri: "bundles/startup".to_string(),
            hash: None,
            version: None,
            mounted: false,
        }]);
        RuntimeAssetLoader::new(temp_dir(), index, mount_table)
    }

    fn record(
        guid: &str,
        id: &str,
        asset_type: &str,
        cooked_asset_id: &str,
        dependencies: Vec<String>,
        version: &str,
    ) -> RuntimeAssetRecord {
        RuntimeAssetRecord {
            asset_guid: guid.to_string(),
            asset_id: id.to_string(),
            asset_type: asset_type.to_string(),
            sub_asset_id: None,
            version: version.to_string(),
            cooked_asset_id: cooked_asset_id.to_string(),
            bundle_id: "startup".to_string(),
            loader_kind: asset_type.to_string(),
            dependencies,
            hash: None,
            size: Some(16),
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    fn cooked(cooked_asset_id: &str) -> CookedAssetRecord {
        CookedAssetRecord {
            cooked_asset_id: cooked_asset_id.to_string(),
            bundle_id: "startup".to_string(),
            path: None,
            offset: None,
            size: Some(16),
            compression: None,
            hash: None,
        }
    }

    fn asset_ref(id: &str, asset_type: &str) -> RuntimeAssetRef {
        RuntimeAssetRef {
            id: id.to_string(),
            asset_type: asset_type.to_string(),
            guid: None,
            sub_asset: None,
        }
    }

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("runtime-asset-loader-{}", stamp))
    }
}
