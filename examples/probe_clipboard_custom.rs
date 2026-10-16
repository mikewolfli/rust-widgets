// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stages an application-defined clipboard MIME type on a live session.
//!
//! # What this answers
//!
//! The designer feedback (`docs/issues/feedback-20261015.md` §十一) reported that the clipboard could
//! carry only the five fixed kinds, so a designer copying its own payload had to smuggle the bytes
//! through `text/plain` and could not negotiate the type by name. This probe pins the three facts the
//! feedback's acceptance asked for, on the session it runs in:
//!
//! * **A** — after `set_contents_with_type(TYPE, payload)`, `has_format(TYPE)` answers `true` (the
//!   type is negotiable by name);
//! * **B** — `get_contents_for_type(TYPE)` returns the payload byte-for-byte;
//! * **C** — the structured route (`set_contents(ClipboardContent::Custom { … })`) lands in the same
//!   place, and a type the clipboard does not hold answers `None` rather than the one it does.
//!
//! # Which backends stage it
//!
//! Linux/GTK (an X11/Wayland selection target), Windows (a clipboard format registered by the MIME
//! name) and macOS (an `NSPasteboard` type named by the MIME string, in both the cocoa and objc2
//! backends; see `src/platform/clipboard_stubs.rs`).
//!
//! # This replaces your clipboard
//!
//! Publishing a custom type makes *this* process the selection owner; the probe saves the plain text
//! that was there and puts it back at the end, but a non-text clipboard (an image, say) cannot be
//! restored and is replaced. That is why it is gated behind the same opt-in as the window probes.
//!
//! Run with:
//!     cargo run --no-default-features --features "desktop" --example probe_clipboard_custom
//!     cargo run --no-default-features --features "desktop,gtk-native" --example probe_clipboard_custom
//!
//! Exit codes: `0` = every check held, `1` = a check failed (the line says which), `2` = this host
//! cannot stage it (no display session / no clipboard, or the operator did not opt in).
//!
//! # The three conditions, spelled once
//!
//! `cfg` attributes cannot name a `cfg` predicate through a helper, so the "this backend carries a
//! custom type natively" condition is written out at each site below and has to stay in one shape:
//! a GDK selection target (Linux), a registered clipboard format (Windows), or an `NSPasteboard`
//! type named by the MIME string (macOS, either backend).

#[cfg(not(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn main() {
    println!(
        "probe_clipboard_custom: this profile has no platform clipboard (alloc-frugal or a \
         non-desktop target), so there is nothing to stage."
    );
}

/// `main` for a desktop host: stages the probe where the backend carries a custom type natively
/// (Linux/GTK, Windows, macOS), and prints the honest refusal where it does not.
#[cfg(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn main() {
    #[cfg(any(
        all(target_os = "linux", feature = "gtk-native"),
        target_os = "windows",
        all(target_os = "macos", any(feature = "cocoa-legacy", feature = "macos"))
    ))]
    {
        run_staging_probe();
    }
    #[cfg(not(any(
        all(target_os = "linux", feature = "gtk-native"),
        target_os = "windows",
        all(target_os = "macos", any(feature = "cocoa-legacy", feature = "macos"))
    )))]
    {
        println!(
            "probe_clipboard_custom: this backend has no native custom-type clipboard \
             (`--features desktop,gtk-native` on a host with a display session, or `desktop` on \
             Windows/macOS, stages it); its write reports the honest refusal instead."
        );
    }
}

#[cfg(all(
    not(alloc_frugal),
    any(
        all(target_os = "linux", feature = "gtk-native"),
        target_os = "windows",
        all(target_os = "macos", any(feature = "cocoa-legacy", feature = "macos"))
    )
))]
fn gui_probe_allowed() -> bool {
    if std::env::var("RW_GUI_PROBE_OK").ok().as_deref() == Some("1") {
        return true;
    }
    println!(
        "this probe replaces the clipboard on the session it runs on; set RW_GUI_PROBE_OK=1 to \
         stage it (and prefer a machine whose clipboard nobody is using)"
    );
    false
}

#[cfg(all(
    not(alloc_frugal),
    any(
        all(target_os = "linux", feature = "gtk-native"),
        target_os = "windows",
        all(target_os = "macos", any(feature = "cocoa-legacy", feature = "macos"))
    )
))]
fn run_staging_probe() {
    if !gui_probe_allowed() {
        std::process::exit(2);
    }
    use rust_widgets::platform::clipboard::ClipboardContent;

    const NODES: &str = "application/vnd.rustage.nodes+json";

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== clipboard custom-type probe ===");
    println!("backend: {}", rust_widgets::platform::backend_name());
    rust_widgets::init();
    if !rust_widgets::platform::capabilities().clipboard {
        println!("unsupported host: no clipboard is reachable in this session");
        std::process::exit(2);
    }

    let Some(clip) = rust_widgets::platform_clipboard() else {
        println!("unsupported host: the backend exposes no rich clipboard here");
        std::process::exit(2);
    };

    // Save the plain text so the session is left roughly as it was found.
    let saved = rust_widgets::clipboard_text();

    let payload = br#"{"vendor":"rustage","kind":"nodes","nodes":[]}"#.to_vec();

    println!("\nA. set_contents_with_type / has_format");
    let wrote = clip.set_contents_with_type(NODES, &payload);
    println!("   set_contents_with_type -> {wrote}");
    check(wrote, "the custom type was stored");
    check(clip.has_format(NODES), "has_format answers true for the custom type");

    println!("\nB. get_contents_for_type");
    let read_back = clip.get_contents_for_type(NODES);
    println!("   get_contents_for_type -> {} byte(s)", read_back.as_ref().map_or(0, Vec::len));
    check(read_back.as_deref() == Some(payload.as_slice()), "the payload comes back byte-for-byte");
    check(
        clip.get_contents_for_type("text/plain").is_none(),
        "a type the clipboard does not hold answers None",
    );

    println!("\nC. the structured Custom route lands in the same place");
    let custom = ClipboardContent::Custom {
        content_type: "application/vnd.rustage.resource+json".into(),
        data: vec![1, 2, 3, 4],
    };
    let custom_type = custom.content_type().to_string();
    let stored = clip.set_contents(custom);
    println!("   set_contents(Custom) -> {stored}");
    check(stored && clip.has_format(&custom_type), "a structured Custom write is readable by name");
    check(
        clip.get_contents_for_type(&custom_type).as_deref() == Some([1u8, 2, 3, 4].as_slice()),
        "its bytes come back unchanged",
    );

    // Leave the session close to how it was found.
    match saved {
        Some(text) => {
            let _ = rust_widgets::set_clipboard_text(&text);
        }
        None => {
            let _ = rust_widgets::set_clipboard_text("");
        }
    }

    println!("\n=== {} check(s) failed ===", failures);
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
