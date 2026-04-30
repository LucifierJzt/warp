//! Streaming OpenAI-compatible HTTP client for 智谱 GLM Coding Plan.
//!
//! Wraps the project's `http_client::Client`, adds bearer-token auth,
//! posts JSON to `{base_url}/chat/completions` with `stream: true`, and
//! exposes the response as an [`async_stream::Stream`] of [`StreamEvent`]s.
//!
//! Cancellation: drop the returned stream. The underlying SSE connection
//! is closed on drop.
//!
//! Auth-key handling: the API key is held in this struct as `String` and
//! only ever passed via [`http_client::RequestBuilder::bearer_auth`] —
//! never logged. We only `log::warn!` shape/parse problems, never the
//! request body.

use std::sync::Arc;

use anyhow::Context;
use async_stream::stream;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use super::types::{ChatRequest, Usage};

/// Events yielded by [`GlmClient::chat_stream`].
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    /// SSE connection opened (model has acknowledged the request).
    Open,
    /// A delta of the assistant's response.
    Chunk(String),
    /// Final usage statistics from the upstream response. May arrive
    /// before [`StreamEvent::Done`].
    Usage(Usage),
    /// Stream completed cleanly (`data: [DONE]` was observed).
    Done,
    /// Terminal error. No further events will be produced after this.
    Error(String),
}

#[derive(Clone)]
pub struct GlmClient {
    http: Arc<http_client::Client>,
    chat_completions_url: String,
    api_key: String,
}

/// Provider's error envelope. Both `code` and `message` are best-effort —
/// 智谱 may return a plain `{"error": {"message": ...}}` or a flatter
/// shape; we tolerate both.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ProviderErrorBody {
    error: Option<ProviderErrorInner>,
    code: Option<serde_json::Value>,
    message: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ProviderErrorInner {
    message: Option<String>,
    code: Option<serde_json::Value>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

impl ProviderErrorBody {
    fn user_message(&self) -> Option<String> {
        if let Some(inner) = &self.error {
            if let Some(msg) = &inner.message {
                return Some(msg.clone());
            }
        }
        self.message.clone()
    }
}

impl GlmClient {
    pub fn new(
        http: Arc<http_client::Client>,
        chat_completions_url: String,
        api_key: String,
    ) -> Self {
        Self {
            http,
            chat_completions_url,
            api_key,
        }
    }

    /// Issues a streaming chat completion. The returned stream yields
    /// [`StreamEvent`]s until it terminates with either `Done` or
    /// `Error(_)`.
    pub fn chat_stream(&self, request: ChatRequest) -> impl Stream<Item = StreamEvent> + Send {
        let http = self.http.clone();
        let url = self.chat_completions_url.clone();
        let api_key = self.api_key.clone();

        stream! {
            let mut event_source = http
                .post(url)
                .bearer_auth(&api_key)
                .json(&request)
                .prevent_sleep("GLM Assistant request in-flight")
                .eventsource();

            while let Some(event) = event_source.next().await {
                match event {
                    Ok(reqwest_eventsource::Event::Open) => {
                        yield StreamEvent::Open;
                    }
                    Ok(reqwest_eventsource::Event::Message(message)) => {
                        let data = message.data.trim();
                        if data.is_empty() {
                            continue;
                        }
                        if data == "[DONE]" {
                            yield StreamEvent::Done;
                            return;
                        }
                        match parse_chunk(data) {
                            Ok(events) => {
                                for ev in events {
                                    yield ev;
                                }
                            }
                            Err(err) => {
                                log::warn!(
                                    "GLM Assistant: skipping malformed SSE chunk: {err:#}",
                                );
                            }
                        }
                    }
                    Err(err) => {
                        yield StreamEvent::Error(format_sse_error(&err));
                        return;
                    }
                }
            }

            yield StreamEvent::Done;
        }
    }
}

fn parse_chunk(data: &str) -> anyhow::Result<Vec<StreamEvent>> {
    if let Ok(err_body) = serde_json::from_str::<ProviderErrorBody>(data) {
        if let Some(msg) = err_body.user_message() {
            return Ok(vec![StreamEvent::Error(format!("provider error: {msg}"))]);
        }
    }

    let chunk: super::types::ChatChunk =
        serde_json::from_str(data).context("decoding GLM SSE chunk")?;
    let mut out = Vec::with_capacity(2);
    for choice in &chunk.choices {
        if let Some(delta) = &choice.delta.content {
            if !delta.is_empty() {
                out.push(StreamEvent::Chunk(delta.clone()));
            }
        }
    }
    if let Some(usage) = chunk.usage {
        out.push(StreamEvent::Usage(usage));
    }
    Ok(out)
}

fn format_sse_error(err: &reqwest_eventsource::Error) -> String {
    use reqwest_eventsource::Error;
    match err {
        Error::InvalidStatusCode(status, _) => {
            format!("HTTP {status}: check your API key, base URL, and model name")
        }
        Error::InvalidContentType(_, _) => {
            "unexpected response content type — is the base URL correct?".to_string()
        }
        Error::Transport(transport) => format!("network: {transport}"),
        Error::Utf8(utf8) => format!("decoding error: {utf8}"),
        Error::Parser(parser) => format!("malformed SSE response: {parser}"),
        Error::StreamEnded => "stream ended unexpectedly".to_string(),
        other => format!("{other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glm::types::ChatMessage;
    use futures::executor::block_on;
    use std::sync::Arc;

    fn make_client(server_url: String, api_key: &str) -> GlmClient {
        GlmClient::new(
            Arc::new(http_client::Client::new_for_test()),
            format!("{server_url}/chat/completions"),
            api_key.to_string(),
        )
    }

    fn sample_request() -> ChatRequest {
        ChatRequest {
            model: "glm-4.6".into(),
            messages: vec![ChatMessage::user("hi")],
            stream: true,
            temperature: None,
            max_tokens: None,
        }
    }

    fn sse_chunk(payload: &str) -> String {
        format!("data: {payload}\n\n")
    }

    #[test]
    fn happy_path_emits_chunks_usage_done_in_order() {
        block_on(async {
            let mut server = mockito::Server::new_async().await;
            let body = format!(
                "{}{}{}{}",
                sse_chunk(r#"{"choices":[{"delta":{"role":"assistant"},"index":0}]}"#),
                sse_chunk(r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#),
                sse_chunk(r#"{"choices":[{"delta":{"content":" world"},"index":0}]}"#),
                sse_chunk(
                    r#"{"choices":[{"delta":{},"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#
                ),
            );
            let body = format!("{body}{}", "data: [DONE]\n\n");

            let mock = server
                .mock("POST", "/chat/completions")
                .match_header("authorization", "Bearer test-key")
                .match_header("content-type", "application/json")
                .with_status(200)
                .with_header("content-type", "text/event-stream")
                .with_body(body)
                .create_async()
                .await;

            let client = make_client(server.url(), "test-key");
            let stream = client.chat_stream(sample_request());
            let events: Vec<StreamEvent> = stream.collect().await;

            mock.assert_async().await;
            assert_eq!(
                events,
                vec![
                    StreamEvent::Open,
                    StreamEvent::Chunk("Hello".into()),
                    StreamEvent::Chunk(" world".into()),
                    StreamEvent::Usage(Usage {
                        prompt_tokens: 5,
                        completion_tokens: 2,
                        total_tokens: 7,
                    }),
                    StreamEvent::Done,
                ]
            );
        });
    }

    #[test]
    fn http_401_terminates_with_error() {
        block_on(async {
            let mut server = mockito::Server::new_async().await;
            let _mock = server
                .mock("POST", "/chat/completions")
                .with_status(401)
                .with_header("content-type", "application/json")
                .with_body(r#"{"error":{"message":"invalid api key"}}"#)
                .create_async()
                .await;

            let client = make_client(server.url(), "bad");
            let mut stream = Box::pin(client.chat_stream(sample_request()));

            let mut last = None;
            while let Some(ev) = stream.next().await {
                last = Some(ev);
            }
            let last = last.expect("at least one event");
            match last {
                StreamEvent::Error(msg) => {
                    assert!(
                        msg.contains("HTTP 401") || msg.to_lowercase().contains("api key"),
                        "expected auth error message, got: {msg}",
                    );
                }
                other => panic!("expected Error, got {other:?}"),
            }
        });
    }

    #[test]
    fn unknown_chunk_fields_are_tolerated() {
        block_on(async {
            let mut server = mockito::Server::new_async().await;
            let body = format!(
                "{}{}",
                sse_chunk(
                    r#"{"choices":[{"delta":{"content":"ok","unknown_delta_field":42},"index":0}],"unknown_top":true}"#,
                ),
                "data: [DONE]\n\n",
            );
            let _mock = server
                .mock("POST", "/chat/completions")
                .with_status(200)
                .with_header("content-type", "text/event-stream")
                .with_body(body)
                .create_async()
                .await;

            let client = make_client(server.url(), "k");
            let events: Vec<StreamEvent> = client.chat_stream(sample_request()).collect().await;
            assert!(events.contains(&StreamEvent::Chunk("ok".into())));
            assert!(events.contains(&StreamEvent::Done));
        });
    }

    #[test]
    fn empty_data_lines_are_skipped() {
        block_on(async {
            let mut server = mockito::Server::new_async().await;
            let body = "\
                data: \n\n\
                data: {\"choices\":[{\"delta\":{\"content\":\"x\"},\"index\":0}]}\n\n\
                data: [DONE]\n\n";
            let _mock = server
                .mock("POST", "/chat/completions")
                .with_status(200)
                .with_header("content-type", "text/event-stream")
                .with_body(body)
                .create_async()
                .await;

            let client = make_client(server.url(), "k");
            let events: Vec<StreamEvent> = client.chat_stream(sample_request()).collect().await;
            assert!(events.contains(&StreamEvent::Chunk("x".into())));
            assert!(events.contains(&StreamEvent::Done));
        });
    }
}
