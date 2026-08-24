//! lists.toml â€” the enrollment registry on master (SPEC Â§3): extra pins
//! beyond the sindresorhus index, and a blocklist for noisy lists.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Registry {
    pub extra: Vec<ExtraList>,
    pub blocklist: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraList {
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub category: Option<String>,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn is_blocked(&self, owner: &str, repo: &str) -> bool {
        let full = format!("{owner}/{repo}");
        self.blocklist.iter().any(|b| b.eq_ignore_ascii_case(&full))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry() {
        let reg: Registry = toml::from_str(
            r#"
            blocklist = ["noisy/awesome-noise"]
            [[extra]]
            owner = "someone"
            repo = "awesome-thing"
            category = "Miscellaneous"
            "#,
        )
        .unwrap();
        assert_eq!(reg.extra.len(), 1);
        assert_eq!(reg.extra[0].category.as_deref(), Some("Miscellaneous"));
        assert!(reg.is_blocked("Noisy", "awesome-noise"));
        assert!(!reg.is_blocked("someone", "awesome-thing"));
    }

    #[test]
    fn empty_file_is_empty_registry() {
        let reg: Registry = toml::from_str("").unwrap();
        assert!(reg.extra.is_empty() && reg.blocklist.is_empty());
    }
}
