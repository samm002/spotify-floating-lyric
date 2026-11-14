#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
  printf '%s\n' 'BLOCKED: Cargo is missing. Install the documented build dependencies.' >&2
  exit 2
fi
if [[ ! -f Cargo.lock ]]; then
  printf '%s\n' 'BLOCKED: Cargo.lock is missing. Generate and retain it before locked verification.' >&2
  exit 2
fi
cargo fmt --all -- --check
cargo build --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
