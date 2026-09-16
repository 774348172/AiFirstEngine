use engine_runtime::atomic_directory_publish::{
    atomic_directory_publish, AtomicDirectoryPublishError,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const RETRY_TIMEOUT: Duration = Duration::from_secs(20);
const RETRY_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishCompletion {
    owner: String,
    payload_hash: String,
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("usage: atomic_publish_fixture <root> <owner>");
        std::process::exit(2);
    }
    let root = PathBuf::from(&args[0]);
    if let Err(error) = publish_with_retry(&root, &args[1]) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn publish_with_retry(root: &Path, owner: &str) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let final_dir = root.join("published");
    let sentinel = root.join("active-publisher.sentinel");
    let history = root.join("publish-history.jsonl");
    let payload = format!("atomic-publish-owner={owner}\n");
    let payload_hash = sha256_prefixed(payload.as_bytes());
    let started = Instant::now();

    loop {
        let result = atomic_directory_publish(
            &final_dir,
            |staging| {
                let sentinel_file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&sentinel)
                    .map_err(|error| format!("atomic_publish_overlap_detected: {error}"))?;
                fs::write(staging.join("payload.txt"), payload.as_bytes())
                    .map_err(|error| error.to_string())?;
                fs::write(staging.join("payload.sha256"), payload_hash.as_bytes())
                    .map_err(|error| error.to_string())?;
                thread::sleep(Duration::from_millis(25));
                drop(sentinel_file);
                fs::remove_file(&sentinel).map_err(|error| error.to_string())
            },
            |candidate| {
                validate_payload(candidate, &payload_hash)?;
                if candidate == final_dir {
                    let completion = PublishCompletion {
                        owner: owner.to_string(),
                        payload_hash: payload_hash.clone(),
                    };
                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&history)
                        .map_err(|error| error.to_string())?;
                    serde_json::to_writer(&mut file, &completion)
                        .map_err(|error| error.to_string())?;
                    file.write_all(b"\n").map_err(|error| error.to_string())?;
                    file.flush().map_err(|error| error.to_string())?;
                }
                Ok(())
            },
        );
        match result {
            Ok(()) => return Ok(()),
            Err(error)
                if error.code == "output_publish_busy" && started.elapsed() < RETRY_TIMEOUT =>
            {
                thread::sleep(RETRY_INTERVAL);
            }
            Err(error) => return Err(format_publish_error(error)),
        }
    }
}

fn validate_payload(candidate: &Path, expected_hash: &str) -> Result<(), String> {
    let payload = fs::read(candidate.join("payload.txt")).map_err(|error| error.to_string())?;
    let recorded_hash =
        fs::read_to_string(candidate.join("payload.sha256")).map_err(|error| error.to_string())?;
    let actual_hash = sha256_prefixed(&payload);
    if actual_hash != expected_hash || recorded_hash != expected_hash {
        return Err(format!(
            "payload hash mismatch: expected={expected_hash} actual={actual_hash} recorded={recorded_hash}"
        ));
    }
    Ok(())
}

fn format_publish_error(error: AtomicDirectoryPublishError) -> String {
    format!(
        "{}: {} ({})",
        error.code,
        error.message,
        error.path.display()
    )
}
