use architecture_review::{
    execute_provider_review, ArchitectureReviewHttpConfig, ProviderCredential, ProviderErrorClass,
};
use quality_gate::architecture_artifact::ReviewOutcome;
use quality_gate::architecture_review::{
    ArchitectureReviewRequest, ARCHITECTURE_REVIEW_SCHEMA_VERSION,
};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[test]
fn loopback_provider_success_and_invalid_schema() {
    let content = serde_json::json!({"outcome":"complete","findings":[]}).to_string();
    let success = envelope(&content, None);
    let (url, server) = serve(200, success, Duration::ZERO);
    let result = runtime().block_on(execute_provider_review(
        &config(&url),
        &ProviderCredential::new("secret"),
        &request(),
        "prompt",
        "{}",
        CancellationToken::new(),
    ));
    assert_eq!(result.unwrap().payload.outcome, ReviewOutcome::Complete);
    server.join().unwrap();

    let (url, server) = serve(200, envelope("not-json", None), Duration::ZERO);
    let error = runtime()
        .block_on(execute_provider_review(
            &config(&url),
            &ProviderCredential::new("secret"),
            &request(),
            "prompt",
            "{}",
            CancellationToken::new(),
        ))
        .unwrap_err();
    assert_eq!(error.class, ProviderErrorClass::InvalidSchema);
    server.join().unwrap();
}

#[test]
fn loopback_provider_refusal_rate_limit_oversize_timeout_and_cancel() {
    let cases = [
        (403, ProviderErrorClass::Refusal),
        (429, ProviderErrorClass::RateLimited),
    ];
    for (status, class) in cases {
        let (url, server) = serve(status, "{}".to_string(), Duration::ZERO);
        let error = runtime()
            .block_on(execute_provider_review(
                &config(&url),
                &ProviderCredential::new("secret"),
                &request(),
                "prompt",
                "{}",
                CancellationToken::new(),
            ))
            .unwrap_err();
        assert_eq!(error.class, class);
        server.join().unwrap();
    }

    let content = "x".repeat(4096);
    let (url, server) = serve(200, content, Duration::ZERO);
    let mut limited = config(&url);
    limited.response_limit_bytes = 32;
    let error = runtime()
        .block_on(execute_provider_review(
            &limited,
            &ProviderCredential::new("secret"),
            &request(),
            "prompt",
            "{}",
            CancellationToken::new(),
        ))
        .unwrap_err();
    assert_eq!(error.class, ProviderErrorClass::Oversize);
    server.join().unwrap();

    let (url, server) = serve(200, "{}".to_string(), Duration::from_millis(200));
    let mut short = config(&url);
    short.timeout_ms = 25;
    let error = runtime()
        .block_on(execute_provider_review(
            &short,
            &ProviderCredential::new("secret"),
            &request(),
            "prompt",
            "{}",
            CancellationToken::new(),
        ))
        .unwrap_err();
    assert_eq!(error.class, ProviderErrorClass::Timeout);
    server.join().unwrap();

    let (url, server) = serve(200, "{}".to_string(), Duration::from_millis(200));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = runtime()
        .block_on(execute_provider_review(
            &config(&url),
            &ProviderCredential::new("secret"),
            &request(),
            "prompt",
            "{}",
            cancellation,
        ))
        .unwrap_err();
    assert_eq!(error.class, ProviderErrorClass::Cancelled);
    server.join().unwrap();
}

fn serve(status: u16, body: String, delay: Duration) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        ready_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connection = None;
        while Instant::now() < deadline {
            match listener.accept() {
                Ok(value) => {
                    connection = Some(value);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return,
            }
        }
        let Some((mut stream, _)) = connection else {
            return;
        };
        if read_http_request(&mut stream).is_err() {
            return;
        }
        thread::sleep(delay);
        let reason = if status == 200 { "OK" } else { "Error" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
    ready_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("loopback server thread should become ready");
    (format!("http://{address}/v1/"), handle)
}

fn read_http_request(stream: &mut TcpStream) -> io::Result<()> {
    const MAX_REQUEST_BYTES: usize = 512 * 1024;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut request = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "loopback request ended before its declared body",
            ));
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "loopback request exceeded fixture limit",
            ));
        }
        if let Some(expected_len) = expected_http_request_len(&request)? {
            if request.len() >= expected_len {
                return Ok(());
            }
        }
    }
}

fn expected_http_request_len(request: &[u8]) -> io::Result<Option<usize>> {
    let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n") else {
        return Ok(None);
    };
    let headers = std::str::from_utf8(&request[..header_end]).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("loopback request headers are not UTF-8: {error}"),
        )
    })?;
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim())
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "loopback request has no Content-Length header",
            )
        })?
        .parse::<usize>()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("loopback Content-Length is invalid: {error}"),
            )
        })?;
    Ok(Some(header_end + 4 + content_length))
}

fn envelope(content: &str, refusal: Option<&str>) -> String {
    serde_json::json!({
        "id": "chatcmpl-loopback",
        "object": "chat.completion",
        "created": 1,
        "model": "test",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": content,
                "refusal": refusal
            }
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    })
    .to_string()
}

fn config(base_url: &str) -> ArchitectureReviewHttpConfig {
    ArchitectureReviewHttpConfig {
        provider_id: "loopback".to_string(),
        base_url: base_url.to_string(),
        model: "test".to_string(),
        timeout_ms: 1000,
        request_limit_bytes: 256 * 1024,
        response_limit_bytes: 256 * 1024,
        max_output_tokens: 1024,
        max_total_tokens: 16_384,
        max_cost_micros: 1_000_000,
        cost_per_1k_tokens_micros: 1,
    }
}

fn request() -> ArchitectureReviewRequest {
    ArchitectureReviewRequest {
        schema_version: ARCHITECTURE_REVIEW_SCHEMA_VERSION.to_string(),
        profile_id: "engine".to_string(),
        base_commit: "a".repeat(40),
        head_commit: "b".repeat(40),
        dirty_patch_digest: None,
        workspace_digest: digest('w'),
        policy_digest: digest('p'),
        prompt_digest: digest('r'),
        response_schema_digest: digest('s'),
        context_digest: digest('c'),
        coverage_digest: digest('v'),
        subjects: Vec::new(),
    }
}

fn digest(ch: char) -> String {
    format!("sha256:{}", ch.to_string().repeat(64))
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
