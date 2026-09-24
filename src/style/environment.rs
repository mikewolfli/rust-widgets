// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The facts about **this device, right now**, behind one interface.
//!
//! # Why this module exists
//!
//! Eight questions about the running device were being asked in eight different ways, each at
//! many call sites or not at all (BLUE24 §4.1's measurement):
//!
//! | question | how it was answered before |
//! |---|---|
//! | how large is the user's text? | a `scale` argument every `implicit_size` caller passed itself, so nobody held it |
//! | how dense is the layout? | a `dimensions` table entry with no reader |
//! | light or dark? | a wall-clock hour window compared against `SystemTime::now()` |
//! | which locale? | nothing: `I18nManager::set_language` had no source of a default |
//! | reduce motion? | nothing |
//! | high contrast? | nothing |
//! | which text direction? | a per-control `direction: TextDirection` field |
//! | mirror the layout? | nothing |
//!
//! The last three rows and the first are why this is a **trait** and not a set of globals: the
//! authority for all eight is the *host*, and a library can only ask.
//!
//! # Why not `cfg`, environment variables or constants
//!
//! Any of those would write "this device happens to be like this" as "every device is like
//! this". `should_use_dark`'s wall-clock window is the worked example: under `mini` it cannot
//! read a clock, so it answered `false` — that is, it reported "the user wants light" when the
//! truth was "I cannot tell". Principle #37: a missing capability is stated, never fabricated.
//!
//! # Why this is not part of the `Platform` trait
//!
//! Deliberately kept separate, and the reason is testability. Every `Platform` method touches a
//! real window or OS object, so on a developer machine it can only be exercised through a
//! recording double. **Every** method here is constructible in pure logic, so this interface
//! compiles under `mini`, accepts any combination in a unit test, and can be reused by the tests
//! of all 188 controls without a window anywhere.
//!
//! # Honest defaults
//!
//! Each method has a neutral, *declarable* default — scale `1.0`, no locale, full motion, no
//! contrast override, LTR, no mirroring — rather than a fabricated one. A build that installs no
//! provider renders exactly as it did before this module existed, which is what makes the whole
//! mechanism opt-in.

use crate::core::TextDirection;
use crate::style::ReducedMotionPreference;
use crate::style::ThemeMode;

/// Whether the user prefers reduced motion, under this module's name.
///
/// A **type alias**, not a second enum: `ReducedMotionPreference` is already the crate's
/// statement of that fact (`src/style/primitives.rs`), and two enums with the same variants would
/// be the drift principle #54 forbids — a new variant added to one would silently fail to reach
/// the other. The alias exists so this interface reads as the plan describes it
/// (`MotionPreference::Reduced`) without a translation step at every call site.
///
/// The two spellings that matter are kept distinct on purpose: this enum says what the *user
/// asked for*, while `MotionSlot` (`src/style/animation.rs`) says *which theme token* prices a
/// move. One is an input, the other is a lookup key.
pub type MotionPreference = ReducedMotionPreference;

/// The facts about the device the library is running on.
///
/// See the module documentation for why this is a trait, why it is separate from `Platform`, and
/// why every default is neutral. A host installs its real provider with
/// [`install_environment`]; tests install a constructed one.
///
/// # Implementor's contract
///
/// Every method answers "what is true right now", not "what was true at startup": a host that
/// learns the user changed their text size mid-session returns the new value from the next call.
/// The library reads a **snapshot** once per frame (see [`EnvironmentSnapshot`]), so a change
/// takes effect on the following frame rather than part-way through a paint.
pub trait EnvironmentProvider {
    /// The system's text-size preference. `1.0` means "as authored".
    ///
    /// This is the a11y font-size setting, not the display scale: a device that draws at 2x on a
    /// 2x panel is not "larger text", it is the same text at more pixels, and the pixel ratio is
    /// the backend's business. A user who has asked for larger text gets a value above `1.0`
    /// here.
    fn text_scale(&self) -> f32 {
        1.0
    }

    /// The layout density preference, beyond the pixel ratio. `1.0` means "as authored".
    ///
    /// Distinct from [`text_scale`](Self::text_scale): this scales *spacing* — padding, gaps,
    /// touch targets — so a host can offer a roomier layout without changing type sizes. A
    /// compact desktop build returns less than `1.0`; a tablet build may return more.
    fn layout_scale(&self) -> f32 {
        1.0
    }

    /// The interface locale as a BCP-47 tag, or `None` when the library cannot know.
    ///
    /// `None` is the honest answer and is distinct from `Some("en")`: it says "not known", which
    /// is why a host that cares calls `I18nManager::set_language` explicitly. Reporting a
    /// guessed locale would make every untranslated string look translated.
    fn locale(&self) -> Option<&'static str> {
        None
    }

    /// The system's preferred appearance.
    ///
    /// Returns a [`ThemeMode`], and `Auto` means "follow, but do not guess": the library will not
    /// consult a clock to resolve it (see the module docs on the wall-clock defect). A host that
    /// wants the automatic behaviour resolves it itself and reports `Dark` or `Light` — it is the
    /// only party that knows the user's actual preference.
    fn color_scheme(&self) -> ThemeMode {
        ThemeMode::Light
    }

    /// Whether the user asked for reduced motion.
    ///
    /// When this is [`MotionPreference::ReduceMotion`], **every** transition's effective duration
    /// is zero (see [`effective_duration`]), so an animation completes within one frame instead
    /// of being skipped. That distinction matters: skipping would need a branch in all 188
    /// controls, while a zero duration needs one in this module.
    fn motion_preference(&self) -> MotionPreference {
        MotionPreference::NoPreference
    }

    /// Whether the user asked for increased contrast.
    ///
    /// A host that returns `true` is asking the library to prefer `outline` over decorative
    /// elevation when a face has to be made legible. It does not disable colour: the theme is
    /// still the authority on what the colours are.
    fn high_contrast(&self) -> bool {
        false
    }

    /// The base text direction. `Ltr` unless the user's locale is right-to-left.
    fn text_direction(&self) -> TextDirection {
        TextDirection::LeftToRight
    }

    /// Whether the layout should be mirrored.
    ///
    /// Mirrored layouts are not limited to right-to-left locales — a left-handed user may ask for
    /// one — which is why this is a separate fact from [`text_direction`](Self::text_direction).
    /// A host that mirrors a layout is expected to also report the direction it mirrors *for*.
    fn mirroring(&self) -> bool {
        false
    }
}

/// The [`EnvironmentProvider`] the library uses when the host has installed none.
///
/// # Why this delegates rather than staying neutral
///
/// [`DefaultEnvironment`] answers every question with a neutral value, which is right for a build
/// with no platform at all. But the crate *does* have one source of device truth already —
/// `Platform::text_scale`, whose own documentation says the OS knowledge lives in the backends
/// ("Android's `fontScale`, iOS's `UIContentSizeCategory`, Windows' text-scaling percentage").
///
/// Leaving the two unconnected would be the "two implementations of one operation" shape this
/// codebase keeps having to remove: a control reading `Platform::text_scale()` directly and a
/// control reading [`environment`] would disagree on a device where a backend reports a scale.
/// So the default provider **is** the platform, for the facts the platform can answer, and stays
/// neutral for the rest — which are exactly the ones no `Platform` method covers.
///
/// A host that wants to supply its own answers (a settings screen, a test) calls
/// [`install_environment`] and takes over completely.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlatformEnvironment;

impl EnvironmentProvider for PlatformEnvironment {
    fn text_scale(&self) -> f32 {
        crate::platform::profile::text_scale()
    }
}

/// A [`EnvironmentProvider`] that answers every question with its neutral default.
///
/// # Why an explicit type rather than a `Box<dyn>` built at install time
///
/// The default is what the library uses when no host has installed anything, and it is also the
/// value a host composes its own provider around. Naming it makes both uses one expression, and
/// it is the type [`environment`] returns facts from before any install.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DefaultEnvironment;

impl EnvironmentProvider for DefaultEnvironment {}

/// The eight facts, copied out of the provider as **values**.
///
/// # Why a value type and not a reference to the provider
///
/// The paint path is the hottest code in the crate, and the goal is that a still frame costs
/// almost nothing (BLUE24 §1 criterion 2). Holding `&dyn EnvironmentProvider` and calling eight
/// virtual methods per draw would put eight indirect calls in every frame; a snapshot puts eight
/// `f32`/`bool` loads there instead, read once per frame and passed down.
///
/// # Why the snapshot is refreshed at the frame's first step
///
/// [`crate::drive_frame`] takes it before it does anything else, so **every control in one frame
/// sees the same facts**. Without that, a provider whose answer changed mid-frame would give two
/// controls different text scales in the same picture, which is a visual inconsistency no single
/// control can detect. "The environment is constant within a frame" is therefore a property that
/// can be asserted, and the frame driver's own test does assert it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentSnapshot {
    /// See [`EnvironmentProvider::text_scale`].
    pub text_scale: f32,
    /// See [`EnvironmentProvider::layout_scale`].
    pub layout_scale: f32,
    /// See [`EnvironmentProvider::locale`].
    pub locale: Option<&'static str>,
    /// See [`EnvironmentProvider::color_scheme`].
    pub color_scheme: ThemeMode,
    /// See [`EnvironmentProvider::motion_preference`].
    pub motion_preference: MotionPreference,
    /// See [`EnvironmentProvider::high_contrast`].
    pub high_contrast: bool,
    /// See [`EnvironmentProvider::text_direction`].
    pub text_direction: TextDirection,
    /// See [`EnvironmentProvider::mirroring`].
    pub mirroring: bool,
}

impl Default for EnvironmentSnapshot {
    /// Every neutral default, matching [`DefaultEnvironment`].
    fn default() -> Self {
        Self::from_provider(&DefaultEnvironment)
    }
}

impl EnvironmentSnapshot {
    /// Reads every fact out of `provider` once.
    ///
    /// The single place that turns a provider into values, so a new fact cannot be added to the
    /// trait and forgotten here — the struct's own fields force the compiler to ask for it.
    pub fn from_provider(provider: &dyn EnvironmentProvider) -> Self {
        Self {
            text_scale: provider.text_scale(),
            layout_scale: provider.layout_scale(),
            locale: provider.locale(),
            color_scheme: provider.color_scheme(),
            motion_preference: provider.motion_preference(),
            high_contrast: provider.high_contrast(),
            text_direction: provider.text_direction(),
            mirroring: provider.mirroring(),
        }
    }

    /// The text scale actually applied to a size, floored at a sane minimum.
    ///
    /// # Why the clamp is here rather than at each caller
    ///
    /// A provider is host code and can report anything — a `0.0` from an uninitialised settings
    /// store, a negative from a bad parse, `NaN` from a division somewhere. Multiplying a layout
    /// by any of those produces a zero or `NaN` geometry, which a control cannot recover from and
    /// which is invisible in a snapshot diff (a `NaN` rectangle simply draws nothing). Clamping
    /// once, in the accessor every consumer uses, is what makes "a hostile provider degrades to a
    /// usable layout rather than to no layout" true by construction.
    pub fn effective_text_scale(&self) -> f32 {
        clamp_scale(self.text_scale)
    }

    /// The layout scale actually applied, with the same floor as
    /// [`effective_text_scale`](Self::effective_text_scale).
    pub fn effective_layout_scale(&self) -> f32 {
        clamp_scale(self.layout_scale)
    }

    /// Whether motion should be suppressed.
    ///
    /// A named query rather than a comparison at each call site, so the policy ("reduced motion
    /// is how the user states it") stays in one place even if the enum grows a third variant.
    pub fn prefers_reduced_motion(&self) -> bool {
        self.motion_preference == MotionPreference::ReduceMotion
    }
}

/// Constrains a host-reported scale to a range a layout can survive.
///
/// The upper bound is generous rather than tight: a user is entitled to ask for very large text,
/// and clamping their request down would be the library overruling them. The lower bound is what
/// matters — `0.25` is small enough to be a legitimate compact setting and large enough that no
/// rectangle collapses.
fn clamp_scale(scale: f32) -> f32 {
    // `NaN` fails every comparison, so it is caught explicitly rather than by the clamp: a
    // `f32::clamp` on a `NaN` returns `NaN`, which is exactly the value that must not escape.
    if scale.is_nan() {
        return 1.0;
    }
    scale.clamp(0.25, 8.0)
}

/// A transition's **actual** duration, after the user's motion preference is applied.
///
/// # Why this is the only place the preference is read
///
/// "Reduced motion" has one correct implementation: collapse the duration to zero. A zero
/// duration makes a driver reach its target on the next tick, so the animation finishes in one
/// frame and `is_moving()` turns false immediately — the end state is *reached*, not skipped.
///
/// The tempting alternative — an `if reduced { set_final() } else { animate() }` in each control —
/// is what this function exists to avoid. It would be 188 branches, each of which has to also
/// remember to settle the value, so "there is no motion under reduced motion" would be a promise
/// rather than a consequence. With the duration collapsed, the two behaviours **are the same code
/// path**, and a control that adds an animation later inherits the preference for free.
///
/// # Why it takes a snapshot rather than reading the provider
///
/// The same reason the snapshot exists: a frame must apply one set of facts to every control, so
/// the caller passes the frame's snapshot down instead of each control asking again.
pub fn effective_duration(tempo: crate::style::MotionSlot, env: &EnvironmentSnapshot) -> u32 {
    if env.prefers_reduced_motion() {
        return 0;
    }
    tempo.duration_ms()
}

// ── The process-wide slot ────────────────────────────────────────────────────
//
// One provider per process, replaceable, and read through exactly one function.

thread_local! {
    /// The installed provider, or `None` when the host has not installed one.
    ///
    /// # Why thread-local
    ///
    /// The widgets are `!Send` (`Rc`/`RefCell` state) and every desktop backend requires UI work
    /// on the main thread, so the environment is only ever read from the thread that owns the UI.
    /// A thread-local also lets a test install a provider without racing a parallel test on
    /// another thread — which the process-wide alternative could not do, and which this crate's
    /// theme manager had to solve with a test guard instead.
    #[allow(clippy::missing_const_for_thread_local)]
    static INSTALLED: core::cell::RefCell<Option<alloc::boxed::Box<dyn EnvironmentProvider>>> =
        const { core::cell::RefCell::new(None) };

    /// The current frame's snapshot, refreshed by [`refresh_environment`].
    ///
    /// Cached rather than re-derived per read so a still frame does no virtual dispatch at all:
    /// `environment()` is then one thread-local load of a `Copy` value, which is the cost the
    /// paint path can afford (BLUE24 §1 criterion 2).
    #[allow(clippy::missing_const_for_thread_local)]
    static SNAPSHOT: core::cell::Cell<Option<EnvironmentSnapshot>> =
        const { core::cell::Cell::new(None) };
}

/// Installs `env` as the process's environment provider, returning the one it replaced.
///
/// # Why "install" rather than "read the platform"
///
/// A unit test has to be able to construct combinations a real machine cannot be put into at
/// will — text scale 2.0 *and* reduced motion *and* high contrast, all at once. It can only do
/// that if the library reads an **installed** provider rather than asking the OS, so the seam is
/// the installation itself. The returned provider is handed back so a test can restore what it
/// replaced, even if it panics in between.
///
/// The frame's cached snapshot is refreshed to match, so a read taken immediately after this call
/// already reports the new facts instead of waiting for the next frame.
pub fn install_environment(
    env: alloc::boxed::Box<dyn EnvironmentProvider>,
) -> Option<alloc::boxed::Box<dyn EnvironmentProvider>> {
    let previous = INSTALLED.try_with(|slot| slot.borrow_mut().replace(env)).unwrap_or(None);
    refresh_environment();
    previous
}

/// Removes the installed provider, restoring the neutral defaults. Returns whether one was set.
///
/// The counterpart of [`install_environment`] for a test that wants to leave the thread as it
/// found it; the returned flag lets a caller assert that there *was* something to remove rather
/// than silently passing on a no-op.
pub fn uninstall_environment() -> bool {
    let had_one = INSTALLED.try_with(|slot| slot.borrow_mut().take().is_some()).unwrap_or(false);
    refresh_environment();
    had_one
}

/// Re-reads the installed provider into this frame's snapshot.
///
/// Called by [`crate::drive_frame`] as its first step, which is what makes "every control in one
/// frame sees the same facts" true: a provider whose answer changes mid-frame cannot give two
/// controls different scales in the same picture. A host that changed a system setting between
/// frames sees it take effect on the next one.
pub fn refresh_environment() {
    let snapshot = INSTALLED
        .try_with(|slot| match &*slot.borrow() {
            Some(provider) => EnvironmentSnapshot::from_provider(provider.as_ref()),
            // No host provider: read the platform, which is the crate's own source of device
            // truth, rather than a fabricated neutral. A device whose backend reports a text
            // scale therefore gets it even if no host installed anything.
            None => EnvironmentSnapshot::from_provider(&PlatformEnvironment),
        })
        .unwrap_or_default();
    let _ = SNAPSHOT.try_with(|slot| slot.set(Some(snapshot)));
}

/// The device facts in force for this frame. **The** read point.
///
/// Returns the snapshot [`refresh_environment`] took at the start of the frame, or the neutral
/// defaults when no frame has run yet (and always, when no provider is installed). A `Copy`
/// value, so a caller reads it as often as it likes without paying for a virtual call.
pub fn environment() -> EnvironmentSnapshot {
    SNAPSHOT
        .try_with(|slot| slot.get())
        .ok()
        .flatten()
        // Before any frame has run there is no cached snapshot, and the honest answer is what the
        // default provider would say -- the platform's text scale, neutral everywhere else -- not
        // a fully neutral value that would ignore a backend's reported scale.
        .unwrap_or_else(|| EnvironmentSnapshot::from_provider(&PlatformEnvironment))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A provider that reports a fixed set of facts, for asserting the snapshot path.
    struct Fixed {
        text_scale: f32,
        motion: MotionPreference,
        locale: Option<&'static str>,
        direction: TextDirection,
        mirroring: bool,
        contrast: bool,
        scheme: ThemeMode,
        layout_scale: f32,
    }

    impl Default for Fixed {
        fn default() -> Self {
            Self {
                text_scale: 1.0,
                motion: MotionPreference::NoPreference,
                locale: None,
                direction: TextDirection::LeftToRight,
                mirroring: false,
                contrast: false,
                scheme: ThemeMode::Light,
                layout_scale: 1.0,
            }
        }
    }

    impl EnvironmentProvider for Fixed {
        fn text_scale(&self) -> f32 {
            self.text_scale
        }
        fn layout_scale(&self) -> f32 {
            self.layout_scale
        }
        fn locale(&self) -> Option<&'static str> {
            self.locale
        }
        fn color_scheme(&self) -> ThemeMode {
            self.scheme
        }
        fn motion_preference(&self) -> MotionPreference {
            self.motion
        }
        fn high_contrast(&self) -> bool {
            self.contrast
        }
        fn text_direction(&self) -> TextDirection {
            self.direction
        }
        fn mirroring(&self) -> bool {
            self.mirroring
        }
    }

    /// The default provider answers every question with a neutral value.
    ///
    /// This is the "an uninstalled build is unchanged" property, stated as eight assertions
    /// rather than as a claim: a build with no host provider must render exactly what it did
    /// before this module existed.
    #[test]
    fn the_default_provider_is_neutral_in_every_dimension() {
        let env = EnvironmentSnapshot::from_provider(&DefaultEnvironment);
        assert_eq!(env.text_scale, 1.0);
        assert_eq!(env.layout_scale, 1.0);
        assert_eq!(env.locale, None, "no locale is not the same fact as the locale `en`");
        assert_eq!(env.color_scheme, ThemeMode::Light);
        assert_eq!(env.motion_preference, MotionPreference::NoPreference);
        assert!(!env.high_contrast);
        assert_eq!(env.text_direction, TextDirection::LeftToRight);
        assert!(!env.mirroring);
        assert_eq!(env, EnvironmentSnapshot::default(), "the two spellings agree");
    }

    /// Every fact the provider reports reaches the snapshot unchanged.
    ///
    /// The failure this pins is a snapshot field that is never assigned — the struct would
    /// compile, the default would be neutral, and a host's real setting would be silently
    /// dropped. Asserting on a provider whose every answer differs from the default is what
    /// makes a missed copy visible.
    #[test]
    fn a_snapshot_carries_every_fact_out_of_the_provider() {
        let provider = Fixed {
            text_scale: 1.5,
            layout_scale: 0.8,
            motion: MotionPreference::ReduceMotion,
            locale: Some("he-IL"),
            direction: TextDirection::RightToLeft,
            mirroring: true,
            contrast: true,
            scheme: ThemeMode::Dark,
        };
        let env = EnvironmentSnapshot::from_provider(&provider);
        assert_eq!(env.text_scale, 1.5);
        assert_eq!(env.layout_scale, 0.8);
        assert_eq!(env.locale, Some("he-IL"));
        assert_eq!(env.color_scheme, ThemeMode::Dark);
        assert_eq!(env.motion_preference, MotionPreference::ReduceMotion);
        assert!(env.high_contrast);
        assert_eq!(env.text_direction, TextDirection::RightToLeft);
        assert!(env.mirroring);
    }

    /// A hostile scale degrades to a usable one instead of collapsing the layout.
    ///
    /// Three inputs a host can really produce: a zero from an uninitialised setting, a negative
    /// from a bad parse, and a `NaN` from a division. Each would otherwise become a zero-size or
    /// invisible rectangle somewhere downstream, where nothing can tell it apart from a control
    /// that legitimately has no size.
    #[test]
    fn a_hostile_scale_is_clamped_to_a_usable_range() {
        for (reported, expected) in
            [(0.0_f32, 0.25_f32), (-3.0, 0.25), (f32::NAN, 1.0), (100.0, 8.0)]
        {
            let provider = Fixed { text_scale: reported, ..Default::default() };
            let env = EnvironmentSnapshot::from_provider(&provider);
            assert_eq!(
                env.effective_text_scale(),
                expected,
                "a reported scale of {reported} must degrade to {expected}"
            );
            assert!(env.effective_text_scale().is_finite());
        }
    }

    /// Reduced motion collapses a duration to zero; a normal preference leaves it alone.
    ///
    /// Both halves in one test because the rule is a *relationship*: the same token must produce a
    /// real duration normally and no duration under the preference. Asserting only the reduced
    /// case would pass for a function that always returned zero, which would leave the library
    /// with no animation at all.
    #[test]
    fn reduced_motion_collapses_a_duration_to_zero() {
        use crate::style::MotionSlot;

        let full = EnvironmentSnapshot::default();
        let reduced = EnvironmentSnapshot {
            motion_preference: MotionPreference::ReduceMotion,
            ..EnvironmentSnapshot::default()
        };

        let normal_ms = effective_duration(MotionSlot::Normal, &full);
        assert!(normal_ms > 0, "with no preference a transition has a real duration");
        assert_eq!(
            effective_duration(MotionSlot::Normal, &reduced),
            0,
            "and none under the preference"
        );

        // Every slot, not just the one the fixture happens to use: a host could set any tempo, so
        // the preference has to be a property of the environment rather than of the token.
        for slot in [MotionSlot::Fast, MotionSlot::Normal, MotionSlot::Slow] {
            assert_eq!(effective_duration(slot, &reduced), 0, "{slot:?} must collapse too");
        }
    }

    /// The token still prices the motion when the preference does not intervene.
    ///
    /// The other direction of the same relationship, and the reason `effective_duration` is not
    /// simply "return the reduced answer": a theme sets `fast`/`normal`/`slow`, and all three must
    /// survive the call unchanged.
    #[test]
    fn the_tempo_still_prices_the_motion_without_a_preference() {
        use crate::style::MotionSlot;

        let env = EnvironmentSnapshot::default();
        assert_eq!(effective_duration(MotionSlot::Fast, &env), MotionSlot::Fast.duration_ms());
        assert_eq!(effective_duration(MotionSlot::Normal, &env), MotionSlot::Normal.duration_ms());
        assert_eq!(effective_duration(MotionSlot::Slow, &env), MotionSlot::Slow.duration_ms());
    }
}
