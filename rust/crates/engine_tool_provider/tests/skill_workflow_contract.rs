use std::fs;
use std::path::PathBuf;

#[test]
fn engine_authoring_skill_has_the_frozen_static_workflow_contract() {
    let skill = repository_root()
        .join("ai-development-pack")
        .join("skills")
        .join("ai-first-game-engine")
        .join("SKILL.md");
    let text = fs::read_to_string(&skill).unwrap_or_else(|error| {
        panic!(
            "read product Engine authoring Skill {}: {error}",
            skill.display()
        )
    });

    assert!(text.starts_with("---\n"));
    assert!(text.contains("\nname: ai-first-game-engine\n"));
    assert!(text.contains("\ndescription:"));
    for required in [
        "Host native file tools",
        "engine_project_inspect",
        "engine_project_mutate",
        "engine_project_rollback",
        "engine_runtime_run",
        "engine_project_build",
        "engine_delivery_verify",
        "engine_project_check",
        "engine_runtime_playtest",
        "engine_runtime_observe",
        "retryability",
        "recommendedLocalTransitions",
        "diagnostics",
        "receiptRef",
        "evidenceRefs",
    ] {
        assert!(
            text.contains(required),
            "Skill is missing frozen token: {required}"
        );
    }
    assert!(text.contains("deferred"));
    let qualified = text
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    assert_eq!(
        qualified.lines().collect::<Vec<_>>(),
        [
            "engine_project_inspect",
            "engine_project_check",
            "engine_project_mutate",
            "engine_project_rollback",
            "engine_runtime_run",
            "engine_project_build",
            "engine_runtime_playtest",
            "engine_runtime_observe",
            "engine_delivery_verify",
        ]
    );
    assert!(text.contains("it may be skipped when run/build"));
    for contract in [
        "Ordinary run/build do not require a playtest scenario or an explicit check.",
        "Observe reads that run without execution, fresh input, or a latest-run fallback.",
        "skip it when the playtest",
        "successful reading does not mean the game passed.",
        "Do not change a correct expectation merely to make a failing test green.",
        "Do not alias tools or claim deferred capabilities are Ready.",
    ] {
        assert!(
            text.contains(contract),
            "missing conditional contract: {contract}"
        );
    }
    assert!(!text.contains("`engine_project_check`, `engine_runtime_playtest`"));
    assert!(!text.contains("engine_catalog"));
    assert!(!text.contains("engine_execute"));
    assert!(!skill.parent().unwrap().join("scripts").exists());
}

#[test]
fn skill_read_execution_and_deferred_choices_match_real_provider_behavior() {
    use engine_tool_provider::{
        CanonicalToolStatus, HostSessionContext, HostToolCall, NativeHostAdapter, ToolSideEffect,
    };
    use serde_json::json;
    let root = std::env::temp_dir().join(format!(
        "aife-skill-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"skill.contract"}"#,
    )
    .unwrap();
    let mut adapter = NativeHostAdapter::attach(HostSessionContext {
        session_id: "skill-behavior".into(),
        workspace_root: root.clone(),
        project_root: Some(root.clone()),
    })
    .unwrap();
    let definitions = adapter.tool_definitions();
    let playtest = definitions
        .iter()
        .find(|d| d.name == "engine_runtime_playtest")
        .unwrap();
    let observe = definitions
        .iter()
        .find(|d| d.name == "engine_runtime_observe")
        .unwrap();
    assert_eq!(playtest.side_effect, ToolSideEffect::ProcessSpawn);
    assert_eq!(observe.side_effect, ToolSideEffect::Read);
    for (id, tool, args, expected) in [
        (
            "inspect",
            "engine_project_inspect",
            json!({}),
            CanonicalToolStatus::Completed,
        ),
        (
            "denied",
            "engine_runtime_playtest",
            json!({}),
            CanonicalToolStatus::RejectedByHost,
        ),
        (
            "unknown",
            "engine_runtime_observe",
            json!({"runRef":"unretained"}),
            CanonicalToolStatus::RejectedByEngine,
        ),
        (
            "deferred",
            "engine_runtime_capture_issue",
            json!({}),
            CanonicalToolStatus::RejectedByEngine,
        ),
    ] {
        let result = adapter.invoke(HostToolCall {
            call_id: id.into(),
            tool_name: tool.into(),
            arguments: args,
            approved: false,
        });
        assert_eq!(result.status, expected);
        assert!(result.evidence_refs.is_empty());
    }
    assert!(
        !root.join("Library").exists(),
        "read/denied/deferred choices must not execute a Player or prepare/build"
    );
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}
