# Session Handoff

> Compact handoff between sessions. Fill this in at the **end** of each
> session so the next session can resume in under a minute. For deep
> history, see `progress.md`.

## Currently Verified

- **Last green verification:** _none yet — Harness was just bootstrapped._
- **Verification command used:** `./init.sh`
- **Repo state:** clean working tree expected; `git status` should be
  empty after committing the harness scaffolding.

## Changes This Session

- Added Harness scaffolding files:
  `AGENTS.md`, `progress.md`, `feature-list.json`, `init.sh`,
  `session-handoff.md`, `clean-state-checklist.md`.
- No source code under `crates/`, `app/`, or `ui/` was modified.

## Still Broken Or Unverified

- `./init.sh` has not been executed yet in this session. The next
  session must run it to confirm the baseline is green.
- Platform bootstrap (`./script/bootstrap`) may be required on a fresh
  machine before `init.sh` can succeed — see `WARP.md`.

## Next Best Action

1. Run `./init.sh`.
2. If green, mark `harness-001` as `passing` in `feature-list.json` and
   record the test counts in its `evidence` array.
3. Pick the next highest-priority feature from `feature-list.json`,
   set its status to `in_progress`, and start work.

## Do Not Touch

- `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `.clippy.toml`,
  `.rustfmt.toml`, `.github/`, license files. See `AGENTS.md` for the
  full out-of-scope list.

## Quick Commands

```bash
# Standard startup + verification
./init.sh

# Fast iteration (skip the slow install step on repeated runs)
SKIP_INSTALL=1 ./init.sh

# Lint-only smoke (very fast)
SKIP_INSTALL=1 VERIFY_CMD="cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features --tests -- -D warnings" ./init.sh

# Full presubmit (run before opening a PR)
./script/presubmit

# Launch the app
cargo run
```
