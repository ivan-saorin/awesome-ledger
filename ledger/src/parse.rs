//! Markdown → entry set (SPEC §4.2): list items with a link become
//! entries; the heading trail is the section; text after the link is the
//! description. Permissive by design — anything that doesn't look like an
//! entry is silently ignored, parse failures never abort a run.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEntry {
    pub name: String,
    pub url: String,
    pub description: String,
    /// Heading trail (h2 and deeper — the h1 is the page title).
    pub section: Vec<String>,
}

#[derive(Default)]
struct ItemCapture {
    url: Option<String>,
    in_link: bool,
    name: String,
    description: String,
}

impl ItemCapture {
    fn push_text(&mut self, t: &str) {
        if self.in_link {
            self.name.push_str(t);
        } else if self.url.is_some() {
            self.description.push_str(t);
        }
        // Text before any link (rare "Name — desc [link]" style) is dropped.
    }

    fn finish(self, trail: &[(u32, String)]) -> Option<ParsedEntry> {
        let url = self.url?;
        let name = squeeze(&self.name);
        let description = clean_description(&self.description);
        let name = if name.is_empty() { url.clone() } else { name };
        Some(ParsedEntry {
            name,
            url,
            description,
            section: trail
                .iter()
                .filter(|(l, _)| *l >= 2)
                .map(|(_, t)| t.clone())
                .collect(),
        })
    }
}

/// Collapse internal whitespace runs and trim.
fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip the leading separator awesome lists put between link and blurb.
fn clean_description(s: &str) -> String {
    // '-', en dash, em dash, ':', middle dot
    const SEPS: [char; 5] = ['-', '\u{2013}', '\u{2014}', ':', '\u{00b7}'];
    let s = squeeze(s);
    s.trim_start_matches(|c: char| SEPS.contains(&c) || c.is_whitespace())
        .trim()
        .to_string()
}

pub fn entries(md: &str) -> Vec<ParsedEntry> {
    let parser = Parser::new_ext(md, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);
    let mut trail: Vec<(u32, String)> = Vec::new();
    let mut heading: Option<(u32, String)> = None;
    let mut items: Vec<ItemCapture> = Vec::new();
    let mut in_image = 0usize;
    let mut out = Vec::new();

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((level as u32, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((lvl, text)) = heading.take() {
                    while trail.last().is_some_and(|(l, _)| *l >= lvl) {
                        trail.pop();
                    }
                    let text = squeeze(&text);
                    if !text.is_empty() {
                        trail.push((lvl, text));
                    }
                }
            }
            Event::Start(Tag::Item) => items.push(ItemCapture::default()),
            Event::End(TagEnd::Item) => {
                if let Some(entry) = items.pop().and_then(|it| it.finish(&trail)) {
                    out.push(entry);
                }
            }
            Event::Start(Tag::Image { .. }) => in_image += 1,
            Event::End(TagEnd::Image) => in_image = in_image.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => {
                if in_image == 0 {
                    if let Some(it) = items.last_mut() {
                        if it.url.is_none() {
                            it.url = Some(dest_url.to_string());
                            it.in_link = true;
                        }
                    }
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(it) = items.last_mut() {
                    if it.in_link && it.name.trim().is_empty() {
                        // Link with no text content — a badge (link-wrapped
                        // image). Not the entry link; keep waiting.
                        it.url = None;
                    }
                    it.in_link = false;
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, h)) = heading.as_mut() {
                    h.push_str(&t);
                } else if in_image == 0 {
                    if let Some(it) = items.last_mut() {
                        it.push_text(&t);
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, h)) = heading.as_mut() {
                    h.push(' ');
                } else if let Some(it) = items.last_mut() {
                    it.push_text(" ");
                }
            }
            _ => {}
        }
    }
    out
}

/// A GitHub list linked from the sindresorhus index: plain
/// github.com/{owner}/{repo} links only (fragment allowed, deeper paths
/// are not list READMEs); category = nearest section heading.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexList {
    pub owner: String,
    pub repo: String,
    pub category: String,
}

pub fn index_lists(md: &str) -> Vec<IndexList> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for e in entries(md) {
        let Some((owner, repo)) = github_repo(&e.url) else {
            continue;
        };
        if !seen.insert(format!("{}/{}", owner.to_lowercase(), repo.to_lowercase())) {
            continue;
        }
        let category = e
            .section
            .last()
            .cloned()
            .unwrap_or_else(|| "Uncategorized".to_string());
        out.push(IndexList { owner, repo, category });
    }
    out
}

/// (owner, repo) for a plain github.com repo URL, else None.
pub fn github_repo(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("https://www.github.com/"))?;
    let mut parts = rest.split(['#', '?']).next()?.split('/').filter(|s| !s.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?.trim_end_matches(".git");
    if parts.next().is_some() || owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Awesome Things\n\n\
[![Awesome](https://awesome.re/badge.svg)](https://awesome.re)\n\n\
## Tools\n\n\
### Editors\n\n\
- [Alpha](https://github.com/a/alpha) - The first one.\n\
- [![badge](https://img.shields.io/x.svg)](https://ci.example.com) [Beta](https://github.com/b/beta) — Second, with a leading badge.\n\
- Plain text bullet without a link.\n\
- [Gamma](https://github.com/c/gamma)\n\
  - [Delta](https://github.com/d/delta) - Nested under Gamma.\n\n\
## Reading\n\n\
- [Site](https://example.com/article?utm_source=x) : an article.\n";

    #[test]
    fn extracts_entries_with_sections_and_descriptions() {
        let es = entries(SAMPLE);
        let names: Vec<&str> = es.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Beta", "Gamma", "Delta", "Site"]);
        let alpha = &es[0];
        assert_eq!(alpha.url, "https://github.com/a/alpha");
        assert_eq!(alpha.description, "The first one.");
        assert_eq!(alpha.section, ["Tools", "Editors"]);
        // Badge image link is skipped; the entry link wins.
        assert_eq!(es[1].url, "https://github.com/b/beta");
        assert_eq!(es[1].description, "Second, with a leading badge.");
        // Nested item is its own entry, same trail.
        assert_eq!(es[2].name, "Gamma");
        assert_eq!(es[2].description, "");
        assert_eq!(es[3].name, "Delta");
        assert_eq!(es[3].description, "Nested under Gamma.");
        assert_eq!(es[4].section, ["Reading"]);
        assert_eq!(es[4].description, "an article.");
    }

    #[test]
    fn index_scan_keeps_only_plain_github_repos() {
        let md = "\
# Awesome\n\n## Contents\n\n- [Platforms](#platforms)\n\n## Platforms\n\n\
- [Node.js](https://github.com/sindresorhus/awesome-nodejs#readme) - JS.\n\
- [Deep](https://github.com/o/r/tree/main/docs) - subfolder, not a list repo.\n\
- [Elsewhere](https://gitlab.com/o/r) - not github.\n\
- [Node.js](https://github.com/sindresorhus/awesome-nodejs) - duplicate.\n";
        let idx = index_lists(md);
        assert_eq!(
            idx,
            [IndexList {
                owner: "sindresorhus".into(),
                repo: "awesome-nodejs".into(),
                category: "Platforms".into()
            }]
        );
    }

    #[test]
    fn github_repo_shapes() {
        assert_eq!(
            github_repo("https://github.com/a/b#readme"),
            Some(("a".into(), "b".into()))
        );
        assert_eq!(github_repo("https://github.com/a/b.git"), Some(("a".into(), "b".into())));
        assert_eq!(github_repo("https://github.com/a"), None);
        assert_eq!(github_repo("https://github.com/a/b/c"), None);
        assert_eq!(github_repo("https://example.com/a/b"), None);
    }
}
