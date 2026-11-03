//! Project-facing schema and safe helpers for the stable project runtime ABI.

pub mod animator;
pub mod generated_content;
pub mod project_storage;
pub mod aui;
pub mod aui_animation;
pub mod aui_draw2d;
pub mod aui_extension;
pub mod aui_host;
pub mod aui_presentation;
pub mod aui_wire;
pub use animator::ProjectRuntimeAnimatorIntent;

use project_runtime_abi::{
    hex_digest, ProjectRuntimeAbiStatus, ProjectRuntimeByteBuffer, ProjectRuntimeByteSlice,
    ProjectRuntimeCallContext, ProjectRuntimeHostCall, ProjectRuntimeModuleCall,
    ProjectRuntimeOpaqueHandle, PROJECT_RUNTIME_ABI_SCHEMA,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

mod cells;
pub use cells::*;
mod spatial;
mod world_streaming;
pub use world_streaming::*;
mod world_persistence;
pub use world_persistence::*;
pub mod physics_query3d;
pub mod spatial_bulk;
pub use spatial::*;

pub const PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES: usize = 256 * 1024;

pub const PROJECT_RUNTIME_SDK_SCHEMA: &str = concat!(
    "project-runtime-sdk.v1;json-utf8;",
    "descriptor;rules;session-create;session-call;rule-call;aui-actions;aui-events.v1;aui-extension.v2;",
    "fixed-update;conditional-ui-state;virtual-list-snapshots;observations;world-query;world-read;",
    "rule-input-actions;rule-collision-pairs;deferred-mutations;spawn-despawn;animator2d-session-intents;audio-source-session-intents;lifecycle-focus;host-control-exit;spatial-session-intents.v1;world-entity-generation.v1;spatial-prefab-receipts.v1;spatial-foundation-intents.v1;cell-session-intents.v1;world-identity-query.v1;spatial-bulk.v1;animator-session-intents.v1;character3d-session-intents.v1;physics3d-session-intents.v1;world-streaming3d-session-intents.v1;world-logical3d-session-intents.v1;vegetation3d-promotion-session-intents.v1;physics3d-world-query.v1;generated-content-session.v1;spatial-bulk.v2;project-storage-session.v1"
);

pub struct ProjectRuntimeAotDigestSource<'a> {
    pub relative_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeAotDigestPayload<'a> {
    module_id: &'a str,
    interface_version: &'a str,
    cargo_manifest: &'a str,
    cargo_package: &'a str,
    player_binary: &'a str,
    sources: Vec<ProjectRuntimeAotDigestSourceHash<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeAotDigestSourceHash<'a> {
    relative_path: &'a str,
    content_hash: String,
}

pub fn project_runtime_aot_digest<'a>(
    module_id: &str,
    interface_version: &str,
    cargo_manifest: &str,
    cargo_package: &str,
    player_binary: &str,
    sources: impl IntoIterator<Item = ProjectRuntimeAotDigestSource<'a>>,
) -> Result<String, serde_json::Error> {
    let mut sources = sources
        .into_iter()
        .map(|source| ProjectRuntimeAotDigestSourceHash {
            relative_path: source.relative_path,
            content_hash: format!("sha256:{:x}", Sha256::digest(source.bytes)),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.relative_path.cmp(right.relative_path));
    let payload = serde_json::to_value(ProjectRuntimeAotDigestPayload {
        module_id,
        interface_version,
        cargo_manifest,
        cargo_package,
        player_binary,
        sources,
    })?;
    let payload = serde_json::to_vec(&payload)?;
    let mut hasher = Sha256::new();
    hasher.update(b"AIFE-CONSISTENCY\0");
    for field in [
        b"consistency-digest.v1".as_slice(),
        b"project-runtime-module-aot-input".as_slice(),
        b"project-runtime-module-aot-input.v1".as_slice(),
        payload.as_slice(),
    ] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeModuleDescriptor {
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
    pub ui_state_producer_id: String,
    pub rules: Vec<ProjectRuntimeRuleDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeRuleDescriptor {
    pub rule_id: String,
    pub artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeSessionCreateRequest {
    pub project_id: String,
    pub module_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeSessionCreateResponse {
    pub handle_value: u64,
    pub handle_generation: u64,
}

impl From<ProjectRuntimeSessionCreateResponse> for ProjectRuntimeOpaqueHandle {
    fn from(value: ProjectRuntimeSessionCreateResponse) -> Self {
        Self {
            value: value.handle_value,
            generation: value.handle_generation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeFrameRequest {
    pub frame_index: u64,
    pub time: ProjectRuntimeTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeLifecycleEvent {
    FocusChanged { focused: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeLifecycleRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub event: ProjectRuntimeLifecycleEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeHostControl {
    Exit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTime {
    pub time: f32,
    pub delta_time: f32,
    pub unscaled_time: f32,
    pub unscaled_delta_time: f32,
    pub fixed_time: f32,
    pub fixed_delta_time: f32,
    pub frame_count: u64,
    pub fixed_frame_count: u64,
    pub time_scale: f32,
    pub in_fixed_step: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeRuleRequest {
    pub rule_id: String,
    pub frame: ProjectRuntimeFrameRequest,
    #[serde(default)]
    pub input_actions: Vec<ProjectRuntimeInputAction>,
    #[serde(default)]
    pub collision_pairs: Vec<ProjectRuntimeCollisionPair>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeInputAction {
    pub action_id: String,
    pub phase: Option<String>,
    pub axis1: Option<f32>,
    /// Axis vector, or target-canvas pixel coordinates when phase is `pointer`.
    /// Pointer coordinates retain the Player's display-to-target mapping.
    pub axis2: Option<[f32; 2]>,
}

impl ProjectRuntimeInputAction {
    /// Phase of this delivered action, not an engine-wide state lookup.
    pub fn input_phase(&self) -> Option<ProjectRuntimeInputPhase> {
        match self.phase.as_deref() {
            Some("pressed") => Some(ProjectRuntimeInputPhase::Pressed),
            Some("held") => Some(ProjectRuntimeInputPhase::Held),
            Some("released") => Some(ProjectRuntimeInputPhase::Released),
            Some("changed") => Some(ProjectRuntimeInputPhase::Changed),
            Some("pointer") => Some(ProjectRuntimeInputPhase::Pointer),
            _ => None,
        }
    }

    pub fn is_pressed(&self) -> bool {
        matches!(self.phase.as_deref(), Some("pressed" | "held"))
    }

    pub fn is_just_pressed(&self) -> bool {
        self.input_phase() == Some(ProjectRuntimeInputPhase::Pressed)
    }

    pub fn is_just_released(&self) -> bool {
        self.input_phase() == Some(ProjectRuntimeInputPhase::Released)
    }
}

/// Typed view of known wire phases; the wire remains forward-compatible strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRuntimeInputPhase {
    Pressed,
    Held,
    Released,
    Changed,
    Pointer,
}

#[cfg(test)]
mod input_phase_tests {
    use super::*;

    #[test]
    fn delivered_input_distinguishes_edges_held_and_unknown_without_changing_wire() {
        for (phase, pressed, down, up) in [
            (Some("pressed"), true, true, false),
            (Some("held"), true, false, false),
            (Some("released"), false, false, true),
            (Some("changed"), false, false, false),
            (Some("pointer"), false, false, false),
            (Some("future-phase"), false, false, false),
            (None, false, false, false),
        ] {
            let action = ProjectRuntimeInputAction {
                action_id: "move".into(),
                phase: phase.map(str::to_string),
                axis1: None,
                axis2: None,
            };
            assert_eq!(
                (
                    action.is_pressed(),
                    action.is_just_pressed(),
                    action.is_just_released()
                ),
                (pressed, down, up)
            );
            assert_eq!(
                action.input_phase().is_none(),
                phase.is_none() || phase == Some("future-phase")
            );
            let wire = serde_json::to_string(&action).unwrap();
            assert_eq!(
                serde_json::from_str::<ProjectRuntimeInputAction>(&wire).unwrap(),
                action
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeCollisionPair {
    pub entity_a: String,
    pub entity_b: String,
    pub is_sensor_pair: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeRuleOutput {
    pub status: ProjectRuntimeStatus,
    pub mutations: Vec<ProjectRuntimeDeferredMutation>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeAuiActionRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub actions: Vec<ProjectRuntimeAuiAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeAuiAction {
    pub action_id: String,
    pub node_id: String,
    pub event: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateIdentity {
    pub producer_epoch: u64,
    pub visible_revision: u64,
    pub binding_set_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeUiBindingSet {
    Known {
        digest: String,
    },
    Replace {
        digest: String,
        active_binding_paths: Vec<String>,
    },
}

impl ProjectRuntimeUiBindingSet {
    pub fn digest(&self) -> &str {
        match self {
            Self::Known { digest } | Self::Replace { digest, .. } => digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateResolveRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub previous_identity: Option<ProjectRuntimeUiStateIdentity>,
    pub binding_set: ProjectRuntimeUiBindingSet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeVirtualListItem {
    pub item_key: String,
    pub text: String,
    pub measured_extent: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeVirtualListSnapshot {
    pub items: Vec<ProjectRuntimeVirtualListItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeUiStateResolveOutput {
    Reuse {
        identity: ProjectRuntimeUiStateIdentity,
    },
    Replace {
        identity: ProjectRuntimeUiStateIdentity,
        producer_id: String,
        values: BTreeMap<String, ProjectRuntimeValue>,
    },
    Uncacheable {
        producer_id: String,
        values: BTreeMap<String, ProjectRuntimeValue>,
    },
    ReplaceWithVirtualLists {
        identity: ProjectRuntimeUiStateIdentity,
        producer_id: String,
        values: BTreeMap<String, ProjectRuntimeValue>,
        virtual_lists: BTreeMap<String, ProjectRuntimeVirtualListSnapshot>,
    },
    UncacheableWithVirtualLists {
        producer_id: String,
        values: BTreeMap<String, ProjectRuntimeValue>,
        virtual_lists: BTreeMap<String, ProjectRuntimeVirtualListSnapshot>,
    },
}

/// Temporary source compatibility for in-process fixtures. Native production modules use the
/// conditional resolve types above.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateRequest {
    pub frame: ProjectRuntimeFrameRequest,
    pub active_binding_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeUiStateOutput {
    pub producer_id: String,
    pub values: BTreeMap<String, ProjectRuntimeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeObservationOutput {
    pub values: BTreeMap<String, ProjectRuntimeValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeStatus {
    Applied,
    NoOp,
    Unhandled,
    Rejected,
    Faulted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeSessionOutput {
    pub host_control: Option<ProjectRuntimeHostControl>,
    pub status: ProjectRuntimeStatus,
    pub handled_action_count: u64,
    pub unhandled_action_count: u64,
    pub rejected_action_count: u64,
    pub mutations: Vec<ProjectRuntimeDeferredMutation>,
    pub diagnostics: Vec<String>,
}

impl ProjectRuntimeSessionOutput {
    /// Request an orderly session exit. The host publishes it only after successful commit.
    pub fn request_exit(&mut self) {
        self.host_control = Some(ProjectRuntimeHostControl::Exit);
        self.status = ProjectRuntimeStatus::Applied;
    }

    pub fn no_op() -> Self {
        Self {
            host_control: None,
            status: ProjectRuntimeStatus::NoOp,
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectRuntimeDeferredMutation {
    GeneratedContent {
        request: generated_content::GeneratedContentRequest,
    },
    GeneratedContentCancel {
        ticket: generated_content::ContentTicket,
    },
    ProjectStorage {
        request: project_storage::ProjectStorageRequest,
    },
    ProjectStorageAbandon {
        ticket: generated_content::ContentTicket,
    },
    Animator {
        entity_id: String,
        generation: u64,
        intent: ProjectRuntimeAnimatorIntent,
    },
    SpatialLocalTransform {
        entity: spatial_bulk::SpatialEntityKey,
        transform: ProjectRuntimeTransform3D,
    },
    Cell {
        intent: ProjectRuntimeCellIntent,
    },
    InstantiateSpatialPrefab {
        request_id: u64,
        prefab: ProjectRuntimeSpatialAssetRef,
    },
    DespawnSpatialPrefab {
        entity_id: String,
        generation: u64,
    },
    Spatial {
        #[serde(rename = "entityId")]
        entity_id: String,
        generation: u64,
        intent: ProjectRuntimeSpatialIntent,
    },
    ParticleEffect {
        entity_id: String,
        #[serde(default)]
        generation: u64,
        intent: ProjectRuntimeParticleIntent,
    },
    AudioSource {
        entity_id: String,
        intent: ProjectRuntimeAudioSourceIntent,
    },
    #[serde(rename = "animator2d")]
    Animator2D {
        entity_id: String,
        intent: ProjectRuntimeAnimator2DIntent,
    },
    WriteTransform {
        entity_id: String,
        transform: ProjectRuntimeTransform,
    },
    WriteComponentField {
        entity_id: String,
        component_type: String,
        field_path: String,
        value: ProjectRuntimeValue,
    },
    ReplaceDynamicComponent {
        entity_id: String,
        component_type: String,
        fields: BTreeMap<String, ProjectRuntimeValue>,
    },
    InstantiatePrefab {
        prefab_id: String,
        /// Optional root position; preserves the prefab's rotation and scale.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<[f32; 3]>,
    },
    DespawnEntity {
        entity_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeAnimator2DIntent {
    SetBool { name: String, value: bool },
    Play { name: String },
    Resume,
    SetPaused { paused: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeAudioSourceIntent {
    Play {},
    Stop {},
    SetPaused { paused: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeParticleIntent {
    Play {},
    Restart {},
    StopEmitting {},
    Clear {},
    SetPaused {
        paused: bool,
    },
    SetParameter {
        name: String,
        value: ProjectRuntimeParticleValue,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectRuntimeParticleValue {
    Float(f32),
    Uint(u32),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

#[cfg(test)]
mod audio_source_wire_tests {
    use super::*;

    #[test]
    fn audio_source_intents_preserve_wire_contract_and_reject_unknown_fields() {
        for intent in [
            serde_json::json!({"operation":"play"}),
            serde_json::json!({"operation":"stop"}),
            serde_json::json!({"operation":"set_paused","paused":true}),
            serde_json::json!({"operation":"set_paused","paused":false}),
        ] {
            let wire =
                serde_json::json!({"kind":"audio_source","entity_id":"speaker","intent":intent});
            let mutation: ProjectRuntimeDeferredMutation =
                serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(mutation).unwrap(), wire);
        }
        assert!(serde_json::from_value::<ProjectRuntimeAudioSourceIntent>(
            serde_json::json!({"operation":"play","clip":"path.wav"})
        )
        .is_err());
        assert!(serde_json::from_value::<ProjectRuntimeAudioSourceIntent>(
            serde_json::json!({"operation":"stop","clip":"path.wav"})
        )
        .is_err());
        assert!(serde_json::from_value::<ProjectRuntimeAudioSourceIntent>(
            serde_json::json!({"operation":"set_paused","paused":true,"clip":"path.wav"})
        )
        .is_err());
        assert!(serde_json::from_value::<ProjectRuntimeAudioSourceIntent>(
            serde_json::json!({"operation":"set_paused"})
        )
        .is_err());
    }
}

#[cfg(test)]
mod spawn_position_tests {
    use super::*;

    #[test]
    fn prefab_spawn_wire_preserves_requested_position_and_accepts_legacy_request() {
        let input = serde_json::json!({"kind":"instantiate_prefab", "prefab_id":"prefab-test", "position":[3.5,-3.25,0.0]});
        let mutation: ProjectRuntimeDeferredMutation =
            serde_json::from_value(input.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(mutation).unwrap()["position"],
            input["position"]
        );
        let legacy = serde_json::json!({"kind":"instantiate_prefab", "prefab_id":"prefab-test"});
        let mutation: ProjectRuntimeDeferredMutation =
            serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(serde_json::to_value(mutation).unwrap(), legacy);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ProjectRuntimeValue {
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
    EntityRef(String),
    AssetRef(String),
    Object(BTreeMap<String, ProjectRuntimeValue>),
    Array(Vec<ProjectRuntimeValue>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldQueryRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_content: Option<generated_content::ContentQuery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_storage: Option<project_storage::StorageQuery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_storage_take: Option<generated_content::ContentTicket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<physics_query3d::PhysicsQueryBatch3D>,
    /// Resolve one live incarnation. Exclusive with other query selectors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<ProjectRuntimeCellQuery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_request_id: Option<u64>,
    #[serde(default)]
    pub all: Vec<String>,
    #[serde(default)]
    pub none: Vec<String>,
}

impl ProjectRuntimeWorldQueryRequest {
    pub fn validate_identity_query(&self) -> Result<(), ProjectRuntimeAbiStatus> {
        let content_selectors = usize::from(self.generated_content.is_some())
            + usize::from(self.project_storage.is_some())
            + usize::from(self.project_storage_take.is_some());
        if content_selectors != 0 {
            return if content_selectors != 1
                || self.physics.is_some()
                || self.entity_id.is_some()
                || self.cell.is_some()
                || self.spawn_request_id.is_some()
                || !self.all.is_empty()
                || !self.none.is_empty()
            {
                Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT)
            } else {
                Ok(())
            };
        }
        if let Some(batch) = &self.physics {
            if !batch.is_valid()
                || self.entity_id.is_some()
                || self.cell.is_some()
                || self.spawn_request_id.is_some()
                || !self.all.is_empty()
                || !self.none.is_empty()
            {
                return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
            }
        }
        if let Some(id) = &self.entity_id {
            if id.trim().is_empty()
                || self.cell.is_some()
                || self.spawn_request_id.is_some()
                || !self.all.is_empty()
                || !self.none.is_empty()
            {
                return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldQueryResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<physics_query3d::PhysicsQueryResult3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<ProjectRuntimeCellReceipt>,
    pub entity_ids: Vec<String>,
    pub entity_generations: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldReadRequest {
    pub entity_id: String,
    pub component_type: String,
    #[serde(default)]
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeWorldReadResponse {
    pub value: ProjectRuntimeValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeSdkError {
    pub status: ProjectRuntimeAbiStatus,
    pub message: String,
}

pub fn project_runtime_contract_digest() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_RUNTIME_ABI_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(PROJECT_RUNTIME_SDK_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(generated_content::GENERATED_CONTENT_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(project_storage::PROJECT_STORAGE_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(aui_wire::UI_UPDATE_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(aui_host::UI_HOST_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(aui_host::UI_STRUCTURE_SCHEMA.as_bytes());
    hasher.update([0]);
    hasher.update(aui_extension::UI_EXTENSION_SCHEMA.as_bytes());
    hasher.finalize().into()
}

pub fn project_runtime_contract_digest_hex() -> String {
    hex_digest(project_runtime_contract_digest())
}

pub fn ffi_boundary<T: Serialize>(
    output: *mut ProjectRuntimeByteBuffer,
    callback: impl FnOnce() -> Result<T, ProjectRuntimeAbiStatus>,
) -> ProjectRuntimeAbiStatus {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(Ok(value)) => match serde_json::to_vec(&value) {
            Ok(bytes) => write_output(output, &bytes),
            Err(_) => ProjectRuntimeAbiStatus::FAILED,
        },
        Ok(Err(status)) => status,
        Err(_) => ProjectRuntimeAbiStatus::PANICKED,
    }
}

/// Rejects an undersized caller buffer before invoking a stateful module callback.
///
/// The regular caller-owned-buffer retry protocol may invoke a callback more than once. Project
/// callbacks that mutate session state must use a capacity floor large enough for their bounded
/// output so the retry only performs sizing and the callback itself executes exactly once.
/// # Safety
/// A non-null `output` must point to a live, aligned, exclusively borrowed descriptor.
/// Its non-null data pointer must reference `capacity` writable bytes for this call.
/// Neither allocation may be accessed by the callback or invalidated until return.
pub unsafe fn ffi_boundary_with_capacity_floor<T: Serialize>(
    output: *mut ProjectRuntimeByteBuffer,
    capacity_floor: usize,
    callback: impl FnOnce() -> Result<T, ProjectRuntimeAbiStatus>,
) -> ProjectRuntimeAbiStatus {
    if output.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    let Ok(required) = u64::try_from(capacity_floor) else {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    };
    // SAFETY: null was rejected; validity and exclusive access are required of the caller.
    let output_ref = unsafe { &mut *output };
    if output_ref.capacity < required {
        output_ref.written = required;
        return ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL;
    }
    if required > 0 && output_ref.data.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    ffi_boundary(output, callback)
}

/// # Safety
/// `input.data` must point to `input.length` readable bytes for this call, or be null when length is
/// zero. The bytes are copied before this function returns.
pub unsafe fn read_input<T: DeserializeOwned>(
    input: ProjectRuntimeByteSlice,
) -> Result<T, ProjectRuntimeAbiStatus> {
    let length =
        usize::try_from(input.length).map_err(|_| ProjectRuntimeAbiStatus::INVALID_ARGUMENT)?;
    if length > 0 && input.data.is_null() {
        return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    }
    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: upheld by the caller contract above.
        unsafe { std::slice::from_raw_parts(input.data, length) }
    };
    serde_json::from_slice(bytes).map_err(|_| ProjectRuntimeAbiStatus::INVALID_ARGUMENT)
}

pub fn call_json<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeModuleCall,
    module_context: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    call_context: Option<&ProjectRuntimeCallContext>,
    request: &Request,
) -> Result<Response, ProjectRuntimeSdkError> {
    let request = serde_json::to_vec(request).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
        message: error.to_string(),
    })?;
    let mut bytes = vec![0_u8; 4096];
    for _ in 0..2 {
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: all pointers borrow live values for only this call; the callee must follow ABI.
        let status = unsafe {
            call(
                module_context,
                session,
                call_context.map_or(std::ptr::null(), std::ptr::from_ref),
                ProjectRuntimeByteSlice::from_slice(&request),
                &mut output,
            )
        };
        if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            let required = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "module returned an invalid output length".to_string(),
            })?;
            bytes.resize(required, 0);
            continue;
        }
        if !status.is_ok() {
            return Err(ProjectRuntimeSdkError {
                status,
                message: format!("project runtime ABI call failed with status {}", status.0),
            });
        }
        let written = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: "module returned an invalid output length".to_string(),
        })?;
        if written > bytes.len() {
            return Err(ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "module wrote beyond the caller-owned output buffer".to_string(),
            });
        }
        return serde_json::from_slice(&bytes[..written]).map_err(|error| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: error.to_string(),
        });
    }
    Err(ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: "module output size changed after retry".to_string(),
    })
}

pub fn call_json_once_with_buffer<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeModuleCall,
    module_context: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    call_context: Option<&ProjectRuntimeCallContext>,
    request: &Request,
    bytes: &mut [u8],
) -> Result<Response, ProjectRuntimeSdkError> {
    call_once_with_buffer_decoder(
        call,
        module_context,
        session,
        call_context,
        request,
        bytes,
        |data| serde_json::from_slice(data).map_err(|_| ProjectRuntimeAbiStatus::FAILED),
    )
}

pub fn call_once_with_buffer_decoder<Request: Serialize, Response>(
    call: ProjectRuntimeModuleCall,
    module_context: ProjectRuntimeOpaqueHandle,
    session: ProjectRuntimeOpaqueHandle,
    call_context: Option<&ProjectRuntimeCallContext>,
    request: &Request,
    bytes: &mut [u8],
    decode: impl FnOnce(&[u8]) -> Result<Response, ProjectRuntimeAbiStatus>,
) -> Result<Response, ProjectRuntimeSdkError> {
    let request = serde_json::to_vec(request).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
        message: error.to_string(),
    })?;
    let mut output = ProjectRuntimeByteBuffer::from_slice(bytes);
    // SAFETY: all pointers borrow live values for only this call; the callee must follow ABI.
    let status = unsafe {
        call(
            module_context,
            session,
            call_context.map_or(std::ptr::null(), std::ptr::from_ref),
            ProjectRuntimeByteSlice::from_slice(&request),
            &mut output,
        )
    };
    if !status.is_ok() {
        let detail = if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            format!(
                "stateful project runtime output requires {} bytes but the one-shot buffer has {} bytes",
                output.written, output.capacity
            )
        } else {
            format!("project runtime ABI call failed with status {}", status.0)
        };
        return Err(ProjectRuntimeSdkError {
            status,
            message: detail,
        });
    }
    let written = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: "module returned an invalid output length".to_string(),
    })?;
    if written > bytes.len() {
        return Err(ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: "module wrote beyond the caller-owned output buffer".to_string(),
        });
    }
    decode(&bytes[..written]).map_err(|status| ProjectRuntimeSdkError {
        status,
        message: "Invalid or unsupported one-shot session output".into(),
    })
}

pub fn call_host_json<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeHostCall,
    host_context: ProjectRuntimeOpaqueHandle,
    request: &Request,
) -> Result<Response, ProjectRuntimeSdkError> {
    call_host_json_with_capacity(call, host_context, request, 4096)
}

/// Reserves caller storage for a known large reply, avoiding a sizing retry in ordinary batches.
/// This is an initial capacity hint; larger replies retain the same caller-owned retry protocol.
pub fn call_host_json_with_capacity<Request: Serialize, Response: DeserializeOwned>(
    call: ProjectRuntimeHostCall,
    host_context: ProjectRuntimeOpaqueHandle,
    request: &Request,
    initial_capacity: usize,
) -> Result<Response, ProjectRuntimeSdkError> {
    let request = serde_json::to_vec(request).map_err(|error| ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::INVALID_ARGUMENT,
        message: error.to_string(),
    })?;
    let mut bytes = vec![0_u8; initial_capacity];
    for _ in 0..2 {
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: all pointers borrow caller-owned storage for only this callback.
        let status = unsafe {
            call(
                host_context,
                ProjectRuntimeByteSlice::from_slice(&request),
                &mut output,
            )
        };
        if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL {
            let required = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "host returned an invalid output length".to_string(),
            })?;
            bytes.resize(required, 0);
            continue;
        }
        if !status.is_ok() {
            return Err(ProjectRuntimeSdkError {
                status,
                message: format!("project runtime host call failed with status {}", status.0),
            });
        }
        let written = usize::try_from(output.written).map_err(|_| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: "host returned an invalid output length".to_string(),
        })?;
        if written > bytes.len() {
            return Err(ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "host wrote beyond the caller-owned output buffer".to_string(),
            });
        }
        return serde_json::from_slice(&bytes[..written]).map_err(|error| ProjectRuntimeSdkError {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: error.to_string(),
        });
    }
    Err(ProjectRuntimeSdkError {
        status: ProjectRuntimeAbiStatus::FAILED,
        message: "host output size changed after retry".to_string(),
    })
}

fn write_output(output: *mut ProjectRuntimeByteBuffer, bytes: &[u8]) -> ProjectRuntimeAbiStatus {
    if output.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    // SAFETY: null was rejected and the ABI caller owns a live buffer descriptor for this call.
    let output = unsafe { &mut *output };
    output.written = bytes.len() as u64;
    if output.capacity < bytes.len() as u64 {
        return ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL;
    }
    if !bytes.is_empty() && output.data.is_null() {
        return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
    }
    if !bytes.is_empty() {
        // SAFETY: capacity was checked and the caller promises writable memory for the call.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output.data, bytes.len()) };
    }
    ProjectRuntimeAbiStatus::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_query_is_exclusive_bounded_and_old_reader_rejects_it() {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct OldQuery {
            #[serde(rename = "all")]
            _all: Option<Vec<String>>,
            #[serde(rename = "none")]
            _none: Option<Vec<String>>,
        }
        let query: ProjectRuntimeWorldQueryRequest =
            serde_json::from_value(serde_json::json!({"entityId":"camera","all":[],"none":[]}))
                .unwrap();
        assert!(query.validate_identity_query().is_ok());
        assert!(serde_json::from_value::<OldQuery>(serde_json::to_value(&query).unwrap()).is_err());
        for patch in [
            serde_json::json!({"entityId":""}),
            serde_json::json!({"all":["engine.transform3d"]}),
            serde_json::json!({"none":["x"]}),
            serde_json::json!({"spawnRequestId":1}),
            serde_json::json!({"cell":{"requestId":1}}),
        ] {
            let mut wire = serde_json::to_value(&query).unwrap();
            wire.as_object_mut()
                .unwrap()
                .extend(patch.as_object().unwrap().clone());
            let invalid: ProjectRuntimeWorldQueryRequest = serde_json::from_value(wire).unwrap();
            assert_eq!(
                invalid.validate_identity_query(),
                Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT)
            );
        }
        let legacy: ProjectRuntimeWorldQueryRequest =
            serde_json::from_value(serde_json::json!({"all":[],"none":[]})).unwrap();
        assert!(legacy.entity_id.is_none());
        assert!(legacy.validate_identity_query().is_ok());
        assert!(serde_json::from_value::<OldQuery>(serde_json::to_value(legacy).unwrap()).is_ok());
    }

    unsafe extern "C" fn echo_call(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        ffi_boundary(output, || {
            // SAFETY: the test caller provides the serialized request for this call.
            let value: ProjectRuntimeFrameRequest = unsafe { read_input(request) }?;
            Ok(value)
        })
    }

    #[test]
    fn sdk_digest_is_stable_and_includes_wire_schema() {
        let mut legacy = Sha256::new();
        legacy.update(PROJECT_RUNTIME_ABI_SCHEMA.as_bytes());
        legacy.update([0]);
        legacy.update(PROJECT_RUNTIME_SDK_SCHEMA.as_bytes());
        let legacy: [u8; 32] = legacy.finalize().into();
        assert_ne!(project_runtime_contract_digest(), legacy);
        assert_eq!(
            project_runtime_contract_digest(),
            project_runtime_contract_digest()
        );
        assert_eq!(project_runtime_contract_digest_hex().len(), 64);
        assert_ne!(
            project_runtime_contract_digest(),
            project_runtime_abi::project_runtime_abi_digest()
        );
    }

    #[test]
    fn caller_owned_buffer_retries_without_transferring_memory() {
        let request = ProjectRuntimeFrameRequest {
            frame_index: 9,
            time: ProjectRuntimeTime {
                delta_time: 0.25,
                ..ProjectRuntimeTime::default()
            },
        };
        let response: ProjectRuntimeFrameRequest = call_json(
            echo_call,
            ProjectRuntimeOpaqueHandle::NULL,
            ProjectRuntimeOpaqueHandle::NULL,
            None,
            &request,
        )
        .expect("echo call");
        assert_eq!(response, request);
    }

    #[test]
    fn facade_contains_panics_before_the_extern_boundary() {
        let mut bytes = [0_u8; 16];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        let status = ffi_boundary::<ProjectRuntimeFrameRequest>(&mut output, || {
            panic!("project callback panic")
        });
        assert_eq!(status, ProjectRuntimeAbiStatus::PANICKED);
    }

    #[test]
    fn invalid_input_and_small_output_fail_closed() {
        // SAFETY: an empty slice is the only valid null input.
        let invalid = unsafe {
            read_input::<ProjectRuntimeFrameRequest>(ProjectRuntimeByteSlice {
                data: std::ptr::null(),
                length: 4,
            })
        };
        assert_eq!(
            invalid.unwrap_err(),
            ProjectRuntimeAbiStatus::INVALID_ARGUMENT
        );

        let mut bytes = [0_u8; 1];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        let status = ffi_boundary(&mut output, || Ok(ProjectRuntimeSessionOutput::no_op()));
        assert_eq!(status, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert!(output.written > output.capacity);
    }

    #[test]
    fn stateful_capacity_floor_sizes_without_invoking_the_callback() {
        let mut small_bytes = [0_u8; 8];
        let mut small_output = ProjectRuntimeByteBuffer::from_slice(&mut small_bytes);
        let callback_count = std::cell::Cell::new(0);
        // SAFETY: the descriptor and backing storage remain exclusively borrowed for this call.
        let status = unsafe {
            ffi_boundary_with_capacity_floor(&mut small_output, 4096, || {
                callback_count.set(callback_count.get() + 1);
                Ok(ProjectRuntimeSessionOutput::no_op())
            })
        };
        assert_eq!(status, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert_eq!(small_output.written, 4096);
        assert_eq!(callback_count.get(), 0);

        let mut adequate_bytes = vec![0_u8; 4096];
        let mut adequate_output = ProjectRuntimeByteBuffer::from_slice(&mut adequate_bytes);
        // SAFETY: the descriptor and backing storage remain exclusively borrowed for this call.
        let status = unsafe {
            ffi_boundary_with_capacity_floor(&mut adequate_output, 4096, || {
                callback_count.set(callback_count.get() + 1);
                Ok(ProjectRuntimeSessionOutput::no_op())
            })
        };
        assert_eq!(status, ProjectRuntimeAbiStatus::OK);
        assert_eq!(callback_count.get(), 1);
    }

    #[test]
    fn stateful_boundary_rejects_null_and_contains_panics() {
        let calls = std::cell::Cell::new(0);
        // SAFETY: null is explicitly accepted as an invalid argument; it is never dereferenced.
        let status = unsafe {
            ffi_boundary_with_capacity_floor(std::ptr::null_mut(), 64, || {
                calls.set(calls.get() + 1);
                Ok(())
            })
        };
        assert_eq!(status, ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
        assert_eq!(calls.get(), 0);
        let mut storage = [0; 64];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut storage);
        // SAFETY: descriptor and backing storage are live and exclusively borrowed during the call.
        let status = unsafe {
            ffi_boundary_with_capacity_floor::<()>(&mut output, 64, || {
                calls.set(calls.get() + 1);
                panic!("callback failed");
            })
        };
        assert_eq!(status, ProjectRuntimeAbiStatus::PANICKED);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn crate_has_no_engine_or_editor_dependency() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in [
            "engine_runtime",
            "engine_input",
            "editor_core",
            "editor_window",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "forbidden dependency: {forbidden}"
            );
        }
    }
}

#[cfg(test)]
mod lifecycle_wire_tests {
    use super::*;
    #[test]
    fn focus_and_exit_preserve_typed_wire_intent() {
        for focused in [false, true] {
            let event = ProjectRuntimeLifecycleEvent::FocusChanged { focused };
            let wire = serde_json::to_vec(&event).unwrap();
            assert_eq!(
                serde_json::from_slice::<ProjectRuntimeLifecycleEvent>(&wire).unwrap(),
                event
            );
        }
        let mut output = ProjectRuntimeSessionOutput::no_op();
        output.request_exit();
        output.request_exit();
        let decoded: ProjectRuntimeSessionOutput =
            serde_json::from_slice(&serde_json::to_vec(&output).unwrap()).unwrap();
        assert_eq!(decoded.status, ProjectRuntimeStatus::Applied);
        assert_eq!(decoded.host_control, Some(ProjectRuntimeHostControl::Exit));
    }
}
