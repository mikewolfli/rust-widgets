// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! # T-23: the generator's output must **compile**, not merely look plausible
//!
//! BLUE19's DoD does not accept "should work" for this task:
//!
//! > stripped 模板的产物**实跑编译通过**：`--no-default-features --features mini`
//! > 与 `--features embedded`（**不是**「应该没问题」）
//!
//! So this file does not assert on the generated *text*. It writes the text to a real crate, runs
//! `cargo check` against the real feature set, and fails on a compiler error. A test that grepped
//! for `add_child` would pass against code that never compiles — which is exactly the failure the
//! requirement exists to prevent, because `mini` is where the `create_*` family is gated out and a
//! generated file that assumed otherwise compiles fine on the host.
//!
//! # Why every case is a real `cargo check`
//!
//! The failure modes this catches, none of which a text assertion can:
//!
//! 1. an import of a module the target does not have (`crate::view` under `mini`);
//! 2. a call to something the target profile gates out (`widget::runtime` is
//!    `cfg(not(alloc_frugal))`, as are most of the `create_*` family);
//! 3. a `&str` where `alloc_frugal` has no standard prelude to coerce it (`Button::new("Go", ..)`
//!    needs `String`);
//! 4. a constructor argument list that does not match the control (`Slider::new(text, geometry)`
//!    when `Slider::new` takes geometry only).
//!
//! All four were found by **this test**, one per run, after the generator looked correct on the
//! desktop host. That is the whole argument for compiling rather than grepping.
//!
//! # Cost
//!
//! Each case is a real compile of the library plus a small file. They are marked `#[ignore]` and run
//! by `tools/check_generator_output_compiles.sh`, so the normal test run stays fast while the
//! requirement stays enforced by a gate.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::designer::{generate, GenerationRequest, TargetProfile};

/// A project that exercises both templates: a container, a text control, a value control, a
/// disabled control and a nested container.
const PROJECT: &str = r#"{
  "window": {
    "id": "root",
    "title": "Generated",
    "width": 640,
    "height": 480,
    "layout": {
      "type": "vbox",
      "spacing": 4,
      "children": [
        { "label": { "id": "title", "text": "Hello" } },
        { "button": { "id": "go", "text": "Go", "enabled": false } },
        { "slider": { "id": "level", "value": 30 } }
      ]
    }
  }
}"#;

/// Generates for `target` and returns the source plus the report's summary.
fn generate_for(target: TargetProfile) -> (String, String) {
    let request = GenerationRequest {
        json: String::from(PROJECT),
        target,
        width: 640,
        height: 480,
        function_name: String::from("build_ui"),
    };
    let generated = generate(&request).expect("the project must parse");
    (generated.source, generated.report.summary())
}

/// Writes `source` into a throwaway crate depending on this library with `features`, then
/// `cargo check`s it. Returns the combined output and the exit code.
///
/// # Why the probe crate reuses this workspace's `target/` directory
///
/// Each probe depends on the same library, so a fresh probe with its own target dir
/// **recompiles the whole library from scratch** — about 15 seconds per case, which made this gate
/// dominate a full `run_all_gates.sh` (265s of a ~450s run). Pointing `CARGO_TARGET_DIR` at the
/// workspace target lets the probe reuse the `rust_widgets` rlib the outer `cargo test` already
/// built, so only the tiny probe crate is compiled per case.
///
/// The directory is keyed by the feature set, not by the process id: `cargo` serialises concurrent
/// access to one target dir itself, and keying by pid would defeat the reuse between the two halves
/// of this gate.
fn check_compiles(source: &str, features: &str) -> (String, bool) {
    let dir = std::env::temp_dir().join(format!("rw_gen_{}", features.replace(',', "_")));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("create the probe crate");

    // The generated manifest is TOML, where `\` starts an escape — so a native Windows path
    // (`D:\Workspace\...`) written verbatim makes the file unparsable, and the probe fails
    // with `missing escaped value` before it ever reaches the generated source. That is a
    // false failure about the host, not a finding about the generator: on Linux the path is
    // `/workspace/...` and contains no backslash, so the gate passed there and the defect
    // only appeared on Windows. TOML accepts forward slashes in a path on every platform, so
    // normalising here keeps the probe a statement about the source under test.
    let manifest_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "/");
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"rw_generated_probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
             \n[workspace]\n\
             \n[dependencies]\n\
             rust_widgets = {{ path = \"{manifest_dir}\", default-features = false, features = \
             [{features_list}] }}\n",
            features_list =
                features.split(',').map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(", ")
        ),
    )
    .expect("write the probe manifest");

    std::fs::write(dir.join("src/lib.rs"), source).expect("write the generated source");

    // Shared with the workspace so the library's rlib is reused across cases.
    //
    // # Dependency on cargo's lock timing, stated rather than assumed
    //
    // The outer `cargo test` that runs this case holds the workspace target dir's build lock while
    // it *compiles*, and releases it before the test binary runs. This probe therefore does not
    // deadlock, and reusing the rlib is what took this gate from 265s to ~10s.
    //
    // That is a real dependency on cargo's behaviour, so the wait is bounded rather than trusted: if
    // a future cargo version held the lock across the test run, `cargo check` would block forever and
    // hang the gate — the failure mode rules #58/#59 exist to prevent. `rw_run_bounded` cannot help
    // here (this is inside a Rust process), so the bound is applied to the spawned command directly.
    let shared_target = std::path::Path::new(&manifest_dir).join("target");
    let mut child = std::process::Command::new(env!("CARGO"))
        .arg("check")
        .arg("--quiet")
        .env("CARGO_TARGET_DIR", &shared_target)
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("cargo check must start");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(PROBE_TIMEOUT_SECS);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_dir_all(&dir);
                    return (
                        format!(
                            "cargo check did not finish within {PROBE_TIMEOUT_SECS}s. \
                             \n\nIf this is a build-dir lock wait, the assumption documented in \
                             `check_compiles` no longer holds and the probe needs its own target \
                             dir (at the cost of ~15s per case)."
                        ),
                        false,
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dir);
                return (format!("cargo check could not be waited on: {error}"), false);
            }
        }
    }

    let output = child.wait_with_output().expect("collect the probe's output");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    (combined, output.status.success())
}

/// How long one probe's `cargo check` may take before it is killed and reported.
///
/// A **bound**, not a target: the measured cost is a few seconds per case once the library's rlib is
/// reused. The value is generous because a cold shared target dir on a fresh checkout legitimately
/// compiles the whole library once, and that is not a failure.
const PROBE_TIMEOUT_SECS: u64 = 600;

/// The default template's output compiles for `desktop`.
#[test]
#[ignore = "compiles a real crate; run by tools/check_generator_output_compiles.sh"]
fn default_template_output_compiles_for_desktop() {
    let (source, summary) = generate_for(TargetProfile::Default);
    let (output, ok) = check_compiles(&source, "desktop");
    assert!(ok, "the generated desktop program must compile.\nreport: {summary}\n{output}");
}

/// **The requirement the DoD names.** The stripped template's output compiles under `mini`.
#[test]
#[ignore = "compiles a real crate; run by tools/check_generator_output_compiles.sh"]
fn stripped_template_output_compiles_for_mini() {
    let (source, summary) = generate_for(TargetProfile::Stripped);
    let (output, ok) = check_compiles(&source, "mini");
    assert!(
        ok,
        "the generated mini program must compile. `mini` gates the `create_*` family behind \
         `cfg(not(alloc_frugal))` and removes `crate::json`/`crate::view`, so any of those in the \
         output is a real defect.\nreport: {summary}\n{output}"
    );
}

/// The stripped template's output compiles under `embedded` too.
#[test]
#[ignore = "compiles a real crate; run by tools/check_generator_output_compiles.sh"]
fn stripped_template_output_compiles_for_embedded() {
    let (source, summary) = generate_for(TargetProfile::Stripped);
    let (output, ok) = check_compiles(&source, "embedded");
    assert!(ok, "the generated embedded program must compile.\nreport: {summary}\n{output}");
}

/// The default template's output compiles for `tablet` and `mobile`, which share its shape.
#[test]
#[ignore = "compiles a real crate; run by tools/check_generator_output_compiles.sh"]
fn default_template_output_compiles_for_tablet_and_mobile() {
    for profile in ["tablet", "mobile"] {
        let (source, summary) = generate_for(TargetProfile::Default);
        let (output, ok) = check_compiles(&source, profile);
        assert!(ok, "the generated {profile} program must compile.\nreport: {summary}\n{output}");
    }
}
