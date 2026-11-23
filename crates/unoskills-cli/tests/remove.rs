use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_unoskills"))
        .arg("--root")
        .arg(root)
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}

fn install(root: &Path, source: &Path, name: &str) {
    fs::create_dir(source).unwrap();
    fs::write(
        source.join("SKILL.md"),
        format!("---\nname: {name}\n---\nFixture\n"),
    )
    .unwrap();
    let result = cli(root, &["install", source.to_str().unwrap()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn remove_prefers_slug_and_rejects_ambiguous_display_names_in_json() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("library");
    install(&root, &temporary.path().join("reviewer"), "Zebra Reviewer");
    install(&root, &temporary.path().join("different"), "reviewer");

    let refused = cli(&root, &["remove", "reviewer"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        serde_json::from_slice::<Value>(&refused.stderr).unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("--yes")
    );
    assert!(root.join("skills/reviewer/SKILL.md").is_file());

    let result = cli(&root, &["remove", "Reviewer", "--yes"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["ok"],
        true
    );
    assert!(!root.join("skills/reviewer").exists());
    assert!(root.join("skills/different/SKILL.md").is_file());

    install(&root, &temporary.path().join("another"), "reviewer");
    let result = cli(&root, &["remove", "reviewer", "--yes"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let failure: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(failure["ok"], false);
    let error = failure["error"].as_str().unwrap();
    assert!(
        error.contains("AMBIGUOUS_SKILL")
            && error.contains("another")
            && error.contains("different")
    );
    let result = cli(&root, &["list"]);
    let skills: Vec<Value> = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(skills.len(), 2);
    for skill in skills {
        assert!(error.contains(skill["id"].as_str().unwrap()));
        assert!(
            root.join("skills")
                .join(skill["slug"].as_str().unwrap())
                .join("SKILL.md")
                .is_file()
        );
    }
}
