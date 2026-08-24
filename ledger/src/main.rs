mod digest;
mod feed;
mod fetch;
mod model;
mod norm;
mod parse;
mod publish;
mod registry;
mod render;
mod store;
mod update;
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
        "usage:\n  awesome-ledger update --state DIR [--lists FILE] [--no-index] [--enroll] [--limit N] [--date YYYY-MM-DD] [--report FILE]\n  awesome-ledger render --state DIR --out DIR [--date YYYY-MM-DD] [--site-url URL]\n  awesome-ledger publish --site DIR [--remote GIT_URL] [--branch NAME]\n  awesome-ledger digest --state DIR --queue DIR [--date YYYY-MM-DD] [--site-url URL]"
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
        let key = k.trim_start_matches("--").to_string();
        if matches!(key.as_str(), "no-index" | "enroll") {
            opts.insert(key, "true".to_string());
            continue;
        }
        let v = it.next().with_context(|| format!("missing value for {k}"))?;
        opts.insert(key, v.clone());
    }
    let get = |k: &str| -> Option<PathBuf> { opts.get(k).map(PathBuf::from) };

    match cmd {
        "update" => {
            let lists = get("lists").unwrap_or_else(|| PathBuf::from("lists.toml"));
            let state = get("state").context("--state DIR is required")?;
            let date = match opts.get("date") {
                Some(d) => d.parse().context("--date must be YYYY-MM-DD")?,
                None => chrono::Utc::now().date_naive(),
            };
            let limit = match opts.get("limit") {
                Some(n) => Some(n.parse::<usize>().context("--limit must be a number")?),
                None => None,
            };
            let mut src = fetch::Http::new()?;
            let summary = update::run(
                &mut src,
                &update::Options {
                    lists_path: &lists,
                    state_dir: &state,
                    use_index: !opts.contains_key("no-index"),
                    force_enroll: opts.contains_key("enroll"),
                    limit,
                    date,
                },
            )?;
            println!("{summary}");
            if let Some(report) = get("report") {
                std::fs::write(&report, summary.to_markdown(date))
                    .with_context(|| format!("writing {}", report.display()))?;
            }
            Ok(())
        }
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
        "digest" => {
            let state = get("state").context("--state DIR is required")?;
            let queue = get("queue").context("--queue DIR is required")?;
            let date = match opts.get("date") {
                Some(d) => d.parse().context("--date must be YYYY-MM-DD")?,
                None => chrono::Utc::now().date_naive(),
            };
            let site_url = opts
                .get("site-url")
                .map(String::as_str)
                .unwrap_or(DEFAULT_SITE_URL);
            let loaded = model::load(&state)?;
            match digest::compose(&loaded.events, date, site_url) {
                Some(chunk) => {
                    let path = digest::enqueue(&queue, &chunk, date)?;
                    println!("digest queued: {}", path.display());
                }
                None => println!("quiet day — no digest chunk"),
            }
            let bearer = std::env::var("STACK_BEARER").ok();
            let base =
                std::env::var("MEM_BASE").unwrap_or_else(|_| digest::MEM_BASE.to_string());
            let flush = digest::flush(&queue, &base, bearer.as_deref())?;
            println!("{flush}");
            Ok(())
        }
        _ => usage(),
    }
}
