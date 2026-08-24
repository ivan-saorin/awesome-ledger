#!/usr/bin/env bash
# Daily update run (SPEC §4): fetch + diff → state, render → gh-pages
# publish, mem0 digest (queue-until-acked), run report. Executed by
# [jobs.update] in a fresh container from master on the workspace image.
# Leftover files in /workspace are committed to the job branch — the run
# report rides that into `recap`.
set -uo pipefail
cd "$(dirname "$0")/.."
rm -f run-report.md

# Volume-backed data (SPEC §6): the declared mount is /data (sigiled.toml
# [volumes]). Fall back to a workspace dir when absent so smoke runs
# outside the job still work — but shout, because state off the volume
# dies with the container.
if [ -z "${DATA_DIR:-}" ]; then
  if [ -d /data ] && [ -w /data ]; then
    DATA_DIR=/data
  else
    DATA_DIR=/workspace/.data
    echo "WARNING: /data volume not mounted — state at $DATA_DIR is container-local" | tee -a run-report.md
  fi
fi
STATE_DIR="${STATE_DIR:-$DATA_DIR/state}"
QUEUE_DIR="${QUEUE_DIR:-$DATA_DIR/queue}"
SITE_DIR="${SITE_DIR:-$DATA_DIR/site}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$DATA_DIR/target}"
mkdir -p "$STATE_DIR"

BIN=(cargo run --release --locked --quiet --manifest-path ledger/Cargo.toml --)
fail=0
step() {
  echo "== $1"; shift
  if ! "$@"; then
    echo "FAILED: $1" | tee -a run-report.md
    fail=1
  fi
}

# 1–3. fetch, parse, set-diff → state. State is written before any
# publish attempt (PLAN risk: a failed push must not lose state).
step update "${BIN[@]}" update --lists lists.toml --state "$STATE_DIR" --report run-report.md

# 4. render + publish. No deploy key yet (operator prereq) = site still
# renders to the volume, push is skipped, run stays green.
step render "${BIN[@]}" render --state "$STATE_DIR" --out "$SITE_DIR"
if [ -n "${GH_DEPLOY_KEY:-}${GH_DEPLOY_KEY_FILE:-}" ]; then
  step publish "${BIN[@]}" publish --site "$SITE_DIR"
else
  echo "publish skipped: GH_DEPLOY_KEY not set" | tee -a run-report.md
fi

# 5. digest chunk → memory service (internal DNS, no credential),
# queue-until-acked: an unreachable service leaves chunks queued.
echo "== digest"
if out=$("${BIN[@]}" digest --state "$STATE_DIR" --queue "$QUEUE_DIR" 2>&1); then
  printf '\n## digest\n\n%s\n' "$out" >> run-report.md
  echo "$out"
else
  echo "$out"
  echo "FAILED: digest" | tee -a run-report.md
  fail=1
fi

exit $fail
