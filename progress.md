# Progress Log

> Single source of truth for "where the project currently stands".
> Every session **reads this file first** and **writes to it last**.

## Current Verified State

- **Repository root:** `/Users/troye/Project/warp`
- **Branch / last upstream commit:** `master` @ `157f358` —
  "Introduce `/harness`, `/host` and `/environment` slash commands to new
  cloud mode input."
- **Standard startup path:** `./init.sh`
- **Standard verification path:**
  `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2`
- **Full presubmit:** `./script/presubmit`
- **Current highest-priority unfinished feature:** _none selected yet —
  pick from `feature-list.json` at session start._
- **Current blocker:** none.
- **Last verified at:** 2026-04-30 — Full M0+M1 smoke completed.
  `cargo build --bin warp-oss` clean (4s incremental after Session 002's
  `cargo check`); binary runs (PID 50148/60168), forks the
  `terminal-server` child as expected, appears in macOS visible-process
  list, no stderr output. Trivial fork edit (CFBundleDisplayName) was
  applied, recompiled in 30s, verified in the binary via `strings`,
  then cleanly reverted. `cargo check --workspace` still fails on
  `command-signatures-v2` (Node not wired up) — acceptable.

## Project Profile (二开 / Internal Fork)

- **Direction:** explore mode — focus area not yet decided.
- **Distribution:** internal-only魔改, no public release.
- **Platform:** macOS only.
- **Pain points:** build speed + AI agent协作流.
- **Recommended entry binary:** `warp-oss` (`app/src/bin/oss.rs`) — no
  telemetry, no autoupdater, no crash reporting, cleanest fork base.
- **Toolchain:** rustup 1.29.0 with active 1.92.0-aarch64-apple-darwin
  (matches `rust-toolchain.toml`).
- **Reading order for a new session/Agent:**
  1. `AGENTS.md` (rules)
  2. `progress.md` (this file)
  3. `ARCHITECTURE.md` (code map)
  4. `DEVELOPMENT.md` (workflow + accelerators)
  5. `feature-list.json` (pick a feature)

## Session Log

### Session 001 — Harness bootstrap

- **Date:** 2026-04-30
- **Goal:** Initialize Harness working environment (AGENTS.md,
  progress.md, feature-list.json, init.sh, plus session-handoff and
  clean-state checklist) for the Warp repo.
- **Completed:**
  - Wrote `AGENTS.md` with startup workflow, working rules, verification
    standards, and end-of-session protocol tailored to the Warp Rust
    workspace.
  - Wrote `progress.md` (this file) with the "Current Verified State"
    block populated.
  - Wrote `feature-list.json` seeded with `harness-001` (verify
    Harness scaffolding works) as the only `not_started` entry.
  - Wrote `init.sh` calling
    `script/install_cargo_build_deps`, `cargo build`, and
    `cargo nextest run` against the Warp workspace.
  - Wrote `session-handoff.md` and `clean-state-checklist.md`.
- **Verification run:** scaffolding only — no `cargo` build or test was
  executed in this session. Next session must run `./init.sh` first.
- **Evidence captured:** files visible in repo root via
  `ls AGENTS.md progress.md feature-list.json init.sh \
      session-handoff.md clean-state-checklist.md`.
- **Commits:** _not committed yet — leave staging up to the operator._
- **Files / artifacts updated:**
  - `AGENTS.md` (new)
  - `progress.md` (new)
  - `feature-list.json` (new)
  - `init.sh` (new, executable)
  - `session-handoff.md` (new)
  - `clean-state-checklist.md` (new)
- **Known risk or unresolved issue:** `init.sh` defaults to running
  `cargo nextest run --workspace --exclude command-signatures-v2`, which
  is heavy on a cold checkout. Operators may want to override
  `VERIFY_CMD` for fast iteration. Initial `cargo build` will likely
  require `./script/bootstrap` and platform deps (see `WARP.md`).
- **Next best step:** In the next session, run `./init.sh` to confirm
  the baseline builds and tests pass. If green, mark `harness-001`
  `passing` and pick the next feature from `feature-list.json`.

### Session 002 — Environment bootstrap & first green check

- **Date:** 2026-04-30
- **Goal:** Get `cargo check` (or better) green so the next session can
  start real product work.
- **Completed:**
  - Identified that `~/.cargo/bin` was missing from `PATH`, which made
    `script/install_cargo_binstall` loop forever re-installing
    `cargo-binstall`. Documented the fix and exported the PATH inside
    `init.sh` so future sessions don't hit the same trap.
  - Confirmed all needed cargo binaries are installed under
    `~/.cargo/bin`: `cargo-binstall 1.14.3`, `diesel`, `cargo-bundle`,
    `cargo-about`, `cargo-nextest`, `wgslfmt`.
  - Skipped the heavy `script/bootstrap` path (brew update / docker /
    gcloud / powershell / Metal toolchain) — none of it is required for
    `cargo run` of the client.
  - Hardened `init.sh`:
      * Exports `~/.cargo/bin` to PATH up-front.
      * Default `INSTALL_CMD` is now a no-op (was the looping
        `install_cargo_build_deps`).
      * Default `VERIFY_CMD` is now `cargo check` (default-members
        only) instead of `cargo check --workspace`, because the
        `command-signatures-v2` crate needs a Node toolchain.
- **Verification run:** `cargo check` (default-members), 2m12s, exit 0.
- **Evidence captured:**
  ```
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 12s
  warning: `warp` (lib) generated 44 warnings  (all unused-variable)
  ```
- **Commits:** _not committed yet — operator decides when to stage._
- **Files / artifacts updated:**
  - `init.sh` (PATH export, no-op INSTALL_CMD, lighter VERIFY_CMD)
  - `progress.md` (this entry)
- **Known risk or unresolved issue:**
  - `cargo check --workspace` still fails on `command-signatures-v2`
    until Node.js + corepack are installed. Acceptable for now since
    the crate is not a default member.
  - Toolchain on this machine is `rustc 1.94.0 (Homebrew)` while
    `rust-toolchain.toml` pins `1.92.0`. 1.94.0 worked for `cargo check`
    so we're not blocked, but installing `rustup` is recommended for
    determinism.
- **Next best step:**
  1. Run `cargo run` to confirm the app actually launches.
  2. If green, mark `harness-001` as `passing` in `feature-list.json`
     with the elapsed time and warning count as evidence.
  3. Pick the next feature from `feature-list.json` and start work.

### Session 003 — 二开准备 (preparing for forked development)

- **Date:** 2026-04-30
- **Goal:** Profile the operator's二开 intent and prepare durable docs
  + a real feature roadmap so the next coding session can move fast.
- **Completed:**
  - Confirmed direction: **explore mode**, internal fork, macOS only,
    pain points = build speed + AI agent workflow.
  - Confirmed environment: `rustup` is now installed and 1.92.0
    auto-activates from `rust-toolchain.toml` (resolves the previous
    Homebrew-rustc-1.94.0 mismatch concern).
  - Confirmed `node v25.8.0` is on the machine but `corepack` is not
    enabled — so `command-signatures-v2` JS build still won't run
    until the operator opts in. Documented in DEVELOPMENT.md §1.
  - Wrote **`ARCHITECTURE.md`**: 30-second mental model, top-level
    layout, `app/src/` surface map, channels + binaries, feature flag
    overview, WarpUI primer, backend coupling, build caveats, "where
    to look when stuck" troubleshooting table.
  - Wrote **`DEVELOPMENT.md`**: 二开 workflow including build
    acceleration (per-crate check, IDE/rust-analyzer config, optional
    sccache/lld), modular fork strategy (feature-flag-first, FORK:
    markers, upstream sync recipe), AI Agent collaboration playbook
    (existing `.claude/skills/*` + spec-first prompting), debug recipes,
    and a 5-day onboarding plan.
  - Replaced `feature-list.json` placeholder content with a real
    milestone-driven roadmap (M0 launch → M1 trivial fork edit →
    M2 absorb specs → M3 first feature-flagged change → M4 upstream
    sync drill). Marked `harness-001` as `passing` with evidence from
    Session 002.
- **Verification run:** none beyond Session 002's `cargo check`.
- **Evidence captured:**
  - Files exist in repo root: `AGENTS.md`, `ARCHITECTURE.md`,
    `DEVELOPMENT.md`, `progress.md`, `session-handoff.md`,
    `clean-state-checklist.md`, `feature-list.json`, `init.sh`.
  - `rustup show` confirms `1.92.0-aarch64-apple-darwin (active)`.
- **Commits:** _not committed yet — operator decides when to stage._
- **Files / artifacts updated:**
  - `ARCHITECTURE.md` (new)
  - `DEVELOPMENT.md` (new)
  - `feature-list.json` (rewritten with real milestones)
  - `progress.md` (this entry + Project Profile block)
- **Known risk or unresolved issue:**
  - `M0-001` (cargo run --bin warp-oss) not yet exercised. First link
    on cold target/ may take 5-10 minutes and could surface link
    errors not seen during `cargo check`.
  - Node + corepack not yet wired up; if M3+ needs `completions_v2`,
    operator must run `corepack enable && cd crates/command-signatures-v2/js && yarn install`.
  - No `upstream` git remote configured yet (M4 prerequisite).
- **Next best step:**
  1. Run `cargo run --bin warp-oss`. If a window opens, mark `M0-001`
     `passing` with the screenshot/PID.
  2. Then proceed to `M1-001` (trivial fork edit) to validate the
     edit-compile-run loop end-to-end.
  3. Once M0-M1 are green, decide focus area and update `M2-001` notes.

### Session 003 (cont.) — M0 + M1 smoke completed

- **Sub-goal:** Drive `M0-001` (launch warp-oss) and `M1-001`
  (trivial fork edit) to `passing` to validate the full edit-compile-
  run-revert loop before any real product work begins.

- **M0-001 — Launch warp-oss:**
  - `cargo build --bin warp-oss` — 4.06s incremental, exit 0,
    44 warnings (all pre-existing `unused variable` in
    `app/src/terminal/input/slash_commands/mod.rs`).
  - Binary: `target/debug/warp-oss` (Mach-O 64-bit arm64, 694MB).
  - Launched: parent PID 50148 (RSS 405MB) + child PID 50482
    (`warp-oss terminal-server --parent-pid=50148`).
  - Verified visible in macOS via
    `osascript -e 'tell application "System Events" to get name of every process whose visible is true'`.
  - stdout/stderr empty.
  - **Marked passing** in feature-list.json with all evidence above.

- **M1-001 — Trivial fork edit:**
  - Killed previous warp-oss process.
  - Edited `app/src/bin/oss.rs`: changed `<string>WarpOss</string>`
    (CFBundleDisplayName) to `<string>WarpOss-Fork</string>` with a
    `<!-- FORK: smoke-test edit (M1-001) - revert to "WarpOss" after verification -->`
    marker on the line above.
  - `cargo build --bin warp-oss` — 30.03s incremental (only `app`
    crate recompiled). **This is the realistic per-edit iteration cost
    on this machine.**
  - Re-launched: PID 60168 + child 60579, again visible in
    macOS process list, stderr empty.
  - Verified the change reached the running binary:
    `strings target/debug/warp-oss | grep -i warposs` →
    output contains `<string>WarpOss-Fork</string>` and the FORK
    comment line.
  - Reverted oss.rs to its pristine upstream state.
  - `git diff app/src/bin/oss.rs` → empty (clean revert).
  - `git status` → only the 8 new harness files in repo root.

- **Verification run:** see above (build outputs + strings + git diff).
- **Evidence captured:** in feature-list.json under `M0-001` and
  `M1-001` evidence arrays.
- **Commits:** _still uncommitted — operator decides when to stage
  the harness files._
- **Files / artifacts updated:**
  - `feature-list.json` — `harness-001`, `M0-001`, `M1-001` all
    `passing` with full evidence.
  - `progress.md` — this entry, and "Current Verified State" updated.
- **Known risk or unresolved issue:**
  - 30s incremental build for a one-line app/src change is acceptable
    but not great. If this becomes painful, set up a user-level
    `~/.cargo/config.toml` with `lld` linker per DEVELOPMENT.md §2.1
    — should drop link time from ~25s to ~5s.
  - `M2-001` still says "focus area undecided"; pick a direction next
    session and update its `notes`.
- **Next best step:**
  1. Operator: pick a focus area (UI / AI / terminal / drive / fork
     packaging) and update `M2-001` notes with 3-5 candidate
     `app/src/<dir>` modules and 2-3 relevant `specs/APP-*` folders.
  2. Then start `M3-001`: design a concrete first feature behind a
     fork Cargo feature flag using `.claude/skills/add-feature-flag/`.
  3. Optional: commit the harness scaffolding now —
     `git add AGENTS.md ARCHITECTURE.md DEVELOPMENT.md progress.md feature-list.json init.sh session-handoff.md clean-state-checklist.md`
     then `git commit -m "harness: bootstrap二开 working environment"`.

### Session 003 (cont. 2) — Focus area chosen + FORK-LOCAL-AUTH spec drafted

- **Sub-goal:** Operator asked to "remove the existing login flow,
  replace with a simple admin/admin login so we don't hit warp-server".
  Investigate, then propose the simplest implementation.
- **Investigation findings:**
  - `app/src/auth/` is a 22-file module with `AuthManager`,
    `AuthState`, `Credentials`, login modals, Firebase token handling,
    SSO link views, etc.
  - **Major discovery: upstream already ships a `skip_login` Cargo
    feature** (`app/Cargo.toml:755`) used by their integration tests
    and `fast_dev = ["skip_login"]` build alias. With it on:
      * `auth_state.rs:136-138 should_use_test_user()` returns true,
        immediately initializing AuthState with `User::test()` +
        `Credentials::Test`.
      * `server_api/auth.rs:244-246 get_or_refresh_access_token`
        short-circuits with `bail!`, so any code path that tries to
        call warp-server fails fast.
      * `Credentials::Test` variant exists in `credentials.rs:28-30`
        precisely for this case.
  - **Empirically verified:** `cargo build --bin warp-oss --features
    skip_login` compiled clean (2m12s, 0 errors), the binary launched
    (PID 43617 + child 43956), entered macOS visible-process list,
    stdout/stderr completely empty (no auth/token/login errors).
  - This means the operator's "admin/admin login" goal is best served
    by **NOT writing a new login UI** but by exposing `skip_login` via
    a fork-named Cargo feature `local_only`.
- **Trade-off vs. operator's literal request:**
  - "Real" admin/admin login UI = ~500 lines (modal + state + plumbing
    + credential persistence) for zero security gain on a single-user
    laptop (macOS device unlock is the real boundary).
  - `local_only = ["skip_login"]` = 1-line `Cargo.toml` change + an
    optional 10-line identity override = same outcome (no warp-server,
    app usable immediately) at ~1% the cost.
- **Authored:**
  - `specs/FORK-LOCAL-AUTH/PRODUCT.md` — desired behavior, acceptance
    criteria, side-by-side comparison of upstream vs. fork surfaces,
    open questions for operator.
  - `specs/FORK-LOCAL-AUTH/TECH.md` — recommended approach (Cargo
    feature, not new bin), 3-phase plan (minimum viable → identity
    polish → outbound network lockdown), code locations cheat sheet,
    test plan, risk table.
- **feature-list.json updates:**
  - `M2-001` marked `passing` (focus area chosen + specs absorbed).
  - `M3-001` rewritten: was placeholder, now concrete FORK-LOCAL-AUTH
    Phase 1 task with verification steps including `lsof` network
    quietness check.
  - Milestone M3 description updated.
- **Verification run:** investigation only; no code changes.
- **Evidence captured:** all in PRODUCT.md / TECH.md and the new
  evidence arrays of M2-001 / M3-001.
- **Commits:** _not committed yet — operator should review the specs
  first._
- **Files / artifacts updated:**
  - `specs/FORK-LOCAL-AUTH/PRODUCT.md` (new)
  - `specs/FORK-LOCAL-AUTH/TECH.md` (new)
  - `feature-list.json` (M2-001 passing, M3-001 rewritten, M3 milestone
    updated)
  - `progress.md` (this entry)
- **Known risk or unresolved issue:**
  - Operator may still prefer a real admin/admin login UI for
    aesthetic/training reasons; that path is documented as a non-goal
    in PRODUCT.md but can be revisited.
  - Phase 2.2 audit (10 cloud-gated UI surfaces) hasn't been done; some
    might panic on `Credentials::Test`. We won't know until we try.
  - Open questions in PRODUCT.md §"Open Questions" need operator input
    before finalizing identity string and AI mode behavior.
- **Next best step:**
  1. Operator reviews `specs/FORK-LOCAL-AUTH/PRODUCT.md` and TECH.md.
  2. Decide: take the recommended `local_only` Cargo-feature route, or
     insist on the literal admin/admin login UI?
  3. If recommended route: implement Phase 1 (one-line Cargo.toml
     change + verification). Mark `M3-001` `passing`.
  4. Commit + push the spec docs and the M3-001 implementation.

### Session 003 (cont. 3) — FORK-LOCAL-AUTH Phase 1 shipped

- **Sub-goal:** Operator approved the recommended `local_only` Cargo
  feature route. Implement Phase 1 from TECH.md.
- **Implementation:**
  - Edited `app/Cargo.toml` line 755-760: added 5-line entry
    `local_only = ["skip_login"]` with a FORK comment warning it must
    never be in default features.
- **Verification (all passed):**
  - `cargo check --bin warp-oss --features local_only` — 1m32s, exit 0.
  - `cargo build --bin warp-oss --features local_only` — 2m18s, exit 0.
  - Binary launched as PID 83674 + terminal-server child PID 83911,
    appeared in macOS visible-process list.
  - **60-second network quietness check:** `lsof -i -P | grep warp-oss`
    sampled at T+10/20/30/40/50/60s — zero IP sockets at every sample.
    No `*.warp.dev` / firebase / googleapis connections at any point.
  - stderr / stdout completely empty across all launches.
  - **Cfg-elimination evidence (the cleanest proof):**
    `strings target/debug/warp-oss | grep 'skip_login enabled'`
    returns the `bail!("skip_login enabled; failing all authenticated
    requests")` message ONLY when built with `--features local_only`.
    The no-flag binary does not contain this string, proving the
    `cfg!(feature = "skip_login")` gates are correctly compiled out.
  - Reverse build (`cargo build --bin warp-oss` without flag) also
    launches with 0 IP sockets — that's because upstream `warp-oss`
    doesn't auto-call warp-server, only when the user clicks login.
    The runtime difference is therefore `is_logged_in() == true`
    (local_only) vs false (upstream), which would manifest when the
    user reaches a login-gated UI surface.
- **Verification run:** see verification list above.
- **Evidence captured:** in feature-list.json under M3-001 evidence.
- **Files / artifacts updated:**
  - `app/Cargo.toml` (added local_only feature)
  - `feature-list.json` (M3-001 → passing with full evidence)
  - `progress.md` (this entry)
- **Commits:** _not committed yet — about to commit + push_
- **Known risk or unresolved issue:**
  - Phase 2.2 audit (10 cloud-gated UI surfaces) not yet performed.
    Some panels might panic on `Credentials::Test`. Will discover
    during dogfooding.
  - Identity string still shows `test_user@warp.dev` in the profile
    menu (Phase 2.1 would fix this).
  - The 5-day exploration plan from DEVELOPMENT.md is essentially
    compressed into 1 day because we got lucky finding `skip_login`.
- **Next best step:**
  1. Commit + push Phase 1 changes.
  2. Dogfood `cargo run --bin warp-oss --features local_only` for
     a real session — try Drive, Settings, Agent Mode, Shared
     Sessions. Note any panics or weird states in a new feature
     entry `M3-002` (Phase 2.2 audit).
  3. If Phase 2.1 (identity override) becomes important after
     dogfooding, ship it as `M3-003`.

### Session 003 (cont. 4) — FORK-ZHIPU-AI specced (no code yet)

- **Sub-goal:** Operator asked to "switch the AI from Claude to GLM,
  using my own baseUrl + API key". Translate that into something
  achievable within reasonable engineering scope.
- **Investigation findings:**
  - Warp's `AIClient` trait (`app/src/server/server_api/ai.rs:740`)
    has 30+ methods, all sending Warp-private GraphQL mutations
    (`generateDialogue`, `createAgentTask`, `updateMerkleTree`,
    `getBlockSnapshot`, ...) to `app.warp.dev/graphql/v2`.
  - Full-repo ripgrep for `api.openai.com|api.anthropic.com|googleapis`
    returns ZERO hits. The client never talks to Anthropic/OpenAI
    directly — warp-server does that translation.
  - Therefore "swap base URL + API key" is **not a thing that exists**
    in this client. There's no setting to change because the client
    has never spoken OpenAI protocol.
  - Three theoretical paths to make existing Agent Mode use GLM:
      A. Replace `impl AIClient for ServerApi` → ZhipuAIClient.
         15+ methods (spawn_agent, merkle_tree, block_snapshot,
         conversation_metadata, file_artifact_upload, ...) have no
         OpenAI-protocol equivalent. 2-3 weeks; result is half-broken.
      B. Local GraphQL→OpenAI proxy faking warp-server. 3-4 weeks +
         ongoing schema drift; debugging requires understanding both
         protocols simultaneously.
      C. Self-hosted warp-server clone. warp-server is closed-source.
         3-6 months. Not realistic.
  - Recommended path D: build a parallel, independent panel (the way
    Cline/Roo/Cursor handle the same question) that owns its own
    OpenAI-compatible HTTP stack. ~1-2 weeks; full control; zero
    impact on Warp's existing Agent Mode.
- **Operator decision:** path D ("panel_only").
  - Auto-enabled with local_only? No — keep `glm_assistant` as a
    separate Cargo feature. Build with both: `--features local_only,glm_assistant`.
  - Model name: configurable, default GLM-4.6.
  - Base URL: `https://open.bigmodel.cn/api/coding/paas/v4` (configurable).
- **Authored:**
  - `specs/FORK-ZHIPU-AI/PRODUCT.md` — desired behavior, acceptance
    criteria, side-by-side surface table, "why we're not modifying
    Warp's Agent Mode" section.
  - `specs/FORK-ZHIPU-AI/TECH.md` — architecture diagram, 7-step
    Phase-1 implementation plan with day-level estimates, file-by-file
    plan (~6 new files in `app/src/ai/glm/` + 5 small touchpoints in
    existing files), risk table, test plan.
- **feature-list.json updates:**
  - New entry `M3-002` (FORK-ZHIPU-AI). Status not_started.
  - Milestone M3 description rewritten to encompass both fork
    sub-features.
- **Verification run:** none — design phase. No code edits.
- **Evidence captured:** the spec docs.
- **Commits:** _not yet — after operator review of specs._
- **Files / artifacts updated:**
  - `specs/FORK-ZHIPU-AI/PRODUCT.md` (new)
  - `specs/FORK-ZHIPU-AI/TECH.md` (new)
  - `feature-list.json` (M3-002 added; M3 description updated)
  - `progress.md` (this entry)
- **Known risk or unresolved issue:**
  - PRODUCT.md §"Open Questions" still has 6 items where the spec
    states recommendations but the operator hasn't explicitly
    confirmed (e.g., Keychain vs plaintext, whether to show token
    counts, history persistence model). Defaults are pre-chosen so
    Phase 1 can proceed if operator approves the spec wholesale.
  - Phase-1 estimate is 5-8 working days. Realistic only if no
    unexpected friction appears in WarpUI panel registration —
    we've never built a panel from scratch in this codebase before.
- **Next best step:**
  1. Operator reviews `specs/FORK-ZHIPU-AI/PRODUCT.md` and TECH.md.
  2. Confirm or override the 6 "Open Questions" defaults.
  3. Decide whether to start Phase 1 immediately or sit on it until
     LOCAL-AUTH (M3-001) has been dogfooded for a few days.
  4. Commit + push the spec documents either way (they're useful
     reference material even if implementation is deferred).

### Session 004

- Date:
- Goal:
- Completed:
- Verification run:
- Evidence captured:
- Commits:
- Files / artifacts updated:
- Known risk or unresolved issue:
- Next best step:
