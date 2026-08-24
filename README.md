# The Awesome Ledger

**→ https://ivan-saorin.github.io/awesome-ledger** · [RSS](https://ivan-saorin.github.io/awesome-ledger/feed.xml)

A daily broadsheet of what's new across the GitHub
[awesome](https://github.com/sindresorhus/awesome) ecosystem. Every night a
small Rust program fetches the enrolled awesome lists, diffs them at entry
level, and prints the additions — every entry below the masthead was chosen
by an actual human curator somewhere.

- **Front page** — the last 30 days, newest first.
- **Per-list pages** — `/list/<owner>-<repo>/`: additions, removals, and the
  list's vital signs.
- **Monthly archives** — `/archive/YYYY-MM/`, kept forever.
- **RSS** — `/feed.xml` sitewide, plus one feed per list.

No analytics, no cookies, no JavaScript. The site lives on the `gh-pages`
branch, force-pushed by the job; `master` carries only code and docs.

## How it works

[SPEC.md](SPEC.md) is the design, [PLAN.md](PLAN.md) the milestones,
[`ledger/`](ledger/) the binary (renderer + publisher; the fetch/diff job
plugs into the state contract described in
[ledger/README.md](ledger/README.md)). The visual reference for every page
is [`design/`](design/). Operational history: [docs/log-operativo.md](docs/log-operativo.md).
