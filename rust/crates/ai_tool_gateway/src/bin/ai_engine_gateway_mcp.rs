use ai_tool_gateway::{resolve_gateway_discovery_path, run_mcp_stdio};
use std::io::{stdin, stdout, BufReader};
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
    if let Err(error) = run_mcp_stdio(
        &discovery_path,
        BufReader::new(stdin().lock()),
        stdout().lock(),
    ) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
