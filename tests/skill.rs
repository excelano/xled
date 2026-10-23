// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! The installed skill's stamp and `-V` have to name the same revision, or the
//! stamp would call a current skill stale.

use std::path::PathBuf;
use std::process::Command;

fn run(args: &[&str], home: &PathBuf) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_xled"))
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{args:?} failed");
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_stamp_names_the_revision_version_prints() {
    let home = std::env::temp_dir().join(format!("xled-skill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();

    let version = run(&["-V"], &home);
    let revision = version
        .trim()
        .strip_suffix(')')
        .and_then(|v| v.rsplit_once("(skill "))
        .map(|(_, r)| r.to_string())
        .unwrap_or_else(|| panic!("no skill revision in {version:?}"));
    assert_eq!(revision.len(), 8, "{version:?}");

    run(&["--install-skill"], &home);
    let skill = std::fs::read_to_string(home.join(".claude/skills/xled/SKILL.md")).unwrap();
    assert!(
        skill.contains(&format!("> This skill is revision {revision}.")),
        "the stamp does not match {version:?}"
    );
    assert!(run(&["--install-skill"], &home).contains("already current"));

    std::fs::remove_dir_all(&home).unwrap();
}
