use quality_gate::architecture_policy::{diff_policy, parse_policy, PolicyMode};

const POLICY: &str = r#"
schema_version = "architecture-policy.v1"
review_authorities = ["local-maintainer"]

[[profiles]]
id = "engine"
mode = "engine_strict"
include = ["crates/**"]
exclude = ["target/**"]

[[profiles]]
id = "project-advisory"
mode = "project_advisory"
include = ["project_modules/**"]

[[profiles]]
id = "project-strict"
mode = "project_strict"
include = ["project_modules/**"]

[[domains]]
id = "tooling"
owner = "engine-quality"
include = ["crates/quality_gate/**"]
facade_only = []
"#;

#[test]
fn committed_shape_parses_strictly() {
    let policy = parse_policy(POLICY).expect("valid policy");
    assert_eq!(policy.profiles[0].mode, PolicyMode::EngineStrict);
}

#[test]
fn policy_relaxation_is_review_required() {
    let previous = parse_policy(POLICY).unwrap();
    let mut candidate = previous.clone();
    candidate.profiles[0]
        .exclude
        .push("crates/editor_core/**".to_string());
    let diff = diff_policy(&previous, &candidate);
    assert!(diff.requires_review());
}

#[test]
fn unknown_schema_fails_closed() {
    let invalid = POLICY.replace("architecture-policy.v1", "architecture-policy.v9");
    let error = parse_policy(&invalid).unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "architecture_policy.unknown_schema"));
}
