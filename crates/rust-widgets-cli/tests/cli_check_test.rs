// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Behavioural tests for the commands stage P0 implements (BLUE31 §2.9).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rwui(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rwui")).args(args).output().expect("rwui must run")
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A scratch directory unique to this test, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("rwui-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&path, contents).expect("write");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const PROJECT: &str = r##"{
  "window": {
    "id": "root",
    "title": "Demo",
    "width": 640,
    "height": 480,
    "layout": {
      "type": "vbox",
      "children": [
        { "label": { "id": "heading", "text": "Hello" } }
      ]
    }
  }
}"##;

/// The committed example bundle reports no **errors** and generates byte-identically.
#[test]
fn the_example_bundle_checks_and_does_not_drift() {
    let bundle = Path::new("../../examples/generated_project");
    let checked = rwui(&["check", bundle.to_str().expect("utf8")]);
    assert_eq!(code(&checked), 0, "check failed: {}", stderr(&checked));
    assert!(!stderr(&checked).contains("error["), "no errors expected: {}", stderr(&checked));

    let drift = rwui(&["json2rust", bundle.to_str().expect("utf8"), "--check"]);
    assert_eq!(code(&drift), 0, "drift: {}", stderr(&drift));
}

/// A document that is not JSON is RWCLI-001 and carries its position.
#[test]
fn a_syntax_error_is_line_addressed() {
    let scratch = Scratch::new("syntax");
    let path = scratch.write("ui.json", "{\n  \"window\": {\n");
    let output = rwui(&["check", path.to_str().expect("utf8")]);
    assert_eq!(code(&output), 1);
    assert!(stderr(&output).contains("RWCLI-001"), "{}", stderr(&output));
    assert!(stderr(&output).contains(":3:"), "{}", stderr(&output));
}

/// A document with more than one root key is RWCLI-002, not a syntax error.
#[test]
fn a_contract_error_is_rwcli_002() {
    let scratch = Scratch::new("contract");
    let path = scratch.write("ui.json", r#"{ "window": {}, "button": {} }"#);
    let output = rwui(&["check", path.to_str().expect("utf8")]);
    assert_eq!(code(&output), 1);
    assert!(stderr(&output).contains("RWCLI-002"), "{}", stderr(&output));
}

/// `--json` writes one JSON object per diagnostic and nothing else.
#[test]
fn json_mode_emits_ndjson() {
    let scratch = Scratch::new("ndjson");
    let path = scratch.write("ui.json", "{ \"window\": {}, \"button\": {} }");
    let output = rwui(&["check", path.to_str().expect("utf8"), "--json"]);
    assert_eq!(code(&output), 1);
    for line in stdout(&output).lines() {
        let value: serde_json::Value = serde_json::from_str(line).expect("every line is JSON");
        assert!(value.get("code").is_some());
        assert!(value.get("severity").is_some());
    }
}

/// A stylesheet the stage cannot apply is reported, and `--strict` makes it an error.
#[test]
fn an_unapplied_stylesheet_is_a_gap_and_strict_promotes_it() {
    let scratch = Scratch::new("css");
    scratch.write("ui.json", PROJECT);
    scratch.write("theme.css", "Button { background-color: #ff0000; }");
    let dir = scratch.path().to_str().expect("utf8");

    let relaxed = rwui(&["check", dir]);
    assert_eq!(code(&relaxed), 0, "{}", stderr(&relaxed));
    assert!(stderr(&relaxed).contains("RWCLI-006"), "{}", stderr(&relaxed));

    let strict = rwui(&["check", dir, "--strict"]);
    assert_eq!(code(&strict), 1, "{}", stderr(&strict));
}

/// A committed file that does not match is RWCLI-011.
#[test]
fn a_stale_artifact_is_drift() {
    let scratch = Scratch::new("drift");
    scratch.write("ui.json", PROJECT);
    let dir = scratch.path().to_str().expect("utf8");
    assert_eq!(code(&rwui(&["json2rust", dir])), 0);

    let changed = PROJECT.replace("Hello", "Changed");
    scratch.write("ui.json", &changed);
    let output = rwui(&["json2rust", dir, "--check"]);
    assert_eq!(code(&output), 1);
    assert!(stderr(&output).contains("RWCLI-011"), "{}", stderr(&output));
}

/// After a write, a second `--check` is clean: the writer and the checker agree byte for byte.
#[test]
fn writing_then_checking_is_clean() {
    let scratch = Scratch::new("roundtrip");
    scratch.write("ui.json", PROJECT);
    let dir = scratch.path().to_str().expect("utf8");
    assert_eq!(code(&rwui(&["json2rust", dir])), 0);
    let again = rwui(&["json2rust", dir, "--check"]);
    assert_eq!(code(&again), 0, "{}", stderr(&again));
}

/// `fmt` reorders keys and preserves every value.
#[test]
fn fmt_is_stable_and_semantics_preserving() {
    let scratch = Scratch::new("fmt");
    let path = scratch.write("ui.json", r#"{"window":{"title":"T","width":10}}"#);
    let output = rwui(&["fmt", path.to_str().expect("utf8")]);
    assert_eq!(code(&output), 0);
    let pretty = stdout(&output);
    let value: serde_json::Value = serde_json::from_str(&pretty).expect("still JSON");
    assert_eq!(value["window"]["title"], "T");
    assert!(pretty.contains("\n  \"window\""), "indented: {pretty}");
}

/// An unknown command is a usage failure (exit 2), not a silent success.
#[test]
fn an_unknown_command_is_a_usage_failure() {
    let output = rwui(&["preview", "whatever"]);
    assert_eq!(code(&output), 2);
    assert!(stderr(&output).contains("unknown command"), "{}", stderr(&output));
}
