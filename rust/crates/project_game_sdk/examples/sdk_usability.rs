//! Run with `cargo run -p project_game_sdk --example sdk_usability`.
//! This exercises project callbacks and deferred output, not a device/audio backend.
use project_game_sdk::*;

#[derive(Default)]
struct Game {
    position: f32,
    dashes: u32,
}
impl ProjectGameSession for Game {
    fn session_id(&self) -> &str {
        "sdk.example"
    }
}
fn create(_: &SessionCreateRequest) -> GameResult<Game> {
    Ok(Game::default())
}

fn gameplay(
    game: &mut Game,
    ctx: &mut RuleContext<'_, '_>,
    request: &RuleRequest,
) -> GameResult<RuleOutput> {
    // Input asset: move.trigger = WhilePressed, dash.trigger = JustPressed.
    for action in &request.input_actions {
        if action.action_id == "move" && action.is_pressed() {
            game.position += 1.0;
        }
        if action.action_id == "dash" && action.is_just_pressed() {
            game.dashes += 1;
        }
    }
    let player = EntityHandle::from_stable_identity("player")?;
    ctx.mutations().write_transform(
        &player,
        Transform {
            position: [game.position, game.dashes as f32, 0.0],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        },
    );
    Ok(RuleOutput {
        status: HandlerStatus::Applied,
        mutations: vec![],
        diagnostics: vec![],
    })
}

fn media(
    _: &mut Game,
    ctx: &mut GameCallContext<'_>,
    _: &FrameRequest,
) -> GameResult<SessionOutput> {
    let source = EntityHandle::from_stable_identity("speaker")?;
    ctx.mutations().audio_source(&source).play();
    let player = EntityHandle::from_stable_identity("player")?;
    ctx.mutations().animator2d(&player).play("idle");
    // This demo calls media once. Real games should play sounds on state transitions.
    let mut output = SessionOutput::no_op();
    output.status = HandlerStatus::Applied;
    Ok(output)
}

fn project_game() -> ProjectGameDefinition<Game> {
    ProjectGameDefinition::new(create)
        .rule_typed("gameplay", gameplay)
        .fixed_update(media)
}

struct EmptyWorld;
impl WorldRead for EmptyWorld {
    fn query(&self, _: &[String], _: &[String]) -> GameResult<Vec<EntityHandle>> {
        Ok(vec![])
    }
    fn read_component(&self, _: &EntityHandle, _: &str) -> GameResult<Value> {
        Err(GameError::new(
            GameErrorCode::Unsupported,
            "example does not read components",
        ))
    }
}

fn main() -> GameResult<()> {
    let minimal = ProjectGameDefinition::new(create);
    minimal.validate()?;
    assert!(!minimal
        .capabilities()
        .contains(&ProjectGameCapability::FixedUpdate));

    let definition = project_game().erase();
    definition.validate()?;
    assert_eq!(definition.rule_ids(), vec!["gameplay"]);
    let mut session = definition.create_session(&SessionCreateRequest {
        project_id: "example".into(),
        module_id: "example".into(),
    })?;
    let frame = FrameRequest {
        frame_index: 1,
        time: GameTime::default(),
    };
    // Same press -> held -> release stream, interpreted by different project intents.
    for (phase, expected_x, expected_dashes) in [
        ("pressed", 1.0, 1.0),
        ("held", 2.0, 1.0),
        ("released", 2.0, 1.0),
    ] {
        let request = RuleRequest {
            rule_id: "gameplay".into(),
            frame: frame.clone(),
            collision_pairs: vec![],
            input_actions: ["move", "dash"]
                .into_iter()
                .map(|id| InputAction {
                    action_id: id.into(),
                    phase: Some(phase.into()),
                    axis1: None,
                    axis2: None,
                })
                .collect(),
        };
        let mut ctx = GameCallContext::new(&EmptyWorld);
        let output = definition.invoke_rule(session.as_mut(), &mut ctx, &request)?;
        assert_eq!(output.status, HandlerStatus::Applied);
        let (mutations, _) = ctx.into_outputs();
        assert!(
            matches!(&mutations[..], [DeferredMutation::WriteTransform { transform, .. }]
            if transform.position == [expected_x, expected_dashes, 0.0])
        );
    }
    let mut ctx = GameCallContext::new(&EmptyWorld);
    let output = definition.fixed_update(session.as_mut(), &mut ctx, &frame)?;
    assert_eq!(output.status, HandlerStatus::Applied);
    let (mutations, _) = ctx.into_outputs();
    assert!(matches!(
        &mutations[..],
        [
            DeferredMutation::AudioSource { .. },
            DeferredMutation::Animator2D { .. }
        ]
    ));
    println!("SDK example passed: optional registration, typed rule dispatch, held/edge input, deferred media.");
    Ok(())
}
