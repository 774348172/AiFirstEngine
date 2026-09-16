use crate::{
    CanonicalSourceInventory, DocumentWriteRequest, DocumentWriteStatus,
    EmbeddedAuthoringProjectContext, MutationCommitProgress, OpenOptions, ProjectLocator,
    ProjectMutation, ProjectMutationBeforeState, ProjectMutationOperation, ProjectQualification,
    ProjectRecoveryDisposition, RefreshStatus, SnapshotLeaseRequest, SnapshotReleaseOutcome,
    SnapshotRequest, SnapshotRetentionPolicy,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn open_valid_project_without_editor_dependencies() {
    let fixture = Fixture::project("valid");
    let mut context = EmbeddedAuthoringProjectContext::new();

    context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .expect("a valid project should open without an EditorSession");
}

#[test]
fn open_missing_root_returns_typed_diagnostic() {
    let fixture = Fixture::missing("missing-root");
    let mut context = EmbeddedAuthoringProjectContext::new();

    let error = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();

    assert_eq!(error.diagnostic.code, "authoring_context.root_unavailable");
}

#[test]
fn open_file_root_returns_typed_diagnostic() {
    let fixture = Fixture::file("file-root");
    let mut context = EmbeddedAuthoringProjectContext::new();

    let error = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.root_not_directory"
    );
}

#[test]
fn open_invalid_manifest_returns_typed_diagnostic() {
    let fixture = Fixture::with_manifest("invalid-manifest", b"{");
    let mut context = EmbeddedAuthoringProjectContext::new();

    let error = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();

    assert_eq!(error.diagnostic.code, "authoring_context.manifest_invalid");
}

#[test]
fn open_empty_project_id_returns_typed_diagnostic() {
    let fixture = Fixture::with_manifest(
        "empty-project-id",
        br#"{"schemaVersion":"aife-project.v2","projectId":""}"#,
    );
    let mut context = EmbeddedAuthoringProjectContext::new();

    let error = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.project_id_invalid"
    );
}

#[test]
fn revision_is_deterministic_and_tracks_canonical_source() {
    let fixture = Fixture::project("revision-deterministic");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    fs::write(fixture.path().join("Scenes/Main.scene.json"), b"scene-a").unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    let first = context.refresh(handle).unwrap();
    let second = context.refresh(handle).unwrap();
    assert_eq!(first.revision, second.revision);
    assert_eq!(second.status, RefreshStatus::Unchanged);
    assert_eq!(second.revision.qualification, ProjectQualification::Ready);

    fs::write(fixture.path().join("Scenes/Main.scene.json"), b"scene-b").unwrap();
    let changed = context.refresh(handle).unwrap();
    assert_eq!(changed.status, RefreshStatus::Changed);
    assert_ne!(first.revision.revision_id, changed.revision.revision_id);
}

#[test]
fn document_save_last_writer_wins_and_refreshes_each_context() {
    let fixture = Fixture::project("document-save-lww");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    fs::write(fixture.path().join("Scenes/Main.scene.json"), b"initial").unwrap();
    let (mut first, first_handle) = open_context(&fixture);
    let (mut second, second_handle) = open_context(&fixture);

    let first_report = first
        .save_document(
            first_handle,
            document_request("Scenes/Main.scene.json", b"first"),
        )
        .unwrap();
    assert_eq!(first_report.status, DocumentWriteStatus::Saved);
    assert_eq!(
        fs::read(fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"first"
    );

    let second_report = second
        .save_document(
            second_handle,
            document_request("Scenes/Main.scene.json", b"second"),
        )
        .unwrap();
    assert_eq!(second_report.status, DocumentWriteStatus::Saved);
    assert_eq!(
        fs::read(fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"second"
    );
    assert_eq!(
        second.current_revision(second_handle).unwrap(),
        second_report.observed_revision
    );
}

#[test]
fn document_save_equal_bytes_is_unchanged() {
    let fixture = Fixture::project("document-save-unchanged");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    fs::write(fixture.path().join("Scenes/Main.scene.json"), b"same").unwrap();
    let (mut context, handle) = open_context(&fixture);
    let before_revision = context.current_revision(handle).unwrap();

    let report = context
        .save_document(handle, document_request("Scenes/Main.scene.json", b"same"))
        .unwrap();

    assert_eq!(report.status, DocumentWriteStatus::Unchanged);
    assert_eq!(report.observed_revision, before_revision);
    assert_eq!(
        report.before_digest.as_deref(),
        Some(report.after_digest.as_str())
    );
}

#[test]
fn document_save_rejects_excluded_paths() {
    let fixture = Fixture::project("document-save-excluded");
    let (mut context, handle) = open_context(&fixture);

    let error = context
        .save_document(handle, document_request("Library/cache.json", b"{}"))
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.document_path_excluded"
    );
    assert!(!fixture.path().join("Library/cache.json").exists());
}

#[test]
fn document_save_does_not_relax_structured_mutation_cas() {
    let fixture = Fixture::project("document-save-keeps-cas");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    fs::write(fixture.path().join("Scenes/Main.scene.json"), b"initial").unwrap();
    let (mut mutation_context, mutation_handle) = open_context(&fixture);
    let mutation = mutation_for(
        &mutation_context,
        mutation_handle,
        "document-save-stale-mutation",
        "Scenes/Main.scene.json",
        b"mutation",
    );
    let (mut document_context, document_handle) = open_context(&fixture);
    document_context
        .save_document(
            document_handle,
            document_request("Scenes/Main.scene.json", b"document"),
        )
        .unwrap();

    let error = mutation_context
        .commit_mutation(mutation_handle, mutation)
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.mutation_revision_drifted"
    );
    assert_eq!(
        fs::read(fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"document"
    );
}

#[test]
fn inventory_excludes_generated_and_nested_cargo_targets() {
    let fixture = Fixture::project("inventory-exclusions");
    fs::create_dir_all(fixture.path().join("Library/cache")).unwrap();
    fs::create_dir_all(fixture.path().join("Tests/Harness/src")).unwrap();
    fs::create_dir_all(fixture.path().join("Tests/Harness/target/debug")).unwrap();
    fs::create_dir_all(fixture.path().join("Assets/target")).unwrap();
    fs::write(fixture.path().join("Library/cache/derived.bin"), b"derived").unwrap();
    fs::write(
        fixture.path().join("Tests/Harness/Cargo.toml"),
        b"[package]\nname='harness'\nversion='0.1.0'\n",
    )
    .unwrap();
    fs::write(fixture.path().join("Tests/Harness/src/lib.rs"), b"source").unwrap();
    fs::write(
        fixture
            .path()
            .join("Tests/Harness/target/debug/generated.bin"),
        b"generated",
    )
    .unwrap();
    fs::write(
        fixture.path().join("Assets/target/authored.asset"),
        b"authored",
    )
    .unwrap();

    let inventory = CanonicalSourceInventory::capture(fixture.path()).unwrap();
    let paths = inventory
        .entries()
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect::<Vec<_>>();

    assert!(paths.contains(&"Tests/Harness/src/lib.rs"));
    assert!(paths.contains(&"Assets/target/authored.asset"));
    assert!(!paths.iter().any(|path| path.starts_with("Library/")));
    assert!(!paths.iter().any(|path| path.contains("/target/debug/")));
}

#[test]
fn inventory_same_bytes_rewritten_keep_the_same_content_identity() {
    let fixture = Fixture::project("inventory-same-bytes");
    let scene = fixture.path().join("Scenes/Main.scene.json");
    fs::create_dir_all(scene.parent().unwrap()).unwrap();
    fs::write(&scene, b"stable-scene").unwrap();
    let first = CanonicalSourceInventory::capture(fixture.path()).unwrap();

    fs::write(&scene, b"stable-scene").unwrap();
    let second = CanonicalSourceInventory::capture(fixture.path()).unwrap();

    assert_eq!(first.source_digest(), second.source_digest());
}

#[test]
fn inventory_between_scan_drift_fails_closed() {
    let fixture = Fixture::project("inventory-between-scan-drift");
    let scene = fixture.path().join("Scenes/Main.scene.json");
    fs::create_dir_all(scene.parent().unwrap()).unwrap();
    fs::write(&scene, b"before").unwrap();

    let error = CanonicalSourceInventory::capture_with_between_scan_hook(fixture.path(), || {
        fs::write(&scene, b"after").unwrap();
    })
    .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.source_changed_during_refresh"
    );
}

#[test]
fn revision_invalid_manifest_replaces_previous_ready_fact() {
    let fixture = Fixture::project("revision-invalid-current-fact");
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    let ready = context.refresh(handle).unwrap();

    fs::write(fixture.path().join("project.aife.json"), b"{").unwrap();
    let invalid = context.refresh(handle).unwrap();

    assert_eq!(invalid.status, RefreshStatus::Changed);
    assert_eq!(
        invalid.revision.qualification,
        ProjectQualification::Invalid
    );
    assert_ne!(ready.revision.revision_id, invalid.revision.revision_id);
    assert!(invalid
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "authoring_context.manifest_invalid"));
}

#[test]
fn revision_clean_refresh_does_not_write_canonical_source() {
    let fixture = Fixture::project("revision-clean-refresh");
    let manifest = fixture.path().join("project.aife.json");
    let before_bytes = fs::read(&manifest).unwrap();
    let before_modified = fs::metadata(&manifest).unwrap().modified().unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    context.refresh(handle).unwrap();

    assert_eq!(fs::read(&manifest).unwrap(), before_bytes);
    assert_eq!(
        fs::metadata(&manifest).unwrap().modified().unwrap(),
        before_modified
    );
}

#[test]
fn snapshot_is_bounded_and_owns_bytes_after_live_source_changes() {
    let fixture = Fixture::project("snapshot-owned-bytes");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    fs::create_dir_all(fixture.path().join("Input")).unwrap();
    let scene = fixture.path().join("Scenes/Main.scene.json");
    fs::write(&scene, b"scene-before").unwrap();
    fs::write(fixture.path().join("Input/input.none.json"), b"unrequested").unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    let snapshot = context
        .snapshot(
            handle,
            SnapshotRequest {
                relative_paths: vec!["Scenes/Main.scene.json".to_string()],
            },
        )
        .unwrap();
    fs::write(&scene, b"scene-after").unwrap();

    assert_eq!(snapshot.files.len(), 1);
    assert_eq!(snapshot.files[0].bytes, b"scene-before");
}

#[test]
fn snapshot_refresh_then_reads_the_new_revision() {
    let fixture = Fixture::project("snapshot-new-revision");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    let scene = fixture.path().join("Scenes/Main.scene.json");
    fs::write(&scene, b"before").unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    let first = context
        .snapshot(
            handle,
            SnapshotRequest {
                relative_paths: vec!["Scenes/Main.scene.json".to_string()],
            },
        )
        .unwrap();

    fs::write(&scene, b"after").unwrap();
    context.refresh(handle).unwrap();
    let second = context
        .snapshot(
            handle,
            SnapshotRequest {
                relative_paths: vec!["Scenes/Main.scene.json".to_string()],
            },
        )
        .unwrap();

    assert_ne!(first.revision.revision_id, second.revision.revision_id);
    assert_eq!(second.files[0].bytes, b"after");
}

#[test]
fn snapshot_detects_live_source_drift_from_the_bound_revision() {
    let fixture = Fixture::project("snapshot-drift");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    let scene = fixture.path().join("Scenes/Main.scene.json");
    fs::write(&scene, b"before").unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    fs::write(&scene, b"after").unwrap();

    let error = context
        .snapshot(
            handle,
            SnapshotRequest {
                relative_paths: vec!["Scenes/Main.scene.json".to_string()],
            },
        )
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.snapshot_source_changed"
    );
}

#[test]
fn snapshot_rejects_excluded_missing_escaping_and_directory_paths() {
    let fixture = Fixture::project("snapshot-invalid-paths");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    let cases = [
        (
            "Library/cache.bin",
            "authoring_context.snapshot_path_excluded",
        ),
        (
            "Scenes/missing.json",
            "authoring_context.snapshot_path_missing",
        ),
        ("../outside", "authoring_context.snapshot_path_invalid"),
        ("Scenes", "authoring_context.snapshot_path_unsupported"),
    ];
    for (path, code) in cases {
        let error = context
            .snapshot(
                handle,
                SnapshotRequest {
                    relative_paths: vec![path.to_string()],
                },
            )
            .unwrap_err();
        assert_eq!(error.diagnostic.code, code, "path: {path}");
    }
}

#[test]
fn lease_keeps_r1_materialization_while_context_refreshes_to_r2() {
    let fixture = Fixture::project("lease-r1-r2");
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    let scene = fixture.path().join("Scenes/Main.scene.json");
    fs::write(&scene, b"r1").unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    let r1 = context
        .acquire_snapshot_lease(
            handle,
            SnapshotLeaseRequest {
                owner: "compiler.prepare".to_string(),
                snapshot: SnapshotRequest {
                    relative_paths: vec!["Scenes/Main.scene.json".to_string()],
                },
            },
        )
        .unwrap();

    fs::write(&scene, b"r2").unwrap();
    let refresh = context.refresh(handle).unwrap();
    let r2 = context
        .acquire_snapshot_lease(
            handle,
            SnapshotLeaseRequest {
                owner: "compiler.prepare".to_string(),
                snapshot: SnapshotRequest {
                    relative_paths: vec!["Scenes/Main.scene.json".to_string()],
                },
            },
        )
        .unwrap();

    assert_eq!(r1.snapshot().files[0].bytes, b"r1");
    assert_eq!(r2.snapshot().files[0].bytes, b"r2");
    assert_ne!(
        r1.snapshot().revision.revision_id,
        refresh.revision.revision_id
    );
    assert_eq!(r2.snapshot().revision, refresh.revision);
}

#[test]
fn lease_identity_retention_and_release_are_operation_bound() {
    let fixture = Fixture::project("lease-lifecycle");
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    let request = || SnapshotLeaseRequest {
        owner: "engine.check".to_string(),
        snapshot: SnapshotRequest {
            relative_paths: vec!["project.aife.json".to_string()],
        },
    };
    let first = context.acquire_snapshot_lease(handle, request()).unwrap();
    let second = context.acquire_snapshot_lease(handle, request()).unwrap();
    let first_id = first.lease_id();
    let first_snapshot_id = first.snapshot().snapshot_id.clone();
    let first_revision_id = first.snapshot().revision.revision_id.clone();

    assert_ne!(first_id, second.lease_id());
    assert_eq!(first.owner(), "engine.check");
    assert_eq!(
        first.retention_policy(),
        SnapshotRetentionPolicy::OperationBound
    );
    let report = first.release();
    assert_eq!(report.lease_id, first_id);
    assert_eq!(report.snapshot_id, first_snapshot_id);
    assert_eq!(report.revision_id, first_revision_id);
    assert_eq!(report.outcome, SnapshotReleaseOutcome::Released);
    drop(second);
}

#[test]
fn lease_acquire_fails_closed_when_live_source_drifted() {
    let fixture = Fixture::project("lease-drift");
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    fs::write(
        fixture.path().join("project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"changed"}"#,
    )
    .unwrap();

    let error = context
        .acquire_snapshot_lease(
            handle,
            SnapshotLeaseRequest {
                owner: "engine.check".to_string(),
                snapshot: SnapshotRequest {
                    relative_paths: vec!["project.aife.json".to_string()],
                },
            },
        )
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.snapshot_source_changed"
    );
}

#[test]
fn mutation_contract_accepts_bounded_declared_file_operations() {
    let mutation = ProjectMutation {
        schema_version: crate::PROJECT_MUTATION_SCHEMA_VERSION.to_string(),
        mutation_id: "mutation-contract-001".to_string(),
        domain: "controlled_source_patch".to_string(),
        expected_revision_id: format!("sha256:{}", "1".repeat(64)),
        validation_digest: format!("sha256:{}", "2".repeat(64)),
        declared_read_set: vec!["project.aife.json".to_string()],
        declared_write_set: vec![
            "Assets/old.txt".to_string(),
            "Assets/new.txt".to_string(),
            "Scenes/Main.scene.json".to_string(),
        ],
        expected_before: vec![
            ProjectMutationBeforeState {
                path: "Assets/old.txt".to_string(),
                content_digest: Some(format!("sha256:{}", "3".repeat(64))),
            },
            ProjectMutationBeforeState {
                path: "Assets/new.txt".to_string(),
                content_digest: None,
            },
            ProjectMutationBeforeState {
                path: "Scenes/Main.scene.json".to_string(),
                content_digest: Some(format!("sha256:{}", "4".repeat(64))),
            },
        ],
        operations: vec![
            ProjectMutationOperation::Move {
                from: "Assets/old.txt".to_string(),
                to: "Assets/new.txt".to_string(),
            },
            ProjectMutationOperation::CreateOrReplace {
                path: "Scenes/Main.scene.json".to_string(),
                bytes: br#"{"schemaVersion":"scene.v1"}"#.to_vec(),
            },
        ],
    };

    mutation.validate().unwrap();
}

#[test]
fn mutation_contract_rejects_undeclared_and_authority_control_writes() {
    let fixture = Fixture::project("mutation-contract-rejects");
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    let revision = context.refresh(handle).unwrap().revision;
    let mut mutation = ProjectMutation {
        schema_version: crate::PROJECT_MUTATION_SCHEMA_VERSION.to_string(),
        mutation_id: "mutation-contract-rejects".to_string(),
        domain: "test".to_string(),
        expected_revision_id: revision.revision_id,
        validation_digest: format!("sha256:{}", "5".repeat(64)),
        declared_read_set: Vec::new(),
        declared_write_set: vec!["Scenes/Declared.json".to_string()],
        expected_before: vec![ProjectMutationBeforeState {
            path: "Scenes/Declared.json".to_string(),
            content_digest: None,
        }],
        operations: vec![ProjectMutationOperation::CreateOrReplace {
            path: "Scenes/Other.json".to_string(),
            bytes: b"{}".to_vec(),
        }],
    };

    assert_eq!(
        mutation.validate().unwrap_err().diagnostic.code,
        "authoring_context.mutation_write_undeclared"
    );

    mutation.declared_write_set = vec![".aife/authoring/authority.lock".to_string()];
    mutation.expected_before = vec![ProjectMutationBeforeState {
        path: ".aife/authoring/authority.lock".to_string(),
        content_digest: None,
    }];
    mutation.operations = vec![ProjectMutationOperation::CreateOrReplace {
        path: ".aife/authoring/authority.lock".to_string(),
        bytes: b"take-over".to_vec(),
    }];
    assert_eq!(
        mutation.validate().unwrap_err().diagnostic.code,
        "authoring_context.mutation_control_path_rejected"
    );
}

#[test]
fn mutation_commit_writes_journal_receipt_and_publishes_after_revision() {
    let fixture = Fixture::project("mutation-commit");
    let (mut context, handle) = open_context(&fixture);
    let before = context.current_revision(handle).unwrap();
    let mutation = mutation_for(
        &context,
        handle,
        "mutation-commit-001",
        "Scenes/Main.scene.json",
        br#"{"schemaVersion":"scene.v1","name":"committed"}"#,
    );

    let receipt = context.commit_mutation(handle, mutation).unwrap();

    assert_eq!(receipt.before_revision, before);
    assert_ne!(receipt.after_revision.revision_id, before.revision_id);
    assert_eq!(
        fs::read(fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        br#"{"schemaVersion":"scene.v1","name":"committed"}"#
    );
    assert!(fixture.path().join(&receipt.journal_path).is_file());
    assert!(fixture.path().join(&receipt.receipt_path).is_file());
}

#[test]
fn mutation_commit_stale_expected_revision_fails_before_writing() {
    let fixture = Fixture::project("mutation-stale");
    let (mut context, handle) = open_context(&fixture);
    let mutation = mutation_for(
        &context,
        handle,
        "mutation-stale-001",
        "Scenes/Main.scene.json",
        b"candidate",
    );
    fs::create_dir_all(fixture.path().join("Settings")).unwrap();
    fs::write(fixture.path().join("Settings/external.json"), b"external").unwrap();

    let error = context.commit_mutation(handle, mutation).unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.mutation_revision_drifted"
    );
    assert!(!fixture.path().join("Scenes/Main.scene.json").exists());
}

#[test]
fn mutation_unknown_write_outside_write_set_compensates_only_own_bytes() {
    let fixture = Fixture::project("mutation-unknown-write");
    let (mut context, handle) = open_context(&fixture);
    let mutation = mutation_for(
        &context,
        handle,
        "mutation-unknown-write-001",
        "Scenes/Main.scene.json",
        b"owned",
    );
    let external_path = fixture.path().join("Settings/external.json");

    let error = context
        .commit_mutation_controlled(handle, mutation, |progress| {
            if matches!(progress, MutationCommitProgress::BeforeVerification) {
                fs::create_dir_all(external_path.parent().unwrap()).unwrap();
                fs::write(&external_path, b"external").unwrap();
            }
            Ok(())
        })
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.mutation_unknown_external_write"
    );
    assert!(!fixture.path().join("Scenes/Main.scene.json").exists());
    assert_eq!(fs::read(external_path).unwrap(), b"external");
}

#[test]
fn mutation_unknown_same_path_bytes_require_recovery_and_block_next_commit() {
    let fixture = Fixture::project("mutation-same-path-write");
    let (mut context, handle) = open_context(&fixture);
    let target = fixture.path().join("Scenes/Main.scene.json");
    let first = mutation_for(
        &context,
        handle,
        "mutation-same-path-write-001",
        "Scenes/Main.scene.json",
        b"owned",
    );

    let error = context
        .commit_mutation_controlled(handle, first, |progress| {
            if matches!(progress, MutationCommitProgress::BeforeVerification) {
                fs::write(&target, b"external-overlap").unwrap();
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "authoring_context.mutation_recovery_required"
    );
    assert_eq!(fs::read(&target).unwrap(), b"external-overlap");

    context.refresh(handle).unwrap();
    let second = mutation_for(
        &context,
        handle,
        "mutation-same-path-write-002",
        "Scenes/Other.scene.json",
        b"second",
    );
    let blocked = context.commit_mutation(handle, second).unwrap_err();
    assert_eq!(
        blocked.diagnostic.code,
        "authoring_context.mutation_recovery_blocked"
    );
}

#[test]
fn mutation_os_lock_is_cross_process_and_lifecycle_bound() {
    const CHILD_ROOT: &str = "AIFE_MUTATION_LOCK_CHILD_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        let _lock = crate::authority_lock::ProjectAuthorityLock::acquire(&root, "child").unwrap();
        fs::write(root.join(".aife/authoring/child-ready"), b"ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !root.join(".aife/authoring/child-release").exists() {
            assert!(
                Instant::now() < deadline,
                "parent did not release child lock test"
            );
            thread::sleep(Duration::from_millis(10));
        }
        return;
    }

    let fixture = Fixture::project("mutation-os-lock");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("tests::mutation_os_lock_is_cross_process_and_lifecycle_bound")
        .arg("--nocapture")
        .env(CHILD_ROOT, fixture.path())
        .spawn()
        .unwrap();
    let ready = fixture.path().join(".aife/authoring/child-ready");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(
            Instant::now() < deadline,
            "child did not acquire mutation lock"
        );
        thread::sleep(Duration::from_millis(10));
    }

    let error = crate::authority_lock::ProjectAuthorityLock::acquire(fixture.path(), "parent")
        .err()
        .expect("the second process must not acquire the same project authority");
    assert_eq!(
        error.diagnostic.code,
        "authoring_context.mutation_authority_busy"
    );
    fs::write(
        fixture.path().join(".aife/authoring/child-release"),
        b"release",
    )
    .unwrap();
    assert!(child.wait().unwrap().success());

    crate::authority_lock::ProjectAuthorityLock::acquire(fixture.path(), "parent-after-exit")
        .unwrap();
}

#[test]
fn mutation_commit_concurrent_contexts_publish_at_most_one_winner() {
    let fixture = Fixture::project("mutation-concurrent-contexts");
    let (context_a, handle_a) = open_context(&fixture);
    let (context_b, handle_b) = open_context(&fixture);
    let mutation_a = mutation_for(
        &context_a,
        handle_a,
        "mutation-concurrent-a",
        "Scenes/Winner.scene.json",
        b"a",
    );
    let mut mutation_b = mutation_for(
        &context_b,
        handle_b,
        "mutation-concurrent-b",
        "Scenes/Winner.scene.json",
        b"b",
    );
    mutation_b.expected_revision_id = mutation_a.expected_revision_id.clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let barrier_a = barrier.clone();
    let worker_a = thread::spawn(move || {
        let mut context = context_a;
        barrier_a.wait();
        context.commit_mutation(handle_a, mutation_a)
    });
    let barrier_b = barrier.clone();
    let worker_b = thread::spawn(move || {
        let mut context = context_b;
        barrier_b.wait();
        context.commit_mutation(handle_b, mutation_b)
    });
    barrier.wait();
    let results = [worker_a.join().unwrap(), worker_b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let loser = results
        .iter()
        .find_map(|result| result.as_ref().err())
        .unwrap();
    assert!(matches!(
        loser.diagnostic.code.as_str(),
        "authoring_context.mutation_authority_busy" | "authoring_context.mutation_revision_drifted"
    ));
    let winner = fs::read(fixture.path().join("Scenes/Winner.scene.json")).unwrap();
    assert!(winner == b"a" || winner == b"b");
}

#[test]
fn mutation_rollback_restores_exact_before_revision() {
    let fixture = Fixture::project("mutation-rollback");
    let (mut context, handle) = open_context(&fixture);
    let before = context.current_revision(handle).unwrap();
    let mutation = mutation_for(
        &context,
        handle,
        "mutation-rollback-001",
        "Scenes/Main.scene.json",
        b"applied",
    );
    let receipt = context.commit_mutation(handle, mutation).unwrap();

    let rollback = context.rollback_mutation(handle, &receipt).unwrap();

    assert_eq!(rollback.restored_revision, before);
    assert_eq!(rollback.replaced_revision, receipt.after_revision);
    assert!(!fixture.path().join("Scenes/Main.scene.json").exists());
    assert!(fixture
        .path()
        .join(&rollback.rollback_receipt_path)
        .is_file());
}

#[test]
fn mutation_rollback_rejects_intervening_revision_and_receipt_tamper() {
    let drift_fixture = Fixture::project("mutation-rollback-drift");
    let (mut drift_context, drift_handle) = open_context(&drift_fixture);
    let mutation = mutation_for(
        &drift_context,
        drift_handle,
        "mutation-rollback-drift-001",
        "Scenes/Main.scene.json",
        b"applied",
    );
    let receipt = drift_context
        .commit_mutation(drift_handle, mutation)
        .unwrap();
    fs::create_dir_all(drift_fixture.path().join("Settings")).unwrap();
    fs::write(
        drift_fixture.path().join("Settings/intervening.json"),
        b"external",
    )
    .unwrap();
    let error = drift_context
        .rollback_mutation(drift_handle, &receipt)
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "authoring_context.rollback_revision_drifted"
    );
    assert_eq!(
        fs::read(drift_fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"applied"
    );

    let tamper_fixture = Fixture::project("mutation-rollback-receipt-tamper");
    let (mut tamper_context, tamper_handle) = open_context(&tamper_fixture);
    let mutation = mutation_for(
        &tamper_context,
        tamper_handle,
        "mutation-rollback-tamper-001",
        "Scenes/Main.scene.json",
        b"applied",
    );
    let mut tampered = tamper_context
        .commit_mutation(tamper_handle, mutation)
        .unwrap();
    tampered.validation_digest = format!("sha256:{}", "7".repeat(64));
    let error = tamper_context
        .rollback_mutation(tamper_handle, &tampered)
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "authoring_context.rollback_receipt_invalid"
    );
}

#[test]
fn recovery_open_reports_no_recovery_for_clean_project() {
    let fixture = Fixture::project("recovery-open-clean");
    let mut context = EmbeddedAuthoringProjectContext::new();

    let report = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    assert!(report.recovery.transactions.is_empty());
    assert_eq!(
        report.recovery.disposition,
        ProjectRecoveryDisposition::NoRecovery
    );
    assert!(context.current_revision(report.handle).is_ok());
}

#[test]
fn recovery_open_acquires_mutation_authority_before_publishing_handle() {
    let fixture = Fixture::project("recovery-open-lock");
    let _authority =
        crate::authority_lock::ProjectAuthorityLock::acquire(fixture.path(), "test-holder")
            .unwrap();
    let mut context = EmbeddedAuthoringProjectContext::new();

    let error = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.mutation_authority_busy"
    );
}

#[test]
fn recovery_classification_child_checkpoint() {
    const CHILD_ROOT: &str = "AIFE_RECOVERY_CRASH_ROOT";
    const CHECKPOINT: &str = "AIFE_RECOVERY_CRASH_CHECKPOINT";
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let checkpoint = std::env::var(CHECKPOINT).unwrap();
    let root = PathBuf::from(root);
    if checkpoint == "during_recovery_after_restore" {
        let canonical_root = root.canonicalize().unwrap();
        crate::mutation::recover_project_controlled(&canonical_root, |_| std::process::exit(74))
            .unwrap();
        panic!("recovery crash child did not restore a path");
    }
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(&root), OpenOptions)
        .unwrap();
    let paths = if checkpoint == "after_one_operation" {
        vec![
            "Scenes/First.scene.json".to_string(),
            "Scenes/Second.scene.json".to_string(),
        ]
    } else if checkpoint == "after_two_operations" {
        vec![
            "Scenes/First.scene.json".to_string(),
            "Scenes/Second.scene.json".to_string(),
            "Scenes/Third.scene.json".to_string(),
        ]
    } else {
        vec!["Scenes/Main.scene.json".to_string()]
    };
    let before = context.capture_mutation_before(handle, &paths).unwrap();
    let operations = paths
        .iter()
        .map(|path| ProjectMutationOperation::CreateOrReplace {
            path: path.clone(),
            bytes: format!("after:{path}").into_bytes(),
        })
        .collect();
    let mutation = ProjectMutation {
        schema_version: crate::PROJECT_MUTATION_SCHEMA_VERSION.to_string(),
        mutation_id: format!("recovery-crash-{checkpoint}"),
        domain: "test".to_string(),
        expected_revision_id: context.current_revision(handle).unwrap().revision_id,
        validation_digest: format!("sha256:{}", "8".repeat(64)),
        declared_read_set: Vec::new(),
        declared_write_set: paths,
        expected_before: before,
        operations,
    };
    let target = root.join("Scenes/Main.scene.json");
    let external = root.join("Settings/external.json");
    let _ = context.commit_mutation_controlled(handle, mutation, |progress| {
        let should_exit = match checkpoint.as_str() {
            "after_prepared" => matches!(progress, MutationCommitProgress::AfterPrepared),
            "before_apply" => matches!(progress, MutationCommitProgress::BeforeApply),
            "after_one_operation" => matches!(
                progress,
                MutationCommitProgress::AfterOperation { index: 0, .. }
            ),
            "after_two_operations" => matches!(
                progress,
                MutationCommitProgress::AfterOperation { index: 1, .. }
            ),
            "before_verification" => {
                matches!(progress, MutationCommitProgress::BeforeVerification)
            }
            "before_verification_external" => {
                if matches!(progress, MutationCommitProgress::BeforeVerification) {
                    fs::create_dir_all(external.parent().unwrap()).unwrap();
                    fs::write(&external, b"external").unwrap();
                    true
                } else {
                    false
                }
            }
            "before_verification_unknown" => {
                if matches!(progress, MutationCommitProgress::BeforeVerification) {
                    fs::write(&target, b"unknown-overlap").unwrap();
                    true
                } else {
                    false
                }
            }
            "after_journal_committed" => {
                matches!(progress, MutationCommitProgress::AfterJournalCommitted)
            }
            _ => panic!("unknown recovery crash checkpoint: {checkpoint}"),
        };
        if should_exit {
            std::process::exit(73);
        }
        Ok(())
    });
    panic!("recovery crash child did not reach checkpoint {checkpoint}");
}

#[test]
fn recovery_classification_all_before_becomes_aborted_and_is_idempotent() {
    let fixture = Fixture::project("recovery-classification-before");
    let before_revision = project_revision(fixture.path());
    spawn_recovery_crash(fixture.path(), "after_prepared");

    let mut context = EmbeddedAuthoringProjectContext::new();
    let report = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    assert_eq!(
        report.recovery.disposition,
        ProjectRecoveryDisposition::Aborted
    );
    assert_eq!(report.recovery.transactions.len(), 1);
    assert_eq!(report.recovery.transactions[0].prior_state, "prepared");
    assert_eq!(
        context.current_revision(report.handle).unwrap(),
        before_revision
    );

    let second = EmbeddedAuthoringProjectContext::new()
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    assert_eq!(
        second.recovery.disposition,
        ProjectRecoveryDisposition::NoRecovery
    );
}

#[test]
fn recovery_classification_partial_apply_restores_exact_before() {
    let fixture = Fixture::project("recovery-classification-partial");
    let before_revision = project_revision(fixture.path());
    spawn_recovery_crash(fixture.path(), "after_one_operation");

    let mut context = EmbeddedAuthoringProjectContext::new();
    let report = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    assert_eq!(
        report.recovery.disposition,
        ProjectRecoveryDisposition::RestoredBefore
    );
    assert!(!fixture.path().join("Scenes/First.scene.json").exists());
    assert!(!fixture.path().join("Scenes/Second.scene.json").exists());
    assert_eq!(
        context.current_revision(report.handle).unwrap(),
        before_revision
    );
}

#[test]
fn recovery_classification_all_after_commits_without_external_drift() {
    let fixture = Fixture::project("recovery-classification-after");
    spawn_recovery_crash(fixture.path(), "before_verification");

    let mut context = EmbeddedAuthoringProjectContext::new();
    let report = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    assert_eq!(
        report.recovery.disposition,
        ProjectRecoveryDisposition::Committed
    );
    let transaction = &report.recovery.transactions[0];
    let receipt = context
        .mutation_receipt_for_journal(report.handle, &transaction.journal_path)
        .unwrap();
    assert!(fixture.path().join(&receipt.receipt_path).is_file());
    assert_eq!(
        fs::read(fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"after:Scenes/Main.scene.json"
    );
}

#[test]
fn recovery_classification_all_after_with_external_drift_restores_owned_paths() {
    let fixture = Fixture::project("recovery-classification-after-drift");
    spawn_recovery_crash(fixture.path(), "before_verification_external");

    let mut context = EmbeddedAuthoringProjectContext::new();
    let report = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    assert_eq!(
        report.recovery.disposition,
        ProjectRecoveryDisposition::RestoredBefore
    );
    assert!(!fixture.path().join("Scenes/Main.scene.json").exists());
    assert_eq!(
        fs::read(fixture.path().join("Settings/external.json")).unwrap(),
        b"external"
    );
}

#[test]
fn recovery_crash_all_durable_checkpoints_converge_and_second_open_is_stable() {
    let cases = [
        ("after_prepared", ProjectRecoveryDisposition::Aborted),
        ("before_apply", ProjectRecoveryDisposition::Aborted),
        (
            "after_one_operation",
            ProjectRecoveryDisposition::RestoredBefore,
        ),
        ("before_verification", ProjectRecoveryDisposition::Committed),
        (
            "after_journal_committed",
            ProjectRecoveryDisposition::Committed,
        ),
    ];

    for (checkpoint, expected) in cases {
        let fixture = Fixture::project(&format!("recovery-crash-{checkpoint}"));
        spawn_recovery_crash(fixture.path(), checkpoint);
        let mut first_context = EmbeddedAuthoringProjectContext::new();
        let first = first_context
            .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
            .unwrap();
        assert_eq!(first.recovery.disposition, expected, "{checkpoint}");
        let first_revision = first_context.current_revision(first.handle).unwrap();
        let journal_path = first.recovery.transactions[0].journal_path.clone();
        let journal_bytes = fs::read(fixture.path().join(&journal_path)).unwrap();
        let receipt_bytes = if expected == ProjectRecoveryDisposition::Committed {
            let receipt = first_context
                .mutation_receipt_for_journal(first.handle, &journal_path)
                .unwrap();
            Some((
                receipt.receipt_path.clone(),
                fs::read(fixture.path().join(&receipt.receipt_path)).unwrap(),
            ))
        } else {
            None
        };

        let mut second_context = EmbeddedAuthoringProjectContext::new();
        let second = second_context
            .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
            .unwrap();
        assert_eq!(
            second.recovery.disposition,
            ProjectRecoveryDisposition::NoRecovery,
            "{checkpoint}"
        );
        assert_eq!(
            second_context.current_revision(second.handle).unwrap(),
            first_revision,
            "{checkpoint}"
        );
        assert_eq!(
            fs::read(fixture.path().join(&journal_path)).unwrap(),
            journal_bytes,
            "{checkpoint}"
        );
        if let Some((receipt_path, bytes)) = receipt_bytes {
            assert_eq!(
                fs::read(fixture.path().join(receipt_path)).unwrap(),
                bytes,
                "{checkpoint}"
            );
        }
    }
}

#[test]
fn recovery_fail_closed_unknown_touched_bytes_are_preserved_and_block_mutation() {
    let fixture = Fixture::project("recovery-fail-closed-unknown");
    let (mut existing_context, existing_handle) = open_context(&fixture);
    let stable = mutation_for(
        &existing_context,
        existing_handle,
        "recovery-stable-before-unknown",
        "Assets/Stable.txt",
        b"stable",
    );
    let stable_receipt = existing_context
        .commit_mutation(existing_handle, stable)
        .unwrap();
    spawn_recovery_crash(fixture.path(), "before_verification_unknown");

    let mut reopening = EmbeddedAuthoringProjectContext::new();
    let error = reopening
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();
    assert_eq!(error.diagnostic.stage, crate::DiagnosticStage::Recovery);
    assert_eq!(error.diagnostic.code, "authoring_context.recovery_required");
    assert_eq!(
        fs::read(fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"unknown-overlap"
    );

    let rollback = existing_context
        .rollback_mutation(existing_handle, &stable_receipt)
        .unwrap_err();
    assert_eq!(
        rollback.diagnostic.code,
        "authoring_context.mutation_recovery_blocked"
    );
    let next = mutation_for(
        &existing_context,
        existing_handle,
        "recovery-blocked-next",
        "Scenes/Next.scene.json",
        b"next",
    );
    let commit = existing_context
        .commit_mutation(existing_handle, next)
        .unwrap_err();
    assert_eq!(
        commit.diagnostic.code,
        "authoring_context.mutation_recovery_blocked"
    );
}

#[test]
fn recovery_fail_closed_journal_material_and_receipt_tamper_do_not_write_source() {
    let journal_fixture = Fixture::project("recovery-fail-closed-journal");
    spawn_recovery_crash(journal_fixture.path(), "after_prepared");
    let journal_path = journal_fixture
        .path()
        .join(".aife/authoring/transactions/recovery-crash-after_prepared/journal.json");
    let journal = fs::read_to_string(&journal_path).unwrap();
    fs::write(
        &journal_path,
        journal.replace("\"prepared\"", "\"applying\""),
    )
    .unwrap();
    let error = EmbeddedAuthoringProjectContext::new()
        .open_with_report(ProjectLocator::new(journal_fixture.path()), OpenOptions)
        .unwrap_err();
    assert_eq!(error.diagnostic.stage, crate::DiagnosticStage::Recovery);
    assert!(!journal_fixture
        .path()
        .join("Scenes/Main.scene.json")
        .exists());

    let material_fixture = Fixture::project("recovery-fail-closed-material");
    fs::create_dir_all(material_fixture.path().join("Scenes")).unwrap();
    fs::write(
        material_fixture.path().join("Scenes/Main.scene.json"),
        b"before-material",
    )
    .unwrap();
    spawn_recovery_crash(material_fixture.path(), "after_prepared");
    let material_path = material_fixture
        .path()
        .join(".aife/authoring/transactions/recovery-crash-after_prepared/before/0000.bin");
    fs::write(&material_path, b"tampered-material").unwrap();
    let error = EmbeddedAuthoringProjectContext::new()
        .open_with_report(ProjectLocator::new(material_fixture.path()), OpenOptions)
        .unwrap_err();
    assert_eq!(error.diagnostic.stage, crate::DiagnosticStage::Recovery);
    assert_eq!(
        fs::read(material_fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"before-material"
    );

    let receipt_fixture = Fixture::project("recovery-fail-closed-receipt");
    let (mut context, handle) = open_context(&receipt_fixture);
    let mutation = mutation_for(
        &context,
        handle,
        "recovery-receipt-tamper",
        "Scenes/Main.scene.json",
        b"committed-source",
    );
    let receipt = context.commit_mutation(handle, mutation).unwrap();
    let receipt_path = receipt_fixture.path().join(&receipt.receipt_path);
    let bytes = fs::read_to_string(&receipt_path).unwrap();
    fs::write(&receipt_path, bytes.replace("\"test\"", "\"tampered\"")).unwrap();
    let error = EmbeddedAuthoringProjectContext::new()
        .open_with_report(ProjectLocator::new(receipt_fixture.path()), OpenOptions)
        .unwrap_err();
    assert_eq!(error.diagnostic.stage, crate::DiagnosticStage::Recovery);
    assert_eq!(
        fs::read(receipt_fixture.path().join("Scenes/Main.scene.json")).unwrap(),
        b"committed-source"
    );
}

#[test]
fn recovery_fail_closed_conflicting_incomplete_journals_are_not_ordered_by_guess() {
    let fixture = Fixture::project("recovery-fail-closed-conflict");
    spawn_recovery_crash(fixture.path(), "after_prepared");
    crate::mutation::clone_incomplete_journal_for_test(
        fixture.path(),
        "recovery-crash-after_prepared",
        "recovery-crash-conflicting-copy",
    )
    .unwrap();

    let error = EmbeddedAuthoringProjectContext::new()
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap_err();

    assert_eq!(
        error.diagnostic.code,
        "authoring_context.recovery_journals_conflict"
    );
    assert!(!fixture.path().join("Scenes/Main.scene.json").exists());
}

#[test]
fn recovery_fail_closed_recovery_crash_is_idempotent_on_next_open() {
    let fixture = Fixture::project("recovery-fail-closed-recovery-crash");
    spawn_recovery_crash(fixture.path(), "after_two_operations");
    spawn_recovery_process_crash(fixture.path());

    let mut context = EmbeddedAuthoringProjectContext::new();
    let report = context
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();

    assert_eq!(
        report.recovery.disposition,
        ProjectRecoveryDisposition::RestoredBefore
    );
    for name in ["First", "Second", "Third"] {
        assert!(!fixture
            .path()
            .join(format!("Scenes/{name}.scene.json"))
            .exists());
    }
    let second = EmbeddedAuthoringProjectContext::new()
        .open_with_report(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    assert_eq!(
        second.recovery.disposition,
        ProjectRecoveryDisposition::NoRecovery
    );
}

fn spawn_recovery_crash(project_root: &Path, checkpoint: &str) {
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("tests::recovery_classification_child_checkpoint")
        .arg("--nocapture")
        .env("AIFE_RECOVERY_CRASH_ROOT", project_root)
        .env("AIFE_RECOVERY_CRASH_CHECKPOINT", checkpoint)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
}

fn spawn_recovery_process_crash(project_root: &Path) {
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("tests::recovery_classification_child_checkpoint")
        .arg("--nocapture")
        .env("AIFE_RECOVERY_CRASH_ROOT", project_root)
        .env(
            "AIFE_RECOVERY_CRASH_CHECKPOINT",
            "during_recovery_after_restore",
        )
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(74));
}

fn project_revision(project_root: &Path) -> crate::ProjectRevision {
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(project_root), OpenOptions)
        .unwrap();
    context.current_revision(handle).unwrap()
}

fn open_context(fixture: &Fixture) -> (EmbeddedAuthoringProjectContext, crate::ProjectHandle) {
    let mut context = EmbeddedAuthoringProjectContext::new();
    let handle = context
        .open(ProjectLocator::new(fixture.path()), OpenOptions)
        .unwrap();
    (context, handle)
}

fn mutation_for(
    context: &EmbeddedAuthoringProjectContext,
    handle: crate::ProjectHandle,
    mutation_id: &str,
    path: &str,
    bytes: &[u8],
) -> ProjectMutation {
    let before = context
        .capture_mutation_before(handle, &[path.to_string()])
        .unwrap();
    ProjectMutation {
        schema_version: crate::PROJECT_MUTATION_SCHEMA_VERSION.to_string(),
        mutation_id: mutation_id.to_string(),
        domain: "test".to_string(),
        expected_revision_id: context.current_revision(handle).unwrap().revision_id,
        validation_digest: format!("sha256:{}", "6".repeat(64)),
        declared_read_set: Vec::new(),
        declared_write_set: vec![path.to_string()],
        expected_before: before,
        operations: vec![ProjectMutationOperation::CreateOrReplace {
            path: path.to_string(),
            bytes: bytes.to_vec(),
        }],
    }
}

fn document_request(path: &str, bytes: &[u8]) -> DocumentWriteRequest {
    DocumentWriteRequest {
        relative_path: path.to_string(),
        domain: "scene".to_string(),
        schema_version: "editor-scene-document.v1".to_string(),
        bytes: bytes.to_vec(),
    }
}

struct Fixture {
    path: PathBuf,
    cleanup_root: Option<PathBuf>,
}

impl Fixture {
    fn project(label: &str) -> Self {
        Self::with_manifest(
            label,
            br#"{"schemaVersion":"aife-project.v2","projectId":"fixture.project"}"#,
        )
    }

    fn with_manifest(label: &str, manifest: &[u8]) -> Self {
        let root = unique_temp_path(label);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("project.aife.json"), manifest).unwrap();
        Self {
            path: root.clone(),
            cleanup_root: Some(root),
        }
    }

    fn missing(label: &str) -> Self {
        let root = unique_temp_path(label);
        Self {
            path: root.clone(),
            cleanup_root: Some(root),
        }
    }

    fn file(label: &str) -> Self {
        let root = unique_temp_path(label);
        fs::write(&root, b"not a directory").unwrap();
        Self {
            path: root.clone(),
            cleanup_root: Some(root),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let Some(root) = self.cleanup_root.take() else {
            return;
        };
        if root.is_dir() {
            let _ = fs::remove_dir_all(root);
        } else if root.is_file() {
            let _ = fs::remove_file(root);
        }
    }
}

fn unique_temp_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("aife-authoring-context-{label}-{stamp}"))
}
