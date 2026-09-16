use editor_core::ProjectWriteScope;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROJECT_WRITE_CONTAINMENT_REPORT_SCHEMA_VERSION: &str =
    "project-write-containment-report.v1";

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWriteContainmentStatus {
    Passed,
    Failed,
    ExplicitlySkipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteContainmentEvidence {
    pub fixture: String,
    pub status: ProjectWriteContainmentStatus,
    pub executed: bool,
    pub outside_sentinel_unchanged: bool,
    pub rejection_code: Option<String>,
    pub os_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteContainmentReport {
    pub schema_version: String,
    pub status: ProjectWriteContainmentStatus,
    pub platform: String,
    pub symlink: ProjectWriteContainmentEvidence,
    pub junction: ProjectWriteContainmentEvidence,
    pub hard_link: ProjectWriteContainmentEvidence,
    pub concurrent_swap: ProjectWriteContainmentEvidence,
    pub diagnostics: Vec<String>,
}

pub fn run_project_write_containment_report() -> ProjectWriteContainmentReport {
    let hard_link = hard_link_fixture();
    let symlink = symlink_fixture();
    let junction = junction_fixture();
    let concurrent_swap = concurrent_swap_fixture();

    let required_link_coverage = if cfg!(windows) {
        junction.status == ProjectWriteContainmentStatus::Passed
    } else {
        symlink.status == ProjectWriteContainmentStatus::Passed
    };
    let passed = required_link_coverage
        && hard_link.status == ProjectWriteContainmentStatus::Passed
        && concurrent_swap.status == ProjectWriteContainmentStatus::Passed
        && symlink.status != ProjectWriteContainmentStatus::Failed;
    let mut diagnostics = Vec::new();
    if !required_link_coverage {
        diagnostics.push("required_link_or_junction_coverage_missing".to_string());
    }
    for evidence in [&hard_link, &symlink, &junction, &concurrent_swap] {
        if evidence.status == ProjectWriteContainmentStatus::Failed {
            diagnostics.push(format!("{}_failed", evidence.fixture));
        }
    }

    ProjectWriteContainmentReport {
        schema_version: PROJECT_WRITE_CONTAINMENT_REPORT_SCHEMA_VERSION.to_string(),
        status: if passed {
            ProjectWriteContainmentStatus::Passed
        } else {
            ProjectWriteContainmentStatus::Failed
        },
        platform: std::env::consts::OS.to_string(),
        symlink,
        junction,
        hard_link,
        concurrent_swap,
        diagnostics,
    }
}

fn hard_link_fixture() -> ProjectWriteContainmentEvidence {
    let root = fixture_root("hard-link");
    let outside = fixture_root("hard-link-outside");
    fs::create_dir_all(root.join("Assets")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("sentinel.bin");
    fs::write(&outside_file, b"outside-sentinel").unwrap();
    fs::hard_link(&outside_file, root.join("Assets/linked.bin")).unwrap();
    let scope = ProjectWriteScope::open(&root).unwrap();
    let result = scope.write_atomic("Assets/linked.bin", b"project-data");
    let unchanged = fs::read(&outside_file).unwrap() == b"outside-sentinel";
    evidence(
        "hard_link",
        result.is_ok() && unchanged,
        true,
        unchanged,
        result.err().map(|error| error.code.to_string()),
        None,
    )
}

fn symlink_fixture() -> ProjectWriteContainmentEvidence {
    let root = fixture_root("symlink");
    let outside = fixture_root("symlink-outside");
    fs::create_dir_all(root.join("Assets")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("sentinel.bin");
    fs::write(&outside_file, b"outside-sentinel").unwrap();
    let link = root.join("Assets/linked.bin");
    if let Err(error) = create_file_symlink(&outside_file, &link) {
        return ProjectWriteContainmentEvidence {
            fixture: "symlink".to_string(),
            status: if cfg!(windows) {
                ProjectWriteContainmentStatus::ExplicitlySkipped
            } else {
                ProjectWriteContainmentStatus::Failed
            },
            executed: false,
            outside_sentinel_unchanged: true,
            rejection_code: None,
            os_error: Some(error),
        };
    }
    let scope = ProjectWriteScope::open(&root).unwrap();
    let write = scope.write_atomic("Assets/linked.bin", b"project-data");
    let remove = scope.remove_file("Assets/linked.bin");
    let unchanged = fs::read(&outside_file).unwrap() == b"outside-sentinel";
    let code = write.as_ref().err().map(|error| error.code.to_string());
    evidence(
        "symlink",
        write.is_err() && remove.is_err() && unchanged,
        true,
        unchanged,
        code,
        None,
    )
}

#[cfg(windows)]
fn junction_fixture() -> ProjectWriteContainmentEvidence {
    let root = fixture_root("junction");
    let outside = fixture_root("junction-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("sentinel.bin");
    fs::write(&outside_file, b"outside-sentinel").unwrap();
    let link = root.join("Build");
    if let Err(error) = create_junction(&outside, &link) {
        return evidence("junction", false, false, true, None, Some(error));
    }
    let scope = ProjectWriteScope::open(&root).unwrap();
    let result = scope.write_atomic("Build/sentinel.bin", b"escaped");
    let unchanged = fs::read(&outside_file).unwrap() == b"outside-sentinel";
    let code = result.as_ref().err().map(|error| error.code.to_string());
    evidence(
        "junction",
        result.is_err() && unchanged,
        true,
        unchanged,
        code,
        None,
    )
}

#[cfg(not(windows))]
fn junction_fixture() -> ProjectWriteContainmentEvidence {
    ProjectWriteContainmentEvidence {
        fixture: "junction".to_string(),
        status: ProjectWriteContainmentStatus::ExplicitlySkipped,
        executed: false,
        outside_sentinel_unchanged: true,
        rejection_code: None,
        os_error: Some("not_applicable_on_non_windows".to_string()),
    }
}

fn concurrent_swap_fixture() -> ProjectWriteContainmentEvidence {
    let root = fixture_root("swap");
    let outside = fixture_root("swap-outside");
    fs::create_dir_all(root.join("slot")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("value.bin");
    fs::write(&outside_file, b"outside-sentinel").unwrap();
    let scope = Arc::new(ProjectWriteScope::open(&root).unwrap());
    let writer_scope = Arc::clone(&scope);
    let barrier = Arc::new(Barrier::new(2));
    let writer_barrier = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        let mut accepted = true;
        for index in 0..16 {
            writer_barrier.wait();
            if let Err(error) =
                writer_scope.write_atomic("slot/value.bin", index.to_string().as_bytes())
            {
                accepted &= error.code.starts_with("project_write.");
            }
            writer_barrier.wait();
        }
        accepted
    });

    let mut swap_executed = false;
    for _ in 0..16 {
        let slot = root.join("slot");
        let parked = root.join("slot-parked");
        let prepared =
            fs::rename(&slot, &parked).is_ok() && create_directory_link(&outside, &slot).is_ok();
        swap_executed |= prepared;
        barrier.wait();
        barrier.wait();
        if prepared {
            let _ = fs::remove_dir(&slot);
            let _ = fs::rename(&parked, &slot);
        } else if parked.exists() {
            let _ = fs::rename(&parked, &slot);
        }
    }
    let writer_accepted = writer.join().unwrap_or(false);
    let unchanged = fs::read(&outside_file).unwrap() == b"outside-sentinel";
    evidence(
        "concurrent_swap",
        swap_executed && writer_accepted && unchanged,
        swap_executed,
        unchanged,
        None,
        (!swap_executed).then(|| "directory_swap_could_not_execute".to_string()),
    )
}

fn evidence(
    fixture: &str,
    passed: bool,
    executed: bool,
    unchanged: bool,
    rejection_code: Option<String>,
    os_error: Option<String>,
) -> ProjectWriteContainmentEvidence {
    ProjectWriteContainmentEvidence {
        fixture: fixture.to_string(),
        status: if passed {
            ProjectWriteContainmentStatus::Passed
        } else {
            ProjectWriteContainmentStatus::Failed
        },
        executed,
        outside_sentinel_unchanged: unchanged,
        rejection_code,
        os_error,
    }
}

fn fixture_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "aife-project-write-containment-{label}-{}-{stamp}-{sequence}",
        std::process::id()
    ))
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_file(target, link).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> Result<(), String> {
    create_junction(target, link)
}

#[cfg(windows)]
fn create_junction(target: &Path, link: &Path) -> Result<(), String> {
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "mklink /J failed status={:?} stdout={} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_write_containment_real_filesystem_gate_passes() {
        let report = run_project_write_containment_report();
        eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
        assert_eq!(report.status, ProjectWriteContainmentStatus::Passed);
        assert!(report.hard_link.outside_sentinel_unchanged);
        assert!(report.concurrent_swap.outside_sentinel_unchanged);
    }
}
