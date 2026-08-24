//! View-model layer: turns loaded state into dumb page structs the askama
//! templates can render without logic. All text (dates, stats, badges) is
//! precomputed here; templates only loop and print.
//!
//! Layout kinds mirror the design (design/The Awesome Ledger.dc.html, turns
//! 6–7): the first item of a day/section is the "hero" (full-width preview),
//! the next two get side previews, the rest are text-only. Items without a
//! GitHub OpenGraph card degrade to text.

use crate::model::{commafy, github_og, slug_of, Event, Kind, ListInfo, State};
use askama::Template;
use chrono::{Datelike, NaiveDate};
use std::collections::{BTreeMap, HashMap};

const FRONT_WINDOW_DAYS: i64 = 30;
const QUIET_DAY_MAX: usize = 2;
const ARCHIVE_NAMES_SHOWN: usize = 8;
const LIST_ADDITIONS_SHOWN: usize = 40;
const LIST_REMOVALS_SHOWN: usize = 20;
pub const FEED_ITEMS: usize = 200;
pub const LIST_FEED_ITEMS: usize = 50;

pub struct Item {
    pub name: String,
    pub url: String,
    pub via: String,      // "AWESOME-RUST"
    pub via_href: String, // "list/<slug>/" — template prefixes {{ root }}
    pub date: String,     // "24 AUG" — shown on per-list pages
    pub desc: String,
    pub layout: &'static str, // "hero" | "side" | "text"
    pub img: String,          // OpenGraph URL or ""
    pub img_alt: String,
    pub badge: String, // "" or "★ ALSO …"
}

pub struct DaySection {
    pub heading: String, // "Today" / "Sat 23"
    pub count_label: String,
    pub today: bool,
    pub quiet: bool,
    pub items: Vec<Item>,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexPage {
    pub root: String,
    pub ed_line: String,
    pub note_lists: String,
    pub foot_made: String,
    pub foot_stats: String,
    pub about_url: String,
    pub days: Vec<DaySection>,
}

pub struct Removal {
    pub date: String,
    pub name: String,
    pub note: String,
}

#[derive(Template)]
#[template(path = "list.html")]
pub struct ListPage {
    pub root: String,
    pub name: String,
    pub stats1: String,
    pub stats2: String,
    pub spark: Vec<u32>,
    pub additions: Vec<Item>,
    pub removals: Vec<Removal>,
    pub foot_line: String,
}

pub struct ArcName {
    pub name: String,
    pub url: String,
    pub star: String, // "" or "★2"
}

pub struct ArchiveRow {
    pub day: String, // "SUN 24"
    pub quiet: bool,
    pub names: Vec<ArcName>,
    pub more: usize,
    pub count: String,
}

#[derive(Template)]
#[template(path = "archive.html")]
pub struct ArchivePage {
    pub root: String,
    pub title: String, // "August 2026"
    pub stats: String,
    pub prev_label: String,
    pub prev_href: String, // "" = disabled
    pub next_label: String,
    pub next_href: String,
    pub rows: Vec<ArchiveRow>,
}

pub struct FeedEntry {
    pub name: String,
    pub url: String,
    pub via: String, // "awesome-rust"
    pub desc: String,
    pub date: NaiveDate,
}

pub struct Site {
    pub index: IndexPage,
    pub lists: Vec<(String, ListPage)>, // (slug, page)
    pub archives: Vec<(String, ArchivePage)>, // ("2026-08", page)
    pub feed: Vec<FeedEntry>,
    pub list_feeds: Vec<(String, String, Vec<FeedEntry>)>, // (slug, list repo name, entries)
}

fn short_date(d: NaiveDate) -> String {
    d.format("%-d %b").to_string().to_uppercase() // "24 AUG"
}

fn day_label(d: NaiveDate) -> String {
    d.format("%a %-d").to_string().to_uppercase() // "SUN 24"
}

/// Cross-list index: canonical url -> (date -> list repo names added that day).
fn cross_index(events: &[Event]) -> HashMap<&str, BTreeMap<NaiveDate, Vec<&str>>> {
    let mut m: HashMap<&str, BTreeMap<NaiveDate, Vec<&str>>> = HashMap::new();
    for ev in events.iter().filter(|e| e.kind == Kind::Added) {
        m.entry(ev.url.as_str())
            .or_default()
            .entry(ev.date)
            .or_default()
            .push(ev.list_repo());
    }
    m
}

fn make_item(ev: &Event, idx: usize, with_previews: bool, badge: String) -> Item {
    let img = if with_previews && idx <= 2 {
        github_og(&ev.url).unwrap_or_default()
    } else {
        String::new()
    };
    let layout = if img.is_empty() {
        "text"
    } else if idx == 0 {
        "hero"
    } else {
        "side"
    };
    Item {
        name: ev.name.clone(),
        url: ev.url.clone(),
        via: ev.list_repo().to_uppercase(),
        via_href: format!(
            "list/{}/",
            slug_of(
                ev.list.split('/').next().unwrap_or(""),
                ev.list_repo()
            )
        ),
        date: short_date(ev.date),
        desc: ev.description.clone(),
        layout,
        img_alt: format!("Preview of {} on GitHub", ev.name),
        img,
        badge,
    }
}

fn month_label(y: i32, m: u32) -> String {
    NaiveDate::from_ymd_opt(y, m, 1)
        .unwrap()
        .format("%B %Y")
        .to_string()
}

fn prev_month(y: i32, m: u32) -> (i32, u32) {
    if m == 1 {
        (y - 1, 12)
    } else {
        (y, m - 1)
    }
}

fn next_month(y: i32, m: u32) -> (i32, u32) {
    if m == 12 {
        (y + 1, 1)
    } else {
        (y, m + 1)
    }
}

pub fn build(state: &State, today: NaiveDate, repo_url: &str) -> Site {
    let cross = cross_index(&state.events);
    let live_lists: Vec<&ListInfo> = state.lists.iter().filter(|l| !l.dead).collect();
    let lists_tracked = live_lists.len() as u64;
    let entries_on_record: u64 = live_lists.iter().map(|l| l.entries).sum();

    // ---- front page -------------------------------------------------------
    let window_start = today - chrono::Duration::days(FRONT_WINDOW_DAYS - 1);
    let mut days: Vec<DaySection> = Vec::new();
    let mut d = today;
    while d >= window_start {
        // Dedupe cross-listed additions: one item per canonical URL per day.
        let mut seen: Vec<&str> = Vec::new();
        let mut evs: Vec<&Event> = Vec::new();
        for ev in state
            .events
            .iter()
            .filter(|e| e.kind == Kind::Added && e.date == d)
        {
            if !seen.contains(&ev.url.as_str()) {
                seen.push(&ev.url);
                evs.push(ev);
            }
        }
        if !evs.is_empty() {
            let is_today = d == today;
            // Cross-listed additions lead the day (the "twice chosen" hero),
            // stable alphabetical order behind them.
            evs.sort_by_key(|ev| {
                std::cmp::Reverse(
                    cross
                        .get(ev.url.as_str())
                        .and_then(|m| m.get(&d))
                        .map(|ls| ls.len())
                        .unwrap_or(1),
                )
            });
            let items = evs
                .iter()
                .enumerate()
                .map(|(i, ev)| {
                    let lists_today = cross
                        .get(ev.url.as_str())
                        .and_then(|m| m.get(&d))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let badge = front_badge(ev.list_repo(), lists_today);
                    make_item(ev, i, is_today, badge)
                })
                .collect::<Vec<_>>();
            days.push(DaySection {
                heading: if is_today {
                    "Today".into()
                } else {
                    d.format("%a %-d").to_string()
                },
                count_label: format!("{} NEW", evs.len()),
                today: is_today,
                quiet: evs.len() <= QUIET_DAY_MAX,
                items,
            });
        }
        d -= chrono::Duration::days(1);
    }

    let edition = state
        .meta
        .edition
        .unwrap_or_else(|| distinct_event_days(&state.events));
    let index = IndexPage {
        root: String::new(),
        ed_line: format!(
            "ED. № {} · {}",
            edition,
            today.format("%a %-d %b %Y").to_string().to_uppercase()
        ),
        note_lists: format!("{} lists diffed nightly", lists_tracked),
        foot_made: "SET DAILY BY A SMALL RUST PROGRAM ON A HOME SERVER".into(),
        foot_stats: format!(
            "TRACKED LISTS: {} · ENTRIES ON RECORD: {}",
            commafy(lists_tracked),
            commafy(entries_on_record)
        ),
        about_url: repo_url.to_string(),
        days,
    };

    // ---- per-list pages ---------------------------------------------------
    let mut list_pages = Vec::new();
    let mut list_feeds = Vec::new();
    for li in &live_lists {
        let full = li.full();
        let mut adds: Vec<&Event> = state
            .events
            .iter()
            .filter(|e| e.kind == Kind::Added && e.list == full)
            .collect();
        let mut rems: Vec<&Event> = state
            .events
            .iter()
            .filter(|e| e.kind == Kind::Removed && e.list == full)
            .collect();
        if adds.is_empty() && rems.is_empty() {
            continue; // nothing to say yet — no page until first event
        }
        adds.reverse(); // newest first
        rems.reverse();

        let added_this_year = adds.iter().filter(|e| e.date.year() == today.year()).count();
        let removed_this_year = rems.iter().filter(|e| e.date.year() == today.year()).count();
        let tracked_since = li
            .enrolled
            .or_else(|| adds.last().map(|e| e.date))
            .map(|d| d.format("%B %Y").to_string().to_uppercase())
            .unwrap_or_else(|| "TODAY".into());

        // 12 monthly addition counts, oldest → newest, as bar heights (%).
        let mut months: Vec<(i32, u32)> = Vec::with_capacity(12);
        let (mut y, mut m) = (today.year(), today.month());
        for _ in 0..12 {
            months.push((y, m));
            (y, m) = prev_month(y, m);
        }
        months.reverse();
        let counts: Vec<usize> = months
            .iter()
            .map(|&(y, m)| {
                adds.iter()
                    .filter(|e| e.date.year() == y && e.date.month() == m)
                    .count()
            })
            .collect();
        let max = counts.iter().copied().max().unwrap_or(0).max(1);
        let spark: Vec<u32> = counts
            .iter()
            .map(|&c| ((c * 100 / max) as u32).max(8))
            .collect();

        let additions: Vec<Item> = adds
            .iter()
            .take(LIST_ADDITIONS_SHOWN)
            .enumerate()
            .map(|(i, ev)| {
                let others: Vec<&str> = cross
                    .get(ev.url.as_str())
                    .and_then(|m| m.get(&ev.date))
                    .map(|ls| {
                        ls.iter()
                            .filter(|l| **l != ev.list_repo())
                            .copied()
                            .collect()
                    })
                    .unwrap_or_default();
                let badge = if others.is_empty() {
                    String::new()
                } else {
                    format!("★ ALSO IN {}", others[0].to_uppercase())
                };
                make_item(ev, i, true, badge)
            })
            .collect();
        let removals: Vec<Removal> = rems
            .iter()
            .take(LIST_REMOVALS_SHOWN)
            .map(|ev| Removal {
                date: short_date(ev.date),
                name: ev.name.clone(),
                note: "// STRUCK FROM THE LEDGER".into(),
            })
            .collect();

        let page = ListPage {
            root: "../../".into(),
            name: li.repo.clone(),
            stats1: format!(
                "{} ENTRIES TODAY · {} ADDED THIS YEAR · {} REMOVED",
                commafy(li.entries),
                added_this_year,
                removed_this_year
            ),
            stats2: format!(
                "TRACKED SINCE {} · MAINTAINED BY {}",
                tracked_since,
                li.owner.to_uppercase()
            ),
            spark,
            additions,
            removals,
            foot_line: format!(
                "ONE OF {} LISTS TRACKED BY THE AWESOME LEDGER",
                commafy(lists_tracked)
            ),
        };
        list_pages.push((li.slug(), page));

        let feed: Vec<FeedEntry> = adds
            .iter()
            .take(LIST_FEED_ITEMS)
            .map(|ev| feed_entry(ev))
            .collect();
        list_feeds.push((li.slug(), li.repo.clone(), feed));
    }

    // ---- archives ---------------------------------------------------------
    let mut archives = Vec::new();
    if let Some(first) = state.events.first().map(|e| e.date) {
        let (first_y, first_m) = (first.year(), first.month());
        let (mut y, mut m) = (first_y, first_m);
        loop {
            archives.push(archive_page(state, &cross, y, m, today, (first_y, first_m)));
            if (y, m) == (today.year(), today.month()) {
                break;
            }
            (y, m) = next_month(y, m);
        }
    }

    // ---- global feed ------------------------------------------------------
    let mut feed: Vec<FeedEntry> = Vec::new();
    let mut seen: Vec<(&str, NaiveDate)> = Vec::new();
    for ev in state.events.iter().rev().filter(|e| e.kind == Kind::Added) {
        if seen.contains(&(ev.url.as_str(), ev.date)) {
            continue; // cross-listed same day: one feed item
        }
        seen.push((&ev.url, ev.date));
        feed.push(feed_entry(ev));
        if feed.len() >= FEED_ITEMS {
            break;
        }
    }

    Site {
        index,
        lists: list_pages,
        archives,
        feed,
        list_feeds,
    }
}

fn feed_entry(ev: &Event) -> FeedEntry {
    FeedEntry {
        name: ev.name.clone(),
        url: ev.url.clone(),
        via: ev.list_repo().to_string(),
        desc: ev.description.clone(),
        date: ev.date,
    }
}

fn front_badge(own_repo: &str, lists_today: &[&str]) -> String {
    if lists_today.len() < 2 {
        return String::new();
    }
    let other = lists_today
        .iter()
        .find(|l| **l != own_repo)
        .copied()
        .unwrap_or(own_repo);
    if lists_today.len() == 2 {
        format!(
            "★ ALSO CURATED IN {} — TWICE CHOSEN TODAY",
            other.to_uppercase()
        )
    } else {
        format!("★ CURATED IN {} LISTS TODAY", lists_today.len())
    }
}

fn distinct_event_days(events: &[Event]) -> u64 {
    let mut days: Vec<NaiveDate> = events.iter().map(|e| e.date).collect();
    days.sort();
    days.dedup();
    days.len() as u64
}

fn archive_page(
    state: &State,
    cross: &HashMap<&str, BTreeMap<NaiveDate, Vec<&str>>>,
    y: i32,
    m: u32,
    today: NaiveDate,
    first_month: (i32, u32),
) -> (String, ArchivePage) {
    let ym = format!("{:04}-{:02}", y, m);
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut busiest: (usize, Option<NaiveDate>) = (0, None);
    let mut rows: Vec<ArchiveRow> = Vec::new();

    let last_day = {
        let (ny, nm) = next_month(y, m);
        NaiveDate::from_ymd_opt(ny, nm, 1).unwrap() - chrono::Duration::days(1)
    };
    let mut d = last_day.min(today);
    while d.month() == m && d.year() == y {
        let mut seen: Vec<&str> = Vec::new();
        let mut names: Vec<ArcName> = Vec::new();
        for ev in state
            .events
            .iter()
            .filter(|e| e.kind == Kind::Added && e.date == d)
        {
            if seen.contains(&ev.url.as_str()) {
                continue;
            }
            seen.push(&ev.url);
            let n = cross
                .get(ev.url.as_str())
                .and_then(|mm| mm.get(&d))
                .map(|ls| ls.len())
                .unwrap_or(1);
            names.push(ArcName {
                name: ev.name.clone(),
                url: ev.url.clone(),
                star: if n > 1 { format!("★{}", n) } else { String::new() },
            });
        }
        removed += state
            .events
            .iter()
            .filter(|e| e.kind == Kind::Removed && e.date == d)
            .count();
        let count = names.len();
        added += count;
        if count > busiest.0 {
            busiest = (count, Some(d));
        }
        let more = names.len().saturating_sub(ARCHIVE_NAMES_SHOWN);
        names.truncate(ARCHIVE_NAMES_SHOWN);
        rows.push(ArchiveRow {
            day: day_label(d),
            quiet: count == 0,
            names,
            more,
            count: count.to_string(),
        });
        d -= chrono::Duration::days(1);
    }

    let busiest_txt = match busiest.1 {
        Some(bd) => format!(" · BUSIEST DAY: {} ({})", short_date(bd), busiest.0),
        None => String::new(),
    };
    let (py, pm) = prev_month(y, m);
    let (ny, nm) = next_month(y, m);
    let has_prev = (py, pm) >= first_month;
    let has_next = (ny, nm) <= (today.year(), today.month());
    let mon = |mm: u32| {
        NaiveDate::from_ymd_opt(2000, mm, 1)
            .unwrap()
            .format("%b")
            .to_string()
            .to_uppercase()
    };
    let page = ArchivePage {
        root: "../../".into(),
        title: month_label(y, m),
        stats: format!("{} ADDED · {} REMOVED{}", added, removed, busiest_txt),
        prev_label: mon(pm),
        prev_href: if has_prev {
            format!("../{:04}-{:02}/", py, pm)
        } else {
            String::new()
        },
        next_label: mon(nm),
        next_href: if has_next {
            format!("../{:04}-{:02}/", ny, nm)
        } else {
            String::new()
        },
        rows,
    };
    (ym, page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Meta;

    fn ev(date: &str, kind: Kind, list: &str, name: &str, url: &str) -> Event {
        Event {
            date: date.parse().unwrap(),
            kind,
            list: list.into(),
            name: name.into(),
            url: url.into(),
            description: String::new(),
            section: vec![],
        }
    }

    fn li(owner: &str, repo: &str, entries: u64) -> ListInfo {
        ListInfo {
            owner: owner.into(),
            repo: repo.into(),
            category: String::new(),
            enrolled: None,
            dead: false,
            entries,
        }
    }

    fn state(lists: Vec<ListInfo>, events: Vec<Event>) -> State {
        let mut events = events;
        events.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.name.cmp(&b.name)));
        State {
            lists,
            events,
            meta: Meta::default(),
        }
    }

    #[test]
    fn cross_listed_same_day_dedupes_and_badges_on_front() {
        let s = state(
            vec![li("a", "awesome-x", 10), li("b", "awesome-y", 20)],
            vec![
                ev("2026-08-24", Kind::Added, "a/awesome-x", "r/r", "https://github.com/r/r"),
                ev("2026-08-24", Kind::Added, "b/awesome-y", "r/r", "https://github.com/r/r"),
            ],
        );
        let site = build(&s, "2026-08-24".parse().unwrap(), "https://example.com");
        assert_eq!(site.index.days.len(), 1);
        let day = &site.index.days[0];
        assert_eq!(day.items.len(), 1);
        assert!(day.items[0].badge.contains("TWICE CHOSEN TODAY"));
        assert!(day.items[0].badge.contains("AWESOME-Y"));
        // and the feed carries it once
        assert_eq!(site.feed.len(), 1);
    }

    #[test]
    fn front_layouts_hero_then_side_then_text_today_only() {
        let mk = |n: &str| {
            ev(
                "2026-08-24",
                Kind::Added,
                "a/awesome-x",
                n,
                &format!("https://github.com/o/{n}"),
            )
        };
        let mut evs: Vec<Event> = ["a", "b", "c", "d"].iter().map(|n| mk(n)).collect();
        evs.push(ev(
            "2026-08-23",
            Kind::Added,
            "a/awesome-x",
            "z",
            "https://github.com/o/z",
        ));
        let s = state(vec![li("a", "awesome-x", 10)], evs);
        let site = build(&s, "2026-08-24".parse().unwrap(), "https://example.com");
        let layouts: Vec<&str> = site.index.days[0].items.iter().map(|i| i.layout).collect();
        assert_eq!(layouts, vec!["hero", "side", "side", "text"]);
        // yesterday gets no previews
        assert_eq!(site.index.days[1].items[0].layout, "text");
        assert!(site.index.days[1].quiet, "1-item day is quiet");
    }

    #[test]
    fn archive_marks_quiet_days_and_counts() {
        let s = state(
            vec![li("a", "awesome-x", 10)],
            vec![
                ev("2026-08-01", Kind::Added, "a/awesome-x", "p/q", "https://github.com/p/q"),
                ev("2026-08-03", Kind::Removed, "a/awesome-x", "r/s", "https://github.com/r/s"),
            ],
        );
        let site = build(&s, "2026-08-04".parse().unwrap(), "https://example.com");
        assert_eq!(site.archives.len(), 1);
        let (ym, page) = &site.archives[0];
        assert_eq!(ym, "2026-08");
        assert_eq!(page.rows.len(), 4); // days 4..=1, capped at today
        assert!(page.rows[0].quiet); // Aug 4: nothing
        assert!(!page.rows[3].quiet); // Aug 1: one addition
        assert!(page.stats.contains("1 ADDED · 1 REMOVED"));
    }

    #[test]
    fn list_page_stats_and_removals() {
        let s = state(
            vec![li("rust-unofficial", "awesome-rust", 1204)],
            vec![
                ev(
                    "2026-08-24",
                    Kind::Added,
                    "rust-unofficial/awesome-rust",
                    "rerun-io/rerun",
                    "https://github.com/rerun-io/rerun",
                ),
                ev(
                    "2026-08-11",
                    Kind::Removed,
                    "rust-unofficial/awesome-rust",
                    "old/gone",
                    "https://github.com/old/gone",
                ),
            ],
        );
        let site = build(&s, "2026-08-24".parse().unwrap(), "https://example.com");
        assert_eq!(site.lists.len(), 1);
        let (slug, page) = &site.lists[0];
        assert_eq!(slug, "rust-unofficial-awesome-rust");
        assert!(page.stats1.contains("1,204 ENTRIES TODAY"));
        assert!(page.stats1.contains("1 ADDED THIS YEAR"));
        assert_eq!(page.spark.len(), 12);
        assert_eq!(page.removals.len(), 1);
        assert_eq!(page.removals[0].date, "11 AUG");
    }
}
