# awesome-ledger — PLAN

## M0 — template port + skeleton
- sigiled.toml v2 (`class="job"`), Dockerfile (rust:slim → debian:slim,
  git present for gh-pages push), non-root.
- Binary reads lists.toml, prints enrolled count, exits 0. Cron wired.

## M1 — enrollment + parser + state
- Index scan (sindresorhus/awesome → lists + categories), lists.toml
  merge/blocklist, conditional fetcher, markdown → entry-set parser,
  URL normalization, state store, silent seeding, set-diff events.
- Smoke: seed 3 lists, hand-edit state, rerun, verify events.

## M2 — site + publish
- Renderer (index / per-list / monthly archive / feed.xml) built from
  the `design/` mockups (same markup structure, askama-templated), gh-pages
  force-push via deploy key, static README on master linking the site.
- Operator: repo public, Pages on gh-pages, add GH_DEPLOY_KEY secret.
- Smoke: full render from seeded state, site reachable.

## M3 — full enrollment + mem0 digest
- Enroll the whole index (seed run), daily cron live, digest chunk with
  queue-until-acked, run reports for recap.
- Smoke: two consecutive real runs; day 2 shows only true deltas.

## M4 — catalog
- Register as service (job) + skill note: "what's new in awesome-X →
  memory search or the site; enrollment edits via session".

## Risks
- Parser vs the wild variety of awesome list formats (tables, nested
  bullets, badges): parse permissively, measure skip-rate in M3 on the
  full index; a skip-rate report per run keeps it honest. Target ≥90%
  of lists parsed clean; the rest are blocklist candidates, not crashes.
- First full seed writes no events but fetches ~600 files uncached —
  keep the 1 s delay, it is one slow run.
- gh-pages force-push races nothing (single sequential job), but a
  failed push must not lose state: state commits before publish.
