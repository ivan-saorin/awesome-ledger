# log operativo — awesome-ledger

## 2026-08-24 — M3 code: digest + run report + job pipeline (session ed5f811a, sigiled-claude)
**Where were we:** M0+M1+M2 on master, tests green; job cron wired but
never run; volume mount syntax unverified; operator prereqs (repo
public, Pages, GH_DEPLOY_KEY, STACK_BEARER) pending.
**Where were we going:** M3 — full enrollment live, digest, run reports.
**Done:** digest.rs (compose day chunk from events — quiet day = none;
ref awesome-ledger/digest/<date> unique per day because the memory store
upserts by provenance; tags [changed, chg0, awesome-ledger]; queue on
volume + flush POST /idx/mem0/chunks queue-until-acked — write verb dug
out of mem0 itself, docs/skill-memory-recall.md in ivan-saorin/memory).
update --report FILE writes the run report with parse skip-rate
(target ≥90%, PLAN risk); jobs/update.sh = full SPEC §4 pipeline with
soft-fail steps (state before publish; publish skipped without key;
digest queues without bearer). sigiled.toml [volumes] fixed to
"/data:rw" — the M0 "rw" shorthand had no mount path. 25 tests green;
live smoke: seed awesome-rust, drop 2 entries, +2 events, chunk queued.
**Deviations:** none of substance. python3 absent in container — job
script stays pure bash. Digest ack not yet live-verified (needs
STACK_BEARER, job-only).
**State:** master = M0..M3 code. No job run yet; runs list empty.
**Next (this session's second act):** close → ~5 min manifest refresh →
trigger run #1 (full index seed, ~674 fetches, one slow run) → run #2
(deltas only) → verify run reports on job branches, gh-pages push or
skip-note, digest chunk in mem0 or queued. Then M4 catalog.

## 2026-08-24 — M1: enrollment + fetcher + parser + state (session c84be46a, sigiled-claude)
**Where were we:** M0 closed same day (template v2, per-project image,
cron wired, update skeleton). This session opened ON the new image
(vm-awesome-ledger:df-65c7d1ec) — Dockerfile validated by the open
itself, no fallback shout; cargo works natively, zig workaround gone.
**Where were we going:** M1 — the fetch/diff half the M2 renderer
already consumes.
**Done:** five modules in ledger/: norm (canonical URL keys — https/www
fold, tracking-param strip, trailing-slash + .git strip, sorted query),
parse (pulldown-cmark event machine: heading-trail sections, first
non-image link wins, badge links — link-wrapped images — rejected by
empty link text; index scan keeps plain github.com/owner/repo only),
fetch (Source trait + blocking reqwest impl: raw.githubusercontent at
HEAD, etag conditional, README-path candidates, 3-try backoff, 1 s
politeness), store (per-list snapshots under state/lists/ with etag +
entry set; lists.json/events.jsonl/meta.json writers — events.jsonl
always exists post-run, renderer requires it), update (enroll = weekly
index scan ∪ extras ∖ blocklist, retire-on-index-drop except pins,
revive on return; set-diff by canonical key → events; silent first-seed;
parsed-0-on-populated = skip not wipe; 404×2 retires). model.rs types now
Serialize; Meta grew first_run + index_scanned. CLI: update --state
[--lists --no-index --enroll --limit --date]. 21 tests green incl. the
M1 smoke offline (FakeSource) AND live: 3 real lists seeded (awesome-rust
= 1755 entries), rerun all-304, hand-edited snapshot → exactly +2 added
events; live index scan enrolled 674 lists with sane categories.
**Deviations:** blocking reqwest, no tokio (sequential + politeness delay
= async buys nothing). Summary grew a `missing` count — first-404s were
invisible (found live: two dead 0xnr lists in the index). Job [volumes]
mount path still unverified (M3).
**State:** master = M0+M1, tests green, release binary builds. Site still
unpublished (operator prereqs pending).
**Next:** M3 — full seed run (one slow run, ~674 fetches), wire render +
publish + mem0 digest + run report into jobs/update.sh, verify volume
mount + cron live; operator: repo public, Pages on gh-pages,
GH_DEPLOY_KEY + STACK_BEARER secrets. Then M4 catalog registration.

## 2026-08-24 — M0: template v2 port + skeleton (session c9073156, sigiled-claude)
**Where were we:** M2 done out of order (renderer + publisher on master,
tests green); repo still on template v1 (mgr.toml, vendored server/, ext/,
build-ext.sh); no manifest, no cron, sessions built Rust via the
userspace rustup + zig workaround.
**Where were we going:** recover M0 then M1 (operator: "let's recover
M0/M1").
**Done:** sigiled.toml v2 (template pin vm-tmpl@0.1.0; `[workspace]
dockerfile` → per-project image; `[jobs.update]` cron 05:30 daily,
45 min, STACK_BEARER + GH_DEPLOY_KEY secrets; volume
awesome-ledger-data rw). New thin Dockerfile: FROM vm-base:0.1.0 +
build-essential + rustup 1.97.1 for uid 1000 — kills the zig workaround
from the next open. Removed v1 template artifacts (mgr.toml, server/,
ext/, build-ext.sh, .dockerignore). lists.toml registry (extra pins +
blocklist) + `update` subcommand skeleton reading it (registry.rs, toml
dep, unit tests) + jobs/update.sh (volume-backed state + cargo target
cache; render/publish/digest hooks land at M3).
**Deviations:** template pin is declarative only — vm-tmpl's
tools/sync-template.sh not vendored (needs template repo access; a later
`sync` adopts it). Job [volumes] schema for v2 unverified — mount path
resolved at M3 first live run; update.sh takes STATE_DIR override.
Code not built in this session (v1 container has no toolchain); session 2
scruple-builds on the new image before M1 — the reopen itself smoke-tests
the Dockerfile (DEC-25 shouts build_error instead of blocking).
**State:** M0 committed on master at close. Cargo.lock stale for the new
toml dep until first build.
**Next:** reopen (pays the image build) → cargo test → M1: index scan,
conditional fetcher, markdown → entry-set parser, URL normalization,
state store + silent seeding + set-diff events; smoke on 3 lists.

## 2026-08-24 — M2: renderer + publisher (session 9cdf5d5d, sigiled-claude)
**Where were we:** docs only (SPEC/PLAN ported, no code); Claude Design
mockups landed in `design/` — final direction is turns 6–7 (centered, no
black blocks) + 3c (archive), per the "look at version 7" commit.
**Where were we going:** M2 — site renderer + gh-pages publish, held
against the design.
**Done:** `ledger/` crate (Rust, askama + chrono): `render` builds the
whole static site (front / per-list / monthly archives / RSS sitewide and
per list / style.css / .nojekyll) from the M1 state contract
(lists.json + events.jsonl + meta.json, documented in ledger/README.md);
`publish` force-pushes the site as a fresh single-commit gh-pages branch
via GH_DEPLOY_KEY. Fixture state + 10 tests green, zero warnings; fixture
site rendered and visually verified against the artboards (desktop + phone).
**Deviations:** M2 taken before M0/M1 (operator asked for M2 directly —
the design had just landed). Renderer consumes the state contract, so M1
slots in behind it. Container base image has no rust/cc: session built
with rustup + zig cc as linker driver (userspace, throwaway).
**State:** master = code + docs, tests green. No site published yet —
publish needs the operator prereqs.
**Next:** M0 remainder (sigiled.toml v2 with `[workspace] dockerfile`
carrying the rust toolchain — kills the zig workaround; cron wiring), then
M1 (fetcher/parser/state). Operator prereqs for going live: repo public,
Pages serving gh-pages, GH_DEPLOY_KEY secret.

## 2026-08-24 — birth (as awesome-ledger)
**Note:** recreated from awesome-updates (2 doc commits) after rename
decision — recreate was cheaper than renaming through registry/keys/
volumes. Old project retired operator-side. Public name: The Awesome
Ledger; design mockups from Claude Design land in `design/` on master.

**Where were we:** stack organs (memory, changed, pdf2md, av2md) spec'd;
Ivan proposed a project tracking updates across GitHub awesome lists.
**Where were we going:** entry-level diff over the awesome ecosystem,
published as a GitHub Pages site (gh-pages branch, force-pushed by the
daily job — master stays sessions-only, so the SIGILED no-merge rule
holds in a single public repo) + RSS + daily mem0 digest (chg0
convention). Enrollment auto-seeded from sindresorhus/awesome.
**Done:** SPEC.md + PLAN.md (M0–M4). No code. Repo on template v1
(M0 ports it).
**Deviations:** none.
**State:** docs only, master clean.
**Next:** M0. Operator prereqs at M2: repo public, Pages on gh-pages,
GH_DEPLOY_KEY.
