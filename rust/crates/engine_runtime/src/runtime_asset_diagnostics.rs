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
}

impl AssetLoadDiagnostics {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, diagnostic: AssetLoadDiagnostic) {
        self.entries.push(diagnostic);
    }

    pub fn entries(&self) -> &[AssetLoadDiagnostic] {
        &self.entries
    }

    pub fn has_error(&self, error_code: AssetLoadErrorCode) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.error_code.as_ref() == Some(&error_code))
    }
}
