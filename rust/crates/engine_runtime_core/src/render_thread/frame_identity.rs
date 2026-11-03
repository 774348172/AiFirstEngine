//! Source identities are frozen at Extract, never reconstructed from a live World.
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);
static PROCESS_INVOCATION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub(crate) fn new_source_epoch() -> u64 {
    NEXT_EPOCH.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn run_identity(epoch: u64) -> String {
    let invocation = PROCESS_INVOCATION.get_or_init(|| {
        let instant = match crate::platform_time::SystemTime::now()
            .duration_since(crate::platform_time::UNIX_EPOCH)
        {
            Ok(d) => format!("unix+{}", d.as_nanos()),
            Err(e) => format!("unix-{}", e.duration().as_nanos()),
        };
        #[cfg(not(target_arch = "wasm32"))]
        let process = std::process::id().to_string();
        #[cfg(target_arch = "wasm32")]
        let process = "web";
        format!("{process}:{instant}")
    });
    format!("{invocation}:{epoch}")
}

/// A clone is the same logical World lineage (transactional rollback included).
/// A new/default World receives a different epoch, even if its entity IDs repeat.
#[derive(Debug, Clone)]
pub(crate) struct WorldEpoch(pub u64);
impl Default for WorldEpoch {
    fn default() -> Self {
        Self(new_source_epoch())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFrameSourceIdentity {
    /// Process invocation + local host epoch; the session label alone may repeat.
    pub run_id: String,
    pub run_epoch: u64,
    pub session_id: String,
    /// RuntimePackage content hash is the runtime revision; absent is unavailable.
    /// It is not a Git revision or an inferred editor project revision.
    pub package_content_hash: Option<String>,
    pub world_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderViewHistoryIdentity {
    pub view_id: u64,
    pub camera_entity_id: Option<String>,
    pub view_version: u64,
    pub history_version: u64,
    /// Exact IEEE-754 values preserve the coordinate basis without lossy formatting.
    pub origin_bits: [u64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFrameIdentity {
    pub source: Arc<RenderFrameSourceIdentity>,
    pub frame_index: u64,
    pub tick_pair: [u64; 2],
    pub presentation_alpha_bits: u32,
    pub views: Arc<Vec<RenderViewHistoryIdentity>>,
}

impl RenderFrameIdentity {
    pub(crate) fn matches_frame(&self, frame_index: u64) -> bool {
        let alpha = f32::from_bits(self.presentation_alpha_bits);
        self.frame_index == frame_index
            && self.source.run_epoch != 0
            && self.source.world_epoch != 0
            && self.tick_pair[0] <= self.tick_pair[1]
            && alpha.is_finite()
            && (0.0..=1.0).contains(&alpha)
    }

    pub(crate) fn freeze(
        source: Arc<RenderFrameSourceIdentity>,
        frame_index: u64,
        presentation: &crate::world::PresentationDelta,
        geometry: &crate::render_geometry::ProductionGeometrySnapshot,
    ) -> Arc<Self> {
        Arc::new(Self {
            source,
            frame_index,
            tick_pair: presentation.tick_pair,
            presentation_alpha_bits: presentation.alpha.to_bits(),
            views: geometry.identity_views.clone(),
        })
    }
}
