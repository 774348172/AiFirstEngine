//! Typed project-authoring interface for native game logic.
//!
//! Projects register ordinary Rust handlers here. The compiler-owned runtime adapter translates
//! this interface to the low-level project runtime contract.

use serde::Serialize;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};

pub mod animator;
pub mod generated_content;
pub mod kinematic2d;
pub use animator::AnimatorReadExt;
pub mod rule_slots;
mod ui_state;
pub use project_runtime_sdk::aui;
pub use project_runtime_sdk::aui_animation;
pub use project_runtime_sdk::aui_draw2d;
pub use project_runtime_sdk::aui_extension;
pub use project_runtime_sdk::aui_presentation;
pub type UiContext<'a> =
    project_runtime_sdk::aui_wire::UiWriteBuffer<'a, dyn aui::UiRuntimeAccess + 'a>;
mod cells;
mod rule_context;
pub use cells::{CellQuery, CellReceipt, CellStatus, CellTicket, VegetationPromotionFields3D};
mod spatial;
pub use project_runtime_sdk::ProjectRuntimeParticleValue as ParticleParameterValue;
pub use project_runtime_sdk::{
    ProjectRuntimeStreamingSource3D as StreamingSource3D, WorldLogicalDeclaration3D,
    WorldLogicalField3D, WorldLogicalSave3D,
};
pub use rule_context::{RuleContext, RuleMutationWriter};
pub use spatial::{CharacterMode3D, SpatialAssetRef, SpawnTicket, Transform3D};
pub use ui_state::{UiBindingRefreshPlan, UiBindingStore};

pub use project_runtime_sdk::{
    ProjectRuntimeAuiAction as AuiAction, ProjectRuntimeAuiActionRequest as AuiActionRequest,
    ProjectRuntimeCollisionPair as CollisionPair,
    ProjectRuntimeDeferredMutation as DeferredMutation, ProjectRuntimeFrameRequest as FrameRequest,
    ProjectRuntimeHostControl as HostControl, ProjectRuntimeInputAction as InputAction,
    ProjectRuntimeInputPhase as InputPhase, ProjectRuntimeLifecycleEvent as LifecycleEvent,
    ProjectRuntimeLifecycleRequest as LifecycleRequest,
    ProjectRuntimeObservationOutput as ObservationOutput, ProjectRuntimeRuleOutput as RuleOutput,
    ProjectRuntimeRuleRequest as RuleRequest,
    ProjectRuntimeSessionCreateRequest as SessionCreateRequest,
    ProjectRuntimeStatus as HandlerStatus, ProjectRuntimeTime as GameTime,
    ProjectRuntimeTransform as Transform, ProjectRuntimeUiBindingSet as UiBindingSet,
    ProjectRuntimeUiStateIdentity as UiStateIdentity, ProjectRuntimeUiStateOutput as UiStateOutput,
    ProjectRuntimeUiStateResolveOutput as UiStateResolveOutput,
    ProjectRuntimeUiStateResolveRequest as UiStateResolveRequest, ProjectRuntimeValue as Value,
    ProjectRuntimeVirtualListItem as VirtualListItem,
    ProjectRuntimeVirtualListSnapshot as VirtualListSnapshot,
    PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES as DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES,
};

pub const PROJECT_GAME_SDK_CONTRACT_ID: &str = "project-game-sdk.v1";
pub const PROJECT_GAME_REGISTRATION_SYMBOL: &str = "project_game";
pub const PROJECT_GAME_REGISTRATION_EXAMPLE: &str = r#"pub fn project_game() -> ProjectGameDefinition<GameSession> {
    ProjectGameDefinition::new(GameSession::create)
        .rule_typed("rule.example", GameSession::run_rule)
    // Add fixed_update/AUI/lifecycle handlers only when the project needs them.
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectGameCapability {
    Sessions,
    Rules,
    AuiActions,
    UiEvents,
    UiExtensions,
    FixedUpdate,
    Lifecycle,
    UiState,
    Observations,
    WorldRead,
    DeferredMutations,
    GeneratedContent,
    ProjectStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGameSdkSymbol {
    pub name: &'static str,
    pub kind: &'static str,
    pub summary: &'static str,
}

pub const PROJECT_GAME_SDK_SYMBOLS: &[ProjectGameSdkSymbol] = &[
    ProjectGameSdkSymbol { name:"ProjectGameDefinition::generated_content",kind:"registration",summary:"Opt in to bounded generated geometry and static collision through existing deferred callbacks; accepted tickets do not prove native publication or display." },
    ProjectGameSdkSymbol { name:"ProjectGameDefinition::project_storage",kind:"registration",summary:"Opt in to opaque project bytes in the trusted Player project/profile namespace; projects never select filesystem paths." },
    ProjectGameSdkSymbol { name:"DeferredMutationWriter::replace_geometry3d",kind:"spatial",summary:"Submit owned GeometryBuilder3D arrays and optional static collision with a live target, increasing request id and expected applied revision." },
    ProjectGameSdkSymbol { name:"DeferredMutationWriter::update_geometry_attributes3d",kind:"spatial",summary:"Update an existing fixed layout with typed AttributePatch3D; position changes recompute bounds, index/layout/count changes require replacement." },
    ProjectGameSdkSymbol { name:"DeferredMutationWriter::replace_collision3d",kind:"spatial",summary:"Replace or clear static collision through the existing physics owner without withdrawing geometry; Keep preserves collision revision." },
    ProjectGameSdkSymbol { name:"WorldRead::query_generated_content",kind:"spatial",summary:"Read bounded ticket/target status, applied/pending/collision revisions and commit epoch; logical Applied is distinct from first simulation or displayed frame." },
    ProjectGameSdkSymbol { name:"WorldRead::take_project_bytes",kind:"storage",summary:"Claim a terminal binary read once; short transport buffers retain the result. Claim or explicitly abandon unread bytes to release budget." },
    ProjectGameSdkSymbol{name:"DeferredMutationWriter::animator",kind:"animation",summary:"Writes named Spine play/restart/resume, bool/trigger, pause, layer fade/clear and skin intents with a resolved entity generation; only FixedUpdate/AUI callbacks."},
    ProjectGameSdkSymbol{name:"AnimatorReadExt::animator",kind:"animation",summary:"Reads declared simulation sockets and source content events by live entity handle; events become visible in the next legal callback."},
    ProjectGameSdkSymbol {
        name: "aui_draw2d::PathBuilder",
        kind: "ui-authoring",
        summary: "Builds bounded local-space paths in logical pixels with radian arcs; pure authoring data, not a runtime drawing handle.",
    },
    ProjectGameSdkSymbol {
        name: "aui_draw2d::Draw2D",
        kind: "ui-authoring",
        summary: "Builds validated retained stroke/fill descriptions; publish through UiExtensionPaintOutput::from_graphics, or reference cooked graphics2d assets by ID/GUID.",
    },
    ProjectGameSdkSymbol {
        name: "aui_presentation::Transform2D",
        kind: "ui-authoring",
        summary: "Shared parent-local affine math, pivots and inverse queries; does not submit node setters.",
    },
    ProjectGameSdkSymbol {
        name: "aui_animation::Animation2D",
        kind: "ui-authoring",
        summary: "Validates typed serial/parallel tracks, loop timing and write conflicts; runtime playback is a separate host capability.",
    },
    ProjectGameSdkSymbol {
        name: "aui_animation::PresentationClock",
        kind: "ui-authoring",
        summary: "Shared frame-indexed clock math with pause, speed and bounded deltas; the presentation owner must supply frame indices.",
    },
    ProjectGameSdkSymbol {
        name: "UiBindingStore",
        kind: "ui-state",
        summary: "Tracks active binding fields, coalesces field notifications and emits cacheable Reuse/Replace snapshots without project-managed revisions.",
    },
    ProjectGameSdkSymbol {
        name: "GameCallContext::ui",
        kind: "context",
        summary: "Resolves session-local typed UI refs and stages setters in lifecycle, AUI and fixed callbacks; Glue submits automatically.",
    },
    ProjectGameSdkSymbol {
        name: "ProjectGameDefinition::ui_event",
        kind: "registration",
        summary: "Registers a stable non-zero callback ID for Changed and lifecycle notifications delivered by generated Glue.",
    },
    ProjectGameSdkSymbol {
        name: "ProjectGameDefinition::ui_extension",
        kind: "registration",
        summary: "Registers a stable schema-described custom measure/layout/paint type; callbacks receive read-only bounded requests.",
    },
    ProjectGameSdkSymbol {
        name: "ProjectGameDefinition::rule_typed",
        kind: "registration",
        summary: "Registers a rule with a stage-restricted RuleContext; legacy rule remains supported.",
    },
    ProjectGameSdkSymbol {
        name: "RuleContext",
        kind: "context",
        summary: "World reads and deferred gameplay writes; media and host control belong to session callbacks.",
    },
    ProjectGameSdkSymbol {
        name: "InputAction::input_phase",
        kind: "input",
        summary: "Typed phase of a delivered action, not a global held-state query. Use WhilePressed for continuous input.",
    },
    ProjectGameSdkSymbol {
        name: "LifecycleRequest",
        kind: "callback",
        summary: "Receives ordered focus changes independently of gameplay ticks.",
    },
    ProjectGameSdkSymbol {
        name: "GameCallContext::request_exit",
        kind: "control",
        summary: "Requests orderly host-owned session exit after successful commit.",
    },
    ProjectGameSdkSymbol {
        name: "ProjectGameDefinition",
        kind: "registration",
        summary: "Declares the session factory and typed project handlers.",
    },
    ProjectGameSdkSymbol {
        name: "ProjectGameSession",
        kind: "session",
        summary: "Supplies the stable logical identity of a project runtime session.",
    },
    ProjectGameSdkSymbol {
        name: "GameCallContext",
        kind: "context",
        summary: "Provides bounded world reads, deferred mutations, and diagnostics.",
    },
    ProjectGameSdkSymbol {
        name: "WorldRead",
        kind: "context",
        summary: "Reads project-visible entities and component values through stable handles.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter",
        kind: "context",
        summary: "Submits mutations for the host to apply after the current callback.",
    },
    ProjectGameSdkSymbol {
        name: "WorldRead::read_transform3d",
        kind: "spatial",
        summary: "Reads metre/quaternion local Transform3D through a resolved live EntityHandle.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::write_local_transform3d",
        kind: "spatial",
        summary: "Submits typed 3D transforms; the host rechecks incarnation and the whole batch before commit.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::write_local_transforms3d",
        kind: "spatial",
        summary: "Validates every local transform before staging the batch; configured tick capacity is checked by the host.",
    },
    ProjectGameSdkSymbol {
        name: "WorldRead::resolve_spatial_entities",
        kind: "spatial",
        summary: "Cold batch resolution to opaque World-scoped handles; commits still check current epoch and incarnation.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::write_spatial_transforms3d",
        kind: "spatial",
        summary: "Bounded typed local-transform batch, with complete-tail validation and the shared tick budget; no raw ECS access.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::teleport3d",
        kind: "spatial",
        summary: "Commits a new local pose and resets interpolation history for the subtree.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_spatial_interpolation",
        kind: "spatial",
        summary: "Changes subtree interpolation policy and resets history at commit.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::reparent3d",
        kind: "spatial",
        summary: "Reparents using current child and parent handles; optional keep-world rejects unrepresentable local shear.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_world_anchor3d",
        kind: "spatial",
        summary: "Changes a root f64 anchor; optional keep-world rebases local displacement atomically.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::write_local_transforms3d",
        kind: "spatial",
        summary: "Validates every local transform before staging the batch; configured tick capacity is checked by the host.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::teleport3d",
        kind: "spatial",
        summary: "Commits a new local pose and resets interpolation history for the subtree.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_spatial_interpolation",
        kind: "spatial",
        summary: "Changes subtree interpolation policy and resets history at commit.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::reparent3d",
        kind: "spatial",
        summary: "Reparents using current child and parent handles; optional keep-world rejects unrepresentable local shear.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_world_anchor3d",
        kind: "spatial",
        summary: "Changes a root f64 anchor; optional keep-world rebases local displacement atomically.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_material",
        kind: "spatial",
        summary: "Binds a stable package material AssetRef to an existing named mesh slot.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_mesh_visible",
        kind: "spatial",
        summary: "Changes mesh visibility through a live handle without removing its components.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_light_enabled",
        kind: "spatial",
        summary: "Toggles an existing authored Light3D through a live handle and atomic engine commit.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_light_color",
        kind: "spatial",
        summary: "Changes finite nonnegative linear RGB on an authored Light3D; in-flight packets retain old values.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::set_light_intensity",
        kind: "spatial",
        summary: "Changes lux for a directional light or candela for a point/spot light; authored kind is preserved.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::select_main_camera",
        kind: "spatial",
        summary: "Explicitly selects a live enabled Camera3D; spawning does not steal the main view.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::load_cell",
        kind: "cell",
        summary: "Accepts an asynchronous cell demand in a separate session callback; use strictly increasing request IDs and a stable prefab AssetRef.",
    },
    ProjectGameSdkSymbol {
        name: "WorldRead::cell_status",
        kind: "cell",
        summary: "Queries a bounded typed receipt; Active requires real resource completion and a simulation safe-point commit.",
    },
    ProjectGameSdkSymbol {
        name: "WorldRead::resolve_cell_entity",
        kind: "cell",
        summary: "Resolves a non-owning placement/source identity only while the actual cell incarnation is active.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::release_cell",
        kind: "cell",
        summary: "Cancels an invisible load or unloads an active cell at a simulation safe point; query errors before retry.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::migrate_cell_subtree",
        kind: "cell",
        summary: "Preserves entity identity and world pose while moving a subtree to a persistent placement with independent resource ownership.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::instantiate_spatial_prefab",
        kind: "spatial",
        summary: "Returns a SpawnTicket; resolve_spawn obtains the actual root handle after commit in a subsequent session callback.",
    },
    ProjectGameSdkSymbol {
        name: "DeferredMutationWriter::despawn_spatial_prefab",
        kind: "spatial",
        summary: "Destroys a prefab using its current root handle and releases assets through the instance loader.",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGameSdkMetadata {
    pub contract_id: &'static str,
    pub registration_symbol: &'static str,
    pub symbols: &'static [ProjectGameSdkSymbol],
    pub registration_example: &'static str,
    pub limitations: &'static [&'static str],
}

pub fn project_game_sdk_metadata() -> ProjectGameSdkMetadata {
    ProjectGameSdkMetadata {
        contract_id: PROJECT_GAME_SDK_CONTRACT_ID,
        registration_symbol: PROJECT_GAME_REGISTRATION_SYMBOL,
        symbols: PROJECT_GAME_SDK_SYMBOLS,
        registration_example: PROJECT_GAME_REGISTRATION_EXAMPLE,
        limitations: &[
            "no_raw_world_or_entity_index",
            "no_renderer_or_physics_internal_objects",
            "no_filesystem_network_or_process_access",
            "mutations_are_deferred",
        ],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameErrorCode {
    InvalidArgument,
    Unsupported,
    Failed,
    TerminalFault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameError {
    pub code: GameErrorCode,
    pub message: String,
}

impl GameError {
    pub fn new(code: GameErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type GameResult<T> = Result<T, GameError>;

impl From<aui::UiError> for GameError {
    fn from(error: aui::UiError) -> Self {
        Self::new(
            GameErrorCode::Failed,
            format!(
                "UI {:?}: handle={:?}, property={:?}",
                error.kind, error.handle, error.property
            ),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityHandle {
    identity: String,
    generation: u64,
}

impl EntityHandle {
    pub fn new(identity: impl Into<String>, generation: u64) -> GameResult<Self> {
        let identity = identity.into();
        if identity.trim().is_empty() {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "entity identity must not be empty",
            ));
        }
        Ok(Self {
            identity,
            generation,
        })
    }

    pub fn from_stable_identity(identity: impl Into<String>) -> GameResult<Self> {
        Self::new(identity, 0)
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

pub use project_runtime_sdk::physics_query3d;
pub use spatial::SpatialEntityHandle;
pub trait WorldRead {
    fn query_project_storage(
        &self,
        _query: generated_content::StorageQuery,
    ) -> GameResult<generated_content::StorageQueryReply> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Project storage unavailable on this host",
        ))
    }
    fn take_project_bytes(
        &self,
        _ticket: generated_content::ContentTicket,
    ) -> GameResult<generated_content::StorageReadReply> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Project byte results unavailable on this host",
        ))
    }
    /// Compact metadata through the call-scoped host; semantic errors remain structured.
    fn query_generated_content(
        &self,
        _query: generated_content::ContentQuery,
    ) -> GameResult<generated_content::ContentQueryReply> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Generated content queries unavailable on this host",
        ))
    }

    /// Read the real owner's latest committed tick without advancing simulation.
    /// FixedUpdate precedes its physics step; None tick explicitly means not ready.
    fn physics_query3d(
        &self,
        _queries: &[physics_query3d::PhysicsQuery3D],
    ) -> GameResult<physics_query3d::PhysicsQueryResult3D> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Physics queries unavailable on this host",
        ))
    }
    /// Resolve names once; keys remain non-owning and are revalidated at every commit.
    fn resolve_spatial_entities(
        &self,
        _identities: &[String],
    ) -> GameResult<Vec<SpatialEntityHandle>> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Typed spatial resolution unavailable",
        ))
    }

    fn cell_status(&self, _ticket: CellTicket) -> GameResult<Option<CellReceipt>> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Cell status unavailable on this host",
        ))
    }
    /// Non-owning authored reference: absent until activation, absent after release.
    fn resolve_cell_entity(
        &self,
        _placement_id: &str,
        _source_entity_id: &str,
    ) -> GameResult<Option<EntityHandle>> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Cell soft references unavailable on this host",
        ))
    }

    /// Resolve a source authored in a streamed cell through its current placement.
    fn resolve_streamed_cell_entity(
        &self,
        owner: &EntityHandle,
        cell: &str,
        source: &str,
    ) -> GameResult<Option<EntityHandle>> {
        let Value::Object(state) = self.read_component(owner, "engine.world_streaming_state")?
        else {
            return Ok(None);
        };
        let Some(Value::Object(placements)) = state.get("placements") else {
            return Ok(None);
        };
        let Some(Value::String(placement)) = placements.get(cell) else {
            return Ok(None);
        };
        self.resolve_cell_entity(placement, source)
    }

    /// The actual committed prefab root; pending, expired or despawned requests return None.
    fn resolve_spawn(&self, _ticket: SpawnTicket) -> GameResult<Option<EntityHandle>> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "Spawn receipts unavailable on this host",
        ))
    }

    fn query(&self, all: &[String], none: &[String]) -> GameResult<Vec<EntityHandle>>;

    fn read_component(&self, entity: &EntityHandle, component_type: &str) -> GameResult<Value>;

    fn read_transform3d(&self, entity: &EntityHandle) -> GameResult<Transform3D> {
        spatial::read_transform(self.read_component(entity, "engine.transform3d")?)
    }

    /// Resolve the current incarnation; stable authoring names alone are not live handles.
    /// ABI/production hosts override this fallback with an exact identity lookup.
    fn resolve_entity(&self, identity: &str) -> GameResult<EntityHandle> {
        self.query(&[], &[])?
            .into_iter()
            .find(|e| e.identity() == identity)
            .ok_or_else(|| GameError::new(GameErrorCode::InvalidArgument, "Entity is not alive"))
    }
}

#[derive(Debug, Default)]
pub struct DeferredMutationWriter {
    mutations: Vec<DeferredMutation>,
}

/// Particle commands commit only when the enclosing handler returns Applied.
pub struct ParticleEffectWriter<'a> {
    writer: &'a mut DeferredMutationWriter,
    entity: &'a EntityHandle,
}
#[cfg(test)]
mod particle_tests {
    use super::*;
    #[test]
    fn particle_writer_preserves_independent_controls_typed_values_and_generation() {
        let entity = EntityHandle::new("effect-source", 7).unwrap();
        let mut writer = DeferredMutationWriter::default();
        {
            let mut effect = writer.particle_effect(&entity);
            effect.play();
            effect.restart();
            effect.stop_emitting();
            effect.clear();
            effect.set_paused(true);
            effect.set_paused(false);
            effect.set_parameter("speed", ParticleParameterValue::Vec3([1.0, 2.0, 3.0]));
        }
        assert_eq!(writer.mutations.len(), 7);
        for mutation in &writer.mutations {
            let DeferredMutation::ParticleEffect {
                entity_id,
                generation,
                ..
            } = mutation
            else {
                panic!("wrong intent")
            };
            assert_eq!(entity_id, "effect-source");
            assert_eq!(*generation, 7);
        }
        use project_runtime_sdk::ProjectRuntimeParticleIntent as Intent;
        let intents = writer
            .mutations
            .iter()
            .map(|m| match m {
                DeferredMutation::ParticleEffect { intent, .. } => intent.clone(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            intents,
            vec![
                Intent::Play {},
                Intent::Restart {},
                Intent::StopEmitting {},
                Intent::Clear {},
                Intent::SetPaused { paused: true },
                Intent::SetPaused { paused: false },
                Intent::SetParameter {
                    name: "speed".into(),
                    value: ParticleParameterValue::Vec3([1.0, 2.0, 3.0])
                }
            ]
        );
    }
}
impl ParticleEffectWriter<'_> {
    fn push(&mut self, intent: project_runtime_sdk::ProjectRuntimeParticleIntent) {
        self.writer
            .mutations
            .push(DeferredMutation::ParticleEffect {
                entity_id: self.entity.identity().into(),
                generation: self.entity.generation(),
                intent,
            });
    }
    pub fn play(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeParticleIntent::Play {});
    }
    /// Restart from zero; the explicit pause flag is preserved.
    pub fn restart(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeParticleIntent::Restart {});
    }
    pub fn stop_emitting(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeParticleIntent::StopEmitting {});
    }
    pub fn clear(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeParticleIntent::Clear {});
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.push(project_runtime_sdk::ProjectRuntimeParticleIntent::SetPaused { paused });
    }
    pub fn set_parameter(&mut self, name: impl Into<String>, value: ParticleParameterValue) {
        self.push(
            project_runtime_sdk::ProjectRuntimeParticleIntent::SetParameter {
                name: name.into(),
                value,
            },
        );
    }
}

/// Deferred AudioSource controls for FixedUpdate and AUI session callbacks.
/// Return `HandlerStatus::Applied` to commit; preparing a call never plays sound.
pub struct AudioSourceWriter<'a> {
    writer: &'a mut DeferredMutationWriter,
    entity: &'a EntityHandle,
}

impl AudioSourceWriter<'_> {
    fn push(&mut self, intent: project_runtime_sdk::ProjectRuntimeAudioSourceIntent) {
        self.writer.mutations.push(DeferredMutation::AudioSource {
            entity_id: self.entity.identity().to_string(),
            intent,
        });
    }

    /// Restart this source's clip from the beginning, preserving its pause flag.
    pub fn play(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeAudioSourceIntent::Play {});
    }

    /// Release this source's current playback, preserving its pause flag.
    pub fn stop(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeAudioSourceIntent::Stop {});
    }

    /// Freeze or resume playback without implicitly creating a playback instance.
    pub fn set_paused(&mut self, paused: bool) {
        self.push(project_runtime_sdk::ProjectRuntimeAudioSourceIntent::SetPaused { paused });
    }
}

/// Deferred Animator2D intent writer for FixedUpdate and AUI session callbacks.
/// Names are validated against the loaded controller; same-name play preserves progress.
pub struct Animator2DWriter<'a> {
    writer: &'a mut DeferredMutationWriter,
    entity: &'a EntityHandle,
}
impl Animator2DWriter<'_> {
    fn push(&mut self, intent: project_runtime_sdk::ProjectRuntimeAnimator2DIntent) {
        self.writer.mutations.push(DeferredMutation::Animator2D {
            entity_id: self.entity.identity().into(),
            intent,
        });
    }
    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.push(
            project_runtime_sdk::ProjectRuntimeAnimator2DIntent::SetBool {
                name: name.into(),
                value,
            },
        );
    }
    pub fn play(&mut self, name: impl Into<String>) {
        self.push(project_runtime_sdk::ProjectRuntimeAnimator2DIntent::Play { name: name.into() });
    }
    pub fn resume(&mut self) {
        self.push(project_runtime_sdk::ProjectRuntimeAnimator2DIntent::Resume);
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.push(project_runtime_sdk::ProjectRuntimeAnimator2DIntent::SetPaused { paused });
    }
}

impl DeferredMutationWriter {
    pub fn animator<'a>(&'a mut self, entity: &'a EntityHandle) -> animator::AnimatorWriter<'a> {
        animator::AnimatorWriter {
            writer: self,
            entity,
        }
    }
    pub fn particle_effect<'a>(&'a mut self, entity: &'a EntityHandle) -> ParticleEffectWriter<'a> {
        ParticleEffectWriter {
            writer: self,
            entity,
        }
    }
    pub fn audio_source<'a>(&'a mut self, entity: &'a EntityHandle) -> AudioSourceWriter<'a> {
        AudioSourceWriter {
            writer: self,
            entity,
        }
    }
    pub fn animator2d<'a>(&'a mut self, entity: &'a EntityHandle) -> Animator2DWriter<'a> {
        Animator2DWriter {
            writer: self,
            entity,
        }
    }
    pub fn write_transform(&mut self, entity: &EntityHandle, transform: Transform) {
        self.mutations.push(DeferredMutation::WriteTransform {
            entity_id: entity.identity().to_string(),
            transform,
        });
    }

    pub fn write_component_field(
        &mut self,
        entity: &EntityHandle,
        component_type: impl Into<String>,
        field_path: impl Into<String>,
        value: Value,
    ) {
        self.mutations.push(DeferredMutation::WriteComponentField {
            entity_id: entity.identity().to_string(),
            component_type: component_type.into(),
            field_path: field_path.into(),
            value,
        });
    }

    pub fn replace_dynamic_component(
        &mut self,
        entity: &EntityHandle,
        component_type: impl Into<String>,
        fields: BTreeMap<String, Value>,
    ) {
        self.mutations
            .push(DeferredMutation::ReplaceDynamicComponent {
                entity_id: entity.identity().to_string(),
                component_type: component_type.into(),
                fields,
            });
    }

    pub fn instantiate_prefab(&mut self, prefab_id: impl Into<String>) {
        self.mutations.push(DeferredMutation::InstantiatePrefab {
            prefab_id: prefab_id.into(),
            position: None,
        });
    }

    pub fn instantiate_prefab_at(&mut self, prefab_id: impl Into<String>, position: [f32; 3]) {
        self.mutations.push(DeferredMutation::InstantiatePrefab {
            prefab_id: prefab_id.into(),
            position: Some(position),
        });
    }

    pub fn despawn(&mut self, entity: &EntityHandle) {
        self.mutations.push(DeferredMutation::DespawnEntity {
            entity_id: entity.identity().to_string(),
        });
    }

    pub fn extend(&mut self, mutations: impl IntoIterator<Item = DeferredMutation>) {
        self.mutations.extend(mutations);
    }

    pub fn as_slice(&self) -> &[DeferredMutation] {
        &self.mutations
    }

    pub fn into_inner(self) -> Vec<DeferredMutation> {
        self.mutations
    }
}

#[derive(Debug, Default)]
pub struct ProjectDiagnosticSink {
    diagnostics: Vec<String>,
}

impl ProjectDiagnosticSink {
    pub fn push(&mut self, diagnostic: impl Into<String>) {
        self.diagnostics.push(diagnostic.into());
    }

    pub fn as_slice(&self) -> &[String] {
        &self.diagnostics
    }

    pub fn into_inner(self) -> Vec<String> {
        self.diagnostics
    }
}

#[derive(Debug, Default)]
pub struct UiStateWriter {
    values: BTreeMap<String, Value>,
}

impl UiStateWriter {
    pub fn set(&mut self, path: impl Into<String>, value: Value) {
        self.values.insert(path.into(), value);
    }

    pub fn into_inner(self) -> BTreeMap<String, Value> {
        self.values
    }
}

#[derive(Debug, Default)]
pub struct ObservationWriter {
    values: BTreeMap<String, Value>,
}

impl ObservationWriter {
    pub fn publish(&mut self, path: impl Into<String>, value: Value) {
        self.values.insert(path.into(), value);
    }

    pub fn into_inner(self) -> BTreeMap<String, Value> {
        self.values
    }
}

pub struct GameCallContext<'a> {
    world: &'a dyn WorldRead,
    mutations: DeferredMutationWriter,
    diagnostics: ProjectDiagnosticSink,
    host_control: Option<HostControl>,
    ui: Option<UiContext<'a>>,
}

impl<'a> GameCallContext<'a> {
    pub fn new(world: &'a dyn WorldRead) -> Self {
        Self {
            world,
            mutations: DeferredMutationWriter::default(),
            diagnostics: ProjectDiagnosticSink::default(),
            host_control: None,
            ui: None,
        }
    }

    #[doc(hidden)]
    pub fn with_ui(world: &'a dyn WorldRead, ui: &'a dyn aui::UiRuntimeAccess) -> Self {
        let mut context = Self::new(world);
        context.ui = Some(UiContext::new(ui));
        context
    }

    pub fn ui(&mut self) -> Result<&mut UiContext<'a>, aui::UiError> {
        self.ui
            .as_mut()
            .ok_or_else(project_runtime_sdk::aui_host::unavailable)
    }

    #[doc(hidden)]
    pub fn finish_ui(
        &mut self,
    ) -> Result<Option<project_runtime_sdk::aui_wire::UiUpdateBatch>, aui::UiError> {
        self.ui.take().map(UiContext::finish).transpose()
    }

    /// Request normal exit from an Applied lifecycle, AUI or fixed-update callback.
    /// NoOp, rejected, failed and panicking callbacks never publish this request.
    pub fn request_exit(&mut self) {
        self.host_control = Some(HostControl::Exit);
    }

    #[doc(hidden)]
    pub fn take_host_control(&mut self) -> Option<HostControl> {
        self.host_control.take()
    }

    pub fn world(&self) -> &dyn WorldRead {
        self.world
    }

    pub fn mutations(&mut self) -> &mut DeferredMutationWriter {
        &mut self.mutations
    }

    pub fn diagnostics(&mut self) -> &mut ProjectDiagnosticSink {
        &mut self.diagnostics
    }

    pub fn into_outputs(self) -> (Vec<DeferredMutation>, Vec<String>) {
        (self.mutations.into_inner(), self.diagnostics.into_inner())
    }
}

/// Project-facing output retains its existing literal construction contract.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionOutput {
    pub status: HandlerStatus,
    pub handled_action_count: u64,
    pub unhandled_action_count: u64,
    pub rejected_action_count: u64,
    pub mutations: Vec<DeferredMutation>,
    pub diagnostics: Vec<String>,
}
impl SessionOutput {
    pub fn no_op() -> Self {
        Self {
            status: HandlerStatus::NoOp,
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
    #[doc(hidden)]
    pub fn into_runtime_output(
        self,
        host_control: Option<HostControl>,
    ) -> project_runtime_sdk::ProjectRuntimeSessionOutput {
        project_runtime_sdk::ProjectRuntimeSessionOutput {
            status: self.status,
            host_control,
            handled_action_count: self.handled_action_count,
            unhandled_action_count: self.unhandled_action_count,
            rejected_action_count: self.rejected_action_count,
            mutations: self.mutations,
            diagnostics: self.diagnostics,
        }
    }
}

pub trait ProjectGameSession: Sized + Send + 'static {
    fn session_id(&self) -> &str;
}

pub type SessionFactory<S> = fn(&SessionCreateRequest) -> GameResult<S>;
pub type RuleHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &RuleRequest) -> GameResult<RuleOutput>;
pub type TypedRuleHandler<S> = for<'call, 'world> fn(
    &mut S,
    &mut RuleContext<'call, 'world>,
    &RuleRequest,
) -> GameResult<RuleOutput>;
pub type AuiActionHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &AuiActionRequest) -> GameResult<SessionOutput>;
pub type LifecycleHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &LifecycleRequest) -> GameResult<SessionOutput>;
pub type FixedUpdateHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &FrameRequest) -> GameResult<SessionOutput>;
pub type UiEventHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &aui::UiEventRequest) -> GameResult<SessionOutput>;
pub type UiExtensionHandler<S> =
    fn(&mut S, &aui_extension::UiExtensionRequest) -> GameResult<aui_extension::UiExtensionOutput>;
pub type UiStateHandler<S> = for<'a> fn(
    &mut S,
    &mut GameCallContext<'a>,
    &UiStateResolveRequest,
) -> GameResult<UiStateResolveOutput>;
pub type ObservationHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &FrameRequest) -> GameResult<ObservationOutput>;

#[derive(Clone, Copy)]
pub struct RuleRegistration<S> {
    rule_id: &'static str,
    handler: RuleHandler<S>,
}

impl<S> RuleRegistration<S> {
    pub fn rule_id(&self) -> &'static str {
        self.rule_id
    }

    pub fn handler(&self) -> RuleHandler<S> {
        self.handler
    }
}

pub struct ProjectGameDefinition<S: ProjectGameSession> {
    session_factory: SessionFactory<S>,
    rules: Vec<RuleRegistration<S>>,
    typed_rules: Vec<(&'static str, TypedRuleHandler<S>)>,
    aui_actions: Option<AuiActionHandler<S>>,
    ui_events: Vec<(u64, UiEventHandler<S>)>,
    ui_extensions: Vec<(
        aui_extension::UiExtensionTypeDescriptor,
        UiExtensionHandler<S>,
    )>,
    fixed_update: Option<FixedUpdateHandler<S>>,
    lifecycle: Option<LifecycleHandler<S>>,
    ui_state: Option<UiStateHandler<S>>,
    observe: Option<ObservationHandler<S>>,
    generated_content: bool,
    project_storage: bool,
}

impl<S: ProjectGameSession> ProjectGameDefinition<S> {
    pub fn new(session_factory: SessionFactory<S>) -> Self {
        Self {
            session_factory,
            rules: Vec::new(),
            typed_rules: Vec::new(),
            aui_actions: None,
            ui_events: Vec::new(),
            ui_extensions: Vec::new(),
            fixed_update: None,
            lifecycle: None,
            ui_state: None,
            observe: None,
            generated_content: false,
            project_storage: false,
        }
    }

    pub fn rule(mut self, rule_id: &'static str, handler: RuleHandler<S>) -> Self {
        self.rules.push(RuleRegistration { rule_id, handler });
        self
    }

    /// Recommended rule registration. The context exposes only Rule-stage writers.
    pub fn rule_typed(mut self, rule_id: &'static str, handler: TypedRuleHandler<S>) -> Self {
        self.typed_rules.push((rule_id, handler));
        self
    }

    /// All rule IDs, including legacy and stage-restricted registrations.
    pub fn rule_ids(&self) -> Vec<&'static str> {
        self.rules
            .iter()
            .map(|rule| rule.rule_id)
            .chain(self.typed_rules.iter().map(|(id, _)| *id))
            .collect()
    }

    pub fn aui_actions(mut self, handler: AuiActionHandler<S>) -> Self {
        self.aui_actions = Some(handler);
        self
    }

    pub fn ui_event(mut self, callback_id: u64, handler: UiEventHandler<S>) -> Self {
        self.ui_events.push((callback_id, handler));
        self
    }

    pub fn ui_extension(
        mut self,
        descriptor: aui_extension::UiExtensionTypeDescriptor,
        handler: UiExtensionHandler<S>,
    ) -> Self {
        self.ui_extensions.push((descriptor, handler));
        self
    }

    pub fn lifecycle(mut self, handler: LifecycleHandler<S>) -> Self {
        self.lifecycle = Some(handler);
        self
    }

    pub fn fixed_update(mut self, handler: FixedUpdateHandler<S>) -> Self {
        self.fixed_update = Some(handler);
        self
    }

    pub fn ui_state(mut self, handler: UiStateHandler<S>) -> Self {
        self.ui_state = Some(handler);
        self
    }

    pub fn observe(mut self, handler: ObservationHandler<S>) -> Self {
        self.observe = Some(handler);
        self
    }

    /// Declare use of generated geometry/static collision through normal callbacks.
    pub fn generated_content(mut self) -> Self {
        self.generated_content = true;
        self
    }

    /// Request a trusted host project/profile byte namespace; no project filesystem path.
    pub fn project_storage(mut self) -> Self {
        self.project_storage = true;
        self
    }

    pub fn validate(&self) -> GameResult<()> {
        if (self.generated_content || self.project_storage)
            && self.fixed_update.is_none()
            && self.lifecycle.is_none()
            && self.aui_actions.is_none()
            && self.ui_events.is_empty()
        {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "generated content/storage require a mutation-capable session callback",
            ));
        }
        let mut ids = BTreeSet::new();
        for rule_id in self.rule_ids() {
            if rule_id.trim().is_empty() {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    "rule id must not be empty",
                ));
            }
            if !ids.insert(rule_id) {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    format!("duplicate rule id: {rule_id}"),
                ));
            }
        }
        let mut callback_ids = BTreeSet::new();
        for (callback_id, _) in &self.ui_events {
            if *callback_id == 0 || !callback_ids.insert(*callback_id) {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    "UI callback ids must be non-zero and unique",
                ));
            }
        }
        let mut extension_types = BTreeSet::new();
        let mut extension_callbacks = BTreeSet::new();
        for (descriptor, _) in &self.ui_extensions {
            aui_extension::validate_descriptor(descriptor).map_err(|_| {
                GameError::new(
                    GameErrorCode::InvalidArgument,
                    "invalid UI extension descriptor",
                )
            })?;
            if !extension_types.insert(descriptor.type_id.as_str())
                || !extension_callbacks.insert(descriptor.callback_id)
            {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    "UI extension type ids and callback ids must be unique",
                ));
            }
        }
        Ok(())
    }

    pub fn capabilities(&self) -> BTreeSet<ProjectGameCapability> {
        let mut capabilities = BTreeSet::from([ProjectGameCapability::Sessions]);
        if self.generated_content || self.project_storage {
            capabilities.extend([
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
            ]);
        }
        if self.generated_content {
            capabilities.insert(ProjectGameCapability::GeneratedContent);
        }
        if self.project_storage {
            capabilities.insert(ProjectGameCapability::ProjectStorage);
        }
        if !self.rules.is_empty() || !self.typed_rules.is_empty() {
            capabilities.extend([
                ProjectGameCapability::Rules,
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
            ]);
        }
        if self.aui_actions.is_some() {
            capabilities.extend([
                ProjectGameCapability::AuiActions,
                ProjectGameCapability::DeferredMutations,
            ]);
        }
        if !self.ui_events.is_empty() {
            capabilities.extend([
                ProjectGameCapability::UiEvents,
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
            ]);
        }
        if !self.ui_extensions.is_empty() {
            capabilities.insert(ProjectGameCapability::UiExtensions);
        }
        if self.lifecycle.is_some() {
            capabilities.extend([
                ProjectGameCapability::Lifecycle,
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
            ]);
        }
        if self.fixed_update.is_some() {
            capabilities.extend([
                ProjectGameCapability::FixedUpdate,
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
            ]);
        }
        if self.ui_state.is_some() {
            capabilities.extend([
                ProjectGameCapability::UiState,
                ProjectGameCapability::WorldRead,
            ]);
        }
        if self.observe.is_some() {
            capabilities.insert(ProjectGameCapability::Observations);
        }
        capabilities
    }

    pub fn create_session(&self, request: &SessionCreateRequest) -> GameResult<S> {
        (self.session_factory)(request)
    }

    /// Legacy registrations. Use `rule_ids()` to enumerate both registration styles.
    pub fn rules(&self) -> &[RuleRegistration<S>] {
        &self.rules
    }

    pub fn rule_handler(&self, rule_id: &str) -> Option<RuleHandler<S>> {
        self.rules
            .iter()
            .find(|rule| rule.rule_id == rule_id)
            .map(|rule| rule.handler)
    }

    pub fn aui_action_handler(&self) -> Option<AuiActionHandler<S>> {
        self.aui_actions
    }

    pub fn lifecycle_handler(&self) -> Option<LifecycleHandler<S>> {
        self.lifecycle
    }

    pub fn ui_event_handler(&self, callback_id: u64) -> Option<UiEventHandler<S>> {
        self.ui_events
            .iter()
            .find(|(id, _)| *id == callback_id)
            .map(|(_, handler)| *handler)
    }

    pub fn ui_extension_handler(
        &self,
        type_id: &str,
    ) -> Option<(
        &aui_extension::UiExtensionTypeDescriptor,
        UiExtensionHandler<S>,
    )> {
        self.ui_extensions
            .iter()
            .find(|(descriptor, _)| descriptor.type_id == type_id)
            .map(|(descriptor, handler)| (descriptor, *handler))
    }

    pub fn fixed_update_handler(&self) -> Option<FixedUpdateHandler<S>> {
        self.fixed_update
    }

    pub fn ui_state_handler(&self) -> Option<UiStateHandler<S>> {
        self.ui_state
    }

    pub fn observation_handler(&self) -> Option<ObservationHandler<S>> {
        self.observe
    }

    #[doc(hidden)]
    pub fn erase(self) -> adapter::ErasedProjectGameDefinition
    where
        S: Sync,
    {
        adapter::ErasedProjectGameDefinition::new(self)
    }
}

#[doc(hidden)]
pub mod adapter {
    use super::*;

    pub trait ErasedProjectSession: Any + Send {
        fn session_id(&self) -> &str;
        fn as_any_mut(&mut self) -> &mut dyn Any;
    }

    impl<S: ProjectGameSession> ErasedProjectSession for S {
        fn session_id(&self) -> &str {
            ProjectGameSession::session_id(self)
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    trait ErasedDefinition: Send + Sync {
        fn validate(&self) -> GameResult<()>;
        fn capabilities(&self) -> BTreeSet<ProjectGameCapability>;
        fn rule_ids(&self) -> Vec<&'static str>;
        fn create_session(
            &self,
            request: &SessionCreateRequest,
        ) -> GameResult<Box<dyn ErasedProjectSession>>;
        fn invoke_rule(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &RuleRequest,
        ) -> GameResult<RuleOutput>;
        fn handle_aui_actions(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &AuiActionRequest,
        ) -> GameResult<SessionOutput>;
        fn lifecycle(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &LifecycleRequest,
        ) -> GameResult<SessionOutput>;
        fn ui_event(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &aui::UiEventRequest,
        ) -> GameResult<SessionOutput>;
        fn ui_extension(
            &self,
            session: &mut dyn ErasedProjectSession,
            request: &aui_extension::UiExtensionRequestBatch,
        ) -> GameResult<aui_extension::UiExtensionResponseBatch>;

        fn fixed_update(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &FrameRequest,
        ) -> GameResult<SessionOutput>;
        fn resolve_ui_state(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &UiStateResolveRequest,
        ) -> GameResult<UiStateResolveOutput>;
        fn observe(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &FrameRequest,
        ) -> GameResult<ObservationOutput>;
    }

    struct TypedDefinition<S: ProjectGameSession>(ProjectGameDefinition<S>);

    impl<S: ProjectGameSession + Sync> TypedDefinition<S> {
        fn session(session: &mut dyn ErasedProjectSession) -> GameResult<&mut S> {
            session.as_any_mut().downcast_mut::<S>().ok_or_else(|| {
                GameError::new(
                    GameErrorCode::TerminalFault,
                    "generated adapter session type does not match project registration",
                )
            })
        }
    }

    impl<S: ProjectGameSession + Sync> ErasedDefinition for TypedDefinition<S> {
        fn validate(&self) -> GameResult<()> {
            self.0.validate()
        }

        fn capabilities(&self) -> BTreeSet<ProjectGameCapability> {
            self.0.capabilities()
        }

        fn rule_ids(&self) -> Vec<&'static str> {
            self.0.rule_ids()
        }

        fn create_session(
            &self,
            request: &SessionCreateRequest,
        ) -> GameResult<Box<dyn ErasedProjectSession>> {
            self.0
                .create_session(request)
                .map(|session| Box::new(session) as Box<dyn ErasedProjectSession>)
        }

        fn invoke_rule(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &RuleRequest,
        ) -> GameResult<RuleOutput> {
            if let Some((_, handler)) = self
                .0
                .typed_rules
                .iter()
                .find(|(id, _)| *id == request.rule_id)
            {
                return handler(
                    Self::session(session)?,
                    &mut RuleContext::new(context),
                    request,
                );
            }
            let handler = self.0.rule_handler(&request.rule_id).ok_or_else(|| {
                GameError::new(GameErrorCode::Unsupported, "project rule is not registered")
            })?;
            handler(Self::session(session)?, context, request)
        }

        fn handle_aui_actions(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &AuiActionRequest,
        ) -> GameResult<SessionOutput> {
            let handler = self.0.aui_action_handler().ok_or_else(|| {
                GameError::new(GameErrorCode::Unsupported, "AUI actions are not registered")
            })?;
            handler(Self::session(session)?, context, request)
        }

        fn lifecycle(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &LifecycleRequest,
        ) -> GameResult<SessionOutput> {
            let handler = self.0.lifecycle_handler().ok_or_else(|| {
                GameError::new(GameErrorCode::Unsupported, "lifecycle is not registered")
            })?;
            handler(Self::session(session)?, context, request)
        }

        fn ui_event(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &aui::UiEventRequest,
        ) -> GameResult<SessionOutput> {
            let handler = self
                .0
                .ui_event_handler(request.callback_id.0)
                .ok_or_else(|| {
                    GameError::new(
                        GameErrorCode::Unsupported,
                        "UI callback id is not registered",
                    )
                })?;
            handler(Self::session(session)?, context, request)
        }

        fn ui_extension(
            &self,
            session: &mut dyn ErasedProjectSession,
            request: &aui_extension::UiExtensionRequestBatch,
        ) -> GameResult<aui_extension::UiExtensionResponseBatch> {
            aui_extension::validate_request_batch(request).map_err(|_| {
                GameError::new(
                    GameErrorCode::InvalidArgument,
                    "invalid UI extension request batch",
                )
            })?;
            let session = Self::session(session)?;
            let mut responses = Vec::with_capacity(request.requests.len());
            for item in &request.requests {
                let (descriptor, handler) =
                    self.0.ui_extension_handler(&item.type_id).ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::Unsupported,
                            "UI extension type is not registered",
                        )
                    })?;
                if descriptor.schema_version != item.schema_version {
                    return Err(GameError::new(
                        GameErrorCode::Unsupported,
                        "UI extension schema version is incompatible",
                    ));
                }
                if !matches!(item.input, aui_extension::UiExtensionInput::Describe)
                    && descriptor.callback_id != item.callback_id
                {
                    return Err(GameError::new(
                        GameErrorCode::Unsupported,
                        "UI extension callback id is incompatible",
                    ));
                }
                aui_extension::validate_properties(descriptor, &item.properties).map_err(|_| {
                    GameError::new(
                        GameErrorCode::InvalidArgument,
                        "UI extension properties do not match the registered descriptor",
                    )
                })?;
                let output = match item.input {
                    aui_extension::UiExtensionInput::Describe => {
                        aui_extension::UiExtensionOutput::Describe {
                            descriptor: descriptor.clone(),
                        }
                    }
                    _ => handler(session, item)?,
                };
                responses.push(aui_extension::UiExtensionResponse {
                    request_id: item.request_id,
                    output,
                });
            }
            let response = aui_extension::UiExtensionResponseBatch {
                version: aui_extension::UI_EXTENSION_VERSION,
                responses,
            };
            aui_extension::validate_response_batch(&response).map_err(|_| {
                GameError::new(
                    GameErrorCode::InvalidArgument,
                    "invalid UI extension response batch",
                )
            })?;
            Ok(response)
        }

        fn fixed_update(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &FrameRequest,
        ) -> GameResult<SessionOutput> {
            let handler = self.0.fixed_update_handler().ok_or_else(|| {
                GameError::new(GameErrorCode::Unsupported, "fixed update is not registered")
            })?;
            handler(Self::session(session)?, context, request)
        }

        fn resolve_ui_state(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &UiStateResolveRequest,
        ) -> GameResult<UiStateResolveOutput> {
            let handler = self.0.ui_state_handler().ok_or_else(|| {
                GameError::new(GameErrorCode::Unsupported, "UI state is not registered")
            })?;
            handler(Self::session(session)?, context, request)
        }

        fn observe(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &FrameRequest,
        ) -> GameResult<ObservationOutput> {
            let handler = self.0.observation_handler().ok_or_else(|| {
                GameError::new(
                    GameErrorCode::Unsupported,
                    "observations are not registered",
                )
            })?;
            handler(Self::session(session)?, context, request)
        }
    }

    pub struct ErasedProjectGameDefinition {
        definition: Box<dyn ErasedDefinition>,
    }

    impl ErasedProjectGameDefinition {
        pub(crate) fn new<S: ProjectGameSession + Sync>(
            definition: ProjectGameDefinition<S>,
        ) -> Self {
            Self {
                definition: Box::new(TypedDefinition(definition)),
            }
        }

        pub fn validate(&self) -> GameResult<()> {
            self.definition.validate()
        }

        pub fn capabilities(&self) -> BTreeSet<ProjectGameCapability> {
            self.definition.capabilities()
        }

        pub fn rule_ids(&self) -> Vec<&'static str> {
            self.definition.rule_ids()
        }

        pub fn create_session(
            &self,
            request: &SessionCreateRequest,
        ) -> GameResult<Box<dyn ErasedProjectSession>> {
            self.definition.create_session(request)
        }

        pub fn invoke_rule(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &RuleRequest,
        ) -> GameResult<RuleOutput> {
            self.definition.invoke_rule(session, context, request)
        }

        pub fn handle_aui_actions(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &AuiActionRequest,
        ) -> GameResult<SessionOutput> {
            self.definition
                .handle_aui_actions(session, context, request)
        }

        pub fn lifecycle(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &LifecycleRequest,
        ) -> GameResult<SessionOutput> {
            self.definition.lifecycle(session, context, request)
        }

        pub fn ui_event(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &aui::UiEventRequest,
        ) -> GameResult<SessionOutput> {
            self.definition.ui_event(session, context, request)
        }

        pub fn ui_extension(
            &self,
            session: &mut dyn ErasedProjectSession,
            request: &aui_extension::UiExtensionRequestBatch,
        ) -> GameResult<aui_extension::UiExtensionResponseBatch> {
            self.definition.ui_extension(session, request)
        }

        pub fn fixed_update(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &FrameRequest,
        ) -> GameResult<SessionOutput> {
            self.definition.fixed_update(session, context, request)
        }

        pub fn resolve_ui_state(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &UiStateResolveRequest,
        ) -> GameResult<UiStateResolveOutput> {
            self.definition.resolve_ui_state(session, context, request)
        }

        pub fn observe(
            &self,
            session: &mut dyn ErasedProjectSession,
            context: &mut GameCallContext<'_>,
            request: &FrameRequest,
        ) -> GameResult<ObservationOutput> {
            self.definition.observe(session, context, request)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn audio_source_writer_preserves_target_and_command_order() {
        use project_runtime_sdk::ProjectRuntimeAudioSourceIntent as Intent;
        let mut writer = DeferredMutationWriter::default();
        let entity = EntityHandle::from_stable_identity("speaker").unwrap();
        let mut audio = writer.audio_source(&entity);
        audio.play();
        audio.set_paused(true);
        audio.set_paused(false);
        audio.stop();
        let expected = [
            Intent::Play {},
            Intent::SetPaused { paused: true },
            Intent::SetPaused { paused: false },
            Intent::Stop {},
        ];
        assert_eq!(writer.as_slice().len(), expected.len());
        for (mutation, intent) in writer.as_slice().iter().zip(expected) {
            assert_eq!(
                mutation,
                &DeferredMutation::AudioSource {
                    entity_id: "speaker".into(),
                    intent
                }
            );
        }
    }
    #[test]
    fn animator2d_writer_preserves_target_and_typed_intents() {
        use super::*;
        let mut writer = DeferredMutationWriter::default();
        let entity = EntityHandle::from_stable_identity("robot").unwrap();
        let mut animator = writer.animator2d(&entity);
        animator.set_bool("moving", true);
        animator.play("walk");
        animator.set_paused(true);
        animator.resume();
        assert_eq!(writer.as_slice().len(), 4);
        assert!(
            matches!(&writer.as_slice()[1], DeferredMutation::Animator2D {entity_id,intent:project_runtime_sdk::ProjectRuntimeAnimator2DIntent::Play {name}} if entity_id == "robot" && name == "walk")
        );
    }
    use super::*;

    struct TestSession;

    impl ProjectGameSession for TestSession {
        fn session_id(&self) -> &str {
            "test.session"
        }
    }

    fn create_session(_: &SessionCreateRequest) -> GameResult<TestSession> {
        Ok(TestSession)
    }

    fn rule(
        _: &mut TestSession,
        _: &mut GameCallContext<'_>,
        _: &RuleRequest,
    ) -> GameResult<RuleOutput> {
        Ok(RuleOutput {
            status: HandlerStatus::NoOp,
            mutations: Vec::new(),
            diagnostics: Vec::new(),
        })
    }

    fn fixed_update(
        _: &mut TestSession,
        _: &mut GameCallContext<'_>,
        _: &FrameRequest,
    ) -> GameResult<SessionOutput> {
        Ok(SessionOutput::no_op())
    }

    fn ui_event(
        _: &mut TestSession,
        _: &mut GameCallContext<'_>,
        _: &aui::UiEventRequest,
    ) -> GameResult<SessionOutput> {
        Ok(SessionOutput::no_op())
    }

    fn ui_extension(
        _: &mut TestSession,
        request: &aui_extension::UiExtensionRequest,
    ) -> GameResult<aui_extension::UiExtensionOutput> {
        match request.input {
            aui_extension::UiExtensionInput::Paint { .. } => {
                Ok(aui_extension::UiExtensionOutput::Paint {
                    output: aui_extension::UiExtensionPaintOutput {
                        vertices: Vec::new(),
                        indices: Vec::new(),
                        asset_id: None,
                        primitives: Vec::new(),
                    },
                })
            }
            _ => Err(GameError::new(
                GameErrorCode::Unsupported,
                "unexpected stage",
            )),
        }
    }

    fn extension_descriptor(
        type_id: &str,
        callback_id: u64,
    ) -> aui_extension::UiExtensionTypeDescriptor {
        aui_extension::UiExtensionTypeDescriptor {
            type_id: type_id.into(),
            schema_version: 1,
            callback_id,
            properties: vec![aui_extension::UiExtensionPropertyDescriptor {
                name: "progress".into(),
                property_type: aui_extension::UiExtensionPropertyType::Float,
                invalidates: vec![aui_extension::UiExtensionInvalidation::Paint],
            }],
            measure: false,
            layout: false,
            paint: true,
        }
    }

    #[test]
    fn absent_optional_handlers_only_declare_sessions() {
        let definition = ProjectGameDefinition::new(create_session);
        assert_eq!(
            definition.capabilities(),
            BTreeSet::from([ProjectGameCapability::Sessions])
        );
        assert!(definition.rule_handler("missing").is_none());
        assert!(definition.fixed_update_handler().is_none());
    }

    #[test]
    fn handler_presence_is_the_capability_truth() {
        let definition = ProjectGameDefinition::new(create_session)
            .rule("rule.test", rule)
            .fixed_update(fixed_update);
        assert_eq!(
            definition.capabilities(),
            BTreeSet::from([
                ProjectGameCapability::Sessions,
                ProjectGameCapability::Rules,
                ProjectGameCapability::FixedUpdate,
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
            ])
        );
    }

    #[test]
    fn generated_content_376_capabilities_are_opt_in_and_require_mutation_callbacks() {
        for definition in [
            ProjectGameDefinition::new(create_session).generated_content(),
            ProjectGameDefinition::new(create_session).project_storage(),
            ProjectGameDefinition::new(create_session)
                .rule("rule.test", rule)
                .generated_content(),
            ProjectGameDefinition::new(create_session)
                .rule("rule.test", rule)
                .project_storage(),
        ] {
            assert_eq!(
                definition.validate().unwrap_err().code,
                GameErrorCode::InvalidArgument
            );
        }
        let definition = ProjectGameDefinition::new(create_session)
            .fixed_update(fixed_update)
            .generated_content()
            .project_storage();
        definition.validate().unwrap();
        assert_eq!(
            definition.capabilities(),
            BTreeSet::from([
                ProjectGameCapability::Sessions,
                ProjectGameCapability::FixedUpdate,
                ProjectGameCapability::WorldRead,
                ProjectGameCapability::DeferredMutations,
                ProjectGameCapability::GeneratedContent,
                ProjectGameCapability::ProjectStorage,
            ])
        );
        let capabilities = definition.capabilities();
        assert_eq!(definition.erase().capabilities(), capabilities);
    }

    #[test]
    fn duplicate_rule_ids_fail_validation() {
        let definition = ProjectGameDefinition::new(create_session)
            .rule("rule.test", rule)
            .rule("rule.test", rule);
        let error = definition.validate().expect_err("duplicate must fail");
        assert_eq!(error.code, GameErrorCode::InvalidArgument);
        assert!(error.message.contains("duplicate rule id"));
    }

    #[test]
    fn ui_extension_registration_rejects_duplicate_type_and_callback_ids() {
        let valid = ProjectGameDefinition::new(create_session)
            .ui_extension(extension_descriptor("sample.circle", 11), ui_extension);
        assert!(valid.validate().is_ok());
        assert!(valid
            .capabilities()
            .contains(&ProjectGameCapability::UiExtensions));

        let duplicate_type = ProjectGameDefinition::new(create_session)
            .ui_extension(extension_descriptor("sample.circle", 11), ui_extension)
            .ui_extension(extension_descriptor("sample.circle", 12), ui_extension);
        assert!(duplicate_type.validate().is_err());
        let duplicate_callback = ProjectGameDefinition::new(create_session)
            .ui_extension(extension_descriptor("sample.circle", 11), ui_extension)
            .ui_extension(extension_descriptor("sample.radial", 11), ui_extension);
        assert!(duplicate_callback.validate().is_err());
    }

    #[test]
    fn ui_callback_ids_are_nonzero_unique_capability_truth() {
        let definition = ProjectGameDefinition::new(create_session).ui_event(7, ui_event);
        definition.validate().unwrap();
        assert!(definition
            .capabilities()
            .contains(&ProjectGameCapability::UiEvents));
        assert!(definition.ui_event_handler(7).is_some());
        for definition in [
            ProjectGameDefinition::new(create_session).ui_event(0, ui_event),
            ProjectGameDefinition::new(create_session)
                .ui_event(7, ui_event)
                .ui_event(7, ui_event),
        ] {
            assert_eq!(
                definition.validate().unwrap_err().code,
                GameErrorCode::InvalidArgument
            );
        }
    }

    #[test]
    fn typed_and_legacy_rules_share_validation_and_capability_truth() {
        fn typed(
            _: &mut TestSession,
            _: &mut RuleContext<'_, '_>,
            _: &RuleRequest,
        ) -> GameResult<RuleOutput> {
            Ok(RuleOutput {
                status: HandlerStatus::NoOp,
                mutations: vec![],
                diagnostics: vec![],
            })
        }
        let definition = ProjectGameDefinition::new(create_session).rule_typed("typed", typed);
        definition.validate().unwrap();
        assert!(definition
            .capabilities()
            .contains(&ProjectGameCapability::Rules));
        assert_eq!(definition.erase().rule_ids(), vec!["typed"]);
        for definition in [
            ProjectGameDefinition::new(create_session)
                .rule("same", rule)
                .rule_typed("same", typed),
            ProjectGameDefinition::new(create_session)
                .rule_typed("same", typed)
                .rule_typed("same", typed),
            ProjectGameDefinition::new(create_session).rule_typed(" ", typed),
        ] {
            assert_eq!(
                definition.validate().unwrap_err().code,
                GameErrorCode::InvalidArgument
            );
        }
    }

    #[test]
    fn entity_handles_reject_empty_identity_and_hide_fields() {
        assert!(EntityHandle::from_stable_identity("  ").is_err());
        let handle = EntityHandle::new("entity.player", 7).expect("valid handle");
        assert_eq!(handle.identity(), "entity.player");
        assert_eq!(handle.generation(), 7);
    }

    #[test]
    fn metadata_comes_from_the_public_sdk_interface() {
        let metadata = project_game_sdk_metadata();
        assert_eq!(metadata.contract_id, PROJECT_GAME_SDK_CONTRACT_ID);
        assert_eq!(metadata.registration_symbol, "project_game");
        assert!(metadata
            .symbols
            .iter()
            .any(|symbol| symbol.name == "WorldRead"));
        assert!(metadata.limitations.contains(&"mutations_are_deferred"));
    }

    #[test]
    fn manifest_has_no_engine_editor_or_raw_abi_dependency() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in ["engine_", "editor_", "project_runtime_abi"] {
            assert!(
                !manifest.contains(forbidden),
                "SDK manifest must not contain {forbidden}"
            );
        }
    }
}

#[cfg(test)]
mod lifecycle_compatibility_tests {
    use super::*;
    #[test]
    fn legacy_session_literal_needs_no_new_fields() {
        let output = SessionOutput {
            status: HandlerStatus::Applied,
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations: vec![],
            diagnostics: vec![],
        };
        let runtime = output.into_runtime_output(Some(HostControl::Exit));
        assert_eq!(runtime.host_control, Some(HostControl::Exit));
        assert_eq!(runtime.status, HandlerStatus::Applied);
    }
}
