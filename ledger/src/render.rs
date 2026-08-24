//! Site assembly: state dir in, static site dir out.

use crate::{feed, model, view};
use anyhow::{Context, Result};
use askama::Template;
use chrono::NaiveDate;
use std::fs;
use std::path::Path;

pub struct Options<'a> {
    pub state_dir: &'a Path,
    pub out_dir: &'a Path,
    /// Render date ("today" on the front page). Defaults to UTC today.
    pub date: Option<NaiveDate>,
    /// Public base URL of the site, used in feeds.
    pub site_url: &'a str,
    /// Repo URL for the ABOUT link.
    pub repo_url: &'a str,
}

pub fn render(opts: &Options) -> Result<()> {
    let state = model::load(opts.state_dir)?;
    let today = opts
        .date
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
    let site = view::build(&state, today, opts.repo_url);
    let base = opts.site_url.trim_end_matches('/');

    let out = opts.out_dir;
    fs::create_dir_all(out)?;
    fs::write(out.join(".nojekyll"), "")?;
    fs::write(out.join("style.css"), include_str!("../assets/style.css"))?;

    write_page(&out.join("index.html"), &site.index.render()?)?;
    fs::write(
        out.join("feed.xml"),
        feed::render("The Awesome Ledger", &format!("{base}/"), &site.feed),
    )?;

    for (slug, page) in &site.lists {
        let dir = out.join("list").join(slug);
        fs::create_dir_all(&dir)?;
        write_page(&dir.join("index.html"), &page.render()?)?;
    }
    for (slug, name, entries) in &site.list_feeds {
        let dir = out.join("list").join(slug);
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("feed.xml"),
            feed::render(
                &format!("The Awesome Ledger — {name}"),
                &format!("{base}/list/{slug}/"),
                entries,
            ),
        )?;
    }
    for (ym, page) in &site.archives {
        let dir = out.join("archive").join(ym);
        fs::create_dir_all(&dir)?;
        write_page(&dir.join("index.html"), &page.render()?)?;
    }

    println!(
        "rendered: {} list pages, {} archive months, {} feed items -> {}",
        site.lists.len(),
        site.archives.len(),
        site.feed.len(),
        out.display()
    );
    Ok(())
}

fn write_page(path: &Path, html: &str) -> Result<()> {
    fs::write(path, html).with_context(|| format!("writing {}", path.display()))
}
