//! Wire-level types for the OpenAI-compatible chat-completions API
//! exposed by 智谱 GLM Coding Plan
//! (`https://open.bigmodel.cn/api/coding/paas/v4/chat/completions`).
//!
//! The shapes are intentionally minimal: only fields we actually
//! send or read. Unknown fields are ignored on deserialize so the
//! upstream provider can add new fields without breaking us.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// Request body for `POST {base_url}/chat/completions`.
#[derive(Clone, Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// One streaming chunk from a `stream: true` response. The provider sends a
/// sequence of these as SSE `data:` events, terminated by a literal `[DONE]`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChatChunk {
    pub choices: Vec<ChunkChoice>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChunkChoice {
    pub delta: Delta,
    pub finish_reason: Option<String>,
    pub index: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Delta {
    pub content: Option<String>,
    pub role: Option<ChatRole>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_helpers_set_role() {
        assert_eq!(ChatMessage::user("hi").role, ChatRole::User);
        assert_eq!(ChatMessage::system("sys").role, ChatRole::System);
        assert_eq!(ChatMessage::assistant("a").role, ChatRole::Assistant);
    }

    #[test]
    fn chat_request_skips_none_optional_fields() {
        let req = ChatRequest {
            model: "glm-4.6".into(),
            messages: vec![ChatMessage::user("hi")],
            stream: true,
            temperature: None,
            max_tokens: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("temperature"));
        assert!(!json.contains("max_tokens"));
        assert!(json.contains("\"stream\":true"));
        assert!(json.contains("\"model\":\"glm-4.6\""));
    }

    #[test]
    fn chat_chunk_tolerates_unknown_fields() {
        let raw = r#"{
            "id": "ignored",
            "object": "chat.completion.chunk",
            "model": "glm-4.6",
            "choices": [
                {"index": 0, "delta": {"role": "assistant", "content": "hello"}, "finish_reason": null}
            ],
            "future_field": {"nested": true}
        }"#;
        let chunk: ChatChunk = serde_json::from_str(raw).expect("parses despite unknown fields");
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("hello"));
    }

    #[test]
    fn chat_chunk_handles_empty_delta() {
        let raw = r#"{"choices":[{"delta":{},"finish_reason":"stop","index":0}]}"#;
        let chunk: ChatChunk = serde_json::from_str(raw).unwrap();
        assert_eq!(chunk.choices[0].delta.content, None);
        assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn usage_chunk_parses() {
        let raw = r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":42,"total_tokens":52}}"#;
        let chunk: ChatChunk = serde_json::from_str(raw).unwrap();
        let usage = chunk.usage.expect("usage present");
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 42);
        assert_eq!(usage.total_tokens, 52);
    }
}
