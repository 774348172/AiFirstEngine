use super::fixtures::*;
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

pub(super) struct FakeHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub delay_ms: u64,
}

pub(super) fn fake_http_server(responses: Vec<FakeHttpResponse>) -> (String, Receiver<String>) {
    spawn_fake_http_server(responses, None)
}

pub(super) fn gated_fake_http_server(
    response: FakeHttpResponse,
) -> (String, Receiver<String>, mpsc::Sender<()>) {
    let (release_sender, release_receiver) = mpsc::channel();
    let (base_url, request_receiver) =
        spawn_fake_http_server(vec![response], Some(release_receiver));
    (base_url, request_receiver, release_sender)
}

fn spawn_fake_http_server(
    responses: Vec<FakeHttpResponse>,
    response_gate: Option<Receiver<()>>,
) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            let header_end = loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break request.len();
                }
                request.extend_from_slice(&buffer[..count]);
                if let Some(index) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let header_text = String::from_utf8_lossy(&request[..header_end]);
            let content_length = header_text
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            while request.len() < header_end + content_length {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
            if let Some(response_gate) = response_gate.as_ref() {
                let _ = response_gate.recv_timeout(Duration::from_secs(5));
            } else if response.delay_ms > 0 {
                thread::sleep(Duration::from_millis(response.delay_ms));
            }
            let reason = match response.status {
                200 => "OK",
                302 => "Found",
                400 => "Bad Request",
                401 => "Unauthorized",
                403 => "Forbidden",
                429 => "Too Many Requests",
                500 => "Internal Server Error",
                _ => "Test",
            };
            let mut head = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                response.status,
                reason,
                response.body.len()
            );
            for (name, value) in response.headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(response.body.as_bytes());
        }
    });
    (format!("http://{address}/v1"), receiver)
}

pub(super) fn success_envelope() -> String {
    let mock = ThinLlmPatchSource::generate_project_patch_json(
        &LlmPatchSourceConfig::deterministic_mock(),
        "create fake HTTP entity",
        "{}",
    )
    .raw_json
    .unwrap();
    serde_json::json!({
        "choices": [{
            "message": { "content": mock, "refusal": null },
            "finish_reason": "stop"
        }]
    })
    .to_string()
}

pub(super) fn openai_config(base_url: String) -> LlmPatchSourceConfig {
    let mut config = LlmPatchSourceConfig::deterministic_mock();
    config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
    config.provider_id = "fake-openai-compatible".to_string();
    config.model = "fake-project-patch-model".to_string();
    config.base_url = base_url;
    config.timeout_ms = 5_000;
    config.api_key = RedactedSecret::new("gate-b-secret");
    config
}

#[test]
fn llm_patch_source_prompt_declares_project_patch_boundaries() {
    let prompt = build_project_patch_generation_prompt(
        "create a test entity",
        "project_root: G:/example\nselected_entity: none",
    );

    assert!(prompt.contains("project-patch.v1"));
    assert!(prompt.contains(
        "Supported capabilities in this stage: Scene, Input, Asset, Prefab, AUI, Rule, Build"
    ));
    assert!(prompt.contains("Only use documented ProjectPatchOperation schemas"));
    assert!(prompt.contains("Do not write files directly"));
    assert!(prompt.contains("Player, Enemy, or Bullet"));
}

#[test]
fn llm_http_strict_success_sends_guarded_chat_completions_request() {
    let (base_url, requests) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 0,
    }]);
    let config = openai_config(base_url);

    let result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "create an entity",
        "{\"contextHash\":\"sha256:test\"}",
    );

    assert_eq!(result.status, LlmPatchSourceStatus::Success);
    assert_eq!(result.transport_attempt_count, 1);
    assert_eq!(result.http_status_class.as_deref(), Some("2xx"));
    serde_json::from_str::<ProjectPatchDocument>(result.raw_json.as_deref().unwrap()).unwrap();
    let request = requests.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer gate-b-secret"));
    let body = request.split("\r\n\r\n").nth(1).unwrap();
    let value: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(value["model"], "fake-project-patch-model");
    assert_eq!(value["response_format"]["type"], "json_schema");
    assert_eq!(value["response_format"]["json_schema"]["strict"], true);
    assert!(value["response_format"]["json_schema"]["schema"].is_object());
}

#[test]
fn llm_http_retries_transient_server_error_once() {
    let (base_url, requests) = fake_http_server(vec![
        FakeHttpResponse {
            status: 500,
            headers: Vec::new(),
            body: "{}".to_string(),
            delay_ms: 0,
        },
        FakeHttpResponse {
            status: 200,
            headers: Vec::new(),
            body: success_envelope(),
            delay_ms: 0,
        },
    ]);
    let config = openai_config(base_url);
    let result = ThinLlmPatchSource::generate_project_patch_json(&config, "create", "{}");

    assert_eq!(result.status, LlmPatchSourceStatus::Success);
    assert_eq!(result.transport_attempt_count, 2);
    assert!(requests.recv_timeout(Duration::from_secs(1)).is_ok());
    assert!(requests.recv_timeout(Duration::from_secs(1)).is_ok());
}

#[test]
fn llm_http_refusal_redirect_and_response_limit_are_rejected() {
    let refusal = serde_json::json!({
        "choices": [{
            "message": { "content": null, "refusal": "no" },
            "finish_reason": "stop"
        }]
    })
    .to_string();
    let (refusal_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: refusal,
        delay_ms: 0,
    }]);
    let refused = ThinLlmPatchSource::generate_project_patch_json(
        &openai_config(refusal_url),
        "create",
        "{}",
    );
    assert_eq!(refused.status, LlmPatchSourceStatus::Refused);

    let (redirect_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 302,
        headers: vec![("Location".to_string(), "http://127.0.0.1/other".to_string())],
        body: "{}".to_string(),
        delay_ms: 0,
    }]);
    let redirected = ThinLlmPatchSource::generate_project_patch_json(
        &openai_config(redirect_url),
        "create",
        "{}",
    );
    assert_eq!(
        redirected.error_code.as_deref(),
        Some("llm_patch_source.redirect_rejected")
    );

    let (large_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: "x".repeat(256),
        delay_ms: 0,
    }]);
    let mut limited = openai_config(large_url);
    limited.maximum_response_bytes = 64;
    let oversized = ThinLlmPatchSource::generate_project_patch_json(&limited, "create", "{}");
    assert_eq!(oversized.status, LlmPatchSourceStatus::ResponseTooLarge);
}

#[test]
fn llm_http_auth_rate_limit_and_timeout_have_typed_results() {
    for (http_status, expected) in [
        (401, LlmPatchSourceStatus::AuthFailed),
        (403, LlmPatchSourceStatus::AuthFailed),
        (429, LlmPatchSourceStatus::RateLimited),
    ] {
        let (base_url, _) = fake_http_server(vec![FakeHttpResponse {
            status: http_status,
            headers: if http_status == 429 {
                vec![("Retry-After".to_string(), "9".to_string())]
            } else {
                Vec::new()
            },
            body: "{}".to_string(),
            delay_ms: 0,
        }]);
        let result = ThinLlmPatchSource::generate_project_patch_json(
            &openai_config(base_url),
            "create",
            "{}",
        );
        assert_eq!(result.status, expected);
        assert_eq!(result.transport_attempt_count, 1);
    }

    let (timeout_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 1_000,
    }]);
    let mut timeout = openai_config(timeout_url);
    timeout.timeout_ms = 20;
    let result = ThinLlmPatchSource::generate_project_patch_json(&timeout, "create", "{}");
    assert_eq!(result.status, LlmPatchSourceStatus::TimedOut);
}

#[test]
fn llm_patch_source_openai_rejects_non_loopback_plain_http_before_send() {
    let config = openai_config("http://example.com/v1".to_string());
    let result = ThinLlmPatchSource::generate_project_patch_json(&config, "create", "{}");
    assert_eq!(
        result.error_code.as_deref(),
        Some("llm_patch_source.base_url_forbidden")
    );
    assert_eq!(result.transport_attempt_count, 0);
}

#[test]
fn llm_provider_redaction_excludes_secret_from_debug_and_serialization() {
    let config = openai_config("https://example.com/v1".to_string());
    let debug = format!("{config:?}");
    let json = serde_json::to_string(&config).unwrap();

    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("gate-b-secret"));
    assert!(!json.contains("gate-b-secret"));
    assert!(!json.contains("api_key"));
}

#[test]
fn llm_patch_context_is_sanitized_stable_and_all_domain() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let first = ProjectPatchLlmContextSnapshot::capture(&session);
    let encoded = first.prompt_json();

    assert_eq!(
        first.supported_capabilities,
        ["Scene", "Input", "Asset", "Prefab", "AUI", "Rule", "Build"]
    );
    assert_eq!(
        first.max_operation_count,
        PatchValidator::MAX_OPERATION_COUNT
    );
    assert!(!encoded.contains("G:\\"));
    assert!(!encoded.contains("I:\\"));
    assert!(!encoded.contains("projectRoot"));

    session
        .ai_panel_messages
        .push(editor_ui_model::AiPanelMessage {
            message_id: "ignored-message".to_string(),
            role: editor_ui_model::AiPanelMessageRole::System,
            text: "report refresh noise".to_string(),
        });
    let after_panel_noise = ProjectPatchLlmContextSnapshot::capture(&session);
    assert_eq!(first.context_hash, after_panel_noise.context_hash);

    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));
    let after_selection = ProjectPatchLlmContextSnapshot::capture(&session);
    assert_ne!(first.context_hash, after_selection.context_hash);
    assert_eq!(
        after_selection.selected_entity_id.as_deref(),
        Some("entity-player")
    );
}

#[test]
fn llm_patch_context_semantic_hash_ignores_its_stored_hash() {
    let session = EditorSession::new();
    let mut context = ProjectPatchLlmContextSnapshot::capture(&session);
    let expected = context.context_hash.clone();
    context.context_hash = "tampered-display-value".to_string();
    assert_eq!(context.semantic_hash(), expected);
}

#[test]
fn llm_patch_source_mock_generates_valid_project_patch_json() {
    let config = LlmPatchSourceConfig::deterministic_mock();
    let result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "create \"Generated By LLM\"",
        "selected_entity: none",
    );

    let raw_json = result.raw_json.expect("mock should produce JSON");
    let patch: ProjectPatchDocument =
        serde_json::from_str(&raw_json).expect("mock JSON should parse");

    assert_eq!(result.provider_id, "mock-llm-patch-source");
    assert_eq!(patch.patch_id, "llm-mock-create-entity");
    assert_eq!(patch.required_capabilities, vec![PatchCapability::Scene]);
    assert!(matches!(
        &patch.operations[0],
        PatchOperation::Scene(ScenePatchOperation::CreateEntity { name, .. })
            if name == "Generated By LLM"
    ));
}

#[test]
fn llm_patch_source_mock_invalid_json_flows_through_import_rejection() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let config = LlmPatchSourceConfig::deterministic_mock();
    let result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "invalid_json",
        "selected_entity: none",
    );
    let raw_json = result
        .raw_json
        .expect("mock invalid path should return raw JSON");
    let request = ProjectPatchImportRequest::ai_structured_output("mock", raw_json);

    let import = ProjectPatchImportService::from_json_string(&session, request);

    assert_eq!(
        import.source_kind,
        ProjectPatchImportSourceKind::AiStructuredOutput
    );
    assert_eq!(import.parse_status, ProjectPatchImportParseStatus::Rejected);
    assert!(import
        .schema_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch_import.parse_failed"));
}

#[test]
fn llm_patch_source_mock_provider_error_returns_no_raw_json() {
    let config = LlmPatchSourceConfig::deterministic_mock();
    let result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "provider_error",
        "selected_entity: none",
    );

    assert!(result.raw_json.is_none());
    assert_eq!(
        result.error_code.as_deref(),
        Some("llm_patch_source.provider_error")
    );
}

#[test]
fn llm_patch_source_mock_all_domain_capability_imports_without_unsupported_diagnostic() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "LlmAllDomain".to_string(),
    }));
    let config = LlmPatchSourceConfig::deterministic_mock();
    let result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "create all_domain patch",
        "selected_entity: none",
    );
    let raw_json = result.raw_json.expect("mock should produce JSON");
    let request = ProjectPatchImportRequest::ai_structured_output("mock", raw_json);

    let import = ProjectPatchImportService::from_json_string(&session, request);

    assert_eq!(import.parse_status, ProjectPatchImportParseStatus::Parsed);
    assert!(import.capability_diagnostics.is_empty());
    assert!(import
        .validation
        .as_ref()
        .is_some_and(|validation| validation.accepted));
    assert!(!import
        .next_actions
        .contains(&"defer_unsupported_project_patch_capability".to_string()));
}

#[test]
#[ignore = "requires explicit editor-only OpenAI-compatible provider environment and network"]
fn llm_real_patch_source_openai_compatible_env_gated_smoke() {
    let config = LlmPatchSourceConfig::openai_compatible_from_env();
    assert!(
        config.enabled,
        "set AI_ENGINE_LLM_PATCH_SOURCE=openai_compatible before running this ignored smoke"
    );
    let result = ThinLlmPatchSource::generate_project_patch_json(
        &config,
        "create \"Real Provider Smoke\"",
        "{\"selectedEntityId\":null}",
    );

    assert_ne!(
        result.error_code.as_deref(),
        Some("llm_patch_source.openai_compatible_not_implemented")
    );
}
