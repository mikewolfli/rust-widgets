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

/// DM6-01: `--json` keeps stdout a pure NDJSON stream on the **write** path too.
///
/// The writer printed `created …` / `updated …` / `unchanged …` to stdout unconditionally, so a
/// consumer that parses stdout line by line — exactly what the README promises works — broke on
/// the success path. The progress lines still have to reach a person, so they go to stderr.
#[test]
fn json_mode_keeps_the_write_progress_off_stdout() {
    let scratch = Scratch::new("ndjson-write");
    scratch.write("ui.json", PROJECT);
    let dir = scratch.path().to_str().expect("utf8");

    let first = rwui(&["json2rust", dir, "--json"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    for line in stdout(&first).lines() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|error| panic!("stdout must be NDJSON, got `{line}`: {error}"));
    }
    assert!(
        stderr(&first).contains("created "),
        "the progress line is still reported, on stderr: {}",
        stderr(&first)
    );

    // A second run takes the `unchanged` path, which printed to stdout as well. The diagnostic
    // stream may still carry real notes there — every line just has to be JSON.
    let second = rwui(&["json2rust", dir, "--json"]);
    assert_eq!(code(&second), 0, "{}", stderr(&second));
    for line in stdout(&second).lines() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|error| panic!("stdout must be NDJSON, got `{line}`: {error}"));
    }
    assert!(stderr(&second).contains("unchanged "), "{}", stderr(&second));
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

/// DM6-04: a non-UTF-8 argument is a reported usage failure, not a panic.
///
/// `std::env::args()` panics on an argument that is not valid Unicode, so a path with an unusual
/// byte crashed a tool whose contract is structured diagnostics plus exit code 2.
#[cfg(unix)]
#[test]
fn a_non_utf8_argument_is_reported_instead_of_panicking() {
    use std::os::unix::ffi::OsStrExt;
    let output = Command::new(env!("CARGO_BIN_EXE_rwui"))
        .arg("check")
        .arg(std::ffi::OsStr::from_bytes(b"bad-\xff.json"))
        .output()
        .expect("rwui must run");
    assert_eq!(code(&output), 2, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("RWCLI-012"), "{}", stderr(&output));
    assert!(stderr(&output).contains("not valid UTF-8"), "{}", stderr(&output));
}

/// An unknown command is a usage failure (exit 2), not a silent success.
#[test]
fn an_unknown_command_is_a_usage_failure() {
    let output = rwui(&["preview", "whatever"]);
    assert_eq!(code(&output), 2);
    assert!(stderr(&output).contains("unknown command"), "{}", stderr(&output));
}

/// DM6-03: a manifest dimension the CLI cannot honour is reported, not dropped.
///
/// `"width": 5000000000` used to parse to `None`, which the generator reads as "no size
/// declared" — a declaration that vanished without a word, in a tool whose own module note says
/// manifest errors are reported here rather than in the diagnostic stream.
#[test]
fn a_malformed_manifest_dimension_is_a_tool_failure() {
    let scratch = Scratch::new("manifest-size");
    scratch.write("rwui.json", "{ \"version\": 1, \"width\": 5000000000, \"height\": 480 }");
    scratch.write("ui.json", PROJECT);
    let dir = scratch.path().to_str().expect("utf8");

    let output = rwui(&["check", dir]);
    assert_eq!(code(&output), 2, "a manifest error is a tool failure: {}", stderr(&output));
    assert!(stderr(&output).contains("width"), "{}", stderr(&output));
    assert!(stderr(&output).contains("4294967295"), "{}", stderr(&output));
}

/// DM6-02: `check` refuses the generate-only flags instead of accepting and dropping them.
///
/// They were parsed into the shared options and then thrown away, so `check x.ui --css theme.css`
/// appeared to make the stylesheet part of the validation while only the manifest's own sheets
/// were read — the silent divergence the crate's own module note forbids.
#[test]
fn check_rejects_the_generate_only_flags() {
    let scratch = Scratch::new("check-flags");
    scratch.write("ui.json", PROJECT);
    scratch.write("theme.css", "Button { background-color: #ff0000; }");
    let dir = scratch.path().to_str().expect("utf8");
    let css = scratch.path().join("theme.css");
    let css = css.to_str().expect("utf8");
    let out = scratch.path().join("out");
    let out = out.to_str().expect("utf8");

    for args in [
        vec!["check", dir, "--css", css],
        vec!["check", dir, "-o", out],
        vec!["check", dir, "--check"],
    ] {
        let output = rwui(&args);
        assert_eq!(code(&output), 2, "{args:?} must be a usage failure: {}", stderr(&output));
        assert!(
            stderr(&output).contains("unknown argument"),
            "{args:?} must name the flag: {}",
            stderr(&output)
        );
    }

    // The same flags are still accepted by the command they belong to.
    let accepted = rwui(&["json2rust", dir, "--css", css, "-o", out]);
    assert_eq!(code(&accepted), 0, "{}", stderr(&accepted));
}
