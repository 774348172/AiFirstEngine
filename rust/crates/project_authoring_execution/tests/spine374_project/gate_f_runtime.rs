//! Frozen recipe behavior is ordinary project code using the public SDK.
use project_game_sdk::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Session {
    config: Option<[usize; 3]>,
    events: i64,
    switches: i64,
    ticks: u64,
}
impl ProjectGameSession for Session {
    fn session_id(&self) -> &str {
        "spine374-f-recipes"
    }
}
fn create(_: &SessionCreateRequest) -> GameResult<Session> {
    Ok(Session::default())
}
fn fixed(
    s: &mut Session,
    c: &mut GameCallContext<'_>,
    r: &FrameRequest,
) -> GameResult<SessionOutput> {
    if s.config.is_none() {
        let entity = c.world().resolve_entity("recipe")?;
        let Value::Object(config) = c
            .world()
            .read_component(&entity, "project.spine374Recipe")?
        else {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "recipe config missing",
            ));
        };
        let mut values = [0usize; 3];
        for (i, name) in ["actors", "layers", "maxChain"].into_iter().enumerate() {
            let Some(Value::Integer(value)) = config.get(name) else {
                return Err(GameError::new(
                    GameErrorCode::InvalidArgument,
                    "recipe config type",
                ));
            };
            values[i] = usize::try_from(*value).map_err(|_| {
                GameError::new(GameErrorCode::InvalidArgument, "recipe config range")
            })?;
        }
        s.config = Some(values);
    }
    let [actors, layers, max_chain] = s.config.unwrap();
    let tick = r.time.fixed_frame_count.saturating_sub(1);
    let at = tick % 120;
    let clip = [0, 30, 36, 42]
        .into_iter()
        .take(max_chain)
        .position(|t| t == at);
    for i in 0..actors {
        let entity = c.world().resolve_entity(&format!("actor-{i}"))?;
        s.events += c.world().animator(&entity).snapshot()?.events.len() as i64;
        if let Some(clip) = clip {
            for layer in 0..layers {
                c.mutations()
                    .animator(&entity)
                    .play(format!("layer{layer}-clip{clip}"));
            }
        }
    }
    s.switches += i64::from(clip.is_some());
    s.ticks = r.time.fixed_frame_count;
    let mut out = SessionOutput::no_op();
    out.status = HandlerStatus::Applied;
    Ok(out)
}
fn observe(
    s: &mut Session,
    _: &mut GameCallContext<'_>,
    _: &FrameRequest,
) -> GameResult<ObservationOutput> {
    let [actors, layers, max_chain] = s.config.unwrap_or_default();
    Ok(ObservationOutput {
        values: BTreeMap::from([
            ("recipe.actors".into(), Value::Integer(actors as i64)),
            ("recipe.layers".into(), Value::Integer(layers as i64)),
            ("recipe.maxChain".into(), Value::Integer(max_chain as i64)),
            ("recipe.events".into(), Value::Integer(s.events)),
            ("recipe.switches".into(), Value::Integer(s.switches)),
            ("recipe.ticks".into(), Value::Integer(s.ticks as i64)),
        ]),
    })
}
pub fn project_game() -> ProjectGameDefinition<Session> {
    ProjectGameDefinition::new(create)
        .fixed_update(fixed)
        .observe(observe)
}
