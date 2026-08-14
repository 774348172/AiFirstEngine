use std::collections::VecDeque;

use crate::font_bundle::RuntimeLoadedFontBundle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFontGeneration {
    pub bundle: RuntimeLoadedFontBundle,
    pub prepared_page_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveFontGeneration {
    pub bundle: RuntimeLoadedFontBundle,
    pub activated_frame: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetiredFontGeneration {
    pub bundle_id: String,
    pub generation: u64,
    pub retire_after_frame: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontGenerationSwapReport {
    pub status: &'static str,
    pub active_generation: Option<u64>,
    pub retired_generation: Option<u64>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Default)]
pub struct RuntimeFontGenerationManager {
    active: Option<ActiveFontGeneration>,
    prepared: Option<PreparedFontGeneration>,
    retired: VecDeque<RetiredFontGeneration>,
}

impl RuntimeFontGenerationManager {
    pub fn active(&self) -> Option<&ActiveFontGeneration> {
        self.active.as_ref()
    }

    pub fn prepare(
        &mut self,
        bundle: RuntimeLoadedFontBundle,
        prepared_page_count: usize,
    ) -> Result<(), String> {
        if prepared_page_count != bundle.metadata.pages.len()
            || bundle.page_payloads.len() != bundle.metadata.pages.len()
        {
            return Err("font_generation.prepare_incomplete".to_string());
        }
        if self.active.as_ref().is_some_and(|active| {
            active.bundle.metadata.font_bundle_id != bundle.metadata.font_bundle_id
        }) {
            return Err("font_generation.bundle_identity_changed".to_string());
        }
        self.prepared = Some(PreparedFontGeneration {
            bundle,
            prepared_page_count,
        });
        Ok(())
    }

    pub fn reject_prepared(&mut self, diagnostic: impl Into<String>) -> FontGenerationSwapReport {
        self.prepared = None;
        FontGenerationSwapReport {
            status: "rejected_kept_active",
            active_generation: self
                .active
                .as_ref()
                .map(|active| active.bundle.metadata.generation),
            retired_generation: None,
            diagnostic: Some(diagnostic.into()),
        }
    }

    pub fn activate_at_frame_boundary(
        &mut self,
        frame_index: u64,
        max_frames_in_flight: u64,
    ) -> FontGenerationSwapReport {
        let Some(prepared) = self.prepared.take() else {
            return FontGenerationSwapReport {
                status: "no_prepared_generation",
                active_generation: self
                    .active
                    .as_ref()
                    .map(|active| active.bundle.metadata.generation),
                retired_generation: None,
                diagnostic: None,
            };
        };
        let previous = self.active.replace(ActiveFontGeneration {
            bundle: prepared.bundle,
            activated_frame: frame_index,
        });
        let retired_generation = previous.as_ref().map(|active| {
            let generation = active.bundle.metadata.generation;
            self.retired.push_back(RetiredFontGeneration {
                bundle_id: active.bundle.metadata.font_bundle_id.clone(),
                generation,
                retire_after_frame: frame_index.saturating_add(max_frames_in_flight),
            });
            generation
        });
        FontGenerationSwapReport {
            status: "activated",
            active_generation: self
                .active
                .as_ref()
                .map(|active| active.bundle.metadata.generation),
            retired_generation,
            diagnostic: None,
        }
    }

    pub fn collect_retired(&mut self, completed_frame: u64) -> Vec<RetiredFontGeneration> {
        let mut collected = Vec::new();
        while self
            .retired
            .front()
            .is_some_and(|generation| generation.retire_after_frame <= completed_frame)
        {
            collected.push(self.retired.pop_front().expect("front checked"));
        }
        collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_bundle::{
        CookedFontBundleAsset, RuntimeLoadedFontBundle, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
    };

    fn bundle(generation: u64) -> RuntimeLoadedFontBundle {
        RuntimeLoadedFontBundle {
            metadata: CookedFontBundleAsset {
                schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
                font_bundle_id: "ui-font".to_string(),
                font_stack_id: "ui-stack".to_string(),
                generation,
                max_bitmap_pages: 0,
                max_msdf_pages: 0,
                legacy_mode: false,
                fallback_used: false,
                quality_gate_eligible: true,
                pages: Vec::new(),
                glyphs: Vec::new(),
                kerning_adjustments: Vec::new(),
                bundle_digest: format!("sha256:generation-{generation}"),
            },
            page_payloads: Vec::new(),
        }
    }

    #[test]
    fn runtime_font_generation_switches_only_at_boundary_and_retires_after_in_flight_frames() {
        let mut manager = RuntimeFontGenerationManager::default();
        manager.prepare(bundle(1), 0).unwrap();
        assert!(manager.active().is_none());
        assert_eq!(
            manager.activate_at_frame_boundary(10, 2).active_generation,
            Some(1)
        );
        manager.prepare(bundle(2), 0).unwrap();
        assert_eq!(manager.active().unwrap().bundle.metadata.generation, 1);

        let report = manager.activate_at_frame_boundary(11, 2);

        assert_eq!(report.active_generation, Some(2));
        assert_eq!(report.retired_generation, Some(1));
        assert!(manager.collect_retired(12).is_empty());
        assert_eq!(manager.collect_retired(13)[0].generation, 1);
    }

    #[test]
    fn runtime_font_generation_failed_prepare_keeps_active_generation() {
        let mut manager = RuntimeFontGenerationManager::default();
        manager.prepare(bundle(1), 0).unwrap();
        manager.activate_at_frame_boundary(1, 2);
        let error = manager.prepare(bundle(2), 1).unwrap_err();
        let report = manager.reject_prepared(error);

        assert_eq!(report.status, "rejected_kept_active");
        assert_eq!(report.active_generation, Some(1));
        assert_eq!(manager.active().unwrap().bundle.metadata.generation, 1);
    }
}
