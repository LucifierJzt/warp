# PRODUCT.md — Local Auth (绕过 warp-server 登录)

> **Status:** proposed (not yet implemented)
> **Owner:** internal fork (LucifierJzt/warp)
> **Tracker entry:** `feature-list.json :: M3-001`

## Problem

The internal fork wants to use Warp's terminal/UI/agent capabilities
**without depending on `warp-server`** (Warp's hosted authentication
and sync backend). Currently, on first launch the user is funneled
through a sign-in flow that calls Firebase + warp-server, and many
features (Drive, AI request limits, shared sessions, telemetry) refuse
to work until that flow completes.

For an internal-only魔改 deployment with no public release, this
upstream login flow is:

- **Useless** — we don't have warp-server access; auth requests
  either fail or pin against Warp's production endpoints.
- **A leak risk** — even failed auth attempts ship the device fingerprint
  and email to a third party.
- **A startup tax** — the modal blocks the first window from being
  usable.

## Desired Behavior

When the **internal fork build** launches:

1. **No sign-in modal appears.** The app boots straight to the main
   workspace.
2. **App is in a "logged-in" state** so all `is_logged_in()`-gated UI
   surfaces (settings, Agent Mode entry point, command palette) render
   normally.
3. **No outbound HTTP/WS requests** are made to `app.warp.dev` or any
   warp-server endpoint at startup or during routine use.
4. **Cloud-only features fail gracefully** rather than blocking the UI:
   - Drive sync, shared sessions, account settings, billing, etc.
     should either be hidden or show a non-blocking "unavailable in
     local mode" hint.
   - Local-only features (terminal, blocks, themes, command palette,
     local AI via API key, MCP servers, workflows-from-disk) keep
     working.
5. **A login UI may exist** (e.g. an "About" page or hidden command)
   but is **opt-in only** and does nothing by default.

### Out of Scope (explicit non-goals)

- A real password database / hashed credential store. There are no
  multi-user separation requirements on a single laptop. The fork is
  a single-operator魔改, not a multi-tenant SaaS.
- An admin/admin password gate **on the desktop launcher itself**.
  macOS already requires the user to unlock the device. Adding a
  second prompt is friction without security gain.
- Replacing the warp-server with our own backend. (That's a separate,
  larger spec — `FORK-SELFHOSTED-BACKEND`, not yet started.)
- Continuing to receive auto-updates from Warp's update channel.

## Acceptance Criteria

The fork build is considered done when:

- [ ] `cargo run --bin warp-oss --features <fork-flag>` opens a Warp
      window with the terminal usable in **< 5 seconds** with **no**
      sign-in modal at any point.
- [ ] `lsof -p <pid> -i` (or Charles/Wireshark) shows **zero**
      connections to `*.warp.dev` for the first 60 seconds after
      launch (other than DNS lookups, which are fine).
- [ ] `auth_state.is_logged_in()` returns `true`; `auth_state.user_email()`
      returns a fixed string (e.g. `"local@fork.dev"`).
- [ ] At least one cloud-gated feature (e.g. Drive panel) is reachable
      via UI without crashing — it may show "unavailable" but must not
      panic.
- [ ] Toggling the fork flag off (`cargo run --bin warp-oss`) returns
      to the upstream behavior — sign-in modal appears as before. This
      proves the change is fully isolated.

## User-Visible Behavior Differences (table)

| Surface | Upstream behavior | Fork behavior |
| ------- | ----------------- | ------------- |
| First launch | Sign-in slide → Firebase OAuth → warp-server | Skip; main workspace immediately. |
| Profile menu (top-right avatar) | Real user email + avatar from Firebase | Fixed string `"local@fork.dev"` (or hide) |
| Settings → Account | Logout, Manage Subscription | Show "Local Mode — cloud account features disabled" |
| Drive panel | Object list from server | Empty state + "unavailable in local mode" |
| AI Agent Mode | Authenticated quota via warp-server | Either disabled, or use user's own API key (existing `APIKeyAuthentication` flow) |
| Crash reporting / telemetry | Sentry + warp-server analytics | Off (already off in `warp-oss` channel) |
| Shared sessions | Live cloud objects | Off |
| Auto-update | Upstream update channel | Off (already off in `warp-oss` channel) |

## Why This Is Cheaper Than It Looks

Investigation revealed that **upstream Warp already has this exact
capability built in**, gated behind a Cargo feature called `skip_login`
(see `app/Cargo.toml:755`). It is used by Warp's own integration test
suite and `fast_dev` builds:

- `app/src/auth/auth_state.rs:136-138` —
  `should_use_test_user()` returns `true` when `skip_login` is enabled,
  initializing `AuthState` with `User::test()` and
  `Credentials::Test` immediately.
- `app/src/auth/credentials.rs:28-30` — `Credentials::Test` variant
  exists specifically for this case.
- `app/src/server/server_api/auth.rs:244-246` — when `skip_login` is
  on, `get_or_refresh_access_token` short-circuits with `bail!`, so any
  code path that tries to talk to warp-server fails fast instead of
  hanging.
- `app/Cargo.toml:644` — `fast_dev = ["skip_login"]` is a pre-existing
  alias. The mechanism is officially supported by upstream.

**Empirically verified** during research (Session 003,
2026-04-30): `cargo build --bin warp-oss --features skip_login`
compiled clean (2m12s, 0 errors), the binary launched, GUI entered
the workspace state, and stdout/stderr contained zero auth/token/login
errors over a 10-second observation window.

The fork therefore does **not** need to write new auth code. It just
needs to:

1. Decide how to expose this build flavor (a fork-named feature flag
   wrapping `skip_login`, a dedicated bin, or a default-on choice in
   the `oss` channel).
2. Audit the cloud-gated UI surfaces and make sure their failure modes
   are non-fatal in `Credentials::Test` mode.
3. Optionally: customize the fixed user identity (email/display name)
   to make it obvious the build is in local mode.

## Open Questions for the Operator

These need answers before TECH.md is finalized:

1. **Surfacing model.** Three options — pick one:
   - (a) New fork feature `local_only` (Cargo) that pulls in `skip_login`.
       `cargo run --bin warp-oss --features local_only` opts in.
       Upstream `cargo run --bin warp-oss` keeps original behavior.
   - (b) Make `skip_login` part of the `oss` binary's `default-features`
       so plain `cargo run --bin warp-oss` always skips login.
   - (c) Make a brand-new bin `warp-fork` in `app/src/bin/fork.rs` that
       calls `warp::run()` after explicitly forcing local-mode state.
2. **Cloud-feature UX.** When the user clicks "Drive" in local mode, do
   we want:
   - (a) The button hidden entirely (cleaner, but invasive — touches
       layout).
   - (b) The button visible but the panel shows a "Local mode — cloud
       features unavailable" empty state (cheaper, less invasive).
3. **Identity string.** What should `auth_state.user_email()` return
   in local mode?
   - Default `test_user@warp.dev` (works today, but visually weird).
   - `local@fork.dev` (clearer intent, requires a 1-line override).
   - Operator's own email (more personal, requires env var or build-time
     config).
4. **AI Agent Mode.** Does the fork want Agent Mode to:
   - (a) Be disabled entirely in local mode.
   - (b) Be reachable but route through the user's own provider API key
       (existing `APIKeyAuthentication` flow).
   - (c) Be wired up to a self-hosted AI proxy later (separate spec).

## Related Specs

- `specs/APP-3679/PRODUCT.md` — recent upstream login changes (read
  before starting, in case our fork conflicts with their direction).
- `specs/REMOTE-1373/PRODUCT.md` — auth-adjacent (skim).
- (future) `FORK-SELFHOSTED-BACKEND` — if we ever want full backend
  parity with our own server.
