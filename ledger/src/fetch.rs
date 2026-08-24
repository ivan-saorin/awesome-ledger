//! Conditional README fetcher (SPEC §4.1): raw.githubusercontent.com at
//! HEAD, etag-conditional, retries with backoff, politeness delay. The
//! `Source` trait keeps the update flow testable offline.

use anyhow::{Context, Result};
use std::thread::sleep;
use std::time::Duration;

/// Outcome of asking for one list's README.
pub enum Doc {
    /// 304 — entry set unchanged, nothing to parse.
    Unchanged,
    /// Fresh body; `path` is the README path that answered (cache it).
    New {
        body: String,
        etag: Option<String>,
        path: String,
    },
    /// Every candidate path 404'd — the repo (or its README) is gone.
    Gone,
}

pub trait Source {
    fn readme(
        &mut self,
        owner: &str,
        repo: &str,
        path_hint: Option<&str>,
        etag: Option<&str>,
    ) -> Result<Doc>;
}

const CANDIDATES: &[&str] = &["README.md", "readme.md", "Readme.md", "README.markdown"];
const RETRY_BACKOFF_SECS: &[u64] = &[1, 4];

pub struct Http {
    client: reqwest::blocking::Client,
    /// Politeness delay before every request (SPEC: ~1 s).
    pub delay: Duration,
}

impl Http {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .user_agent("awesome-ledger (+https://github.com/ivan-saorin/awesome-ledger)")
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .build()
                .context("building http client")?,
            delay: Duration::from_secs(1),
        })
    }

    /// One URL, conditional, with retries on transport errors / 5xx.
    /// Ok(None) = 404 for this path.
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Option<Doc>> {
        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 0..=RETRY_BACKOFF_SECS.len() {
            if attempt > 0 {
                sleep(Duration::from_secs(RETRY_BACKOFF_SECS[attempt - 1]));
            }
            sleep(self.delay);
            let mut req = self.client.get(url);
            if let Some(tag) = etag {
                req = req.header(reqwest::header::IF_NONE_MATCH, tag);
            }
            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status == reqwest::StatusCode::NOT_MODIFIED {
                        return Ok(Some(Doc::Unchanged));
                    }
                    if status == reqwest::StatusCode::NOT_FOUND {
                        return Ok(None);
                    }
                    if status.is_success() {
                        let new_etag = resp
                            .headers()
                            .get(reqwest::header::ETAG)
                            .and_then(|v| v.to_str().ok())
                            .map(String::from);
                        let body = resp.text().context("reading body")?;
                        return Ok(Some(Doc::New {
                            body,
                            etag: new_etag,
                            path: String::new(), // filled by caller
                        }));
                    }
                    last_err = Some(anyhow::anyhow!("{url}: HTTP {status}"));
                    if status.is_client_error() {
                        break; // 4xx other than 404 won't heal by retrying
                    }
                }
                Err(e) => last_err = Some(anyhow::Error::new(e).context(url.to_string())),
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("{url}: unreachable")))
    }
}

impl Source for Http {
    fn readme(
        &mut self,
        owner: &str,
        repo: &str,
        path_hint: Option<&str>,
        etag: Option<&str>,
    ) -> Result<Doc> {
        let mut paths: Vec<&str> = Vec::new();
        if let Some(h) = path_hint {
            paths.push(h);
        }
        paths.extend(CANDIDATES.iter().filter(|c| Some(**c) != path_hint));

        for path in paths {
            let url = format!("https://raw.githubusercontent.com/{owner}/{repo}/HEAD/{path}");
            match self.get(&url, etag)? {
                Some(Doc::New { body, etag, .. }) => {
                    return Ok(Doc::New {
                        body,
                        etag,
                        path: path.to_string(),
                    })
                }
                Some(doc) => return Ok(doc),
                None => continue, // 404 → next candidate
            }
        }
        Ok(Doc::Gone)
    }
}
