// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The **committed** generated sources must compile, and compile **warning-free**.
//!
//! # Why this is separate from `generator_output_compiles_test.rs`
//!
//! That test compiles what the generator emits **for a fixture it constructs**. This one compiles
//! what is actually **checked into the tree** — `examples/generated_project/src/generated/`. The two
//! answer different questions:
//!
//! * the other test asks "can the generator produce compiling code?";
//! * this one asks "is the code in the tree the code the generator would produce, and does it
//!   build clean?".
//!
//! The second question is the one D7-b-3 creates, because committing an artifact is only safe if the
//! committed artifact is verified. `tools/check_generated_sources.sh` covers "is it in sync"; this
//! covers "is what is committed any good".
//!
//! # Why `-D warnings`
//!
//! The first run of this file found two real defects in the committed output, in opposite directions,
//! from the same root cause — the generator's `mut` placement was a guess:
//!
//! * `warning: variable does not need to be mutable` on every child whose setters ran inside their own
//!   block (the outer binding is read once by `add_child`);
//! * `error: cannot borrow root as mutable` on the root, which `base_mut()` **does** need.
//!
//! Neither showed up in `generator_output_compiles_test.rs`, because that test's fixture happened to
//! have a child with no setters at the root level. A generated file that warns under the host's own
//! lints fails a downstream `-D warnings` build for a reason that has nothing to do with the project
//! document, so warnings are treated as failures here.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::designer::artifact::{plan_artifacts, ArtifactPaths, GENERATED_MARKER};

/// Where the committed project lives, relative to the crate root.
const PROJECT_DIR: &str = "examples/generated_project";

/// The document the committed artifacts were generated from.
fn project_json() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT_DIR).join("project.json");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

/// Compiles `sources` as a crate that depends on this library, denying warnings.
///
/// # Why `--all-targets` is not used
///
/// The subject is a `lib` file, and the lint that matters (`unused_mut`) is reported for any target.
/// Keeping it to the default target means one compile rather than several, which matters because this
/// gate is in the full-suite path.
fn compile_denying_warnings(sources: &[(String, String)]) -> (String, bool) {
    let dir = std::env::temp_dir().join("rw_committed_artifacts_probe");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("create the probe crate");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"rw_committed_probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
             \n[workspace]\n\
             \n[dependencies]\n\
             rust_widgets = {{ path = \"{manifest_dir}\", default-features = false, features = \
             [\"desktop\"] }}\n"
        ),
    )
    .expect("write the probe manifest");

    let mut module_list = String::new();
    for (name, text) in sources {
        std::fs::write(dir.join(format!("src/{name}.rs")), text)
            .expect("write a committed artifact");
        module_list.push_str(&format!("pub mod {name};\n"));
    }
    std::fs::write(dir.join("src/lib.rs"), module_list).expect("write the probe module list");

    // Shared with the workspace so the library's rlib is reused rather than rebuilt. The dependency
    // on cargo releasing its build lock before the test binary runs is the same one documented in
    // `generator_output_compiles_test.rs::check_compiles`, and it is bounded the same way.
    let shared_target = std::path::Path::new(manifest_dir).join("target");
    let mut child = std::process::Command::new(env!("CARGO"))
        .arg("check")
        .arg("--all-targets")
        // The point of this test: a warning is a failure.
        .env("RUSTFLAGS", "-D warnings")
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
                        format!("cargo check did not finish within {PROBE_TIMEOUT_SECS}s"),
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

/// See `generator_output_compiles_test.rs` for the reasoning behind the bound.
const PROBE_TIMEOUT_SECS: u64 = 600;

/// **The committed artifacts must be in sync with their project document.**
///
/// This is the in-process half of `tools/check_generated_sources.sh`. It is duplicated here on
/// purpose: the gate is what CI runs, but a developer running `cargo test` should see the same
/// failure without having to know the gate exists.
#[test]
fn the_committed_artifacts_match_their_project_document() {
    let json = project_json();
    // The geometry the committed files were generated with; changing it here without regenerating
    // them is exactly the drift this detects.
    let artifacts = plan_artifacts(&json, 640, 480).expect("the committed project must parse");

    for artifact in &artifacts {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT_DIR).join(&artifact.path);
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "{} is missing: {error}. Regenerate with `cargo run --no-default-features \
                 --features desktop,designer --example designer_generate -- --project {PROJECT_DIR}/project.json \
                 --root {PROJECT_DIR} --width 640 --height 480`",
                path.display()
            )
        });
        assert_eq!(
            committed,
            artifact.source,
            "{} does not match {PROJECT_DIR}/project.json. Regenerate it rather than editing it.",
            path.display()
        );
    }
}

/// **The committed artifacts must compile under `-D warnings`.**
#[test]
#[ignore = "compiles a real crate; run by tools/check_generated_sources.sh"]
fn the_committed_artifacts_compile_warning_free() {
    let json = project_json();
    let artifacts = plan_artifacts(&json, 640, 480).expect("the committed project must parse");

    // Read the **committed** files, not the freshly generated text: this test's subject is what is in
    // the tree, and `the_committed_artifacts_match_their_project_document` separately proves the two
    // are the same. Reading the generated text instead would make this a second copy of the other
    // compile test.
    let mut sources = Vec::new();
    for artifact in &artifacts {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT_DIR).join(&artifact.path);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        assert!(
            text.contains(GENERATED_MARKER),
            "{} must carry the generated marker",
            path.display()
        );
        let module = path.file_stem().and_then(|stem| stem.to_str()).expect("a module name");
        sources.push((String::from(module), text));
    }
    assert_eq!(sources.len(), 2, "both targets are committed");

    let (output, ok) = compile_denying_warnings(&sources);
    assert!(
        ok,
        "the committed generated sources must compile with `-D warnings`.\n\
         A warning here fails a downstream build for a reason unrelated to the project document.\n\
         {output}"
    );
}

/// The committed project must exercise both templates, or the pair proves little.
#[test]
fn the_committed_project_exercises_both_templates() {
    let json = project_json();
    let artifacts = plan_artifacts(&json, 640, 480).expect("parses");
    let default = artifacts
        .iter()
        .find(|a| a.path == ArtifactPaths::path_for(rust_widgets::designer::TargetProfile::Default))
        .expect("the default artifact");
    let stripped = artifacts
        .iter()
        .find(|a| {
            a.path == ArtifactPaths::path_for(rust_widgets::designer::TargetProfile::Stripped)
        })
        .expect("the stripped artifact");

    assert!(default.source.contains("Node::new"), "the default template builds a tree");
    assert!(stripped.source.contains("try_add_child"), "the stripped template adds imperatively");
    // A project that generated nothing would satisfy both `contains` checks vacuously, so the node
    // counts are asserted too.
    assert!(default.report.nodes_emitted >= 3, "the fixture must be non-trivial");
    assert_eq!(
        default.report.nodes_emitted, stripped.report.nodes_emitted,
        "both templates describe the same document"
    );
    assert!(
        stripped.report.capacity_overflow.is_empty(),
        "the committed fixture must fit the stripped target"
    );
}
