# PRODUCT.md — GLM Assistant Panel (智谱直连侧栏)

> **Status:** proposed (not yet implemented)
> **Owner:** internal fork (LucifierJzt/warp)
> **Tracker entry:** `feature-list.json :: M3-002`
> **Depends on:** `M3-001` (FORK-LOCAL-AUTH) shipped — fork already
> runs in `local_only` mode without warp-server.

## TL;DR

Add a **new** right-side AI panel called **"GLM Assistant"** that
talks **directly** to 智谱 GLM Coding Plan via its OpenAI-compatible
endpoint. Coexists with Warp's existing Agent Mode (which is dormant
in `local_only` builds because warp-server is unreachable).

This is **not** an attempt to "replace Claude with GLM" inside Warp's
existing Agent Mode. That path was investigated and rejected — see
"Why we are not modifying Warp's Agent Mode" below.

## Why This Spec Exists

Naive reading of the request "把 Claude 的 API 换成 GLM" suggests
just changing a base URL and an API key. **That option does not
exist in Warp's client code.** Discovered during research:

- Warp's `AIClient` trait (`app/src/server/server_api/ai.rs:740`)
  has 30+ methods, all sending **Warp-private GraphQL mutations**
  (`generateDialogue`, `generateCommands`, `createAgentTask`,
  `updateMerkleTree`, etc.) to `app.warp.dev/graphql/v2`.
- The Anthropic/OpenAI HTTP calls only happen **server-side** on
  warp-server. The client never knew an Anthropic URL existed.
- 全仓 ripgrep `api.openai.com|api.anthropic.com|googleapis` —
  zero matches. Confirms client never speaks those protocols.

So "swap base URL" is impossible without either:

1. Reimplementing 30+ AIClient methods against OpenAI protocol
   (~2-3 weeks, ~50% of methods can't be implemented).
2. Building a local GraphQL→OpenAI translation proxy (~3-4 weeks,
   ongoing maintenance against upstream schema drift).
3. **This spec** — a parallel, simple, OpenAI-compatible chat panel
   that we fully own. Cline / Roo / Cursor all chose this same path.

## Desired Behavior

When the operator runs the fork build with `--features local_only`
(see `M3-001`):

1. **Existing Warp Agent Mode** is dormant (its requests `bail!`
   because `Credentials::Test` returns `AuthToken::NoAuth`). Visible
   but non-functional. **We accept this** — it's the price of being
   off warp-server.
2. **A new "GLM Assistant" panel** is reachable via:
   - A new toolbar button next to the existing Warp AI button.
   - A new keyboard shortcut (e.g. `⌘+⇧+G`).
   - Optionally a new slash command `/glm`.
3. **First-run experience:**
   - Panel opens with an "Add API key" empty state.
   - User pastes their 智谱 API key (from `https://bigmodel.cn/usercenter/proj-mgmt/apikeys`).
   - Optionally edits Base URL (default `https://open.bigmodel.cn/api/coding/paas/v4`).
   - Optionally picks a model (default `glm-4.6`, free-text editable).
   - Settings stored in macOS Keychain via `crates/warpui_extras/secure_storage`.
4. **Steady-state usage:**
   - Multi-turn chat with persistent history (per session, optional
     per-workspace).
   - Streaming output (SSE) — tokens appear word-by-word.
   - Markdown rendering with code-block syntax highlighting (reuses
     existing renderer).
   - Copy-button on code blocks.
   - **Stop / regenerate** buttons.
   - Show the configured model name in the input area.
5. **Block-context awareness (Phase 2, optional):**
   - Right-click a terminal block → "Ask GLM about this".
   - The block's command + output gets prepended to the next prompt.
6. **Errors are user-visible:**
   - Bad API key → "Auth failed (401), check your key in Settings".
   - Network error → "Cannot reach `<host>`, check your network".
   - Quota exhausted → forward 智谱 server's error message.
   - No silent failures, no hangs.

### Out of Scope (explicit non-goals)

- Touching Warp's existing Agent Mode, AIClient trait, or
  `warp_multi_agent_api`. Zero changes to those paths.
- File-edit tools (Cline-style automatic code modification). Phase 3+
  if ever — first prove the basic chat works.
- MCP server integration. Phase 3+.
- Codebase indexing / embeddings.
- Skills system (Warp's bundled skills require warp-server).
- Conversation sharing / cloud history.
- Replacing the Anthropic protocol path — even if 智谱 supports
  Anthropic Messages API, going that route still requires the same
  HTTP-client work and gives no advantage over OpenAI protocol.
- Multi-provider abstraction (OpenAI + Anthropic + Google + ...). The
  fork only needs 智谱; YAGNI on a generic provider system.

## Acceptance Criteria

- [ ] `cargo run --bin warp-oss --features local_only` opens the app.
- [ ] User can open the new GLM Assistant panel via toolbar or shortcut.
- [ ] Empty state prompts for API key; key persists across restarts in
      macOS Keychain.
- [ ] User sends "写一个 Python 快排" → tokens stream into the panel
      within 2s of the request, full answer arrives.
- [ ] `lsof -i -P | grep warp-oss` shows the connection going to
      `open.bigmodel.cn:443` (and only there — no warp.dev).
- [ ] Multi-turn: ask "再加上类型注解", model receives prior turn as
      context, returns updated code.
- [ ] Code blocks in the response render with syntax highlighting and
      a copy button that puts content on the system clipboard.
- [ ] Streaming can be cancelled mid-response with a Stop button.
- [ ] Wrong API key produces a visible error toast within 5s of send.
- [ ] No panic / no hang reachable through the panel UI (manual fuzzing
      with empty input, very long input, rapid send-cancel-send, etc.).

## User-Visible Surface Changes

| Surface | Today (after M3-001) | After this spec |
| ------- | -------------------- | --------------- |
| Toolbar | Warp AI button (dormant) | Warp AI button (dormant) + GLM Assistant button (active) |
| Right-side panel | Warp AI panel opens but requests fail | + new GLM Assistant panel that actually works |
| Keyboard shortcut | `⌘+\` for Warp AI (dormant) | `⌘+\` (dormant) + `⌘+⇧+G` for GLM |
| Settings | Account / Privacy / Theme / ... | + new "GLM Assistant" section: API key, base URL, model, system prompt |
| Slash commands | `/help`, `/host`, `/harness` ... | + `/glm <message>` (Phase 2) |
| Settings file | `settings.yaml` | + `glm_assistant.{base_url, model, system_prompt}` (key in Keychain) |

## Dependencies on Existing Code

We will **read** these but not modify their behavior:

- `crates/http_client/src/lib.rs` — reuse the project's HTTP client
  (already has `reqwest 0.12` + `reqwest-eventsource 0.6` for SSE).
- `crates/warpui_extras/secure_storage` — for API key persistence.
- `app/src/ai_assistant/panel.rs` — read for layout patterns; copy
  the structural skeleton, not the warp-server wiring.
- `crates/markdown_parser/` — reuse for response rendering.
- Existing keyboard-shortcut registration pattern in
  `app/src/workspace/`.

We will **add** (not modify):

- `app/src/ai/glm/` — new module with provider, conversation model,
  panel view, settings.
- `app/Cargo.toml` — one new feature flag `glm_assistant`.

## Open Questions for the Operator

Decide before TECH.md is finalized:

1. **Auto-enable with `local_only`?**
   - (a) `local_only = ["skip_login", "glm_assistant"]` — anyone
       building local mode gets GLM panel for free.
   - (b) Keep separate: `local_only` and `glm_assistant` as two
       independent features. Build with `--features local_only,glm_assistant`.
   - **Default recommendation: (b)**. Keeps each fork concern atomic;
     doesn't conflate auth bypass with AI provider choice.

2. **Should base URL be configurable?**
   - (a) Hard-code `https://open.bigmodel.cn/api/coding/paas/v4` in
       the binary. User can only change API key.
   - (b) Editable in Settings (default to the above).
   - **Recommendation: (b)**, costs ~10 extra lines. Lets you point
     at a self-hosted gateway later, or test against the staging URL.

3. **Where do API keys live?**
   - (a) macOS Keychain via existing `secure_storage` — encrypted at
       rest, auto-cleared on user logout.
   - (b) Plaintext in settings.yaml — simpler but leaks on backup.
   - **Recommendation: (a)**, the project already has the pattern.

4. **First-message UX:**
   - (a) Show empty input + system-prompt placeholder.
   - (b) Show 4-6 suggested starter prompts ("Explain my last
       command", "Fix this error", "Generate a regex for...").
   - **Recommendation: (a) for Phase 1**, defer (b) to Phase 2.

5. **Handle conversation history persistence:**
   - (a) In-memory only — closing the panel loses context.
   - (b) Persisted to local SQLite (Warp already has `crates/persistence/`).
   - **Recommendation: (a) for Phase 1** — ship the chat first, add
     persistence in Phase 2 once we know the data shape we want.

6. **Token / cost display:**
   - 智谱 returns `usage.total_tokens` in each response. Show running
     total in panel footer? Yes/no?
   - **Recommendation: yes** — operator wants to know they're not
     burning their Coding Plan quota.

## Why we are not modifying Warp's Agent Mode

For posterity (so future sessions / agents don't re-litigate):

| Approach | Effort | Verdict |
| -------- | ------ | ------- |
| Replace `impl AIClient for ServerApi` with `impl AIClient for ZhipuAIClient` | 2-3 weeks; ~15 of 30 methods (`spawn_agent`, `update_merkle_tree`, `get_block_snapshot`, `list_ai_conversation_metadata`, `create_file_artifact_upload_target`, ...) have no OpenAI-protocol equivalent and would NotImplemented | Rejected |
| Build a local GraphQL→OpenAI proxy that pretends to be warp-server | 3-4 weeks + ongoing maintenance against upstream `warp-proto-apis` schema drift; debugging requires understanding both protocols simultaneously | Rejected |
| Run a self-hosted clone of warp-server | 3-6 months; warp-server is closed-source | Rejected |
| **Add a parallel panel that owns its own protocol stack** (this spec) | 1-2 weeks; full control; aligns with how Cline/Roo/Cursor approached the same question | **Accepted** |

The cost of preserving "Agent Mode UI but powered by GLM" is
approximately 10x the cost of building a fresh, simple, working chat
panel. The operator confirmed Phase 1 = chat-only. Tools (file edits,
MCP, etc.) are explicitly Phase 3+ and may not be needed at all.

## Related Specs

- `specs/FORK-LOCAL-AUTH/` — `M3-001`, prerequisite (no warp-server).
- `specs/APP-3679/PRODUCT.md` — recent Warp AI changes (skim, in case
  upstream introduces conflicting auth flows we'll have to resolve).
- (future) `FORK-GLM-AGENT` — if Phase 1 chat panel proves valuable
  enough to want file-edit tooling.
- (future) `FORK-MCP-CLIENT` — independent of GLM, but related; if
  the operator later wants MCP servers to be reachable from the fork.
