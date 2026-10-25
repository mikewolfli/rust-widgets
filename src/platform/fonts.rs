// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Process-wide snapshot of the host's installed fonts.
//!
//! # Why a snapshot
//!
//! [`Platform::system_fonts`](crate::platform::Platform::system_fonts) asks the operating system:
//! `NSFontManager`, `EnumFontFamiliesExW`, `fc-list`, `/system/fonts`. Those are not free, and a
//! font chooser asks for the list every time it is opened — and, with the C ABI below, once per
//! row when a host enumerates it. One process-wide snapshot makes an opened dialog and a host
//! enumerator read the same list without repeating the OS query per call.
//!
//! The set of installed fonts changes only when the host installs one, which is not a state this
//! library observes; a caller that has just installed a font can ask
//! [`refresh_system_fonts`] to drop the snapshot.
//!
//! One policy question lives here too, because it needs the snapshot: which installed family a host
//! should register for full CJK coverage — see [`register_system_cjk_font`].

use crate::compat::{OnceLock, RwLock, String, ToString, Vec};
use crate::platform::SystemFont;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// The current snapshot. `None` means "not queried yet".
///
/// A swappable slot rather than a plain `OnceLock<Vec<SystemFont>>`: `refresh_system_fonts`
/// promises to drop the snapshot, and the old cell could not be reset. The slice is `'static`
/// because that is the published return type, so each accepted snapshot is leaked once — see
/// [`refresh_system_fonts`] for why the leak is bounded by real changes rather than calls.
static SNAPSHOT: OnceLock<RwLock<Option<&'static [SystemFont]>>> = OnceLock::new();

/// Whether a background warm-up query is currently in flight.
static WARMING: AtomicBool = AtomicBool::new(false);

/// Revision of the backend's own font enumeration.
///
/// # Why this exists (P-05)
///
/// A backend may keep its own enumeration cache (the Unix backends cache `fc-list` inside
/// [`crate::platform::fonts_unix`]) so that a direct `Platform::system_fonts` call does not spawn
/// the enumerator every time. [`refresh_system_fonts`] is the **only** explicit invalidation path,
/// so it is what advances this revision; the backend compares it and re-enumerates when it moved.
///
/// It is bumped on every platform rather than only where a backend consumes it: the value is a
/// plain counter, and gating it per OS would put OS knowledge about the *consumer* into this
/// shared module.
static ENUMERATION_EPOCH: AtomicU64 = AtomicU64::new(0);

/// The current font-enumeration revision, for a backend's own cache.
///
/// Gated with its only consumer ([`crate::platform::fonts_unix`]): a platform whose backend has no
/// enumeration cache of its own must not compile a reader for a value nothing consumes.
#[cfg(all(unix_desktop, not(alloc_frugal)))]
pub(crate) fn enumeration_epoch() -> u64 {
    ENUMERATION_EPOCH.load(Ordering::SeqCst)
}

fn slot() -> &'static RwLock<Option<&'static [SystemFont]>> {
    SNAPSHOT.get_or_init(|| RwLock::new(None))
}

/// Runs `f` with the installed snapshot under a read lock.
///
/// The lock type differs by profile (`std::sync::RwLock` recovers from poisoning, `spin::RwLock`
/// has no `Result`), so the profile difference lives here rather than at every caller.
fn with_snapshot<R>(f: impl FnOnce(Option<&'static [SystemFont]>) -> R) -> R {
    #[cfg(not(alloc_frugal))]
    {
        let guard = slot().read().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(*guard)
    }
    #[cfg(alloc_frugal)]
    {
        f(*slot().read())
    }
}

/// Runs `f` with the snapshot slot under a write lock. See [`with_snapshot`].
fn with_snapshot_mut<R>(f: impl FnOnce(&mut Option<&'static [SystemFont]>) -> R) -> R {
    #[cfg(not(alloc_frugal))]
    {
        let mut guard = slot().write().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
    #[cfg(alloc_frugal)]
    {
        let mut guard = slot().write();
        f(&mut guard)
    }
}

/// Reads the current snapshot.
fn read_snapshot() -> Option<&'static [SystemFont]> {
    with_snapshot(|snapshot| snapshot)
}

/// The host's installed fonts, queried once per process.
pub fn system_fonts() -> &'static [SystemFont] {
    if let Some(fonts) = read_snapshot() {
        return fonts;
    }
    // Query outside the lock: the platform call can be slow (`fc-list`) and holding the slot
    // across it would block every reader. Two racing first calls both query; only one snapshot is
    // installed, so the other is a bounded, one-time allocation.
    let fresh: &'static [SystemFont] =
        Box::leak(crate::platform::platform_facts().system_fonts().into_boxed_slice());
    with_snapshot_mut(|slot| match *slot {
        Some(existing) => existing,
        None => {
            *slot = Some(fresh);
            fresh
        }
    })
}

/// The snapshot if it has already been queried, without touching the operating system.
pub fn cached_system_fonts() -> Option<&'static [SystemFont]> {
    read_snapshot()
}

/// The installed-font count already known to the process, or `None` when the snapshot is not ready.
pub fn cached_system_font_count() -> Option<usize> {
    read_snapshot().map(<[SystemFont]>::len)
}

/// Start populating the snapshot on a background thread, if that has not happened yet.
///
/// Enumerating system fonts (`fc-list` on Unix, the platform font store on macOS/Windows) can take
/// hundreds of milliseconds. Running it on the UI thread at first use froze the caller; this moves
/// the one-time cost off it. Safe to call repeatedly: an installed snapshot or an in-flight query
/// makes it a no-op.
pub fn prewarm_system_fonts() {
    if read_snapshot().is_some() {
        return;
    }
    if WARMING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("system-fonts".to_string()).spawn(|| {
        system_fonts();
        WARMING.store(false, Ordering::SeqCst);
    });
    if let Err(e) = spawned {
        WARMING.store(false, Ordering::SeqCst);
        log::warn!("[fonts] failed to start the system-font warm-up worker: {e}");
    }
}

/// The installed-font count without ever blocking on the OS query.
///
/// Returns `0` and starts the background warm-up when the snapshot is not ready yet. A host that
/// sees `0` for that reason should retry later; this is the non-blocking counterpart of
/// [`system_fonts`].
pub fn system_font_count_nonblocking() -> usize {
    match cached_system_font_count() {
        Some(count) => count,
        None => {
            prewarm_system_fonts();
            0
        }
    }
}

/// Drops the snapshot so the next [`system_fonts`] call queries the OS again.
///
/// # Why the leak is bounded
///
/// The return type is a process-lifetime slice, so an accepted snapshot cannot be freed while a
/// reader may hold it. The previous implementation leaked a **new** snapshot on every call and
/// never installed it, so the global readers were not refreshed at all and repeated calls grew
/// the process without bound. This version installs what it returns and compares first: a refresh
/// that produces the same font set returns the installed slice instead of leaking another copy.
/// Memory therefore grows with the number of **real** font-set changes, not with calls.
pub fn refresh_system_fonts() -> &'static [SystemFont] {
    // Advance the revision *before* asking the platform: a backend that caches its own enumeration
    // compares the value, so this is what makes the call below reach the operating system rather
    // than answer from that cache (P-05).
    ENUMERATION_EPOCH.fetch_add(1, Ordering::SeqCst);
    let fresh = crate::platform::platform_facts().system_fonts();
    with_snapshot_mut(|slot| {
        if let Some(existing) = *slot {
            if existing == fresh.as_slice() {
                return existing;
            }
        }
        let leaked: &'static [SystemFont] = Box::leak(fresh.into_boxed_slice());
        *slot = Some(leaked);
        leaked
    })
}

/// Sorted, de-duplicated family names from [`system_fonts`].
pub fn families() -> Vec<String> {
    let mut families: Vec<String> = system_fonts()
        .iter()
        .map(|font| font.family.trim().to_string())
        .filter(|family| !family.is_empty())
        .collect();
    families.sort_unstable_by_key(|family| family.to_lowercase());
    families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    families
}

/// The families compiled into this build, in active-face order, sorted and de-duplicated.
///
/// This is the fallback for a backend that reports no installed fonts (Harmony, WASM,
/// Redox) and for a host preview with no system font store: those families are exactly the
/// ones [`face_for_family`](crate::render::text::font_assets::face_for_family) can draw.
#[cfg(feature = "text-shaping")]
pub fn bundled_families() -> Vec<String> {
    let mut families: Vec<String> = crate::render::text::font_assets::active_faces()
        .iter()
        .map(|face| face.name.trim().to_string())
        .filter(|family| !family.is_empty())
        .collect();
    families.sort_unstable_by_key(|family| family.to_lowercase());
    families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    families
}

/// The families a font chooser should offer.
///
/// Uses the host's installed families when the backend reports any, and the bundled families
/// otherwise — an empty chooser on a backend with no system font store would hide the faces the
/// build can still draw.
pub fn chooser_families() -> Vec<String> {
    let system = families();
    if !system.is_empty() {
        return system;
    }
    #[cfg(feature = "text-shaping")]
    {
        bundled_families()
    }
    #[cfg(not(feature = "text-shaping"))]
    {
        Vec::new()
    }
}

/// Registers the readable font file backing `family`, when the host reported one.
///
/// [`register_face`](crate::render::text::register_face) needs `'static` bytes, so the file is
/// read once and leaked: a registered face lives for the rest of the process by contract, and the
/// registry holds a small fixed number of them. Returns `false` when the family is not in the
/// snapshot, the backend reported no file (AppKit and GDI report families only), the file cannot
/// be read, or the bytes do not parse.
#[cfg(all(feature = "runtime-fonts", not(alloc_frugal)))]
pub fn register_family(family: &str) -> bool {
    // Re-registering would read and leak the file again, so an already-registered family is a
    // success without touching the disk. `active_runtime_faces` is the registry's own view, which
    // keeps this check from keeping a second list that could disagree with it.
    let (faces, len) = crate::render::text::runtime_fonts::active_runtime_faces();
    if faces[..len].iter().any(|face| face.name.eq_ignore_ascii_case(family.trim())) {
        return true;
    }
    // The registry is fixed-capacity, so a full registry cannot accept another face. Checking
    // before reading matters: the bytes are leaked on the success path, and a read that is
    // guaranteed to be refused would cost a disk hit and permanent memory for nothing.
    if crate::render::text::registered_face_count() >= crate::render::text::MAX_RUNTIME_FACES {
        return false;
    }
    let Some(font) =
        system_fonts().iter().find(|font| font.family.eq_ignore_ascii_case(family.trim()))
    else {
        return false;
    };
    let Some(path) = font.source.as_ref() else {
        return false;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    // `register_face` keys by the name it is given and keeps that name for the process lifetime,
    // so the leaked spellings are the registry's own storage rather than a per-call copy.
    let name: &'static str = Box::leak(font.family.clone().into_boxed_str());
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    crate::render::text::register_face(name, bytes)
}

/// The family names a desktop host most often ships for Simplified Chinese UI text.
///
/// Ordered from the face a UI would choose first to the broad fallback. The list is **data**:
/// each backend's [`system_fonts`] reports which of these are actually installed, so nothing here
/// branches on the operating system. It exists because the compiled CJK face is a *subset* of
/// U+4E00..U+9FFF (`tools/cjk_vector_codepoints.txt`), and a host whose UI text goes beyond that
/// subset has to name the installed family itself.
#[cfg(all(feature = "runtime-fonts", not(alloc_frugal)))]
pub const SYSTEM_CJK_FAMILIES: &[&str] = &[
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    "WenQuanYi Micro Hei",
    "Droid Sans Fallback",
];

/// Registers the first installed family from [`SYSTEM_CJK_FAMILIES`] and returns its name.
///
/// Returns `None` when none of the candidates is installed (or the backend reports names without
/// readable files, as AppKit does). A `None` is worth reporting rather than ignoring: the compiled
/// face covers only part of the CJK block, so without an installed face the rest renders as tofu.
///
/// # Two consequences worth knowing
///
/// * **A registered face is consulted first**, for every character it covers — that is
///   [`register_face`](crate::render::text::register_face)'s documented precedence. A CJK family
///   also carries Latin glyphs, so the UI's Latin text will be drawn (and measured) by this family
///   rather than by the compiled Latin face. That is one consistent typography choice, not a bug,
///   but it is a visible change and the reason this is an opt-in call rather than an automatic
///   fallback.
/// * **A font collection resolves to its first face.** The registered bytes are a `.ttc` on most
///   hosts, and the renderer parses index 0. For Noto Sans CJK that is the JP face: it covers the
///   same codepoints, so nothing is tofu, but a few glyphs take the JP default form. Selecting a
///   sub-face would need a face index in `FaceBytes` and at every parse site, which is a larger
///   change than this entry point; until then, a single-face family (Droid Sans Fallback) is the
///   choice for a host that needs SC defaults exactly.
///
/// # Why this is a selector, not a second registration path
///
/// The reading, leaking, and registering is [`register_family`]'s job; this function only decides
/// *which* family to ask for. Keeping one implementation means a fix to the file-reading or
/// replacement rules cannot leave this caller behind.
#[cfg(all(feature = "runtime-fonts", not(alloc_frugal)))]
pub fn register_system_cjk_font() -> Option<String> {
    SYSTEM_CJK_FAMILIES.iter().find(|family| register_family(family)).map(|family| (*family).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refresh that finds the same font set must reuse the installed slice.
    ///
    /// This pins the fix for the unbounded leak: every call used to hand out a newly leaked
    /// snapshot that `system_fonts()` never saw. Pointer equality is the observable that says
    /// "the second refresh accepted the existing snapshot instead of allocating another".
    #[test]
    fn a_refresh_with_no_change_reuses_the_installed_snapshot() {
        let first = refresh_system_fonts();
        let second = refresh_system_fonts();
        assert!(core::ptr::eq(first, second), "an unchanged refresh leaked a second snapshot");
        assert_eq!(second, system_fonts(), "the refresh must install what it returns");
    }

    #[test]
    fn families_are_sorted_and_have_no_blank_or_duplicate_entries() {
        let families = families();
        assert!(families.windows(2).all(|pair| pair[0].to_lowercase() <= pair[1].to_lowercase()));
        assert!(families.iter().all(|family| !family.trim().is_empty()));
        let mut unique = families.clone();
        unique.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        assert_eq!(unique.len(), families.len());
    }

    #[cfg(all(feature = "runtime-fonts", not(alloc_frugal)))]
    #[test]
    fn an_unknown_family_is_not_registered() {
        assert!(!register_family("This Family Does Not Exist 12345"));
    }

    /// Whatever this host has, a reported registration must be visible in the face registry: a
    /// `Some("X")` that did not put `X` in the list would be exactly the "reported success for
    /// something that did not happen" failure the project rules forbid. `None` is a legitimate
    /// answer on a host with no CJK face, so the assertion is conditional on the return value.
    ///
    /// The second half is the end-to-end check the demo depends on: a Han character the compiled
    /// subset does not carry (U+7A97 窗 is past the subset's `U+4E00..=U+51E7` range) must resolve
    /// to the face just registered, not to the subset face and not to tofu.
    #[cfg(all(feature = "runtime-fonts", not(alloc_frugal)))]
    #[test]
    fn a_reported_cjk_registration_is_actually_in_the_registry() {
        // The registry is process-wide and `cargo test` runs tests in parallel; the same lock the
        // registry's own tests take keeps this sequence from interleaving with a `clear_faces`.
        let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
        let registered = register_system_cjk_font();
        let Some(family) = registered else {
            // No installed CJK face on this host: the refusal path is the whole contract here.
            return;
        };
        let (faces, len) = crate::render::text::runtime_fonts::active_runtime_faces();
        assert!(
            faces[..len].iter().any(|face| face.name.eq_ignore_ascii_case(&family)),
            "reported `{family}` but the registry holds {:?}",
            faces[..len].iter().map(|face| face.name).collect::<Vec<_>>()
        );

        // Outside the compiled subset, so only a registered face can answer.
        let face = crate::render::text::font_assets::face_for_char('窗')
            .expect("a registered CJK face must cover U+7A97");
        assert!(
            face.name.eq_ignore_ascii_case(&family),
            "U+7A97 resolved to `{}`, not the registered `{family}`",
            face.name
        );

        // Leave the process as this test found it. The registry is global, and a face left behind
        // would answer for every later test that asks which face covers a Han character (two of
        // `raster`'s tests assert exactly that).
        crate::render::text::clear_faces();
    }

    #[cfg(feature = "text-shaping")]
    #[test]
    fn bundled_families_are_sorted_and_blank_free() {
        let families = bundled_families();
        assert!(families.windows(2).all(|pair| pair[0].to_lowercase() <= pair[1].to_lowercase()));
        assert!(families.iter().all(|family| !family.trim().is_empty()));
    }
}
