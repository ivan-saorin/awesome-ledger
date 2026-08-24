# log operativo — awesome-ledger

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
