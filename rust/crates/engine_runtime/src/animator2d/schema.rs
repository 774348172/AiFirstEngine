use crate::canonical_digest::{CanonicalDigestError, ConsistencyDigest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const COOKED_ANIMATOR2D_REGISTRY_SCHEMA_VERSION: &str = "cooked-animator2d-registry.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DPlayback {
    Loop,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DParameterKind {
    Bool,
    Trigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DTransitionTiming {
    Immediate,
    ClipEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedSpriteAnimationFrame2D {
    pub sprite_asset_id: String,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedSpriteAnimationClip2D {
    pub id: String,
    pub playback: Animator2DPlayback,
    pub frames: Vec<CookedSpriteAnimationFrame2D>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedAnimator2DParameter {
    pub id: String,
    pub kind: Animator2DParameterKind,
    pub default_bool: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedAnimator2DState {
    pub id: String,
    pub clip_index: u32,
    pub speed_permille: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum CookedAnimator2DCondition {
    BoolEquals { parameter_index: u32, value: bool },
    Triggered { parameter_index: u32 },
}

impl CookedAnimator2DCondition {
    pub fn parameter_index(&self) -> u32 {
        match self {
            Self::BoolEquals {
                parameter_index, ..
            }
            | Self::Triggered { parameter_index } => *parameter_index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedAnimator2DTransition {
    pub id: String,
    pub from_state_index: u32,
    pub to_state_index: u32,
    pub timing: Animator2DTransitionTiming,
    pub priority: i32,
    pub conditions: Vec<CookedAnimator2DCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedAnimatorController2D {
    pub id: String,
    pub entry_state_index: u32,
    pub parameters: Vec<CookedAnimator2DParameter>,
    pub states: Vec<CookedAnimator2DState>,
    pub transitions: Vec<CookedAnimator2DTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedAnimator2DRegistry {
    pub schema_version: String,
    pub registry_digest: String,
    pub clips: Vec<CookedSpriteAnimationClip2D>,
    pub controllers: Vec<CookedAnimatorController2D>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeAnimator2D {
    pub controller_id: String,
    pub controller_index: u32,
    pub registry_digest: String,
    pub enabled: bool,
    #[serde(default)]
    pub initial_bools: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Animator2DDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
    pub next_action: String,
}

impl Animator2DDiagnostic {
    pub fn error(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
            next_action: next_action.into(),
        }
    }
}

impl CookedAnimator2DRegistry {
    pub fn empty() -> Self {
        Self::from_parts(Vec::new(), Vec::new()).expect("empty Animator2D registry must be valid")
    }

    pub fn from_parts(
        mut clips: Vec<CookedSpriteAnimationClip2D>,
        mut controllers: Vec<CookedAnimatorController2D>,
    ) -> Result<Self, Vec<Animator2DDiagnostic>> {
        clips.sort_by(|left, right| left.id.cmp(&right.id));
        controllers.sort_by(|left, right| left.id.cmp(&right.id));
        let mut registry = Self {
            schema_version: COOKED_ANIMATOR2D_REGISTRY_SCHEMA_VERSION.to_string(),
            registry_digest: String::new(),
            clips,
            controllers,
        };
        let diagnostics = registry.structural_diagnostics();
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        registry.registry_digest = registry
            .computed_digest()
            .expect("validated Animator2D registry must be canonically encodable");
        Ok(registry)
    }

    pub fn validate(&self) -> Result<(), Vec<Animator2DDiagnostic>> {
        let mut diagnostics = self.structural_diagnostics();
        if diagnostics.is_empty() {
            match self.computed_digest() {
                Ok(digest) if digest == self.registry_digest => {}
                Ok(digest) => diagnostics.push(Animator2DDiagnostic::error(
                    "animator2d.registry_digest_mismatch",
                    "registryDigest",
                    format!(
                        "Animator2D registry digest mismatch: expected {digest}, got {}.",
                        self.registry_digest
                    ),
                    "Rebuild the RuntimePackage from the canonical Animator2D assets.",
                )),
                Err(error) => diagnostics.push(Animator2DDiagnostic::error(
                    "animator2d.registry_digest_failed",
                    "registryDigest",
                    error.to_string(),
                    "Fix the cooked registry so it can be canonically encoded.",
                )),
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    pub fn controller_index(&self, controller_id: &str) -> Option<u32> {
        self.controllers
            .binary_search_by(|controller| controller.id.as_str().cmp(controller_id))
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }

    fn computed_digest(&self) -> Result<String, CanonicalDigestError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct DigestPayload<'a> {
            schema_version: &'a str,
            clips: &'a [CookedSpriteAnimationClip2D],
            controllers: &'a [CookedAnimatorController2D],
        }
        ConsistencyDigest::sha256(
            "cooked-animator2d-registry",
            COOKED_ANIMATOR2D_REGISTRY_SCHEMA_VERSION,
            &DigestPayload {
                schema_version: &self.schema_version,
                clips: &self.clips,
                controllers: &self.controllers,
            },
        )
        .map(|digest| digest.prefixed_value())
    }

    fn structural_diagnostics(&self) -> Vec<Animator2DDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema_version != COOKED_ANIMATOR2D_REGISTRY_SCHEMA_VERSION {
            diagnostics.push(Animator2DDiagnostic::error(
                "animator2d.registry_schema_unsupported",
                "schemaVersion",
                format!(
                    "Expected {}, got {}.",
                    COOKED_ANIMATOR2D_REGISTRY_SCHEMA_VERSION, self.schema_version
                ),
                "Re-cook the Animator2D assets with the current engine.",
            ));
        }
        validate_sorted_unique_ids(
            self.clips.iter().map(|clip| clip.id.as_str()),
            "clips",
            &mut diagnostics,
        );
        for (clip_index, clip) in self.clips.iter().enumerate() {
            let path = format!("clips[{clip_index}]");
            if clip.frames.is_empty() {
                diagnostics.push(Animator2DDiagnostic::error(
                    "animator2d.clip_frames_empty",
                    format!("{path}.frames"),
                    "Animation clip must contain at least one frame.",
                    "Add at least one Sprite frame.",
                ));
            }
            for (frame_index, frame) in clip.frames.iter().enumerate() {
                if frame.sprite_asset_id.trim().is_empty() {
                    diagnostics.push(Animator2DDiagnostic::error(
                        "animator2d.sprite_missing",
                        format!("{path}.frames[{frame_index}].spriteAssetId"),
                        "Cooked Sprite asset id must not be empty.",
                        "Select a valid Sprite asset before cooking.",
                    ));
                }
                if frame.duration_ticks == 0 {
                    diagnostics.push(Animator2DDiagnostic::error(
                        "animator2d.frame_duration_invalid",
                        format!("{path}.frames[{frame_index}].durationTicks"),
                        "Frame durationTicks must be greater than zero.",
                        "Use a positive fixed-tick duration.",
                    ));
                }
            }
        }
        validate_sorted_unique_ids(
            self.controllers
                .iter()
                .map(|controller| controller.id.as_str()),
            "controllers",
            &mut diagnostics,
        );
        for (controller_index, controller) in self.controllers.iter().enumerate() {
            validate_controller(
                controller_index,
                controller,
                self.clips.len(),
                &mut diagnostics,
            );
        }
        diagnostics
    }
}

fn validate_controller(
    controller_index: usize,
    controller: &CookedAnimatorController2D,
    clip_count: usize,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) {
    let path = format!("controllers[{controller_index}]");
    validate_sorted_unique_ids(
        controller
            .parameters
            .iter()
            .map(|parameter| parameter.id.as_str()),
        &format!("{path}.parameters"),
        diagnostics,
    );
    validate_sorted_unique_ids(
        controller.states.iter().map(|state| state.id.as_str()),
        &format!("{path}.states"),
        diagnostics,
    );
    validate_sorted_unique_ids(
        controller
            .transitions
            .iter()
            .map(|transition| transition.id.as_str()),
        &format!("{path}.transitions"),
        diagnostics,
    );
    if controller.states.is_empty()
        || usize::try_from(controller.entry_state_index)
            .ok()
            .is_none_or(|index| index >= controller.states.len())
    {
        diagnostics.push(Animator2DDiagnostic::error(
            "animator2d.entry_state_invalid",
            format!("{path}.entryStateIndex"),
            "Entry state index does not identify a cooked state.",
            "Choose an existing state as the Controller entry.",
        ));
    }
    for (state_index, state) in controller.states.iter().enumerate() {
        if usize::try_from(state.clip_index)
            .ok()
            .is_none_or(|index| index >= clip_count)
        {
            diagnostics.push(Animator2DDiagnostic::error(
                "animator2d.clip_missing",
                format!("{path}.states[{state_index}].clipIndex"),
                "State clip index does not identify a cooked clip.",
                "Assign a valid SpriteAnimationClip2D asset.",
            ));
        }
        if state.speed_permille == 0 {
            diagnostics.push(Animator2DDiagnostic::error(
                "animator2d.speed_invalid",
                format!("{path}.states[{state_index}].speedPermille"),
                "speedPermille must be greater than zero.",
                "Use a positive fixed-point playback speed.",
            ));
        }
    }
    for (transition_index, transition) in controller.transitions.iter().enumerate() {
        for (field, state_index) in [
            ("fromStateIndex", transition.from_state_index),
            ("toStateIndex", transition.to_state_index),
        ] {
            if usize::try_from(state_index)
                .ok()
                .is_none_or(|index| index >= controller.states.len())
            {
                diagnostics.push(Animator2DDiagnostic::error(
                    "animator2d.transition_target_invalid",
                    format!("{path}.transitions[{transition_index}].{field}"),
                    "Transition state index does not identify a cooked state.",
                    "Point the transition at existing states.",
                ));
            }
        }
        for (condition_index, condition) in transition.conditions.iter().enumerate() {
            let parameter_index = condition.parameter_index();
            let parameter = usize::try_from(parameter_index)
                .ok()
                .and_then(|index| controller.parameters.get(index));
            let compatible = matches!(
                (condition, parameter.map(|value| value.kind)),
                (
                    CookedAnimator2DCondition::BoolEquals { .. },
                    Some(Animator2DParameterKind::Bool)
                ) | (
                    CookedAnimator2DCondition::Triggered { .. },
                    Some(Animator2DParameterKind::Trigger)
                )
            );
            if !compatible {
                diagnostics.push(Animator2DDiagnostic::error(
                    "animator2d.condition_parameter_invalid",
                    format!("{path}.transitions[{transition_index}].conditions[{condition_index}]"),
                    "Condition parameter index or kind is invalid.",
                    "Use BoolEquals with Bool and Triggered with Trigger parameters.",
                ));
            }
        }
    }
}

fn validate_sorted_unique_ids<'a>(
    ids: impl IntoIterator<Item = &'a str>,
    path: &str,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) {
    let mut previous: Option<&str> = None;
    let mut seen = BTreeSet::new();
    for (index, id) in ids.into_iter().enumerate() {
        if id.trim().is_empty() {
            diagnostics.push(Animator2DDiagnostic::error(
                "animator2d.id_invalid",
                format!("{path}[{index}].id"),
                "Stable id must not be empty.",
                "Assign a non-empty stable id.",
            ));
        }
        if !seen.insert(id) {
            diagnostics.push(Animator2DDiagnostic::error(
                "animator2d.id_duplicate",
                format!("{path}[{index}].id"),
                format!("Duplicate stable id: {id}."),
                "Use unique stable ids in this collection.",
            ));
        }
        if previous.is_some_and(|value| value > id) {
            diagnostics.push(Animator2DDiagnostic::error(
                "animator2d.order_noncanonical",
                path,
                "Cooked collection is not ordered by stable id.",
                "Re-cook the assets using canonical stable-id ordering.",
            ));
            break;
        }
        previous = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animator2d_schema_rejects_unknown_fields() {
        let result = serde_json::from_str::<CookedAnimator2DRegistry>(
            r#"{"schemaVersion":"cooked-animator2d-registry.v1","registryDigest":"sha256:x","clips":[],"controllers":[],"unexpected":true}"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn animator2d_schema_registry_digest_is_stable_for_permuted_top_level_input() {
        let first = CookedAnimator2DRegistry::from_parts(
            vec![fixture_clip("b"), fixture_clip("a")],
            Vec::new(),
        )
        .unwrap();
        let second = CookedAnimator2DRegistry::from_parts(
            vec![fixture_clip("a"), fixture_clip("b")],
            Vec::new(),
        )
        .unwrap();
        assert_eq!(first.registry_digest, second.registry_digest);
        assert_eq!(first.clips, second.clips);
    }

    fn fixture_clip(id: &str) -> CookedSpriteAnimationClip2D {
        CookedSpriteAnimationClip2D {
            id: id.to_string(),
            playback: Animator2DPlayback::Loop,
            frames: vec![CookedSpriteAnimationFrame2D {
                sprite_asset_id: format!("sprite-{id}"),
                duration_ticks: 1,
            }],
        }
    }
}
