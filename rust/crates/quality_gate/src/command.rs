use std::collections::{HashMap, VecDeque};
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityCommandSpec {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub environment: Vec<(String, String)>,
}

impl QualityCommandSpec {
    pub fn new(
        id: impl Into<String>,
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
        working_directory: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Self {
        Self {
            id: id.into(),
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            working_directory: working_directory.into(),
            timeout,
            environment: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityCommandOutcome {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
    pub timed_out: bool,
    pub output_truncated: bool,
    pub spawn_error: Option<String>,
}

impl QualityCommandOutcome {
    pub fn success(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            exit_code: Some(0),
            stdout: stdout.into(),
            stderr: Vec::new(),
            duration: Duration::ZERO,
            timed_out: false,
            output_truncated: false,
            spawn_error: None,
        }
    }

    pub fn passed(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && self.spawn_error.is_none()
    }
}

pub trait QualityCommandExecutor {
    fn execute(&self, spec: &QualityCommandSpec) -> QualityCommandOutcome;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemQualityCommandExecutor;

impl QualityCommandExecutor for SystemQualityCommandExecutor {
    fn execute(&self, spec: &QualityCommandSpec) -> QualityCommandOutcome {
        let started = Instant::now();
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.working_directory)
            .env_clear()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in inherited_environment_allowlist() {
            if let Some(value) = env::var_os(key) {
                command.env(key, value);
            }
        }
        for (key, value) in &spec.environment {
            command.env(key, value);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return QualityCommandOutcome {
                    exit_code: None,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    duration: started.elapsed(),
                    timed_out: false,
                    output_truncated: false,
                    spawn_error: Some(error.to_string()),
                };
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_reader = thread::spawn(move || read_bounded(stdout));
        let stderr_reader = thread::spawn(move || read_bounded(stderr));

        let mut timed_out = false;
        let exit_code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None) if started.elapsed() < spec.timeout => {
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    timed_out = true;
                    let _ = child.kill();
                    break child.wait().ok().and_then(|status| status.code());
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let (stdout, stdout_truncated) =
                        stdout_reader.join().unwrap_or_else(|_| (Vec::new(), true));
                    let (stderr, stderr_truncated) =
                        stderr_reader.join().unwrap_or_else(|_| (Vec::new(), true));
                    return QualityCommandOutcome {
                        exit_code: None,
                        stdout,
                        stderr,
                        duration: started.elapsed(),
                        timed_out: false,
                        output_truncated: stdout_truncated || stderr_truncated,
                        spawn_error: Some(error.to_string()),
                    };
                }
            }
        };

        let (stdout, stdout_truncated) =
            stdout_reader.join().unwrap_or_else(|_| (Vec::new(), true));
        let (stderr, stderr_truncated) =
            stderr_reader.join().unwrap_or_else(|_| (Vec::new(), true));
        QualityCommandOutcome {
            exit_code,
            stdout,
            stderr,
            duration: started.elapsed(),
            timed_out,
            output_truncated: stdout_truncated || stderr_truncated,
            spawn_error: None,
        }
    }
}

fn inherited_environment_allowlist() -> &'static [&'static str] {
    &[
        "APPDATA",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "COMSPEC",
        "HOME",
        "LOCALAPPDATA",
        "NUMBER_OF_PROCESSORS",
        "PATH",
        "PATHEXT",
        "PROGRAMDATA",
        "RUSTUP_HOME",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "WINDIR",
    ]
}

fn read_bounded<R: Read + Send + 'static>(reader: Option<R>) -> (Vec<u8>, bool) {
    let Some(mut reader) = reader else {
        return (Vec::new(), false);
    };
    const TRUNCATION_MARKER: &[u8] = b"\n<quality-gate-output-truncated>\n";
    let head_limit = (MAX_CAPTURE_BYTES - TRUNCATION_MARKER.len()) / 2;
    let tail_limit = MAX_CAPTURE_BYTES - TRUNCATION_MARKER.len() - head_limit;
    let mut head = Vec::new();
    let mut tail = VecDeque::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let head_remaining = head_limit.saturating_sub(head.len());
                let head_bytes = head_remaining.min(read);
                head.extend_from_slice(&buffer[..head_bytes]);
                for byte in &buffer[head_bytes..read] {
                    if tail.len() == tail_limit {
                        tail.pop_front();
                        truncated = true;
                    }
                    tail.push_back(*byte);
                }
            }
            Err(_) => {
                truncated = true;
                break;
            }
        }
    }
    if tail.is_empty() {
        (head, truncated)
    } else {
        let mut kept = head;
        if truncated {
            kept.extend_from_slice(TRUNCATION_MARKER);
        }
        kept.extend(tail);
        (kept, truncated)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScriptedQualityCommandExecutor {
    outcomes: Arc<Mutex<HashMap<String, VecDeque<QualityCommandOutcome>>>>,
    observed: Arc<Mutex<Vec<QualityCommandSpec>>>,
}

impl ScriptedQualityCommandExecutor {
    pub fn push(&self, id: impl Into<String>, outcome: QualityCommandOutcome) {
        self.outcomes
            .lock()
            .expect("scripted outcome lock")
            .entry(id.into())
            .or_default()
            .push_back(outcome);
    }

    pub fn observed(&self) -> Vec<QualityCommandSpec> {
        self.observed.lock().expect("observed lock").clone()
    }
}

impl QualityCommandExecutor for ScriptedQualityCommandExecutor {
    fn execute(&self, spec: &QualityCommandSpec) -> QualityCommandOutcome {
        self.observed
            .lock()
            .expect("observed lock")
            .push(spec.clone());
        self.outcomes
            .lock()
            .expect("scripted outcome lock")
            .get_mut(&spec.id)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| QualityCommandOutcome {
                exit_code: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                duration: Duration::ZERO,
                timed_out: false,
                output_truncated: false,
                spawn_error: Some(format!("missing scripted outcome for {}", spec.id)),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn scripted_executor_records_and_returns_outcome() {
        let executor = ScriptedQualityCommandExecutor::default();
        executor.push("fmt", QualityCommandOutcome::success("ok"));
        let spec = QualityCommandSpec::new("fmt", "cargo", ["fmt"], ".", Duration::from_secs(1));
        assert!(executor.execute(&spec).passed());
        assert_eq!(executor.observed(), vec![spec]);
    }

    #[test]
    fn system_executor_times_out_and_reaps_child() {
        let spec = if cfg!(windows) {
            QualityCommandSpec::new(
                "timeout",
                "powershell",
                ["-NoProfile", "-Command", "Start-Sleep -Seconds 5"],
                ".",
                Duration::from_millis(25),
            )
        } else {
            QualityCommandSpec::new(
                "timeout",
                "sh",
                ["-c", "sleep 5"],
                ".",
                Duration::from_millis(25),
            )
        };
        let outcome = SystemQualityCommandExecutor.execute(&spec);
        assert!(outcome.timed_out);
        assert!(outcome.duration < Duration::from_secs(2));
    }

    #[test]
    fn bounded_capture_preserves_head_and_tail() {
        let mut input = vec![b'a'; MAX_CAPTURE_BYTES + 1024];
        input[0] = b'H';
        let last = input.len() - 1;
        input[last] = b'T';
        let (captured, truncated) = read_bounded(Some(Cursor::new(input)));
        assert!(truncated);
        assert_eq!(captured.first(), Some(&b'H'));
        assert_eq!(captured.last(), Some(&b'T'));
        assert!(captured.len() <= MAX_CAPTURE_BYTES);
    }
}
