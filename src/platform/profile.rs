// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Single source of truth for compile-time runtime-profile facts.
//!
//! # Why this module exists
//!
//! Before it, the questions "does this build have an OS runtime?", "does it
//! stream input from an OS backend?", "is the complete widget set compiled in?"
//! were each re-answered with a hand-written conjunction of `feature` tests, at
//! every call site. `src/lib.rs` alone carried six copies of
//! `runtime_profile_name()`, two of `runtime_route_name()`, three of
//! `init_runtime_backend()`, and four of `init_i18n_runtime()` — all differing
//! only in their `cfg` attribute. `src/` held roughly 1500 further
//! `feature = "mini" | "embedded"` tests.
//!
//! Copies of a conjunction drift. `full_widgets` and `stripped_widgets` already
//! exist in `build.rs` for exactly that reason, and the earlier `full_widgets`
//! bug (370 compile errors under `embedded`, caused by `not(mini)` being read as
//! "full") is the proof: two spellings of "the complete set", silently
//! disagreeing.
//!
//! # The contract
//!
//! This is the **only** module in `src/` allowed to test a profile feature name.
//! Everything else asks a semantic question here:
//!
//! ```ignore
//! if crate::platform::profile::has_os_runtime() { /* … */ }
//! ```
//!
//! `mini` and `embedded` remain the *input* — they are Cargo features and cannot
//! be redefined. What changes is that the **translation from features to facts**
//! happens once, here, and the rest of the crate is free of the feature names
//! (BLUE15 rules #57/#58).

use crate::core::RuntimeProfile;
use crate::render_engine::RenderEngine;

/// What kind of device this build targets.
///
/// This is the `mini`/`embedded` distinction reduced to the only two facts that
/// actually differ, so the 1500 scattered feature tests cannot come back.
///
/// Named `ProfileClass` rather than `DeviceClass` because
/// [`crate::core::DeviceClass`] already names *form factors* (Desktop/Tablet/
/// Mobile/Projector) — a different question, and two same-named types would be a
/// trap (principle #49).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileClass {
    /// `desktop` / `tablet` / `mobile`: a full device with an OS runtime.
    Device,
    /// `embedded`, or a build with no device profile at all: a bare render
    /// surface with no OS-hosted window.
    Surface,
    /// `mini`: no OS runtime *and* an alloc-frugal memory budget.
    Minimal,
}

/// `true` when this build targets OpenHarmony / HarmonyOS.
///
/// # Why this predicate exists (and why it tests `target_env`, not `target_os`)
///
/// Every OpenHarmony Rust target is spelled `*-unknown-linux-ohos`
/// (`aarch64`, `armv7`, `x86_64`, `loongarch64`). Despite the name, rustc reports
/// `target_os = "linux"` for them — verified with
/// `rustc --target aarch64-unknown-linux-ohos --print cfg`:
///
/// ```text
/// target_abi=""
/// target_env="ohos"      <- the only discriminator
/// target_family="unix"
/// target_os="linux"
/// ```
///
/// So `cfg(target_os = "ohos")` can never match, and a backend selected by it
/// silently falls through to whatever the `linux` arm provides. The honest test is
/// `target_env`, which is exactly the field OpenHarmony's target spec overrides.
///
/// `cfg(unix)` is deliberately **not** used here: it is true for both OpenHarmony
/// and ordinary Linux, so it cannot tell them apart.
///
/// Note that this is a *compile-time target fact*, so it is legitimately a `cfg` in
/// the sense of BLUE15 principle #42: it describes the execution environment the
/// artifact is built for, not "which OS the developer happens to use".
///
/// The `harmony` Cargo feature remains the way to select the backend on a
/// *non*-OpenHarmony host for development and testing.
pub const fn is_openharmony_target() -> bool {
    cfg!(target_env = "ohos")
}

/// Compile-time proof that the discriminator above is the only one that works.
///
/// Phrased as an implication so it is evaluated — and vacuously true — on every
/// target. The earlier form (`if cfg!(target_env = "ohos") { const { .. } }`)
/// looked equivalent but was not: an inline `const` block is const-evaluated even
/// in a branch that is never taken, so the assertion also fired on Windows,
/// macOS and wasm (where `target_os` is not `linux`) and broke `cargo test --lib`
/// there. As an implication the check is real on OpenHarmony and inert elsewhere.
const _: () = assert!(
    !cfg!(target_env = "ohos") || cfg!(target_os = "linux"),
    "OpenHarmony targets report target_os=linux; if that ever stops being true, \
     `is_openharmony_target()` must be re-derived from a different field"
);

/// How this build is driven at runtime.
///
/// The pair distinguishes "the OS owns the loop" from "the library owns the
/// loop", while `ProfileClass` names which memory/host budget applies. Encoding
/// both keeps the two independent: a `Surface` device is still OS-hosted when a
/// caller supplies the loop, and `mini` is never OS-hosted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineClass {
    /// The OS owns the window and pumps the event loop.
    OsHosted(ProfileClass),
    /// The library owns the loop over its own surface.
    SelfHosted(ProfileClass),
}

/// The device class this build targets.
///
/// Note that a build with **no** device feature at all reports
/// [`ProfileClass::Surface`] rather than `Device`: such a build has no OS runtime
/// to host a window, and the earlier code recorded that as `"unknown"`.
pub const fn profile_class() -> ProfileClass {
    if cfg!(feature = "mini") {
        ProfileClass::Minimal
    } else if cfg!(any(feature = "desktop", feature = "tablet", feature = "mobile")) {
        ProfileClass::Device
    } else {
        // `embedded`, `profile-embedded-mini`, or no device feature at all.
        ProfileClass::Surface
    }
}

/// How this build is driven at runtime.
pub const fn engine_class() -> EngineClass {
    if cfg!(feature = "mini") {
        EngineClass::SelfHosted(ProfileClass::Minimal)
    } else if cfg!(any(feature = "desktop", feature = "tablet", feature = "mobile")) {
        EngineClass::OsHosted(ProfileClass::Device)
    } else {
        // `embedded` renders through the library-owned loop; it has a surface
        // but no OS-hosted window.
        EngineClass::SelfHosted(ProfileClass::Surface)
    }
}

/// `true` when this build has an OS runtime that can host a window.
///
/// Equivalent to `stripped_widgets` being off *and* a device profile being on.
/// Callers use this to decide whether to talk to a real backend
/// (`platform::get_platform`) or to the surface-only fallback.
pub const fn has_os_runtime() -> bool {
    matches!(engine_class(), EngineClass::OsHosted(_))
}

/// `true` when this build streams input from an OS backend.
///
/// Today this is the same predicate as [`has_os_runtime`], but the two are kept
/// separate because they are separate questions: a build could grow a window
/// without an input pump (a kiosk renderer), and the input modules should not
/// have to change when it does.
pub const fn has_os_input() -> bool {
    has_os_runtime()
}

/// `true` when the complete widget set is compiled in.
///
/// Reads the `build.rs` alias, so this stays correct for builds that select no
/// device profile at all (`--no-default-features --features gpu`), where
/// `not(any(mini, embedded))` would wrongly answer `true`.
pub const fn full_widget_set() -> bool {
    cfg!(full_widgets)
}

/// `true` when a reduced widget set is compiled in (`mini` or `embedded`).
pub const fn stripped_widget_set() -> bool {
    cfg!(stripped_widgets)
}

/// `true` when the widget set was *not* deliberately reduced.
///
/// This is the precise meaning of the `not(any(feature = "mini",
/// feature = "embedded"))` conjunction that was hand-written at 350+ call sites.
/// It is deliberately **not** the same as [`full_widget_set`]: a build that
/// selects no device profile (`--no-default-features --features gpu`) has an
/// unreduced widget set but no device transport, so `widgets_unstripped()` is
/// true while `full_widget_set()` is false. Collapsing the two would silently
/// change what such a build compiles (rule #47).
pub const fn widgets_unstripped() -> bool {
    cfg!(widgets_unstripped)
}

/// `true` when this build is the alloc-frugal `mini` profile.
///
/// The `mini` profile has no platform singleton and a much smaller memory budget.
/// This accessor exists so an upper layer that *legitimately* differs there asks a
/// semantic question instead of testing the feature name.
pub const fn is_alloc_frugal() -> bool {
    cfg!(alloc_frugal)
}

/// `true` when this build targets the `embedded` surface profile.
///
/// Distinct from [`stripped_widget_set`], which is also true under `mini`. A
/// caller that needs "embedded specifically" — for example when naming the
/// fallback platform — must not get `mini` folded in.
pub const fn is_embedded_surface() -> bool {
    cfg!(embedded_surface)
}

/// Human-readable profile name, reported by `RUST_WIDGETS_TRACE_RUNTIME`.
///
/// Kept as a single `match` on the typed facts rather than a fresh `cfg` tower,
/// so adding a profile means adding one arm instead of one more divergent
/// `cfg`-gated function.
pub const fn profile_name() -> &'static str {
    match engine_class() {
        EngineClass::OsHosted(ProfileClass::Device) => {
            if cfg!(feature = "desktop") {
                "desktop"
            } else if cfg!(feature = "tablet") {
                "tablet"
            } else {
                "mobile"
            }
        }
        EngineClass::SelfHosted(ProfileClass::Surface) => "embedded",
        EngineClass::SelfHosted(ProfileClass::Minimal) => "mini",
        // Not reachable through `engine_class()`: an OS-hosted build is always a
        // `Device`. Spelled out rather than using `_` so a future variant forces
        // this function to be revisited.
        EngineClass::OsHosted(_) | EngineClass::SelfHosted(ProfileClass::Device) => "unknown",
    }
}

/// Human-readable name of the mechanism that lands widgets.
///
/// Replaces the two `runtime_route_name()` overloads in `src/lib.rs`, which split
/// on the OS backend feature. The answer is now single-valued and deliberately so:
/// BLUE15 #55 requires the control-landing mechanism to be unique *and runtime
/// auditable*, so this reports the mechanism (`self-drawn`) rather than the runtime
/// question — a build with an OS backend still paints its own controls.
///
/// The runtime question is answered separately by [`has_os_runtime`] and reported
/// next to this in the `RUST_WIDGETS_TRACE_RUNTIME` line as `host=…`, so nothing is
/// lost by no longer overloading the word "route" with it.
///
/// The value also keeps mechanism vocabulary out of user-visible output: the
/// previous spelling was `native-platform`, which named the implementation instead
/// of what it does (principle #52).
pub const fn route_name() -> &'static str {
    "self-drawn"
}

/// Which loop drives this build, for the `RUST_WIDGETS_TRACE_RUNTIME` audit line.
///
/// `os-hosted` — the OS owns the window and pumps its own event loop (desktop,
/// tablet, mobile). `surface-only` — the library drives its own loop over a bare
/// drawing surface (`embedded`, `mini`, and any host without an OS backend).
///
/// Kept beside [`route_name`] because the two facts used to be one string; they are
/// independent, and a reader of the trace line needs both.
pub const fn host_name() -> &'static str {
    if has_os_runtime() {
        "os-hosted"
    } else {
        "surface-only"
    }
}

/// What this build's self-hosted runtime must provide.
///
/// # Why a table
///
/// `embedded` and `mini` differ in exactly two facts — whether an OS window
/// exists, and whether the alloc-frugal caps apply — but before this table those
/// two facts were re-derived at every call site that needed a budget: buffer size,
/// texture cap, font cache, event queue. Each derivation was a fresh chance to
/// disagree, and the four answers below were previously four separate `if`s in
/// `src/embedded/flags.rs` reading two unrelated atomic flags.
///
/// Encoding the whole policy once means a new profile is a new row here, and a
/// caller that needs a budget asks this table instead of testing a feature name
/// (rules #57/#58).
///
/// # Fields
///
/// * `os_window` — does an OS own a window this build paints into?
/// * `recommended_window` — default window size, in logical pixels.
/// * `max_widgets` — upper bound on simultaneously mounted controls.
/// * `max_texture` — largest square texture the surface may allocate, in pixels.
/// * `font_cache_bytes` — glyph atlas budget.
/// * `event_queue` — depth of the platform event queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfacePolicy {
    /// Whether an OS owns a window this build paints into.
    pub os_window: bool,
    /// Default window size, in logical pixels.
    pub recommended_window: (u32, u32),
    /// Upper bound on simultaneously mounted controls.
    pub max_widgets: usize,
    /// Largest square texture the surface may allocate, in pixels.
    pub max_texture: u32,
    /// Glyph atlas budget, in bytes.
    pub font_cache_bytes: usize,
    /// Depth of the platform event queue.
    pub event_queue: usize,
}

/// The resource policy for this build's profile.
///
/// The three arms correspond to the three `ProfileClass` values, so a new profile
/// cannot be added without deciding its budget — this `match` is exhaustive by
/// construction rather than by a comment asking the next person to remember.
///
/// A device profile keeps desktop-sized budgets: it has both an OS window and a
/// full allocator. `Surface` (a bare render surface, no OS runtime) sits in the
/// middle, and `Minimal` (`mini`) is deliberately frugal.
pub const fn surface_policy() -> SurfacePolicy {
    match profile_class() {
        ProfileClass::Device => SurfacePolicy {
            os_window: true,
            recommended_window: (1920, 1080),
            max_widgets: 4096,
            max_texture: 4096,
            font_cache_bytes: 2 * 1024 * 1024,
            event_queue: 256,
        },
        ProfileClass::Surface => SurfacePolicy {
            os_window: false,
            recommended_window: (1024, 768),
            max_widgets: 512,
            max_texture: 2048,
            font_cache_bytes: 1024 * 1024,
            event_queue: 128,
        },
        ProfileClass::Minimal => SurfacePolicy {
            os_window: false,
            recommended_window: (800, 600),
            max_widgets: 64,
            max_texture: 1024,
            font_cache_bytes: 256 * 1024,
            event_queue: 64,
        },
    }
}

/// Runtime profile category, for code that needs the `core` enum rather than
/// this module's typed facts.
pub const fn runtime_profile() -> RuntimeProfile {
    match engine_class() {
        EngineClass::OsHosted(_) => RuntimeProfile::Full,
        // `core::RuntimeProfile` has only two variants, so everything that is
        // not a full OS-hosted device reports the reduced profile.
        EngineClass::SelfHosted(_) => RuntimeProfile::Embedded,
    }
}

/// The render engine this profile drives its loop with.
///
/// This is the single selection point for the `embedded`-versus-native runtime
/// decision that `src/lib.rs` used to make with three separate `cfg`-gated
/// function pairs.
pub fn runtime_engine() -> Box<dyn RenderEngine> {
    crate::render_engine::default_render_engine()
}

/// Brings up the runtime this profile uses.
///
/// The `mini` profile has no platform singleton at all (`get_platform` does not
/// exist there), so the OS-hosted branch cannot be written as a runtime `if`:
/// the `cfg` must eliminate the call. Keeping that `cfg` **here** — and nowhere
/// else in the crate — is the whole point of this module (rules #57/#58).
pub fn runtime_init() {
    #[cfg(feature = "mini")]
    {
        log::info!("rust_widgets: mini mode init (no platform runtime)");
    }
    #[cfg(not(feature = "mini"))]
    {
        if has_os_runtime() {
            crate::platform::init();
        } else {
            // A surface-only build owns its loop.
            runtime_engine().init();
        }
    }
}

/// Runs the runtime's event loop.
pub fn runtime_run() {
    #[cfg(feature = "mini")]
    {
        log::info!("rust_widgets: mini mode run (no platform event loop)");
    }
    #[cfg(not(feature = "mini"))]
    {
        if has_os_runtime() {
            crate::platform::run();
        } else {
            runtime_engine().run();
        }
    }
}

/// Requests runtime shutdown.
pub fn runtime_quit() {
    #[cfg(feature = "mini")]
    {
        log::info!("rust_widgets: mini mode quit (no platform to shut down)");
    }
    #[cfg(not(feature = "mini"))]
    {
        if has_os_runtime() {
            crate::platform::quit();
        } else {
            runtime_engine().quit();
        }
    }
}

/// Initializes whatever optional subsystems this profile supports.
///
/// Replaces the four `init_i18n_runtime()` overloads: rather than a separate
/// `cfg`-gated function per profile, one function asks the compiled-in feature
/// flags. A profile without the `i18n` feature simply logs why it skipped.
pub fn init_optional_subsystems() {
    // `feature = "i18n"` is a *capability* feature, not a profile gate, so a
    // compile-time `cfg` on the call is still correct here — and necessary, since
    // `cfg!` cannot eliminate a path to a module that was not compiled.
    #[cfg(feature = "i18n")]
    if has_os_runtime() {
        crate::i18n::init();
        return;
    }

    log::debug!(
        "i18n init skipped in profile '{}' — {} ",
        profile_name(),
        if cfg!(feature = "i18n") {
            "no OS runtime to attach the translation catalogue to"
        } else {
            "the i18n feature is not enabled"
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two boolean questions must agree with the typed facts.
    #[test]
    fn capability_questions_match_the_engine_class() {
        assert_eq!(has_os_runtime(), matches!(engine_class(), EngineClass::OsHosted(_)));
        assert_eq!(has_os_input(), has_os_runtime());
    }

    /// The OpenHarmony predicate must agree with the target it claims to describe.
    ///
    /// `is_openharmony_target()` is the single place that knows how OpenHarmony is
    /// spelled at compile time. The fact it encodes is easy to get wrong and
    /// impossible to notice: every `*-unknown-linux-ohos` target reports
    /// `target_os = "linux"` and `target_env = "ohos"`, so a `cfg(target_os =
    /// "ohos")` test never matches and a backend selected by it silently falls
    /// through to the Linux arm. This pins the predicate to that one field.
    #[test]
    fn openharmony_predicate_tracks_the_target_env_field() {
        assert_eq!(
            is_openharmony_target(),
            cfg!(target_env = "ohos"),
            "is_openharmony_target() must report exactly whether this artifact targets OpenHarmony"
        );

        // The reason `target_env` is the discriminator and `target_os` is not —
        // OpenHarmony targets inherit the Linux OS name — is enforced at compile
        // time by the `const _` assertion right after `is_openharmony_target()`,
        // on every target rather than only inside an OpenHarmony branch.
    }

    /// The selected backend must be the one this target actually needs.
    ///
    /// This is the behavioural guard, phrased against a *runtime* fact rather
    /// than a `cfg`: on an OpenHarmony artifact the library must not be serving
    /// the GTK-only Linux backend, which cannot exist there. Before the
    /// `target_env` fix this assertion failed on `aarch64-unknown-linux-ohos`,
    /// because `create_native_platform` matched the `target_os = "linux"` arm.
    ///
    /// Gated off `mini` for the same reason `backend_name` itself is: that
    /// profile has no platform singleton, so there is no serving backend to ask.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_serving_backend_matches_the_target_family() {
        let name = crate::platform::backend_name();
        if is_openharmony_target() {
            assert!(
                name.starts_with("harmony"),
                "an OpenHarmony artifact must be served by the Harmony backend, not {name:?}"
            );
        } else {
            assert!(
                !name.starts_with("harmony") || cfg!(feature = "harmony"),
                "{name:?} claims to be the Harmony backend, but neither the target nor the \
                 `harmony` feature asked for it"
            );
        }
    }

    /// The widget-set aliases must be complementary whenever a device profile is
    /// selected, and both false when none is.
    #[test]
    fn widget_set_aliases_are_never_both_true() {
        assert!(!(full_widget_set() && stripped_widget_set()));
    }

    /// `mini` is the only alloc-frugal class, and it is never OS-hosted.
    #[test]
    fn minimal_device_is_surface_only() {
        if profile_class() == ProfileClass::Minimal {
            assert!(!has_os_runtime(), "mini must not claim an OS runtime");
            assert_eq!(profile_name(), "mini");
        }
    }

    /// Every profile must name itself; `"unknown"` on a device profile would mean
    /// `profile_name()` fell through its match.
    #[test]
    fn profile_name_is_never_unknown_for_a_device() {
        if profile_class() == ProfileClass::Device {
            assert_ne!(profile_name(), "unknown");
            assert_eq!(runtime_profile(), RuntimeProfile::Full);
        }
    }

    /// The route name must report the control-landing mechanism, which BLUE15 #55
    /// makes single-valued: every kind is painted by the library on every host.
    #[test]
    fn route_name_reports_the_single_landing_mechanism() {
        assert_eq!(route_name(), "self-drawn");
    }

    /// The runtime question must still be answerable, now under its own name.
    #[test]
    fn host_name_follows_the_runtime_question() {
        assert_eq!(host_name(), if has_os_runtime() { "os-hosted" } else { "surface-only" });
    }

    /// The policy table's `os_window` must agree with the runtime question.
    ///
    /// These are two descriptions of one fact. If they diverged, a build could
    /// report "no OS runtime" while budgeting for a window (or the reverse), and the
    /// budget would mislead every caller that sizes itself from it.
    #[test]
    fn policy_os_window_matches_the_runtime_question() {
        assert_eq!(
            surface_policy().os_window,
            has_os_runtime(),
            "surface_policy().os_window and has_os_runtime() describe the same fact",
        );
    }

    /// A restrained profile must not budget *more* than a fuller one.
    ///
    /// This is the property that makes the table worth having: the four budgets
    /// have to move together as the profile shrinks. A row edited in isolation
    /// (say, `Minimal` given a 4096 texture) would silently overshoot the device's
    /// memory and break the exact thing the profile exists to guarantee.
    #[test]
    fn each_budget_is_monotonic_as_the_profile_shrinks() {
        // The declared rows, loosest first. Kept as data rather than three
        // hand-written comparisons so the ordering itself is asserted.
        let rows = [
            SurfacePolicy {
                os_window: true,
                recommended_window: (1920, 1080),
                max_widgets: 4096,
                max_texture: 4096,
                font_cache_bytes: 2 * 1024 * 1024,
                event_queue: 256,
            },
            SurfacePolicy {
                os_window: false,
                recommended_window: (1024, 768),
                max_widgets: 512,
                max_texture: 2048,
                font_cache_bytes: 1024 * 1024,
                event_queue: 128,
            },
            SurfacePolicy {
                os_window: false,
                recommended_window: (800, 600),
                max_widgets: 64,
                max_texture: 1024,
                font_cache_bytes: 256 * 1024,
                event_queue: 64,
            },
        ];

        for pair in rows.windows(2) {
            let (looser, tighter) = (&pair[0], &pair[1]);
            assert!(
                tighter.max_widgets <= looser.max_widgets
                    && tighter.max_texture <= looser.max_texture
                    && tighter.font_cache_bytes <= looser.font_cache_bytes
                    && tighter.event_queue <= looser.event_queue,
                "a restrained profile must not budget more than a fuller one: \
                 {looser:?} then {tighter:?}",
            );
        }
    }

    /// The live table must match the row its own profile selects.
    ///
    /// The literals above are the specification; this asserts the implementation
    /// actually serves whichever row applies to the build under test.
    #[test]
    fn the_served_policy_is_one_of_the_declared_rows() {
        let policy = surface_policy();
        let declared = [
            (4096usize, 4096u32, 2 * 1024 * 1024, 256usize, true),
            (512, 2048, 1024 * 1024, 128, false),
            (64, 1024, 256 * 1024, 64, false),
        ];
        let tuple = (
            policy.max_widgets,
            policy.max_texture,
            policy.font_cache_bytes,
            policy.event_queue,
            policy.os_window,
        );
        assert!(
            declared.contains(&tuple),
            "surface_policy() returned {tuple:?}, which is not one of the declared rows"
        );
    }
}
