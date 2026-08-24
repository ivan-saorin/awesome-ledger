//! State store contract between the update job (M1) and the renderer (M2).
//!
//! Layout of the state directory (volume `awesome-ledger-data/state`):
//!   lists.json    — Vec<ListInfo>: every enrolled list, dead ones included
//!   events.jsonl  — one Event per line, append-only, the full history
//!   meta.json     — optional Meta (edition counter, first run date)

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ListInfo {
    pub owner: String,
    pub repo: String,
    /// Index section from sindresorhus/awesome — not rendered until the
    /// per-category views (SPEC §8); part of the M1 contract regardless.
    #[allow(dead_code)]
    #[serde(default)]
    pub category: String,
    /// First seen by the ledger (seed date).
    #[serde(default)]
    pub enrolled: Option<NaiveDate>,
    #[serde(default)]
    pub dead: bool,
    /// Current entry count from the last successful parse.
    #[serde(default)]
    pub entries: u64,
}

impl ListInfo {
    pub fn full(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
    /// URL slug per SPEC §5: /list/<owner>-<repo>/
    pub fn slug(&self) -> String {
        slug_of(&self.owner, &self.repo)
    }
}

pub fn slug_of(owner: &str, repo: &str) -> String {
    format!("{}-{}", owner, repo).to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Added,
    Removed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub date: NaiveDate,
    pub kind: Kind,
    /// Source list as "owner/repo".
    pub list: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    /// Heading trail inside the source list — carried in state for future
    /// views; the M2 pages don't show it.
    #[allow(dead_code)]
    #[serde(default)]
    pub section: Vec<String>,
}

impl Event {
    /// Display name of the source list — the repo half, e.g. "awesome-rust".
    pub fn list_repo(&self) -> &str {
        self.list.rsplit('/').next().unwrap_or(&self.list)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub edition: Option<u64>,
}

pub struct State {
    pub lists: Vec<ListInfo>,
    /// Sorted by date ascending (stable on name for determinism).
    pub events: Vec<Event>,
    pub meta: Meta,
}

pub fn load(dir: &Path) -> Result<State> {
    let lists: Vec<ListInfo> = serde_json::from_str(
        &fs::read_to_string(dir.join("lists.json")).context("reading lists.json")?,
    )
    .context("parsing lists.json")?;

    let raw = fs::read_to_string(dir.join("events.jsonl")).context("reading events.jsonl")?;
    let mut events: Vec<Event> = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ev: Event = serde_json::from_str(line)
            .with_context(|| format!("parsing events.jsonl line {}", i + 1))?;
        events.push(ev);
    }
    events.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.name.cmp(&b.name)));

    let meta: Meta = match fs::read_to_string(dir.join("meta.json")) {
        Ok(s) => serde_json::from_str(&s).context("parsing meta.json")?,
        Err(_) => Meta::default(),
    };

    Ok(State { lists, events, meta })
}

/// GitHub OpenGraph preview card for a repo URL (public, no API key).
/// None for anything that is not a plain github.com/{owner}/{repo} link.
pub fn github_og(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let mut parts = rest.split(['/', '?', '#']);
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let repo = parts.next().filter(|s| !s.is_empty())?;
    let repo = repo.trim_end_matches(".git");
    Some(format!("https://opengraph.githubassets.com/1/{owner}/{repo}"))
}

/// 214309 -> "214,309"
pub fn commafy(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn og_url_for_plain_repo() {
        assert_eq!(
            github_og("https://github.com/rerun-io/rerun").as_deref(),
            Some("https://opengraph.githubassets.com/1/rerun-io/rerun")
        );
    }

    #[test]
    fn og_url_strips_git_suffix_and_deep_paths() {
        assert_eq!(
            github_og("https://github.com/oxc-project/oxc.git").as_deref(),
            Some("https://opengraph.githubassets.com/1/oxc-project/oxc")
        );
        assert_eq!(
            github_og("https://github.com/foo/bar/tree/main/docs").as_deref(),
            Some("https://opengraph.githubassets.com/1/foo/bar")
        );
    }

    #[test]
    fn og_url_rejects_non_repo_links() {
        assert_eq!(github_og("https://example.com/x"), None);
        assert_eq!(github_og("https://github.com/onlyowner"), None);
    }

    #[test]
    fn commafy_groups_thousands() {
        assert_eq!(commafy(7), "7");
        assert_eq!(commafy(1204), "1,204");
        assert_eq!(commafy(214309), "214,309");
    }
}
