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
//   linux-gtk     → libgtk-3-dev (Linux, via the `gtk` crate behind `gtk-native`)
//   video-codecs  → libavcodec-dev, ... (Linux), brew ffmpeg (macOS), vcpkg (Windows)

fn main() {
    declare_cfg_aliases();
    check_system_dependencies();
}

/// Declares derived cfgs so gating conditions cannot drift apart.
///
/// **Seven** aliases answer genuinely different questions, and the distinctions
/// matter — `--no-default-features --features gpu` selects no device profile at
/// all, and `--no-default-features --features embedded` has no OS runtime, so no
/// single `not(...)` expression substitutes for another:
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
/// - `embedded_surface` — `embedded` is on. Names the bare-surface profile, the
///   companion to `alloc_frugal` for the stripped case.
///
/// Two further aliases are declared (and emitted) here for the same reason, but
/// are gated by a *feature conjunction* rather than by the profile matrix:
///
/// - `designer_tooling` — `full_widgets` and the `designer` feature. See its own
///   note below for why it is not folded into `full_widgets`.
/// - `cjk_outline_face` — `fonts-cjk`, a single shard, or a `fonts-cjk-shard-*`
///   shard. See its own note below.
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
    // The designer gate (BLUE19 D7-b-3). Declared here so a `cfg(designer_tooling)`
    // typo is a build error rather than a silently-false condition.
    println!("cargo:rustc-check-cfg=cfg(designer_tooling)");
    // The CJK outline-face gate. The single `fonts-cjk` face and the sharded `fonts-cjk-shard-*`
    // faces are the same kind of thing — an outline face that `VectorSource`/`outline` can draw —
    // so every site that gates on "is there a CJK outline face?" must accept either. Spelled once
    // here because the disjunction appears at 20+ sites and a hand-written copy would drift the
    // moment a shard is added (rule #47).
    println!("cargo:rustc-check-cfg=cfg(cjk_outline_face)");

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

    // ── `designer_tooling` — the code generator and its artifact writer ──
    //
    // A separate alias rather than a bare `feature = "designer"` at every `cfg`,
    // because the condition is a **conjunction** and rule #47 forbids hand-writing
    // one at more than a couple of call sites. The conjunction is:
    //
    //   * a real device profile with an unstripped widget set (`full_widgets`),
    //     because generating needs `serde_json` to parse a project; and
    //   * the caller asked for it (`designer`, which `desktop` turns on by
    //     default — see the feature's own note in `Cargo.toml`).
    //
    // # Why this is not folded into `full_widgets`
    //
    // `full_widgets` answers "does this build have the widget tree and the
    // capability table?", which a shipping application needs. This answers "is this
    // build **also** a design tool?", which it does not: linking the generator and
    // `std::fs::write` into every `tablet`/`mobile` build would put a
    // development-time facility in a delivery artifact, and mode 2 exists partly to
    // keep delivery artifacts free of exactly that kind of weight.
    let designer_opted_in = feature_enabled("designer");
    if has_profile && !is_stripped && designer_opted_in {
        println!("cargo:rustc-cfg=designer_tooling");
    }

    // `cjk_outline_face` — the single vector CJK face *or* any of its shards. They are different
    // packagings of the same capability (a CJK outline a `VectorSource` can rasterise), so a gate
    // asking "can this build draw CJK from outlines?" must accept either. See `declare_cfg_aliases`'s
    // doc for why this is a named alias rather than a written-out `any(...)`.
    let cjk_outline = feature_enabled("fonts-cjk")
        || feature_enabled("fonts-cjk-shard-latin")
        || feature_enabled("fonts-cjk-shard-symbols")
        || feature_enabled("fonts-cjk-shard-kana")
        || feature_enabled("fonts-cjk-shard-fullwidth")
        || feature_enabled("fonts-cjk-shard-han");
    if cjk_outline {
        println!("cargo:rustc-cfg=cjk_outline_face");
    }

    // Re-run when any of the inputs change; Cargo tracks feature changes itself,
    // but the explicit list documents the dependency and keeps `cargo build`
    // correct for out-of-tree invocations that set the env vars directly.
    //
    // `NO_DECLARATIVE_VIEW` must be here: this script turns it into an `rustc-cfg`,
    // so without the rerun directive toggling the feature on an already-built tree
    // would reuse the cached aliases and appear to have no effect.
    for feature in [
        "desktop",
        "tablet",
        "mobile",
        "mini",
        "embedded",
        "portable",
        "no-declarative-view",
        // The positive spelling (see the feature's own note in `Cargo.toml`). It does not generate a
        // `rustc-cfg` on its own — `declarative_view` is resolved from the opt-out — but it is
        // listed because toggling it changes the *feature set* the resolution reads, and a stale
        // cached alias would then describe a configuration this build is not.
        "declarative-view",
        // `designer` is in this list for the same reason as `no-declarative-view`: it
        // becomes an `rustc-cfg`, so without the directive, toggling it on an
        // already-built tree would reuse the cached aliases and appear to do nothing.
        "designer",
    ] {
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
    // Not part of the match above: the XComponent bridge is a *feature*, not a target, and it
    // can be enabled on any host. On an OpenHarmony target it links the SDK; on any other
    // host it compiles but stays inert, and the check warns there (see `check_xcomponent`).
    // Checking it here means the message appears whichever target is being built.
    check_xcomponent();
}

fn check_linux() {
    if feature_enabled("audio-output") {
        check_pkg("alsa", "libasound2-dev", "sudo apt-get install -y libasound2-dev");
    }
    if feature_enabled("linux-gtk") {
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

/// Emits the link directive for the ArkUI XComponent bridge, and warns when it cannot work.
///
/// # Why this is a build script concern and not a `#[link]` attribute
///
/// The library lives in the OpenHarmony SDK's sysroot, at a path only the environment knows.
/// `cargo-ohos` computes the sysroot and passes `-L` for it; a `#[link(name = "ace_ndk")]`
/// attribute would additionally require naming the library the same way on every SDK layout.
/// Emitting `cargo:rustc-link-lib` from here keeps the name in one place and lets the preflight
/// below explain a failure before the linker does — a link error alone reads as "undefined
/// symbol: OH_NativeXComponent_*", which does not tell a reader that the SDK is missing.
///
/// # The honest half
///
/// On a host build (`x86_64-unknown-linux-gnu`, not `*-ohos`) the link directives are
/// **not** emitted at all: the emission below is gated on `target_env == "ohos"`.
/// The bridge therefore compiles on the host (the FFI declarations it calls are
/// themselves `#[cfg(target_env = "ohos")]`) and links cleanly — it is simply
/// inert, because there is no `libace_ndk` to attach an accessibility provider
/// from. The warning makes that explicit up front so a host build does not look
/// like a working bridge.
fn check_xcomponent() {
    if !feature_enabled("xcomponent") {
        return;
    }
    // `OHOS_SDK_NATIVE` is read below and turns into `rustc-link-search` /
    // `rustc-link-arg` directives. Without this directive, changing the SDK
    // path on an already-built tree would reuse the cached build-script output
    // and link against the stale directory — the same hazard the feature list
    // in `declare_cfg_aliases` documents for its cfgs.
    println!("cargo:rerun-if-env-changed=OHOS_SDK_NATIVE");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // The link directives are emitted **only for an OpenHarmony target**.
    //
    // # Why the emission is gated and not just warned about
    //
    // `-lace_ndk.z` names a library that exists only in the OpenHarmony sysroot. Emitting it
    // for a host build asked the host linker for a library that is not there, so *every*
    // `cargo test` on a developer machine failed to link — which meant nothing in
    // `src/platform/harmony/` could be unit-tested anywhere, including CI. The module's own
    // tests are how a defect in the bridge's non-FFI half (the name store, the event-type
    // constants, the "no provider ⇒ nothing posted" rule) gets caught, so an untestable
    // module is the expensive outcome and a warning is the cheap one.
    //
    // `harmony xcomponent` remains a valid **host** configuration: the code still compiles,
    // because the declarations it calls are themselves `#[cfg(target_env = "ohos")]`. What
    // changes is that the host build no longer tries to link a device library it does not
    // have. The warning below still says the bridge is inert there.
    if target_env != "ohos" {
        warn(&format!(
            "'xcomponent' is enabled for target_os='{target_os}' target_env='{target_env}', which is \
             not an OpenHarmony target. The bridge compiles but is inert here: it will not link \
             the SDK's libace_ndk, so no accessibility provider is attached and no key, touch or \
             mouse callback can fire. Build for a '*-unknown-linux-ohos' target to use it."
        ));
        return;
    }

    // The library is `libace_ndk.z.so`, so the link name is `ace_ndk.z` and not `ace_ndk` —
    // the `.z` is part of the soname the SDK ships (`OHOS` uses it for the zh-CN build variant,
    // and it is present in every SDK layout). Getting this wrong compiles fine and then fails
    // with `unable to find library -lace_ndk`, which names a library that does not exist rather
    // than the one that does.
    println!("cargo:rustc-link-lib=ace_ndk.z");
    // The SDK's own sysroot is where the `.so` lives, and `cargo-ohos` sets `--sysroot` for the
    // compiler but does not add the arch-specific library directory to the linker's search
    // path. Adding it here is what makes the bridge linkable rather than merely compilable.
    if let Ok(native) = std::env::var("OHOS_SDK_NATIVE") {
        let target = std::env::var("TARGET").unwrap_or_default();
        let arch_dir = match target.split('-').next().unwrap_or_default() {
            "aarch64" => "aarch64-linux-ohos",
            "armv7" | "arm" => "arm-linux-ohos",
            "x86_64" => "x86_64-linux-ohos",
            other => {
                warn(&format!(
                    "'xcomponent' is enabled for target '{target}', whose OHOS library directory \
                     this build script does not know; expected one of aarch64/armv7/x86_64. \
                     Link will fail unless the SDK's lib directory is passed with -L."
                ));
                let _ = other;
                ""
            }
        };
        if !arch_dir.is_empty() {
            let lib_dir = format!("{native}/sysroot/usr/lib/{arch_dir}");
            if std::path::Path::new(&lib_dir).is_dir() {
                println!("cargo:rustc-link-search=native={lib_dir}");
            } else {
                warn(&format!(
                    "'xcomponent': expected the OpenHarmony libraries at '{lib_dir}', which does \
                     not exist. Check that OHOS_SDK_NATIVE points at the SDK's 'native' directory."
                ));
            }
            // Rust needs to find the shared object at link time; the device supplies it at run
            // time from its own system libraries.
            println!("cargo:rustc-link-arg=-Wl,-rpath-link,{lib_dir}");
        }
    } else {
        warn("'xcomponent' is enabled for an OpenHarmony target but OHOS_SDK_NATIVE is not set.");
        warn("  The ArkUI bridge links the SDK's 'libace_ndk.z.so':");
        warn("    export OHOS_SDK_NATIVE=<sdk>/linux/native");
        warn("  and build with, e.g.:");
        warn("    cargo ohos build -t aarch64 --features 'harmony xcomponent'");
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
