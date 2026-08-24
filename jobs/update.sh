#!/usr/bin/env bash
# Daily update run (SPEC Â§4), executed by [jobs.update] in a fresh
# container from master on the workspace image (rust toolchain present).
set -euo pipefail
cd "$(dirname "$0")/.."

# Volume-backed dirs (SPEC Â§6): state survives the disposable container;
# the cargo target cache keeps rebuilds warm across runs. Until the
# volume mount path is verified live (M3), STATE_DIR can be overridden.
DATA_DIR="${DATA_DIR:-/workspace/.data}"
STATE_DIR="${STATE_DIR:-$DATA_DIR/state}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$DATA_DIR/target}"
mkdir -p "$STATE_DIR"

cargo run --release --locked --manifest-path ledger/Cargo.toml -- \
    update --lists lists.toml --state "$STATE_DIR"

# M3 adds: render + gh-pages publish + mem0 digest + run report.
