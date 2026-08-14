use super::*;

fn goal() -> AiGoalBinding {
    AiGoalBinding::new(
        "goal-fix-hud",
        "Restore the missing HUD and keep the verified project change.",
        "project-1",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        AiGoalCompletionPolicy::CommitVerified,
    )
    .expect("valid goal binding")
}

fn low_risk() -> AiRiskEnvelope {
    AiRiskEnvelope::project_owned_low_risk(
        vec!["project://assets/ui".to_string()],
        vec!["project://target".to_string()],
        Vec::new(),
        4,
        600_000,
        0,
    )
    .expect("valid risk envelope")
}

#[test]
fn ai_goal_grant_binding_and_risk_are_canonical_and_strict() {
    let goal = goal();
    let risk = low_risk();

    goal.validate_integrity().unwrap();
    risk.validate_integrity().unwrap();
    assert!(goal.binding_digest.starts_with("sha256:"));
    assert!(risk.envelope_digest.starts_with("sha256:"));
    assert_eq!(risk.max_mutation_count, 4);
    assert!(!risk.allow_delete);
    assert!(!risk.allow_dependency_change);
    assert!(!risk.allow_network);

    let mut tampered = goal.clone();
    tampered.user_visible_outcome.push_str(" changed");
    assert_eq!(
        tampered.validate_integrity().unwrap_err().code,
        "ai_goal.binding_digest_mismatch"
    );

    let encoded = serde_json::to_value(&risk).unwrap();
    let mut with_unknown = encoded.as_object().unwrap().clone();
    with_unknown.insert(
        "toolOrder".to_string(),
        serde_json::json!(["inspect", "mutate"]),
    );
    assert!(
        serde_json::from_value::<AiRiskEnvelope>(serde_json::Value::Object(with_unknown)).is_err()
    );
}

#[test]
fn ai_goal_grant_low_risk_rejects_escalation_and_does_not_default_to_one_mutation() {
    let risk = AiRiskEnvelope::new(AiRiskEnvelopeSpec {
        risk_class: AiGoalRiskClass::ProjectOwnedLowRisk,
        allowed_paths: Vec::new(),
        denied_paths: Vec::new(),
        allowed_objects: Vec::new(),
        max_mutation_count: 1,
        time_budget_ms: 600_000,
        external_cost_budget_microunits: 0,
        allow_delete: true,
        allow_dependency_change: false,
        allow_network: false,
    })
    .unwrap_err();
    assert_eq!(risk.code, "ai_goal.low_risk_escalated");

    let risk = AiRiskEnvelope::default_project_owned_low_risk().unwrap();
    assert_eq!(risk.max_mutation_count, 16);
}

#[test]
fn ai_goal_grant_spec_binds_goal_risk_and_project_session() {
    let spec = AiGoalGrantSpec::new(
        goal(),
        low_risk(),
        "gateway-session-1",
        "native-editor-user",
        Some(1_000_000),
    )
    .expect("valid goal grant spec");

    spec.validate_integrity().unwrap();
    assert!(spec.approval_digest.starts_with("sha256:"));
    assert_eq!(spec.goal_binding.project_identity, "project-1");
    assert_eq!(spec.risk_envelope.max_mutation_count, 4);

    let mut tampered = spec;
    tampered.client_session_id = "gateway-session-2".to_string();
    assert_eq!(
        tampered.validate_integrity().unwrap_err().code,
        "ai_goal.approval_digest_mismatch"
    );
}

#[test]
fn ai_goal_grant_capability_uses_approved_goal_and_risk_budgets() {
    let expires_at_epoch_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 60_000;
    let spec = AiGoalGrantSpec::new(
        goal(),
        low_risk(),
        "gateway-session-1",
        "native-editor-user",
        Some(expires_at_epoch_ms),
    )
    .unwrap();
    let grant = AiCapabilityGrant::project_owned_low_risk_for_goal(spec).unwrap();

    grant.validate_integrity().unwrap();
    assert_eq!(grant.max_mutation_count, 4);
    assert_eq!(grant.time_budget_ms, 600_000);
    assert_eq!(grant.expires_at_epoch_ms, Some(expires_at_epoch_ms));
    assert!(grant.allowed_domains.contains(&"build".to_string()));
    assert_eq!(grant.goal_binding.as_ref().unwrap().goal_id, "goal-fix-hud");
    assert_eq!(grant.risk_envelope.as_ref().unwrap().max_mutation_count, 4);
}

#[test]
fn ai_goal_grant_elevated_capability_preserves_approved_scope_and_integrity() {
    let risk = AiRiskEnvelope::new(AiRiskEnvelopeSpec {
        risk_class: AiGoalRiskClass::Elevated,
        allowed_paths: vec!["project://assets/input".to_string()],
        denied_paths: vec!["project://target".to_string()],
        allowed_objects: Vec::new(),
        max_mutation_count: 3,
        time_budget_ms: 300_000,
        external_cost_budget_microunits: 25,
        allow_delete: true,
        allow_dependency_change: false,
        allow_network: false,
    })
    .unwrap();
    let spec = AiGoalGrantSpec::new(
        goal(),
        risk,
        "gateway-session-elevated",
        "native-editor-user",
        None,
    )
    .unwrap();
    let grant = AiCapabilityGrant::elevated_for_goal(spec).unwrap();

    grant.validate_integrity().unwrap();
    assert_eq!(grant.kind, AiCapabilityGrantKind::Elevated);
    assert_eq!(grant.scope_mode, AiCapabilityScopeMode::Elevated);
    assert!(grant.allow_delete);
    assert_eq!(grant.max_mutation_count, 3);
    assert_eq!(grant.external_cost_budget_microunits, 25);
    assert_eq!(
        grant.goal_binding.as_ref().unwrap().project_identity,
        "project-1"
    );
    assert_eq!(
        grant.risk_envelope.as_ref().unwrap().risk_class,
        AiGoalRiskClass::Elevated
    );

    let mut tampered = grant;
    tampered.max_mutation_count += 1;
    assert_eq!(
        tampered.validate_integrity().unwrap_err().code,
        "ai_tool.grant_digest_mismatch"
    );
}
