#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLoadStage {
    Resolve,
    Dependency,
    BundleMount,
    Read,
    Decode,
    Prepare,
    Apply,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLoadState {
    Ok,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLoadErrorCode {
    MissingAssetRef,
    TypeMismatch,
    SubAssetMissing,
    BundleNotMounted,
    CookedAssetMissing,
    UnsafePackagePath,
    DependencyMissing,
    CyclicDependency,
    DecodeFailed,
    GpuPrepareFailed,
    ReleaseGenerationMismatch,
    ReleaseInUse,
    VersionMismatch,
    SyncLoadInHotPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLoadDiagnostic {
    pub request_id: u64,
    pub asset_ref_id: String,
    pub asset_guid: Option<String>,
    pub asset_type: String,
    pub stage: AssetLoadStage,
    pub state: AssetLoadState,
    pub error_code: Option<AssetLoadErrorCode>,
    pub bundle_id: Option<String>,
    pub cooked_asset_id: Option<String>,
    pub loader_kind: Option<String>,
    pub dependency_chain: Vec<String>,
    pub handle_id: Option<u64>,
    pub generation: Option<u64>,
    pub sync_load: bool,
    pub recommended_action: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AssetLoadDiagnostics {
    entries: Vec<AssetLoadDiagnostic>,
    omitted_count: u64,
}

const MAX_RETAINED_DIAGNOSTICS: usize = 1024;

impl AssetLoadDiagnostics {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.capacities.insert(
            "loader.diagnostics.retained".into(),
            self.entries.len() as u64,
        );
        sample
            .capacities
            .insert("loader.diagnostics.omitted".into(), self.omitted_count);
        sample.cpu_vec(&self.entries, "instrumentation", &["instrumentation"]);
        for d in &self.entries {
            for v in [&d.asset_ref_id, &d.asset_type].into_iter().chain(
                [
                    &d.asset_guid,
                    &d.bundle_id,
                    &d.cooked_asset_id,
                    &d.loader_kind,
                    &d.recommended_action,
                ]
                .into_iter()
                .flatten(),
            ) {
                sample.cpu_string(v, "instrumentation", &["instrumentation"]);
            }
            sample.cpu_vec(&d.dependency_chain, "instrumentation", &["instrumentation"]);
            for v in &d.dependency_chain {
                sample.cpu_string(v, "instrumentation", &["instrumentation"]);
            }
        }
        sample.partial(
            "cpu.owner.instrumentation",
            "loader.diagnostics.vec+strings.capacity",
        );
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: AssetLoadDiagnostic) {
        if self.entries.len() == MAX_RETAINED_DIAGNOSTICS {
            // Keep the first failure available to the ordinary first-cause
            // consumers. Compact in batches rather than shifting a full log
            // on every successful load/release. The remaining slice stays in
            // chronological order, with explicit omission accounting.
            let (start, end) = match self
                .entries
                .iter()
                .position(|entry| entry.state == AssetLoadState::Failed)
            {
                Some(0) => (1, 1 + MAX_RETAINED_DIAGNOSTICS / 2),
                Some(index) => (0, index),
                None => (0, MAX_RETAINED_DIAGNOSTICS / 2),
            };
            self.omitted_count = self.omitted_count.saturating_add((end - start) as u64);
            self.entries.drain(start..end);
        }
        self.entries.push(diagnostic);
    }

    /// Retained chronological context, including the first failure. This is a
    /// bounded history; omitted_count reports events discarded by compaction.
    pub fn entries(&self) -> &[AssetLoadDiagnostic] {
        &self.entries
    }

    pub fn omitted_count(&self) -> u64 {
        self.omitted_count
    }

    pub fn has_error(&self, error_code: AssetLoadErrorCode) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.error_code.as_ref() == Some(&error_code))
    }
}
