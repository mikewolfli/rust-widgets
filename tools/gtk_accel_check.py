#!/usr/bin/env python3
"""Type-check the Linux/GTK menu accelerator code on a non-Linux host.

Scratch crate: /tmp/gtkaccel. Extracts `parse_accelerator` verbatim from
`platform/linux/menu_impl.rs` and compiles it against the real gtk/gdk crates so
the accelerator API surface (Key::from_name, AccelGroup, add_accelerator, ...) is
genuinely type-checked rather than assumed.

Usage: PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig python3 tools/gtk_accel_check.py
"""

import os
import re
import sys

SOURCE = "/Users/mikewolfli/Desktop/workspace/rust-widgets/src/platform/linux/menu_impl.rs"
TARGET_DIR = "/tmp/gtkaccel"

CARGO_TOML = """[package]
name = "gtkaccel"
version = "0.0.0"
edition = "2021"

[dependencies]
gtk = "0.18"
gdk = "0.18"

[[bin]]
name = "gtkaccel"
path = "src/main.rs"
"""


def extract_block(text: str, start_marker: str) -> str:
    """Return the brace-balanced block starting at start_marker."""
    start = text.index(start_marker)
    brace = text.index("{", start)
    depth = 0
    for i in range(brace, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[start : i + 1]
    raise ValueError("unbalanced braces for " + start_marker)


def main() -> int:
    with open(SOURCE, "r", encoding="utf-8") as fh:
        source = fh.read()

    struct_block = extract_block(source, "struct ParsedAccelerator")
    keyval_block = extract_block(source, "fn keyval_for_token")
    parse_block = extract_block(source, "fn parse_accelerator")

    main_rs = "\n".join(
        [
            "#![allow(dead_code)]",
            "use gtk::prelude::*;",
            "",
            struct_block,
            "",
            keyval_block,
            "",
            parse_block,
            "",
            "/// Exercises the parsing plus the exact GTK binding calls used by",
            "/// `menu_add_item_impl`, so an API mismatch fails the build.",
            "fn bind(item: &gtk::MenuItem, group: &gtk::AccelGroup, parsed: &ParsedAccelerator) {",
            "    if let Some((keyval, modifiers)) = parsed.binding {",
            "        item.add_accelerator(\"activate\", group, keyval, modifiers, gtk::AccelFlags::VISIBLE);",
            "    }",
            "}",
            "",
            "/// Fails loudly if a chord that should bind does not, so the check cannot",
            "/// pass by silently binding nothing.",
            "fn expect_bound(case: &str, parsed: &ParsedAccelerator) {",
            "    assert!(parsed.binding.is_some(), \"expected {case} to bind, got none\");",
            "    assert!(parsed.display.is_some(), \"expected {case} to keep display text\");",
            "}",
            "",
            "fn main() {",
            "    let _ = gtk::init();",
            "    let cases = [\"Ctrl+S\", \"Cmd+S\", \"Ctrl+Shift+Z\", \"\\u{2318}S\",",
            "                 \"\\u{21e7}\\u{2318}Z\", \"F1\", \"F12\", \"Alt+F4\", \"Ctrl+/\",",
            "                 \"Ctrl+Home\", \"Ctrl+PageDown\", \"Ctrl+Shift+K\"];",
            "    let group = gtk::AccelGroup::new();",
            "    let item = gtk::MenuItem::with_label(\"Test\");",
            "    for case in cases {",
            "        let parsed = parse_accelerator(Some(case));",
            "        expect_bound(case, &parsed);",
            "        bind(&item, &group, &parsed);",
            "        println!(\"bound {case} -> {}\", parsed.display.as_deref().unwrap_or(\"\"));",
            "    }",
            "    // Absent/blank input must produce no binding rather than a bad one.",
            "    for case in [None, Some(\"\"), Some(\"   \")] {",
            "        let parsed = parse_accelerator(case);",
            "        assert!(parsed.binding.is_none(), \"blank input must not bind\");",
            "    }",
            "    println!(\"all {} accelerators bound\", cases.len());",
            "}",
            "",
        ]
    )

    os.makedirs(os.path.join(TARGET_DIR, "src"), exist_ok=True)
    with open(os.path.join(TARGET_DIR, "Cargo.toml"), "w", encoding="utf-8") as fh:
        fh.write(CARGO_TOML)
    with open(os.path.join(TARGET_DIR, "src", "main.rs"), "w", encoding="utf-8") as fh:
        fh.write(main_rs)
    print("wrote", os.path.join(TARGET_DIR, "src", "main.rs"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
