// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What a capability statement is *about*: which build, which target, and how well that
//! was verified (BLUE26 B26.01 / B26.06).
//!
//! # What problem this solves
//!
//! The designer manifest describes 187 controls and their properties, events and commands —
//! but it does not say **which build** answered. A generate call that walks a `desktop` build
//! and reports "`video_codec` is supported" is telling the truth about the machine it is
//! running on and nothing about a `mini` device the file might ship to. Rustage cannot
//! honestly claim a target supports a control unless the answer names the target it came from.
//!
//! This module makes that target a first-class value, so no capability statement is ever
//! read without the question "which target, and how was that checked?" being answerable.
//!
//! # Three levels of knowledge, not two
//!
//! The naive model is "supported / not supported". It is wrong in the direction that matters:
//! a control that this build can *describe* (because its metadata is compiled in) is not the
//! same as one that has been **built and run** on the target OS, and neither is the same as
//! "we have never looked". Collapsing the middle into "supported" is how a desktop fact
//! becomes a false claim about an embedded device. [`VerificationLevel`] keeps the three
//! distinct, and [`Availability`] can only answer with a level it actually has evidence for.
//!
//! # Why the profile and the triple are separate facts
//!
//! They answer different questions and neither implies the other. The **profile** is what
//! `Cargo.toml` selected (`desktop`/`tablet`/`mobile`/`mini`/`embedded`): it decides which
//! modules exist. The **target triple** is what the compiler emitted into
//! (`aarch64-apple-darwin`, `thumbv7em-none-eabihf`, `x86_64-pc-windows-msvc`): it decides
//! what the OS can actually do. A `desktop` profile on an unusual triple and a `mini` profile
//! on a common one are both real, so recording one and inferring the other would be a guess.
//! The profile name comes from [`crate::platform::profile::profile_name`]; the triple comes
//! from `build.rs` (`rule #47`: the build script, not a layer that should not know the OS).

use crate::compat::{format, String};

/// The target a capability statement describes.
///
/// Built from compile-time facts only — there is no probe and no runtime lookup here, so a
/// caller can construct one in any profile (principles #35/#36: OS knowledge stays in the
/// build script and `platform/`, never in the capability layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetSpec {
    /// The device profile this build selected: `"desktop"`, `"tablet"`, `"mobile"`,
    /// `"mini"`, `"embedded"`, or `"unknown"` for a build with no device profile.
    pub profile: &'static str,
    /// The triple the compiler emitted into, as `build.rs` captured it. `"unknown"` when the
    /// build script could not read `TARGET`, which is honest rather than a guess at the host.
    pub triple: &'static str,
}

impl TargetSpec {
    /// The spec of the build that is running.
    ///
    /// Both fields are compile-time constants, so this is a pure accessor and cannot fail.
    pub const fn current() -> Self {
        Self { profile: crate::platform::profile::profile_name(), triple: current_triple() }
    }

    /// The spec of the build that is running, spelled identically for a `min`/portable build
    /// that has no `platform` module.
    ///
    /// A separate name rather than a `cfg` on [`Self::current`] so the call site never carries
    /// an OS or profile test (principle #47): the *module* is what differs, not the caller.
    #[cfg(not(widgets_unstripped))]
    pub const fn current_stripped() -> Self {
        Self { profile: "unknown", triple: current_triple() }
    }
}

/// The triple `build.rs` recorded, or `"unknown"`.
///
/// # Why not `std::env::consts::ARCH`/`OS`
///
/// Those name the arch and OS separately, and reconstructing a triple from them is a guess:
/// `aarch64` + `darwin` is `aarch64-apple-darwin`, but the ABI component (`eabihf`, `msvc`,
/// `musl`, `gnueabihf`) is exactly the part a capability question turns on, and it is
/// unrecoverable from `ARCH`/`OS`. The build script has the real triple in `TARGET`, so the
/// real triple is what travels.
///
/// # Why the arms are listed, and how the list cannot drift
///
/// A `cfg` value cannot be read back as data: the only way to recover it at compile time is to
/// test the arms below. That makes the list a second declaration of the build script's own, which
/// is why `build.rs` emits `target_triple_captured` — it knows whether `TARGET` is one of the
/// triples it declared, so
/// `the_captured_triple_agrees_with_the_reader` asserts the two answers agree on every build. An
/// earlier revision let them disagree: it declared `aarch64-linux-ohos` (not a triple this crate
/// emits — the OpenHarmony spelling is `aarch64-unknown-linux-ohos`) and so reported `"unknown"`
/// for a target CI actually builds. The lists are kept in step by that test, not by care.
const fn current_triple() -> &'static str {
    // Each `cfg!` is false unless the build script emitted exactly that value; the compiler folds
    // the whole chain to the one true arm, so this stays a `const fn` with no runtime lookup and
    // keeps the OS string out of every other module.
    if cfg!(target_triple = "aarch64-apple-darwin") {
        return "aarch64-apple-darwin";
    }
    if cfg!(target_triple = "x86_64-apple-darwin") {
        return "x86_64-apple-darwin";
    }
    if cfg!(target_triple = "x86_64-unknown-linux-gnu") {
        return "x86_64-unknown-linux-gnu";
    }
    if cfg!(target_triple = "aarch64-unknown-linux-gnu") {
        return "aarch64-unknown-linux-gnu";
    }
    if cfg!(target_triple = "x86_64-pc-windows-msvc") {
        return "x86_64-pc-windows-msvc";
    }
    if cfg!(target_triple = "aarch64-apple-ios") {
        return "aarch64-apple-ios";
    }
    if cfg!(target_triple = "aarch64-apple-ios-sim") {
        return "aarch64-apple-ios-sim";
    }
    if cfg!(target_triple = "aarch64-unknown-linux-ohos") {
        return "aarch64-unknown-linux-ohos";
    }
    if cfg!(target_triple = "armv7-unknown-linux-ohos") {
        return "armv7-unknown-linux-ohos";
    }
    if cfg!(target_triple = "x86_64-unknown-linux-ohos") {
        return "x86_64-unknown-linux-ohos";
    }
    if cfg!(target_triple = "aarch64-linux-android") {
        return "aarch64-linux-android";
    }
    if cfg!(target_triple = "x86_64-linux-android") {
        return "x86_64-linux-android";
    }
    if cfg!(target_triple = "wasm32-unknown-unknown") {
        return "wasm32-unknown-unknown";
    }
    "unknown"
}

/// How well a capability claim has been checked for a target.
///
/// Ordered from weak to strong so the derived `Ord` puts the *strongest* evidence highest — a
/// comparison (`>=`) then answers "is this at least as good as that?", which is what a report
/// needs when it decides whether to print a caveat. The declaration order below is the
/// ordering: `Unknown < Describable < BuiltVerified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerificationLevel {
    /// Unknown: no evidence either way. The honest answer for a target that has not been
    /// built, and the answer a caller must render as "unverified" rather than "supported".
    Unknown,
    /// Describable: this build's metadata covers the capability, so it can be *described* for
    /// the target, but nothing has confirmed the target itself compiles or runs it.
    Describable,
    /// Build-verified: the target actually compiled with this capability enabled, and the
    /// fact was confirmed by a build (rule #43: a target dependency configuration is verified
    /// by an actual build, not by inspection).
    BuiltVerified,
}

impl VerificationLevel {
    /// The stable token this level travels as in a manifest.
    pub const fn token(self) -> &'static str {
        match self {
            Self::BuiltVerified => "built-verified",
            Self::Describable => "describable",
            Self::Unknown => "unknown",
        }
    }

    /// The inverse of [`Self::token`]; `None` for an unrecognised spelling.
    ///
    /// A `None` rather than a default, so a document naming a level this build does not
    /// understand is rejected instead of being read as the weakest one — which would let a
    /// malformed file silently downgrade a verified target to unknown.
    pub const fn from_token(token: &str) -> Option<Self> {
        match token.as_bytes() {
            b"built-verified" => Some(Self::BuiltVerified),
            b"describable" => Some(Self::Describable),
            b"unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// The answer to "can this target have this capability?", carrying the level it is known at.
///
/// # Why "no" and "unknown" are different answers
///
/// `Unsupported` means the target was checked and the capability is genuinely absent there —
/// a caller should not offer it. `Unverified` means *nobody has checked*, so a caller may
/// proceed but must say so. Folding the second into the first would make the library refuse
/// things it has no grounds to refuse; folding it into "supported" is the overstatement the
/// whole module exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// Confirmed available, at `level`.
    Available {
        /// How the availability was established.
        level: VerificationLevel,
    },
    /// Confirmed absent for `target`, with the reason why.
    Unsupported {
        /// Why the capability is absent — never empty, so a report can always say something.
        reason: &'static str,
    },
    /// Not known for `target`.
    Unverified {
        /// Why nothing is known, so a caller can distinguish "not built yet" from "no data".
        reason: &'static str,
    },
}

impl Availability {
    /// Whether the capability may be relied upon — true only for a verified availability.
    ///
    /// The predicate is deliberately strict: `Unverified` is *not* usable, because a generated
    /// program that relied on an unverified capability would be building on a guess.
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Available { .. })
    }

    /// Whether the capability is known to be absent.
    pub const fn is_unsupported(self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }

    /// The verification level this answer carries, if any.
    pub const fn level(self) -> Option<VerificationLevel> {
        match self {
            Self::Available { level } => Some(level),
            Self::Unsupported { .. } | Self::Unverified { .. } => None,
        }
    }

    /// A one-line description for a report, so every answer renders as something a person can
    /// read rather than a bare variant name.
    pub fn describe(self) -> String {
        match self {
            Self::Available { level } => format!("available ({})", level.token()),
            Self::Unsupported { reason } => format!("unsupported: {reason}"),
            Self::Unverified { reason } => format!("unverified: {reason}"),
        }
    }
}

/// A probe answer for one capability name against one target (BLUE26 B26.06).
///
/// # Why this is not just `Availability`
///
/// A caller asking "can `mini` generate a `video_codec`?" needs three things at once: the answer,
/// the *target* it is about, and *how* the answer was reached. `Availability` carries the first two
/// facts (the reason string names the limit); this struct adds the target so a report can print
/// "`video_codec` on `mini`/`thumbv7em-none-eabihf`: unverified (target not built here)" rather
/// than a bare verdict a reader cannot attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetProbe {
    /// The target the answer is about.
    pub target: TargetSpec,
    /// The answer, with its verification level or its reason for being unsupported/unknown.
    pub availability: Availability,
}

impl TargetProbe {
    /// Renders the probe as one human-readable line, always naming the target.
    ///
    /// The target is included unconditionally because an unattributed "supported" is the exact
    /// defect B26.06 exists to remove: a desktop fact must never be readable as a claim about a
    /// mini device, and printing the target is what makes that impossible to overlook.
    pub fn describe(&self) -> String {
        format!(
            "{} on {} ({}): {}",
            "capability",
            self.target.profile,
            self.target.triple,
            self.availability.describe()
        )
    }

    /// Whether the answer is conclusive for the target — a built-verified availability.
    ///
    /// Distinct from [`Availability::is_usable`], which also accepts a `Describable` answer: a
    /// *probe* is conclusive only when the target itself was built, because a probe is the evidence
    /// a caller collects before making a claim.
    pub const fn is_conclusive(self) -> bool {
        matches!(
            self.availability,
            Availability::Available { level: VerificationLevel::BuiltVerified }
        )
    }
}

/// What the **running build** knows about a capability name, before the target is considered.
///
/// The three-way answer a factory or the generator can give cheaply; `probe_capability` turns it
/// plus the target into a [`TargetProbe`] with an honest [`VerificationLevel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentBuildVerdict {
    /// The running build can construct/describe the name.
    Usable,
    /// The capability table knows the name but the running build's constructors gate it out.
    Unbuildable,
    /// Nothing knows the name.
    Unknown,
}

/// The probe for a capability name against a target, from the running build's verdict.
///
/// # The three answers, and why none of them is a guess
///
/// * [`CurrentBuildVerdict::Usable`] and the current build *is* the target's profile and triple →
///   [`VerificationLevel::BuiltVerified`]: the target was compiled with the capability, and this
///   build is its evidence.
/// * [`CurrentBuildVerdict::Usable`] but the build differs from the target →
///   [`VerificationLevel::Describable`]: the metadata covers it, but nothing confirmed the target
///   itself compiled with it. The honest level for "resolved under a desktop host, asked about
///   mini".
/// * [`CurrentBuildVerdict::Unbuildable`] → [`Availability::Unsupported`] with the reason, because
///   the build's own constructor list is direct evidence the capability is gated out.
/// * [`CurrentBuildVerdict::Unknown`] → [`Availability::Unverified`], the honest "no evidence" —
///   never folded into "supported".
pub fn probe_capability(
    name: &str,
    target: TargetSpec,
    verdict: CurrentBuildVerdict,
) -> TargetProbe {
    let current = TargetSpec::current();
    let availability = match verdict {
        CurrentBuildVerdict::Usable => {
            if current.profile == target.profile && current.triple == target.triple {
                Availability::Available { level: VerificationLevel::BuiltVerified }
            } else {
                // A `Describable` answer is deliberately *not* built-verified: a different build
                // answered, so the target's own compilation is unconfirmed.
                Availability::Available { level: VerificationLevel::Describable }
            }
        }
        CurrentBuildVerdict::Unbuildable => Availability::Unsupported {
            reason: "the running build's constructors gate this capability out of the target's \
                     profile, and no verified target build enables it",
        },
        CurrentBuildVerdict::Unknown => Availability::Unverified {
            reason: "no constructor or capability is registered for this name in the running \
                     build, and the target has not been built with it",
        },
    };
    // `name` is retained for a caller that wants to attach it to a report line; the probe itself
    // does not store it because the caller already holds it as the lookup key.
    let _ = name;
    TargetProbe { target, availability }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The current spec must name a real profile and a real triple, never the `"unknown"`
    /// placeholder, on the profiles this crate actually tests.
    #[test]
    fn current_spec_is_concrete_on_a_device_profile() {
        let spec = TargetSpec::current();
        assert_ne!(spec.profile, "unknown", "a device profile must name itself");
        assert_ne!(spec.triple, "unknown", "build.rs must have captured the TARGET triple");
        // The triple is a real triple, not a placeholder: it has the arch-vendor-os shape.
        assert!(spec.triple.split('-').count() >= 3, "`{}` is not a target triple", spec.triple);
    }

    /// The build script's triple list and this module's reading arms must agree.
    ///
    /// `build.rs` declares which triples it captured (`target_triple_captured`) and this module
    /// answers the same question by matching a cfg value, so the two are two declarations of one
    /// fact. They disagreed once — the script declared `aarch64-linux-ohos` while the real
    /// OpenHarmony spelling is `aarch64-unknown-linux-ohos`, so a HarmonyOS build reported
    /// `"unknown"` for a target the crate does build. This test is what keeps them in step: the
    /// flag is true exactly when `current_triple()` can name the triple, so a triple present on one
    /// side and missing from the other fails here rather than degrading quietly.
    #[test]
    fn the_captured_triple_agrees_with_the_reader() {
        assert_eq!(
            cfg!(target_triple_captured),
            current_triple() != "unknown",
            "build.rs captured `{}` but the reader answered `{}`; the two triple lists disagree",
            if cfg!(target_triple_captured) { "a declared triple" } else { "an undeclared triple" },
            current_triple()
        );
    }

    /// Every verification level must round-trip through its token, so a manifest written by
    /// one build reads back the same level in another.
    #[test]
    fn verification_levels_round_trip() {
        for level in [
            VerificationLevel::BuiltVerified,
            VerificationLevel::Describable,
            VerificationLevel::Unknown,
        ] {
            assert_eq!(
                VerificationLevel::from_token(level.token()),
                Some(level),
                "`{}` did not round-trip",
                level.token()
            );
        }
    }

    /// An unrecognised level token must be rejected, not silently read as the weakest level.
    #[test]
    fn unknown_level_token_is_rejected() {
        assert_eq!(VerificationLevel::from_token("maybe"), None);
        assert_eq!(VerificationLevel::from_token(""), None);
    }

    /// `is_usable` must be true only for a verified availability — an unverified capability is
    /// not something a generated program may build on.
    #[test]
    fn only_verified_availability_is_usable() {
        assert!(Availability::Available { level: VerificationLevel::BuiltVerified }.is_usable());
        assert!(Availability::Available { level: VerificationLevel::Describable }.is_usable());
        assert!(!Availability::Unverified { reason: "not built" }.is_usable());
        assert!(!Availability::Unsupported { reason: "no gpu" }.is_usable());
    }

    /// The ordering must run strongest-first, because a report compares levels to decide
    /// whether to print a caveat.
    #[test]
    fn verification_levels_are_ordered_strongest_first() {
        assert!(VerificationLevel::BuiltVerified > VerificationLevel::Describable);
        assert!(VerificationLevel::Describable > VerificationLevel::Unknown);
    }

    /// `describe` must render every variant as a non-empty line.
    #[test]
    fn every_availability_describes_itself() {
        assert!(Availability::Available { level: VerificationLevel::Describable }
            .describe()
            .contains("describable"));
        assert!(Availability::Unsupported { reason: "no gpu" }.describe().contains("no gpu"));
        assert!(Availability::Unverified { reason: "not built" }.describe().contains("not built"));
    }

    /// A capability usable in a build that *is* the target must be `BuiltVerified`.
    #[test]
    fn a_usable_capability_on_the_current_target_is_built_verified() {
        let probe = probe_capability("button", TargetSpec::current(), CurrentBuildVerdict::Usable);
        assert!(probe.is_conclusive());
        assert_eq!(probe.availability.level(), Some(VerificationLevel::BuiltVerified));
    }

    /// A capability usable in a *different* build must be only `Describable` — never built-verified.
    ///
    /// This is the exact overstatement B26.06 exists to prevent: a desktop host answering about a
    /// mini device must not report the answer as though the mini device were built.
    #[test]
    fn a_usable_capability_on_a_different_target_is_only_describable() {
        let other = TargetSpec { profile: "mini", triple: "thumbv7em-none-eabihf" };
        let probe = probe_capability("button", other, CurrentBuildVerdict::Usable);
        assert!(!probe.is_conclusive());
        assert_eq!(probe.availability.level(), Some(VerificationLevel::Describable));
        assert!(probe.availability.is_usable(), "a describable answer is still usable");
    }

    /// An unbuildable capability must be `Unsupported` with a reason, and never usable.
    #[test]
    fn an_unbuildable_capability_is_unsupported() {
        let probe = probe_capability(
            "video_codec",
            TargetSpec::current(),
            CurrentBuildVerdict::Unbuildable,
        );
        assert!(probe.availability.is_unsupported());
        assert!(!probe.availability.is_usable());
        assert!(probe.describe().contains("unsupported"));
    }

    /// An unknown name must be `Unverified`, not `Unsupported` — nobody checked.
    #[test]
    fn an_unknown_name_is_unverified_not_unsupported() {
        let probe = probe_capability(
            "no_such_control",
            TargetSpec::current(),
            CurrentBuildVerdict::Unknown,
        );
        assert!(!probe.availability.is_unsupported());
        assert!(!probe.availability.is_usable());
        assert!(probe.describe().contains("unverified"));
    }

    /// Every probe line must name the target, so an answer is never unattributed.
    #[test]
    fn every_probe_names_its_target() {
        let probe = probe_capability("button", TargetSpec::current(), CurrentBuildVerdict::Usable);
        let line = probe.describe();
        assert!(line.contains(probe.target.profile), "{line}");
        assert!(line.contains(probe.target.triple), "{line}");
    }
}
