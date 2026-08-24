# awesome-ledger — SPEC

(Public name: **The Awesome Ledger** — site at
ivan-saorin.github.io/awesome-ledger until a domain is chosen.)

Discovery feed over the GitHub "awesome" ecosystem. A daily job fetches
enrolled awesome lists, diffs them at **entry level** (not text level),
and publishes what's new as a static site on GitHub Pages of this same
repo, plus a daily digest chunk into mem0. The public artifact is the
site; the stack stays invisible.

Status: draft 0.1 — 2026-08-24. Decisions here + `docs/log-operativo.md`.

## 1. Repo layout (one repo, two audiences)

- **master** — sessions only: code, SPEC/PLAN, static README (what this
  is + link to the site). The job never commits here.
- **gh-pages** — job-owned, force-pushed each run with the rendered
  site. Disposable: rebuildable from state at any time. This respects
  the SIGILED rule (jobs never merge to master) while giving the job a
  publishable surface.
- Job branches `job-update-*` — run reports for `recap`, as usual.

Operator once: repo public + Pages enabled on `gh-pages`.

## 2. Model

- **List** — an enrolled awesome list: `owner/repo`, default branch,
  README path, category (taken from the sindresorhus index sections).
- **Entry** — a normalized item inside a list: key = canonical URL
  (lowercased host, stripped tracking params, trailing slash, `.git`);
  fields: name, url, description, source list, section path within the
  list (heading trail). Normalization by URL survives renames/reorders.
- **Event** — entry `added` or `removed` from a list on date D. Moves
  within a list are not events. An entry appearing in a *second* list is
  an event (it is a curation signal).
- **State** — per-list entry set + list README sha/etag, on volume
  `awesome-ledger-data`. First fetch of a list seeds silently.

## 3. Enrollment

- Seeded from `sindresorhus/awesome` (the master index): every linked
  GitHub list, category = index section. Re-scanned weekly — new lists
  in the index auto-enroll (seeded), removed ones are retired (kept in
  state, marked dead, no longer fetched).
- `lists.toml` on master can pin extra lists not in the index and
  blocklist noisy ones. Registry edits = sessions, as always.

## 4. Job flow (`[jobs.update]`, cron daily)

1. Sequential fetch of every enrolled list README via
   `raw.githubusercontent.com`, conditional (etag/If-Modified-Since),
   ~1 s politeness delay, 3 retries backoff; 404 twice in a row → mark
   dead, report once.
2. Parse markdown → entry set (list items with a link; description =
   tail text; section = heading trail). Parser failures degrade to
   "list skipped, reported", never abort the run.
3. Set-diff vs state → events. Update state.
4. Render site → force-push `gh-pages`.
5. Digest chunk → mem0: `tags:["changed","chg0","awesome-ledger"]`,
   text = "N added, M removed across K lists" + top 10 additions
   (name — one-liner — source list). Queue-until-acked.
6. Run report → job branch.

Full run ≈ 600 conditional fetches ≈ 10–15 min; most 304.

## 5. Site (gh-pages)

Visual reference: `design/` on master (Claude Design output — index,
list, archive, stylesheet). The M2 renderer is held against it; the
design files are the spec for markup structure and CSS.

Static HTML, no JS framework, built by the job:
- **/** — last 30 days, newest first, grouped by day then source list.
- **/list/<owner>-<repo>/** — per-list page: recent events + current
  entry count.
- **/archive/YYYY-MM/** — monthly archives, forever.
- **/feed.xml** — RSS of additions (last 200). The actual product for
  anyone who wants to follow.
- Footer: "tracked lists: K · entries: N · updated daily". No analytics,
  no cookies. Generated-by line links the repo.

## 6. Runtime

- Rust, single binary, class `job`, no ports. Deps: tokio, reqwest,
  pulldown-cmark (parse), askama or plain format! templates (render),
  git2 or shelling to git (gh-pages push), serde, toml, sha2.
- Volume `awesome-ledger-data`: `state/`, `queue/`.
- Secrets: `GH_DEPLOY_KEY` (write key for this repo, gh-pages push),
  env only. The mem0 digest needs none: the job talks to the memory
  service over the internal network (`http://memory:8080`) — auth lives
  at the edge, for callers outside the stack network.

## 7. Non-goals (v1)

- No stars/activity enrichment per entry (GitHub API rate budget; v2).
- No dedup judgment ("is this actually cool") — the feed reports
  curation, it does not curate. LLM curation would be a separate job.
- No web UI beyond static Pages; no search on-site (the RSS + memory
  cover it; site search is v2 with a static index if wanted).

## 8. v2 candidates
1. Entry enrichment: stars, language, last-commit via GitHub API
   (token, budgeted) — shown on the site, enables "notable" filtering.
2. Weekly "notable additions" section chosen by a genie job (separate
   project per the LLM-curation rule).
3. Static search index (lunr-style) on the site.
4. Per-category RSS feeds.

## 9. Open questions
- Removals on the site: shown (transparency) or additions-only (signal)?
  Draft: additions on the front page, removals only on per-list pages.
- mem0 digest daily even when quiet? Draft: skip the chunk on zero-event
  days.
