# ledger/ — the awesome-ledger binary

Single Rust binary (SPEC §6). M2 ships the renderer + publisher; the
fetch/diff half (M1) plugs in by writing the state dir this crate reads.

## CLI

```
awesome-ledger render  --state DIR --out DIR [--date YYYY-MM-DD] [--site-url URL]
awesome-ledger publish --site DIR [--remote GIT_URL] [--branch NAME]
```

`render` builds the whole static site (front page, per-list pages, monthly
archives, RSS feeds, style.css, .nojekyll) from the state dir.
`publish` force-pushes the rendered dir as a fresh single-commit `gh-pages`
branch — auth via `GH_DEPLOY_KEY` (private key content) or
`GH_DEPLOY_KEY_FILE` (path). gh-pages is disposable by design (SPEC §1).

## State dir contract (written by the M1 update job)

- `lists.json` — array of `{owner, repo, category, enrolled (date, opt),
  dead (bool, opt), entries (current count)}` for every enrolled list.
- `events.jsonl` — append-only, one JSON object per line:
  `{date, kind: "added"|"removed", list: "owner/repo", name, url,
  description, section: [heading trail]}`.
- `meta.json` — optional `{edition, first_run}`; `edition` is the daily run
  counter shown in the masthead (falls back to counting distinct event days).

`fixtures/state/` is a working example and drives `tests/render.rs`.

## Design fidelity

The markup structure and stylesheet are held against
`design/The Awesome Ledger.dc.html` — turns 6–7 (centered, no black
blocks; desktop + 390px phone artboards) for front and per-list pages,
turn 3c for the monthly archive. Change the design there first, then here.
