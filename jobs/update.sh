#!/usr/bin/env bash
# Daily update run (SPEC §4): fetch + diff → state, render → gh-pages
# publish, mem0 digest (queue-until-acked), run report. Executed by
# [jobs.update] in a fresh container from master on the workspace image.
# Leftover files in /workspace are committed to the job branch — the run
# report rides that into `recap`.
#
# Persistence: job containers mount NO volumes (verified against the
# SIGILED source — only resident apps get [app.volumes]), so state lives
# on the job-owned `state` branch: fetched here at run start, pushed
# after every update. Same doctrine as gh-pages — a branch the job owns,
# never merged. The digest queue rides inside it (queue/).
#
# Credentials: every container gets the project deploy key at
# $GIT_SSH_KEY (it pushes all branches) — publish and the state push use
# it; no separate secret.
set -uo pipefail
cd "$(dirname "$0")/.."
rm -f run-report.md

DATA_DIR="${DATA_DIR:-/tmp/ledger-run}"
STATE_REPO="${STATE_REPO:-$DATA_DIR/state}"
QUEUE_DIR="$STATE_REPO/queue"
SITE_DIR="${SITE_DIR:-$DATA_DIR/site}"
REMOTE="${REMOTE:-$(git -C . remote get-url origin)}"
GIT_ID=(-c user.name=awesome-ledger -c user.email=ledger@016180.xyz)
export GIT_SSH_COMMAND="ssh ${GIT_SSH_KEY:+-i $GIT_SSH_KEY} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
if [ -z "${GH_DEPLOY_KEY:-}${GH_DEPLOY_KEY_FILE:-}" ] && [ -n "${GIT_SSH_KEY:-}" ]; then
  export GH_DEPLOY_KEY_FILE="$GIT_SSH_KEY"
fi
mkdir -p "$DATA_DIR"

fail=0
step() {
  local label="$1"; shift
  echo "== $label"
  if ! "$@" 2>/tmp/step.err; then
    { echo "FAILED: $label"; sed 's/^/    /' /tmp/step.err | tail -6; } | tee -a run-report.md
    fail=1
  fi
  cat /tmp/step.err >&2 || true
}

BIN=(cargo run --release --locked --quiet --manifest-path ledger/Cargo.toml --)

# 0. state in: the state branch, or empty on the very first run.
STATE_NOTE=""
if ! git clone -q --single-branch --branch state "$REMOTE" "$STATE_REPO" 2>/tmp/step.err; then
  STATE_NOTE="state branch absent — fresh state (seed run)"
  echo "$STATE_NOTE"
  mkdir -p "$STATE_REPO"
fi

# 1–3. fetch, parse, set-diff → state files. (--report truncates the
# file, so the state note is re-appended after.)
step update "${BIN[@]}" update --lists lists.toml --state "$STATE_REPO" --report run-report.md
[ -n "$STATE_NOTE" ] && echo "$STATE_NOTE" >> run-report.md

# 4. digest chunk → memory service (internal DNS, no credential),
# queue-until-acked. Runs before the state push so queue leftovers are
# persisted with the state.
echo "== digest"
if out=$("${BIN[@]}" digest --state "$STATE_REPO" --queue "$QUEUE_DIR" 2>/tmp/step.err); then
  printf '\n## digest\n\n%s\n' "$out" >> run-report.md
  echo "$out"
else
  { echo "FAILED: digest"; sed 's/^/    /' /tmp/step.err | tail -6; } | tee -a run-report.md
  fail=1
fi

# 5. state out — before any publish attempt (PLAN risk: a failed push
# must not lose state).
# Subshell body: a mid-push failure must not leak the cwd change.
push_state() (
  cd "$STATE_REPO"
  if [ ! -d .git ]; then
    git init -q
    git checkout -q -b state
    git remote add origin "$REMOTE"
  fi
  git add -A
  git "${GIT_ID[@]}" commit -q -m "state: $(date -u +%F) run" || true
  git push -q origin HEAD:refs/heads/state
)
step state-push push_state

# 6. render + publish over the container deploy key.
step render "${BIN[@]}" render --state "$STATE_REPO" --out "$SITE_DIR"
if [ -n "${GH_DEPLOY_KEY:-}${GH_DEPLOY_KEY_FILE:-}" ]; then
  step publish "${BIN[@]}" publish --site "$SITE_DIR"
else
  echo "publish skipped: no deploy key in this container" | tee -a run-report.md
fi

exit $fail
