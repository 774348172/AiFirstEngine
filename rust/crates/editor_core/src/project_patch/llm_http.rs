use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use url::{Host, Url};

use super::{
    build_project_patch_generation_prompt, project_patch_json_schema, LlmCredentialLease,
    LlmPatchSourceResult, LlmPatchSourceStatus, LlmStructuredOutputMode, LlmTransportConfig,
};

pub(super) fn generate(
    config: &LlmTransportConfig,
    credential: &LlmCredentialLease,
    user_prompt: &str,
    context_summary: &str,
) -> LlmPatchSourceResult {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            return LlmPatchSourceResult::error(
                config,
                LlmPatchSourceStatus::TransportError,
                "llm_request_controller.executor_unavailable",
                "The editor LLM async executor could not be created.",
                "Restart the editor and retry the request.",
            )
        }
    };
    runtime.block_on(generate_async(
        config,
        credential,
        user_prompt,
        context_summary,
        CancellationToken::new(),
    ))
}

pub(super) async fn generate_async(
    config: &LlmTransportConfig,
    credential: &LlmCredentialLease,
    user_prompt: &str,
    context_summary: &str,
    cancellation: CancellationToken,
) -> LlmPatchSourceResult {
    let started = Instant::now();
    let endpoint = match endpoint(config, credential) {
        Ok(endpoint) => endpoint,
        Err(result) => return result,
    };
    let request = request_body(config, user_prompt, context_summary);
    let request_bytes = match serde_json::to_vec(&request) {
        Ok(bytes) => bytes,
        Err(_) => {
            return error(
                config,
                LlmPatchSourceStatus::InvalidProviderResponse,
                "llm_patch_source.request_invalid",
                "The structured LLM request could not be serialized.",
                "Check the generated ProjectPatch schema.",
                started,
                None,
                0,
            )
        }
    };
    if request_bytes.len() > config.maximum_request_bytes {
        return error(
            config,
            LlmPatchSourceStatus::ResponseTooLarge,
            "llm_patch_source.request_too_large",
            "The structured LLM request exceeds the configured byte limit.",
            "Reduce the editor context or prompt size.",
            started,
            None,
            0,
        );
    }

    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return error(
                config,
                LlmPatchSourceStatus::TransportError,
                "llm_patch_source.transport_failed",
                "The LLM HTTP client could not be created.",
                "Restart the editor and retry the request.",
                started,
                None,
                0,
            )
        }
    };
    let maximum_attempts = config.maximum_transport_retries.min(1) + 1;
    let operation = async {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match send_once(
                &client,
                config,
                credential,
                endpoint.as_str(),
                &request,
                cancellation.clone(),
            )
            .await
            {
                Ok(response) => {
                    if should_retry_status(&response, config) && attempt < maximum_attempts {
                        if response.retry_after_ms > 0 {
                            tokio::select! {
                                _ = cancellation.cancelled() => return cancelled(config, started, attempt),
                                _ = tokio::time::sleep(Duration::from_millis(response.retry_after_ms)) => {}
                            }
                        }
                        continue;
                    }
                    return finish_response(config, response, started, attempt);
                }
                Err(transport) => {
                    if transport.status == LlmPatchSourceStatus::Cancelled {
                        return cancelled(config, started, attempt);
                    }
                    if transport.retryable && attempt < maximum_attempts {
                        continue;
                    }
                    return error(
                        config,
                        transport.status,
                        transport.code,
                        transport.message,
                        transport.next_action,
                        started,
                        None,
                        attempt,
                    );
                }
            }
        }
    };
    tokio::select! {
        _ = cancellation.cancelled() => cancelled(config, started, 0),
        result = tokio::time::timeout(Duration::from_millis(config.timeout_ms.max(1)), operation) => {
            match result {
                Ok(result) => result,
                Err(_) => error(
                    config,
                    LlmPatchSourceStatus::TimedOut,
                    "llm_patch_source.timeout",
                    "The LLM provider request timed out.",
                    "Check provider availability or adjust the editor timeout.",
                    started,
                    None,
                    0,
                ),
            }
        }
    }
}

fn endpoint(
    config: &LlmTransportConfig,
    credential: &LlmCredentialLease,
) -> Result<Url, LlmPatchSourceResult> {
    let base = Url::parse(&config.base_url).map_err(|_| {
        LlmPatchSourceResult::error(
            config,
            LlmPatchSourceStatus::HttpClientError,
            "llm_patch_source.config_missing",
            "LLM base URL is invalid.",
            "Set AI_ENGINE_LLM_BASE_URL to an HTTPS API root or loopback HTTP root.",
        )
    })?;
    if !base.username().is_empty() || base.password().is_some() {
        return Err(LlmPatchSourceResult::error(
            config,
            LlmPatchSourceStatus::HttpClientError,
            "llm_patch_source.base_url_forbidden",
            "LLM base URL must not contain credentials.",
            "Remove username and password data from AI_ENGINE_LLM_BASE_URL.",
        ));
    }
    let loopback = match base.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if base.scheme() != "https" && !(base.scheme() == "http" && loopback) {
        return Err(LlmPatchSourceResult::error(
            config,
            LlmPatchSourceStatus::HttpClientError,
            "llm_patch_source.base_url_forbidden",
            "Plain HTTP is only allowed for loopback LLM providers.",
            "Use HTTPS or a localhost/loopback development endpoint.",
        ));
    }
    if !loopback && !credential.is_present() {
        return Err(LlmPatchSourceResult::error(
            config,
            LlmPatchSourceStatus::AuthFailed,
            "llm_patch_source.config_missing",
            "A credential is required for a non-loopback LLM provider.",
            "Set AI_ENGINE_LLM_API_KEY in the editor process environment.",
        ));
    }
    let mut endpoint = base;
    let path = format!("{}/chat/completions", endpoint.path().trim_end_matches('/'));
    endpoint.set_path(&path);
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    Ok(endpoint)
}

fn request_body(config: &LlmTransportConfig, user_prompt: &str, context_summary: &str) -> Value {
    let response_format = match config.structured_output_mode {
        LlmStructuredOutputMode::StrictJsonSchema => json!({
            "type": "json_schema",
            "json_schema": {
                "name": "project_patch_document",
                "strict": true,
                "schema": project_patch_json_schema(),
            }
        }),
        LlmStructuredOutputMode::JsonObject => json!({ "type": "json_object" }),
    };
    json!({
        "model": config.model,
        "messages": [{
            "role": "user",
            "content": build_project_patch_generation_prompt(user_prompt, context_summary),
        }],
        "response_format": response_format,
    })
}

struct HttpResponse {
    status: u16,
    body: String,
    retry_after_ms: u64,
}

struct TransportFailure {
    status: LlmPatchSourceStatus,
    code: &'static str,
    message: &'static str,
    next_action: &'static str,
    retryable: bool,
}

async fn send_once(
    client: &reqwest::Client,
    config: &LlmTransportConfig,
    credential: &LlmCredentialLease,
    endpoint: &str,
    request: &Value,
    cancellation: CancellationToken,
) -> Result<HttpResponse, TransportFailure> {
    let mut builder = client
        .post(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .json(request);
    let authorization;
    if let Some(secret) = credential.expose() {
        authorization = format!("Bearer {secret}");
        builder = builder.header(reqwest::header::AUTHORIZATION, authorization.as_str());
    }
    let mut response = tokio::select! {
        _ = cancellation.cancelled() => return Err(cancelled_transport()),
        result = builder.send() => result.map_err(classify_transport)?,
    };
    let status = response.status().as_u16();
    let retry_after_ms = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000))
        .unwrap_or(0);
    let mut body = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(cancelled_transport()),
            result = response.chunk() => result.map_err(classify_transport)?,
        };
        let Some(chunk) = chunk else { break };
        if body.len().saturating_add(chunk.len()) > config.maximum_response_bytes {
            return Err(TransportFailure {
                status: LlmPatchSourceStatus::ResponseTooLarge,
                code: "llm_transport.body_limit_exceeded",
                message: "The LLM provider response exceeds the configured byte limit.",
                next_action: "Reduce provider output or increase the guarded local limit.",
                retryable: false,
            });
        }
        body.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(body).map_err(|_| TransportFailure {
        status: LlmPatchSourceStatus::InvalidProviderResponse,
        code: "llm_patch_source.response_invalid",
        message: "The LLM provider returned a non-UTF-8 response.",
        next_action: "Use an OpenAI-compatible JSON response endpoint.",
        retryable: false,
    })?;
    Ok(HttpResponse {
        status,
        body,
        retry_after_ms,
    })
}

fn classify_transport(error: reqwest::Error) -> TransportFailure {
    if error.is_timeout() {
        TransportFailure {
            status: LlmPatchSourceStatus::TimedOut,
            code: "llm_patch_source.timeout",
            message: "The LLM provider request timed out.",
            next_action: "Check provider availability or adjust the editor timeout.",
            retryable: false,
        }
    } else {
        TransportFailure {
            status: LlmPatchSourceStatus::TransportError,
            code: "llm_patch_source.transport_failed",
            message: "The LLM provider transport failed.",
            next_action: "Check the provider URL and network availability.",
            retryable: true,
        }
    }
}

fn cancelled_transport() -> TransportFailure {
    TransportFailure {
        status: LlmPatchSourceStatus::Cancelled,
        code: "llm_transport.cancelled",
        message: "The local LLM transport was cancelled.",
        next_action: "Wait for the joined cancellation receipt before starting another request.",
        retryable: false,
    }
}

fn cancelled(config: &LlmTransportConfig, started: Instant, attempts: u8) -> LlmPatchSourceResult {
    error(
        config,
        LlmPatchSourceStatus::Cancelled,
        "llm_transport.cancelled",
        "The local LLM transport was cancelled.",
        "Wait for the joined cancellation receipt before starting another request.",
        started,
        None,
        attempts,
    )
}

fn should_retry_status(response: &HttpResponse, config: &LlmTransportConfig) -> bool {
    matches!(response.status, 500 | 502 | 503 | 504)
        || (response.status == 429
            && response.retry_after_ms > 0
            && response.retry_after_ms <= config.maximum_retry_after_ms)
}

fn finish_response(
    config: &LlmTransportConfig,
    response: HttpResponse,
    started: Instant,
    attempts: u8,
) -> LlmPatchSourceResult {
    if !(200..300).contains(&response.status) {
        let (status, code, message, next_action) = match response.status {
            400 if config.structured_output_mode == LlmStructuredOutputMode::StrictJsonSchema => (
                LlmPatchSourceStatus::StructuredOutputUnsupported,
                "llm_patch_source.structured_output_unsupported",
                "The provider rejected strict JSON Schema output.",
                "Use a provider with strict schema support or explicitly select degraded json_object mode.",
            ),
            401 | 403 => (
                LlmPatchSourceStatus::AuthFailed,
                "llm_patch_source.auth_failed",
                "The LLM provider rejected editor credentials.",
                "Check AI_ENGINE_LLM_API_KEY without placing it in project assets.",
            ),
            429 => (
                LlmPatchSourceStatus::RateLimited,
                "llm_patch_source.rate_limited",
                "The LLM provider rate limit was reached.",
                "Wait before submitting another ProjectPatch request.",
            ),
            500..=599 => (
                LlmPatchSourceStatus::HttpServerError,
                "llm_patch_source.http_server_error",
                "The LLM provider returned a server error.",
                "Retry after the provider recovers.",
            ),
            300..=399 => (
                LlmPatchSourceStatus::HttpClientError,
                "llm_patch_source.redirect_rejected",
                "The LLM provider attempted to redirect the request.",
                "Configure the final provider API root directly.",
            ),
            _ => (
                LlmPatchSourceStatus::HttpClientError,
                "llm_patch_source.http_client_error",
                "The LLM provider rejected the request.",
                "Check the endpoint, model, and structured output configuration.",
            ),
        };
        return error(
            config,
            status,
            code,
            message,
            next_action,
            started,
            Some(status_class(response.status)),
            attempts,
        );
    }
    parse_success(config, &response.body, started, attempts)
}

#[derive(Deserialize)]
struct ProviderEnvelope {
    choices: Vec<ProviderChoice>,
}

#[derive(Deserialize)]
struct ProviderChoice {
    message: ProviderMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ProviderMessage {
    content: Option<String>,
    refusal: Option<String>,
}

fn parse_success(
    config: &LlmTransportConfig,
    body: &str,
    started: Instant,
    attempts: u8,
) -> LlmPatchSourceResult {
    let envelope: ProviderEnvelope = match serde_json::from_str(body) {
        Ok(envelope) => envelope,
        Err(_) => {
            return error(
                config,
                LlmPatchSourceStatus::InvalidProviderResponse,
                "llm_patch_source.response_invalid",
                "The LLM provider returned an invalid response envelope.",
                "Check OpenAI-compatible Chat Completions response shape.",
                started,
                Some("2xx".to_string()),
                attempts,
            )
        }
    };
    let Some(choice) = envelope.choices.into_iter().next() else {
        return error(
            config,
            LlmPatchSourceStatus::EmptyOutput,
            "llm_patch_source.output_empty",
            "The LLM provider returned no output choice.",
            "Retry the request or inspect provider availability.",
            started,
            Some("2xx".to_string()),
            attempts,
        );
    };
    if choice.message.refusal.is_some() {
        return error(
            config,
            LlmPatchSourceStatus::Refused,
            "llm_patch_source.refused",
            "The LLM provider refused to generate a ProjectPatch candidate.",
            "Revise the request without bypassing ProjectPatch safety boundaries.",
            started,
            Some("2xx".to_string()),
            attempts,
        );
    }
    if choice
        .finish_reason
        .as_deref()
        .is_some_and(|reason| reason != "stop")
    {
        return error(
            config,
            LlmPatchSourceStatus::InvalidProviderResponse,
            "llm_patch_source.response_invalid",
            "The LLM provider did not finish a complete structured response.",
            "Retry with a provider that can complete the ProjectPatch response.",
            started,
            Some("2xx".to_string()),
            attempts,
        );
    }
    let Some(content) = choice
        .message
        .content
        .filter(|content| !content.trim().is_empty())
    else {
        return error(
            config,
            LlmPatchSourceStatus::EmptyOutput,
            "llm_patch_source.output_empty",
            "The LLM provider returned empty ProjectPatch content.",
            "Retry the request or inspect provider availability.",
            started,
            Some("2xx".to_string()),
            attempts,
        );
    };
    if content.len() > config.maximum_candidate_bytes {
        return error(
            config,
            LlmPatchSourceStatus::ResponseTooLarge,
            "llm_patch_source.response_too_large",
            "The LLM provider content exceeds the ProjectPatch candidate byte limit.",
            "Reduce provider output to one bounded ProjectPatchDocument candidate.",
            started,
            Some("2xx".to_string()),
            attempts,
        );
    }
    let mut result = LlmPatchSourceResult::success(config, content);
    result.latency_ms = started.elapsed().as_millis() as u64;
    result.http_status_class = Some("2xx".to_string());
    result.transport_attempt_count = attempts;
    result
}

#[allow(clippy::too_many_arguments)]
fn error(
    config: &LlmTransportConfig,
    status: LlmPatchSourceStatus,
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
    started: Instant,
    http_status_class: Option<String>,
    attempts: u8,
) -> LlmPatchSourceResult {
    let mut result = LlmPatchSourceResult::error(config, status, code, message, next_action);
    result.latency_ms = started.elapsed().as_millis() as u64;
    result.http_status_class = http_status_class;
    result.transport_attempt_count = attempts;
    result
}

fn status_class(status: u16) -> String {
    format!("{}xx", status / 100)
}
