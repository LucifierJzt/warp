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
