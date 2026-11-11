// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n global - global functions and static instances
//!
//! # Reachability
//!
//! **`tr!`:** live. The macro resolves at construction time and is used by built-in strings
//! (`src/widget/dialog/file_dialog.rs` in the widget layer, plus the JSON loader's declarative
//! path), so a catalogue change is only visible to labels built afterwards unless the widget
//! registered a retranslator.
//!
//! **`register_retranslator` / `retranslate`:** live. `FileDialog` registers a retranslator
//! (`src/widget/dialog/file_dialog.rs`): a language switch bumps the generation the dialog stores,
//! and its `draw` re-resolves the title and button labels from the new catalogue (RT2-03). A host
//! that stores translated text in its own control registers its own callback the same way.
//! [`set_language`] therefore reaches both the catalogue for subsequent lookups and the built-in
//! dialog's already-built text.
use crate::compat::{lock, MiniToString, Mutex, MutexGuard, OnceLock, String, Vec};
use crate::i18n::manager::I18nManager;
use crate::i18n::options::{InitOptions, InitReport};
use crate::i18n::types::{LanguageChanged, TranslationFile};
use crate::signal::Signal;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
/// Embedded English translations as a compile-time fallback.
const EMBEDDED_EN_JSON: &str = include_str!("../../language/en.json");
/// Global i18n manager instance using Mutex for thread-safe access
pub(crate) static GLOBAL_I18N: Mutex<Option<I18nManager>> = Mutex::new(None);
/// Bumped whenever the catalogue or the active language changes.
///
/// A control that caches a resolved label keys its cache on this number: the label is a pure
/// function of (key, language, catalogue), so a newer generation means every cached resolution
/// is stale. It is a counter rather than a lock so the paint path can read it for free.
static LANGUAGE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// The current catalogue/language generation.
///
/// See `LANGUAGE_GENERATION`; controls that cache translated text compare this before reusing
/// a label built under an older catalogue.
pub fn language_generation() -> u64 {
    LANGUAGE_GENERATION.load(Ordering::Relaxed)
}

fn bump_language_generation() {
    LANGUAGE_GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Switch the active language at runtime.
///
/// Returns whether the language actually changed. The manager's [`LanguageChanged`] signal is
/// emitted after the process-wide i18n lock is released, so a listener that calls [`translate`]
/// from its slot cannot deadlock on the non-reentrant lock.
///
/// This is the entry point a language picker should call: it changes the catalogue used by every
/// subsequent `tr!` lookup and notifies listeners that already-built labels are stale. A widget
/// that stored the result of `tr!` should either re-resolve on the signal or resolve at draw time;
/// no built-in control registers a retranslator, so this call does not re-resolve stored text by
/// itself (see the module-level Reachability note).
pub fn set_language(language: &str) -> bool {
    let (changed, signal, event) = {
        let mut guard = get_manager();
        if !ensure_initialized(&mut guard) {
            return false;
        }
        let Some(manager) = guard.as_mut() else {
            return false;
        };
        let previous = manager.current_language().clone();
        let changed = previous != language;
        manager.set_language_quiet(language);
        let event = LanguageChanged { previous, current: language.to_string() };
        (changed, manager.on_language_changed().clone(), event)
    };
    if !changed {
        return false;
    }
    bump_language_generation();
    // EX4-03: the language tag also decides the base reading direction. Writing the library
    // default here is the one place a language switch can mirror every layout that did not pin
    // its own direction; a container that called `set_text_direction` keeps its explicit value.
    crate::core::set_default_text_direction(crate::core::TextDirection::for_language(language));
    signal.emit(event);
    arm_repaint_for_language_change();
    // A language switch is exactly the event `register_retranslator` exists for: a control that
    // stored translated text resolves it again here. The lock is already released, so a callback
    // may call `translate` (RT2-03).
    run_retranslators();
    true
}

/// The process-wide language-change signal.
///
/// The manager is initialised on first use (with the embedded English catalogue) so a caller can
/// subscribe before deciding on a language. A host that replaces the manager through
/// [`init_with_options`] must re-acquire this signal afterwards: it is the new manager's signal,
/// and subscriptions made against the previous one are not carried across an explicit
/// re-initialisation.
pub fn on_language_changed() -> Signal<LanguageChanged> {
    let mut guard = get_manager();
    if !ensure_initialized(&mut guard) {
        return Signal::new();
    }
    guard.as_ref().map(|manager| manager.on_language_changed().clone()).unwrap_or_default()
}

/// Requests that every label built from `tr!` be resolved again.
///
/// This is for a catalogue edit that reloaded while the language stayed the same. It emits the
/// same [`LanguageChanged`] event as a real switch (`previous == current`) and then runs every
/// callback registered through [`register_retranslator`]. The i18n lock is released before any
/// callback runs, so a callback may call [`translate`] safely.
pub fn retranslate() {
    let event = {
        let mut guard = get_manager();
        if !ensure_initialized(&mut guard) {
            None
        } else {
            guard.as_ref().map(|manager| {
                let language = manager.current_language().clone();
                (
                    manager.on_language_changed().clone(),
                    LanguageChanged { previous: language.clone(), current: language },
                )
            })
        }
    };
    if let Some((signal, event)) = event {
        // Emitted after the global i18n lock is released, so a slot may call `translate`.
        signal.emit(event);
    }
    bump_language_generation();
    arm_repaint_for_language_change();
    run_retranslators();
}

/// A callback run by [`retranslate`] after the language-change signal.
///
/// This is the mechanism a widget uses when it stores translated text: the signal reports that
/// the text is stale, and the retranslator re-resolves it into the widget's own field. The
/// callback runs with no i18n lock held and must schedule its own redraw through the normal
/// widget path (or rely on [`retranslate`]'s global repaint request).
pub fn register_retranslator<F>(callback: F) -> RetranslatorHandle
where
    F: Fn() + Send + Sync + 'static,
{
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    lock(retranslators()).push(Retranslator { id, callback: Arc::new(callback) });
    RetranslatorHandle { id }
}

/// Handle for a registered retranslator; dropping it unregisters the callback.
#[derive(Debug)]
pub struct RetranslatorHandle {
    id: u64,
}

impl Drop for RetranslatorHandle {
    fn drop(&mut self) {
        lock(retranslators()).retain(|entry| entry.id != self.id);
    }
}

struct Retranslator {
    id: u64,
    callback: Arc<dyn Fn() + Send + Sync>,
}

fn retranslators() -> &'static Mutex<Vec<Retranslator>> {
    static RETRANSLATORS: OnceLock<Mutex<Vec<Retranslator>>> = OnceLock::new();
    RETRANSLATORS.get_or_init(|| Mutex::new(Vec::new()))
}

fn run_retranslators() {
    // `retranslators()` creates the registry on first use; an empty registry costs one lock and
    // no allocation beyond the (empty) snapshot.
    let registry = retranslators();
    // Snapshot the callbacks and release the lock first: a retranslator may register or drop
    // another one, and running user code under the registry lock would deadlock that.
    let callbacks: Vec<Arc<dyn Fn() + Send + Sync>> =
        lock(registry).iter().map(|entry| Arc::clone(&entry.callback)).collect();
    for callback in callbacks {
        callback();
    }
}

fn arm_repaint_for_language_change() {
    #[cfg(device_profile)]
    crate::widget::runtime::arm_global_repaint();
}

#[cfg(test)]
std::thread_local! {
    /// Explicit [`init`] calls made by the **current test thread**.
    ///
    /// A process-global assertion cannot prove "this call did not initialise i18n":
    /// [`translate`] lazily populates `GLOBAL_I18N` by design, so any test that renders a
    /// translated label concurrently puts a manager in the slot, and the `is_none()` assertion
    /// fails for a reason unrelated to the switch under test. A per-thread counter observes only
    /// the calls the test's own thread made.
    static INIT_CALLS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// How many times [`init`] ran on the calling thread (test-only).
#[cfg(test)]
pub(crate) fn init_call_count() -> usize {
    INIT_CALLS.with(core::cell::Cell::get)
}

/// Drops the global manager so the next lookup re-initialises from the embedded catalogue.
///
/// # Why this exists
///
/// `init()` is idempotent by contract: it builds the default catalogue **if none exists** and
/// otherwise leaves a host-installed manager alone. Several tests used it as a “reset”, so a
/// test that installed a custom catalogue could leak it into a later test (observed as
/// `calendar.month.1` / `common.button.cancel` translating to their keys in a parallel run).
/// Test isolation needs the literal reset; production code keeps `init()`'s idempotence.
#[cfg(test)]
pub(crate) fn reset_for_tests() {
    *crate::compat::lock(&GLOBAL_I18N) = None;
    LANGUAGE_GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Initialize the i18n system, loading the embedded English translations.
pub fn init() {
    #[cfg(test)]
    INIT_CALLS.with(|calls| calls.set(calls.get() + 1));
    let mut guard = crate::compat::lock(&GLOBAL_I18N);
    ensure_initialized(&mut guard);
}

/// Builds the embedded catalogue into `slot` when it is empty.
///
/// # Why this is a helper and not four lines inside `init`
///
/// [`translate`] calls it too, and the contract it enforces is the reason: **a label
/// lookup never returns a raw key**. A host that forgot `init` used to get
/// `dialog.font.select_font` painted inside the dialog — every `tr!` call site
/// showed its identifier, which reads as a broken label, not as a missing catalogue
/// entry. Loading the compiled-in English catalogue on first use makes the default
/// build correct with no setup, and an explicit `init_with_options` still replaces
/// it (including the active language) when the host supplies real translations.
///
/// Returns `false` only when the embedded JSON cannot be parsed, which is a build
/// error rather than a runtime condition; callers then leave the key untouched.
fn ensure_initialized(slot: &mut Option<I18nManager>) -> bool {
    if slot.is_some() {
        return true;
    }
    let Ok(translation_file) = serde_json::from_str::<TranslationFile>(EMBEDDED_EN_JSON) else {
        log::error!("[i18n] Failed to parse embedded en.json");
        return false;
    };
    let mut manager = I18nManager::new();
    let language = translation_file.language.clone();
    manager.set_language(&language);
    manager.inject_translations(language, translation_file);
    log::info!(
        "[i18n] Loaded embedded English translations ({} messages)",
        manager.translation_count()
    );
    *slot = Some(manager);
    true
}
/// Initialize the i18n system with options
pub fn init_with_options(options: InitOptions) -> InitReport {
    let mut report = InitReport::new();
    let mut manager = I18nManager::new();
    manager.set_language(&options.language);
    let diagnostics = options.diagnostics;

    // Embedded English is the compile-time fallback that `init` also loads. It
    // must be available through this entry point too, so a locale that only
    // partially translates still falls back to the source language. Files loaded
    // below override it when they declare the same language, so file-provided
    // translations keep their priority.
    match serde_json::from_str::<TranslationFile>(EMBEDDED_EN_JSON) {
        Ok(translation_file) => {
            let language = translation_file.language.clone();
            manager.inject_translations(language, translation_file);
        }
        Err(e) => {
            report.errors.push(format!("Failed to parse embedded en.json: {e}"));
        }
    }

    // Load translations from directory if specified
    if let Some(dir) = options.preload_dir {
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(e) => {
                            report.errors.push(format!(
                                "Failed to read an entry in translation directory '{dir}': {e}"
                            ));
                            continue;
                        }
                    };
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json") {
                        let path_str = match path.to_str() {
                            Some(p) => p,
                            None => {
                                report.errors.push(format!("Non-UTF-8 path: {path:?}"));
                                continue;
                            }
                        };
                        match manager.load_translations(path_str) {
                            Ok(()) => {
                                report.files_loaded += 1;
                                if diagnostics {
                                    log::info!("[i18n] Loaded translations from: {path:?}");
                                }
                            }
                            Err(e) => {
                                report.errors.push(format!("Failed to load {path:?}: {e}"));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                report.errors.push(format!("Translation directory '{dir}' could not be read: {e}"));
            }
        }
    }

    report.translations_count = manager.translation_count();
    let mut guard = crate::compat::lock(&GLOBAL_I18N);
    *guard = Some(manager);
    report
}
/// Translate a key to the current language
pub fn translate(key: &str) -> String {
    let mut out = String::new();
    translate_into(key, &mut out);
    out
}
/// Translate a key into `out` without allocating.
///
/// The zero-allocation counterpart of [`translate`]: the resolved message is appended to the
/// caller's buffer. `translate` itself must allocate because its signature returns `String`;
/// a draw loop that reuses one buffer should call this instead.
pub fn translate_into(key: &str, out: &mut String) {
    let mut guard = crate::compat::lock(&GLOBAL_I18N);
    if !ensure_initialized(&mut guard) {
        out.push_str(key);
        return;
    }
    match *guard {
        Some(ref mut manager) => manager.translate_into(key, out),
        None => out.push_str(key),
    }
}
/// Translate a key with context and plural count into `out` without allocating.
pub fn translate_into_with_context(key: &str, context: Option<&str>, count: u32, out: &mut String) {
    let mut guard = crate::compat::lock(&GLOBAL_I18N);
    if !ensure_initialized(&mut guard) {
        out.push_str(key);
        return;
    }
    match *guard {
        Some(ref mut manager) => manager.translate_with_context_into(key, context, count, out),
        None => out.push_str(key),
    }
}
/// Translate a key with optional context and plural count
pub fn translate_with_context(key: &str, context: Option<&str>, count: u32) -> String {
    let mut guard = crate::compat::lock(&GLOBAL_I18N);
    if !ensure_initialized(&mut guard) {
        return key.to_string();
    }
    match *guard {
        Some(ref mut manager) => manager.translate_with_context(key, context, count),
        None => key.to_string(),
    }
}

/// Translate a key and fill its `{name}` placeholders.
///
/// The global counterpart of [`I18nManager::translate_with_args`]; the substitution rule is
/// [`crate::i18n::interpolate_into`], so there is one definition of what a placeholder means.
pub fn translate_with_args(
    key: &str,
    context: Option<&str>,
    count: u32,
    args: &[(&str, &str)],
) -> String {
    let mut out = String::new();
    translate_into_with_args(key, context, count, args, &mut out);
    out
}

/// Translate a key and fill its placeholders into `out`.
///
/// When the global manager is not initialised the key itself is emitted and then filled: a host
/// that reads `"{count} files"` before `init()` gets `"3 files"` rather than a raw identifier
/// with the argument lost.
pub fn translate_into_with_args(
    key: &str,
    context: Option<&str>,
    count: u32,
    args: &[(&str, &str)],
    out: &mut String,
) {
    let mut guard = crate::compat::lock(&GLOBAL_I18N);
    if !ensure_initialized(&mut guard) {
        crate::i18n::interpolate_into(key, args, out);
        return;
    }
    match *guard {
        Some(ref mut manager) => {
            manager.translate_with_args_into(key, context, count, args, out);
        }
        None => crate::i18n::interpolate_into(key, args, out),
    }
}
/// Translate and fill typed placeholders with the active language's number formatting.
///
pub fn translate_with_typed_args(
    key: &str,
    context: Option<&str>,
    count: u32,
    args: &[(&str, crate::i18n::PlaceholderArgument<'_>)],
) -> String {
    let mut out = String::new();
    translate_into_with_typed_args(key, context, count, args, &mut out);
    out
}

/// Translate and fill typed placeholders into `out`.
///
/// The no-manager path emits the key and fills it with the C-locale format, matching the string
/// filler's behaviour: a host that has not initialised i18n still sees its values rather than a
/// raw identifier.
pub fn translate_into_with_typed_args(
    key: &str,
    context: Option<&str>,
    count: u32,
    args: &[(&str, crate::i18n::PlaceholderArgument<'_>)],
    out: &mut String,
) {
    let mut guard = get_manager();
    if !ensure_initialized(&mut guard) {
        crate::i18n::interpolate_typed_into(key, args, None, out);
        return;
    }
    match *guard {
        Some(ref mut manager) => {
            manager.translate_with_typed_args_into(key, context, count, args, out);
        }
        None => crate::i18n::interpolate_typed_into(key, args, None, out),
    }
}

/// Get the global i18n manager
pub fn get_manager() -> MutexGuard<'static, Option<I18nManager>> {
    crate::compat::lock(&GLOBAL_I18N)
}
/// Check and reload all modified translation files
pub fn check_and_reload_all() -> Vec<crate::i18n::types::ReloadEvent> {
    let mut guard = get_manager();
    if let Some(ref mut manager) = *guard {
        manager.check_and_reload()
    } else {
        Vec::new()
    }
}

/// Serializes tests that mutate the process-global i18n state.
///
/// `GLOBAL_I18N` is a process-wide static, and Rust runs `#[test]` functions on
/// parallel threads. Tests that call `init()` / `init_with_options()` /
/// `reset` race with each other; holding this lock keeps them sequential.
#[cfg(test)]
pub(crate) fn global_i18n_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_init_loads_english() {
        let _lock = global_i18n_test_lock();
        init();
        let count = {
            let mgr = get_manager();
            mgr.as_ref().map(|m| m.translation_count()).unwrap_or(0)
        };
        assert!(count > 0, "init should load English translations");
        // Reset global state so other tests don't see loaded translations
        // (drop the manager guard first, then acquire the lock again for reset)
        *crate::compat::lock(&GLOBAL_I18N) = None;
    }

    /// `set_language` changes the active catalogue and emits the language-change event after the
    /// global lock is released.
    #[test]
    fn a_runtime_language_switch_emits_and_changes_the_catalogue() {
        let _lock = global_i18n_test_lock();
        init_with_options(InitOptions::default());

        let events = alloc::sync::Arc::new(crate::compat::Mutex::new(Vec::new()));
        let sink = alloc::sync::Arc::clone(&events);
        let signal = on_language_changed();
        signal.connect(move |event: alloc::sync::Arc<LanguageChanged>| {
            crate::compat::lock(&sink).push((*event).clone());
        });

        assert!(set_language("fr"), "a different language is a change");
        assert!(!set_language("fr"), "assigning the same language again is not");
        let reported = {
            let events = crate::compat::lock(&events);
            events.first().map(|event| event.current.clone())
        };
        assert_eq!(reported.as_deref(), Some("fr"));
        let active = {
            let guard = get_manager();
            guard.as_ref().map(|manager| manager.current_language().clone())
        };
        assert_eq!(active.as_deref(), Some("fr"));

        *crate::compat::lock(&GLOBAL_I18N) = None;
    }

    /// A language switch also mirrors the writing direction for layouts that have not pinned
    /// one (EX4-03). The direction is a thread-local fact, so the assertion runs on the same
    /// thread that performs the switch.
    #[test]
    fn a_language_switch_mirrors_the_default_text_direction() {
        let _lock = global_i18n_test_lock();
        init_with_options(InitOptions::default());

        assert!(set_language("ar"));
        assert_eq!(
            crate::core::default_text_direction(),
            crate::core::TextDirection::RightToLeft,
            "an RTL language must set the default direction"
        );

        assert!(set_language("en"));
        assert_eq!(
            crate::core::default_text_direction(),
            crate::core::TextDirection::LeftToRight,
            "switching back to an LTR language must restore the default direction"
        );

        *crate::compat::lock(&GLOBAL_I18N) = None;
    }

    /// A registered retranslator runs on `retranslate` and stops after its handle is dropped.
    #[test]
    fn retranslate_runs_registered_callbacks_until_their_handle_is_dropped() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        let _lock = global_i18n_test_lock();
        init_with_options(InitOptions::default());
        let calls = alloc::sync::Arc::new(AtomicUsize::new(0));
        let counter = alloc::sync::Arc::clone(&calls);
        let handle = register_retranslator(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        retranslate();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(handle);
        retranslate();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "a dropped handle must unregister");

        *crate::compat::lock(&GLOBAL_I18N) = None;
    }
}
