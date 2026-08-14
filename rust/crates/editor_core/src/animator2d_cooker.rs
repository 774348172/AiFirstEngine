use engine_runtime::animator2d::{
    Animator2DDiagnostic, Animator2DParameterKind, Animator2DPlayback, Animator2DTransitionTiming,
    CookedAnimator2DCondition, CookedAnimator2DParameter, CookedAnimator2DRegistry,
    CookedAnimator2DState, CookedAnimator2DTransition, CookedAnimatorController2D,
    CookedSpriteAnimationClip2D, CookedSpriteAnimationFrame2D,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION: &str = "sprite-animation-clip-2d.v1";
pub const ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION: &str = "animator-controller-2d.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAnimationClip2DAsset {
    #[serde(rename = "schema")]
    pub schema_version: String,
    pub asset_id: String,
    pub playback: Animator2DPlayback,
    pub frames: Vec<SpriteAnimationFrame2DAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAnimationFrame2DAsset {
    pub sprite_ref: String,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorController2DAsset {
    #[serde(rename = "schema")]
    pub schema_version: String,
    pub asset_id: String,
    #[serde(default)]
    pub parameters: Vec<AnimatorController2DParameterAsset>,
    pub entry_state_id: String,
    pub states: Vec<AnimatorController2DStateAsset>,
    #[serde(default)]
    pub transitions: Vec<AnimatorController2DTransitionAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorController2DParameterAsset {
    pub id: String,
    pub kind: Animator2DParameterKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_bool: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorController2DStateAsset {
    pub id: String,
    pub clip_ref: String,
    #[serde(default = "default_speed_permille")]
    pub speed_permille: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorController2DTransitionAsset {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(rename = "when")]
    pub timing: Animator2DTransitionTiming,
    pub priority: i32,
    #[serde(default)]
    pub conditions: Vec<AnimatorController2DConditionAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimatorController2DConditionOperation {
    Equals,
    Triggered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorController2DConditionAsset {
    pub parameter: String,
    #[serde(rename = "op")]
    pub operation: AnimatorController2DConditionOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animator2DCookFailure {
    pub diagnostics: Vec<Animator2DDiagnostic>,
}

pub struct Animator2DAssetCooker;

impl Animator2DAssetCooker {
    pub fn parse_clip_json(
        source: &str,
        text: &str,
    ) -> Result<SpriteAnimationClip2DAsset, Animator2DCookFailure> {
        serde_json::from_str(text).map_err(|error| Animator2DCookFailure {
            diagnostics: vec![diagnostic(
                "animator2d.clip_parse_failed",
                source,
                format!("Failed to parse SpriteAnimationClip2D: {error}"),
                "Fix the clip JSON to match sprite-animation-clip-2d.v1.",
            )],
        })
    }

    pub fn parse_controller_json(
        source: &str,
        text: &str,
    ) -> Result<AnimatorController2DAsset, Animator2DCookFailure> {
        serde_json::from_str(text).map_err(|error| Animator2DCookFailure {
            diagnostics: vec![diagnostic(
                "animator2d.controller_parse_failed",
                source,
                format!("Failed to parse AnimatorController2D: {error}"),
                "Fix the controller JSON to match animator-controller-2d.v1.",
            )],
        })
    }

    pub fn cook(
        mut clips: Vec<SpriteAnimationClip2DAsset>,
        mut controllers: Vec<AnimatorController2DAsset>,
        available_sprite_ids: &BTreeSet<String>,
    ) -> Result<CookedAnimator2DRegistry, Animator2DCookFailure> {
        clips.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        controllers.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        let mut diagnostics = Vec::new();
        validate_unique_asset_ids(
            clips.iter().map(|clip| clip.asset_id.as_str()),
            "clips",
            &mut diagnostics,
        );
        validate_unique_asset_ids(
            controllers
                .iter()
                .map(|controller| controller.asset_id.as_str()),
            "controllers",
            &mut diagnostics,
        );

        let cooked_clips = clips
            .iter()
            .enumerate()
            .map(|(index, clip)| cook_clip(index, clip, available_sprite_ids, &mut diagnostics))
            .collect::<Vec<_>>();
        let clip_indices = cooked_clips
            .iter()
            .enumerate()
            .filter_map(|(index, clip)| {
                u32::try_from(index)
                    .ok()
                    .map(|index| (clip.id.clone(), index))
            })
            .collect::<BTreeMap<_, _>>();
        let cooked_controllers = controllers
            .iter()
            .enumerate()
            .map(|(index, controller)| {
                cook_controller(index, controller, &clip_indices, &mut diagnostics)
            })
            .collect::<Vec<_>>();

        if !diagnostics.is_empty() {
            return Err(Animator2DCookFailure { diagnostics });
        }
        CookedAnimator2DRegistry::from_parts(cooked_clips, cooked_controllers)
            .map_err(|diagnostics| Animator2DCookFailure { diagnostics })
    }
}

fn cook_clip(
    index: usize,
    clip: &SpriteAnimationClip2DAsset,
    available_sprite_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) -> CookedSpriteAnimationClip2D {
    let path = format!("clips[{index}]");
    if clip.schema_version != SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "animator2d.clip_schema_unsupported",
            format!("{path}.schema"),
            format!(
                "Expected {}, got {}.",
                SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION, clip.schema_version
            ),
            "Migrate the clip asset to sprite-animation-clip-2d.v1.",
        ));
    }
    if clip.asset_id.trim().is_empty() {
        diagnostics.push(diagnostic(
            "animator2d.id_invalid",
            format!("{path}.assetId"),
            "Clip assetId must not be empty.",
            "Assign a stable clip asset id.",
        ));
    }
    if clip.frames.is_empty() {
        diagnostics.push(diagnostic(
            "animator2d.clip_frames_empty",
            format!("{path}.frames"),
            "Animation clip must contain at least one frame.",
            "Add at least one Sprite frame.",
        ));
    }
    let frames = clip
        .frames
        .iter()
        .enumerate()
        .map(|(frame_index, frame)| {
            if frame.duration_ticks == 0 {
                diagnostics.push(diagnostic(
                    "animator2d.frame_duration_invalid",
                    format!("{path}.frames[{frame_index}].durationTicks"),
                    "durationTicks must be greater than zero.",
                    "Use a positive fixed-tick duration.",
                ));
            }
            if !available_sprite_ids.contains(&frame.sprite_ref) {
                diagnostics.push(diagnostic(
                    "animator2d.sprite_missing",
                    format!("{path}.frames[{frame_index}].spriteRef"),
                    format!("Sprite asset is not available: {}.", frame.sprite_ref),
                    "Import or select an existing Sprite asset.",
                ));
            }
            CookedSpriteAnimationFrame2D {
                sprite_asset_id: frame.sprite_ref.clone(),
                duration_ticks: frame.duration_ticks,
            }
        })
        .collect();
    CookedSpriteAnimationClip2D {
        id: clip.asset_id.clone(),
        playback: clip.playback,
        frames,
    }
}

fn cook_controller(
    controller_index: usize,
    controller: &AnimatorController2DAsset,
    clip_indices: &BTreeMap<String, u32>,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) -> CookedAnimatorController2D {
    let path = format!("controllers[{controller_index}]");
    if controller.schema_version != ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "animator2d.controller_schema_unsupported",
            format!("{path}.schema"),
            format!(
                "Expected {}, got {}.",
                ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION, controller.schema_version
            ),
            "Migrate the controller asset to animator-controller-2d.v1.",
        ));
    }
    let mut parameters = controller.parameters.clone();
    parameters.sort_by(|left, right| left.id.cmp(&right.id));
    validate_unique_asset_ids(
        parameters.iter().map(|parameter| parameter.id.as_str()),
        &format!("{path}.parameters"),
        diagnostics,
    );
    let parameter_indices = parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            u32::try_from(index)
                .ok()
                .map(|index| (parameter.id.clone(), index))
        })
        .collect::<BTreeMap<_, _>>();
    let cooked_parameters = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            if parameter.kind == Animator2DParameterKind::Trigger
                && parameter.default_bool.is_some()
            {
                diagnostics.push(diagnostic(
                    "animator2d.parameter_default_invalid",
                    format!("{path}.parameters[{index}].defaultBool"),
                    "Trigger parameters cannot declare a default Bool value.",
                    "Remove defaultBool from the Trigger parameter.",
                ));
            }
            CookedAnimator2DParameter {
                id: parameter.id.clone(),
                kind: parameter.kind,
                default_bool: parameter.default_bool.unwrap_or(false),
            }
        })
        .collect::<Vec<_>>();

    let mut states = controller.states.clone();
    states.sort_by(|left, right| left.id.cmp(&right.id));
    validate_unique_asset_ids(
        states.iter().map(|state| state.id.as_str()),
        &format!("{path}.states"),
        diagnostics,
    );
    let state_indices = states
        .iter()
        .enumerate()
        .filter_map(|(index, state)| {
            u32::try_from(index)
                .ok()
                .map(|index| (state.id.clone(), index))
        })
        .collect::<BTreeMap<_, _>>();
    let cooked_states = states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let clip_index = clip_indices
                .get(&state.clip_ref)
                .copied()
                .unwrap_or_else(|| {
                    diagnostics.push(diagnostic(
                        "animator2d.clip_missing",
                        format!("{path}.states[{index}].clipRef"),
                        format!("Clip asset is not available: {}.", state.clip_ref),
                        "Assign an existing SpriteAnimationClip2D asset.",
                    ));
                    u32::MAX
                });
            if state.speed_permille == 0 {
                diagnostics.push(diagnostic(
                    "animator2d.speed_invalid",
                    format!("{path}.states[{index}].speedPermille"),
                    "speedPermille must be greater than zero.",
                    "Use a positive fixed-point playback speed.",
                ));
            }
            CookedAnimator2DState {
                id: state.id.clone(),
                clip_index,
                speed_permille: state.speed_permille,
            }
        })
        .collect::<Vec<_>>();
    let entry_state_index = state_indices
        .get(&controller.entry_state_id)
        .copied()
        .unwrap_or_else(|| {
            diagnostics.push(diagnostic(
                "animator2d.entry_state_invalid",
                format!("{path}.entryStateId"),
                format!(
                    "Entry state is not available: {}.",
                    controller.entry_state_id
                ),
                "Choose an existing Controller state as entry.",
            ));
            u32::MAX
        });

    let mut transitions = controller.transitions.clone();
    transitions.sort_by(|left, right| left.id.cmp(&right.id));
    validate_unique_asset_ids(
        transitions.iter().map(|transition| transition.id.as_str()),
        &format!("{path}.transitions"),
        diagnostics,
    );
    let cooked_transitions = transitions
        .iter()
        .enumerate()
        .map(|(index, transition)| {
            let from_state_index = resolve_state_index(
                &state_indices,
                &transition.from,
                &format!("{path}.transitions[{index}].from"),
                diagnostics,
            );
            let to_state_index = resolve_state_index(
                &state_indices,
                &transition.to,
                &format!("{path}.transitions[{index}].to"),
                diagnostics,
            );
            let conditions = transition
                .conditions
                .iter()
                .enumerate()
                .map(|(condition_index, condition)| {
                    cook_condition(
                        condition,
                        cooked_parameters.as_slice(),
                        &parameter_indices,
                        &format!("{path}.transitions[{index}].conditions[{condition_index}]"),
                        diagnostics,
                    )
                })
                .collect();
            CookedAnimator2DTransition {
                id: transition.id.clone(),
                from_state_index,
                to_state_index,
                timing: transition.timing,
                priority: transition.priority,
                conditions,
            }
        })
        .collect();

    CookedAnimatorController2D {
        id: controller.asset_id.clone(),
        entry_state_index,
        parameters: cooked_parameters,
        states: cooked_states,
        transitions: cooked_transitions,
    }
}

fn resolve_state_index(
    indices: &BTreeMap<String, u32>,
    state_id: &str,
    path: &str,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) -> u32 {
    indices.get(state_id).copied().unwrap_or_else(|| {
        diagnostics.push(diagnostic(
            "animator2d.transition_target_invalid",
            path,
            format!("Transition state is not available: {state_id}."),
            "Point the transition at an existing state.",
        ));
        u32::MAX
    })
}

fn cook_condition(
    condition: &AnimatorController2DConditionAsset,
    parameters: &[CookedAnimator2DParameter],
    parameter_indices: &BTreeMap<String, u32>,
    path: &str,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) -> CookedAnimator2DCondition {
    let parameter_index = parameter_indices
        .get(&condition.parameter)
        .copied()
        .unwrap_or(u32::MAX);
    let parameter_kind = usize::try_from(parameter_index)
        .ok()
        .and_then(|index| parameters.get(index))
        .map(|parameter| parameter.kind);
    match (condition.operation, parameter_kind, condition.value) {
        (
            AnimatorController2DConditionOperation::Equals,
            Some(Animator2DParameterKind::Bool),
            Some(value),
        ) => CookedAnimator2DCondition::BoolEquals {
            parameter_index,
            value,
        },
        (
            AnimatorController2DConditionOperation::Triggered,
            Some(Animator2DParameterKind::Trigger),
            None,
        ) => CookedAnimator2DCondition::Triggered { parameter_index },
        _ => {
            diagnostics.push(diagnostic(
                "animator2d.condition_parameter_invalid",
                path,
                format!(
                    "Condition {:?} is incompatible with parameter '{}' ({parameter_kind:?}).",
                    condition.operation, condition.parameter
                ),
                "Use equals + Bool + value, or triggered + Trigger without value.",
            ));
            match condition.operation {
                AnimatorController2DConditionOperation::Equals => {
                    CookedAnimator2DCondition::BoolEquals {
                        parameter_index,
                        value: condition.value.unwrap_or(false),
                    }
                }
                AnimatorController2DConditionOperation::Triggered => {
                    CookedAnimator2DCondition::Triggered { parameter_index }
                }
            }
        }
    }
}

fn validate_unique_asset_ids<'a>(
    ids: impl IntoIterator<Item = &'a str>,
    path: &str,
    diagnostics: &mut Vec<Animator2DDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for (index, id) in ids.into_iter().enumerate() {
        if id.trim().is_empty() {
            diagnostics.push(diagnostic(
                "animator2d.id_invalid",
                format!("{path}[{index}]"),
                "Stable id must not be empty.",
                "Assign a non-empty stable id.",
            ));
        }
        if !seen.insert(id) {
            diagnostics.push(diagnostic(
                "animator2d.id_duplicate",
                format!("{path}[{index}]"),
                format!("Duplicate stable id: {id}."),
                "Use a unique stable id in this collection.",
            ));
        }
    }
}

fn diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> Animator2DDiagnostic {
    Animator2DDiagnostic::error(code, path, message, next_action)
}

fn default_speed_permille() -> u32 {
    1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn animator2d_cook_rejects_zero_duration_and_missing_sprite() {
        let clip = SpriteAnimationClip2DAsset {
            schema_version: SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION.to_string(),
            asset_id: "clip-idle".to_string(),
            playback: engine_runtime::animator2d::Animator2DPlayback::Loop,
            frames: vec![SpriteAnimationFrame2DAsset {
                sprite_ref: "sprite-missing".to_string(),
                duration_ticks: 0,
            }],
        };
        let failure =
            Animator2DAssetCooker::cook(vec![clip], Vec::new(), &BTreeSet::new()).unwrap_err();
        assert!(failure
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "animator2d.frame_duration_invalid"));
        assert!(failure
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "animator2d.sprite_missing"));
    }

    #[test]
    fn animator2d_cook_rejects_condition_parameter_kind_mismatch() {
        let controller = fixture_controller_with_trigger_bool_condition();
        let failure = Animator2DAssetCooker::cook(
            vec![fixture_clip()],
            vec![controller],
            &BTreeSet::from(["sprite-idle".to_string()]),
        )
        .unwrap_err();
        assert!(failure
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "animator2d.condition_parameter_invalid" }));
    }

    #[test]
    fn animator2d_cook_rejects_unknown_fields_and_schema() {
        let parse = Animator2DAssetCooker::parse_clip_json(
            "Animations/idle.sprite-animation-clip-2d.json",
            r#"{"schema":"sprite-animation-clip-2d.v1","assetId":"idle","playback":"loop","frames":[],"unexpected":true}"#,
        );
        assert!(parse.is_err());

        let mut clip = fixture_clip();
        clip.schema_version = "sprite-animation-clip-2d.v0".to_string();
        let failure = Animator2DAssetCooker::cook(
            vec![clip],
            Vec::new(),
            &BTreeSet::from(["sprite-idle".to_string()]),
        )
        .unwrap_err();
        assert!(failure
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "animator2d.clip_schema_unsupported" }));
    }

    #[test]
    fn animator2d_cook_rejects_duplicate_ids_and_missing_references() {
        let mut controller = fixture_controller();
        controller.states.push(controller.states[0].clone());
        controller.entry_state_id = "missing".to_string();
        controller.states[0].clip_ref = "missing-clip".to_string();
        let failure = Animator2DAssetCooker::cook(
            vec![fixture_clip(), fixture_clip()],
            vec![controller],
            &BTreeSet::from(["sprite-idle".to_string()]),
        )
        .unwrap_err();
        for expected in [
            "animator2d.id_duplicate",
            "animator2d.clip_missing",
            "animator2d.entry_state_invalid",
        ] {
            assert!(failure
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected));
        }
    }

    #[test]
    fn animator2d_cook_is_stable_for_permuted_assets_and_sensitive_to_semantics() {
        let mut second_clip = fixture_clip();
        second_clip.asset_id = "clip-run".to_string();
        second_clip.frames[0].sprite_ref = "sprite-run".to_string();
        let sprites = BTreeSet::from(["sprite-idle".to_string(), "sprite-run".to_string()]);
        let first = Animator2DAssetCooker::cook(
            vec![second_clip.clone(), fixture_clip()],
            vec![fixture_controller()],
            &sprites,
        )
        .unwrap();
        let second = Animator2DAssetCooker::cook(
            vec![fixture_clip(), second_clip.clone()],
            vec![fixture_controller()],
            &sprites,
        )
        .unwrap();
        assert_eq!(first, second);

        second_clip.frames[0].duration_ticks += 1;
        let changed = Animator2DAssetCooker::cook(
            vec![fixture_clip(), second_clip],
            vec![fixture_controller()],
            &sprites,
        )
        .unwrap();
        assert_ne!(first.registry_digest, changed.registry_digest);
    }

    #[test]
    fn animator2d_cook_resolves_stable_indices_and_transition_order() {
        let mut controller = fixture_controller();
        controller.transitions = vec![
            AnimatorController2DTransitionAsset {
                id: "z-last".to_string(),
                from: "idle".to_string(),
                to: "idle".to_string(),
                timing: Animator2DTransitionTiming::ClipEnd,
                priority: 1,
                conditions: Vec::new(),
            },
            AnimatorController2DTransitionAsset {
                id: "a-first".to_string(),
                from: "idle".to_string(),
                to: "idle".to_string(),
                timing: Animator2DTransitionTiming::Immediate,
                priority: 9,
                conditions: Vec::new(),
            },
        ];
        let registry = Animator2DAssetCooker::cook(
            vec![fixture_clip()],
            vec![controller],
            &BTreeSet::from(["sprite-idle".to_string()]),
        )
        .unwrap();
        assert_eq!(registry.controllers[0].entry_state_index, 0);
        assert_eq!(registry.controllers[0].states[0].clip_index, 0);
        assert_eq!(registry.controllers[0].transitions[0].id, "a-first");
        assert_eq!(registry.controllers[0].transitions[1].id, "z-last");
    }

    fn fixture_clip() -> SpriteAnimationClip2DAsset {
        SpriteAnimationClip2DAsset {
            schema_version: SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION.to_string(),
            asset_id: "clip-idle".to_string(),
            playback: engine_runtime::animator2d::Animator2DPlayback::Loop,
            frames: vec![SpriteAnimationFrame2DAsset {
                sprite_ref: "sprite-idle".to_string(),
                duration_ticks: 2,
            }],
        }
    }

    fn fixture_controller_with_trigger_bool_condition() -> AnimatorController2DAsset {
        AnimatorController2DAsset {
            schema_version: ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION.to_string(),
            asset_id: "controller".to_string(),
            parameters: vec![AnimatorController2DParameterAsset {
                id: "attack".to_string(),
                kind: engine_runtime::animator2d::Animator2DParameterKind::Trigger,
                default_bool: None,
            }],
            entry_state_id: "idle".to_string(),
            states: vec![AnimatorController2DStateAsset {
                id: "idle".to_string(),
                clip_ref: "clip-idle".to_string(),
                speed_permille: 1000,
            }],
            transitions: vec![AnimatorController2DTransitionAsset {
                id: "invalid".to_string(),
                from: "idle".to_string(),
                to: "idle".to_string(),
                timing: engine_runtime::animator2d::Animator2DTransitionTiming::Immediate,
                priority: 10,
                conditions: vec![AnimatorController2DConditionAsset {
                    parameter: "attack".to_string(),
                    operation: AnimatorController2DConditionOperation::Equals,
                    value: Some(true),
                }],
            }],
        }
    }

    fn fixture_controller() -> AnimatorController2DAsset {
        AnimatorController2DAsset {
            schema_version: ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION.to_string(),
            asset_id: "controller".to_string(),
            parameters: Vec::new(),
            entry_state_id: "idle".to_string(),
            states: vec![AnimatorController2DStateAsset {
                id: "idle".to_string(),
                clip_ref: "clip-idle".to_string(),
                speed_permille: 1000,
            }],
            transitions: Vec::new(),
        }
    }
}
