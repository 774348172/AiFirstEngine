use std::collections::HashMap;

use editor_core::{stable_game_view_surface_id, GameViewRuntimeFrame};
use editor_wgpu_renderer::{
    GameViewPublicationReceipt, GameViewPublicationStatus, RuntimeContentIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFramePublicationError {
    pub code: &'static str,
    pub message: String,
}

impl EditorFramePublicationError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFramePublicationResult {
    pub receipt: GameViewPublicationReceipt,
    pub submitted_runtime_write: bool,
}

#[derive(Default)]
pub struct EditorFramePublicationModule {
    last_good_by_surface: HashMap<String, LastGoodPublication>,
}

struct LastGoodPublication {
    receipt: GameViewPublicationReceipt,
    aui_presentation_identity: String,
}

impl EditorFramePublicationModule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish<F>(
        &mut self,
        frame: &GameViewRuntimeFrame,
        submit_runtime_write: F,
    ) -> Result<EditorFramePublicationResult, EditorFramePublicationError>
    where
        F: FnOnce() -> Result<GameViewPublicationReceipt, EditorFramePublicationError>,
    {
        let expected_surface = stable_game_view_surface_id(&frame.session_id, &frame.target_id);
        if frame.texture_id != expected_surface {
            return Err(EditorFramePublicationError::new(
                "publication.surface_identity_mismatch",
                format!(
                    "GameView frame surface '{}' does not match stable identity '{}'.",
                    frame.texture_id, expected_surface
                ),
            ));
        }

        if let Some(last_good) = self.last_good_by_surface.get(&expected_surface) {
            if frame.frame_index < last_good.receipt.content.frame_index {
                return Err(EditorFramePublicationError::new(
                    "publication.runtime_frame_regressed",
                    format!(
                        "Runtime frame {} is older than last published frame {}.",
                        frame.frame_index, last_good.receipt.content.frame_index
                    ),
                ));
            }
            if Self::matches_last_good(frame, last_good) {
                let mut reused = last_good.receipt.clone();
                reused.status = GameViewPublicationStatus::Reused;
                return Ok(EditorFramePublicationResult {
                    receipt: reused,
                    submitted_runtime_write: false,
                });
            }
        }

        let receipt = submit_runtime_write()?;
        Self::validate_receipt(frame, &expected_surface, &receipt)?;
        self.last_good_by_surface.insert(
            expected_surface,
            LastGoodPublication {
                receipt: receipt.clone(),
                aui_presentation_identity: frame.aui_presentation_identity.clone(),
            },
        );
        Ok(EditorFramePublicationResult {
            receipt,
            submitted_runtime_write: true,
        })
    }

    pub fn last_good(
        &self,
        session_id: &str,
        target_id: &str,
    ) -> Option<&GameViewPublicationReceipt> {
        self.last_good_by_surface
            .get(&stable_game_view_surface_id(session_id, target_id))
            .map(|last_good| &last_good.receipt)
    }

    pub fn reusable_last_good(
        &self,
        frame: &GameViewRuntimeFrame,
    ) -> Option<&GameViewPublicationReceipt> {
        self.last_good_by_surface
            .get(&stable_game_view_surface_id(
                &frame.session_id,
                &frame.target_id,
            ))
            .filter(|last_good| Self::matches_last_good(frame, last_good))
            .map(|last_good| &last_good.receipt)
    }

    pub fn retire_session(&mut self, session_id: &str) -> usize {
        let before = self.last_good_by_surface.len();
        self.last_good_by_surface
            .retain(|_, last_good| last_good.receipt.content.session_id != session_id);
        before - self.last_good_by_surface.len()
    }

    fn matches_last_good(frame: &GameViewRuntimeFrame, last_good: &LastGoodPublication) -> bool {
        frame.frame_index == last_good.receipt.content.frame_index
            && frame.frame_hash == last_good.receipt.content.frame_hash
            && frame.width.max(1) == last_good.receipt.width
            && frame.height.max(1) == last_good.receipt.height
            && frame.aui_presentation_identity == last_good.aui_presentation_identity
    }

    fn validate_receipt(
        frame: &GameViewRuntimeFrame,
        expected_surface: &str,
        receipt: &GameViewPublicationReceipt,
    ) -> Result<(), EditorFramePublicationError> {
        let expected_content = RuntimeContentIdentity {
            session_id: frame.session_id.clone(),
            frame_index: frame.frame_index,
            frame_hash: frame.frame_hash.clone(),
        };
        if receipt.content != expected_content {
            return Err(EditorFramePublicationError::new(
                "publication.content_identity_mismatch",
                "Publication receipt content does not match the produced Runtime frame.",
            ));
        }
        if receipt.publication.surface_id != expected_surface {
            return Err(EditorFramePublicationError::new(
                "publication.receipt_surface_mismatch",
                "Publication receipt references a different stable surface.",
            ));
        }
        if receipt.width != frame.width.max(1) || receipt.height != frame.height.max(1) {
            return Err(EditorFramePublicationError::new(
                "publication.receipt_extent_mismatch",
                "Publication receipt extent does not match the produced Runtime frame.",
            ));
        }
        if receipt.status != GameViewPublicationStatus::Published {
            return Err(EditorFramePublicationError::new(
                "publication.receipt_status_invalid",
                "A new Runtime write must return a Published receipt.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_wgpu_renderer::{GameViewPublicationIdentity, GameViewPublicationStatus};

    fn frame(frame_index: u64, frame_hash: &str) -> GameViewRuntimeFrame {
        GameViewRuntimeFrame {
            schema_version: "game-view-runtime-frame.v1".to_string(),
            session_id: "session-a".to_string(),
            scene_id: "scene-main".to_string(),
            frame_index,
            frame_hash: frame_hash.to_string(),
            target_id: "viewport-main".to_string(),
            texture_id: stable_game_view_surface_id("session-a", "viewport-main"),
            width: 1280,
            height: 720,
            aui_presentation_identity: "aui:base".to_string(),
            presentation_scale_policy:
                engine_runtime::game_view_presentation::GameViewScalePolicy::Stretch,
            renderable_count: 0,
            ui_draw_item_count: 0,
            aui_present_status: "success".to_string(),
            input_bridge_status: "not_requested".to_string(),
            runtime_input_event_count: 0,
            filtered_runtime_input_event_count: 0,
            aui_consumed_event_count: 0,
            aui_feedback_override_count: 0,
            aui_feedback_profile_ids: Vec::new(),
            gameplay_action_count: 0,
            gameplay_action_ids: Vec::new(),
            texture_descriptor_status: "descriptor_only".to_string(),
            gpu_present_status: "gpu_unavailable".to_string(),
            rhi_command_count: 0,
            render_graph_pass_count: 0,
            runtime_target_kind: "ViewportTexture".to_string(),
            animator2d_play_observations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn receipt(frame: &GameViewRuntimeFrame, publication_index: u64) -> GameViewPublicationReceipt {
        GameViewPublicationReceipt {
            content: RuntimeContentIdentity {
                session_id: frame.session_id.clone(),
                frame_index: frame.frame_index,
                frame_hash: frame.frame_hash.clone(),
            },
            publication: GameViewPublicationIdentity {
                surface_id: frame.texture_id.clone(),
                surface_generation: 1,
                publication_index,
            },
            submit_serial: publication_index,
            width: frame.width,
            height: frame.height,
            format: "Bgra8UnormSrgb".to_string(),
            status: GameViewPublicationStatus::Published,
        }
    }

    #[test]
    fn editor_frame_publication_reuses_last_good_without_runtime_submit() {
        let mut module = EditorFramePublicationModule::new();
        let produced = frame(1, "hash-1");
        let first = module
            .publish(&produced, || Ok(receipt(&produced, 1)))
            .expect("first publication");
        assert!(first.submitted_runtime_write);

        let reused = module
            .publish(&produced, || panic!("redraw must not submit Runtime work"))
            .expect("redraw reuse");
        assert!(!reused.submitted_runtime_write);
        assert_eq!(reused.receipt.status, GameViewPublicationStatus::Reused);
        assert_eq!(reused.receipt.publication.publication_index, 1);
    }

    #[test]
    fn editor_frame_publication_submits_aui_only_revision_without_runtime_advance() {
        let mut module = EditorFramePublicationModule::new();
        let produced = frame(1, "hash-1");
        module
            .publish(&produced, || Ok(receipt(&produced, 1)))
            .expect("first publication");

        let mut aui_revision = produced.clone();
        aui_revision.aui_presentation_identity = "aui:pressed".to_string();
        let republished = module
            .publish(&aui_revision, || Ok(receipt(&aui_revision, 2)))
            .expect("AUI-only revision must publish");

        assert!(republished.submitted_runtime_write);
        assert_eq!(republished.receipt.content.frame_index, 1);
        assert_eq!(republished.receipt.content.frame_hash, "hash-1");
        assert_eq!(republished.receipt.publication.publication_index, 2);
    }

    #[test]
    fn editor_frame_publication_rejects_regression_and_invalid_receipt() {
        let mut module = EditorFramePublicationModule::new();
        let second = frame(2, "hash-2");
        module
            .publish(&second, || Ok(receipt(&second, 1)))
            .expect("second frame");
        let older = frame(1, "hash-1");
        assert_eq!(
            module
                .publish(&older, || Ok(receipt(&older, 2)))
                .expect_err("frame regression must fail")
                .code,
            "publication.runtime_frame_regressed"
        );

        let newer = frame(3, "hash-3");
        let mut invalid = receipt(&newer, 2);
        invalid.content.frame_hash = "wrong".to_string();
        assert_eq!(
            module
                .publish(&newer, || Ok(invalid))
                .expect_err("invalid receipt must fail")
                .code,
            "publication.content_identity_mismatch"
        );
    }

    #[test]
    fn failed_runtime_write_preserves_last_good_publication() {
        let mut module = EditorFramePublicationModule::new();
        let first = frame(1, "hash-1");
        module
            .publish(&first, || Ok(receipt(&first, 1)))
            .expect("first publication");
        let second = frame(2, "hash-2");

        let error = module
            .publish(&second, || {
                Err(EditorFramePublicationError::new(
                    "wgpu.texture_binding_missing",
                    "missing handle Texture(7:2)",
                ))
            })
            .expect_err("missing GPU texture must reject publication");

        assert_eq!(error.code, "wgpu.texture_binding_missing");
        let last_good = module
            .last_good(&first.session_id, &first.target_id)
            .expect("last-good publication");
        assert_eq!(last_good.content.frame_index, 1);
        assert_eq!(last_good.publication.publication_index, 1);
    }
}
