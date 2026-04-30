# AGENTS.md

This repository is set up for long-running coding-agent work on the Warp
client (a Rust-based terminal emulator with the custom WarpUI framework).
The goal is not to maximize raw code output. The goal is to leave the repo in
a state where the next session can continue without guessing.

> **Reading order for any new session:**
>
> 1. `AGENTS.md` (this file) — operating rules.
> 2. `progress.md` — current verified state + last session's notes.
> 3. `ARCHITECTURE.md` — code map; where features live in the repo.
> 4. `DEVELOPMENT.md` — 二开 workflow, build accelerators, AI agent playbook.
> 5. `WARP.md` — project commands, coding style, WarpUI gotchas.
> 6. `feature-list.json` — pick the next feature.

## Startup Workflow

Before writing code, every session must:

1. Confirm the working directory with `pwd` (must be the repo root,
   `/Users/troye/Project/warp` or equivalent).
2. Read `progress.md` for the latest verified state and "next best step".
3. Read `feature-list.json` and pick the highest-priority feature whose
   status is `not_started` (or continue the single feature already
   `in_progress`).
4. Review recent commits with `git log --oneline -10`.
5. Run `./init.sh` to sync dependencies and run baseline verification.
6. If `./init.sh` fails, **stop**: fix the broken baseline before starting
   any new feature work.

## Working Rules

- Work on **one** feature at a time. Only one entry in `feature-list.json`
  may have status `in_progress`.
- Do not mark a feature `passing` just because code was added. Status
  changes require recorded evidence.
- Keep changes within the selected feature scope unless a blocker forces a
  narrow supporting fix. Record any such fix in `progress.md`.
- Do not silently change verification rules during implementation.
- Prefer durable repo artifacts (`progress.md`, `feature-list.json`,
  commits, `specs/`) over chat summaries.
- Follow the coding conventions documented in `WARP.md` (Rust style,
  WarpUI patterns, terminal-model locking rules, etc.).

## Required Artifacts

| File | Purpose |
| ---- | ------- |
| `AGENTS.md` | This file — operating rules for agents. |
| `ARCHITECTURE.md` | Code map: where each feature lives in the repo. |
| `DEVELOPMENT.md` | 二开 workflow, build accelerators, AI agent playbook. |
| `WARP.md` | Project commands, architecture, coding conventions. |
| `progress.md` | Session log and current verified state. |
| `feature-list.json` | Source of truth for feature status. |
| `init.sh` | Standard startup, install and verification entrypoint. |
| `session-handoff.md` | Optional compact handoff between sessions. |
| `clean-state-checklist.md` | End-of-session checklist. |

## Verification Standards

Match the verification command to the area you touched:

- **Format / lint baseline:** `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings`
- **Unit & integration tests:**
  `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2`
- **Doc tests:** `cargo test --doc`
- **Full presubmit:** `./script/presubmit`
- **Local smoke run:** `cargo run` (or with feature flags as documented in
  `WARP.md`).

If a feature requires UI verification, capture the smoke-run command and
the manual steps in `feature-list.json -> verification`.

## Definition Of Done

A feature is done only when **all** of the following are true:

- The target user-visible behavior is implemented.
- The required verification actually ran and passed.
- Evidence (command output, screenshot, log path, or commit hash) is
  recorded in `feature-list.json` and summarized in `progress.md`.
- Repo still starts cleanly via `./init.sh` from a fresh checkout.
- No clippy or fmt warnings introduced; presubmit still passes.

## End Of Session

Before ending a session:

1. Update `progress.md` with goal, completed work, verification run,
   evidence captured, commits, known risks, and next best step.
2. Update `feature-list.json` (status, evidence). Demote any
   `in_progress` feature to `not_started` or `blocked` if it is not
   actually finished — never leave silent half-done state.
3. Record any unresolved risk or blocker.
4. Commit with a descriptive message once the work is in a safe state.
5. Run through `clean-state-checklist.md`.
6. Leave the repo clean enough that the next session can run `./init.sh`
   immediately and continue.

## Out Of Scope For Agents

Unless explicitly requested by the user:

- Do not modify `Cargo.lock` outside what `cargo` produces naturally.
- Do not change `rust-toolchain.toml`, `deny.toml`, `.clippy.toml`,
  `.rustfmt.toml`, or CI workflows under `.github/`.
- Do not edit `LICENSE-*`, `CODE_OF_CONDUCT.md`, `SECURITY.md`.
- Do not push, force-push, or rewrite history on `main`.
