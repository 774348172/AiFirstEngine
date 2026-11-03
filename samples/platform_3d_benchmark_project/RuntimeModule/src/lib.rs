//! Static diagnostic: no gameplay, no runtime asset generation.
use project_game_sdk::{
    GameResult, ProjectGameDefinition, ProjectGameSession, SessionCreateRequest,
};

pub struct Session;
impl ProjectGameSession for Session {
    fn session_id(&self) -> &str {
        "371-static-diagnostic"
    }
}
fn create(_: &SessionCreateRequest) -> GameResult<Session> {
    Ok(Session)
}
pub fn project_game() -> ProjectGameDefinition<Session> {
    ProjectGameDefinition::new(create)
}
