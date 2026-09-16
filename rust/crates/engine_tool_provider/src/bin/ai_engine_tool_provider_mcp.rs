use engine_tool_provider::mcp_stdio::run_mcp_stdio;
use engine_tool_provider::HostSessionContext;
use std::io::{self, BufReader};
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut workspace_root = None;
    let mut project_root = None;
    let mut session_id = format!("mcp-process-{}", std::process::id());
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--workspace-root" => {
                workspace_root =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--workspace-root requires a path.".to_string()
                    })?));
            }
            "--project-root" => {
                project_root =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--project-root requires a path.".to_string()
                    })?));
            }
            "--session-id" => {
                session_id = arguments
                    .next()
                    .ok_or_else(|| "--session-id requires a value.".to_string())?
                    .to_string_lossy()
                    .into_owned();
            }
            unknown => return Err(format!("Unknown argument: {unknown}")),
        }
    }
    let workspace_root = workspace_root
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|error| error.to_string())?;
    run_mcp_stdio(
        HostSessionContext {
            session_id,
            workspace_root,
            project_root,
        },
        BufReader::new(io::stdin().lock()),
        io::stdout().lock(),
    )
}
