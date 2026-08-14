use serde::{Deserialize, Serialize};

use super::{PatchApplyReport, ProjectPatchDocument};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHistory {
    pub entries: Vec<PatchHistoryEntry>,
}

impl PatchHistory {
    pub fn record(&mut self, entry: PatchHistoryEntry) {
        self.entries.push(entry);
    }

    pub fn last(&self) -> Option<&PatchHistoryEntry> {
        self.entries.last()
    }

    pub(crate) fn pop_last_if_patch_id(&mut self, patch_id: &str) -> Option<PatchHistoryEntry> {
        (self.entries.last()?.patch_id == patch_id)
            .then(|| self.entries.pop())
            .flatten()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHistoryEntry {
    pub patch_id: String,
    pub applied_at: String,
    pub original_patch: ProjectPatchDocument,
    pub inverse_patch: ProjectPatchDocument,
    pub apply_report: PatchApplyReport,
}
