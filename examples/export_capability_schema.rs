// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AG-13 — export the machine-readable capability document.
//!
//! # What this is for
//!
//! "What properties and events does this control have?" used to require reading
//! `widget/capability/access.rs` plus `capability/types.rs` — a thousand lines each — and then
//! re-deriving the answer by hand. The answer already exists at runtime: the factory holds one
//! `WidgetCapability` per registered name, and the designer manifest can serialise it.
//!
//! So this writes that answer once, as `docs/capability_schema.json`: one object per control, in
//! name order, with every published property (kind, readable, writable, accepted tokens,
//! constraints, default value) and every published event (payload kind and shape), plus the
//! commands. A tool — a designer, an editor plugin, a diff in review — reads a file instead of
//! the source.
//!
//! # Why a committed file rather than a runtime call
//!
//! The document is only useful if it can be **read without building the crate** and **diffed in
//! review**. That means a committed artifact, and a committed artifact needs a producer that
//! agrees with it: `tools/check_capability_schema_is_current.sh` runs this example with `--check`
//! and compares the regenerated text with the committed file, so a capability that changes
//! without a regenerated document fails a gate instead of quietly going stale.
//!
//! The document deliberately carries **no** hand-written prose: it is a description of the code,
//! and anything a human writes about a control belongs in the cookbook.
//!
//! # Usage
//!
//! ```sh
//! cargo run --example export_capability_schema              # write the committed file
//! cargo run --example export_capability_schema -- --check   # compare, exit 1 on drift
//! ```

use std::fs;
use std::path::Path;

/// The committed document this example owns.
const OUTPUT: &str = "docs/capability_schema.json";

/// Replaces the two fields that describe **the build host** with a fixed value.
///
/// # Why the document is normalized
///
/// The manifest carries `profile` (the active Cargo profile) and `triple` (the host the library
/// was compiled for). Both are facts about the machine that ran the exporter, not about the
/// control, so committing them would make the file regenerate differently on every architecture —
/// a committed artifact that fails its own drift gate on another host. They are pinned to `"any"`
/// and the rest of the document (every property, event and command) stays exactly what the
/// capability table says. `library` is deliberately **not** pinned: it is the crate version, and
/// a release bump is supposed to move it and be reviewed.
fn pin_host_fields(json: &str) -> String {
    let mut out = String::with_capacity(json.len() + 16);
    for line in json.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        if trimmed.starts_with("\"profile\":") || trimmed.starts_with("\"triple\":") {
            let key = if trimmed.starts_with("\"profile\":") { "profile" } else { "triple" };
            out.push_str(indent);
            out.push_str(&format!("\"{key}\": \"any\","));
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let check = std::env::args().any(|argument| argument == "--check");

    let factory = rust_widgets::widget::WidgetFactory::new_with_defaults();
    let json = rust_widgets::widget::capability::all_capability_manifests_json(&factory)
        .map_err(|error| error.to_string())?;
    let json = pin_host_fields(&json);

    let path = Path::new(OUTPUT);
    if check {
        let committed = fs::read_to_string(path).map_err(|error| {
            format!(
                "{OUTPUT} is missing ({error}); run `cargo run --example export_capability_schema`"
            )
        })?;
        if committed != json {
            eprintln!(
                "❌ {OUTPUT} does not match a fresh export ({} committed bytes, {} regenerated).\n\
                 \x20  Run `cargo run --example export_capability_schema` and commit the result.",
                committed.len(),
                json.len()
            );
            std::process::exit(1);
        }
        println!("capability schema is current ({} bytes)", json.len());
        return Ok(());
    }

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, &json)?;
    println!("wrote {OUTPUT} ({} bytes)", json.len());
    Ok(())
}
