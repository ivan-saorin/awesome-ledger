//! URL normalization (SPEC §2): the canonical key that makes an entry the
//! same entry across renames, reorders and cosmetic link edits.

use url::Url;

const TRACKING: &[&str] = &["fbclid", "gclid", "igshid", "mc_cid", "mc_eid", "ref", "ref_src"];

/// Canonical key for an entry URL: https, lowercased host without www,
/// no trailing slash, no `.git`, no fragment, tracking params dropped,
/// remaining query sorted. None for non-http(s) links (anchors, mailto,
/// relative paths) — those are not entries.
pub fn canonical(raw: &str) -> Option<String> {
    let url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host_lower = url.host_str()?.to_ascii_lowercase();
    let host = host_lower.strip_prefix("www.").unwrap_or(&host_lower);
    let mut path = url.path().trim_end_matches('/').to_string();
    if let Some(stripped) = path.strip_suffix(".git") {
        path = stripped.to_string();
    }
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !k.starts_with("utm_") && !TRACKING.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort();
    let mut key = format!("https://{host}{path}");
    for (i, (k, v)) in pairs.iter().enumerate() {
        key.push(if i == 0 { '?' } else { '&' });
        key.push_str(k);
        if !v.is_empty() {
            key.push('=');
            key.push_str(v);
        }
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::canonical;

    #[test]
    fn folds_scheme_www_slash_git_and_fragment() {
        for raw in [
            "http://www.GitHub.com/Foo/Bar.git",
            "https://github.com/Foo/Bar/",
            "https://github.com/Foo/Bar#readme",
        ] {
            assert_eq!(canonical(raw).as_deref(), Some("https://github.com/Foo/Bar"));
        }
    }

    #[test]
    fn drops_tracking_keeps_real_query_sorted() {
        assert_eq!(
            canonical("https://ex.com/p?utm_source=x&b=2&a=1&ref=hn").as_deref(),
            Some("https://ex.com/p?a=1&b=2")
        );
    }

    #[test]
    fn rejects_non_http() {
        assert_eq!(canonical("#contents"), None);
        assert_eq!(canonical("mailto:x@y.z"), None);
        assert_eq!(canonical("../other.md"), None);
    }
}
