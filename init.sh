#!/usr/bin/env bash
#
# init.sh — Harness startup entrypoint for the Warp repo.
#
# Behavior:
#   1. cd into the repo root (the directory containing this script).
#   2. Install / sync Cargo build dependencies.
#   3. Run baseline verification (build + tests).
#   4. Print the standard start command.
#
# Override knobs (set as env vars to customize a session):
#   INSTALL_CMD    - command run during the install step
#   VERIFY_CMD     - command run during verification (e.g. fast smoke vs full)
#   START_CMD      - command printed (or run) as the standard start path
#   SKIP_INSTALL=1 - skip the install step (useful for repeated runs)
#   SKIP_VERIFY=1  - skip verification (e.g. when scaffolding a new harness)
#   RUN_START_COMMAND=1 - actually launch the app at the end

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

# Make sure cargo-installed binaries (cargo-binstall, diesel, etc.) are on PATH.
# Without this, install_cargo_build_deps loops forever re-installing cargo-binstall
# because `command -v cargo-binstall` keeps returning false.
export PATH="$HOME/.cargo/bin:$PATH"

# Defaults are tuned for the Warp Rust workspace. See WARP.md for context.
#
# Notes on defaults:
#   * INSTALL_CMD is intentionally a no-op (`true`). The upstream
#     `script/install_cargo_build_deps` does heavy work (cargo-binstall
#     self-update, brew, gcloud, docker, etc.) that is not required for
#     `cargo run` and is fragile in restricted-network environments.
#     Run `./script/bootstrap` manually if you need the full dev setup.
#   * VERIFY_CMD defaults to `cargo check` (default-members only) instead
#     of `--workspace`, because `--workspace` pulls in `command-signatures-v2`
#     which requires a Node.js/yarn toolchain via its build.rs.
#     Override with VERIFY_CMD=... for a heavier verification (see
#     session-handoff.md for examples).
INSTALL_CMD=${INSTALL_CMD:-"true"}
VERIFY_CMD=${VERIFY_CMD:-"cargo check"}
START_CMD=${START_CMD:-"cargo run"}

echo "==> Working directory: $PWD"
echo "==> Git HEAD: $(git rev-parse --short HEAD 2>/dev/null || echo 'not a git repo')"
echo "==> PATH includes ~/.cargo/bin: $(echo "$PATH" | tr ':' '\n' | grep -q "$HOME/.cargo/bin" && echo yes || echo no)"

if [ "${SKIP_INSTALL:-0}" = "1" ]; then
  echo "==> Skipping install step (SKIP_INSTALL=1)"
else
  echo "==> Syncing build dependencies"
  echo "    \$ $INSTALL_CMD"
  bash -c "$INSTALL_CMD"
fi

if [ "${SKIP_VERIFY:-0}" = "1" ]; then
  echo "==> Skipping verification step (SKIP_VERIFY=1)"
else
  echo "==> Running baseline verification"
  echo "    \$ $VERIFY_CMD"
  bash -c "$VERIFY_CMD"
fi

echo "==> Standard start command:"
echo "    \$ $START_CMD"

if [ "${RUN_START_COMMAND:-0}" = "1" ]; then
  echo "==> Launching app via START_CMD"
  exec bash -c "$START_CMD"
fi

echo "Set RUN_START_COMMAND=1 to launch the app directly via init.sh."
