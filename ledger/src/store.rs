//! State-dir writer half of the contract in model.rs, plus the per-list
//! snapshots (entry set + fetch cache) the diff runs against. Snapshots
//! live under state/lists/ and are internal to the update job — the
//! renderer only reads lists.json / events.jsonl / meta.json.

use crate::model::{Event, ListInfo, Meta};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredEntry {
    /// Canonical URL (norm::canonical) — the diff identity.
    pub key: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub section: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub readme_path: Option<String>,
    /// Consecutive 404s; 2 marks the list dead (SPEC §4.1).
    #[serde(default)]
    pub misses: u32,
    #[serde(default)]
    pub entries: Vec<StoredEntry>,
}

fn snapshot_path(state: &Path, owner: &str, repo: &str) -> PathBuf {
    state
        .join("lists")
        .join(format!("{}--{}.json", owner.to_lowercase(), repo.to_lowercase()))
}

pub fn load_snapshot(state: &Path, owner: &str, repo: &str) -> Result<Option<Snapshot>> {
    let path = snapshot_path(state, owner, repo);
    match fs::read_to_string(&path) {
        Ok(s) => Ok(Some(
            serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display()))?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn save_snapshot(state: &Path, owner: &str, repo: &str, snap: &Snapshot) -> Result<()> {
    let path = snapshot_path(state, owner, repo);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(snap)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn load_lists(state: &Path) -> Result<Vec<ListInfo>> {
    match fs::read_to_string(state.join("lists.json")) {
        Ok(s) => serde_json::from_str(&s).context("parsing lists.json"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e).context("reading lists.json"),
    }
}

pub fn save_lists(state: &Path, lists: &[ListInfo]) -> Result<()> {
    fs::create_dir_all(state)?;
    fs::write(state.join("lists.json"), serde_json::to_vec_pretty(lists)?)
        .context("writing lists.json")
}

/// Appends events; always leaves events.jsonl existing (the renderer
/// requires it even after a zero-event seed run).
pub fn append_events(state: &Path, events: &[Event]) -> Result<()> {
    fs::create_dir_all(state)?;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join("events.jsonl"))
        .context("opening events.jsonl")?;
    for ev in events {
        serde_json::to_writer(&mut f, ev)?;
        f.write_all(b"\n")?;
    }
    Ok(())
}

pub fn load_meta(state: &Path) -> Result<Meta> {
    match fs::read_to_string(state.join("meta.json")) {
        Ok(s) => serde_json::from_str(&s).context("parsing meta.json"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Meta::default()),
        Err(e) => Err(e).context("reading meta.json"),
    }
}

pub fn save_meta(state: &Path, meta: &Meta) -> Result<()> {
    fs::create_dir_all(state)?;
    fs::write(state.join("meta.json"), serde_json::to_vec_pretty(meta)?)
        .context("writing meta.json")
}
