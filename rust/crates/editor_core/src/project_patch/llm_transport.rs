use std::future::Future;
use std::pin::Pin;

use tokio_util::sync::CancellationToken;

use super::{LlmCredentialLease, LlmPatchSourceResult, LlmTransportConfig};

pub(crate) trait LlmTransport: Send + Sync {
    fn execute<'a>(
        &'a self,
        config: &'a LlmTransportConfig,
        credential: &'a LlmCredentialLease,
        prompt: &'a str,
        context_json: &'a str,
        cancellation: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = LlmPatchSourceResult> + Send + 'a>>;
}

#[derive(Debug, Default)]
pub(crate) struct ReqwestAsyncTransport;

impl LlmTransport for ReqwestAsyncTransport {
    fn execute<'a>(
        &'a self,
        config: &'a LlmTransportConfig,
        credential: &'a LlmCredentialLease,
        prompt: &'a str,
        context_json: &'a str,
        cancellation: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = LlmPatchSourceResult> + Send + 'a>> {
        Box::pin(crate::project_patch::llm_http::generate_async(
            config,
            credential,
            prompt,
            context_json,
            cancellation,
        ))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use tokio::sync::Notify;

    use super::*;
    use crate::{LlmPatchSourceStatus, LlmStructuredOutputMode};

    #[derive(Default)]
    pub(crate) struct ControllableLoopbackTransport {
        entered: Arc<AtomicBool>,
        release: Arc<Notify>,
    }

    impl ControllableLoopbackTransport {
        pub(crate) fn entered_probe(&self) -> Arc<AtomicBool> {
            self.entered.clone()
        }

        pub(crate) fn release(&self) {
            self.release.notify_one();
        }
    }

    impl LlmTransport for ControllableLoopbackTransport {
        fn execute<'a>(
            &'a self,
            config: &'a LlmTransportConfig,
            _credential: &'a LlmCredentialLease,
            _prompt: &'a str,
            _context_json: &'a str,
            cancellation: CancellationToken,
        ) -> Pin<Box<dyn Future<Output = LlmPatchSourceResult> + Send + 'a>> {
            Box::pin(async move {
                self.entered.store(true, Ordering::SeqCst);
                tokio::select! {
                    _ = cancellation.cancelled() => LlmPatchSourceResult {
                        provider_id: config.provider_id.clone(),
                        model: config.model.clone(),
                        status: LlmPatchSourceStatus::Cancelled,
                        structured_output_mode: config.structured_output_mode,
                        degraded: config.structured_output_mode == LlmStructuredOutputMode::JsonObject,
                        raw_json: None,
                        error_code: Some("llm_transport.cancelled".to_string()),
                        error_message: Some("The local LLM transport was cancelled.".to_string()),
                        next_action: None,
                        latency_ms: 0,
                        http_status_class: None,
                        transport_attempt_count: 0,
                    },
                    _ = self.release.notified() => LlmPatchSourceResult::success(config, "{}".to_string()),
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::test_support::ControllableLoopbackTransport;
    use super::*;
    use crate::{LlmPatchSourceConfig, LlmPatchSourceKind, LlmPatchSourceStatus, RedactedSecret};

    fn openai_config(base_url: String) -> LlmPatchSourceConfig {
        let mut config = LlmPatchSourceConfig::deterministic_mock();
        config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
        config.base_url = base_url;
        config.api_key = RedactedSecret::new("transport-test-secret");
        config
    }

    #[test]
    fn llm_transport_cancellation_interrupts_controllable_adapter() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let transport = ControllableLoopbackTransport::default();
        let entered = transport.entered_probe();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let source_config = LlmPatchSourceConfig::deterministic_mock();
        let config = source_config.transport_config();
        let result = runtime.block_on(async {
            let operation = transport.execute(
                &config,
                &source_config.api_key,
                "prompt",
                "{}",
                cancellation,
            );
            tokio::pin!(operation);
            tokio::select! {
                _ = async {
                    while !entered.load(Ordering::SeqCst) {
                        tokio::task::yield_now().await;
                    }
                    cancel.cancel();
                } => operation.await,
                result = &mut operation => result,
            }
        });
        assert_eq!(result.status, LlmPatchSourceStatus::Cancelled);
    }

    #[test]
    fn llm_transport_cancellation_interrupts_real_header_wait() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_sender, accepted_receiver) = mpsc::channel();
        let response_written = Arc::new(AtomicBool::new(false));
        let server_response_written = Arc::clone(&response_written);
        let server = thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut accepted = None;
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok(connection) => {
                        accepted = Some(connection);
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return false,
                }
            }
            let Some((mut stream, _)) = accepted else {
                return false;
            };
            let _ = accepted_sender.send(());
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            thread::sleep(Duration::from_secs(2));
            server_response_written.store(true, Ordering::SeqCst);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
            true
        });
        let config = openai_config(format!("http://{address}/v1"));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let cancel_thread = thread::spawn(move || {
            let _ = accepted_receiver.recv_timeout(Duration::from_secs(2));
            cancel.cancel();
        });
        let result = runtime.block_on(async {
            ReqwestAsyncTransport
                .execute(
                    &config.transport_config(),
                    &config.api_key,
                    "prompt",
                    "{}",
                    cancellation,
                )
                .await
        });
        cancel_thread.join().unwrap();
        assert_eq!(result.status, LlmPatchSourceStatus::Cancelled);
        assert!(
            !response_written.load(Ordering::SeqCst),
            "cancellation must return before the delayed header response is written"
        );
        assert!(server.join().unwrap(), "loopback client never connected");
    }

    #[test]
    fn llm_transport_rejects_non_loopback_plain_http() {
        let config = openai_config("http://example.com/v1".to_string());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(ReqwestAsyncTransport.execute(
            &config.transport_config(),
            &config.api_key,
            "prompt",
            "{}",
            CancellationToken::new(),
        ));
        assert_eq!(
            result.error_code.as_deref(),
            Some("llm_patch_source.base_url_forbidden")
        );
    }

    #[test]
    fn llm_transport_config_is_cloneable_without_secret() {
        let config = openai_config("https://example.com/v1".to_string());
        let metadata = config.transport_config();
        let encoded = serde_json::to_string(&metadata.clone()).unwrap();
        assert!(!encoded.contains("transport-test-secret"));
        assert!(!encoded.contains("api_key"));
        assert!(format!("{config:?}").contains("[REDACTED]"));
    }
}
