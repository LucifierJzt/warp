# Clean State Checklist

> Run through this list before ending every session. The goal: the next
> session can `./init.sh` and immediately keep going.

## Repo Hygiene

- [ ] `git status` is either clean, or every uncommitted change is
      intentional and recorded in `progress.md`.
- [ ] No stray scratch files (`*.tmp`, `*.bak`, ad-hoc scripts in repo
      root) committed by accident.
- [ ] No secrets, tokens, or local paths committed.

## Verification

- [ ] `./init.sh` was run **at least once** during the session and
      finished green — or the failure is documented in `progress.md`
      under "Known risk or unresolved issue".
- [ ] If a feature was finished, the verification command from its
      `feature-list.json` entry actually ran and passed.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings`
      passes (or `-D warnings` regressions are explicitly justified).

## Documentation

- [ ] `progress.md` has a new session entry, including:
      goal, completed, verification run, evidence, commits, known risks,
      next best step.
- [ ] `feature-list.json` reflects reality:
      - At most one feature is `in_progress`.
      - Any feature marked `passing` has non-empty `evidence`.
      - Any feature marked `blocked` has its blocker described in
        `progress.md` or in its `notes`.
- [ ] `session-handoff.md` is updated with the new "Next Best Action".

## Build / Run

- [ ] `cargo build` (or `cargo check --workspace`) still succeeds from
      a clean shell.
- [ ] `cargo run` still launches the app, or the regression is
      documented as a blocker.

## Commits

- [ ] Each commit message describes what changed, not how. Example:
      `harness: add init.sh, AGENTS.md and feature-list.json`.
- [ ] Harness-only changes (this file, `progress.md`, etc.) are kept
      in their own commits, separate from product feature commits.
- [ ] No force-push, no history rewrite on `master`.
