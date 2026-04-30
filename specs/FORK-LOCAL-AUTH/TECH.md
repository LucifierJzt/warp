# TECH.md — Local Auth Implementation

> Read `PRODUCT.md` first. This file is the implementation plan that
> assumes the product spec is approved.

## Recommendation: Approach (a) — `local_only` Cargo feature

After research, **we strongly recommend Approach (a)** from PRODUCT.md
"Open Questions" §1:

> Add a new fork-named Cargo feature `local_only` that pulls in
> upstream's existing `skip_login` feature. `cargo run --bin warp-oss
> --features local_only` opts in.

### Why (a) over (b) and (c)

| Aspect | (a) Cargo feature | (b) Default-on for warp-oss | (c) New bin `warp-fork` |
| ------ | ----------------- | --------------------------- | ----------------------- |
| Code touched | ~5 lines (`Cargo.toml` + maybe one identity override) | Same Cargo.toml change, but in `default` set | New `app/src/bin/fork.rs` (~70 lines) + `[[bin]]` section |
| Upstream rebase risk | Tiny (we add one line in the features section, easy to merge) | Medium (changing `oss` defaults conflicts with upstream's intent for that bin) | Tiny (we own a new file) |
| Reversibility | `cargo run --bin warp-oss` (no flag) restores upstream behavior immediately | Have to remove from defaults to test upstream | Have to choose which bin to run |
| Discoverability | Documented in `DEVELOPMENT.md` and `feature-list.json` | Implicit; surprises future contributors | Explicit; visible in `cargo run --list` |
| Production-fork story | We bundle with `--features local_only` always | We modify the bin we depend on | We have a separate bin to maintain |
| Effort | **~1 hour incl. tests** | ~30 min (but invasive) | ~3 hours (need to copy oss.rs and audit) |

(a) wins on **lowest invasiveness + lowest rebase risk**. The whole
change is additive to `app/Cargo.toml`'s features section + (optionally)
one or two small `#[cfg(feature = "local_only")]` overrides for
identity strings.

If later we decide we **always** want local mode for the fork's
shipped binary, we can layer (c) on top trivially: a new `app/src/bin/fork.rs`
that just sets `local_only` always. But we recommend not doing this
on day 1.

## Plan

### Phase 1 — Minimum viable: just the feature (1 hour)

Goal: `cargo run --bin warp-oss --features local_only` boots straight
into the workspace, no sign-in modal, no warp-server traffic.

#### Step 1.1 — Add `local_only` feature in `app/Cargo.toml`

In the `[features]` section, add (place near the existing `skip_login`
line, ~755):

```toml
# FORK: enables local-only mode (no warp-server / Firebase auth).
# Pulls in upstream's skip_login mechanism so all auth-gated code
# paths fall back to a fixed test user + Credentials::Test.
local_only = ["skip_login"]
```

That is the entire mandatory change. Everything else in this phase is
optional polish.

#### Step 1.2 — Verify

```bash
# Builds cleanly with the new feature
cargo check --bin warp-oss --features local_only

# Launches without sign-in modal
cargo run --bin warp-oss --features local_only

# Confirm zero warp-server traffic in first 60s:
lsof -p $(pgrep -f 'target/debug/warp-oss$') -i 2>&1 | grep -i warp
# Expected: no output (no connections to *.warp.dev).

# Reverse-confirm upstream behavior is preserved when flag is off:
cargo run --bin warp-oss
# Expected: original sign-in slide appears.
```

#### Step 1.3 — Add evidence to `feature-list.json`

Mark `M3-001` as `passing` with the launch time, network-quiet evidence,
and confirmation that flagless build still triggers sign-in.

### Phase 2 — Identity & UX polish (optional, ~1 hour)

Pick these up after Phase 1 is committed and you've used local mode for
a day to feel out the rough edges.

#### Step 2.1 — Customize local-mode identity

Currently the fixed user is `test_user@warp.dev`, which renders in the
profile menu and looks like a bug to anyone unfamiliar with the build.

**Smallest possible change** — override in `app/src/auth/user.rs`:

```rust
// Around the existing User::test() impl
impl User {
    pub fn test() -> Self {
        Self {
            local_id: UserUid::new(TEST_USER_UID),
            metadata: UserMetadata {
                #[cfg(feature = "local_only")]
                email: "local@fork.dev".to_string(),
                #[cfg(not(feature = "local_only"))]
                email: TEST_USER_EMAIL.to_string(),
                display_name: {
                    #[cfg(feature = "local_only")]
                    { Some("Local Mode".to_string()) }
                    #[cfg(not(feature = "local_only"))]
                    { None }
                },
                photo_url: None,
            },
            // ... rest unchanged
        }
    }
}
```

Reasoning: keeps `test_user@warp.dev` for actual integration tests
(which assert on it), but flips the identity for fork builds.

**Risk:** if integration tests assert against user metadata in a way
we haven't seen, this could break a test. Easy to discover via
`cargo nextest run -p warp --features local_only`. Roll back the
override if so.

#### Step 2.2 — Audit cloud-gated panels for graceful failure

The risky failure mode is **a UI surface that assumes a real user
exists and panics on `Credentials::Test`**. Surfaces to manually
exercise in local mode:

1. Drive panel (sidebar)
2. Settings → Account
3. Settings → Privacy
4. Shared sessions modal
5. Workflows panel (cloud workflows tab)
6. Notebooks panel (cloud notebooks)
7. Profile menu (top-right avatar)
8. Command palette: any item that fetches from cloud
9. Agent Mode (entry point)
10. Billing / subscription views

For each, **observe the behavior** and decide:

- **Crash / panic** → file a `local_only` bug, add a fix (usually a
  `if !auth_state.is_user_local() { ... }` guard, or convert a
  `.unwrap()` of cloud data to a `.unwrap_or_default()`).
- **Hangs** → check it isn't waiting on a network reply that won't come.
  `Credentials::Test` already returns `AuthToken::NoAuth` so the
  request should fail fast; if it hangs, file a bug.
- **Empty state** → done, no action.
- **Error toast** → consider downgrading to a quieter empty state if it's
  noisy.

Track the audit results inline in this file as it evolves.

### Phase 3 — Lock down outbound network (optional, deferred)

If we want a guarantee of zero warp-server traffic (for compliance,
or for offline operation), add a runtime guard:

In `crates/http_client/` (or the central HTTP entrypoint), under
`#[cfg(feature = "local_only")]`, intercept any URL whose host matches
`*.warp.dev` and short-circuit with an error before sending. This is a
**defense in depth** — `Credentials::Test` already blocks the
*authenticated* code paths, but unauthenticated startup pings (telemetry
beacons, autoupdate checks) might still leak.

Defer this until Phase 2 is done and we have evidence that some
request is actually going out (run `lsof -p <pid> -i` periodically and
look for non-DNS connections to warp.dev).

## Code Locations Cheat Sheet

| Concept | File | Line |
| ------- | ---- | ---- |
| `should_use_test_user()` (the gate) | `app/src/auth/auth_state.rs` | 136-138 |
| `User::test()` (fixed identity) | `app/src/auth/user.rs` | ~165 |
| `TEST_USER_EMAIL` constant | `crates/warp_server_client/src/auth/user_uid.rs` | 5 |
| `Credentials::Test` variant | `app/src/auth/credentials.rs` | 28-30 |
| `bail!("skip_login enabled")` (network short-circuit) | `app/src/server/server_api/auth.rs` | 244-246 |
| `is_logged_in()` consumer | `app/src/auth/auth_state.rs` | 232 |
| `attempt_login_gated_feature` (UI gates) | `app/src/auth/auth_manager.rs` | (search) |
| Login modal UI | `app/src/auth/auth_view_modal.rs` | (whole file) |
| `AuthState::initialize` startup wiring | `app/src/lib.rs` | 1104 |
| `default = [...]` feature set | `app/Cargo.toml` | (search "^default = ") |

## Test Plan

### Automated

```bash
# Unit + integration tests with local_only on
cargo nextest run -p warp --features local_only

# Same without (sanity: upstream tests still pass)
cargo nextest run -p warp
```

If `--features local_only` causes test failures that don't reproduce
on the flagless build, the failures are caused by our identity override
in Step 2.1 — either fix the test or revert that override.

### Manual smoke (per Phase)

| Phase | Smoke |
| ----- | ----- |
| 1 | Launch with flag, confirm no modal, confirm `lsof` is quiet |
| 1 | Launch without flag, confirm sign-in modal still appears |
| 2.1 | Profile menu shows `local@fork.dev` when flag is on |
| 2.2 | Click each cloud-gated panel; nothing crashes |
| 3 | `lsof -p <pid> -i` shows zero non-DNS warp.dev hits in 5 minutes |

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
| ---- | ---------- | ---------- |
| Upstream removes `skip_login` feature | Low (used by their own CI) | Vendor the relevant code (`Credentials::Test`, `should_use_test_user`) into the fork before relying on it |
| Some panel panics on `Credentials::Test` | Medium | Phase 2.2 audit catches this; fix with guards |
| Identity override breaks an integration test | Low | Roll back the override; tests are scoped to `--features integration_tests` not `local_only` |
| Phase 2.2 reveals a panel needs deep refactor to support local mode | Medium | Defer that panel — show "unavailable in local mode" placeholder and move on |
| `local_only` feature accidentally gets pulled into `default` features | Medium-High during merges | Add a comment in `Cargo.toml` that this **must never be in default**; consider a CI grep check |

## Out Of This TECH.md (explicit deferrals)

- A real password gate / multi-user system → not needed; macOS device
  unlock is the real boundary.
- Self-hosted backend → separate spec `FORK-SELFHOSTED-BACKEND`.
- Hiding cloud-feature UI elements (vs. showing-but-disabling) → defer
  until Phase 2.2 audit has data.
- Replacing the upstream auto-updater with a fork updater → separate spec.
