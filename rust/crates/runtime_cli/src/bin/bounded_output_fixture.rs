use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const FLOOD_BYTES: usize = 1024 * 1024 + 257;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--semantic-playtest-worker") => {
            let input: serde_json::Value =
                serde_json::from_slice(&std::fs::read(required_path_argument(2)).unwrap()).unwrap();
            match input["scenario"]["scenarioId"].as_str() {
                Some("fixture.timeout") => {
                    let output = required_path_argument(3);
                    let mut descendant = Command::new(std::env::current_exe().unwrap())
                        .arg("timeout")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .unwrap();
                    std::fs::write(
                        output.with_extension("ready"),
                        serde_json::to_vec(&[std::process::id(), descendant.id()]).unwrap(),
                    )
                    .unwrap();
                    println!("semantic fixture waiting for timeout");
                    eprintln!("semantic fixture timeout first cause");
                    std::thread::sleep(Duration::from_secs(30));
                    let _ = descendant.kill();
                    let _ = descendant.wait();
                }
                Some("fixture.no-report") => {}
                _ => std::process::exit(23),
            }
        }
        Some("semantic-owner") => {
            let root = required_path_argument(2);
            let scenario =
                serde_json::from_slice(&std::fs::read(root.join("scenario.json")).unwrap())
                    .unwrap();
            let _ = runtime_cli::run_bounded_semantic_playtest(
                runtime_cli::SemanticPlaytestProcessRequest {
                    player_executable: std::env::current_exe().unwrap(),
                    runtime_package: root.clone(),
                    scenario,
                    output_dir: root.join("owned-run"),
                },
            );
        }
        Some("flood-success") => {
            write_flood(io::stdout().lock(), b'o');
            write_flood(io::stderr().lock(), b'e');
        }
        Some("nonzero") => {
            println!("bounded fixture stdout nonzero");
            eprintln!("bounded fixture stderr nonzero");
            std::process::exit(23);
        }
        Some("timeout") => {
            println!("bounded fixture waiting for timeout");
            eprintln!("bounded fixture timeout stderr");
            std::thread::sleep(Duration::from_secs(30));
        }
        Some("print-environment") => {
            println!(
                "{}",
                std::env::var("AIFE_BOUNDED_CHILD_TEST").unwrap_or_default()
            );
        }
        Some("spawn-grandchild") => {
            let sentinel = required_path_argument(2);
            let mut grandchild = Command::new(std::env::current_exe().unwrap())
                .arg("grandchild-sentinel")
                .arg(sentinel)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            println!("bounded fixture spawned grandchild");
            std::thread::sleep(Duration::from_secs(30));
            let _ = grandchild.kill();
            let _ = grandchild.wait();
        }
        Some("grandchild-sentinel") => {
            let sentinel = required_path_argument(2);
            std::thread::sleep(Duration::from_millis(1250));
            std::fs::write(sentinel, b"grandchild survived").unwrap();
            std::thread::sleep(Duration::from_secs(30));
        }
        _ => std::process::exit(2),
    }
}

fn required_path_argument(index: usize) -> PathBuf {
    std::env::args_os()
        .nth(index)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::process::exit(2))
}

fn write_flood(mut stream: impl Write, byte: u8) {
    let buffer = [byte; 16 * 1024];
    let mut remaining = FLOOD_BYTES;
    while remaining > 0 {
        let count = remaining.min(buffer.len());
        stream.write_all(&buffer[..count]).unwrap();
        remaining -= count;
    }
    stream.flush().unwrap();
}
