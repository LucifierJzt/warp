# ARCHITECTURE.md

> Code map for **二次开发 (internal fork)** on macOS.
> Read this with `WARP.md` (commands + style) and `AGENTS.md` (rules).
> Goal: cut the time from "I want to change behavior X" to "I know which
> file to open" from hours to minutes.

## 30-Second Mental Model

Warp is a **Rust-only** desktop terminal app. There is **no Electron, no
web layer**. The UI is a custom retained-mode framework called
**WarpUI** (Flutter-inspired, Entity-Handle pattern), rendered on the
GPU via WGPU.

```
┌──────────────────────────────────────────────────────────────────┐
│  app/  ← business logic (terminal, AI, drive, settings, ...)     │
│   uses                                                            │
│  crates/warpui*  ← retained-mode UI framework (Entity-Handle)    │
│   uses                                                            │
│  crates/warp_core / warp_terminal / warp_completer / ...         │
│   uses                                                            │
│  crates/graphql / warp_server_client  ← backend integration      │
└──────────────────────────────────────────────────────────────────┘
```

- **Process model**: single binary, multiple internal "channels"
  (`oss`, `local`, `dev`, `stable`, `preview`) — each is a separate
  `[[bin]]` in `app/Cargo.toml`. They share the same `warp::run()`
  entry point but configure feature flags / server URLs / app id
  differently.
- **Default `cargo run` binary**: `warp` → `app/src/bin/local.rs`. For
  internal forks the **`oss` binary** (`app/src/bin/oss.rs`) is the
  cleanest starting point — it has no telemetry, no autoupdater, no
  crash reporting.

## Top-Level Layout

| Path | What lives there |
| ---- | ---------------- |
| `app/` | The product. Almost all user-visible behavior is here. |
| `app/src/lib.rs` | Wires the channels together; exposes `warp::run()`. |
| `app/src/bin/*.rs` | One file per release channel = one `[[bin]]`. |
| `crates/warpui/`, `crates/warpui_core/`, `crates/warpui_extras/` | Custom UI framework. Don't fork unless you must. |
| `crates/warp_core/` | App identity, channels, feature flags, paths. |
| `crates/warp_terminal/` | PTY, shell, ANSI, command parsing core. |
| `crates/warp_completer/` | Shell autocompletion engine (v1 + v2). |
| `crates/ai/` | Model-side AI plumbing (used by `app/ai/`). |
| `crates/graphql/`, `crates/warp_server_client/`, `crates/warp_graphql_schema/` | Talks to `warp-server`. |
| `crates/integration/` | E2E tests using the Builder/TestStep harness. |
| `crates/command-signatures-v2/` | **Has a JS build step** (Node + corepack). Skip with default `cargo run`; not a default member. |
| `specs/` | 129+ feature specs (`APP-XXXX/` folders). Authoritative for behavior. Read these before changing a feature. |
| `script/` | Bootstrap, presubmit, install helpers, packaging. |
| `.warp/` | Warp-specific dev metadata (`launch_configurations`, etc.). |
| `resources/` | Fonts, icons, themes, embedded assets. |

## `app/src/` — Where Most Edits Land

The ~100 modules under `app/src/` are organized by **product surface**.
When asked to change a feature, this is your first lookup table:

| Surface | Folder / file |
| ------- | ------------- |
| Agent Mode (AI in terminal) | `app/src/ai/`, `app/src/ai_assistant/` |
| Slash commands (`/harness`, `/host`, ...) | `app/src/terminal/input/slash_commands/` |
| Block rendering / command output | `app/src/terminal/` |
| Editor (input box) | `app/src/editor/` |
| Settings UI | `app/src/settings/`, `app/src/settings_view/` |
| Themes | `app/src/themes/`, `app/src/appearance.rs` |
| Drive / cloud objects | `app/src/drive/`, `app/src/cloud_object/` |
| Workflows | `app/src/workflows/` |
| Notebooks | `app/src/notebooks/` |
| Search / command palette | `app/src/search/`, `app/src/command_palette.rs` |
| Tabs / panes | `app/src/tab.rs`, `app/src/pane_group/`, `app/src/workspace/` |
| Login / auth | `app/src/auth/` |
| Updater / channel | `app/src/autoupdate/`, `app/src/channel.rs` |
| Crash reporting (Sentry) | `app/src/crash_reporting/` |
| Plugin host (MCP, etc.) | `app/src/plugin/` |
| Cross-cutting view code | `app/src/root_view.rs`, `app/src/view_components/`, `app/src/ui_components/` |

> Tip: `rg --files app/src/<area>/` is faster than reading directory
> listings.

## Channels & Binaries

`app/src/bin/` defines five real binaries plus dev tools:

| `cargo run --bin <name>` | Source | What it is |
| ------------------------ | ------ | ---------- |
| `warp` *(default)* | `local.rs` | Local development build. Warp-internal infra. |
| `warp-oss` | `oss.rs` | OSS build. **No telemetry, no autoupdater, no crash reporting.** Best base for internal forks. |
| `dev` | `dev.rs` | Dev channel; enables `DEBUG_FLAGS + DOGFOOD_FLAGS + PREVIEW_FLAGS`. |
| `stable` | `stable.rs` | Stable channel. |
| `preview` | `preview.rs` | Preview channel; gated by `preview_channel` feature. |
| `generate_settings_schema` | dev tool | Regenerates `settings.schema.json`. |

**Recommendation for internal魔改:** start from `warp-oss`. It's the
only binary that doesn't depend on Warp-internal cloud config / signing
infrastructure. Switch later if you need cloud features.

```bash
cargo run --bin warp-oss            # recommended starting point
cargo run                            # default = warp (= local.rs)
```

## Feature Flags (`app/Cargo.toml [features]`)

Warp gates **most** of its behavior behind Cargo features. The `default`
feature pulls in ~80 sub-features. To **see what's currently on**:

```bash
rg -A 200 '^default = \[' app/Cargo.toml | head -120
```

Important categories:

- **`agent_mode*`** — Agent Mode in the terminal.
- **`completions_v2`** — pulls in `command-signatures-v2` which **needs
  Node**. Default-on; if Node is missing, build with
  `--no-default-features` and re-add only what you need.
- **`crash_reporting`** — Sentry. Off in `oss`.
- **`autoupdate`** — auto-updater. Off in `oss`.
- **Many product features** — `agent_management_view`, `mcp_server`,
  `kitty_images`, `image_as_context`, `rect_selection`, ...

There is **also** a runtime feature system in `crates/warp_features/`
(`features::DEBUG_FLAGS`, `DOGFOOD_FLAGS`, `PREVIEW_FLAGS`) layered on
top — see the `dev.rs` binary for how they combine.

> **二开常见操作:** add a new Cargo feature, gate your code with it,
> add it to the `default` set when ready. Use the `add-feature-flag`
> skill in `.claude/skills/` for the boilerplate.

## Specs (`specs/APP-*`)

129+ folders, one per Linear ticket / feature. Each typically contains:

- `PRODUCT.md` — desired user behavior.
- `TECH.md` — implementation plan.
- Sometimes screenshots, fixtures, JSON examples.

**Always read the relevant spec before modifying a feature.** They
encode constraints that aren't in the code.

```bash
ls specs/                            # browse
rg -l "agent.mode" specs/            # find specs touching a topic
```

## WarpUI in 60 Seconds

- Single global `App`. Owns all entities.
- A "view" is an entity. References to other views are
  `ViewHandle<T>`, not direct references.
- During render / event handling, you get an `AppContext` /
  `ViewContext` / `ModelContext` — the param is conventionally named
  `ctx` and goes **last** in any signature.
- Elements describe layout (think Flutter `Widget`).
- Mouse state must be captured once into a `MouseStateHandle` and
  cloned/passed around — **do not** call `MouseStateHandle::default()`
  inline during render (silent break).
- Custom format-arg lints: prefer `eprintln!("{message}")` over
  `eprintln!("{}", message)`.

See `WARP.md` "Coding Style Preferences" + "Terminal Model Locking" for
the full set of gotchas.

## Backend Coupling

Warp clients talk to a Warp-operated `warp-server` over GraphQL +
WebSocket:

- HTTP: `SERVER_ROOT_URL` (default `https://app.warp.dev`)
- WS:   `WS_SERVER_URL` (default `wss://app.warp.dev/graphql/v2`)

For a **local server** (or to point at your own backend):

```bash
SERVER_ROOT_URL=http://localhost:8082 \
WS_SERVER_URL=ws://localhost:8082/graphql/v2 \
cargo run --features with_local_server --bin warp
```

The `oss` binary uses the same default URLs. If your fork doesn't have
backend access, expect login / drive / shared sessions to fail
gracefully; pure-terminal + Agent-Mode-with-your-own-API-key paths
should still work.

## Build System Caveats

- **`Cargo.lock` is committed.** Don't regenerate it casually.
- **`rust-toolchain.toml` pins `1.92.0`.** Use `rustup`; don't rely on
  `brew install rust`.
- **`.cargo/config.toml`** sets `MACOSX_DEPLOYMENT_TARGET=10.14`,
  enables `git-fetch-with-cli` (uses your SSH config / proxy), and
  passes `-C symbol-mangling-version=v0`. Don't override these
  carelessly.
- **`command-signatures-v2`** has `build.rs` that calls Node + yarn
  via `corepack`. Either install Node + run `corepack enable`, or
  build without `--workspace` and without the `completions_v2` feature.
- **Integration tests** (`crates/integration/`) require a working
  Warp build and the test harness in
  `.claude/skills/warp-integration-test/`.

## Where To Look When You're Stuck

| Symptom | Look at |
| ------- | ------- |
| "How is feature X supposed to behave?" | `specs/APP-*/PRODUCT.md` |
| "Where does this command get handled?" | `rg "command_name" app/src/terminal/input/` |
| "How do channels differ?" | `app/src/bin/*.rs` + `crates/warp_core/src/channel.rs` |
| "What feature flags are on?" | `app/Cargo.toml [features].default` |
| "How is this UI element rendered?" | grep for the type in `app/src/view_components/` and `crates/warpui/` |
| "Build error mentioning JS/yarn/node" | `crates/command-signatures-v2/build.rs` — disable feature `completions_v2` |
| "How does this talk to the server?" | `crates/graphql/`, `crates/warp_server_client/` |
