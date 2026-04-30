//! Multi-turn conversation state for the GLM Assistant panel.
//!
//! This `Model` owns the canonical chat history. It exposes one
//! "send a user message and stream the assistant reply" entry point
//! and a small set of getters for the panel view.
//!
//! Architectural notes:
//! - History is stored in a single `Vec<ChatMessage>`. The system
//!   prompt (if any) is *not* in `history` — it's prepended at
//!   request-build time so the operator can edit it without
//!   invalidating prior turns.
//! - While a request is in flight, `pending_assistant` accumulates
//!   delta tokens. On stream completion it is moved into `history`
//!   as the assistant's full reply.
//! - To keep the implementation small and reviewable, this first
//!   cut collects the *entire* response before notifying the panel.
//!   Token-level streaming UX (Step 1.3.5) builds on top once
//!   the panel has a stable ctx handle to work against.
//! - Cancellation is via `AbortHandle`. Dropping the model also
//!   aborts.

use std::sync::Arc;

use futures::stream::AbortHandle;
use futures::StreamExt;
use warpui::{Entity, ModelContext};

use super::client::{GlmClient, StreamEvent};
use super::settings::GlmSettings;
use super::types::{ChatMessage, ChatRequest, Usage};

const HISTORY_MAX_TURNS: usize = 20;

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Idle,
    InFlight,
    Error(String),
}

impl State {
    pub fn is_in_flight(&self) -> bool {
        matches!(self, Self::InFlight)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// History or pending state changed; views should re-render.
    Changed,
    /// Stream finished cleanly. Carried for consumers that want
    /// to react (e.g., scroll-to-bottom, toast).
    Finished { usage: Option<Usage> },
}

pub struct GlmConversation {
    client: Arc<GlmClient>,
    settings: GlmSettings,
    history: Vec<ChatMessage>,
    pending_assistant: Option<String>,
    in_flight_abort: Option<AbortHandle>,
    state: State,
    last_usage: Option<Usage>,
}

impl Entity for GlmConversation {
    type Event = Event;
}

impl GlmConversation {
    pub fn new(client: Arc<GlmClient>, settings: GlmSettings) -> Self {
        Self {
            client,
            settings,
            history: Vec::new(),
            pending_assistant: None,
            in_flight_abort: None,
            state: State::Idle,
            last_usage: None,
        }
    }

    pub fn history(&self) -> &[ChatMessage] {
        &self.history
    }

    pub fn pending_assistant(&self) -> Option<&str> {
        self.pending_assistant.as_deref()
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn last_usage(&self) -> Option<Usage> {
        self.last_usage
    }

    pub fn settings(&self) -> &GlmSettings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: GlmSettings, ctx: &mut ModelContext<Self>) {
        self.settings = settings;
        ctx.emit(Event::Changed);
        ctx.notify();
    }

    pub fn replace_client(&mut self, client: Arc<GlmClient>) {
        self.client = client;
    }

    /// Clears history and any pending state. If a request is in flight
    /// it is aborted. Equivalent to "Restart" / "New chat".
    pub fn reset(&mut self, ctx: &mut ModelContext<Self>) {
        self.cancel_in_flight();
        self.history.clear();
        self.pending_assistant = None;
        self.last_usage = None;
        self.state = State::Idle;
        ctx.emit(Event::Changed);
        ctx.notify();
    }

    /// Cancels the in-flight request, if any. Discards any partially
    /// accumulated assistant text. Idempotent.
    pub fn cancel_in_flight(&mut self) {
        if let Some(handle) = self.in_flight_abort.take() {
            handle.abort();
        }
        self.pending_assistant = None;
        if self.state.is_in_flight() {
            self.state = State::Idle;
        }
    }

    /// Sends a user message and starts streaming the assistant reply.
    /// Returns immediately; the reply is delivered asynchronously and
    /// surfaced via `Event::Changed` / `Event::Finished` and ctx.notify().
    pub fn send(&mut self, message: String, ctx: &mut ModelContext<Self>) {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.state.is_in_flight() {
            log::warn!("GLM Assistant: ignoring send() while a request is already in flight");
            return;
        }

        self.history.push(ChatMessage::user(trimmed));
        self.truncate_history_if_needed();
        self.pending_assistant = Some(String::new());
        self.state = State::InFlight;
        self.last_usage = None;
        ctx.emit(Event::Changed);
        ctx.notify();

        let request = self.build_request();
        let client = self.client.clone();

        let future = async move { collect_response(client, request).await };

        let handle = ctx.spawn(future, |this: &mut Self, outcome, ctx| {
            this.in_flight_abort = None;
            match outcome {
                Ok(StreamOutcome { content, usage }) => {
                    let final_text = match this.pending_assistant.take() {
                        Some(buf) if !buf.is_empty() => buf,
                        _ => content,
                    };
                    if !final_text.is_empty() {
                        this.history.push(ChatMessage::assistant(final_text));
                        this.truncate_history_if_needed();
                    }
                    this.last_usage = usage;
                    this.state = State::Idle;
                    ctx.emit(Event::Finished { usage });
                    ctx.emit(Event::Changed);
                    ctx.notify();
                }
                Err(err) => {
                    this.pending_assistant = None;
                    this.state = State::Error(err);
                    ctx.emit(Event::Changed);
                    ctx.notify();
                }
            }
        });
        self.in_flight_abort = Some(handle.abort_handle());
    }

    fn build_request(&self) -> ChatRequest {
        let mut messages = Vec::with_capacity(self.history.len() + 1);
        if let Some(prompt) = &self.settings.system_prompt {
            let trimmed = prompt.trim();
            if !trimmed.is_empty() {
                messages.push(ChatMessage::system(trimmed));
            }
        }
        messages.extend(self.history.iter().cloned());

        ChatRequest {
            model: self.settings.model.clone(),
            messages,
            stream: true,
            temperature: self.settings.temperature,
            max_tokens: self.settings.max_tokens,
        }
    }

    fn truncate_history_if_needed(&mut self) {
        let max = HISTORY_MAX_TURNS * 2;
        if self.history.len() > max {
            let drop_count = self.history.len() - max;
            self.history.drain(0..drop_count);
        }
    }
}

impl Drop for GlmConversation {
    fn drop(&mut self) {
        if let Some(handle) = self.in_flight_abort.take() {
            handle.abort();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct StreamOutcome {
    content: String,
    usage: Option<Usage>,
}

async fn collect_response(
    client: Arc<GlmClient>,
    request: ChatRequest,
) -> Result<StreamOutcome, String> {
    let mut stream = Box::pin(client.chat_stream(request));
    let mut content = String::new();
    let mut usage = None;

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Open => {}
            StreamEvent::Chunk(delta) => content.push_str(&delta),
            StreamEvent::Usage(u) => usage = Some(u),
            StreamEvent::Done => return Ok(StreamOutcome { content, usage }),
            StreamEvent::Error(msg) => return Err(msg),
        }
    }

    Ok(StreamOutcome { content, usage })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> GlmSettings {
        GlmSettings::default()
    }

    fn dummy_client() -> Arc<GlmClient> {
        Arc::new(GlmClient::new(
            Arc::new(http_client::Client::new_for_test()),
            "http://127.0.0.1:1/chat/completions".into(),
            "test-key".into(),
        ))
    }

    #[test]
    fn build_request_includes_system_prompt_then_history() {
        let mut conv = GlmConversation::new(dummy_client(), settings());
        conv.settings.system_prompt = Some("you are helpful".to_string());
        conv.history.push(ChatMessage::user("hi"));
        conv.history.push(ChatMessage::assistant("hello"));
        conv.history.push(ChatMessage::user("again"));

        let req = conv.build_request();
        assert_eq!(req.messages.len(), 4);
        assert_eq!(req.messages[0].content, "you are helpful");
        assert_eq!(req.messages[1].content, "hi");
        assert_eq!(req.messages[2].content, "hello");
        assert_eq!(req.messages[3].content, "again");
    }

    #[test]
    fn build_request_omits_blank_system_prompt() {
        let mut conv = GlmConversation::new(dummy_client(), settings());
        conv.settings.system_prompt = Some("   \n   ".to_string());
        conv.history.push(ChatMessage::user("hi"));

        let req = conv.build_request();
        assert_eq!(req.messages.len(), 1, "only the user message");
        assert_eq!(req.messages[0].content, "hi");
    }

    #[test]
    fn truncate_history_caps_at_max_turns_pairs() {
        let mut conv = GlmConversation::new(dummy_client(), settings());
        for i in 0..(HISTORY_MAX_TURNS * 2 + 5) {
            conv.history.push(ChatMessage::user(format!("u{i}")));
        }
        conv.truncate_history_if_needed();
        assert_eq!(conv.history.len(), HISTORY_MAX_TURNS * 2);
        assert!(conv.history.first().unwrap().content.starts_with("u5"));
    }

    #[tokio::test]
    async fn collect_response_aggregates_chunks_and_usage() {
        use crate::glm::client::GlmClient;
        use std::sync::Arc;

        let mut server = mockito::Server::new_async().await;
        let body = "\
            data: {\"choices\":[{\"delta\":{\"content\":\"foo\"},\"index\":0}]}\n\n\
            data: {\"choices\":[{\"delta\":{\"content\":\" bar\"},\"index\":0}]}\n\n\
            data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"total_tokens\":3}}\n\n\
            data: [DONE]\n\n";
        let _mock = server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(body)
            .create_async()
            .await;

        let client = Arc::new(GlmClient::new(
            Arc::new(http_client::Client::new_for_test()),
            format!("{}/chat/completions", server.url()),
            "k".into(),
        ));
        let req = ChatRequest {
            model: "glm-4.6".into(),
            messages: vec![ChatMessage::user("hi")],
            stream: true,
            temperature: None,
            max_tokens: None,
        };
        let outcome = collect_response(client, req).await.unwrap();
        assert_eq!(outcome.content, "foo bar");
        assert_eq!(outcome.usage.unwrap().total_tokens, 3);
    }

    #[tokio::test]
    async fn collect_response_surfaces_provider_error() {
        use crate::glm::client::GlmClient;
        use std::sync::Arc;

        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/chat/completions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":{"message":"bad key"}}"#)
            .create_async()
            .await;

        let client = Arc::new(GlmClient::new(
            Arc::new(http_client::Client::new_for_test()),
            format!("{}/chat/completions", server.url()),
            "k".into(),
        ));
        let req = ChatRequest {
            model: "glm-4.6".into(),
            messages: vec![ChatMessage::user("hi")],
            stream: true,
            temperature: None,
            max_tokens: None,
        };
        let err = collect_response(client, req).await.unwrap_err();
        assert!(
            err.contains("HTTP 401") || err.to_lowercase().contains("api key"),
            "got: {err}"
        );
    }
}
