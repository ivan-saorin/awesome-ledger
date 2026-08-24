//! Daily digest into the memory service (SPEC §4.5): one chunk per
//! non-quiet run — "N added, M removed across K lists" + top additions —
//! queued on the volume and flushed queue-until-acked: an unreachable
//! memory service delays delivery, never loses it.
//!
//! Services call each other on the internal network with no credential —
//! auth lives at the edge, for callers outside. Write verb (memory
//! service, docs/skill-memory-recall.md there):
//!   POST {MEM_BASE}/idx/mem0/chunks   {"text", "ref", "tags"}

use crate::model::{Event, Kind};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Internal DNS name of the memory service (override with MEM_BASE).
pub const MEM_BASE: &str = "http://memory:8080";
const TOP_ADDITIONS: usize = 10;

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    /// Provenance ref — unique per day so successive digests never upsert
    /// over each other in the store.
    #[serde(rename = "ref")]
    pub reference: String,
    pub tags: Vec<String>,
}

/// The day's digest from the day's events; None on a quiet day (SPEC §9
/// draft: no chunk when nothing happened).
pub fn compose(events: &[Event], date: NaiveDate, site_url: &str) -> Option<Chunk> {
    let today: Vec<&Event> = events.iter().filter(|e| e.date == date).collect();
    if today.is_empty() {
        return None;
    }
    let added: Vec<&Event> = today
        .iter()
        .copied()
        .filter(|e| e.kind == Kind::Added)
        .collect();
    let removed = today.len() - added.len();
    let lists: std::collections::BTreeSet<&str> =
        today.iter().map(|e| e.list.as_str()).collect();
    let mut text = format!(
        "awesome-ledger {date}: {} added, {} removed across {} list{}.",
        added.len(),
        removed,
        lists.len(),
        if lists.len() == 1 { "" } else { "s" }
    );
    for e in added.iter().take(TOP_ADDITIONS) {
        let blurb = e.description.trim().trim_end_matches('.');
        if blurb.is_empty() {
            text.push_str(&format!("\n- {} ({})", e.name, e.list));
        } else {
            text.push_str(&format!("\n- {} — {blurb} ({})", e.name, e.list));
        }
    }
    if added.len() > TOP_ADDITIONS {
        text.push_str(&format!(
            "\n… plus {} more additions.",
            added.len() - TOP_ADDITIONS
        ));
    }
    text.push_str(&format!("\nFull ledger: {site_url}"));
    Some(Chunk {
        text,
        reference: format!("awesome-ledger/digest/{date}"),
        tags: vec!["changed".into(), "chg0".into(), "awesome-ledger".into()],
    })
}

/// Queue the chunk on the volume. A same-day rerun overwrites the pending
/// file — one digest per day, latest wins.
pub fn enqueue(queue: &Path, chunk: &Chunk, date: NaiveDate) -> Result<PathBuf> {
    fs::create_dir_all(queue).with_context(|| format!("creating {}", queue.display()))?;
    let path = queue.join(format!("digest-{date}.json"));
    fs::write(&path, serde_json::to_vec_pretty(chunk)?)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

#[derive(Debug, Default)]
pub struct Flush {
    pub sent: usize,
    pub left: usize,
    pub notes: Vec<String>,
}

impl std::fmt::Display for Flush {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "digest queue: {} sent, {} pending", self.sent, self.left)?;
        for n in &self.notes {
            write!(f, "\n  {n}")?;
        }
        Ok(())
    }
}

/// Deliver every queued chunk, oldest first. An ack (2xx) deletes the
/// file; anything else keeps it for the next run.
pub fn flush(queue: &Path, base: &str) -> Result<Flush> {
    let mut pending: Vec<PathBuf> = match fs::read_dir(queue) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", queue.display())),
    };
    pending.sort();
    let mut out = Flush::default();
    if pending.is_empty() {
        return Ok(out);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("building http client")?;
    let url = format!("{}/idx/mem0/chunks", base.trim_end_matches('/'));
    for path in pending {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let body = fs::read_to_string(&path)?;
        match client
            .post(&url)
            .header("content-type", "application/json")
            .body(body)
            .send()
        {
            Ok(r) if r.status().is_success() => {
                fs::remove_file(&path).ok();
                out.sent += 1;
            }
            Ok(r) => {
                out.left += 1;
                out.notes.push(format!("{name}: HTTP {} — kept in queue", r.status()));
            }
            Err(e) => {
                out.left += 1;
                out.notes.push(format!("{name}: {e} — kept in queue"));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(d: u32, kind: Kind, name: &str, list: &str, desc: &str) -> Event {
        Event {
            date: NaiveDate::from_ymd_opt(2026, 8, d).unwrap(),
            kind,
            list: list.into(),
            name: name.into(),
            url: format!("https://github.com/x/{name}"),
            description: desc.into(),
            section: Vec::new(),
        }
    }

    #[test]
    fn quiet_day_composes_nothing() {
        let yesterday = [ev(23, Kind::Added, "Old", "a/one", "stale.")];
        let date = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
        assert!(compose(&yesterday, date, "https://site").is_none());
        assert!(compose(&[], date, "https://site").is_none());
    }

    #[test]
    fn digest_counts_top_additions_and_tags() {
        let mut events: Vec<Event> = (0..12)
            .map(|i| ev(24, Kind::Added, &format!("Tool{i:02}"), "a/one", "does things."))
            .collect();
        events.push(ev(24, Kind::Removed, "Gone", "b/two", ""));
        events.push(ev(23, Kind::Added, "NotToday", "c/three", "")); // other day
        let date = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
        let chunk = compose(&events, date, "https://site").unwrap();
        assert!(chunk
            .text
            .starts_with("awesome-ledger 2026-08-24: 12 added, 1 removed across 2 lists."));
        // Only the top 10 additions are named; the trailing period on the
        // blurb is trimmed (the line carries its own punctuation).
        assert_eq!(chunk.text.matches("\n- ").count(), 10);
        assert!(chunk.text.contains("Tool00 — does things (a/one)"));
        assert!(chunk.text.contains("… plus 2 more additions."));
        assert!(!chunk.text.contains("NotToday"));
        assert!(chunk.text.ends_with("Full ledger: https://site"));
        assert_eq!(chunk.tags, ["changed", "chg0", "awesome-ledger"]);
        assert_eq!(chunk.reference, "awesome-ledger/digest/2026-08-24");
    }

    #[test]
    fn queue_survives_unreachable_service_and_rerun_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let queue = tmp.path().join("queue");
        let date = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
        let events = [ev(24, Kind::Added, "Tool", "a/one", "x.")];
        let chunk = compose(&events, date, "https://site").unwrap();
        enqueue(&queue, &chunk, date).unwrap();

        // Unreachable service: nothing sent, nothing lost.
        let f = flush(&queue, "http://127.0.0.1:9").unwrap();
        assert_eq!((f.sent, f.left), (0, 1));
        assert!(!f.notes.is_empty());
        assert!(queue.join("digest-2026-08-24.json").exists());

        // Same-day rerun overwrites — still exactly one pending file.
        enqueue(&queue, &chunk, date).unwrap();
        let files: Vec<_> = std::fs::read_dir(&queue).unwrap().collect();
        assert_eq!(files.len(), 1);

        // Round-trips as the wire shape ("ref", not "reference").
        let raw = std::fs::read_to_string(queue.join("digest-2026-08-24.json")).unwrap();
        assert!(raw.contains("\"ref\""));
        let back: Chunk = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.reference, chunk.reference);
    }
}
