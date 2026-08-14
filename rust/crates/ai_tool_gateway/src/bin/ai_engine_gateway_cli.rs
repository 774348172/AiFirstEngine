use ai_tool_gateway::{
    resolve_gateway_discovery_path, ClientKind, GatewayRemoteAdapter, GatewayRequestPayload,
};
use std::io::Read;
use std::path::PathBuf;

fn main() {
    let explicit_discovery_path = std::env::var_os("AI_ENGINE_GATEWAY_DISCOVERY")
        .map(PathBuf::from)
        .or_else(|| std::env::args_os().nth(1).map(PathBuf::from));
    let expected_editor_instance_id = std::env::var("AI_ENGINE_GATEWAY_EDITOR_INSTANCE_ID").ok();
    let discovery_path = resolve_gateway_discovery_path(
        explicit_discovery_path.as_deref(),
        expected_editor_instance_id.as_deref(),
    )
    .unwrap_or_else(|error| {
        eprintln!("{}: {} {}", error.code, error.message, error.next_action);
        std::process::exit(1);
    });
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .unwrap_or_else(|error| {
            eprintln!("Failed to read CLI request: {error}");
            std::process::exit(2);
        });
    let payload: GatewayRequestPayload = serde_json::from_str(&input).unwrap_or_else(|error| {
        eprintln!("CLI request is not a typed GatewayRequestPayload: {error}");
        std::process::exit(2);
    });
    let mut adapter = GatewayRemoteAdapter::connect_from_discovery(
        &discovery_path,
        ClientKind::Cli,
        "ai-first-game-engine-cli.v1",
    )
    .unwrap_or_else(|error| {
        eprintln!("{}: {}", error.code, error.message);
        std::process::exit(1);
    });
    let reply = adapter.dispatch(payload).unwrap_or_else(|error| {
        eprintln!("{}: {}", error.code, error.message);
        std::process::exit(1);
    });
    println!("{}", serde_json::to_string(&reply).unwrap());
    let _ = adapter.close();
}
