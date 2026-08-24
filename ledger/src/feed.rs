//! RSS 2.0 feed — hand-assembled; entries are already newest-first.

use crate::view::FeedEntry;
use chrono::NaiveDate;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn rfc2822(d: NaiveDate) -> String {
    d.and_hms_opt(6, 0, 0).unwrap().and_utc().to_rfc2822()
}

/// `title` — channel title; `page_url` — the HTML page this feed mirrors.
pub fn render(title: &str, page_url: &str, entries: &[FeedEntry]) -> String {
    let mut out = String::with_capacity(1024 + entries.len() * 512);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<rss version=\"2.0\">\n<channel>\n");
    out.push_str(&format!("<title>{}</title>\n", xml_escape(title)));
    out.push_str(&format!("<link>{}</link>\n", xml_escape(page_url)));
    out.push_str(
        "<description>New entries across the GitHub awesome ecosystem, diffed nightly.</description>\n",
    );
    if let Some(first) = entries.first() {
        out.push_str(&format!(
            "<lastBuildDate>{}</lastBuildDate>\n",
            rfc2822(first.date)
        ));
    }
    for e in entries {
        out.push_str("<item>\n");
        out.push_str(&format!(
            "<title>{} — via {}</title>\n",
            xml_escape(&e.name),
            xml_escape(&e.via)
        ));
        out.push_str(&format!("<link>{}</link>\n", xml_escape(&e.url)));
        out.push_str(&format!(
            "<guid isPermaLink=\"false\">{}#{}</guid>\n",
            xml_escape(&e.url),
            e.date
        ));
        out.push_str(&format!("<pubDate>{}</pubDate>\n", rfc2822(e.date)));
        if !e.desc.is_empty() {
            out.push_str(&format!(
                "<description>{}</description>\n",
                xml_escape(&e.desc)
            ));
        }
        out.push_str("</item>\n");
    }
    out.push_str("</channel>\n</rss>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_stamps() {
        let entries = vec![FeedEntry {
            name: "a<b>/c&d".into(),
            url: "https://github.com/a/c".into(),
            via: "awesome-x".into(),
            desc: "big & small".into(),
            date: "2026-08-24".parse().unwrap(),
        }];
        let xml = render("The Awesome Ledger", "https://site/", &entries);
        assert!(xml.contains("a&lt;b&gt;/c&amp;d"));
        assert!(xml.contains("big &amp; small"));
        assert!(xml.contains("<pubDate>Mon, 24 Aug 2026"));
        assert!(!xml.contains("<b>"));
    }
}
