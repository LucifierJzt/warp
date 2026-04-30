# TECH.md — GLM Assistant Panel Implementation

> Read PRODUCT.md first. This file assumes the product spec is approved.
>
> Recommended phasing: ship Phase 1 (basic chat) before deciding
> whether Phase 2 (block context) and Phase 3 (tools) are worth it.

## Architecture

```
┌────────────────────────────────────────────────────────────────┐
│  app/src/ai/glm/  (new module — fork-only)                     │
│                                                                 │
│  panel.rs        ← right-side ViewHandle, renders messages      │
│  conversation.rs ← Model: messages Vec, streaming state         │
│  client.rs       ← async OpenAI-compatible HTTP client (SSE)    │
│  settings.rs     ← API key/base URL/model persistence           │
│  types.rs        ← ChatMessage, ChatRequest, ChatChunk          │
│                                                                 │
│  Reuses (read-only):                                           │
│    - crates/http_client (reqwest 0.12 + eventsource)           │
│    - crates/warpui_extras/secure_storage (Keychain)            │
│    - crates/markdown_parser (response rendering)               │
│    - app/src/ui_components/* (button, input, scroll list)      │
└────────────────────────────────────────────────────────────────┘
                               │
                               ▼
            POST https://open.bigmodel.cn/api/coding/paas/v4/chat/completions
                  Authorization: Bearer <api_key>
                  Body: { model, messages, stream: true }
                  Response: text/event-stream (SSE)
```

The new module is **completely independent** of `app/src/ai/` and
`app/src/ai_assistant/`. Zero edits to those directories. We register
ourselves with the workspace as a sibling panel.

## Cargo Feature

In `app/Cargo.toml [features]` (next to `local_only`):

```toml
# FORK: enables the GLM Assistant panel — a parallel, fork-only AI
# panel that talks directly to 智谱 GLM Coding Plan via OpenAI
# protocol. Independent of Warp's existing Agent Mode.
# See specs/FORK-ZHIPU-AI/.
glm_assistant = []
```

**Not** added to `local_only`'s deps; the operator builds with
`--features local_only,glm_assistant`. PRODUCT.md §"Open Questions" 1
documents the rationale.

All new code is gated:

```rust
#[cfg(feature = "glm_assistant")]
pub mod glm;
```

## Dependencies (zero new crates if possible)

Required:

- `reqwest` (workspace, already used) — HTTP transport.
- `reqwest-eventsource` (already in `Cargo.toml`) — SSE streaming.
- `serde` / `serde_json` — request/response shapes.
- `tokio` — async runtime (already pervasive).
- `anyhow` — error handling.

No new external crates. **Avoid `async-openai`**: too heavy, drags in
its own auth/types model that doesn't quite fit.

## Phase 1 — Basic chat (target: 5–8 working days)

### Step 1.1 — Settings & secure storage (~1 day)

`app/src/ai/glm/settings.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlmSettings {
    pub base_url: String,        // default: https://open.bigmodel.cn/api/coding/paas/v4
    pub model: String,           // default: glm-4.6
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>, // default None (= server default)
    pub max_tokens: Option<u32>,
    // api_key NOT here — lives in Keychain.
}

const KEYCHAIN_KEY: &str = "GlmAssistantApiKey";

pub fn load_api_key(ctx: &AppContext) -> Option<String> { ... }
pub fn save_api_key(ctx: &AppContext, key: &str) -> Result<()> { ... }
pub fn clear_api_key(ctx: &AppContext) -> Result<()> { ... }
```

Settings (non-secret) persist via Warp's existing
`UserPreferences` system. API key persists via `secure_storage`
(Keychain on macOS).

**Verify:** save key → relaunch → key still loaded.

### Step 1.2 — HTTP client (~2 days, the meat)

`app/src/ai/glm/types.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole { System, User, Assistant }

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    pub choices: Vec<ChunkChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}
#[derive(Debug, Deserialize)]
pub struct ChunkChoice {
    pub delta: Delta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub content: Option<String>,
}
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

`app/src/ai/glm/client.rs`:

```rust
pub struct GlmClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

pub enum StreamEvent {
    Chunk(String),                  // delta text
    Usage(Usage),                   // final usage stats
    Done,
    Error(String),                  // user-facing message
}

impl GlmClient {
    pub fn new(http: reqwest::Client, base_url: String, api_key: String) -> Self;

    /// Returns a stream of StreamEvents. Caller can drop the receiver
    /// to cancel mid-stream (the underlying HTTP body drops too).
    pub fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> impl Stream<Item = StreamEvent>;
}
```

Implementation outline:

1. POST to `{base_url}/chat/completions` with bearer auth, `stream: true`.
2. Use `reqwest_eventsource::EventSource` to consume SSE.
3. For each `data:` line: if it equals `[DONE]` → emit `Done`.
   Otherwise parse as `ChatChunk`, emit `Chunk(delta.content)` and
   accumulate `usage` if present.
4. On HTTP 4xx/5xx: read body, emit `Error("{status}: {body}")`.
5. On network error: emit `Error("network: {err}")`.

**Verify with a unit test** that uses `mockito` (already a project
dep) to fake an SSE endpoint. Sequence: 3 chunks → final usage →
[DONE]. Asserts `chat_stream` emits exactly 5 events in order.

### Step 1.3 — Conversation model (~1 day)

`app/src/ai/glm/conversation.rs`:

```rust
pub struct GlmConversation {
    history: Vec<ChatMessage>,         // committed messages
    pending_assistant: Option<String>,  // streaming buffer
    state: ConversationState,
    last_usage: Option<Usage>,
}

pub enum ConversationState {
    Idle,
    Streaming { task_id: TaskId },     // so Stop button can cancel
    Error(String),
}

pub enum ConversationEvent {
    StreamProgress,    // pending_assistant grew
    StreamFinished,    // moved pending_assistant into history
    StateChanged,
}

impl warpui::Model for GlmConversation { ... }

impl GlmConversation {
    pub fn send(&mut self, user_msg: String, ctx: &mut ModelContext<Self>);
    pub fn stop(&mut self, ctx: &mut ModelContext<Self>);
    pub fn reset(&mut self, ctx: &mut ModelContext<Self>);
    pub fn messages_for_view(&self) -> impl Iterator<Item = MessageView>;
}
```

`send` flow:

1. Push `ChatMessage { role: User, content: user_msg }` to `history`.
2. Build `ChatRequest` from `history` + system prompt.
3. Spawn the stream as a task; store the task handle in
   `ConversationState::Streaming`.
4. On each `StreamEvent::Chunk(s)`, append to `pending_assistant`,
   emit `StreamProgress`.
5. On `StreamEvent::Done`, move `pending_assistant` into `history`
   as an `Assistant` message, save `last_usage`, emit
   `StreamFinished`, transition to `Idle`.
6. On `StreamEvent::Error(s)`, transition to `ConversationState::Error(s)`.

`stop` aborts the task and discards `pending_assistant`.

### Step 1.4 — Panel view (~2 days)

`app/src/ai/glm/panel.rs`:

- A `ViewHandle<GlmAssistantPanel>` with a `ViewHandle<GlmConversation>`.
- Layout (from top to bottom):
  - Header: "GLM Assistant · {model}" + settings gear icon.
  - Scrollable message list (reuse `app/src/ui_components` or
    `markdown_parser` rendering).
  - Footer:
    - If state is `Error`: red banner + retry button.
    - Streaming: "▍ generating..." + Stop button.
    - Idle: text input + Send button + token-count badge if
      `last_usage` is set.

Empty state (no API key configured): "Add your 智谱 API key to
get started" → links to Settings page.

### Step 1.5 — Wire into workspace (~1 day)

In `app/src/workspace/view.rs` (one of the few files we touch):

- Under `#[cfg(feature = "glm_assistant")]`, register a new
  `ViewHandle<GlmAssistantPanel>` next to the existing AI panel.
- Add a toolbar button + a keyboard shortcut (`⌘+⇧+G`).
- Add panel show/hide actions to the workspace action set.

Touch points:

- `build_*` constructors at workspace creation.
- The render function (add panel into the layout when visible).
- `WorkspaceAction` enum + `register_editable_bindings`.

These are the *only* edits in pre-existing files. Everything else
is in the new `app/src/ai/glm/` module.

### Step 1.6 — Settings page (~0.5 day)

In `app/src/settings_view/`, under feature flag, add a new section:

```
GLM Assistant
─────────────
API key:    [········]  [Reveal] [Clear]
Base URL:   [https://open.bigmodel.cn/api/coding/paas/v4]
Model:      [glm-4.6]
System:     [textarea for system prompt, optional]
Temperature: [0.7]
[Save]
```

### Step 1.7 — End-to-end smoke (~0.5 day)

Manual test plan:

1. `cargo run --bin warp-oss --features local_only,glm_assistant`.
2. Open Settings → GLM Assistant → paste API key + verify save.
3. Click GLM toolbar button or press `⌘+⇧+G`.
4. Send "写一个 Python 快排带类型注解" and watch tokens stream in.
5. Send follow-up "再加一个测试用例", verify multi-turn context.
6. Press Stop mid-stream, verify it actually stops and history
   stays intact.
7. Clear API key, send a message → verify error toast.
8. `lsof -i -P | grep warp-oss` → connection to
   `open.bigmodel.cn:443` only. No warp.dev.

## Phase 2 — Block context (target: 2–3 days, do later)

Ship after Phase 1 dogfooded for ~1 week.

- Right-click context menu on any terminal block:
  "Ask GLM about this block".
- Action: prepend `[The user is asking about this terminal output:\n
  ```\n{command}\n{stdout}\n```\n]\n` to the next prompt.
- New slash command `/glm <message>`: opens panel + sends.
- Suggested starter prompts in empty state.

## Phase 3 — Tools / file edits (DEFER, may not be worth it)

Only revisit if Phase 1+2 dogfooding shows operator regularly wants
the model to read files / propose patches. This phase essentially
reimplements Cline-style agent loop and is **2-3 weeks on top** of
Phases 1-2. Spec it separately as `FORK-GLM-AGENT` when/if needed.

Out of scope for this TECH.md.

## File-by-File Plan (Phase 1 only)

### New files

```
app/src/ai/glm/
  mod.rs               (~30 lines, re-exports)
  client.rs            (~250 lines)
  conversation.rs      (~250 lines)
  panel.rs             (~400 lines)
  settings.rs          (~150 lines)
  types.rs             (~120 lines)

specs/FORK-ZHIPU-AI/
  PRODUCT.md           (this commit)
  TECH.md              (this commit)
```

### Modified files (small surgical edits)

```
app/Cargo.toml
  +1 feature: glm_assistant = []

app/src/lib.rs
  +1 line under #[cfg]: pub mod ai_glm; (or wherever the module tree dictates)

app/src/workspace/view.rs
  +20 lines under #[cfg(feature = "glm_assistant")]:
  - Add panel ViewHandle
  - Add WorkspaceAction variant
  - Add toolbar button + shortcut binding

app/src/settings_view/mod.rs
  +1 line under #[cfg]: register GLM settings page

app/src/settings_view/glm_settings_page.rs (new but lives in settings_view)
  ~200 lines

feature-list.json
  M3-002 evidence updates

progress.md
  Session N entry
```

**Total: ~6 new files, ~5 light modifications. Zero changes to
existing AI / auth / server code.**

## Test Plan

### Unit tests (Phase 1)

In `app/src/ai/glm/`:

- `client::tests::stream_yields_chunks_in_order` — uses `mockito` to
  fake SSE response with 3 deltas + `[DONE]`, asserts events.
- `client::tests::http_error_emits_error_event` — fake 401 response,
  assert `StreamEvent::Error` contains `"401"`.
- `client::tests::network_error_does_not_panic` — point at
  `127.0.0.1:1` (closed port), assert `StreamEvent::Error`.
- `conversation::tests::multi_turn_context_includes_history` — fake
  client, check that the second `send` includes prior assistant
  message in the request payload.
- `conversation::tests::stop_aborts_in_flight_stream` — start stream,
  call stop, assert state is `Idle` and pending discarded.

Run: `cargo nextest run -p warp glm`

### Integration tests (Phase 2+)

Defer until Phase 1 ships. Then add a test that goes through the
panel's user flow end-to-end using `crates/integration/`.

### Manual smoke (Phase 1 acceptance)

See PRODUCT.md "Acceptance Criteria" — eight `- [ ]` items.

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
| ---- | ---------- | ---------- |
| 智谱 changes SSE chunk format | Low | Keep parser tolerant: ignore unknown JSON fields, log unknown events. |
| `reqwest-eventsource` doesn't handle 智谱's exact SSE encoding | Low-Medium | Add a fallback: parse `text/event-stream` manually if eventsource crate emits errors on the first tested response. Time-box this to half a day. |
| Tokio task leak when panel is closed mid-stream | Medium | Ensure `Drop` on `GlmConversation` aborts the streaming task. Add a unit test. |
| Conversation state grows unbounded over a long session | Medium | Hard cap at last N=20 turns; older turns get dropped (with a "history truncated" indicator). Phase 2 = optional summarization. |
| API key accidentally logged | Medium-High | Add a tracing layer that scrubs the `Authorization` header from logs. Audit all `tracing::info!`/`error!` for naked key inclusion. Add a unit test for log redaction. |
| Upstream rebase changes `app/src/workspace/view.rs` near our edits | Medium | Use `// FORK: glm_assistant` markers around all touch points; resolve-merge-conflicts skill handles them. |
| Operator's 智谱 plan throttles requests | Low | Show 429 errors verbatim in the panel; no auto-retry (would burn quota faster). |
| User accidentally commits the API key in their settings.yaml | Low | API key never in settings.yaml; only in Keychain. Verify by grep before each release: `git grep ZAI_KEY` etc. |

## Out of Scope (explicit deferrals)

- File-read / file-edit tools.
- MCP server integration.
- Codebase indexing / RAG.
- Local model routing (Ollama, llama.cpp).
- Anthropic-protocol path (智谱 supports it, but would double the
  code without functional gain on Phase 1).
- Conversation persistence to SQLite (Phase 2).
- Cost / quota dashboard beyond the per-message token badge.
- Replacing existing Warp AI panel — it stays dormant.
