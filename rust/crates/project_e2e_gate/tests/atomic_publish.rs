use engine_runtime::canonical_digest::sha256_prefixed;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const ROUNDS: usize = 6;
const PROCESS_COUNT: usize = 3;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishCompletion {
    owner: String,
    payload_hash: String,
}

#[test]
fn atomic_publish_three_process_multi_round_handoff_is_single_owner() {
    let root = temp_root();
    let fixture = env!("CARGO_BIN_EXE_atomic_publish_fixture");

    for round in 0..ROUNDS {
        let children = (0..PROCESS_COUNT)
            .map(|process| {
                spawn_fixture(fixture, &root, &format!("round-{round}-process-{process}"))
            })
            .collect::<Vec<_>>();
        wait_for_round(children, round);
    }

    let history = fs::read_to_string(root.join("publish-history.jsonl")).unwrap();
    let completions = history
        .lines()
        .map(|line| serde_json::from_str::<PublishCompletion>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(completions.len(), ROUNDS * PROCESS_COUNT);
    let last = completions.last().unwrap();
    let final_dir = root.join("published");
    let payload = fs::read(final_dir.join("payload.txt")).unwrap();
    assert_eq!(sha256_prefixed(&payload), last.payload_hash);
    assert_eq!(
        String::from_utf8(payload).unwrap(),
        format!("atomic-publish-owner={}\n", last.owner)
    );
    assert_eq!(
        fs::read_to_string(final_dir.join("payload.sha256")).unwrap(),
        last.payload_hash
    );
    assert!(root.join(".published.publish.lock").is_file());
    assert!(!final_dir.join(".published.publish.lock").exists());
    assert!(!root.join("active-publisher.sentinel").exists());
}

fn spawn_fixture(fixture: &str, root: &PathBuf, owner: &str) -> Child {
    Command::new(fixture)
        .arg(root)
        .arg(owner)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn wait_for_round(children: Vec<Child>, round: usize) {
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "round {round} fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn temp_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("atomic-publish-e2e-{stamp}"))
}
