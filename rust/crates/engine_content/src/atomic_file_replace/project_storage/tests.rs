use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

fn test_root(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let root =
        PathBuf::from(std::env::var_os("AIFE_376_TEMP_ROOT").expect("explicit isolated test root"))
            .join(format!(
                "{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn project_storage_376_commit_faults_preserve_old_and_cleanup_warning_stays_committed() {
    let root = test_root("faults");
    let mut owner = ProjectStorage::open(&root, "project-a", "default").unwrap();
    let initial = owner.write("save/data", b"old");
    assert_eq!(initial.state, CommitState::Committed, "{initial:?}");
    for point in [
        WritePoint::BeforeWrite,
        WritePoint::BeforeSync,
        WritePoint::BeforeReplace,
    ] {
        let result = owner.write_observed("save/data", b"new", |seen| {
            if seen == point {
                Err(io::Error::other("injected failure"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.state, CommitState::NotCommitted);
        assert!(result.error.is_some());
        assert_eq!(owner.read("save/data", 100).unwrap(), b"old");
    }
    let result = owner.write_observed("save/data", b"new", |point| {
        if point == WritePoint::AfterReplace {
            Err(io::Error::other("cleanup failure"))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.state, CommitState::Committed);
    assert!(result.error.is_some());
    assert_eq!(owner.read("save/data", 100).unwrap(), b"new");
}

#[test]
fn project_storage_376_keys_namespace_empty_missing_bounds_and_lease() {
    let root = test_root("namespace");
    let mut owner = ProjectStorage::open(&root, "project-a", "default").unwrap();
    assert_eq!(
        ProjectStorage::open(&root, "project-a", "default")
            .unwrap_err()
            .kind,
        StorageErrorKind::Busy
    );
    for key in [
        "",
        "/absolute",
        "../escape",
        "a/../b",
        "a//b",
        "C:/save",
        "\\\\host\\save",
        "a:stream",
        "a/.",
        "a/last.",
        "a/last ",
    ] {
        assert_eq!(
            owner.write(key, b"invalid").error.unwrap().kind,
            StorageErrorKind::InvalidKey,
            "{key}"
        );
    }
    assert_eq!(
        owner.read("missing", 100).unwrap_err().kind,
        StorageErrorKind::NotFound
    );
    let empty = owner.write("CON/NUL", b"");
    assert_eq!(empty.state, CommitState::Committed, "{empty:?}");
    assert_eq!(owner.read("CON/NUL", 0).unwrap(), b"");
    assert_eq!(owner.write("value", b"abc").state, CommitState::Committed);
    assert_eq!(
        owner.read("value", 2).unwrap_err().kind,
        StorageErrorKind::TooLarge
    );
    assert_eq!(owner.delete("value").state, CommitState::Committed);
    assert_eq!(
        owner.read("value", 100).unwrap_err().kind,
        StorageErrorKind::NotFound
    );
    for (identity, profile) in [("project-b", "default"), ("project-a", "other")] {
        let other = ProjectStorage::open(&root, identity, profile).unwrap();
        assert_eq!(
            other.read("CON/NUL", 100).unwrap_err().kind,
            StorageErrorKind::NotFound
        );
    }
    drop(owner);
    // No package revision/hash enters this identity; a new owner reads the same namespace.
    let reloaded = ProjectStorage::open(&root, "project-a", "default").unwrap();
    assert_eq!(reloaded.read("CON/NUL", 0).unwrap(), b"");
    let isolated = ProjectStorage::open(&test_root("isolated"), "project-a", "default").unwrap();
    assert_eq!(
        isolated.read("CON/NUL", 0).unwrap_err().kind,
        StorageErrorKind::NotFound
    );
}

#[test]
fn project_storage_376_directory_swap_and_hard_link_escape_are_rejected() {
    let root = test_root("escape");
    let mut owner = ProjectStorage::open(&root, "project", "profile").unwrap();
    let outside = test_root("outside");
    std::fs::write(outside.join("private"), b"private").unwrap();
    let leaf = owner.root.join(key_name("secret").unwrap());
    std::fs::hard_link(outside.join("private"), &leaf).unwrap();
    assert_eq!(
        owner.read("secret", 100).unwrap_err().kind,
        StorageErrorKind::CapabilityGap
    );
    assert_eq!(
        owner.delete("secret").error.unwrap().kind,
        StorageErrorKind::CapabilityGap
    );
    assert_eq!(owner.write("secret", b"safe").state, CommitState::Committed);
    assert_eq!(std::fs::read(outside.join("private")).unwrap(), b"private");
    assert!(std::fs::rename(&owner.root, root.join("swapped")).is_err());
    assert!(std::fs::rename(&root, root.with_extension("swapped")).is_err());
    // A preexisting junction must not be followed while opening a storage root.
    let link = root.join("junction");
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link.as_os_str().to_string_lossy().replace('/', "\\"))
        .arg(outside.as_os_str().to_string_lossy().replace('/', "\\"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "junction fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        ProjectStorage::open(&link, "project", "profile")
            .unwrap_err()
            .kind,
        StorageErrorKind::CapabilityGap
    );
    std::fs::remove_dir(&link).unwrap(); // Remove only the junction entry, never its target.
    drop(owner);
    assert_eq!(std::fs::read(outside.join("private")).unwrap(), b"private");
}

#[test]
fn project_storage_376_rename_error_is_unknown_unless_rejection_is_known() {
    assert_eq!(
        windows::rename_failure_state(&io::Error::from_raw_os_error(5)),
        CommitState::NotCommitted
    );
    assert_eq!(
        windows::rename_failure_state(&io::Error::from_raw_os_error(1117)),
        CommitState::Unknown
    );
}

#[test]
fn project_storage_376_replace_keeps_open_old_view_and_new_opens_see_complete_new_value() {
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let root = test_root("readers");
    let mut owner = ProjectStorage::open(&root, "project", "profile").unwrap();
    assert_eq!(
        owner.write("state", &vec![0x21; 65_536]).state,
        CommitState::Committed
    );
    let path = owner.root.join(key_name("state").unwrap());
    let mut old = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(&path)
        .unwrap();
    let swapped = owner.write("state", &vec![0x7e; 65_536]);
    assert_eq!(swapped.state, CommitState::Committed, "{swapped:?}");
    let mut bytes = Vec::new();
    old.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, vec![0x21; 65_536]);
    assert_eq!(owner.read("state", 65_536).unwrap(), vec![0x7e; 65_536]);
}

#[test]
fn project_storage_376_concurrent_opens_during_replacement_never_see_missing_or_partial_value() {
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let root = test_root("concurrent");
    let mut owner = ProjectStorage::open(&root, "project", "profile").unwrap();
    assert_eq!(
        owner.write("state", &vec![0x21; 65_536]).state,
        CommitState::Committed
    );
    let path = owner.root.join(key_name("state").unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
    let reader = std::thread::spawn(move || {
        let mut count = 0;
        loop {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .open(&path)
                .unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes.len(), 65_536);
            assert!(bytes.iter().all(|b| *b == 0x21) || bytes.iter().all(|b| *b == 0x7e));
            if count == 0 {
                entered_tx.send(()).unwrap();
            }
            count += 1;
            if stopped.load(Ordering::Acquire) {
                return count;
            }
        }
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    for index in 0..32 {
        let result = owner.write(
            "state",
            &vec![if index % 2 == 0 { 0x7e } else { 0x21 }; 65_536],
        );
        assert_eq!(result.state, CommitState::Committed, "{result:?}");
    }
    stop.store(true, Ordering::Release);
    assert!(reader.join().unwrap() > 1);
}

fn reparse(path: &Path, target: Option<&Path>) -> io::Result<()> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::{
        Foundation::GENERIC_WRITE, Storage::FileSystem::*, System::IO::DeviceIoControl,
    };
    let file = std::fs::OpenOptions::new()
        .access_mode(GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&0xa0000003u32.to_le_bytes());
    if let Some(target) = target {
        let print = target.to_str().unwrap().replace('/', "\\");
        let substitute: Vec<u16> = format!("\\??\\{print}").encode_utf16().collect();
        let print: Vec<u16> = print.encode_utf16().collect();
        let length = 8 + (substitute.len() + print.len() + 2) * 2;
        buffer.extend_from_slice(&(length as u16).to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        for value in [
            0,
            substitute.len() * 2,
            (substitute.len() + 1) * 2,
            print.len() * 2,
        ] {
            buffer.extend_from_slice(&(value as u16).to_le_bytes());
        }
        for value in substitute
            .into_iter()
            .chain(Some(0))
            .chain(print)
            .chain(Some(0))
        {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
    } else {
        buffer.extend_from_slice(&[0; 4]);
    }
    let mut returned = 0;
    // SAFETY: live non-overlapped directory and complete mount-point reparse buffer.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            if target.is_some() { 0x900a4 } else { 0x900ac },
            buffer.as_ptr().cast(),
            buffer.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[test]
fn project_storage_376_reparse_inserted_after_base_check_cannot_redirect_relative_io() {
    let root = test_root("race");
    let outside = test_root("race-outside");
    let name = namespace("project", "profile").unwrap();
    let opened = windows::Owner::open_observed(&root, &name, |_| {
        reparse(&root, Some(&outside))?;
        // Counterfactual: a full-path open really would follow the inserted junction.
        std::fs::create_dir(root.join("path-control"))?;
        assert!(outside.join("path-control").exists());
        std::fs::remove_dir(outside.join("path-control"))?;
        Ok(())
    });
    // The race is real: the checked base was converted to a junction, before child open.
    assert!(
        std::fs::symlink_metadata(&root)
            .unwrap()
            .file_type()
            .is_symlink()
            || {
                use std::os::windows::fs::MetadataExt;
                std::fs::symlink_metadata(&root).unwrap().file_attributes() & 0x400 != 0
            }
    );
    let succeeded = match opened {
        Ok((_, mut owner)) => {
            let result = owner.write(&key_name("safe").unwrap(), b"inside", |_| Ok(()));
            assert_eq!(result.state, CommitState::Committed, "{result:?}");
            assert_eq!(
                owner.read(&key_name("safe").unwrap(), 100).unwrap(),
                b"inside"
            );
            true
        }
        Err(error) => {
            assert_eq!(error.kind, StorageErrorKind::CapabilityGap);
            println!("changed-base native open safely rejected: {error:?}");
            false
        }
    };
    assert!(
        !outside.join(&name).exists(),
        "relative child open escaped through changed base"
    );
    reparse(&root, None).unwrap();
    assert_eq!(root.join(name).exists(), succeeded);
}

fn handshake(root: &Path, label: &str) {
    std::fs::write(root.join("signal"), label).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !root.join("continue").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "parent handshake timeout"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn project_storage_376_process_child() {
    let Some(root) = std::env::var_os("AIFE_376_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("AIFE_376_CHILD_MODE").unwrap();
    let mut owner = ProjectStorage::open(&root, "project", "profile").unwrap();
    if mode == "lease" {
        handshake(&root, "lease");
        return;
    }
    let point = if mode == "before" {
        WritePoint::BeforeReplace
    } else {
        WritePoint::AfterReplace
    };
    let result = owner.write_observed("state", &vec![0x7e; 65_536], |seen| {
        if seen == point {
            handshake(&root, &mode);
        }
        Ok(())
    });
    assert_eq!(result.state, CommitState::Committed);
}

struct OwnedChild(std::process::Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
fn project_storage_376_real_process_interrupt_lease_recovery_and_independent_reload() {
    use std::process::{Command, Stdio};
    for mode in ["lease", "before", "after"] {
        let root = test_root(mode);
        let mut initial = ProjectStorage::open(&root, "project", "profile").unwrap();
        assert_eq!(
            initial.write("state", &vec![0x21; 65_536]).state,
            CommitState::Committed
        );
        drop(initial);
        let mut child = OwnedChild(Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "atomic_file_replace::project_storage::tests::project_storage_376_process_child", "--nocapture"])
            .env("AIFE_376_CHILD_ROOT", &root).env("AIFE_376_CHILD_MODE", mode)
            .stdout(Stdio::from(std::fs::File::create(root.join("child.stdout.log")).unwrap()))
            .stderr(Stdio::from(std::fs::File::create(root.join("child.stderr.log")).unwrap()))
            .spawn().unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !root.join("signal").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited before {mode} handshake; see {}",
                root.display()
            );
            assert!(
                std::time::Instant::now() < deadline,
                "child {mode} handshake timeout"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(std::fs::read_to_string(root.join("signal")).unwrap(), mode);
        assert_eq!(
            ProjectStorage::open(&root, "project", "profile")
                .unwrap_err()
                .kind,
            StorageErrorKind::Busy
        );
        child.0.kill().unwrap();
        let status = child.0.wait().unwrap();
        assert!(!status.success());
        let owner = ProjectStorage::open(&root, "project", "profile").unwrap();
        let expected = if mode == "after" { 0x7e } else { 0x21 };
        let bytes = owner.read("state", 65_536).unwrap();
        assert_eq!(bytes, vec![expected; 65_536]);
        assert!(!std::fs::read_dir(&owner.root).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".aife-tmp-")));
        std::fs::write(root.join("process-result.json"), serde_json::json!({"mode": mode, "pid": child.0.id(), "exit": status.code(), "sha256": format!("{:x}", Sha256::digest(&bytes)), "byteCount": bytes.len(), "leaseRecovered": true}).to_string()).unwrap();
    }
}
