mod feed;
mod model;
mod publish;
mod render;
mod view;

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

const DEFAULT_SITE_URL: &str = "https://ivan-saorin.github.io/awesome-ledger";
const DEFAULT_REPO_URL: &str = "https://github.com/ivan-saorin/awesome-ledger";
const DEFAULT_REMOTE: &str = "git@github.com:ivan-saorin/awesome-ledger.git";

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  awesome-ledger render --state DIR --out DIR [--date YYYY-MM-DD] [--site-url URL]\n  awesome-ledger publish --site DIR [--remote GIT_URL] [--branch NAME]"
    );
    std::process::exit(2);
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    let cmd = match it.next() {
        Some(c) => c.as_str(),
        None => usage(),
    };
    let mut opts = std::collections::HashMap::<String, String>::new();
    while let Some(k) = it.next() {
        if !k.starts_with("--") {
            bail!("unexpected argument {k}");
        }
        let v = it.next().with_context(|| format!("missing value for {k}"))?;
        opts.insert(k.trim_start_matches("--").to_string(), v.clone());
    }
    let get = |k: &str| -> Option<PathBuf> { opts.get(k).map(PathBuf::from) };

    match cmd {
        "render" => {
            let state_dir = get("state").context("--state DIR is required")?;
            let out_dir = get("out").context("--out DIR is required")?;
            let date = match opts.get("date") {
                Some(d) => Some(d.parse().context("--date must be YYYY-MM-DD")?),
                None => None,
            };
            let site_url = opts
                .get("site-url")
                .map(String::as_str)
                .unwrap_or(DEFAULT_SITE_URL);
            render::render(&render::Options {
                state_dir: &state_dir,
                out_dir: &out_dir,
                date,
                site_url,
                repo_url: DEFAULT_REPO_URL,
            })
        }
        "publish" => {
            let site = get("site").context("--site DIR is required")?;
            let remote = opts
                .get("remote")
                .map(String::as_str)
                .unwrap_or(DEFAULT_REMOTE);
            let branch = opts.get("branch").map(String::as_str).unwrap_or("gh-pages");
            publish::publish(&site, remote, branch)
        }
        _ => usage(),
    }
}
