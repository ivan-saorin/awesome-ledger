//! The daily update flow (SPEC §4, steps 1–3): enrollment, fetch, parse,
//! set-diff → events, state write. Rendering/publish/digest are separate
//! steps (M3 wires them into the job).

use crate::fetch::{Doc, Source};
use crate::model::{Event, Kind, ListInfo, Meta};
use crate::registry::Registry;
use crate::store::{self, Snapshot, StoredEntry};
use crate::{norm, parse};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::path::Path;

const INDEX_OWNER: &str = "sindresorhus";
const INDEX_REPO: &str = "awesome";
const INDEX_RESCAN_DAYS: i64 = 7;
const DEAD_AFTER_MISSES: u32 = 2;

pub struct Options<'a> {
    pub lists_path: &'a Path,
    pub state_dir: &'a Path,
    /// false = extras-only (smoke runs); true = enroll from the index.
    pub use_index: bool,
    /// Force an index re-scan even if the last one is fresh.
    pub force_enroll: bool,
    /// Fetch at most N lists this run (smoke / partial runs).
    pub limit: Option<usize>,
    pub date: NaiveDate,
}

#[derive(Debug, Default)]
pub struct Summary {
    pub lists: usize,
    pub fetched: usize,
    pub unchanged: usize,
    pub seeded: usize,
    pub added: usize,
    pub removed: usize,
    /// READMEs that 404'd this run (first strike; two retire the list).
    pub missing: usize,
    pub died: Vec<String>,
    pub skipped: Vec<(String, String)>,
    pub edition: u64,
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "edition {} — {} lists: {} fetched, {} unchanged, {} seeded; +{} / -{} events",
            self.edition, self.lists, self.fetched, self.unchanged, self.seeded, self.added,
            self.removed
        )?;
        if self.missing > 0 {
            write!(f, "; {} missing (404)", self.missing)?;
        }
        if !self.died.is_empty() {
            write!(f, "; dead: {}", self.died.join(", "))?;
        }
        for (list, why) in &self.skipped {
            write!(f, "\nskipped {list}: {why}")?;
        }
        Ok(())
    }
}

pub fn run(src: &mut dyn Source, opts: &Options) -> Result<Summary> {
    let registry = Registry::load(opts.lists_path)?;
    let mut meta = store::load_meta(opts.state_dir)?;
    let mut lists: BTreeMap<String, ListInfo> = store::load_lists(opts.state_dir)?
        .into_iter()
        .map(|l| (l.full().to_lowercase(), l))
        .collect();

    enroll(src, opts, &registry, &mut meta, &mut lists)?;

    let mut summary = Summary {
        lists: lists.len(),
        ..Summary::default()
    };
    let mut events: Vec<Event> = Vec::new();

    let live: Vec<String> = lists
        .values()
        .filter(|l| !l.dead)
        .map(|l| l.full().to_lowercase())
        .take(opts.limit.unwrap_or(usize::MAX))
        .collect();

    for key in live {
        let info = lists.get_mut(&key).expect("live key");
        let (owner, repo) = (info.owner.clone(), info.repo.clone());
        let full = info.full();
        let mut snap = store::load_snapshot(opts.state_dir, &owner, &repo)?;
        let hint = snap.as_ref().and_then(|s| s.readme_path.as_deref());
        let etag = snap.as_ref().and_then(|s| s.etag.as_deref());

        match src.readme(&owner, &repo, hint, etag) {
            Err(e) => summary.skipped.push((full, format!("fetch: {e:#}"))),
            Ok(Doc::Unchanged) => {
                summary.unchanged += 1;
                if let Some(s) = snap.as_mut() {
                    if s.misses != 0 {
                        s.misses = 0;
                        store::save_snapshot(opts.state_dir, &owner, &repo, s)?;
                    }
                }
            }
            Ok(Doc::Gone) => {
                summary.missing += 1;
                let mut s = snap.unwrap_or_default();
                s.misses += 1;
                s.etag = None;
                if s.misses >= DEAD_AFTER_MISSES {
                    info.dead = true;
                    summary.died.push(full);
                }
                store::save_snapshot(opts.state_dir, &owner, &repo, &s)?;
            }
            Ok(Doc::New { body, etag, path }) => {
                summary.fetched += 1;
                let entries = to_entries(&body);
                let had = snap.as_ref().map(|s| s.entries.len()).unwrap_or(0);
                if entries.is_empty() && had > 0 {
                    // A populated list never parses to zero — that is a
                    // parser casualty, not a mass removal (SPEC §4.2).
                    summary.skipped.push((full, format!("parsed 0 entries (had {had})")));
                    continue;
                }
                match &snap {
                    None => summary.seeded += 1, // first sight seeds silently
                    Some(prev) => {
                        let (add, rem) = diff(&prev.entries, &entries, &full, opts.date);
                        summary.added += add.len();
                        summary.removed += rem.len();
                        events.extend(add);
                        events.extend(rem);
                    }
                }
                info.entries = entries.len() as u64;
                store::save_snapshot(
                    opts.state_dir,
                    &owner,
                    &repo,
                    &Snapshot {
                        etag,
                        readme_path: Some(path),
                        misses: 0,
                        entries,
                    },
                )?;
            }
        }
    }

    events.sort_by(|a, b| a.list.cmp(&b.list).then_with(|| a.name.cmp(&b.name)));
    store::append_events(opts.state_dir, &events)?;

    meta.edition = Some(meta.edition.unwrap_or(0) + 1);
    meta.first_run.get_or_insert(opts.date);
    summary.edition = meta.edition.unwrap_or(1);
    store::save_meta(opts.state_dir, &meta)?;
    store::save_lists(opts.state_dir, &lists.into_values().collect::<Vec<_>>())?;
    Ok(summary)
}

/// Enrollment (SPEC §3): index scan (weekly, or forced, or first run)
/// plus lists.toml extras, minus the blocklist. Lists that leave the
/// index retire (dead, kept); returners revive.
fn enroll(
    src: &mut dyn Source,
    opts: &Options,
    registry: &Registry,
    meta: &mut Meta,
    lists: &mut BTreeMap<String, ListInfo>,
) -> Result<()> {
    let scan_due = opts.force_enroll
        || lists.is_empty()
        || meta
            .index_scanned
            .is_none_or(|d| (opts.date - d).num_days() >= INDEX_RESCAN_DAYS);

    if opts.use_index && scan_due {
        match src
            .readme(INDEX_OWNER, INDEX_REPO, Some("readme.md"), None)
            .context("fetching the awesome index")?
        {
            Doc::New { body, .. } => {
                let index = parse::index_lists(&body);
                let in_index: std::collections::BTreeSet<String> = index
                    .iter()
                    .map(|l| format!("{}/{}", l.owner, l.repo).to_lowercase())
                    .collect();
                for l in index {
                    upsert(lists, &l.owner, &l.repo, &l.category, opts.date);
                }
                // Retire enrolled lists the index dropped — unless pinned.
                let pinned: std::collections::BTreeSet<String> = registry
                    .extra
                    .iter()
                    .map(|e| format!("{}/{}", e.owner, e.repo).to_lowercase())
                    .collect();
                for (key, info) in lists.iter_mut() {
                    if !in_index.contains(key) && !pinned.contains(key) {
                        info.dead = true;
                    }
                }
                meta.index_scanned = Some(opts.date);
            }
            Doc::Unchanged => {}
            Doc::Gone => eprintln!("warning: awesome index README not found; scan skipped"),
        }
    }

    for extra in &registry.extra {
        let category = extra.category.as_deref().unwrap_or("Uncategorized");
        upsert(lists, &extra.owner, &extra.repo, category, opts.date);
    }
    lists.retain(|_, l| !registry.is_blocked(&l.owner, &l.repo));
    Ok(())
}

fn upsert(
    lists: &mut BTreeMap<String, ListInfo>,
    owner: &str,
    repo: &str,
    category: &str,
    date: NaiveDate,
) {
    let key = format!("{owner}/{repo}").to_lowercase();
    match lists.get_mut(&key) {
        Some(info) => {
            info.dead = false;
            info.category = category.to_string();
        }
        None => {
            lists.insert(
                key,
                ListInfo {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    category: category.to_string(),
                    enrolled: Some(date),
                    dead: false,
                    entries: 0,
                },
            );
        }
    }
}

/// Parse a README body into the stored entry set: canonical-keyed,
/// first occurrence of a key wins.
fn to_entries(body: &str) -> Vec<StoredEntry> {
    let mut seen = std::collections::BTreeSet::new();
    parse::entries(body)
        .into_iter()
        .filter_map(|e| {
            let key = norm::canonical(&e.url)?;
            seen.insert(key.clone()).then_some(StoredEntry {
                key,
                name: e.name,
                url: e.url,
                description: e.description,
                section: e.section,
            })
        })
        .collect()
}

/// Set-diff by canonical key (SPEC §2): moves within a list are not
/// events; additions and removals are.
fn diff(
    old: &[StoredEntry],
    new: &[StoredEntry],
    list: &str,
    date: NaiveDate,
) -> (Vec<Event>, Vec<Event>) {
    let old_keys: std::collections::BTreeSet<&str> =
        old.iter().map(|e| e.key.as_str()).collect();
    let new_keys: std::collections::BTreeSet<&str> =
        new.iter().map(|e| e.key.as_str()).collect();
    let ev = |e: &StoredEntry, kind: Kind| Event {
        date,
        kind,
        list: list.to_string(),
        name: e.name.clone(),
        url: e.url.clone(),
        description: e.description.clone(),
        section: e.section.clone(),
    };
    let added = new
        .iter()
        .filter(|e| !old_keys.contains(e.key.as_str()))
        .map(|e| ev(e, Kind::Added))
        .collect();
    let removed = old
        .iter()
        .filter(|e| !new_keys.contains(e.key.as_str()))
        .map(|e| ev(e, Kind::Removed))
        .collect();
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Offline Source: bodies by "owner/repo"; None = Gone.
    struct Fake(HashMap<String, Option<String>>);

    impl Fake {
        fn set(&mut self, full: &str, body: &str) {
            self.0.insert(full.into(), Some(body.into()));
        }
    }

    impl Source for Fake {
        fn readme(
            &mut self,
            owner: &str,
            repo: &str,
            _hint: Option<&str>,
            _etag: Option<&str>,
        ) -> Result<Doc> {
            match self.0.get(&format!("{owner}/{repo}")) {
                Some(Some(body)) => Ok(Doc::New {
                    body: body.clone(),
                    etag: None,
                    path: "README.md".into(),
                }),
                _ => Ok(Doc::Gone),
            }
        }
    }

    fn body(entries: &[(&str, &str)]) -> String {
        let mut md = String::from("# List\n\n## Stuff\n\n");
        for (name, url) in entries {
            md.push_str(&format!("- [{name}]({url}) - blurb.\n"));
        }
        md
    }

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, d).unwrap()
    }

    fn write_lists_toml(dir: &Path, lists: &[&str]) -> std::path::PathBuf {
        let mut toml = String::new();
        for full in lists {
            let (o, r) = full.split_once('/').unwrap();
            toml.push_str(&format!("[[extra]]\nowner = \"{o}\"\nrepo = \"{r}\"\n\n"));
        }
        let p = dir.join("lists.toml");
        std::fs::write(&p, toml).unwrap();
        p
    }

    /// The M1 smoke (PLAN): seed 3 lists, change state, rerun, verify
    /// events — plus 304 short-circuit and death by double 404.
    #[test]
    fn seed_then_diff_then_die() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let lists_toml = write_lists_toml(tmp.path(), &["a/one", "b/two", "c/three"]);
        let mut src = Fake(HashMap::new());
        src.set("a/one", &body(&[("Alpha", "https://github.com/x/alpha")]));
        src.set(
            "b/two",
            &body(&[
                ("Beta", "https://github.com/x/beta"),
                ("Gamma", "https://github.com/x/gamma"),
            ]),
        );
        src.set("c/three", &body(&[("Delta", "https://github.com/x/delta")]));
        let opts = |d: u32| Options {
            lists_path: &lists_toml,
            state_dir: &state,
            use_index: false,
            force_enroll: false,
            limit: None,
            date: day(d),
        };

        // Day 1: everything seeds silently.
        let s1 = run(&mut src, &opts(1)).unwrap();
        assert_eq!((s1.seeded, s1.added, s1.removed, s1.edition), (3, 0, 0, 1));
        // Seeding is silent, but the file must exist for the renderer.
        assert_eq!(std::fs::read_to_string(state.join("events.jsonl")).unwrap(), "");

        // Day 2: b/two gains Epsilon and loses Gamma (reorder of the rest
        // included — must not produce events).
        src.set(
            "b/two",
            &body(&[
                ("Epsilon", "https://github.com/x/epsilon"),
                ("Beta", "https://github.com/x/beta/"), // cosmetic slash
            ]),
        );
        let s2 = run(&mut src, &opts(2)).unwrap();
        assert_eq!((s2.added, s2.removed, s2.seeded), (1, 1, 0));
        let events = std::fs::read_to_string(state.join("events.jsonl")).unwrap();
        assert_eq!(events.lines().count(), 2);
        assert!(events.contains(r#""kind":"added""#) && events.contains("Epsilon"));
        assert!(events.contains(r#""kind":"removed""#) && events.contains("Gamma"));
        // Renderer-side loader accepts what we wrote.
        let loaded = crate::model::load(&state).unwrap();
        assert_eq!(loaded.events.len(), 2);
        assert_eq!(loaded.meta.edition, Some(2));

        // Hand-edit the state (drop Alpha from a/one's snapshot): the next
        // run must re-report it as added.
        let snap_path = state.join("lists").join("a--one.json");
        let mut snap: Snapshot =
            serde_json::from_str(&std::fs::read_to_string(&snap_path).unwrap()).unwrap();
        snap.entries.clear();
        std::fs::write(&snap_path, serde_json::to_vec(&snap).unwrap()).unwrap();
        let s3 = run(&mut src, &opts(3)).unwrap();
        assert_eq!((s3.added, s3.removed), (1, 0));

        // Death: c/three vanishes — first run a miss, second marks dead.
        src.0.insert("c/three".into(), None);
        let s4 = run(&mut src, &opts(4)).unwrap();
        assert!(s4.died.is_empty());
        let s5 = run(&mut src, &opts(5)).unwrap();
        assert_eq!(s5.died, ["c/three"]);
        let lists = store::load_lists(&state).unwrap();
        assert!(lists.iter().find(|l| l.repo == "three").unwrap().dead);
        assert_eq!(lists.len(), 3, "dead lists are kept, not dropped");
    }

    #[test]
    fn empty_parse_of_populated_list_skips_not_wipes() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let lists_toml = write_lists_toml(tmp.path(), &["a/one"]);
        let mut src = Fake(HashMap::new());
        src.set("a/one", &body(&[("Alpha", "https://github.com/x/alpha")]));
        let opts = |d: u32| Options {
            lists_path: &lists_toml,
            state_dir: &state,
            use_index: false,
            force_enroll: false,
            limit: None,
            date: day(d),
        };
        run(&mut src, &opts(1)).unwrap();
        src.set("a/one", "nothing that parses to a list\n");
        let s = run(&mut src, &opts(2)).unwrap();
        assert_eq!(s.removed, 0);
        assert_eq!(s.skipped.len(), 1);
        let snap = store::load_snapshot(&state, "a", "one").unwrap().unwrap();
        assert_eq!(snap.entries.len(), 1, "old entry set survives the bad parse");
    }

    #[test]
    fn index_enrollment_retires_dropped_lists_but_not_pinned() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        // pin b/two; index initially has a/one and b/two
        let lists_toml = write_lists_toml(tmp.path(), &["b/two"]);
        let index_v1 = "# Awesome\n\n## Cat\n\n- [One](https://github.com/a/one#readme) - x.\n- [Two](https://github.com/b/two#readme) - y.\n";
        let index_v2 = "# Awesome\n\n## Cat\n\n- [Two](https://github.com/b/two#readme) - y.\n";
        let mut src = Fake(HashMap::new());
        src.set("sindresorhus/awesome", index_v1);
        src.set("a/one", &body(&[("Alpha", "https://github.com/x/alpha")]));
        src.set("b/two", &body(&[("Beta", "https://github.com/x/beta")]));
        let opts = |d: u32, force: bool| Options {
            lists_path: &lists_toml,
            state_dir: &state,
            use_index: true,
            force_enroll: force,
            limit: None,
            date: day(d),
        };
        let s1 = run(&mut src, &opts(1, false)).unwrap();
        assert_eq!(s1.lists, 2);
        // a/one leaves the index → retired; pinned b/two stays live.
        src.set("sindresorhus/awesome", index_v2);
        run(&mut src, &opts(2, true)).unwrap();
        let lists = store::load_lists(&state).unwrap();
        assert!(lists.iter().find(|l| l.repo == "one").unwrap().dead);
        assert!(!lists.iter().find(|l| l.repo == "two").unwrap().dead);
    }
}
