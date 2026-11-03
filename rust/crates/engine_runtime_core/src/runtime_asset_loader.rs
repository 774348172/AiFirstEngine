#[cfg_attr(test, path = "runtime_asset_loader/streaming.rs")]
mod streaming;
use crate::runtime_asset::{
    prepared_kind, DecodedAsset, PreparedRuntimeResource, RuntimeAssetHandle, RuntimeAssetIndex,
    RuntimeAssetLoadState, RuntimeAssetResolveError, RuntimePackageMountTable,
};
use crate::runtime_asset_diagnostics::{
    AssetLoadDiagnostic, AssetLoadDiagnostics, AssetLoadErrorCode, AssetLoadStage, AssetLoadState,
};
use crate::runtime_package::RuntimeAssetRef;
use crate::runtime_package_source::RuntimePackageSource;
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(test)]
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
pub use streaming::{StreamingBudget, StreamingError, StreamingSnapshot};

#[cfg(test)]
thread_local! {
    static RELEASE_CACHE_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Debug)]
pub struct RuntimeAssetLoader {
    package_dir: PathBuf,
    source: RuntimePackageSource,
    index: RuntimeAssetIndex,
    mount_table: RuntimePackageMountTable,
    decoded_cache: HashMap<String, DecodedAsset>,
    handles: HashMap<u64, RuntimeAssetHandle>,
    released_handles: VecDeque<u64>,
    dependency_handles_by_owner: HashMap<u64, Vec<RuntimeAssetHandle>>,
    diagnostics: AssetLoadDiagnostics,
    next_request_id: u64,
    next_handle_id: u64,
    asset_revision_epoch: u64,
    external_cpu_reservation: usize,
    external_io_bytes: usize,
    external_io_operations: usize,
    streaming: streaming::StreamingState,
    cache_keys_by_handle: HashMap<u64, String>,
}

impl RuntimeAssetLoader {
    #[cfg(test)]
    pub(crate) fn gated_streaming_fixture(
        phase: usize,
    ) -> (Self, Arc<streaming::TestWorkerGate>, PathBuf) {
        let (mut loader, root) = tests::real_streaming_loader();
        let gate = Arc::new(streaming::TestWorkerGate::default());
        gate.phase
            .store(phase, std::sync::atomic::Ordering::Release);
        gate.blocked
            .store(true, std::sync::atomic::Ordering::Release);
        loader.streaming.gate = Some(gate.clone());
        (loader, gate, root)
    }

    /// Read-only authority shared with render contract resolution.
    pub fn asset_index(&self) -> &RuntimeAssetIndex {
        &self.index
    }

    pub(crate) fn package_dir(&self) -> &std::path::Path {
        &self.package_dir
    }

    pub(crate) fn release_texture_cpu_payload(&mut self, reference: &RuntimeAssetRef) {
        let Ok(record) = self.index.resolve(reference) else {
            return;
        };
        let key = self.cache_identity(record);
        let Some(cached) = self.decoded_cache.get_mut(&key) else {
            return;
        };
        if let Some(crate::runtime_asset::RuntimeDecodedPayload::Texture(texture)) =
            cached.payload.as_deref()
        {
            let bytes = texture.payload().rgba8.len();
            cached.payload = None;
            cached.bytes_len = cached.bytes_len.saturating_sub(bytes);
        }
    }

    pub fn new(
        package_dir: impl Into<PathBuf>,
        index: RuntimeAssetIndex,
        mount_table: RuntimePackageMountTable,
    ) -> Self {
        let package_dir = package_dir.into();
        Self {
            source: RuntimePackageSource::directory(&package_dir),
            package_dir,
            index,
            mount_table,
            decoded_cache: HashMap::new(),
            handles: HashMap::new(),
            released_handles: VecDeque::new(),
            dependency_handles_by_owner: HashMap::new(),
            diagnostics: AssetLoadDiagnostics::new(),
            next_request_id: 1,
            next_handle_id: 1,
            asset_revision_epoch: 1,
            external_cpu_reservation: 0,
            external_io_bytes: 0,
            external_io_operations: 0,
            streaming: streaming::StreamingState::default(),
            cache_keys_by_handle: HashMap::new(),
        }
    }

    /// Reuses the package validated at startup, including downloaded bytes.
    pub fn with_package_source(mut self, source: RuntimePackageSource) -> Self {
        self.package_dir = source.root().to_path_buf();
        self.source = source;
        self
    }

    /// Opt-in borrowed allocation census. Worker metadata is enabled by the first
    /// capture; dispatches made before that capture are explicitly uncovered.
    /// Reservations are never reported as allocated bytes.
    pub fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        self.observe_streaming_memory(sample);
        self.index.observe_memory(sample);
        self.mount_table.observe_memory(sample);
        self.diagnostics.observe_memory(sample);
        let (first, second) = self.released_handles.as_slices();
        let storage = if second.is_empty() { first } else { second };
        sample.cpu_storage(
            storage.as_ptr() as usize,
            self.released_handles.capacity() * std::mem::size_of::<u64>(),
            "assets",
            &["decodedCache"],
        );
        sample.cpu_storage(
            self.package_dir.as_os_str().as_encoded_bytes().as_ptr() as usize,
            self.package_dir.capacity(),
            "assets",
            &["resident"],
        );
        for value in self.cache_keys_by_handle.values() {
            sample.cpu_string(value, "assets", &["decodedCache"]);
        }
        for handles in self.dependency_handles_by_owner.values() {
            sample.cpu_vec(handles, "queues", &["queuePayload"]);
        }
        for handle in self
            .handles
            .values()
            .chain(self.dependency_handles_by_owner.values().flatten())
        {
            handle.observe_memory(sample, "assets", "decodedCache");
        }
        for (key, asset) in &self.decoded_cache {
            sample.cpu_string(key, "assets", &["decodedCache"]);
            asset.observe_memory(sample, "decodedCache");
        }
        sample.partial("cpu.owner.assets", "assets.index+mount+cache.raw+typedPayload+scenePrefabNested+consumerStrings;stdNodes+arcControlBlocks+allocatorOpaque");
        sample.partial(
            "cpu.role.decodedCache",
            "assets.cache.raw+typedPayload+scenePrefabNested;stdNodes+arcControlBlocksOpaque",
        );
    }

    pub fn mount_bundle(&mut self, bundle_id: &str) -> bool {
        self.mount_table.mount_bundle(bundle_id)
    }

    pub fn unmount_bundle(&mut self, bundle_id: &str) {
        self.asset_revision_epoch += 1;
        self.invalidate_pending_streams();
        self.mount_table.unmount_bundle(bundle_id);
    }

    pub fn mount_patch_index(&mut self, patch: RuntimeAssetIndex) {
        self.asset_revision_epoch += 1;
        self.invalidate_pending_streams();
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
        self.request_async(asset_ref).map_err(|_| ())
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

        if stored.generation != handle.generation || stored.state == RuntimeAssetLoadState::Released
        {
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

        let owns_decoded = self.owns_decoded_reference(handle.handle_id);
        self.detach_stream_ticket(handle.handle_id);
        let released = {
            let stored = self
                .handles
                .get_mut(&handle.handle_id)
                .expect("handle was checked above");
            stored.state = RuntimeAssetLoadState::Released;
            stored.ref_count = 0;
            stored.clone()
        };
        let key = self.cache_keys_by_handle.remove(&released.handle_id);
        if let Some(key) = key.filter(|_| owns_decoded) {
            #[cfg(test)]
            RELEASE_CACHE_VISITS.with(|visits| visits.set(visits.get() + 1));
            if let std::collections::hash_map::Entry::Occupied(mut entry) =
                self.decoded_cache.entry(key)
            {
                let decoded = entry.get_mut();
                decoded.ref_count = decoded.ref_count.saturating_sub(1);
                if decoded.ref_count == 0 {
                    entry.remove();
                }
            }
        }
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
        self.released_handles.push_back(released.handle_id);
        self.prune_released_handles(self.streaming_budget().max_handles);
        Ok(())
    }

    pub fn get_handle_state(&self, handle: &RuntimeAssetHandle) -> Option<RuntimeAssetLoadState> {
        self.handles
            .get(&handle.handle_id)
            .filter(|stored| stored.generation == handle.generation)
            .map(|stored| stored.state.clone())
    }

    pub fn diagnostics(&self) -> &AssetLoadDiagnostics {
        &self.diagnostics
    }

    pub fn decoded_cache_len(&self) -> usize {
        self.decoded_cache.len()
    }

    pub fn get_asset_bytes(&self, handle: &RuntimeAssetHandle) -> Option<Arc<[u8]>> {
        let stored = self.handles.get(&handle.handle_id)?;
        if stored.generation != handle.generation
            || stored.state != RuntimeAssetLoadState::Ready
            || stored.cooked_asset_id != handle.cooked_asset_id
            || stored.asset_guid != handle.asset_guid
        {
            return None;
        }
        let cache_key = self.cache_keys_by_handle.get(&stored.handle_id)?;
        let cached = self.decoded_cache.get(cache_key)?;
        if let Some(bytes) = &cached.bytes {
            return Some(bytes.clone());
        }
        if cached.asset_type != "mesh" {
            return None;
        }
        // Mesh preparation may relinquish the cache's raw CPU backing without
        // releasing the asset or its dependencies. Reacquire from this package
        // only while the complete record/cooked identity still matches; an old
        // patch handle must never read the replacement's source. Consumers keep
        // their existing payload validation, including hash and size checks.
        let record = self.index.record_by_guid(&stored.asset_guid)?;
        if self.cache_identity(record) != *cache_key {
            return None;
        }
        let cooked = self.index.cooked_asset(&record.cooked_asset_id)?;
        self.source
            .read(cooked.path.as_deref()?)
            .ok()
            .map(Arc::from)
    }

    pub(crate) fn acquire_render_asset_bytes(
        &mut self,
        handle: &RuntimeAssetHandle,
    ) -> Option<Arc<[u8]>> {
        let bytes = self.get_asset_bytes(handle)?;
        let cache_key = self.cache_keys_by_handle.get(&handle.handle_id)?;
        let cached = self.decoded_cache.get_mut(cache_key)?;
        if cached.asset_type == "mesh" {
            // The returned Arc and any existing readers own the bytes until
            // validation/decoding finishes. Keep typed payloads and asset refs;
            // only relinquish the serialized backing, and do not recache rereads.
            if let Some(raw) = cached.bytes.take() {
                cached.bytes_len = cached.bytes_len.saturating_sub(raw.len());
            }
        }
        Some(bytes)
    }

    pub fn load_texture_payload(
        &self,
        reference: &RuntimeAssetRef,
    ) -> Result<crate::runtime_texture::RuntimeTexturePayload, String> {
        if let Some(verified) = self.cached_verified_texture_payload(reference) {
            return Ok(verified.payload().as_ref().clone());
        }
        crate::runtime_texture::load_checked_runtime_texture_payload_from_source(
            &self.source,
            &self.index,
            reference,
        )
    }

    pub(crate) fn load_verified_texture_payload(
        &self,
        reference: &RuntimeAssetRef,
    ) -> Result<crate::runtime_texture::VerifiedTexturePayload, String> {
        if let Some(verified) = self.cached_verified_texture_payload(reference) {
            return Ok(verified);
        }
        crate::runtime_texture::load_checked_runtime_texture_payload_from_source(
            &self.source,
            &self.index,
            reference,
        )
        .map(crate::runtime_texture::VerifiedTexturePayload::new)
    }

    fn cached_verified_texture_payload(
        &self,
        reference: &RuntimeAssetRef,
    ) -> Option<crate::runtime_texture::VerifiedTexturePayload> {
        if let Ok(record) = self.index.resolve(reference) {
            if let Some(decoded) = self.decoded_cache.get(&self.cache_identity(record)) {
                if let Some(payload) = &decoded.payload {
                    if let crate::runtime_asset::RuntimeDecodedPayload::Texture(texture) =
                        payload.as_ref()
                    {
                        return Some(texture.clone());
                    }
                }
            }
        }
        None
    }

    pub fn get_audio_clip(
        &self,
        handle: &RuntimeAssetHandle,
    ) -> Option<Arc<crate::audio::DecodedAudioClip>> {
        let stored = self.handles.get(&handle.handle_id)?;
        if stored.generation != handle.generation
            || stored.state != RuntimeAssetLoadState::Ready
            || stored.asset_type != "audio"
            || stored.asset_guid != handle.asset_guid
            || stored.cooked_asset_id != handle.cooked_asset_id
        {
            return None;
        }
        self.decoded_cache
            .get(self.cache_keys_by_handle.get(&stored.handle_id)?)?
            .audio_clip
            .clone()
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
            let dependency_handle = match self.load_internal(
                &dependency_ref,
                request_id,
                sync_load,
                dependency_chain,
                visiting,
            ) {
                Ok(handle) => handle,
                Err(()) => {
                    self.release_loaded_dependencies(loaded_dependencies);
                    visiting.remove(&record.asset_guid);
                    dependency_chain.pop();
                    return Err(());
                }
            };
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
        let cache_key = self.cache_identity(&record);
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
        self.cache_keys_by_handle
            .insert(handle.handle_id, cache_key.clone());
        self.handles.insert(handle.handle_id, handle.clone());
        if !loaded_dependencies.is_empty() {
            self.dependency_handles_by_owner
                .insert(handle.handle_id, loaded_dependencies);
        }
        if let Some(decoded) = self.decoded_cache.get_mut(&cache_key) {
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
        let cache_key = self.cache_identity(record);
        if self.decoded_cache.contains_key(&cache_key) {
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

        let (bytes, source_debug) = if let Some(path) = cooked.path {
            let full_path = match self.source.resolve(&path) {
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
            match self.source.read_path(&full_path) {
                Ok(bytes) => (Some(bytes), full_path.display().to_string()),
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
            (None, "compatibility_fake_decoded_asset".to_string())
        };

        let audio_clip = if record.asset_type == "audio" {
            let decoded = bytes
                .as_deref()
                .ok_or_else(|| {
                    "Audio requires real cooked WAV bytes; metadata-only assets cannot play."
                        .to_string()
                })
                .and_then(crate::audio::decode_audio_wav);
            match decoded {
                Ok(clip) => Some(Arc::new(clip)),
                Err(message) => {
                    self.push_error(
                        request_id,
                        &record.asset_id,
                        Some(record.asset_guid.clone()),
                        &record.asset_type,
                        AssetLoadStage::Decode,
                        AssetLoadErrorCode::DecodeFailed,
                        Some(record.bundle_id.clone()),
                        Some(record.cooked_asset_id.clone()),
                        Some(record.loader_kind.clone()),
                        Vec::new(),
                        None,
                        None,
                        sync_load,
                        Some(&format!("{message} Rebuild the referenced audio asset.")),
                    );
                    return Err(());
                }
            }
        } else {
            None
        };
        let mut bytes_len = bytes
            .as_ref()
            .map_or(record.size.unwrap_or(0) as usize, Vec::len);

        // Synchronous scene hydration must publish the same immutable texture
        // payload as the worker path. Otherwise material preparation repeatedly
        // falls back to package I/O and retains one pixel copy per binding.
        // Legacy logical texture records are not cooked pixel descriptors.
        let is_cooked_texture = record.asset_type == "texture"
            && bytes
                .as_ref()
                .and_then(|data| serde_json::from_slice::<serde_json::Value>(data).ok())
                .is_some_and(|data| {
                    data["schemaVersion"] == crate::runtime_package::COOKED_TEXTURE_SCHEMA_VERSION
                });
        let payload = if record.asset_type == "environment" {
            let decoded = bytes
                .as_deref()
                .ok_or_else(|| "environment.bytes_missing".to_string())
                .and_then(crate::environment_lighting::EnvironmentMap::from_bytes);
            match decoded {
                Ok(map) => {
                    bytes_len = bytes_len.saturating_add(map.words().len() * 4);
                    Some(Arc::new(
                        crate::runtime_asset::RuntimeDecodedPayload::Environment(map),
                    ))
                }
                Err(message) => {
                    self.push_error(
                        request_id,
                        &record.asset_id,
                        Some(record.asset_guid.clone()),
                        &record.asset_type,
                        AssetLoadStage::Decode,
                        AssetLoadErrorCode::DecodeFailed,
                        Some(record.bundle_id.clone()),
                        Some(record.cooked_asset_id.clone()),
                        Some(record.loader_kind.clone()),
                        Vec::new(),
                        None,
                        None,
                        sync_load,
                        Some(&message),
                    );
                    return Err(());
                }
            }
        } else if is_cooked_texture {
            let reference = RuntimeAssetRef {
                id: record.asset_id.clone(),
                asset_type: record.asset_type.clone(),
                guid: Some(record.asset_guid.clone()),
                sub_asset: None,
            };
            match crate::runtime_texture::load_runtime_texture_payload_from_source(
                &self.source,
                &self.index,
                &reference,
            ) {
                Ok(texture) => {
                    bytes_len = bytes_len.saturating_add(texture.rgba8.len());
                    Some(Arc::new(
                        crate::runtime_asset::RuntimeDecodedPayload::Texture(
                            crate::runtime_texture::VerifiedTexturePayload::new(texture),
                        ),
                    ))
                }
                Err(error) => {
                    self.push_error(
                        request_id,
                        &record.asset_id,
                        Some(record.asset_guid.clone()),
                        &record.asset_type,
                        AssetLoadStage::Decode,
                        AssetLoadErrorCode::DecodeFailed,
                        Some(record.bundle_id.clone()),
                        Some(record.cooked_asset_id.clone()),
                        Some(record.loader_kind.clone()),
                        Vec::new(),
                        None,
                        None,
                        sync_load,
                        Some(&format!("{error:?} Rebuild the referenced cooked texture.")),
                    );
                    return Err(());
                }
            }
        } else {
            None
        };

        self.decoded_cache.insert(
            cache_key,
            DecodedAsset {
                cooked_asset_id: record.cooked_asset_id.clone(),
                asset_type: record.asset_type.clone(),
                bytes_len,
                source_debug,
                ref_count: 0,
                audio_clip,
                payload,
                bytes: if matches!(
                    record.asset_type.as_str(),
                    "particle-effect"
                        | "mesh"
                        | "model"
                        | "material"
                        | "character3d"
                        | "collision3d"
                        | "navigation3d"
                        | "vegetation3d"
                ) {
                    bytes.map(Arc::from)
                } else {
                    None
                },
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
        static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn asset_revision_epoch(&self) -> u64 {
        self.asset_revision_epoch
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
    fn loading_async_does_not_publish_ready_on_calling_thread() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let bytes = crate::audio::tests::audio_wav_fixture(1, 44_100);
        fs::write(root.join("sound.wav"), &bytes).unwrap();
        let mut payload = cooked("cooked-audio");
        payload.path = Some("sound.wav".into());
        payload.size = Some(bytes.len() as u64);
        payload.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
        let mut asset = record("guid-audio", "sound", "audio", "cooked-audio", vec![], "1");
        asset.size = payload.size;
        asset.hash = payload.hash.clone();
        let index = RuntimeAssetIndex::new(vec![asset], vec![payload]);
        let mut mounts = RuntimePackageMountTable::new(vec![BundleRecord {
            bundle_id: "startup".into(),
            mount_id: None,
            uri: "startup".into(),
            hash: None,
            version: None,
            mounted: true,
        }]);
        mounts.mount_bundle("startup");
        let mut loader = RuntimeAssetLoader::new(&root, index, mounts);
        let ticket = loader.load_async(&asset_ref("sound", "audio")).unwrap();
        assert_eq!(
            ticket.state,
            RuntimeAssetLoadState::Loading,
            "load_async must return before the worker result is applied"
        );
        loader.release(&ticket).unwrap();
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn audio_loader_shares_pcm_and_releases_only_after_last_handle() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("sound.wav"),
            crate::audio::tests::audio_wav_fixture(1, 44_100),
        )
        .unwrap();
        let mut cooked = cooked("cooked-audio");
        cooked.path = Some("sound.wav".into());
        let index = RuntimeAssetIndex::new(
            vec![record(
                "guid-audio",
                "audio-main",
                "audio",
                "cooked-audio",
                vec![],
                "1",
            )],
            vec![cooked],
        );
        let mut loader = RuntimeAssetLoader::new(
            &root,
            index,
            RuntimePackageMountTable::new(vec![BundleRecord {
                bundle_id: "startup".into(),
                mount_id: None,
                uri: "startup".into(),
                hash: None,
                version: None,
                mounted: true,
            }]),
        );
        let reference = RuntimeAssetRef {
            id: "audio-main".into(),
            asset_type: "audio".into(),
            guid: Some("guid-audio".into()),
            sub_asset: None,
        };
        let mut bad_ref = reference.clone();
        bad_ref.id = "different-id".into();
        assert!(loader.load(&bad_ref).is_err());
        bad_ref = reference.clone();
        bad_ref.guid = Some("missing-guid".into());
        assert!(loader.load(&bad_ref).is_err());
        let first = loader.load(&reference).unwrap();
        let second = loader.load(&reference).unwrap();
        let clip = loader.get_audio_clip(&first).unwrap();
        assert!(Arc::ptr_eq(&clip, &loader.get_audio_clip(&second).unwrap()));
        let weak = Arc::downgrade(&clip);
        drop(clip);
        loader.release(&first).unwrap();
        assert!(loader.get_audio_clip(&first).is_none());
        assert!(weak.upgrade().is_some());
        loader.release(&second).unwrap();
        assert!(weak.upgrade().is_none());
        assert_eq!(loader.decoded_cache_len(), 0);
        fs::remove_file(root.join("sound.wav")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn audio_loader_rejects_metadata_only_and_json_payloads() {
        let mut loader = loader_with_records(vec![record(
            "audio",
            "audio",
            "audio",
            "audio",
            vec![],
            "1",
        )]);
        loader.mount_bundle("startup");
        assert!(loader.load(&asset_ref("audio", "audio")).is_err());
        assert!(loader
            .diagnostics()
            .has_error(AssetLoadErrorCode::DecodeFailed));
        assert_eq!(loader.decoded_cache_len(), 0);
        fs::create_dir_all(&loader.package_dir).unwrap();
        fs::write(loader.package_dir.join("audio.json"), b"{}").unwrap();
        let mut payload = cooked("audio");
        payload.path = Some("audio.json".into());
        loader.mount_patch_index(RuntimeAssetIndex::new(
            vec![record("audio", "audio", "audio", "audio", vec![], "1")],
            vec![payload],
        ));
        assert!(loader.load(&asset_ref("audio", "audio")).is_err());
        assert_eq!(loader.decoded_cache_len(), 0);
        fs::remove_file(loader.package_dir.join("audio.json")).unwrap();
        fs::remove_dir(&loader.package_dir).unwrap();
    }

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
            .get(loader.cache_keys_by_handle.get(&handle.handle_id).unwrap())
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

    pub(super) fn real_streaming_loader() -> (RuntimeAssetLoader, PathBuf) {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let bytes = crate::audio::tests::audio_wav_fixture(1, 44_100);
        fs::write(root.join("sound.wav"), &bytes).unwrap();
        let mut c = cooked("cooked-audio");
        c.path = Some("sound.wav".into());
        c.size = Some(bytes.len() as u64);
        c.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
        let mut r = record("guid-audio", "sound", "audio", "cooked-audio", vec![], "1");
        r.size = c.size;
        r.hash = c.hash.clone();
        let index = RuntimeAssetIndex::new(vec![r], vec![c]);
        let mut loader = RuntimeAssetLoader::new(
            &root,
            index,
            RuntimePackageMountTable::new(vec![BundleRecord {
                bundle_id: "startup".into(),
                mount_id: None,
                uri: "startup".into(),
                hash: None,
                version: None,
                mounted: true,
            }]),
        );
        loader
            .set_streaming_budget(StreamingBudget {
                max_asset_bytes: 65536,
                max_decode_bytes: 65536,
                max_in_flight_bytes: 1024 * 1024,
                max_cpu_resident_bytes: 1024 * 1024,
                ..Default::default()
            })
            .unwrap();
        (loader, root)
    }
    #[test]
    fn streaming_memory_observes_real_decode_ready_cancel_failure_and_drain() {
        use crate::resource_memory::ResourceMemorySample;
        use std::sync::atomic::Ordering;
        fn census(loader: &RuntimeAssetLoader) -> ResourceMemorySample {
            let mut s = ResourceMemorySample::new(1, 1, 0);
            loader.observe_streaming_memory(&mut s);
            s
        }
        fn wait_for(mut condition: impl FnMut() -> bool) {
            let start = crate::platform_time::Instant::now();
            while !condition() {
                assert!(
                    start.elapsed() < std::time::Duration::from_secs(3),
                    "worker progress timeout"
                );
                std::thread::yield_now();
            }
        }
        for outcome in ["success", "cancel", "failure"] {
            let (mut loader, root) = real_streaming_loader();
            if outcome == "failure" {
                fs::write(root.join("sound.wav"), b"corrupt").unwrap();
            }
            let gate = Arc::new(streaming::TestWorkerGate::default());
            gate.blocked.store(true, Ordering::Release);
            loader.streaming.gate = Some(gate.clone());
            let ticket = loader.request_async(&asset_ref("sound", "audio")).unwrap();
            wait_for(|| gate.entered.load(Ordering::Acquire));
            let s = census(&loader);
            assert_eq!(s.capacities["activeRequests"], 1);
            assert_eq!(s.capacities["concurrentDecodes"], 1);
            assert_eq!(s.capacities["readyQueue"], 0);
            assert_eq!(s.capacities["failureCache"], 0);
            if outcome == "cancel" {
                loader.release(&ticket).unwrap();
            }
            gate.blocked.store(false, Ordering::Release);
            wait_for(|| census(&loader).capacities["readyQueue"] == 1);
            let s = census(&loader);
            assert_eq!(s.capacities["concurrentDecodes"], 0);
            drain_stream(&mut loader);
            let s = census(&loader);
            assert_eq!(s.capacities["activeRequests"], 0);
            assert_eq!(s.capacities["readyQueue"], 0);
            assert_eq!(s.capacities["concurrentDecodes"], 0);
            assert_eq!(
                s.capacities["failureCache"],
                u64::from(outcome == "failure")
            );
            assert_eq!(
                s.coverage["cpu.owner.queues"],
                crate::resource_memory::MemoryCoverage::Partial
            );
            if outcome != "cancel" {
                loader.release(&ticket).unwrap();
            }
            assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn loading_filesystem_path_resolution_is_worker_owned() {
        use std::sync::atomic::Ordering;
        let (mut loader, root) = real_streaming_loader();
        loader.package_dir = root.join("not-created");
        loader.source = RuntimePackageSource::directory(&loader.package_dir);
        let gate = Arc::new(streaming::TestWorkerGate::default());
        gate.blocked.store(true, Ordering::Release);
        loader.streaming.gate = Some(gate.clone());
        let ticket = loader.request_async(&asset_ref("sound", "audio")).expect(
            "request admission may validate path syntax, but must not touch the filesystem",
        );
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Loading)
        );
        gate.blocked.store(false, Ordering::Release);
        drain_stream(&mut loader);
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Failed)
        );
        assert_eq!(
            loader.streaming_error(&ticket),
            Some(&StreamingError::UnsafePath)
        );
        loader.release(&ticket).unwrap();
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_async_completion_preserves_a_sync_consumer_of_the_same_generation() {
        use std::sync::atomic::Ordering;
        let (mut loader, root) = real_streaming_loader();
        let gate = Arc::new(streaming::TestWorkerGate::default());
        gate.blocked.store(true, Ordering::Release);
        loader.streaming.gate = Some(gate.clone());
        let asynchronous = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let synchronous = loader.load(&asset_ref("sound", "audio")).unwrap();
        assert!(loader.get_audio_clip(&synchronous).is_some());
        loader.poll_async();
        assert_eq!(
            loader.get_handle_state(&asynchronous),
            Some(RuntimeAssetLoadState::Loading)
        );
        gate.blocked.store(false, Ordering::Release);
        drain_stream(&mut loader);
        loader.release(&asynchronous).unwrap();
        assert!(
            loader.get_audio_clip(&synchronous).is_some(),
            "worker completion must not overwrite a resident consumer's ownership"
        );
        loader.release(&synchronous).unwrap();
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_cancel_before_completion_does_not_release_a_sync_consumers_cache() {
        use std::sync::atomic::Ordering;
        let (mut loader, root) = real_streaming_loader();
        let gate = Arc::new(streaming::TestWorkerGate::default());
        gate.blocked.store(true, Ordering::Release);
        loader.streaming.gate = Some(gate.clone());
        let asynchronous = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let synchronous = loader.load(&asset_ref("sound", "audio")).unwrap();
        loader.release(&asynchronous).unwrap();
        assert!(loader.get_audio_clip(&synchronous).is_some());
        gate.blocked.store(false, Ordering::Release);
        drain_stream(&mut loader);
        assert!(loader.get_audio_clip(&synchronous).is_some());
        loader.release(&synchronous).unwrap();
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_worker_rejects_unknown_prefab_schema_before_cpu_ready() {
        let (mut loader, root) = real_streaming_loader();
        let bytes = br#"{"schemaVersion":"runtime-prefab.v999","id":"sound","name":"Unknown","rootEntityId":null,"entities":[]}"#;
        fs::write(root.join("sound.wav"), bytes).unwrap();
        let mut r = loader.index.record_by_guid("guid-audio").unwrap().clone();
        r.asset_type = "prefab".into();
        r.loader_kind = "prefab".into();
        r.size = Some(bytes.len() as u64);
        r.hash = Some(crate::canonical_digest::sha256_prefixed(bytes));
        let mut c = loader.index.cooked_asset("cooked-audio").unwrap().clone();
        c.size = r.size;
        c.hash = r.hash.clone();
        loader.mount_patch_index(RuntimeAssetIndex::new(vec![r], vec![c]));
        let handle = loader.request_async(&asset_ref("sound", "prefab")).unwrap();
        drain_stream(&mut loader);
        assert_eq!(
            loader.get_handle_state(&handle),
            Some(RuntimeAssetLoadState::Failed)
        );
        assert!(
            matches!(loader.streaming_error(&handle), Some(StreamingError::DecodeFailed(message)) if message.contains("schema_unsupported"))
        );
        loader.release(&handle).unwrap();
        assert_eq!(loader.decoded_cache_len(), 0);
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_dependency_edges_and_existing_cpu_reservations_obey_admission_budget() {
        let (mut loader, root) = real_streaming_loader();
        let mut r = loader.index.record_by_guid("guid-audio").unwrap().clone();
        let mut other = r.clone();
        other.asset_guid = "guid-other".into();
        other.asset_id = "other".into();
        r.dependencies = vec!["guid-other".into(); 3];
        loader.mount_patch_index(RuntimeAssetIndex::new(vec![r, other], vec![]));
        let mut budget = loader.streaming_budget();
        budget.max_dependency_edges = 2;
        loader.set_streaming_budget(budget).unwrap();
        assert_eq!(
            loader.request_async(&asset_ref("sound", "audio")),
            Err(StreamingError::BudgetExceeded)
        );
        assert_eq!(loader.streaming_snapshot().dispatched_operations, 0);
        loader.reserve_prepared_cpu(131072).unwrap();
        budget.max_cpu_resident_bytes = 65536;
        assert_eq!(
            loader.set_streaming_budget(budget),
            Err(StreamingError::BudgetExceeded)
        );
        assert_eq!(
            loader.streaming_budget().max_cpu_resident_bytes,
            1024 * 1024
        );
        loader.release_prepared_cpu(131072);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_three_completion_orders_never_publish_an_incomplete_closure() {
        use std::sync::atomic::Ordering;
        for order in 0..3 {
            let (mut loader, root) = real_streaming_loader();
            let mut budget = loader.streaming_budget();
            budget.max_completions_per_poll = 1;
            loader.set_streaming_budget(budget).unwrap();
            let mut other = loader.index.record_by_guid("guid-audio").unwrap().clone();
            other.asset_guid = "guid-other".into();
            other.asset_id = "other".into();
            other.cooked_asset_id = "cooked-other".into();
            let mut other_cooked = loader.index.cooked_asset("cooked-audio").unwrap().clone();
            other_cooked.cooked_asset_id = "cooked-other".into();
            let bytes = br#"{"schemaVersion":"runtime-prefab.v1","id":"root","name":"Root","rootEntityId":null,"entities":[]}"#;
            fs::write(root.join("root.json"), bytes).unwrap();
            let mut record = record(
                "guid-root",
                "root",
                "prefab",
                "cooked-root",
                vec!["guid-audio".into(), "guid-other".into()],
                "1",
            );
            record.size = Some(bytes.len() as u64);
            record.hash = Some(crate::canonical_digest::sha256_prefixed(bytes));
            let mut cooked = cooked("cooked-root");
            cooked.path = Some("root.json".into());
            cooked.size = record.size;
            cooked.hash = record.hash.clone();
            loader.mount_patch_index(RuntimeAssetIndex::new(
                vec![record, other],
                vec![cooked, other_cooked],
            ));
            let gate = Arc::new(streaming::TestWorkerGate::default());
            gate.blocked.store(true, Ordering::Release);
            loader.streaming.gate = Some(gate.clone());
            let mut consumers = Vec::new();
            if order != 0 {
                consumers.push(loader.request_async(&asset_ref("sound", "audio")).unwrap());
                let start = crate::platform_time::Instant::now();
                while !gate.entered.load(Ordering::Acquire) {
                    assert!(start.elapsed() < std::time::Duration::from_secs(3));
                    std::thread::yield_now();
                }
            }
            if order == 2 {
                consumers.push(
                    loader
                        .request_async_priority(&asset_ref("other", "audio"), 10)
                        .unwrap(),
                );
            }
            let ticket = loader.request_async(&asset_ref("root", "prefab")).unwrap();
            gate.blocked.store(false, Ordering::Release);
            let start = crate::platform_time::Instant::now();
            while loader.streaming_snapshot().pending_tickets != 0
                || loader.streaming_snapshot().pending_operations != 0
            {
                loader.poll_async();
                if loader.get_handle_state(&ticket) == Some(RuntimeAssetLoadState::Ready) {
                    assert!(loader.closure_contains_ready(&ticket, &asset_ref("sound", "audio")));
                    assert!(loader.closure_contains_ready(&ticket, &asset_ref("other", "audio")));
                }
                assert!(start.elapsed() < std::time::Duration::from_secs(3));
                std::thread::yield_now();
            }
            let completed = gate.completed.lock().unwrap().clone();
            assert_eq!(
                completed,
                match order {
                    0 => vec!["root", "sound", "other"],
                    1 => vec!["sound", "root", "other"],
                    _ => vec!["sound", "other", "root"],
                }
            );
            loader.release(&ticket).unwrap();
            for consumer in consumers {
                loader.release(&consumer).unwrap();
            }
            assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
            assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
            fs::remove_dir_all(root).unwrap();
        }
    }
    fn drain_stream(loader: &mut RuntimeAssetLoader) {
        let start = crate::platform_time::Instant::now();
        while loader.streaming_snapshot().pending_operations != 0
            || loader.streaming_snapshot().pending_tickets != 0
        {
            loader.poll_async();
            assert!(
                start.elapsed() < std::time::Duration::from_secs(3),
                "{:?}",
                loader.streaming_snapshot()
            );
            std::thread::yield_now();
        }
    }
    #[test]
    fn loading_shared_worker_has_independent_consumers_and_no_caller_io() {
        use std::sync::atomic::Ordering;
        let (mut loader, root) = real_streaming_loader();
        let gate = Arc::new(streaming::TestWorkerGate::default());
        gate.blocked.store(true, Ordering::Release);
        loader.streaming.gate = Some(gate.clone());
        let a = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let b = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let c = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let start = crate::platform_time::Instant::now();
        while !gate.entered.load(Ordering::Acquire) {
            assert!(start.elapsed() < std::time::Duration::from_secs(3));
            std::thread::yield_now();
        }
        assert_ne!(
            gate.thread.lock().unwrap().unwrap(),
            std::thread::current().id()
        );
        assert_eq!(loader.streaming_snapshot().dispatched_operations, 1);
        assert!(loader.get_audio_clip(&a).is_none());
        loader.release(&a).unwrap();
        assert!(
            loader.release(&a).is_err(),
            "double release must not consume B's reference"
        );
        gate.blocked.store(false, Ordering::Release);
        drain_stream(&mut loader);
        assert_eq!(
            loader.get_handle_state(&b),
            Some(RuntimeAssetLoadState::Ready)
        );
        let pcm = loader.get_audio_clip(&b).unwrap();
        assert!(Arc::ptr_eq(&pcm, &loader.get_audio_clip(&c).unwrap()));
        loader.release(&b).unwrap();
        assert_eq!(
            loader.get_handle_state(&c),
            Some(RuntimeAssetLoadState::Ready),
            "post-completion cancellation of B must preserve the third consumer"
        );
        fs::remove_file(root.join("sound.wav")).unwrap();
        let sync = loader.load(&asset_ref("sound", "audio")).unwrap();
        assert!(
            Arc::ptr_eq(&pcm, &loader.get_audio_clip(&sync).unwrap()),
            "same original cache must serve synchronous consumers without re-read"
        );
        loader.release(&sync).unwrap();
        loader.release(&c).unwrap();
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_all_cancelled_late_result_cannot_resurrect_and_reload_uses_new_generation() {
        use std::sync::atomic::Ordering;
        let (mut loader, gate, root) = RuntimeAssetLoader::gated_streaming_fixture(0);
        let a = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let b = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let third = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        loader.release(&a).unwrap();
        loader.release(&b).unwrap();
        loader.release(&third).unwrap();
        let c = loader
            .request_async(&asset_ref("sound", "audio"))
            .expect("a cancelled generation must not block an independently admitted retry");
        assert_ne!(c.generation, a.generation);
        assert_eq!(loader.streaming_snapshot().pending_operations, 2);
        gate.blocked.store(false, Ordering::Release);
        drain_stream(&mut loader);
        assert_eq!(loader.decoded_cache_len(), 1);
        assert_eq!(
            loader.get_handle_state(&c),
            Some(RuntimeAssetLoadState::Ready)
        );
        assert_ne!(c.generation, a.generation);
        let mut stale = c.clone();
        stale.generation = a.generation;
        assert!(loader.get_handle_state(&stale).is_none());
        assert!(loader.get_audio_clip(&stale).is_none());
        assert!(loader.release(&stale).is_err());
        loader.release(&c).unwrap();
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_budget_and_integrity_fail_before_publication() {
        let (mut loader, root) = real_streaming_loader();
        let mut budget = loader.streaming_budget();
        budget.max_asset_bytes = 1;
        loader.set_streaming_budget(budget).unwrap();
        assert_eq!(
            loader.request_async(&asset_ref("sound", "audio")),
            Err(StreamingError::BudgetExceeded)
        );
        assert_eq!(loader.streaming_snapshot().dispatched_operations, 0);
        budget.max_asset_bytes = 65536;
        loader.set_streaming_budget(budget).unwrap();
        let mut corrupt = fs::read(root.join("sound.wav")).unwrap();
        corrupt[0] ^= 1;
        fs::write(root.join("sound.wav"), corrupt).unwrap();
        let ticket = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        drain_stream(&mut loader);
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Failed)
        );
        assert_eq!(
            loader.streaming_error(&ticket),
            Some(&StreamingError::IntegrityMismatch)
        );
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        loader.release(&ticket).unwrap();
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_patch_invalidates_pending_without_overwriting_new_revision() {
        use std::sync::atomic::Ordering;
        let (mut loader, root) = real_streaming_loader();
        let gate = Arc::new(streaming::TestWorkerGate::default());
        gate.blocked.store(true, Ordering::Release);
        loader.streaming.gate = Some(gate.clone());
        let old = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        let mut r = loader.index.record_by_guid("guid-audio").unwrap().clone();
        r.version = "2".into();
        let c = loader.index.cooked_asset("cooked-audio").unwrap().clone();
        loader.mount_patch_index(RuntimeAssetIndex::new(vec![r], vec![c]));
        assert_eq!(
            loader.streaming_error(&old),
            Some(&StreamingError::RevisionChanged)
        );
        let new = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        gate.blocked.store(false, Ordering::Release);
        drain_stream(&mut loader);
        assert_eq!(
            loader.get_handle_state(&old),
            Some(RuntimeAssetLoadState::Failed)
        );
        assert_eq!(loader.current_handle(&new).unwrap().version, "2");
        loader.release(&old).unwrap();
        loader.release(&new).unwrap();
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loading_prefab_and_scene_parser_backing_is_borrowed_and_removed_on_success_cancel_and_unwind(
    ) {
        use crate::platform_time::Instant;
        use crate::resource_memory::ResourceMemorySample;
        use std::sync::atomic::Ordering;
        use std::time::Duration;
        fn census(loader: &RuntimeAssetLoader) -> ResourceMemorySample {
            let mut sample = ResourceMemorySample::new(1, 0, 0);
            loader.observe_memory(&mut sample);
            sample
        }
        for asset_type in ["prefab", "scene"] {
            for outcome in ["success", "off", "cancel", "panic"] {
                let (mut loader, root) = real_streaming_loader();
                eprintln!("parser-backing fixture: {}", root.display());
                let bytes = serde_json::to_vec(&serde_json::json!({
                    "schemaVersion":format!("runtime-{asset_type}.v1"), "id":"parser-prefab",
                    "gravity":0.0,"background":"#000000","skyColor":"#000000",
                    "name":"owned-parser-name-backing", "rootEntityId":"root", "entities":[]
                }))
                .unwrap();
                fs::write(root.join("prefab.json"), &bytes).unwrap();
                let mut record = loader.index.record_by_guid("guid-audio").unwrap().clone();
                let mut cooked = loader
                    .index
                    .cooked_asset(&record.cooked_asset_id)
                    .unwrap()
                    .clone();
                record.asset_id = "parser-prefab".into();
                record.asset_type = asset_type.into();
                record.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
                record.size = Some(bytes.len() as u64);
                cooked.hash = record.hash.clone();
                cooked.size = record.size;
                cooked.path = Some("prefab.json".into());
                loader.index = RuntimeAssetIndex::new(vec![record], vec![cooked]);
                if outcome != "off" {
                    census(&loader);
                }
                let gate = Arc::new(streaming::TestWorkerGate::default());
                gate.phase.store(5, Ordering::Release);
                gate.blocked.store(true, Ordering::Release);
                loader.streaming.gate = Some(gate.clone());
                let ticket = loader
                    .request_async(&asset_ref("parser-prefab", asset_type))
                    .unwrap();
                let started = Instant::now();
                while !gate.entered.load(Ordering::Acquire) {
                    assert!(started.elapsed() < Duration::from_secs(3));
                    std::thread::yield_now();
                }
                let id = format!(
                    "cpu-storage:{:x}",
                    gate.decoded_backing_address.load(Ordering::Acquire)
                );
                let sample = census(&loader);
                let allocation = sample.allocations.iter().find(|a| a.allocation_id == id);
                if outcome == "off" {
                    assert!(
                        allocation.is_none(),
                        "Off must not retrospectively scan parser backing"
                    );
                } else {
                    let a = allocation.expect("live parsed String backing missing");
                    assert!(a.roles.contains("decodeWork"));
                    assert_eq!(
                        a.bytes,
                        gate.parser_backing_bytes.load(Ordering::Acquire) as u64
                    );
                }
                if outcome == "cancel" {
                    loader.release(&ticket).unwrap();
                }
                if outcome == "panic" {
                    gate.panic_on_resume.store(true, Ordering::Release);
                }
                gate.blocked.store(false, Ordering::Release);
                drain_stream(&mut loader);
                assert!(
                    !census(&loader)
                        .allocations
                        .iter()
                        .any(|a| a.allocation_id == id && a.roles.contains("decodeWork")),
                    "parser observation outlived its borrowed Value"
                );
                if matches!(outcome, "success" | "off") {
                    assert_eq!(
                        loader.get_handle_state(&ticket),
                        Some(RuntimeAssetLoadState::Ready)
                    );
                }
                if outcome != "cancel" {
                    loader.release(&ticket).unwrap();
                }
                assert!(loader.shutdown_streaming(Duration::from_secs(2)));
                fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn loading_decoded_backing_transfers_from_worker_to_queue_and_cache_without_stale_observation()
    {
        use crate::platform_time::Instant;
        use crate::resource_memory::ResourceMemorySample;
        use std::sync::atomic::Ordering;
        use std::time::Duration;
        fn census(loader: &RuntimeAssetLoader) -> ResourceMemorySample {
            let mut sample = ResourceMemorySample::new(1, 0, 0);
            loader.observe_memory(&mut sample);
            sample
        }
        for outcome in ["success", "late-capture", "cancel", "panic"] {
            let (mut loader, root) = real_streaming_loader();
            eprintln!("decoded-backing fixture: {}", root.display());
            if outcome != "late-capture" {
                census(&loader); // Enable future worker transitions, no retroactive scan.
            }
            let gate = Arc::new(streaming::TestWorkerGate::default());
            gate.phase.store(4, Ordering::Release);
            gate.blocked.store(true, Ordering::Release);
            loader.streaming.gate = Some(gate.clone());
            let ticket = loader.request_async(&asset_ref("sound", "audio")).unwrap();
            let start = Instant::now();
            while !gate.entered.load(Ordering::Acquire) {
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::yield_now();
            }
            let id = format!(
                "cpu-storage:{:x}",
                gate.decoded_backing_address.load(Ordering::Acquire)
            );
            let sample = census(&loader);
            let allocation = sample.allocations.iter().find(|a| a.allocation_id == id);
            if outcome == "late-capture" {
                assert!(allocation.is_none(), "Off must not scan finished payloads");
            } else {
                let allocation = allocation.expect(
                    "actual decoded PCM backing must be visible before completion publication",
                );
                assert_eq!(allocation.bytes, 4 * std::mem::size_of::<f32>() as u64);
                assert!(allocation.roles.contains("decodeWork"));
            }
            if outcome == "cancel" {
                loader.release(&ticket).unwrap();
            } else if outcome == "panic" {
                gate.panic_on_resume.store(true, Ordering::Release);
            }
            gate.blocked.store(false, Ordering::Release);
            if matches!(outcome, "success" | "late-capture") {
                loop {
                    let sample = census(&loader);
                    if sample.capacities["readyQueue"] == 1 {
                        let allocation = sample
                            .allocations
                            .iter()
                            .find(|a| a.allocation_id == id)
                            .unwrap();
                        assert!(allocation.roles.contains("queuePayload"));
                        assert!(!allocation.roles.contains("decodeWork"));
                        break;
                    }
                    assert!(start.elapsed() < Duration::from_secs(3));
                    std::thread::yield_now();
                }
            }
            drain_stream(&mut loader);
            let sample = census(&loader);
            if matches!(outcome, "success" | "late-capture") {
                let allocation = sample
                    .allocations
                    .iter()
                    .find(|a| a.allocation_id == id)
                    .unwrap();
                assert!(allocation.roles.contains("decodedCache"));
                assert!(
                    !allocation.roles.contains("decodeWork")
                        && !allocation.roles.contains("queuePayload")
                );
            } else {
                assert!(!sample.allocations.iter().any(|a| a.allocation_id == id));
            }
            if outcome != "cancel" {
                loader.release(&ticket).unwrap();
            }
            assert!(loader.shutdown_streaming(Duration::from_secs(2)));
            assert!(!census(&loader)
                .allocations
                .iter()
                .any(|a| a.allocation_id == id));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn loading_worker_loss_closes_admission_and_releases_only_after_real_exit() {
        use crate::platform_time::Instant;
        use std::sync::atomic::Ordering;
        use std::time::Duration;
        for phase in [0, 1, 2] {
            let (mut loader, root) = real_streaming_loader();
            eprintln!("worker-loss fixture: {}", root.display());
            let gate = Arc::new(streaming::TestWorkerGate::default());
            gate.phase.store(phase, Ordering::Release);
            gate.blocked.store(true, Ordering::Release);
            loader.streaming.gate = Some(gate.clone());
            let cancelled = loader.request_async(&asset_ref("sound", "audio")).unwrap();
            let first = loader.request_async(&asset_ref("sound", "audio")).unwrap();
            let second = loader.request_async(&asset_ref("sound", "audio")).unwrap();
            loader.release(&cancelled).unwrap();
            let start = Instant::now();
            while !gate.entered.load(Ordering::Acquire) {
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::yield_now();
            }
            assert_eq!(loader.streaming_snapshot().dispatched_operations, 1);
            assert!(loader.streaming_snapshot().reserved_bytes > 0);
            gate.panic_on_resume.store(true, Ordering::Release);
            gate.blocked.store(false, Ordering::Release);
            while loader.streaming_error(&first).is_none() {
                loader.poll_async();
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::yield_now();
            }
            assert_eq!(
                loader.streaming_error(&first),
                Some(&StreamingError::WorkerStopped)
            );
            assert_eq!(
                loader.streaming_error(&second),
                Some(&StreamingError::WorkerStopped)
            );
            assert_eq!(
                loader.get_handle_state(&cancelled),
                Some(RuntimeAssetLoadState::Released)
            );
            assert_eq!(
                loader.request_async(&asset_ref("sound", "audio")),
                Err(StreamingError::WorkerStopped)
            );
            while loader.streaming_snapshot().pending_operations != 0 {
                loader.poll_async();
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::yield_now();
            }
            assert_eq!(loader.streaming_snapshot().pending_tickets, 0);
            assert_eq!(loader.streaming_snapshot().reserved_bytes, 0);
            assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
            assert_eq!(loader.decoded_cache_len(), 0);
            assert_eq!(
                loader.get_handle_state(&first),
                Some(RuntimeAssetLoadState::Failed)
            );
            assert_eq!(
                loader.get_handle_state(&second),
                Some(RuntimeAssetLoadState::Failed)
            );
            assert!(loader.shutdown_streaming(Duration::from_secs(2)));
            assert_eq!(
                loader.streaming_error(&first),
                Some(&StreamingError::WorkerStopped)
            );
            loader.release(&first).unwrap();
            loader.release(&second).unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn release_visits_only_its_shared_cache_entry_at_all_resident_scales() {
        for count in [1, 16, 128, 1024] {
            let records = (0..count)
                .map(|i| {
                    record(
                        &format!("guid-{i}"),
                        &format!("asset-{i}"),
                        "texture",
                        &format!("cooked-{i}"),
                        vec![],
                        "1",
                    )
                })
                .collect();
            let mut loader = loader_with_records(records);
            loader.mount_bundle("startup");
            let handles: Vec<_> = (0..count)
                .map(|i| {
                    loader
                        .load(&asset_ref(&format!("asset-{i}"), "texture"))
                        .unwrap()
                })
                .collect();
            let shared = loader.load(&asset_ref("asset-0", "texture")).unwrap();
            RELEASE_CACHE_VISITS.with(|v| v.set(0));
            loader.release(&handles[0]).unwrap();
            let visits = RELEASE_CACHE_VISITS.with(|v| v.get());
            eprintln!("resident={count}, one shared release cache visits={visits}");
            assert_eq!(loader.decoded_cache_len(), count);
            assert_eq!(
                loader.get_handle_state(&shared),
                Some(RuntimeAssetLoadState::Ready)
            );
            assert_eq!(visits, 1, "release scanned unrelated resident resources");
            RELEASE_CACHE_VISITS.with(|v| v.set(0));
            loader.release(&shared).unwrap();
            assert_eq!(RELEASE_CACHE_VISITS.with(|v| v.get()), 1);
            assert_eq!(loader.decoded_cache_len(), count - 1);
            for handle in &handles[1..] {
                loader.release(handle).unwrap();
            }
            assert_eq!(loader.decoded_cache_len(), 0);
        }
    }

    #[test]
    fn sync_load_release_history_is_bounded_without_evicting_live_consumers() {
        for cycles in [16, 128, 1024] {
            let mut loader = loader_with_records(vec![record(
                "guid-main",
                "main",
                "texture",
                "cooked-main",
                vec![],
                "1",
            )]);
            loader.mount_bundle("startup");
            loader
                .set_streaming_budget(StreamingBudget {
                    max_handles: 8,
                    ..StreamingBudget::default()
                })
                .unwrap();
            let active = loader.load(&asset_ref("main", "texture")).unwrap();
            let mut previous_generation = 0;
            for _ in 0..cycles {
                let handle = loader.load(&asset_ref("main", "texture")).unwrap();
                assert!(handle.generation > previous_generation);
                previous_generation = handle.generation;
                loader.release(&handle).unwrap();
                assert_eq!(
                    loader.get_handle_state(&handle),
                    Some(RuntimeAssetLoadState::Released)
                );
                assert_eq!(
                    loader.get_handle_state(&active),
                    Some(RuntimeAssetLoadState::Ready)
                );
                assert_eq!(loader.decoded_cache_len(), 1);
            }
            eprintln!(
                "cycles={cycles}, retained handle records={}",
                loader.handles.len()
            );
            assert!(
                loader.handles.len() <= 9,
                "released sync handles grew with completed loads"
            );
            let mut stale = active.clone();
            stale.generation += 1;
            assert!(loader.release(&stale).is_err());
            assert_eq!(
                loader.get_handle_state(&active),
                Some(RuntimeAssetLoadState::Ready)
            );
            loader.release(&active).unwrap();
            assert_eq!(loader.decoded_cache_len(), 0);
            assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        }
    }

    #[test]
    fn failed_ticket_release_remains_queryable_at_one_handle_history_budget() {
        use std::time::Duration;
        let (mut loader, root) = real_streaming_loader();
        loader
            .set_streaming_budget(StreamingBudget {
                max_handles: 1,
                ..loader.streaming_budget()
            })
            .unwrap();
        let ticket = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        assert!(loader.shutdown_streaming(Duration::from_secs(2)));
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Failed)
        );
        assert!(loader.released_handles.is_empty());
        loader.release(&ticket).unwrap();
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Released)
        );
        assert_eq!(
            loader
                .released_handles
                .iter()
                .filter(|id| **id == ticket.handle_id)
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn released_history_respects_smaller_budgets_async_admission_and_failed_tickets() {
        use std::time::Duration;
        let (mut loader, root) = real_streaming_loader();
        loader
            .set_streaming_budget(StreamingBudget {
                max_handles: 8,
                ..StreamingBudget::default()
            })
            .unwrap();
        let active = loader.load(&asset_ref("sound", "audio")).unwrap();
        let mut expired = None;
        for _ in 0..32 {
            let h = loader.load(&asset_ref("sound", "audio")).unwrap();
            expired.get_or_insert_with(|| h.clone());
            loader.release(&h).unwrap();
        }
        assert!(loader.current_handle(expired.as_ref().unwrap()).is_none());
        assert!(loader.release(expired.as_ref().unwrap()).is_err());
        assert_eq!(
            loader.get_handle_state(&active),
            Some(RuntimeAssetLoadState::Ready)
        );
        loader
            .set_streaming_budget(StreamingBudget {
                max_handles: 2,
                ..StreamingBudget::default()
            })
            .unwrap();
        assert!(loader.released_handles.len() <= 2);
        assert!(loader.handles.len() <= 3);
        let retained = loader.retain_handle(&active).unwrap();
        assert_eq!(loader.handles.len(), 2);
        loader.release(&retained).unwrap();
        // Admission reclaims only released metadata even when a live sync
        // consumer owns the same decoded backing.
        let ticket = loader.request_async(&asset_ref("sound", "audio")).unwrap();
        assert_eq!(loader.handles.len(), 2);
        assert_eq!(
            loader.get_handle_state(&active),
            Some(RuntimeAssetLoadState::Ready)
        );
        assert!(loader.shutdown_streaming(Duration::from_secs(2)));
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Failed)
        );
        for _ in 0..8 {
            let h = loader.load(&asset_ref("sound", "audio")).unwrap();
            loader.release(&h).unwrap();
        }
        // fail_pending promotes its internal release to a visible failure;
        // history expiry must preserve that live failure and its error.
        assert_eq!(
            loader.get_handle_state(&ticket),
            Some(RuntimeAssetLoadState::Failed)
        );
        assert_eq!(
            loader.streaming_error(&ticket),
            Some(&StreamingError::Cancelled)
        );
        loader.release(&active).unwrap();
        loader.release(&ticket).unwrap();
        assert_eq!(loader.decoded_cache_len(), 0);
        fs::remove_dir_all(root).unwrap();
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
