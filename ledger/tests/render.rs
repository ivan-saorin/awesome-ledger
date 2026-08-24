//! Full render smoke over the checked-in fixture state (M2 smoke per PLAN).

use std::fs;
use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_awesome-ledger")
}

#[test]
fn renders_full_site_from_fixture_state() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("site");
    let state = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/state");

    let status = Command::new(bin())
        .args([
            "render",
            "--state",
            state.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--date",
            "2026-08-24",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    // front page
    let index = fs::read_to_string(out.join("index.html")).unwrap();
    assert!(index.contains("ED. № 847 · MON 24 AUG 2026"));
    assert!(index.contains("7 NEW"), "today has 7 unique additions");
    assert!(index.contains("art--hero"), "today's lead gets the hero card");
    assert!(
        index.contains("★ ALSO CURATED IN AWESOME-ROBOTICS — TWICE CHOSEN TODAY"),
        "cross-listed rerun is badged"
    );
    assert_eq!(
        index.matches("rerun-io/rerun</a>").count(),
        1,
        "cross-listed addition appears once on the front page"
    );
    assert!(index.contains("// A QUIET DAY IN THE CANON //"), "2-item day is quiet");
    assert!(index.contains("TRACKED LISTS: 7"), "dead lists not counted");
    assert!(index.contains("style.css"));

    // per-list page
    let rust = fs::read_to_string(
        out.join("list/rust-unofficial-awesome-rust/index.html"),
    )
    .unwrap();
    assert!(rust.contains("FROM THE LEDGER OF"));
    assert!(rust.contains("1,204 ENTRIES TODAY"));
    assert!(rust.contains("TRACKED SINCE MARCH 2019 · MAINTAINED BY RUST-UNOFFICIAL"));
    assert!(rust.contains("// STRUCK FROM THE LEDGER"));
    assert!(rust.contains("carllerche/mio-old"));
    assert!(rust.contains("★ ALSO IN AWESOME-POSTGRES"), "limbo cross-badge");
    assert!(rust.contains("class=\"spark\""));

    // dead list gets no page
    assert!(!out.join("list/dead-example-awesome-flash").exists());

    // archives: July and August, linked to each other
    let aug = fs::read_to_string(out.join("archive/2026-08/index.html")).unwrap();
    assert!(aug.contains("August 2026"));
    assert!(aug.contains("+4 MORE"), "12 additions on Aug 21, 8 shown");
    assert!(aug.contains("★2"), "cross-listed day gets a star count");
    assert!(aug.contains("// A QUIET DAY IN THE CANON — NO NEW ENTRIES"));
    assert!(aug.contains("../2026-07/"));
    let jul = fs::read_to_string(out.join("archive/2026-07/index.html")).unwrap();
    assert!(jul.contains("July 2026"));
    assert!(jul.contains("../2026-08/"));

    // feeds
    let feed = fs::read_to_string(out.join("feed.xml")).unwrap();
    assert!(feed.starts_with("<?xml"));
    assert_eq!(
        feed.matches("<link>https://github.com/rerun-io/rerun</link>").count(),
        1,
        "cross-listed addition is one feed item"
    );
    let list_feed =
        fs::read_to_string(out.join("list/rust-unofficial-awesome-rust/feed.xml")).unwrap();
    assert!(list_feed.contains("awesome-rust"));

    // pages hygiene
    assert!(out.join(".nojekyll").exists());
}
