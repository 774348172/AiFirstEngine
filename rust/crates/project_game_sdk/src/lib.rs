//! Typed project-authoring interface for native game logic.
//!
//! Projects register ordinary Rust handlers here. The compiler-owned runtime adapter translates
//! this interface to the low-level project runtime contract.

use serde::Serialize;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};

pub mod kinematic2d;
pub use project_runtime_sdk::ProjectRuntimeParticleValue as ParticleParameterValue;

pub use project_runtime_sdk::{
    ProjectRuntimeAuiAction as AuiAction, ProjectRuntimeAuiActionRequest as AuiActionRequest,
    ProjectRuntimeCollisionPair as CollisionPair,
    ProjectRuntimeDeferredMutation as DeferredMutation, ProjectRuntimeFrameRequest as FrameRequest,
    ProjectRuntimeInputAction as InputAction, ProjectRuntimeObservationOutput as ObservationOutput,
    ProjectRuntimeRuleOutput as RuleOutput, ProjectRuntimeRuleRequest as RuleRequest,
    ProjectRuntimeSessionCreateRequest as SessionCreateRequest,
    ProjectRuntimeSessionOutput as SessionOutput, ProjectRuntimeStatus as HandlerStatus,
    ProjectRuntimeTime as GameTime, ProjectRuntimeTransform as Transform,
    ProjectRuntimeUiBindingSet as UiBindingSet, ProjectRuntimeUiStateIdentity as UiStateIdentity,
    ProjectRuntimeUiStateOutput as UiStateOutput,
    ProjectRuntimeUiStateResolveOutput as UiStateResolveOutput,
    ProjectRuntimeUiStateResolveRequest as UiStateResolveRequest, ProjectRuntimeValue as Value,
    PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES as DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES,
};

pub const PROJECT_GAME_SDK_CONTRACT_ID: &str = "project-game-sdk.v1";
pub const PROJECT_GAME_REGISTRATION_SYMBOL: &str = "project_game";
pub const PROJECT_GAME_REGISTRATION_EXAMPLE: &str = r#"pub fn project_game() -> ProjectGameDefinition<GameSession> {
    ProjectGameDefinition::new(GameSession::create)
        .fixed_update(GameSession::fixed_update)
        .rule("rule.example", GameSession::run_rule)
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectGameCapability {
    Sessions,
    Rules,
    AuiActions,
    FixedUpdate,
    UiState,
    Observations,
    WorldRead,
    DeferredMutations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGameSdkSymbol {
    pub name: &'static str,
    pub kind: &'static str,
    pub summary: &'static str,
}

pub const PROJECT_GAME_SDK_SYMBOLS: &[ProjectGameSdkSymbol] = &[
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

pub trait WorldRead {
    fn query(&self, all: &[String], none: &[String]) -> GameResult<Vec<EntityHandle>>;

    fn read_component(&self, entity: &EntityHandle, component_type: &str) -> GameResult<Value>;
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
}

impl<'a> GameCallContext<'a> {
    pub fn new(world: &'a dyn WorldRead) -> Self {
        Self {
            world,
            mutations: DeferredMutationWriter::default(),
            diagnostics: ProjectDiagnosticSink::default(),
        }
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

pub trait ProjectGameSession: Sized + Send + 'static {
    fn session_id(&self) -> &str;
}

pub type SessionFactory<S> = fn(&SessionCreateRequest) -> GameResult<S>;
pub type RuleHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &RuleRequest) -> GameResult<RuleOutput>;
pub type AuiActionHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &AuiActionRequest) -> GameResult<SessionOutput>;
pub type FixedUpdateHandler<S> =
    for<'a> fn(&mut S, &mut GameCallContext<'a>, &FrameRequest) -> GameResult<SessionOutput>;
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
    aui_actions: Option<AuiActionHandler<S>>,
    fixed_update: Option<FixedUpdateHandler<S>>,
    ui_state: Option<UiStateHandler<S>>,
    observe: Option<ObservationHandler<S>>,
}

impl<S: ProjectGameSession> ProjectGameDefinition<S> {
    pub fn new(session_factory: SessionFactory<S>) -> Self {
        Self {
            session_factory,
            rules: Vec::new(),
            aui_actions: None,
            fixed_update: None,
            ui_state: None,
            observe: None,
        }
    }

    pub fn rule(mut self, rule_id: &'static str, handler: RuleHandler<S>) -> Self {
        self.rules.push(RuleRegistration { rule_id, handler });
        self
    }

    pub fn aui_actions(mut self, handler: AuiActionHandler<S>) -> Self {
        self.aui_actions = Some(handler);
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

    pub fn validate(&self) -> GameResult<()> {
        let mut ids = BTreeSet::new();
        for rule in &self.rules {
            if rule.rule_id.trim().is_empty() {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    "rule id must not be empty",
                ));
            }
            if !ids.insert(rule.rule_id) {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    format!("duplicate rule id: {}", rule.rule_id),
                ));
            }
        }
        Ok(())
    }

    pub fn capabilities(&self) -> BTreeSet<ProjectGameCapability> {
        let mut capabilities = BTreeSet::from([ProjectGameCapability::Sessions]);
        if !self.rules.is_empty() {
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
        fn session<'a>(session: &'a mut dyn ErasedProjectSession) -> GameResult<&'a mut S> {
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
            self.0
                .rules()
                .iter()
                .map(RuleRegistration::rule_id)
                .collect()
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
    fn duplicate_rule_ids_fail_validation() {
        let definition = ProjectGameDefinition::new(create_session)
            .rule("rule.test", rule)
            .rule("rule.test", rule);
        let error = definition.validate().expect_err("duplicate must fail");
        assert_eq!(error.code, GameErrorCode::InvalidArgument);
        assert!(error.message.contains("duplicate rule id"));
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
