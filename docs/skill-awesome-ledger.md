---
name: awesome-ledger
description: Answer "what's new in awesome-X / across the awesome ecosystem" from the stack's awesome-ledger service — memory search for recall, the site for browsing. Also for enrolling, removing, or blocklisting awesome lists. Trigger phrases: "what's new in awesome", "awesome additions", "new in awesome-rust", "the ledger", "enroll a list", "blocklist a list".
---

# awesome-ledger — what's new across the awesome ecosystem

Job-class service, no HTTP surface of its own. Every night (`update` job,
05:30) it fetches the enrolled GitHub awesome lists, diffs them at entry
level, publishes the static site + RSS on `gh-pages`, and drops one digest
chunk in mem0 per eventful day. It answers in memory and on the site.

## Recall — the machine leg

    GET https://memory.016180.xyz/search?q=<topic>&idx=mem0&tags=awesome-ledger

One chunk per eventful day: `"awesome-ledger YYYY-MM-DD: N added, M removed
across K lists"` + the top additions with their curator blurbs + the site
link; `ref` is `awesome-ledger/digest/<date>`. Quiet days write **no**
chunk — zero hits for a date means nothing happened, not a broken job.
Delivery is queue-until-acked, so a chunk may land a day late after a
memory-service outage.

## Browsing — the human leg

- Front page (last 30 days): https://ivan-saorin.github.io/awesome-ledger
- Per-list pages: `/list/<owner>-<repo>/` — additions, removals, vital signs
- Monthly archives: `/archive/YYYY-MM/`, kept forever
- RSS: `/feed.xml` sitewide, plus one feed per list

## Enrollment edits

Enrollment is the sindresorhus/awesome index scan merged with `lists.toml`
(explicit adds + blocklist) on master. To change it: open a session on
`awesome-ledger`, edit `lists.toml`, close. The next nightly run picks it
up; a newly enrolled list seeds silently (no flood of fake "additions").

## Deeper recap

Each run leaves a report on its `job-update-*` branch: `recap
awesome-ledger update`, or in a session `GET /git/show?ref=origin/job-update-…`.
State lives on the job-owned `state` branch — read-only for humans, never
merged.
