use super::*;

const CATALOG_V2: &str = "ai-tool-catalog.v2";

fn created_session(name: &str) -> (EditorSession, PathBuf) {
    let root = fixtures::unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: name.to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    (session, root)
}

fn v2_request() -> AiToolCatalogRequest {
    AiToolCatalogRequest {
        schema_version: CATALOG_V2.to_string(),
    }
}

#[test]
fn ai_tool_catalog_v2_schema_is_strict_complete_and_deterministic() {
    let kernel = AiCapabilityToolKernel::new();
    let first = serde_json::to_value(kernel.catalog(v2_request()).unwrap()).unwrap();
    let second = serde_json::to_value(kernel.catalog(v2_request()).unwrap()).unwrap();

    assert_eq!(first, second);
    assert_eq!(first["schemaVersion"], CATALOG_V2);
    assert!(first["catalogDigest"].as_str().is_some());
    assert!(first["availabilityDigest"].as_str().is_some());
    assert!(first["basis"].is_object());
    assert!(first["tools"]
        .as_array()
        .is_some_and(|tools| !tools.is_empty()));

    let mut unknown = first;
    unknown["unknownRequiredSemantic"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AiToolCatalog>(unknown).is_err());
}

#[test]
fn ai_tool_catalog_v1_preserves_the_exact_descriptors_only_shape() {
    let kernel = AiCapabilityToolKernel::new();
    let value = serde_json::to_value(kernel.catalog(AiToolCatalogRequest::v1()).unwrap()).unwrap();
    let object = value.as_object().expect("v1 Catalog object");

    assert_eq!(object.len(), 2);
    assert_eq!(value["schemaVersion"], AI_TOOL_CATALOG_V1_SCHEMA_VERSION);
    assert!(value["tools"]
        .as_array()
        .is_some_and(|tools| !tools.is_empty()));
    assert!(value.get("catalogDigest").is_none());
    assert!(value.get("availabilityDigest").is_none());
    assert!(value.get("basis").is_none());
    assert!(serde_json::from_value::<AiToolCatalog>(value).is_ok());
}

#[test]
fn ai_tool_catalog_v2_rejects_malformed_or_mismatched_digests() {
    let kernel = AiCapabilityToolKernel::new();
    let value = serde_json::to_value(kernel.catalog(v2_request()).unwrap()).unwrap();

    for field in ["catalogDigest", "availabilityDigest"] {
        let mut tampered = value.clone();
        tampered[field] = serde_json::json!("sha256:not-a-valid-canonical-digest");
        assert!(serde_json::from_value::<AiToolCatalog>(tampered).is_err());
    }

    let mut inconsistent_basis = value;
    inconsistent_basis["tools"][0]["availability"]["basis"]["accessGeneration"] =
        serde_json::json!(99);
    assert!(serde_json::from_value::<AiToolCatalog>(inconsistent_basis).is_err());
}

#[test]
fn ai_tool_catalog_probe_is_side_effect_free() {
    let kernel = AiCapabilityToolKernel::new();
    let (session, root) = created_session("CatalogSideEffectFree");
    let before = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();

    let first =
        serde_json::to_vec(&kernel.catalog_for_session(&session, v2_request()).unwrap()).unwrap();
    let second =
        serde_json::to_vec(&kernel.catalog_for_session(&session, v2_request()).unwrap()).unwrap();
    let after = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();

    assert_eq!(first, second);
    assert_eq!(before, after);
    assert!(session.pending_project_preview_frame_ticket().is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_tool_catalog_v2_keeps_every_registered_tool_visible_without_project() {
    let kernel = AiCapabilityToolKernel::new();
    let empty = EditorSession::new();
    let catalog = kernel.catalog_for_session(&empty, v2_request()).unwrap();
    let value = serde_json::to_value(catalog).unwrap();
    let tools = value["tools"].as_array().expect("v2 tools");

    assert_eq!(
        tools.len(),
        AiToolContractRegistry::new().descriptors().len()
    );
    assert!(tools.iter().all(|entry| entry["descriptor"].is_object()));
    assert!(tools.iter().all(|entry| {
        let tool_id = entry["descriptor"]["toolId"].as_str();
        let state = entry["availability"]["state"].as_str();
        if tool_id == Some(TOOL_ID_PROJECT_CREATE) {
            state == Some("ready")
        } else {
            state == Some("blocked")
        }
    }));
}

#[test]
fn ai_tool_catalog_v2_separates_contract_and_availability_digests() {
    let kernel = AiCapabilityToolKernel::new();
    let (session, root) = created_session("CatalogDigestSeparation");
    let static_catalog = serde_json::to_value(kernel.catalog(v2_request()).unwrap()).unwrap();
    let session_catalog =
        serde_json::to_value(kernel.catalog_for_session(&session, v2_request()).unwrap()).unwrap();

    assert_eq!(
        static_catalog["catalogDigest"],
        session_catalog["catalogDigest"]
    );
    assert_ne!(
        static_catalog["availabilityDigest"],
        session_catalog["availabilityDigest"]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_tool_catalog_v2_reports_all_reasons_with_stable_precedence() {
    let kernel = AiCapabilityToolKernel::new();
    let empty = EditorSession::new();
    let value =
        serde_json::to_value(kernel.catalog_for_session(&empty, v2_request()).unwrap()).unwrap();
    let mutation = value["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["descriptor"]["toolId"] == TOOL_ID_PROJECT_MUTATE)
        .expect("goal mutation entry");

    assert_eq!(mutation["availability"]["state"], "blocked");
    let reasons = mutation["availability"]["reasons"].as_array().unwrap();
    assert!(reasons.len() >= 2);
    assert!(reasons
        .iter()
        .any(|reason| reason["category"] == "project_state"));
    assert!(reasons
        .iter()
        .any(|reason| reason["category"] == "authorization"));
}
