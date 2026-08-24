//! gh-pages publish: force-push the rendered site as a fresh single-commit
//! branch. The branch is disposable by design (SPEC §1) — history lives in
//! state and job branches, never on gh-pages.
//!
//! Auth: GH_DEPLOY_KEY (private key content) or GH_DEPLOY_KEY_FILE (path).

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn publish(site: &Path, remote: &str, branch: &str) -> Result<()> {
    if !site.join("index.html").exists() {
        bail!("refusing to publish: {}/index.html missing", site.display());
    }

    let key_file = match env::var("GH_DEPLOY_KEY_FILE") {
        Ok(p) => p,
        Err(_) => {
            let key = env::var("GH_DEPLOY_KEY")
                .context("neither GH_DEPLOY_KEY nor GH_DEPLOY_KEY_FILE set")?;
            let path = env::temp_dir().join("ledger_deploy_key");
            fs::write(&path, ensure_trailing_newline(&key))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            }
            path.to_string_lossy().into_owned()
        }
    };
    let ssh_cmd = format!(
        "ssh -i {key_file} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
    );

    // Fresh throwaway repo inside the rendered site dir.
    let git_dir = site.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir)?;
    }
    git(site, &ssh_cmd, &["init", "-q"])?;
    git(site, &ssh_cmd, &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")])?;
    git(site, &ssh_cmd, &["config", "user.name", "awesome-ledger"])?;
    git(site, &ssh_cmd, &["config", "user.email", "ledger@016180.xyz"])?;
    git(site, &ssh_cmd, &["add", "-A"])?;
    git(
        site,
        &ssh_cmd,
        &[
            "commit",
            "-q",
            "-m",
            &format!("publish {}", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")),
        ],
    )?;
    git(
        site,
        &ssh_cmd,
        &["push", "--force", "-q", remote, &format!("HEAD:refs/heads/{branch}")],
    )?;
    fs::remove_dir_all(&git_dir).ok();
    println!("published {} -> {remote} ({branch})", site.display());
    Ok(())
}

fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_string()
    } else {
        format!("{s}\n")
    }
}

fn git(cwd: &Path, ssh_cmd: &str, args: &[&str]) -> Result<()> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_SSH_COMMAND", ssh_cmd)
        .output()
        .context("spawning git")?;
    if !out.status.success() {
        bail!(
            "git {:?} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}
