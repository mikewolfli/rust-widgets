// Build script for rust_widgets.
//
// Two jobs:
//   1. Declare the `full_widgets` cfg alias, which is the single source of truth
//      for "this profile has the complete widget set". See `main` for why.
//   2. Detect required system libraries at build time and print helpful
//      installation instructions if they are missing.
//
// Feature-specific system dependencies:
//   audio-output  → libasound2-dev (Linux), CoreAudio (macOS), WASAPI (Windows)
//   webkit-engine → libwebkit2gtk-4.1-dev + deps (Linux-only)
//   video-codecs  → libavcodec-dev, ... (Linux), brew ffmpeg (macOS), vcpkg (Windows)

fn main() {
    declare_cfg_aliases();
    check_system_dependencies();
}

/// Declares derived cfgs so gating conditions cannot drift apart.
///
/// Six aliases are emitted. They answer six genuinely different questions, and
/// the distinctions matter — `--no-default-features --features gpu` selects no
/// device profile at all, and `--no-default-features --features embedded` has no
/// OS runtime, so no single `not(...)` expression substitutes for another:
///
/// - `device_profile` — `desktop`, `tablet` or `mobile` is on. The question
///   "is this a device build?", answered without repeating the three-feature
///   `any(..)` at every site.
/// - `alloc_frugal` — `mini` is on. The build has no platform singleton and runs
///   on a tight allocation budget. Its complement (`not(feature = "mini")`) was
///   hand-written at ~1000 call sites.
/// - `widgets_unstripped` — neither `mini` nor `embedded`. The widget set was not
///   deliberately reduced. This is the exact meaning of the
///   `not(any(feature = "mini", feature = "embedded"))` conjunction that was
///   hand-written at 350+ call sites.
/// - `full_widgets` — a real device profile **and** unstripped. Adds the
///   requirement that the transport/API layer for a device exists, so modules
///   needing `capability`/`app`/`json`/`view` are only built here.
/// - `stripped_widgets` — `mini` or `embedded` is on. The complement of the
///   second for every build that selects a profile.
/// - `declarative_view` — `full_widgets` **and** the caller has not opted out
///   with `no-declarative-view`. This is the gate for `crate::view`.
///
/// Before these aliases existed, `widget/mod.rs` used
/// `not(any(mini, embedded)) + any(desktop, tablet, mobile)` while several
/// `capability/*.rs` importers used only `not(mini)`. The two conditions are not
/// equivalent under `embedded`, so those imports resolved to missing modules and
/// the `embedded` profile failed to compile with 370 errors. Having one name per
/// question makes that class of mismatch unrepresentable.
///
/// The names exist so a module can state its gate by *intent* (BLUE15 rule #57).
/// Writing the same conjunction at 1000+ call sites is what guarantees drift.
///
/// A retired alias is worth one sentence of history: `desktop_surface`
/// (`desktop && !embedded`) used to decide which `WidgetKind` the top-level
/// `create_menu_bar`/`create_menu`/`create_tool_bar`/`create_status_bar` aliases
/// targeted. It was the wrong question — those variants are gated
/// `widgets_unstripped`, which is true on `tablet`/`mobile` where
/// `desktop_surface` is false — so the four `create_*` calls silently produced a
/// `Panel` on those two profiles. The alias is gone so that the narrower
/// predicate cannot be reached for again; the kinds are routed on
/// `widgets_unstripped` in `src/lib.rs`.
fn declare_cfg_aliases() {
    // `cargo:rustc-check-cfg` keeps `--check-cfg` quiet on recent toolchains.
    println!("cargo:rustc-check-cfg=cfg(device_profile)");
    println!("cargo:rustc-check-cfg=cfg(full_widgets)");
    println!("cargo:rustc-check-cfg=cfg(stripped_widgets)");
    println!("cargo:rustc-check-cfg=cfg(widgets_unstripped)");
    println!("cargo:rustc-check-cfg=cfg(alloc_frugal)");
    println!("cargo:rustc-check-cfg=cfg(embedded_surface)");
    println!("cargo:rustc-check-cfg=cfg(declarative_view)");

    let has_profile =
        ["desktop", "tablet", "mobile"].iter().any(|feature| feature_enabled(feature));
    let is_mini = feature_enabled("mini");
    let is_embedded = feature_enabled("embedded");
    let is_stripped = is_mini || is_embedded;
    let view_opted_out = feature_enabled("no-declarative-view");

    if is_mini {
        println!("cargo:rustc-cfg=alloc_frugal");
    }
    if is_embedded {
        println!("cargo:rustc-cfg=embedded_surface");
    }
    if has_profile {
        println!("cargo:rustc-cfg=device_profile");
    }
    if !is_stripped {
        println!("cargo:rustc-cfg=widgets_unstripped");
    }
    if has_profile && !is_stripped {
        println!("cargo:rustc-cfg=full_widgets");
    }
    if is_stripped {
        println!("cargo:rustc-cfg=stripped_widgets");
    }
    // The declarative layer is on for a device build unless the caller opted out.
    // Both halves are required: a stripped profile has no widget tree to carry it,
    // and an explicit opt-out must be able to remove it from a device build.
    if has_profile && !is_stripped && !view_opted_out {
        println!("cargo:rustc-cfg=declarative_view");
    }

    // Re-run when any of the inputs change; Cargo tracks feature changes itself,
    // but the explicit list documents the dependency and keeps `cargo build`
    // correct for out-of-tree invocations that set the env vars directly.
    //
    // `NO_DECLARATIVE_VIEW` must be here: this script turns it into an `rustc-cfg`,
    // so without the rerun directive toggling the feature on an already-built tree
    // would reuse the cached aliases and appear to have no effect.
    for feature in
        ["desktop", "tablet", "mobile", "mini", "embedded", "portable", "no-declarative-view"]
    {
        println!(
            "cargo:rerun-if-env-changed=CARGO_FEATURE_{}",
            feature.to_uppercase().replace('-', "_")
        );
    }
}

fn feature_enabled(name: &str) -> bool {
    let env_name = format!("CARGO_FEATURE_{}", name.to_uppercase().replace('-', "_"));
    std::env::var(env_name).is_ok()
}

fn check_system_dependencies() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "linux" => check_linux(),
        "macos" => check_macos(),
        "windows" => check_windows(),
        _ => {}
    }
}

fn check_linux() {
    if feature_enabled("audio-output") {
        check_pkg("alsa", "libasound2-dev", "sudo apt-get install -y libasound2-dev");
    }
    if feature_enabled("webkit-engine") {
        check_pkg("webkit2gtk-4.1", "libwebkit2gtk-4.1-dev",
            "sudo apt-get install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev");
        check_pkg("gtk+-3.0", "libgtk-3-dev", "sudo apt-get install -y libgtk-3-dev");
    }
    if feature_enabled("video-codecs") {
        check_ffmpeg();
    }
}

fn check_macos() {
    if feature_enabled("video-codecs") {
        check_ffmpeg();
    }
}

fn check_windows() {
    if feature_enabled("video-codecs") {
        warn("'video-codecs' feature requires FFmpeg. Install: vcpkg install ffmpeg");
    }
}

fn check_pkg(name: &str, dev_pkg: &str, install_cmd: &str) {
    let ok = std::process::Command::new("pkg-config")
        .args(["--exists", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok {
        // Keep successful dependency probes silent; Cargo warnings are for
        // actionable missing dependencies only.
    } else {
        warn(&format!("Missing system library: {name} ({dev_pkg})"));
        warn(&format!("  Install: {install_cmd}"));
    }
}

fn check_ffmpeg() {
    let mut libs = vec![
        "libavcodec",
        "libavformat",
        "libavutil",
        "libavfilter",
        "libavdevice",
        "libswscale",
        "libswresample",
        "libpostproc",
    ];
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // Homebrew's FFmpeg formula does not ship libpostproc; ffmpeg-next's
        // video path does not require it.
        libs.retain(|lib| *lib != "libpostproc");
    }
    let mut all_ok = true;
    for lib in &libs {
        let ok = std::process::Command::new("pkg-config")
            .args(["--exists", lib])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            // Keep successful dependency probes silent; missing libraries are
            // reported below with an actionable installation command.
        } else {
            println!("cargo:warning=  ❌ Missing: {lib}");
            all_ok = false;
        }
    }
    if !all_ok {
        warn("Missing FFmpeg development libraries. Install all:");
        warn("  sudo apt-get install -y libavcodec-dev libavformat-dev \\");
        warn("    libavutil-dev libavfilter-dev libavdevice-dev \\");
        warn("    libswscale-dev libswresample-dev libpostproc-dev");
    }
}

fn warn(msg: &str) {
    println!("cargo:warning={msg}");
}
