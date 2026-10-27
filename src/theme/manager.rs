// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::{Material, SystemAccent};
use super::{
    AppearanceMode, Borders, Colors, Fonts, IconSlot, IconTokens, Motion, Spacing, Theme,
    ThemeOverrides, WidgetRole,
};
use crate::compat::atomic::{AtomicU64, Ordering};
use crate::compat::{HashMap, Rc, Vec};
use crate::core::{Color, Font};
use crate::signal::Signal;
use crate::style::{
    HighContrastMode, Margin, Padding, SemanticState, ThemeMode, WidgetState, WidgetStyle,
};

/// Counts changes that can alter what the process-wide theme resolves to.
///
/// # Why a revision rather than clearing the cache in each mutator
///
/// [`resolved_theme_style`] is on every control's draw path. It resolved the active theme under
/// the global manager's mutex each time — a lock plus a role lookup per widget per frame, measured
/// at ~2.5µs a call. The cache below is only correct if it is invalidated on **every** mutation,
/// and a per-mutator clear is a list that drifts. A monotonically increasing revision makes a
/// missed invalidation impossible to hide: a cached entry carries the revision it was resolved
/// at, and an older one is simply ignored.
static THEME_REVISION: AtomicU64 = AtomicU64::new(0);

// The revision a cache on **this thread** validates against.
//
// In a test build each thread tracks its own revision, alongside its own theme registry (see
// `global_theme_manager`). A theme switch in one test would otherwise invalidate the caches of
// every other test thread, turning a cache-hit assertion into a race: the second read would
// re-resolve and hand back a different `Rc`.
#[cfg(test)]
thread_local! {
    static TEST_THEME_REVISION: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// The revision the calling thread should validate cached resolutions against.
fn theme_revision() -> u64 {
    #[cfg(test)]
    {
        TEST_THEME_REVISION.with(|revision| revision.get())
    }
    #[cfg(not(test))]
    {
        THEME_REVISION.load(Ordering::Relaxed)
    }
}

/// Advances the revision on this thread and, in production, process-wide.
fn bump_theme_revision() {
    THEME_REVISION.fetch_add(1, Ordering::Relaxed);
    #[cfg(test)]
    TEST_THEME_REVISION.with(|revision| revision.set(revision.get().wrapping_add(1)));
}

/// Marks the process-wide theme as changed, so cached resolutions are no longer used.
///
/// # Locking
///
/// Callers hold the theme mutex ([`global_theme_manager`]), so this function must not take a
/// lock whose owner can call back into the theme manager. Its only cross-subsystem step is
/// [`crate::widget::runtime::arm_global_repaint`], which takes the `frame_wait` lock; the
/// lock order is therefore **theme -> frame_wait** and never the reverse. The frame-step
/// crossfade tick deliberately does not call this function: it advances the revision and the
/// caller arms the repaint after the theme guard is dropped.
fn note_theme_mutation() {
    bump_theme_revision();
    // The revision only makes the *next* paint resolve the new colours; this arms the
    // paint itself. Every control paints differently after a theme change while no
    // control's own state changed, so without it an idle, event-driven host kept the
    // frame it already drew until unrelated input arrived.
    #[cfg(not(alloc_frugal))]
    crate::widget::runtime::arm_global_repaint();
}

/// The default duration of a faded theme switch, in milliseconds.
///
/// The host itself takes roughly two hundred milliseconds to fade between its light and dark
/// appearances, and a transition much shorter than that reads as a cut while a longer one delays
/// input that arrives during it. Hosts that want another tempo pass it to
/// [`ThemeManager::set_theme_faded`].
pub const THEME_CROSSFADE_DEFAULT_MS: u32 = 220;

/// Theme registry and active-theme resolver.
pub struct ThemeManager {
    /// Registered themes keyed by theme name.
    themes: HashMap<String, Theme>,
    /// Theme names in **registration order**, oldest first.
    ///
    /// # Why the manager keeps an order the map does not
    ///
    /// [`ThemeManager::set_appearance`] must choose one theme when several share an appearance.
    /// Deriving that choice from the map's iteration order is non-deterministic: the registry is
    /// a `std::collections::HashMap` on desktop and an `alloc::collections::BTreeMap` under mini,
    /// so the same program picks different themes on different runs and different profiles
    /// (D09-THEME-01). This vector makes the tie-break explicit and auditable — **first
    /// registered wins** — and identical on every profile. A name is appended the first time it
    /// is registered and never moved when it is re-registered (an update keeps its slot); see
    /// [`ThemeManager::register_theme`].
    registration_order: Vec<String>,
    /// Active theme name.
    current_theme: String,
    /// Signal emitted when the active theme changes.
    theme_changed: Signal<()>,
    /// The active high-contrast override, if any.
    ///
    /// Held on the manager rather than on [`Theme`]: it is a *user preference*
    /// that applies across every theme, so switching from light to dark must not
    /// silently drop it.
    high_contrast: HighContrastMode,
    /// An in-progress theme colour transition, if any.
    ///
    /// `None` outside [`ThemeManager::set_theme_faded`]'s window, which is why the ordinary
    /// [`ThemeManager::set_theme`] path and every existing caller keep their instant behaviour.
    crossfade: Option<ThemeCrossfade>,
}

/// One in-progress colour transition between two theme palettes.
///
/// # Why the source palette is copied rather than named
///
/// The transition can be re-targeted while it runs (a user toggling light/dark twice quickly).
/// Naming a source *theme* would restart from that theme's palette and jump; capturing the
/// palette actually on screen keeps the colours continuous, which is the whole point of the
/// transition. The struct is deliberately small and private: the public surface is the
/// [`ThemeManager`] methods below.
#[derive(Debug, Clone)]
struct ThemeCrossfade {
    /// The palette the transition started from.
    from: Colors,
    /// How much of `duration_ms` has elapsed.
    elapsed_ms: u32,
    /// How long the transition runs. Never zero: a zero-duration request takes the instant path.
    duration_ms: u32,
}

impl ThemeManager {
    /// Creates a theme manager seeded with the default theme.
    pub fn new() -> Self {
        let default = Theme::default();
        let current_theme = default.name.clone();
        let registration_order = Vec::from([current_theme.clone()]);
        let mut themes = HashMap::new();
        themes.insert(default.name.clone(), default);
        Self {
            themes,
            registration_order,
            current_theme,
            theme_changed: Signal::new(),
            high_contrast: HighContrastMode::None,
            crossfade: None,
        }
    }

    /// Sets the high-contrast override and emits `theme_changed`.
    ///
    /// While a mode other than [`HighContrastMode::None`] is active,
    /// [`Self::resolve_style`] replaces the resolved background and text colour
    /// with the mode's forced pair, so every control switches together. The rest
    /// of the resolved style (fonts, spacing, borders, radius) is unaffected, so a
    /// forced palette does not also flatten the layout.
    ///
    /// A pairing supplied through [`HighContrastMode::Custom`] is accepted as-is;
    /// use [`HighContrastMode::contrast_ratio`] if the caller wants to check it.
    pub fn set_high_contrast(&mut self, mode: HighContrastMode) {
        self.high_contrast = mode;
        note_theme_mutation();
        self.theme_changed.emit(());
    }

    /// The active high-contrast override.
    pub fn high_contrast(&self) -> HighContrastMode {
        self.high_contrast
    }

    /// Whether the host asked for translucent materials to be replaced with opaque surfaces.
    ///
    /// The fact comes from the environment snapshot, which is refreshed once per frame, so a
    /// user who toggles the system setting sees the material degrade on the next paint rather
    /// than after a restart. `false` covers both "the setting was observed and is off" and
    /// "this backend has no such setting"; that collapse is a rendering decision and not a
    /// claim about the user (see [`EnvironmentProvider::reduce_transparency`]).
    ///
    /// [`EnvironmentProvider::reduce_transparency`]: crate::style::environment::EnvironmentProvider::reduce_transparency
    pub fn reduce_transparency(&self) -> bool {
        crate::style::environment::environment().reduce_transparency
    }

    /// The layer a control must push to draw `material`, or `None` when there is nothing to
    /// composite.
    ///
    /// The answer is `None` for [`Material::None`] and whenever the host asked to reduce
    /// transparency; in the latter case the caller draws [`Self::material_surface`] instead. A
    /// `Some` answer is `(blur_radius, tint)`, where `tint` is the palette colour and the weight
    /// to lay over the blurred backdrop. The radius is a Gaussian's 2-sigma point, the same
    /// convention `LayerEffect` uses, so a material can be handed to a `PushLayer` without
    /// converting units.
    ///
    /// This is the widget/window-level request API: a control names the material it wants and
    /// receives either a layer description or an opaque fill, and never has to read the
    /// environment itself.
    pub fn material_layer(&self, material: Material) -> Option<(u32, Option<(Color, f32)>)> {
        if self.material_is_opaque(material) {
            return None;
        }
        let radius = material.backdrop_blur_radius()?;
        Some((radius, material.tint(&self.effective_colors())))
    }

    /// Whether `material` must be drawn as an opaque surface rather than a composed layer.
    ///
    /// True for [`Material::None`] (there is no material to draw) and whenever the host asked
    /// to reduce transparency. A caller branches on this once and then draws either the layer
    /// from [`Self::material_layer`] or the fill from [`Self::material_surface`].
    pub fn material_is_opaque(&self, material: Material) -> bool {
        material == Material::None || self.reduce_transparency()
    }

    /// The opaque fill that stands in for `material`.
    ///
    /// A material always resolves to a concrete palette colour, whether or not transparency is
    /// reduced; a caller that degrades a material draws this and keeps the surface legible.
    pub fn material_surface(&self, material: Material) -> Color {
        material.opaque_surface(&self.effective_colors())
    }

    /// Names of every registered theme.
    ///
    /// Exposed so a caller can populate a theme picker without having to track
    /// registrations itself, and so a diagnostic can report what *is* available
    /// when `set_theme` refuses a name.
    pub fn theme_names(&self) -> Vec<&str> {
        self.themes.keys().map(String::as_str).collect()
    }

    /// The name of the active theme.
    pub fn current_theme_name(&self) -> &str {
        &self.current_theme
    }

    /// Selects the theme whose [`AppearanceMode`] matches, if one is registered.
    ///
    /// This is the light/dark switch a caller reaches for, and it selects from the
    /// themes that are actually registered rather than assuming a theme named
    /// `"dark"` exists. Returns `false` when no registered theme has that
    /// appearance, which is the honest answer rather than silently keeping the
    /// current theme.
    pub fn set_appearance(&mut self, appearance: AppearanceMode) -> bool {
        // D09-THEME-01: pick the **first-registered** theme of this appearance, not the map's
        // first match. `self.themes` is unordered, so `.iter().find(...)` returned a different
        // theme per process/profile when two shared an appearance; `registration_order` is the
        // manager's explicit, auditable tie-break.
        let candidate = self
            .registration_order
            .iter()
            .find(|name| self.themes.get(*name).is_some_and(|theme| theme.appearance == appearance))
            .cloned();
        match candidate {
            Some(name) => self.set_theme(&name),
            None => false,
        }
    }

    /// Registers a theme in memory.
    ///
    /// # Selection when appearances collide (D09-THEME-01)
    ///
    /// Multiple themes may share an [`AppearanceMode`]; the manager does not reject them,
    /// because a caller may legitimately keep several light themes. Instead
    /// [`Self::set_appearance`] resolves the collision by an explicit, auditable rule:
    /// **the first-registered theme of that appearance wins**, recorded in
    /// `Self::registration_order`. Registering the same name again **updates** it in place and
    /// does not change its position, so re-loading a theme file never shifts the tie-break.
    /// Passing an explicit name to [`Self::set_theme`] bypasses the rule entirely.
    pub fn register_theme(&mut self, theme: Theme) {
        let name = theme.name.clone();
        let is_new = self.themes.insert(name.clone(), theme).is_none();
        if is_new {
            self.registration_order.push(name);
        }
        // A re-registration replaces the theme's body, so it changes resolution as surely as a
        // new name does.
        note_theme_mutation();
    }

    /// Selects the appearance the **device** asks for, if a theme declares it.
    ///
    /// # Why this is the only automatic theme switch
    ///
    /// [`EnvironmentProvider::color_scheme`](crate::style::environment::EnvironmentProvider::color_scheme)
    /// and [`Theme::appearance`](crate::theme::AppearanceMode) answer two halves of one question.
    /// The provider says what the *user asked for* (including "follow the system", which it resolves
    /// itself); the theme says what it *is*. Nothing joined them, so a host that had faithfully
    /// reported `Dark` through the environment still painted light — the fact was declared and read
    /// by nobody.
    ///
    /// The conversion is where the two meet. It is deliberately **not** a `From` impl: this is a
    /// resolution of one preference *against the currently registered themes*, and a `From` would
    /// imply the mapping is a property of the values alone.
    ///
    /// # What each mode resolves to
    ///
    /// * [`ThemeMode::Light`] / [`ThemeMode::Dark`] — the matching [`AppearanceMode`].
    /// * [`ThemeMode::Auto`] — **no answer**, so this does nothing and returns `false`.
    ///
    /// The `Auto` arm is the one that matters. `color_scheme`'s own contract says `Auto` means
    /// "follow, but do not guess": the provider is expected to resolve the system preference
    /// itself and report `Dark` or `Light`. Treating `Auto` here as "pick light" would be the
    /// fabricated answer principle #37 forbids — and it would silently overrule a host whose
    /// *themes* are ordered differently from the guess.
    ///
    /// Returns whether the active theme actually changed.
    pub fn follow_environment_appearance(&mut self) -> bool {
        match crate::style::environment::environment().color_scheme {
            ThemeMode::Light => self.set_appearance(AppearanceMode::Light),
            ThemeMode::Dark => self.set_appearance(AppearanceMode::Dark),
            ThemeMode::Auto => false,
        }
    }

    /// Re-reads the host accent and re-derives the active palette when it changed.
    ///
    /// # What this is for
    ///
    /// The macOS presets derive `primary` and `accent` (and the state table those two feed) from
    /// the host accent *when the preset is built*. A user who changes the accent in System
    /// Settings after that would otherwise keep the old action colours until the application
    /// relaunches, even though every native window around it has already changed.
    ///
    /// # Why this is an explicit entry point, not a per-frame re-derivation
    ///
    /// The frame's environment snapshot already carries the accent as a *value*. Turning a
    /// changed value into a new palette is the expensive half: it rewrites `primary` and `accent`
    /// and rebuilds the theme's state table, and it invalidates the resolved-style cache. Doing
    /// that on every paint would be work proportional to the state table for a fact that changes
    /// a few times in a session, so the re-derivation is event-driven instead: the snapshot
    /// supplies the fact at frame start, and this method is what acts on a change.
    ///
    /// On macOS the cocoa backend installs an `NSSystemColorsDidChangeNotification` observer
    /// once (see `platform::macos::environment::install_system_color_observer`), and that
    /// observer calls this method. Backends without a notification source call it from wherever
    /// they learn the fact — for example after reading their own settings screen. No polling of
    /// the accent for the purpose of re-deriving the palette was added to the frame loop.
    ///
    /// # Which themes are re-derived
    ///
    /// Only a theme that was **derived** from its recorded accent is touched: `primary` and
    /// `accent` must both equal `theme.system_accent.color(...)` for the theme's appearance. The
    /// neutral presets declare `SystemAccent::Blue` as a fact about their palette without using
    /// it for their action colours, so they are left exactly as authored. Changing them to the
    /// host accent would be the library overruling a palette the application chose.
    ///
    /// Returns whether the active palette changed. An unchanged accent, a host that reports no
    /// accent, and a theme that did not derive from it all return `false` without arming a
    /// paint, which is what makes repeated refreshes idempotent.
    pub fn refresh_system_accent(&mut self) -> bool {
        // Re-read the provider rather than the frame's cached snapshot: a notification can
        // arrive between frames, and the snapshot would still describe the previous frame.
        crate::style::environment::refresh_environment();
        let Some(reported) = crate::style::environment::environment().system_accent else {
            // No observation is not the same fact as the default blue; leave the palette alone.
            return false;
        };
        let reported = SystemAccent::nearest(reported);

        let Some(theme) = self.themes.get(&self.current_theme) else {
            return false;
        };
        let dark = theme.appearance == AppearanceMode::Dark;
        let recorded = theme.system_accent.color(dark);
        let derived_from_recorded =
            theme.colors.primary == recorded && theme.colors.accent == recorded;
        if theme.system_accent == reported || !derived_from_recorded {
            return false;
        }

        let accent = reported.color(dark);
        let Some(theme) = self.themes.get_mut(&self.current_theme) else {
            // The active name was resolved above; a missing entry here would mean the map
            // changed without the name, which no mutator does. Answer honestly rather than
            // fabricate a palette.
            return false;
        };
        theme.system_accent = reported;
        theme.colors.primary = accent;
        theme.colors.accent = accent;
        // The state table is derived from the palette, so replacing the action colours without
        // re-deriving it would leave hover/pressed fills describing the old accent.
        theme.overrides.styles = super::preset_states::preset_state_overrides(&theme.colors);

        note_theme_mutation();
        self.theme_changed.emit(());
        true
    }

    /// Loads and registers a theme from a JSON file path.
    #[cfg(not(alloc_frugal))]
    pub fn load_theme(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = serde_json::from_str(&content)?;
        // Route through `register_theme` so a loaded theme joins the deterministic
        // registration order; inserting directly here would leave it unselectable by
        // `set_appearance` (D09-THEME-01).
        self.register_theme(theme);
        Ok(())
    }

    /// Loads a theme from a JSON file and makes it the active theme.
    ///
    /// [`Self::load_theme`] only registers the theme (kept for callers that
    /// pre-load a library of themes and switch later). This variant does what most
    /// callers actually mean by "load my theme": register it **and** activate it,
    /// emitting `theme_changed` so listeners restyle.
    ///
    /// Activation is by the name recorded *in the file*, not by the file name, so a
    /// file may be called anything while the theme it contains keeps its own name.
    #[cfg(not(alloc_frugal))]
    pub fn load_and_activate_theme(
        &mut self,
        path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = serde_json::from_str(&content)?;
        let name = theme.name.clone();
        // Same order bookkeeping as `load_theme` (D09-THEME-01).
        self.register_theme(theme);
        // `set_theme` always succeeds here: the entry was just inserted under this
        // exact name. Asserting rather than ignoring keeps a future change to either
        // method from silently leaving the loaded theme inactive.
        debug_assert!(
            self.set_theme(&name),
            "the theme was just inserted under this name; set_theme must find it"
        );
        Ok(name)
    }

    /// Serializes the current active theme to a JSON file at the given path.
    #[cfg(not(alloc_frugal))]
    pub fn save_theme(&self, path: &str) -> Result<(), String> {
        let theme = self.current_theme().ok_or_else(|| {
            format!(
                "no active theme to save to '{path}': {} theme(s) are registered but none is \
                 active; select one with `set_theme` first",
                self.themes.len()
            )
        })?;
        let json = serde_json::to_string_pretty(theme).map_err(|e| {
            format!(
                "theme '{}' could not be serialized to JSON (one of its colour or \
                 metric fields is not representable): {e}",
                theme.name
            )
        })?;
        std::fs::write(path, &json).map_err(|e| {
            format!(
                "theme JSON ({} bytes) could not be written to '{path}': {e} (check that the \
                 directory exists and is writable)",
                json.len()
            )
        })?;
        Ok(())
    }

    /// Selects active theme by name. Emits `theme_changed` on success.
    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            note_theme_mutation();
            self.theme_changed.emit(());
            return true;
        }
        false
    }

    /// Selects active theme by name, crossfading the colour tokens over `duration_ms`.
    ///
    /// # What this is for
    ///
    /// [`Self::set_theme`] replaces the palette between two frames, which reads as a flash when
    /// the two palettes are a light and a dark one. This variant records the palette that was on
    /// screen and lets [`Self::tick_theme_crossfade`] move every token toward the target over the
    /// requested duration; [`Self::effective_colors`] is the palette to paint with meanwhile.
    ///
    /// # Collapse rules
    ///
    /// * `duration_ms == 0`, an unknown name, or a switch to the theme already active — the
    ///   ordinary instant path, exactly as [`Self::set_theme`].
    /// * The user asked for reduced motion — the duration collapses to zero, so the end state is
    ///   **reached** rather than skipped. This is the same fold
    ///   [`crate::style::environment::effective_duration`] applies to every other transition.
    ///
    /// The source palette is captured from [`Self::effective_colors`], so re-targeting a running
    /// transition continues from the colours currently on screen instead of jumping.
    ///
    /// # What moves, and what does not
    ///
    /// The blend covers the palette tokens ([`Colors`]). The state table in
    /// `theme.overrides.styles` is derived from the *target* palette when a preset is built, so a
    /// hover or pressed fill takes the target value while the base colours interpolate. That is the
    /// boundary this state machine declares (`from: Colors`): interpolating the override table as
    /// well would need a second copy of every token for the duration of the transition.
    pub fn set_theme_faded(&mut self, name: &str, duration_ms: u32) -> bool {
        if !self.themes.contains_key(name) {
            // Leave any running transition alone: a rejected request must not cancel work the
            // caller asked for earlier.
            return false;
        }
        if name == self.current_theme {
            self.crossfade = None;
            return self.set_theme(name);
        }
        let from = self.effective_colors();
        let reduced = crate::style::environment::environment().prefers_reduced_motion();
        let duration_ms = if reduced { 0 } else { duration_ms };
        // `set_theme` emits `theme_changed` and arms the repaint for both paths, so there is one
        // place that says "the active theme changed" rather than two that could drift.
        let changed = self.set_theme(name);
        debug_assert!(changed, "the name was checked against the registry above");
        self.crossfade = if duration_ms == 0 {
            None
        } else {
            Some(ThemeCrossfade { from, elapsed_ms: 0, duration_ms })
        };
        changed
    }

    /// Selects a theme with the default 220 ms crossfade (`THEME_CROSSFADE_DEFAULT_MS`).
    pub fn set_theme_faded_default(&mut self, name: &str) -> bool {
        self.set_theme_faded(name, THEME_CROSSFADE_DEFAULT_MS)
    }

    /// The palette to paint with **this frame**.
    ///
    /// Outside a transition this is the active theme's palette. During one it is every token
    /// blended from the palette that was on screen toward the target, eased with the theme's
    /// motion curve, so a control that reads it moves continuously instead of in two steps.
    ///
    /// Returns [`Colors::default`] when no theme is active, which a caller-owned manager can
    /// produce by selecting a name before registering anything; callers that can distinguish the
    /// two ask [`Self::current_theme`] first.
    pub fn effective_colors(&self) -> Colors {
        let colors = match self.current_theme() {
            None => Colors::default(),
            Some(theme) => {
                let Some(crossfade) = &self.crossfade else {
                    return self.forced_palette_or(theme.colors.clone());
                };
                if crossfade.duration_ms == 0 {
                    theme.colors.clone()
                } else {
                    let progress = (crossfade.elapsed_ms as f32 / crossfade.duration_ms as f32)
                        .clamp(0.0, 1.0);
                    let eased = crate::style::EasingFunction::EaseOut.apply(progress);
                    crossfade.from.blend(&theme.colors, eased)
                }
            }
        };
        self.forced_palette_or(colors)
    }

    /// Replaces `colors` with the high-contrast palette when one is active.
    ///
    /// Applied to the palette as well as to a resolved style so a control that reads a role
    /// directly (`semantic_color`, `layer_color`, a link) moves with the forced mode. The
    /// override is a user preference rather than a property of the theme, so it is applied
    /// after the crossfade and survives a theme switch.
    fn forced_palette_or(&self, colors: Colors) -> Colors {
        self.high_contrast.forced_colors().unwrap_or(colors)
    }

    /// Advances a running theme crossfade by `delta_ms`.
    ///
    /// Returns `true` while another frame is still owed, `false` when no transition is running
    /// (including the frame it finishes on). That is the same contract
    /// [`crate::style::PropertyDriver::is_moving`] has, so a frame driver can treat both the same
    /// way: keep scheduling while either says yes.
    ///
    /// Each step revalidates the theme revision against the **blended palette's content**:
    /// the revision moves only when the blend actually produced different colours than the
    /// previous frame's. That is the same guarantee the old unconditional bump gave — a cached
    /// resolved style never survives a palette change — without the waste: the blend is the
    /// only state a tick can change (the registry, the overrides, the fonts and the
    /// high-contrast mode are all frozen for the length of a transition), so a tick whose
    /// 8-bit blend equals the previous frame's (the eased head and tail of a transition, or a
    /// step too small to move a channel) invalidates nothing, and the resolved-style caches
    /// keep entries that would have re-resolved to exactly what they already held (G-402).
    /// # Locking
    ///
    /// Runs under the theme mutex. It advances the theme revision when the blended palette
    /// actually moved (see above), but **does not** arm the
    /// global repaint: arming takes the `frame_wait` lock, which would extend the theme lock's
    /// scope into the wake subsystem. The caller arms the repaint after dropping the guard,
    /// which keeps the lock order one-way (theme -> frame_wait).
    pub fn tick_theme_crossfade(&mut self, delta_ms: u32) -> bool {
        if self.crossfade.is_none() {
            return false;
        }
        // The palette in force before this tick. A tick is the only writer of the blend, so
        // comparing the palettes on either side of the advance is the whole "did anything the
        // caches depend on change" question.
        let before = self.effective_colors();
        let Some(crossfade) = self.crossfade.as_mut() else {
            return false;
        };
        crossfade.elapsed_ms = crossfade.elapsed_ms.saturating_add(delta_ms);
        let finished = crossfade.elapsed_ms >= crossfade.duration_ms;
        if finished {
            self.crossfade = None;
        }
        let after = self.effective_colors();
        if before != after {
            bump_theme_revision();
        }
        !finished
    }

    /// Whether a colour crossfade is currently running.
    ///
    /// The frame step checks this **before** [`Self::tick_theme_crossfade`] so it can arm the
    /// repaint for the finishing step too, after the theme guard is released.
    pub fn is_crossfading(&self) -> bool {
        self.crossfade.is_some()
    }

    /// Returns currently active theme.
    pub fn current_theme(&self) -> Option<&Theme> {
        self.themes.get(&self.current_theme)
    }

    /// Returns a registered theme by name.
    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// The active theme's nominal icon size for `slot`, in logical pixels (THM-02).
    ///
    /// Exposed on the manager so a control asks the *active* theme rather than holding a theme
    /// reference — the same way controls read colours through [`active_palette_shared`]. Returns
    /// `None` only when no theme is active, which the global manager never produces but a
    /// caller-owned manager whose active theme was removed can.
    pub fn icon_size(&self, slot: IconSlot) -> Option<u32> {
        self.current_theme().map(|theme| theme.icon_size(slot))
    }

    /// Returns a reference to the `theme_changed` signal.
    ///
    /// Connect slots to this signal to be notified when the active theme is switched.
    ///
    /// # In-crate wiring (G-404)
    ///
    /// Every mounted control subscribes at registration (`finish_registration` in
    /// `widget/runtime.rs`): the slot only records the control's id, and the recorded
    /// restyles are merged by `drain_pending_theme_restyle` — from the frame path, or from
    /// `crate::reapply_active_theme` when a switch path needs the restyle synchronously.
    /// The deferral exists because emits happen while the caller holds the theme mutex and
    /// restyling resolves styles through that same non-reentrant lock. Hosts may subscribe
    /// here too; slots run in connection order, and a host slot must obey the same
    /// no-theme-lock rule the in-crate slot follows.
    pub fn on_theme_changed(&self) -> &Signal<()> {
        &self.theme_changed
    }

    /// Resolves a widget style for a class using current theme tokens.
    ///
    /// `class_name` is matched against the theme's own override keys, so a theme
    /// can name a class (`"primary"`) and have it win over the role default.
    /// Colour selection for a class with no override falls back to
    /// [`WidgetRole::for_kind_name`], which maps the name to one of seven visual
    /// treatments instead of the thirteen hardcoded spellings this used to carry.
    ///
    /// The resolved style includes the theme's font: `Theme::fonts` has nine
    /// tokens and none of them used to reach a widget, so every control kept the
    /// font its constructor picked.
    ///
    /// A high-contrast override, if one is set on the manager, replaces the
    /// resolved background and text colour with the mode's forced pair. It is
    /// applied inside this method rather than by the caller so there is exactly one
    /// place where a forced palette takes effect.
    pub fn resolve_style(&self, class_name: &str) -> WidgetStyle {
        self.resolve_style_for_state(class_name, None)
    }

    /// Resolves a style from a **kind** name and an optional **class** name.
    ///
    /// # Why the two names are separate
    ///
    /// [`resolve_style`](Self::resolve_style) takes one name that serves two purposes: it is
    /// classified by [`WidgetRole::for_kind_name`] and it is looked up as an override key. The
    /// role table is keyed on *widget kinds* (`button`, `label`, `line_edit`, ...), so passing a
    /// CSS class there is a category error — `class: "primary"` classified as
    /// [`WidgetRole::Surface`] because `primary` is not a control kind, and the node was painted
    /// as a grey panel instead of a filled brand-coloured button.
    ///
    /// Here the role always comes from `kind_name`, and `class_name` is consulted only for the
    /// per-class override, which is the vocabulary it belongs to. A class that names a role
    /// (`primary`, `danger`) therefore still selects an override written for that class, without
    /// being asked to act as a control kind.
    pub fn resolve_style_for(
        &self,
        kind_name: &str,
        class_name: Option<&str>,
        state: Option<WidgetState>,
    ) -> WidgetStyle {
        let mut style = self.resolve_base_style_for(kind_name, class_name);
        if let Some(state) = state {
            // The state word comes from `WidgetState::state_suffix`, the crate's single spelling
            // source (G-405), so a theme key and a CSS pseudo-class cannot name one state two
            // ways.
            // The state key is tried for the class first, then the kind, so a theme can
            // describe either `Button.primary:hover` or a kind-wide `button:hover`.
            let suffixes = match class_name {
                Some(class) => [
                    Some(format!("{class}:{}", state.state_suffix())),
                    Some(format!("{kind_name}:{}", state.state_suffix())),
                    None,
                ],
                None => [None, Some(format!("{kind_name}:{}", state.state_suffix())), None],
            };
            for key in suffixes.into_iter().flatten() {
                if let Some(token) = self.current_theme().and_then(|t| t.overrides.styles.get(&key))
                {
                    let theme = self.current_theme();
                    apply_token_as_child(&mut style, token, theme);
                    break;
                }
            }
        }
        self.apply_high_contrast(&mut style);
        style
    }

    /// The role-default appearance for a kind plus an optional class override.
    fn resolve_base_style_for(&self, kind_name: &str, class_name: Option<&str>) -> WidgetStyle {
        let Some(theme) = self.current_theme() else {
            return WidgetStyle::default();
        };
        let mut style =
            self.role_base_style(theme, kind_name, WidgetRole::for_kind_name(kind_name));

        // A class-level override wins over the role default. The class is looked up
        // verbatim first, then by the role it stands for, so a theme may override either a
        // specific class or a whole role.
        if let Some(class_name) = class_name {
            let token = theme.overrides.styles.get(class_name).or_else(|| {
                theme.overrides.styles.get(role_key(WidgetRole::for_kind_name(class_name)))
            });
            if let Some(token) = token {
                apply_token(&mut style, token, Some(theme));
            }
        }
        style
    }

    /// Forces the high-contrast palette onto `style` when one is set.
    ///
    /// Applied **last** everywhere, so neither a theme override nor a state variant can
    /// reintroduce a low-contrast colour.
    fn apply_high_contrast(&self, style: &mut WidgetStyle) {
        if let Some(forced) = self.high_contrast.forced_colors() {
            style.background_color = Some(forced.background);
            style.text_color = Some(forced.foreground);
            // A border is drawn on the surface, so it takes the ink. Leaving it at the
            // theme's value let a low-contrast outline survive a forced mode.
            style.border_color = Some(forced.outline);
            // A gradient or a texture would defeat the point of a flat forced palette.
            style.background_gradient = None;
        }
    }

    /// Resolves a widget style for a class in a specific interaction state.
    ///
    /// The base appearance comes from [`Self::resolve_style`]; then a state
    /// override named `"<class>:<state>"` (for example `"button:hover"`) is
    /// merged over the top. Naming the state in the override key rather than
    /// adding state fields to `ThemeStyleToken` keeps the token one flat,
    /// `Option`-valued record and lets a theme describe only the states it cares
    /// about.
    ///
    /// `None` means "no state-specific treatment", which is the resting state.
    ///
    /// # The three semantic variants do not arrive here from a control's interaction
    ///
    /// [`WidgetState::Error`], [`WidgetState::Warning`] and [`WidgetState::Success`] keep their
    /// entries in the `"<kind>:<state>"` table, but no control returns them from
    /// [`crate::widget::Widget::widget_state`] any more (its default and every override report an
    /// interaction). A control that carries a meaning reports it through
    /// [`crate::widget::Widget::semantic_state`] instead, and the meaning resolves to a border
    /// via [`resolved_semantic_border`] rather than to a whole style through this method. So a
    /// caller that reaches here through the interaction path never matches those three keys; the
    /// keys remain so the theme files that name them keep loading, and the tooling gate
    /// `check_semantic_state_is_not_in_the_interaction_chain` is what keeps it that way.
    ///
    /// Precedence is deliberate: a forced high-contrast pair is applied **last**, so
    /// neither a theme override nor a state variant can reintroduce a low-contrast
    /// colour. A user who has asked for maximum contrast must get it.
    pub fn resolve_style_for_state(
        &self,
        class_name: &str,
        state: Option<WidgetState>,
    ) -> WidgetStyle {
        let mut style = self.resolve_base_style(class_name);
        if let Some(state) = state {
            // The `"<kind>:<state>"` key is only built when there is a table to probe: an
            // override-free theme (the common case) paid one `format!` per resolution for a
            // lookup that could not succeed. Asking the theme once also replaces the second
            // `current_theme()` call the arm used to make.
            if let Some(theme) = self.current_theme() {
                if !theme.overrides.styles.is_empty() {
                    let key = format!("{class_name}:{}", state.state_suffix());
                    if let Some(token) = theme.overrides.styles.get(&key) {
                        apply_token_as_child(&mut style, token, Some(theme));
                    }
                }
            }
        }
        self.apply_high_contrast(&mut style);
        style
    }

    /// The role-default appearance for a widget kind, before any class or state overlay.
    ///
    /// The caller passes the [`WidgetRole`] it classified the name into, so one resolution
    /// classifies the name once even when both the base colours and the role override need it.
    ///
    /// The name is classified through [`WidgetRole::for_kind_name`], so it must be a *kind*
    /// name; a caller holding a CSS class should use
    /// [`resolve_style_for`](Self::resolve_style_for), which keeps the two vocabularies apart.
    fn role_base_style(&self, theme: &Theme, kind_name: &str, role: WidgetRole) -> WidgetStyle {
        // The face's material, resolved from the role table in `render::surface`.
        //
        // # Why the shadow now comes from a level rather than a literal
        //
        // This used to build one `Shadow { x: 0, y: 2, blur: 6, .. }` and hand it to every
        // control, so a floating toast and a flush list row were painted at the same height: the
        // layer a face sits on was not expressible. It is now a *level* the theme resolves
        // (`Theme::elevation`), and the role table says which level a kind sits at — so a card is
        // above the page and a toolbar button is on it, in one place instead of in each control.
        //
        // A kind the role table does not classify gets `SurfaceStyle::solid()`, i.e. level 0, i.e.
        // **no shadow at all**. That is a deliberate change from the old behaviour, where every
        // control cast one, and it is the change the plan's criterion 9 pins: the default theme
        // must render byte-for-byte as before, so the old shadow's *value* had to survive as the
        // level an ordinary control resolves to. It does — as `Elevation::Level2`, the shape the
        // literal described (`y: 2, blur: 6`).
        //
        // A theme that draws no shadows keeps drawing none, because `Theme::elevation` answers
        // `None` when `borders.shadow` is off — the same condition the literal was behind.
        //
        // # Why the theme, and not the role table, decides flatness
        //
        // A preset used to flatten the raised roles by carrying a class override for each of them
        // — a list of 22 kind names, in a second file, that had to be kept in step with the role
        // table by hand. The list could not be complete: the role table's fallback is a raised
        // level, so every kind the list forgot (`adaptive_scaffold`, `app_bar`, ...) went on
        // floating in a style that calls itself flat. That is a rule stated as data, which is why
        // it drifted.
        //
        // It is stated as a rule instead: flat means flat for every kind. The dimensional preset
        // answers `false` and gets the whole of the role table back, which is what "flat and
        // dimensional are two corners of one parameter space" has to mean to be true of the space
        // and not only of the kinds someone remembered to write down.
        let surface = if theme.flat_surfaces {
            crate::render::SurfaceStyle::solid()
        } else {
            crate::render::role_surface_style(kind_name)
        };
        // A shadow's hue, stated once; each level applies its own alpha (`Theme::elevation`), so the
        // ladder owns the opacity rather than this call site.
        let shadow = theme.elevation(surface.elevation, Color::BLACK);
        // Resolved from the **effective** palette, not the theme's stored one: during a faded
        // theme switch the stored palette is already the target while the frame should show the
        // blend. Off the transition path this is the same value the theme carries.
        let colors = self.effective_colors();
        let (background_color, text_color, border_color) = role_colors(&colors, role);

        // The minimum touch target for this build's device class.
        //
        // `TouchTargetSize` and `Size::dimensions()` were both correct and both unused: the values
        // lived in `style::primitives` and nothing ever wrote one into a control's style, so the
        // hit-expansion mechanism behind them had no way to engage. Writing it here, once per
        // resolved base style, is what makes every control's hit area follow the device class
        // without each control having to know about device classes.
        //
        // A theme token may still override it (`theme.rs`'s `touch_target`), and a caller's own
        // builder value wins last because `apply_active_theme` merges rather than overwrites;
        // this is the base a control is born with.
        let touch_target = Some(crate::platform::profile::recommended_touch_target().dimensions());

        WidgetStyle {
            background_color,
            text_color,
            border_color,
            border_width: Some(theme.borders.width),
            border_radius: Some(theme.borders.radius),
            padding: Padding::all(theme.spacing.medium),
            margin: Margin::all(theme.spacing.small),
            shadow,
            // The resolved face itself, not only its shadow. The shadow above is the *raised* half
            // of a surface; the bevel, material and edge are the other three dimensions, and a draw
            // path reads them from here rather than re-deriving the role table per control
            // (BLUE24 §10A.4). `surface` is the role default; the two `overrides.styles` layers
            // replace it in `apply_token`.
            surface: Some(surface),
            touch_target,
            // The theme's own base font token, **scaled by the device's text-size preference**.
            //
            // `Font::scaled` and the platform's `text_scale` accessor both existed and neither was
            // called from here, so the field was write-only: a device that asked for larger text
            // got the nominal size, and a control's font was one property nobody could influence
            // through the theme it came from. Scaling at this one point is what makes it apply to
            // every control — each of them takes this font unless it names another.
            //
            // The scale comes from the **environment** rather than from
            // `platform::profile::text_scale()` directly, so there is one source: a host that
            // installs its own `EnvironmentProvider` (a settings screen, a test) changes this
            // value too, and the platform's own answer is what the default provider reads
            // (BLUE24 §4).
            font: Some(
                theme
                    .fonts
                    .body
                    .clone()
                    .scaled(crate::style::environment::environment().effective_text_scale()),
            ),
            ..Default::default()
        }
    }

    /// The role-default appearance for a widget name, before any state overlay.
    ///
    /// # Why this delegates rather than building the style again
    ///
    /// This was a **second, byte-for-byte copy** of `role_base_style` plus the class-override step,
    /// and the two had drifted: the copy was missing the `touch_target` that `role_base_style`
    /// writes. `resolve_style` reaches this one, so the touch target never arrived at any control
    /// styled through it — the mechanism looked implemented (a field, a value table, an
    /// accessor, a merge rule) and was unreachable in practice. That is the same shape as the
    /// two style chains in `switch` and `badge`: one operation with two implementations, where
    /// only the one nobody calls is correct.
    ///
    /// Delegating makes the drift impossible rather than merely fixed: there is one derivation of
    /// a role's base style, and the class override is layered on top of it.
    fn resolve_base_style(&self, class_name: &str) -> WidgetStyle {
        let Some(theme) = self.current_theme() else {
            return WidgetStyle::default();
        };

        // Classified once here and passed down: the role is needed twice (for the base colours
        // and for the role-key override), and classifying the same name twice was the second
        // allocation this path made per resolution.
        let role = WidgetRole::for_kind_name(class_name);
        let mut style = self.role_base_style(theme, class_name, role);

        let token = theme
            .overrides
            .styles
            .get(class_name)
            .or_else(|| theme.overrides.styles.get(role_key(role)));
        if let Some(token) = token {
            apply_token(&mut style, token, Some(theme));
        }
        style
    }
}

/// The palette colours for a widget name, by role.
///
/// Split out so the classification and the colour choice are separate: the role
/// table answers "what kind of visual treatment is this?" and this function
/// answers "what does that treatment look like in this palette?".
fn role_colors(colors: &Colors, role: WidgetRole) -> (Option<Color>, Option<Color>, Option<Color>) {
    // `contrast_color` is the crate's single contrast decision; see `Color`.
    match role {
        WidgetRole::Primary => {
            (Some(colors.primary), Some(colors.primary.contrast_color()), Some(colors.primary))
        }
        WidgetRole::Text => (None, Some(colors.foreground), None),
        WidgetRole::Input => {
            (Some(colors.input_background()), Some(colors.foreground), Some(colors.secondary))
        }
        WidgetRole::Accent => (Some(colors.accent), Some(colors.accent.contrast_color()), None),
        WidgetRole::Choice => {
            (Some(colors.input_background()), Some(colors.foreground), Some(colors.secondary))
        }
        WidgetRole::Danger => (Some(colors.error), Some(colors.error.contrast_color()), None),
        WidgetRole::Surface => (
            // # Why a surface is not the window's own background
            //
            // This arm used to resolve to `theme.colors.background` — literally the colour a window
            // paints. Every control that falls through to `Surface` (a card, a panel, a group box, a
            // container, and any third-party kind the role table does not name) was therefore filled
            // with the colour behind it, so its extent was invisible: the frame rendered correctly
            // and showed nothing where the control was. The same defect the `Input` arm's own
            // documentation records for list boxes, one role over.
            //
            // `Colors::surface_container` is the role that exists for exactly this: "the container
            // colour for cards and panels sitting on `background`". A theme whose container really is
            // the window colour still gets it, because the default derivation of the role is a step
            // away from `background` — see its `default_surface_container_color` — and a theme may
            // override it to anything, including `background` itself.
            Some(colors.surface_container),
            Some(colors.foreground),
            Some(colors.secondary),
        ),
    }
}

/// The override key a role's defaults live under.
///
/// Reuses the role's own lowercase name, so a theme author writes `"input"`
/// rather than having to learn a second vocabulary.
fn role_key(role: WidgetRole) -> &'static str {
    match role {
        WidgetRole::Surface => "surface",
        WidgetRole::Primary => "primary",
        WidgetRole::Text => "text",
        WidgetRole::Input => "input",
        WidgetRole::Accent => "accent",
        WidgetRole::Choice => "choice",
        WidgetRole::Danger => "danger",
    }
}

/// Apply a partial override token to a resolved style.
///
/// Only the token's `Some` fields are written, so an override may adjust one
/// property without restating the rest.
fn apply_token(style: &mut WidgetStyle, token: &super::ThemeStyleToken, theme: Option<&Theme>) {
    if let Some(color) = token.background {
        style.background_color = Some(color);
    }
    if let Some(color) = token.foreground {
        style.text_color = Some(color);
    }
    if let Some(color) = token.border {
        style.border_color = Some(color);
    }
    if let Some(width) = token.border_width {
        style.border_width = Some(width);
    }
    if let Some(radius) = token.radius {
        style.border_radius = Some(radius);
    }
    if let Some(padding) = &token.padding {
        // THM-01: the token's cascade resolves through the style layer's own `PaddingSpec`, so a
        // theme-written padding and a programmatic `with_padding_spec` value mean the same thing.
        style.padding = padding.resolve();
    }
    if let Some(opacity) = token.opacity {
        // D09-STYLE-04: a programmatically-set `NaN` token must not reach the resolved style;
        // `normalized_opacity` applies the same policy the builder and CSS paths use.
        style.opacity = Some(crate::style::normalized_opacity(opacity));
    }
    if token.shadow != super::ShadowOverride::Inherit {
        // A named three-way choice rather than a nested `Option`: see
        // `ShadowOverride` for why the nested form lost the "clear it" case.
        style.shadow = token.shadow.apply(style.shadow.take());
    }
    if let Some([width, height]) = token.touch_target {
        style.touch_target = Some(crate::core::Size::new(width, height));
    }
    // ── The four surface dimensions (BLUE24 §10A.4) ──
    //
    // Each is applied **onto the resolved surface** rather than replacing it, so a token that
    // names one dimension keeps the other three the role default supplied. That is what makes
    // `"button:pressed": { "bevel": "inset" }` mean "the role's face, pressed" rather than
    // "a face with no elevation, no material and no edge".
    //
    // Run **before** the shadow block above would be wrong, and the order here is the reason: a
    // token that raises a face implies a shadow, and a token that *also* states one must win. The
    // shadow block ran first, so the elevation's implied shadow would have overwritten it. The
    // solution is not an ordering — it is that `apply_surface_token` only derives a shadow when
    // the token does not state one, which is passed in below.
    apply_surface_token(style, token, theme, token.shadow == super::ShadowOverride::Inherit);
    if let Some(font) = &token.font {
        style.font = Some(font.clone());
    }
    // A token that does not name a font keeps the base font the role resolver set. There used to be
    // an `else if let Some(fonts) = fonts { let _ = fonts; }` branch here that claimed reading the
    // theme's token set "is what makes the token set reachable at all" -- it was a no-op, because
    // nothing read the discarded value and the base font comes from `role_base_style`.
}

/// Applies a state/class token as a **child** of the style resolved so far (STY-005).
///
/// A token states only what changes; every property it leaves out keeps the base value. That
/// is exactly [`WidgetStyle::inherit_from`]'s contract, so the token is resolved into its own
/// style and inherited from the base rather than written directly onto it. The three
/// non-`Option` geometry fields and the shadow are seeded from the base first: padding/margin/
/// spacing deliberately do not inherit (a control's own metric is not its parent's), and an
/// explicit `shadow: none` must be distinguishable from "the token did not mention a shadow".
fn apply_token_as_child(
    style: &mut WidgetStyle,
    token: &super::ThemeStyleToken,
    theme: Option<&Theme>,
) {
    let mut overlay = WidgetStyle::default();
    overlay.padding = style.padding;
    overlay.margin = style.margin;
    overlay.spacing = style.spacing;
    overlay.shadow = style.shadow.clone();
    // A surface token is applied *onto* the resolved face (see `apply_surface_token`), so the
    // overlay must start from the base face; otherwise the token machinery writes an identity
    // `solid` surface for every state, discarding the role's bevel and elevation.
    overlay.surface = style.surface;
    apply_token(&mut overlay, token, theme);
    *style = overlay.inherit_from(style);
}

/// Applies a token's four surface dimensions onto the style's resolved face.
///
/// # Why this reads the *current* surface rather than rebuilding one
///
/// A token is an **override layer**, not a complete declaration: a theme that says
/// `"card": { "elevation": 3 }` has named one dimension of the card role's face and left the
/// other three alone. Rebuilding a `SurfaceStyle` from the token alone would discard the role
/// table's bevel, material and edge — so the token starts from what the role produced and
/// replaces only what it names (the same precedence every other key in
/// [`ThemeStyleToken`] follows).
///
/// A style that carries no surface yet (one that never went through the theme) starts from
/// [`crate::render::SurfaceStyle::solid`], the identity, so an override alone is complete.
fn apply_surface_token(
    style: &mut WidgetStyle,
    token: &super::ThemeStyleToken,
    theme: Option<&Theme>,
    may_derive_shadow: bool,
) {
    let mut surface = style.surface.unwrap_or_else(crate::render::SurfaceStyle::solid);
    // The shadow is derived from the elevation, so a token that changes the level must
    // re-derive the shadow too — otherwise a theme could raise a face without casting anything
    // and the two spellings of "how high is this?" would disagree.
    //
    // It is derived only when the token did **not** state a shadow (`may_derive_shadow`): a
    // token that says both "level3" and "shadow: null" has said two things, and the specific
    // one wins over the implied one — the same precedence every other pair in this function
    // follows.
    let mut rederive_shadow = false;

    if let Some(level) = token.elevation {
        surface.elevation = level.0;
        rederive_shadow = may_derive_shadow;
    }
    if token.bevel != super::BevelOverride::Inherit {
        surface.bevel = token.bevel.apply(surface.bevel);
    }
    if let Some(material) = token.material {
        surface.material = material.0;
    }
    if let Some(hairline) = token.hairline {
        surface.hairline = hairline.0;
    }

    if rederive_shadow {
        // The same two inputs `role_base_style` uses, read from the same place, so a token that
        // raises a face gets the shadow the role table would have given it. `None` (no active
        // theme) leaves the shadow the base layer produced, which is the honest answer.
        if let Some(theme) = theme {
            style.shadow = theme.elevation(surface.elevation, crate::core::Color::BLACK);
        }
    }

    // `style.shadow` and `surface.custom_shadow` must not disagree — see
    // `WidgetStyle::reconcile_shadow_into_surface`, which the stylesheet path calls too, so there
    // is one derivation rather than two that can drift.
    style.surface = Some(surface);
    style.reconcile_shadow_into_surface();
}

impl Colors {
    /// The interior colour for an editable field.
    ///
    /// Derived from the theme's own background rather than hardcoded to white:
    /// a dark theme whose inputs were forced white was the previous behaviour, and
    /// it made every input a glaring rectangle.
    pub fn input_background(&self) -> Color {
        // One step toward the foreground from the background: lighter on a light
        // theme, darker on a dark one, so the field reads as raised in both.
        let mix = |b: u8, f: u8| ((b as u16 * 3 + f as u16) / 4) as u8;
        Color::rgba(
            mix(self.background.r, self.foreground.r),
            mix(self.background.g, self.foreground.g),
            mix(self.background.b, self.foreground.b),
            self.background.a,
        )
    }
}

crate::impl_default_via_new!(ThemeManager);

// ── Process-wide active theme ───────────────────────────────────────────────
//
// The registry above is an ordinary value, so a caller may hold its own. The
// accessor below is what makes a theme *take effect*: widget creation, the JSON
// loader and the CSS path consult it rather than each keeping a private copy, so
// switching the theme in one place restyles the whole application.
//
// A `OnceLock<Mutex<..>>` rather than a `static mut`: the theme is written from
// the UI thread and read from wherever a widget is built, and the lock is held
// only for the duration of a clone-or-resolve call.

use crate::compat::{lock, Mutex, MutexGuard, OnceLock};

/// The process-wide theme registry.
///
/// Lazily initialised with the default light theme registered, and seeded with
/// the dark preset alongside it so [`ThemeManager::set_appearance`] works without
/// the caller having to register anything first. That seeding is what turns the
/// previously unreachable [`Theme::dark`] preset into a usable switch.
///
/// # Lock order (recorded at the mutex)
///
/// The theme mutex may be held while taking the **`frame_wait`** lock (through
/// `note_theme_mutation` -> `crate::widget::runtime::arm_global_repaint` -> `wake`).
/// No code may take the theme mutex while holding the `frame_wait` lock: that would close the
/// cycle. The frame step ([`crate::widget::runtime`]) releases this guard before arming a
/// repaint, and the macOS accent observer is delivered from AppKit's run loop, never from
/// inside this guard.
///
/// [`crate::style::environment::environment`] is **lock-free** (a thread-local snapshot plus
/// atomics), so reading it under this guard cannot deadlock; it must stay lock-free, because a
/// provider that re-entered this manager would close a second cycle.
pub fn global_theme_manager() -> MutexGuard<'static, ThemeManager> {
    // A test build isolates the registry per thread.
    //
    // The registry is process-wide by design, which is right for an application but wrong
    // for a test binary: a test that switches the appearance (or registers a theme) is
    // otherwise visible to every frame another test is asserting on, so a pixel comparison
    // can fail or pass depending on thread scheduling. The per-thread copy keeps such a
    // switch local to the test that made it. Integration tests are separate crates and do
    // not see `cfg(test)`, so they keep the process-wide registry and serialise their
    // mutations through [`theme_test_guard`].
    #[cfg(test)]
    {
        TEST_MANAGER.with(|manager| lock(manager))
    }
    #[cfg(not(test))]
    {
        static MANAGER: OnceLock<Mutex<ThemeManager>> = OnceLock::new();
        lock(MANAGER.get_or_init(seeded_theme_manager))
    }
}

/// Creates the registry a fresh process (or a fresh test thread) starts with.
///
/// The dark preset is registered alongside the light default so a caller can switch without
/// registering anything first. `register_theme` keys by the theme's own name, so re-seeding is
/// impossible and the dark preset cannot overwrite the light default.
fn seeded_theme_manager() -> Mutex<ThemeManager> {
    let mut manager = ThemeManager::new();
    let dark = Theme::dark();
    manager.register_theme(dark);
    // Arm the platform's accent observer once, on the thread that creates the registry.
    //
    // # Why here
    //
    // The observer has to be installed exactly once per process and needs AppKit's main
    // thread; the first access to the global registry is the earliest point at which both
    // are known, and it is called from the UI thread of every desktop host. An installation
    // that fails (off-main, or an AppKit without the notification) is not fatal: the
    // observer function leaves the explicit `refresh_system_accent` entry point as the
    // hook, which is the documented boundary for backends with no notification source.
    #[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
    crate::platform::macos::environment::install_system_color_observer();
    Mutex::new(manager)
}

// The per-thread registry a test build resolves against.
//
// The value is leaked so the guard `global_theme_manager` hands out can borrow it for
// `'static`, which is the lifetime its non-test form also has. A test binary has a bounded
// number of threads and exits afterwards, so the leak is bounded by the test run itself.
#[cfg(test)]
thread_local! {
    static TEST_MANAGER: &'static Mutex<ThemeManager> =
        crate::compat::Box::leak(crate::compat::Box::new(seeded_theme_manager()));
}

/// Serialises tests that mutate the process-wide theme registry.
///
/// The registry is shared state, so two tests that each switch the active theme
/// would race and see each other's writes. Existing precedent: the embedded
/// profile's `embedded_test_guard`, added for the same reason.
///
/// Public rather than `#[cfg(test)]` because integration tests are separate crates
/// and cannot see crate-test-only items, yet they exercise the same registry. Not
/// for production use — an application has no other tests to race against.
pub fn theme_test_guard() -> crate::compat::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    lock(GUARD.get_or_init(|| Mutex::new(())))
}

// ── The per-thread resolution caches ──
//
// Four caches validate against the same `THEME_REVISION`: the resolved style of a kind, the
// resolved style of a `(kind, state)` pair, the resolved style of a `(kind, class)` pair
// (the JSON layout walk's batch path, G-401), and the semantic border of a `(kind, meaning)`
// pair. They are thread-local because the draw path is per-thread and the values are small; a
// shared cache would put a lock on the path these caches exist to remove.

/// One cache entry: the revision it was resolved at, when it was last read, and its value.
///
/// `last_used` exists for eviction (G-169): a `HashMap` iterates in an order unrelated to
/// insertion or usage, so `keys().next()` removed an **arbitrary** entry — often the hottest
/// name, which then re-resolved every frame. Every hit refreshes the stamp, and eviction takes
/// the minimum, which makes the policy the least-recently-used one the comment always claimed.
struct CacheEntry<T> {
    /// The theme revision this value was resolved at.
    revision: u64,
    /// A monotonic recency stamp, refreshed on every hit.
    last_used: u64,
    /// The cached value.
    value: T,
}

impl<T> CacheEntry<T> {
    /// A fresh entry for a value resolved at `revision`.
    fn new(revision: u64, value: T) -> Self {
        Self { revision, last_used: next_use_tick(), value }
    }

    /// Records a hit, making this entry the most recently used one.
    fn mark_used(&mut self) {
        self.last_used = next_use_tick();
    }
}

// The source of `CacheEntry::last_used`.
//
// Per-thread because the caches are: a shared counter would need an atomic for no benefit, and
// the only invariant that matters is "later is larger" within one thread's cache.
thread_local! {
    static CACHE_USE_TICK: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// The next recency stamp. Wrapping is harmless: only the relative order of the entries in one
/// cache matters, and a wrap far beyond a frame's worth of reads cannot reorder them.
fn next_use_tick() -> u64 {
    CACHE_USE_TICK.with(|tick| {
        let next = tick.get().wrapping_add(1);
        tick.set(next);
        next
    })
}

/// `widget name -> entry`.
type ResolvedStyleCache = HashMap<String, CacheEntry<Rc<WidgetStyle>>>;

// Remembers the last resolved style for each widget name, with the revision it was resolved at.
//
// `WidgetStyle` is a small `Option`-heavy value (12ns to clone, measured), so a hit is a lookup
// plus one refcount bump. The bound is a working set, not a policy: the crate has roughly 180
// control kinds, so 256 entries cannot thrash for a normal application; a program that produces
// more names evicts its least-recently-used entry (G-169), not the whole table.
thread_local! {
    static RESOLVED_STYLE_CACHE: crate::compat::RefCell<ResolvedStyleCache> =
        crate::compat::RefCell::new(HashMap::new());
}

/// The largest number of resolved names the per-thread style cache keeps.
const RESOLVED_STYLE_CACHE_LIMIT: usize = 256;

/// One widget kind's cached `(kind, class)` resolutions (G-401).
///
/// Split into a class-free slot and a per-class table so a lookup probes `&str` keys at both
/// levels. A composite `(String, Option<String>)` key would allocate on every probe — and the
/// walk this cache exists for probes it once per node — while `HashMap<String, _>` accepts a
/// `&str` probe for free.
#[derive(Default)]
struct ResolvedForKindEntry {
    /// The resolution for a node that declares no `class`.
    no_class: Option<CacheEntry<Rc<WidgetStyle>>>,
    /// The resolutions for nodes that declare one, keyed by the class spelling.
    by_class: HashMap<String, CacheEntry<Rc<WidgetStyle>>>,
}

impl ResolvedForKindEntry {
    /// The entry for `class_name`, if one is cached.
    fn entry_mut(&mut self, class_name: Option<&str>) -> Option<&mut CacheEntry<Rc<WidgetStyle>>> {
        match class_name {
            Some(class) => self.by_class.get_mut(class),
            None => self.no_class.as_mut(),
        }
    }

    /// Whether the kind holds no cached resolution at all.
    fn is_empty(&self) -> bool {
        self.no_class.is_none() && self.by_class.is_empty()
    }
}

/// `widget kind -> its cached resolutions`.
type ResolvedForStyleCache = HashMap<String, ResolvedForKindEntry>;

// Remembers the last resolved `(kind, class)` style, with the revision it was resolved at.
//
// The bound is a working set, not a policy: a layout names a bounded set of kinds and classes,
// so 256 resolutions cannot thrash for a real document; past the bound the least-recently-used
// entry is evicted (G-169), not the whole table.
thread_local! {
    static RESOLVED_FOR_STYLE_CACHE: crate::compat::RefCell<ResolvedForStyleCache> =
        crate::compat::RefCell::new(HashMap::new());
}

/// The largest number of `(kind, class)` resolutions the per-thread cache keeps.
const RESOLVED_FOR_STYLE_CACHE_LIMIT: usize = 256;

/// The number of cached resolutions across all kinds. A kind row holds one class-free slot
/// plus one entry per declared class, so the table's `len` under-counts; callers of this are
/// on the insert path, which a cache miss has already made the expensive one.
fn resolved_for_cache_len(cache: &ResolvedForStyleCache) -> usize {
    cache.values().map(|kind| kind.no_class.is_some() as usize + kind.by_class.len()).sum()
}

/// Removes the single coldest `(kind, class)` resolution, wherever it sits (G-169).
fn evict_resolved_for_least_recently_used(cache: &mut ResolvedForStyleCache) {
    // O(capacity) with a constant capacity, which is what the "evict one entry" policy needs.
    let mut coldest: Option<(String, Option<String>)> = None;
    let mut coldest_used = u64::MAX;
    for (kind, entry) in cache.iter() {
        if let Some(classless) = &entry.no_class {
            if classless.last_used < coldest_used {
                coldest_used = classless.last_used;
                coldest = Some((kind.clone(), None));
            }
        }
        for (class, classed) in entry.by_class.iter() {
            if classed.last_used < coldest_used {
                coldest_used = classed.last_used;
                coldest = Some((kind.clone(), Some(class.clone())));
            }
        }
    }
    let Some((kind, class)) = coldest else {
        return;
    };
    let Some(row) = cache.get_mut(&kind) else {
        return;
    };
    match class {
        Some(class) => {
            row.by_class.remove(&class);
        }
        None => row.no_class = None,
    }
    if row.is_empty() {
        cache.remove(&kind);
    }
}

/// Advances the revision the resolved-style caches validate against.
///
/// The theme registry bumps it whenever a theme is registered or activated. The environment
/// snapshot — text scale, colour scheme, high contrast — also changes what a style resolves to
/// and is not part of that registry, so `style::environment::refresh_environment` calls this
/// when the snapshot differs from the previous frame's. Without it a 1.5× text preference
/// resolved the same cached style as 1.0×.
pub(crate) fn invalidate_resolved_style_cache() {
    bump_theme_revision();
}

/// `kind name -> (state -> entry)`.
///
/// The two-level shape is what lets a cache hit borrow the kind name instead of allocating a
/// composite `"kind:state"` key on every lookup.
type StateStyleCache = HashMap<String, HashMap<Option<WidgetState>, CacheEntry<Rc<WidgetStyle>>>>;

// The same cache for the `(kind, state)` resolver.
//
// `resolved_theme_style_for_state` is on the hover/press path: every control that paints a
// hover state asks it once per frame, and every call took the global manager's mutex and rebuilt
// the role style. The outer key is the kind and the inner key the state, so a hit borrows the
// kind name rather than allocating a composite key.
thread_local! {
    static RESOLVED_STATE_STYLE_CACHE: crate::compat::RefCell<StateStyleCache> =
        crate::compat::RefCell::new(HashMap::new());
}

/// The largest number of cached `(kind, state)` resolutions the state cache keeps.
///
/// Counted in entries rather than kind names so the eviction scan (G-169) is bounded by a
/// constant: one kind can hold twelve states, and counting kinds would let the table reach
/// twelve times the bound.
const RESOLVED_STATE_STYLE_CACHE_LIMIT: usize = 256;

/// `kind name -> (semantic meaning -> entry)` for [`resolved_semantic_border`] (G-168).
///
/// Same two-level shape as [`StateStyleCache`], for the same reason: a lookup borrows the
/// caller's kind name and the `Copy` meaning enum, so a hit allocates neither a `format!` key nor
/// a `String`.
type SemanticBorderCache = HashMap<String, HashMap<SemanticState, CacheEntry<Option<Color>>>>;

thread_local! {
    static RESOLVED_SEMANTIC_BORDER_CACHE: crate::compat::RefCell<SemanticBorderCache> =
        crate::compat::RefCell::new(HashMap::new());
}

/// The largest number of cached `(kind, meaning)` border answers the cache keeps.
const RESOLVED_SEMANTIC_BORDER_CACHE_LIMIT: usize = 256;

/// Evicts the least-recently-used name from the resolved-style cache.
fn evict_style_least_recently_used(cache: &mut ResolvedStyleCache) {
    // O(capacity) with a constant capacity, which is what the "evict one entry" policy needs.
    let coldest =
        cache.iter().min_by_key(|(_, entry)| entry.last_used).map(|(name, _)| name.clone());
    if let Some(name) = coldest {
        cache.remove(&name);
    }
}

/// Evicts the least-recently-used **single** `(kind, state)` entry.
///
/// G-169: the old code removed an arbitrary *kind*, dropping all of its states at once. A kind
/// with a hot `hover` entry and a cold `pressed` one therefore lost both, and the next hover
/// rebuilt the role style. Evicting one entry keeps the rest of the kind's working set.
fn evict_state_least_recently_used(cache: &mut StateStyleCache) {
    let mut coldest: Option<(String, Option<WidgetState>, u64)> = None;
    for (kind, states) in cache.iter() {
        for (state, entry) in states.iter() {
            let colder = match &coldest {
                Some((_, _, stamp)) => entry.last_used < *stamp,
                None => true,
            };
            if colder {
                coldest = Some((kind.clone(), *state, entry.last_used));
            }
        }
    }
    if let Some((kind, state, _)) = coldest {
        let emptied = match cache.get_mut(&kind) {
            Some(states) => {
                states.remove(&state);
                states.is_empty()
            }
            None => false,
        };
        if emptied {
            cache.remove(&kind);
        }
    }
}

/// Evicts the least-recently-used **single** `(kind, meaning)` border entry.
fn evict_semantic_border_least_recently_used(cache: &mut SemanticBorderCache) {
    let mut coldest: Option<(String, SemanticState, u64)> = None;
    for (kind, meanings) in cache.iter() {
        for (meaning, entry) in meanings.iter() {
            let colder = match &coldest {
                Some((_, _, stamp)) => entry.last_used < *stamp,
                None => true,
            };
            if colder {
                coldest = Some((kind.clone(), *meaning, entry.last_used));
            }
        }
    }
    if let Some((kind, meaning, _)) = coldest {
        let emptied = match cache.get_mut(&kind) {
            Some(meanings) => {
                meanings.remove(&meaning);
                meanings.is_empty()
            }
            None => false,
        };
        if emptied {
            cache.remove(&kind);
        }
    }
}

/// Resolves the active theme's style for `widget_name`.
///
/// This is the single entry point the rest of the crate uses to ask "what should
/// this control look like?". It returns the theme's resolved style, which a
/// caller merges *under* any explicit style it already has, so an explicit style
/// always wins over the theme.
///
/// Returns `None` only when no theme is active, which cannot happen through the
/// global manager (it always has the default registered) but can for a caller's
/// own [`ThemeManager`] whose active theme was removed.
///
/// # Why the result is cached
///
/// A control's `draw` asks this question once per frame, and there are ~270 such call sites. Each
/// uncached call took the global manager's mutex, classified the name and rebuilt the role's style
/// (including its `Font`, a heap `String`) — measured at ~2.5µs, i.e. several hundred µs of pure
/// theme resolution in a 100-widget frame. The result changes only when the theme registry does,
/// which is what `THEME_REVISION` records, so a cache hit is exact rather than approximate.
/// Resolves the active theme's style for `widget_name`, sharing the cached value.
///
/// The hot callers (theme application on hover/press) only read the style while merging it into
/// a control. They used the owned-returning [`resolved_theme_style`], so every cache **hit**
/// still deep-cloned `WidgetStyle` — `Font.family` and gradient stops included — defeating the
/// cache. This variant returns the cache entry itself; a hit is a refcount bump.
pub fn resolved_theme_style_shared(widget_name: &str) -> Option<Rc<WidgetStyle>> {
    let revision = theme_revision();
    let hit = RESOLVED_STYLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.get_mut(widget_name)?;
        if entry.revision != revision {
            return None;
        }
        // Refreshing the stamp on a hit is what makes eviction LRU rather than arbitrary
        // (G-169); it costs one thread-local read.
        entry.mark_used();
        Some(Rc::clone(&entry.value))
    });
    if let Some(style) = hit {
        return Some(style);
    }

    let style = {
        let manager = global_theme_manager();
        manager.current_theme()?;
        manager.resolve_style(widget_name)
    };
    let style = Rc::new(style);
    // Re-read the revision: resolving may have initialised the registry, and the seeding
    // registers the dark preset, which bumps the revision. Storing the earlier value would
    // make the entry stale the moment it was written.
    let revision = theme_revision();
    RESOLVED_STYLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= RESOLVED_STYLE_CACHE_LIMIT {
            // Evict the least-recently-used entry rather than clearing the table. A clear turned
            // a single new kind name past the bound into a full re-resolution of every control
            // in the frame (G-169).
            evict_style_least_recently_used(&mut cache);
        }
        cache.insert(widget_name.to_string(), CacheEntry::new(revision, Rc::clone(&style)));
    });
    Some(style)
}

/// The owned-returning form of [`resolved_theme_style_shared`], kept for callers that store or
/// mutate the style. A cache hit copies the value, so hot read-only paths should use the shared
/// variant instead.
pub fn resolved_theme_style(widget_name: &str) -> Option<WidgetStyle> {
    resolved_theme_style_shared(widget_name).map(|style| (*style).clone())
}

// The active palette, validated by the same revision as the style caches.
//
// Widgets that paint a semantic token or a layer colour read the palette directly, and a handful
// of them did so several times in one `draw` (a table took three locks, the diff viewer five,
// a chart up to twenty for its axis chrome and legend). The cache makes the first read of a frame
// the only one that touches the manager; every later read is a refcount bump.
thread_local! {
    static ACTIVE_PALETTE_CACHE: crate::compat::RefCell<Option<(u64, Rc<Colors>)>> =
        const { crate::compat::RefCell::new(None) };
}

/// The active palette in force for this frame, shared and cached for the current theme revision.
///
/// During a faded theme switch this is the blended palette
/// ([`ThemeManager::effective_colors`]), not the target theme's stored palette, so widgets that
/// read semantic or layer colours move with the resolved styles instead of jumping ahead of them.
///
/// Returns `None` only when no theme is active, which the global manager never produces but a
/// caller-owned [`ThemeManager`] can. The returned value is the cache entry itself, so a hit does
/// not clone the palette.
pub fn active_palette_shared() -> Option<Rc<Colors>> {
    let revision = theme_revision();
    if let Some(colors) = ACTIVE_PALETTE_CACHE.with(|cache| {
        cache
            .borrow()
            .as_ref()
            .filter(|(cached_revision, _)| *cached_revision == revision)
            .map(|(_, colors)| Rc::clone(colors))
    }) {
        return Some(colors);
    }

    let manager = global_theme_manager();
    // An inactive theme is still `None`, not an empty palette: `effective_colors` has a default
    // for callers that cannot express absence, but this accessor's contract is to report it.
    manager.current_theme()?;
    let colors = Rc::new(manager.effective_colors());
    // Re-read after the resolution for the same reason as the style cache: the registry may
    // have been seeded by this very call, and seeding bumps the revision.
    let revision = theme_revision();
    ACTIVE_PALETTE_CACHE.with(|cache| {
        *cache.borrow_mut() = Some((revision, Rc::clone(&colors)));
    });
    Some(colors)
}

/// Resolves a theme style from a widget **kind** name plus an optional CSS **class**,
/// sharing the cached value.
///
/// This is the batch path for callers that resolve the same `(kind, class)` pair many times
/// (G-401): the JSON layout loader asks this question once per node, and the uncached
/// accessor answered it with a theme-mutex lock and a full style rebuild **per node**. The
/// answer changes only when the theme revision does — the same contract the kind-only
/// [`resolved_theme_style_shared`] cache validates against — so a 500-node document pays one
/// resolution per distinct pair per revision, and every later node gets a refcount bump.
pub fn resolved_theme_style_for_shared(
    kind_name: &str,
    class_name: Option<&str>,
) -> Option<Rc<WidgetStyle>> {
    let revision = theme_revision();
    let hit = RESOLVED_FOR_STYLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.get_mut(kind_name)?.entry_mut(class_name)?;
        if entry.revision != revision {
            return None;
        }
        // Refreshing the stamp on a hit is what makes eviction LRU rather than arbitrary
        // (G-169); it costs one thread-local read.
        entry.mark_used();
        Some(Rc::clone(&entry.value))
    });
    if let Some(style) = hit {
        return Some(style);
    }

    let style = {
        let manager = global_theme_manager();
        manager.current_theme()?;
        Rc::new(manager.resolve_style_for(kind_name, class_name, None))
    };
    // Re-read the revision: resolving may have initialised the registry, and the seeding
    // registers the dark preset, which bumps the revision. Storing the earlier value would
    // make the entry stale the moment it was written.
    let revision = theme_revision();
    RESOLVED_FOR_STYLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if resolved_for_cache_len(&cache) >= RESOLVED_FOR_STYLE_CACHE_LIMIT {
            // Evict the least-recently-used entry rather than clearing the table. A clear
            // turned one new pair past the bound into a full re-resolution of every pair the
            // walk had already paid for (G-169).
            evict_resolved_for_least_recently_used(&mut cache);
        }
        let row = cache.entry(kind_name.to_string()).or_default();
        let fresh = CacheEntry::new(revision, Rc::clone(&style));
        match class_name {
            Some(class) => {
                row.by_class.insert(class.to_string(), fresh);
            }
            None => row.no_class = Some(fresh),
        }
    });
    Some(style)
}

/// Resolves a theme style from a widget **kind** name plus an optional CSS **class**.
///
/// The owned form of [`resolved_theme_style_for_shared`], which it resolves through: the
/// signature and the semantics are unchanged (it is the same style
/// [`ThemeManager::resolve_style_for`] produces for the current revision), but a repeated ask
/// for the same pair is answered from the cache instead of re-locking the theme and
/// rebuilding the style every call.
///
/// The counterpart of [`resolved_theme_style`] for callers that hold both names, such as the
/// JSON loader reading a node's `"class"` key. Keep the roles of the two arguments apart: the
/// kind determines the visual role, the class selects an override. See
/// [`ThemeManager::resolve_style_for`] for why passing a class as the role name misclassifies
/// the widget.
pub fn resolved_theme_style_for(kind_name: &str, class_name: Option<&str>) -> Option<WidgetStyle> {
    resolved_theme_style_for_shared(kind_name, class_name).map(|style| (*style).clone())
}

/// Resolves a theme style from a widget **kind** name and its current [`WidgetState`].
///
/// The accessor that makes state overrides reachable. `resolve_style_for_state` and the
/// `"{kind}:{state}"` key format were both implemented and both tested, but every caller passed
/// `None` for the state, so a theme author could write `"button:hover"` and nothing would ever read
/// it. This is the entry point that supplies the state, and `theme::apply::apply_active_theme` is its
/// one production caller — it asks the control through `Widget::widget_state`.
///
/// A control in [`WidgetState::Normal`] resolves exactly as [`resolved_theme_style`] would: the
/// resting state has no `"{kind}:normal"` override in the shipped presets, and applying one is
/// harmless when it exists.
pub fn resolved_theme_style_for_state_shared(
    kind_name: &str,
    state: crate::style::WidgetState,
) -> Option<Rc<WidgetStyle>> {
    let revision = theme_revision();
    let cached = RESOLVED_STATE_STYLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.get_mut(kind_name)?.get_mut(&Some(state))?;
        if entry.revision != revision {
            return None;
        }
        entry.mark_used();
        Some(Rc::clone(&entry.value))
    });
    if let Some(style) = cached {
        return Some(style);
    }

    let manager = global_theme_manager();
    manager.current_theme()?;
    let style = Rc::new(manager.resolve_style_for_state(kind_name, Some(state)));
    // Re-read for the same reason as the other caches: this call may have seeded the registry.
    let revision = theme_revision();
    RESOLVED_STATE_STYLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entries: usize = cache.values().map(|states| states.len()).sum();
        if entries >= RESOLVED_STATE_STYLE_CACHE_LIMIT {
            // Evict one `(kind, state)` entry, not the whole kind and not the whole table
            // (G-169). See `evict_state_least_recently_used`.
            evict_state_least_recently_used(&mut cache);
        }
        cache
            .entry(kind_name.to_string())
            .or_default()
            .insert(Some(state), CacheEntry::new(revision, Rc::clone(&style)));
    });
    Some(style)
}

/// The owned-returning form of [`resolved_theme_style_for_state_shared`].
pub fn resolved_theme_style_for_state(
    kind_name: &str,
    state: crate::style::WidgetState,
) -> Option<WidgetStyle> {
    resolved_theme_style_for_state_shared(kind_name, state).map(|style| (*style).clone())
}

/// The border colour a control's [`SemanticState`](crate::style::SemanticState) asks for, or
/// `None` when it carries no meaning.
///
/// # Why the meaning resolves to a colour rather than to a whole style
///
/// This is the border half of the split [`SemanticState`](crate::style::SemanticState) documents:
/// a theme may name `"line_edit:error"` and that key carries a **border**, but the control's
/// **fill** must keep answering to the interaction (`Hover`, `Pressed`, …). Returning one colour
/// rather than a merged style is what keeps the two channels from overwriting each other: a
/// caller takes the fill from [`resolved_theme_style_for_state`] and this colour for the outline.
///
/// The name is resolved through the same `"<kind>:<state>"` vocabulary as every other state key
/// (`SemanticState::state_suffix`), so a theme author writes one form of override and the three
/// semantic meanings are reachable without a second key convention.
///
/// `None` when no theme is active, when the semantic state is `None`, or when the theme's
/// override does not name a border colour — in every case the caller keeps the border its
/// interaction style already gave it.
///
/// # Why the answer is cached (G-168)
///
/// This is a draw-path entry point: a control that carries a meaning asks it once per frame, and
/// the uncached form took the global manager's mutex, allocated a `format!("<kind>:<suffix>")`
/// key and searched a `BTreeMap` on every call. It is now resolved through the semantic-border
/// cache, whose key is the caller's own `&str` plus the `Copy` meaning enum — a hit neither locks
/// nor allocates. The entry is validated by the same `THEME_REVISION` every other resolution
/// cache uses, so a theme switch (or an environment change) invalidates it on the same path. A
/// `None` answer is cached too: "this theme names no border for this meaning" is a resolution,
/// not an absence of one.
pub fn resolved_semantic_border(kind_name: &str, semantic: SemanticState) -> Option<Color> {
    // `SemanticState::None` carries no key at all, so there is nothing to look up or cache.
    let suffix = semantic.state_suffix()?;
    let revision = theme_revision();
    let cached = RESOLVED_SEMANTIC_BORDER_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.get_mut(kind_name)?.get_mut(&semantic)?;
        if entry.revision != revision {
            return None;
        }
        entry.mark_used();
        Some(entry.value)
    });
    if let Some(border) = cached {
        return border;
    }

    // A miss is the only path that allocates the composite key.
    let border = {
        let manager = global_theme_manager();
        match manager.current_theme() {
            Some(theme) => {
                let key = format!("{kind_name}:{suffix}");
                theme.overrides.styles.get(&key).and_then(|token| token.border)
            }
            None => None,
        }
    };
    // Re-read after resolution for the same reason as the style caches: this call may have seeded
    // the registry, and seeding bumps the revision.
    let revision = theme_revision();
    RESOLVED_SEMANTIC_BORDER_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entries: usize = cache.values().map(|meanings| meanings.len()).sum();
        if entries >= RESOLVED_SEMANTIC_BORDER_CACHE_LIMIT {
            evict_semantic_border_least_recently_used(&mut cache);
        }
        cache
            .entry(kind_name.to_string())
            .or_default()
            .insert(semantic, CacheEntry::new(revision, border));
    });
    border
}

/// A semantic state a control can be in, mapped 1:1 onto `theme.colors`.
///
/// # Why this exists
///
/// `Theme::colors` declares `error` / `warning` / `success` / `info` as named
/// tokens, but before this there was no *accessor*: a control that wanted "the
/// theme's error colour" had no way to ask, so it wrote a literal. A census found
/// the four tokens had **zero** consumers in the control layer — the theme author
/// could change `error` and nothing on screen moved. That is the same defect class
/// as "an event is published but never emitted": a declaration with no consumer,
/// which is undetectable because nothing is broken in isolation.
///
/// This enum is the missing consumer-side handle. It is deliberately a closed set
/// — adding a token to `Colors` without adding it here is caught by the
/// `every_semantic_token_is_reachable` test, so the two cannot drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticColor {
    /// The information indicator: neutral, neither good nor bad.
    Info,
    /// A completed, successful outcome.
    Success,
    /// Something that needs attention but is not a failure.
    Warning,
    /// A failure.
    Error,
}

impl SemanticColor {
    /// Every semantic token, in a fixed order.
    ///
    /// Enumerated so a gate can walk *all* of them rather than sample the ones
    /// someone remembered — the failure mode that let the tokens go unread.
    pub const ALL: [SemanticColor; 4] =
        [SemanticColor::Info, SemanticColor::Success, SemanticColor::Warning, SemanticColor::Error];

    /// The token name as it appears in `theme.colors`.
    pub fn token(self) -> &'static str {
        match self {
            SemanticColor::Info => "info",
            SemanticColor::Success => "success",
            SemanticColor::Warning => "warning",
            SemanticColor::Error => "error",
        }
    }

    /// Reads this token out of a theme's palette.
    pub fn of(self, theme: &Theme) -> Color {
        match self {
            SemanticColor::Info => theme.colors.info,
            SemanticColor::Success => theme.colors.success,
            SemanticColor::Warning => theme.colors.warning,
            SemanticColor::Error => theme.colors.error,
        }
    }
}

/// The active theme's colour for `token`.
///
/// This is what a control that paints a *state* (a banner severity, a validation
/// message, a completed progress run) should read — instead of a literal. Returns
/// `None` only when no theme is active, which the global manager never produces.
pub fn semantic_color(token: SemanticColor) -> Option<Color> {
    // Read through the cached palette: this is the per-severity read a banner, toast, badge and
    // validation message make, often once per row in a list.
    let colors = active_palette_shared()?;
    Some(match token {
        SemanticColor::Info => colors.info,
        SemanticColor::Success => colors.success,
        SemanticColor::Warning => colors.warning,
        SemanticColor::Error => colors.error,
    })
}

/// Sets the process-wide high-contrast override.
///
/// A convenience for the common case of switching accessibility mode without
/// holding the manager. Emits `theme_changed`, so a listener that restyles on
/// that signal picks the change up the same way it picks up a theme switch.
pub fn set_global_high_contrast(mode: crate::style::HighContrastMode) {
    global_theme_manager().set_high_contrast(mode);
}

/// The active process-wide high-contrast override.
pub fn global_high_contrast() -> crate::style::HighContrastMode {
    global_theme_manager().high_contrast()
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            appearance: AppearanceMode::Light,
            // The base palette comes from `Colors::default()`; only the roles whose
            // light-appearance value this preset deliberately overrides are named.
            // Spelling every field out here is what made a new colour role an
            // unbounded edit — this preset and `dark()` both had to be found and
            // changed, and neither was marked as a construction site.
            colors: Colors {
                background: Color { r: 240, g: 240, b: 240, a: 255 },
                primary: Color { r: 33, g: 150, b: 243, a: 255 },
                secondary: Color { r: 158, g: 158, b: 158, a: 255 },
                accent: Color { r: 255, g: 152, b: 0, a: 255 },
                error: Color { r: 244, g: 67, b: 54, a: 255 },
                warning: Color { r: 255, g: 193, b: 7, a: 255 },
                success: Color { r: 76, g: 175, b: 80, a: 255 },
                disabled: Color { r: 200, g: 200, b: 200, a: 255 },
                ..Colors::default()
            },
            fonts: Fonts {
                regular: Font::simple("Arial", 14.0),
                bold: Font::bold("Arial", 14.0),
                italic: Font::with_weight("Arial", 14.0, Font::REGULAR_WEIGHT, true),
                monospace: Font::simple("Courier New", 12.0),
                caption: Font::simple("Arial", 11.0),
                body: Font::simple("Arial", 14.0),
                title: Font::bold("Arial", 16.0),
                headline: Font::bold("Arial", 20.0),
                display: Font::bold("Arial", 28.0),
            },
            spacing: Spacing { small: 4, medium: 8, large: 16, extra_large: 24 },
            icons: IconTokens::default(),
            borders: Borders { width: 1, radius: 4, shadow: true, ..Borders::default() },
            // This preset is the flat one, and it says so with the rule rather than with a list of
            // kinds: every role's bevel and elevation is flattened, in `role_base_style`, for every
            // kind. See that function for why the list had to go.
            flat_surfaces: true,
            // The neutral presets are not derived from a host accent: their palettes state their
            // own action colours. `Blue` is both the field's serde default and the accent macOS
            // reports when the user has not chosen one, so an older theme file gains a value that
            // describes it rather than a new one that changes it.
            system_accent: SystemAccent::Blue,
            metrics_profile: crate::widget::metrics::ControlMetricsProfile::Material,
            overrides: ThemeOverrides {
                // The state table alone. The *surface* half of this preset is `flat_surfaces`, and
                // moving it there is what stopped the preset having two half-answers to one
                // question — a rule and a 22-entry table that could disagree.
                //
                // The state keys are colour, not shape, so they stay data: a pressed colour is a
                // token a host may want to retheme, whereas "this preset is flat" is a property of
                // the preset and not something a host should have to spell out key by key.
                styles: crate::theme::preset_states::preset_state_overrides(&Colors::default()),
            },
            // Material's own tempo: `kRadialReactionDuration` 100 ms, `kThemeChangeDuration`
            // 200 ms, the switch's toggle 300 ms. A theme that wants a different rhythm sets
            // `theme.motion`; every animated control reads it from there rather than carrying its
            // own constant.
            motion: crate::theme::Motion::default(),
        }
    }
}

impl Theme {
    /// Creates a dark theme preset with Material Dark-inspired colors.
    ///
    /// This complements the light `Theme::default()` for dark/light mode switching.
    /// Fonts, spacing, and borders are identical to the default light theme.
    pub fn dark() -> Self {
        // Built once and used twice: the preset's own palette, and the state overrides
        // derived from it. Spelling the block twice is exactly the drift this guards
        // against -- a palette edit would move one copy and not the other, and the hover
        // fill would silently stop matching the surface it sits on.
        let colors = Colors {
            background: Color { r: 18, g: 18, b: 18, a: 255 },
            foreground: Color { r: 225, g: 225, b: 225, a: 255 },
            primary: Color { r: 100, g: 181, b: 246, a: 255 },
            secondary: Color { r: 130, g: 130, b: 130, a: 255 },
            accent: Color { r: 255, g: 171, b: 64, a: 255 },
            error: Color { r: 239, g: 83, b: 80, a: 255 },
            warning: Color { r: 255, g: 213, b: 79, a: 255 },
            success: Color { r: 129, g: 199, b: 132, a: 255 },
            disabled: Color { r: 80, g: 80, b: 80, a: 255 },
            // Lightened for the dark surface, like the three tokens above it.
            // It used to be `Color::INFO`, the *light* preset's value, which
            // made it the one semantic token that did not move with the
            // appearance — a control reading it could never respond to a theme
            // switch, and the census caught exactly that.
            info: Color { r: 138, g: 180, b: 248, a: 255 },
            // The separator family exists so a divider and a focus ring stop
            // being the same line, so the dark preset must move both with the
            // surface — left at the light values they would glare on a near-black
            // window.
            outline: Color { r: 147, g: 143, b: 153, a: 255 },
            outline_variant: Color { r: 73, g: 69, b: 79, a: 255 },
            // A scrim dims the surface it covers, on **every** appearance — that direction is not
            // an appearance-dependent choice. This used to be a white veil (`rgba(255,255,255,38)`)
            // on the argument that "a dark theme cannot dim by darkening". It can, and the veil was
            // wrong: over the dark window (`rgb(18,18,18)`) it composites to `rgb(54,54,54)`, so the
            // modal backdrop came out **brighter** than the page it covered — BLUE21 B23's defect,
            // re-derived through the token that was introduced to fix it. BLUE21 B23 lists the four
            // platforms (Material `Colors.black54`, UIKit, the reference toolkit,
            // the declarative mainstream) and **none of them lightens a backdrop toward the
            // foreground**. Judgement 15 in BLUE23 §5 pins the direction: the dark scrim must be
            // darker than the face beneath it.
            //
            // It is also deliberately not the light preset's `rgba(0,0,0,82)`: a scrim that
            // composites to the same number on both appearances is the "coincidence, not a derived
            // quantity" signature BLUE21 B23 named. A dark theme starts from a near-black backdrop,
            // so it needs a *heavier* veil to reach the same separation the light preset gets from a
            // light one — the ratio below is that heavier veil, stated as a value rather than left to
            // whatever the light preset happens to be.
            scrim: Color { r: 0, g: 0, b: 0, a: 130 },
            surface_container: Color { r: 30, g: 30, b: 33, a: 255 },
            surface_container_high: Color { r: 40, g: 40, b: 44, a: 255 },
            inverse_surface: Color { r: 228, g: 225, b: 229, a: 255 },
            on_inverse_surface: Color { r: 49, g: 48, b: 51, a: 255 },
            link: Color { r: 138, g: 180, b: 248, a: 255 },
            link_visited: Color { r: 187, g: 134, b: 252, a: 255 },
            placeholder_text: Color { r: 158, g: 158, b: 158, a: 255 },
            bright_text: Color { r: 255, g: 255, b: 255, a: 255 },
        };
        Self {
            name: "dark".to_string(),
            appearance: AppearanceMode::Dark,
            colors: colors.clone(),
            fonts: Fonts {
                regular: Font::simple("Arial", 14.0),
                bold: Font::bold("Arial", 14.0),
                italic: Font::with_weight("Arial", 14.0, Font::REGULAR_WEIGHT, true),
                monospace: Font::simple("Courier New", 12.0),
                caption: Font::simple("Arial", 11.0),
                body: Font::simple("Arial", 14.0),
                title: Font::bold("Arial", 16.0),
                headline: Font::bold("Arial", 20.0),
                display: Font::bold("Arial", 28.0),
            },
            spacing: Spacing { small: 4, medium: 8, large: 16, extra_large: 24 },
            icons: IconTokens::default(),
            borders: Borders { width: 1, radius: 4, shadow: true, ..Borders::default() },
            // The dark preset is flat too, for the same reason the default one is: the two shipped
            // presets are the crate's existing appearance, and their 377 snapshots are the safety
            // rope (BLUE24 §10A.6 criterion 9). The **dimensional** style is a separate, opt-in
            // preset — see `Theme::dimensional`.
            flat_surfaces: true,
            system_accent: SystemAccent::Blue,
            metrics_profile: crate::widget::metrics::ControlMetricsProfile::Material,
            overrides: ThemeOverrides {
                // The state table alone, for the reason [`Theme::default`] gives.
                styles: crate::theme::preset_states::preset_state_overrides(&colors),
            },
            // Material's own tempo: `kRadialReactionDuration` 100 ms, `kThemeChangeDuration`
            // 200 ms, the switch's toggle 300 ms. A theme that wants a different rhythm sets
            // `theme.motion`; every animated control reads it from there rather than carrying its
            // own constant.
            motion: crate::theme::Motion::default(),
        }
    }

    /// The **dimensional** preset: the same palette as [`Theme::default`], with the role table's
    /// bevels left in place.
    ///
    /// # Why a preset and not a flag
    ///
    /// BLUE24 §10A settles this: "flat" and "dimensional" are not two code paths, they are two
    /// corners of one parameter space, and the difference between them is **data**. A flag would
    /// be a second mechanism for the same question, and it would put the decision in the control
    /// rather than in the theme.
    ///
    /// # What actually differs from [`Theme::default`]
    ///
    /// Exactly two things, and both are data:
    ///
    /// 1. **`flat_surfaces: false`.** The default preset answers `true`, so every role's bevel and
    ///    elevation is flattened; this one does not, so [`crate::render::role_surface_style`]'s own
    ///    character shows through — a button is a raised key, a field is a well. This is a single
    ///    boolean and not a table of kind names, which is the point: see `role_base_style` for the
    ///    drift a table produced.
    /// 2. **A pressed state inverts the bevel.** `"button:pressed": { "bevel": "inset" }` is the
    ///    end-to-end demonstration of BLUE24 §10A's most valuable layer: the state channel built in
    ///    BLUE23 §2.4 already reaches `"<kind>:<state>"`, so a 3D press feedback needs one table
    ///    entry and **zero** control code.
    ///
    /// # Reachability
    ///
    /// Exposed as a public constructor rather than registered by default, because switching an
    /// application's whole appearance is the application's decision. A host calls
    /// `manager.register_theme(Theme::dimensional())` then `set_theme("dimensional")`.
    pub fn dimensional() -> Self {
        // Built with `..Theme::default()` rather than by assigning to a binding, so the palette,
        // fonts, spacing and borders are visibly the default preset's — the two presets differ in
        // exactly one place, and this spelling makes that a property of the code rather than a
        // comment.
        let mut theme =
            Theme { name: "dimensional".to_string(), flat_surfaces: false, ..Theme::default() };
        // Start from the state table alone — `flat_surfaces: false` already put the role table's
        // shapes back — then state the one piece of dimensional *interaction* the role table cannot:
        // a press turns the key in.
        theme.overrides.styles = crate::theme::preset_states::preset_state_overrides(&theme.colors);
        for kind in ["button", "toggle_button", "tool_button"] {
            theme.overrides.styles.insert(
                format!("{kind}:pressed"),
                crate::theme::ThemeStyleToken {
                    bevel: crate::theme::BevelOverride::Set(crate::render::BevelDirection::Inset),
                    ..crate::theme::ThemeStyleToken::default()
                },
            );
        }
        theme
    }

    /// A light palette built from macOS system colours.
    ///
    /// # What this is, and is not
    ///
    /// A **host-facing preset**, not the library's default. Registering it by default would
    /// change every control and every SVG snapshot; the palette is a style choice an
    /// application makes, and [`Theme::default`] stays the neutral light theme.
    ///
    /// The tokens map Apple's semantic colours onto this crate's roles: `systemBlue` is
    /// `primary`, `label` / `secondaryLabel` are `foreground` / `secondary`, `separator` is
    /// `outline`, and the window/card/raised greys are the three surface roles. The corner
    /// radius is 6 px for push buttons and text fields; surfaces stay flat because macOS
    /// controls are flat.
    ///
    /// # Host facts this preset reads
    ///
    /// * **The accent.** The host's current accent colour is read when the preset is built and
    ///   mapped to the nearest [`SystemAccent`]; `primary` and `accent` both take that colour,
    ///   and the state table is re-derived from it. A host that cannot report an accent keeps
    ///   `SystemAccent::Blue`, which is the value the preset states explicitly rather than a
    ///   guess.
    /// * **The metrics profile.** The preset declares `Macos`, so controls sized from the theme
    ///   use the shorter, squarer desktop table rather than the shipped default.
    /// * **The system UI face.** When the runtime face registry is available and the system UI
    ///   font file exists, the preset names that family; otherwise it keeps the theme it was
    ///   built from instead of claiming a face that is not installed.
    pub fn macos_light() -> Self {
        let system_accent = host_system_accent();
        let accent = system_accent.color(false);
        let mut theme = Theme {
            name: "macos-light".to_string(),
            flat_surfaces: true,
            system_accent,
            metrics_profile: crate::widget::metrics::ControlMetricsProfile::Macos,
            ..Theme::default()
        };
        theme.appearance = AppearanceMode::Light;
        theme.borders = Borders { width: 1, radius: 6, shadow: true, ..Borders::default() };
        theme.colors = Colors {
            background: Color::rgb(236, 236, 236),
            foreground: Color::rgb(29, 29, 31),
            primary: accent,
            secondary: Color::rgb(110, 110, 115),
            // The host accent drives controls and value indicators alike, so both roles take it;
            // the default preset's orange accent made sliders and progress bars look like
            // warnings.
            accent,
            error: Color::rgb(255, 59, 48),
            warning: Color::rgb(255, 204, 0),
            success: Color::rgb(52, 199, 89),
            disabled: Color::rgb(199, 199, 204),
            info: Color::rgb(50, 173, 230),
            outline: Color::rgb(142, 142, 147),
            outline_variant: Color::rgb(209, 209, 214),
            scrim: Color::rgba(0, 0, 0, 92),
            surface_container: Color::rgb(245, 245, 247),
            surface_container_high: Color::rgb(255, 255, 255),
            inverse_surface: Color::rgb(29, 29, 31),
            on_inverse_surface: Color::rgb(245, 245, 247),
            link: accent,
            link_visited: Color::rgb(88, 86, 214),
            placeholder_text: Color::rgb(142, 142, 147),
            bright_text: Color::rgb(255, 255, 255),
        };
        // The state table (hover/pressed/checked fills) is derived from a palette, so replacing
        // the palette without re-deriving it would leave those fills describing the replaced
        // colours — the same drift `Theme::dark` guards against.
        theme.overrides.styles = crate::theme::preset_states::preset_state_overrides(&theme.colors);
        if let Some(family) = register_macos_system_ui_face() {
            apply_system_ui_family(&mut theme.fonts, family);
        }
        // Native macOS motion is slightly slower than the Material tempo: 120 ms for a direct
        // pointer reaction, 220 ms for a control's own state change, 300 ms for a larger
        // transition. The values are the platform baseline; they do **not** replace
        // `Motion::default()`, which stays the Material preset's 100/200/300.
        theme.motion = Motion { fast: 120, normal: 220, slow: 300, ..Motion::default() };
        theme
    }

    /// A dark palette built from macOS system colours.
    ///
    /// The dark counterpart of [`Theme::macos_light`], built from [`Theme::dark`] so spacing and
    /// the state-table shape are the dark preset's. `systemBlue` in dark mode is `#0A84FF` (Apple
    /// lightens it so it stays legible on `#1E1E1E`), and the three greys are the vibrancy layers
    /// a dark window, card and raised surface use. The accent and metrics profile are read the
    /// same way as in [`Theme::macos_light`].
    pub fn macos_dark() -> Self {
        let system_accent = host_system_accent();
        let accent = system_accent.color(true);
        let mut theme = Theme {
            name: "macos-dark".to_string(),
            flat_surfaces: true,
            system_accent,
            metrics_profile: crate::widget::metrics::ControlMetricsProfile::Macos,
            ..Theme::dark()
        };
        theme.appearance = AppearanceMode::Dark;
        theme.borders = Borders { width: 1, radius: 6, shadow: true, ..Borders::default() };
        theme.colors = Colors {
            background: Color::rgb(30, 30, 30),
            foreground: Color::rgb(245, 245, 247),
            primary: accent,
            secondary: Color::rgb(152, 152, 157),
            accent,
            error: Color::rgb(255, 69, 58),
            warning: Color::rgb(255, 214, 10),
            success: Color::rgb(48, 209, 88),
            disabled: Color::rgb(72, 72, 74),
            info: Color::rgb(100, 210, 255),
            outline: Color::rgb(142, 142, 147),
            outline_variant: Color::rgb(58, 58, 60),
            scrim: Color::rgba(0, 0, 0, 120),
            surface_container: Color::rgb(44, 44, 46),
            surface_container_high: Color::rgb(58, 58, 60),
            inverse_surface: Color::rgb(245, 245, 247),
            on_inverse_surface: Color::rgb(29, 29, 31),
            link: accent,
            link_visited: Color::rgb(94, 92, 230),
            placeholder_text: Color::rgb(142, 142, 147),
            bright_text: Color::rgb(255, 255, 255),
        };
        theme.overrides.styles = crate::theme::preset_states::preset_state_overrides(&theme.colors);
        if let Some(family) = register_macos_system_ui_face() {
            apply_system_ui_family(&mut theme.fonts, family);
        }
        theme.motion = Motion { fast: 120, normal: 220, slow: 300, ..Motion::default() };
        theme
    }
}

/// The accent the host reports, or the system default when it cannot be read.
///
/// The read goes through the environment snapshot rather than calling the platform directly, so a
/// host that installs its own [`EnvironmentProvider`](crate::style::environment::EnvironmentProvider)
/// (a settings screen, a test) can state the accent once and have every preset agree. The honest
/// part is the `unwrap_or`: a backend with no accent concept (or a call from a thread that cannot
/// ask) leaves the preset on `SystemAccent::Blue` rather than inventing a colour.
fn host_system_accent() -> SystemAccent {
    crate::style::environment::environment()
        .system_accent
        .map(SystemAccent::nearest)
        .unwrap_or(SystemAccent::Blue)
}

/// Points every text token at the registered system UI family.
///
/// Only the text roles move: the monospace role keeps its own family, because the system UI face
/// is a proportional face and code and aligned columns need a fixed advance. The font **descriptor**
/// keeps the weight and italic flags [`Font::with_family`] carries over, and the face itself is the
/// host's system UI font. On a stock macOS that face is the variable font at
/// `/System/Library/Fonts/SFNS.ttf`, so one registered slot serves every text role: the renderer
/// instantiates it at the role's [`Font::weight`] through the `wght` axis
/// ([`crate::render::text::VariableFace`]), which is why the bold role draws bold without a second
/// static face being installed. A host face with no `wght` axis keeps its default instance, which
/// is the honest answer when there is no axis to move.
fn apply_system_ui_family(fonts: &mut Fonts, family: &str) {
    fonts.regular = fonts.regular.with_family(family);
    fonts.bold = fonts.bold.with_family(family);
    fonts.italic = fonts.italic.with_family(family);
    fonts.caption = fonts.caption.with_family(family);
    fonts.body = fonts.body.with_family(family);
    fonts.title = fonts.title.with_family(family);
    fonts.headline = fonts.headline.with_family(family);
    fonts.display = fonts.display.with_family(family);
}

/// Registers the host's system UI face and returns the family name it was registered under.
///
/// `None` is the honest answer on every path where the face is not available: the runtime face
/// registry is compiled out, this is not macOS, the system file is absent, or the registry refused
/// the bytes. The macOS presets check this **before** naming a family, so a preset never claims a
/// face the renderer cannot draw.
#[cfg(all(target_os = "macos", feature = "cocoa-legacy", feature = "runtime-fonts"))]
fn register_macos_system_ui_face() -> Option<&'static str> {
    crate::platform::macos::environment::register_system_ui_face()
}

#[cfg(all(
    target_os = "macos",
    not(feature = "cocoa-legacy"),
    feature = "macos",
    feature = "runtime-fonts"
))]
fn register_macos_system_ui_face() -> Option<&'static str> {
    crate::platform::macos_objc2::native::register_system_ui_face()
}

#[cfg(not(all(
    target_os = "macos",
    any(feature = "cocoa-legacy", feature = "macos"),
    feature = "runtime-fonts"
)))]
fn register_macos_system_ui_face() -> Option<&'static str> {
    None
}

#[cfg(all(test, feature = "desktop", widgets_unstripped))]
mod environment_appearance_tests {
    use super::*;
    use crate::core::TextDirection;
    use crate::style::environment::{
        install_environment, uninstall_environment, EnvironmentProvider,
    };
    use crate::style::{MotionPreference, ThemeMode};
    use alloc::boxed::Box;

    /// A provider whose only interesting answer is the appearance the device asks for.
    ///
    /// The other seven facts are the provider trait's own neutral defaults, written out rather
    /// than left to a `..Default::default()` so that adding a fact to the trait makes this
    /// fixture fail to compile — which is the point of a fixture that claims to be complete.
    struct DeviceAsking(ThemeMode);

    impl EnvironmentProvider for DeviceAsking {
        fn text_scale(&self) -> f32 {
            1.0
        }
        fn layout_scale(&self) -> f32 {
            1.0
        }
        fn locale(&self) -> Option<&'static str> {
            None
        }
        fn color_scheme(&self) -> ThemeMode {
            self.0
        }
        fn motion_preference(&self) -> MotionPreference {
            MotionPreference::NoPreference
        }
        fn high_contrast(&self) -> bool {
            false
        }
        fn reduce_transparency(&self) -> bool {
            false
        }
        fn system_accent(&self) -> Option<crate::core::Color> {
            None
        }
        fn text_direction(&self) -> TextDirection {
            TextDirection::LeftToRight
        }
        fn mirroring(&self) -> bool {
            false
        }
    }

    /// A manager holding a light and a dark theme, so `set_appearance` has both to choose from.
    ///
    /// `Theme::default()` is the light preset; the dark one is renamed so the assertion can tell
    /// which theme won by name, rather than by comparing colour tables that a preset change would
    /// move underneath it.
    fn manager_with_both_appearances() -> ThemeManager {
        let mut manager = ThemeManager::new();
        let mut dark = Theme::dark();
        dark.name = "dark-fixture".to_string();
        dark.appearance = AppearanceMode::Dark;
        manager.register_theme(dark);
        manager
    }

    /// The device's answer reaches the active theme: this is the whole join, end to end.
    ///
    /// # The defect this pins
    ///
    /// `EnvironmentProvider::color_scheme` and `Theme::appearance` answered two halves of one
    /// question — what the user asked for, and what a theme is — and **nothing joined them**. A
    /// host could faithfully report `Dark` through the installed provider and the app would still
    /// paint light: a declared fact with no reader. This test walks the join rather than asserting
    /// on `follow_environment_appearance` in isolation, because "the provider says dark" and "the
    /// dark theme is active" being two separate passing facts is exactly the state that was broken.
    #[test]
    fn the_devices_appearance_chooses_the_active_theme() {
        let _guard = crate::theme::theme_test_guard();
        let mut manager = manager_with_both_appearances();
        assert_eq!(manager.current_theme_name(), Theme::default().name, "light at rest");

        // The device says dark: the dark theme becomes active.
        let previous = install_environment(Box::new(DeviceAsking(ThemeMode::Dark)));
        let changed = manager.follow_environment_appearance();
        assert!(changed, "the appearance actually changed, so the answer must be `true`");
        assert_eq!(manager.current_theme_name(), "dark-fixture");

        // And back: the device saying light selects the light theme again.
        install_environment(Box::new(DeviceAsking(ThemeMode::Light)));
        assert!(manager.follow_environment_appearance());
        assert_eq!(manager.current_theme_name(), Theme::default().name);

        drop(previous);
        uninstall_environment();
    }

    /// `Auto` is **not** a synonym for light.
    ///
    /// # The defect this pins
    ///
    /// `Auto` means "follow, but do not guess": the provider is expected to resolve the system
    /// preference itself and report `Light` or `Dark`. A library that turned `Auto` into "pick
    /// light" would be inventing an answer the provider explicitly declined to give — principle
    /// #37's fabricated capability — and it would overrule a host whose themes are ordered
    /// differently from the guess. So the honest response is *no change* and `false`, which is
    /// also what lets a caller tell "nothing needed doing" from "no theme declares that
    /// appearance".
    #[test]
    fn an_auto_preference_changes_nothing_rather_than_guessing_light() {
        let _guard = crate::theme::theme_test_guard();
        let mut manager = manager_with_both_appearances();

        // Start on dark, so a guess of "light" would be observable as a change.
        assert!(manager.set_appearance(AppearanceMode::Dark));
        assert_eq!(manager.current_theme_name(), "dark-fixture");

        let previous = install_environment(Box::new(DeviceAsking(ThemeMode::Auto)));
        assert!(
            !manager.follow_environment_appearance(),
            "an unresolved preference must not be answered by guessing"
        );
        assert_eq!(
            manager.current_theme_name(),
            "dark-fixture",
            "and it must not have moved the active theme off dark"
        );

        drop(previous);
        uninstall_environment();
    }

    /// A device asking for an appearance no registered theme provides reports `false` and keeps
    /// the current theme, rather than silently falling back to an unrelated one.
    ///
    /// This is the "packaging error worth surfacing" the public API's docs describe: the caller
    /// shipped a light-only theme set on a dark device. Reporting `true` here would hide it, and
    /// switching to whatever theme happens to be first would be a fabricated answer.
    #[test]
    fn a_device_asking_for_an_unregistered_appearance_is_reported_not_guessed() {
        let _guard = crate::theme::theme_test_guard();
        // Only the default (light) theme is registered; nothing declares `Dark`.
        let mut manager = ThemeManager::new();
        let before = manager.current_theme_name().to_string();

        let previous = install_environment(Box::new(DeviceAsking(ThemeMode::Dark)));
        assert!(
            !manager.follow_environment_appearance(),
            "no registered theme provides dark, so the answer is `false`"
        );
        assert_eq!(
            manager.current_theme_name(),
            before,
            "and the light theme the app had still stands rather than being swapped arbitrarily"
        );

        drop(previous);
        uninstall_environment();
    }
}

#[cfg(all(test, feature = "desktop", widgets_unstripped))]
mod macos_preset_tests {
    use super::*;

    /// The presets state their appearance and the semantic roles the demos rely on.
    ///
    /// What this pins: the presets are the **light/dark pair** (`set_appearance` picks by the
    /// declared appearance, not by luminance), the action colour follows the host accent the preset
    /// recorded, and surfaces are flat with the 6 px corner radius. A change to any of those
    /// silently re-skins three demos, so it is worth a failing test.
    ///
    /// The eight exact accent values are pinned in `theme::types`' own test; here the palette must
    /// agree with whichever accent this host reported, so the assertion stays true on a machine
    /// whose user picked a non-default accent.
    #[test]
    fn the_macos_presets_state_their_appearance_and_roles() {
        let expected_accent = host_system_accent();

        let light = Theme::macos_light();
        assert_eq!(light.name, "macos-light");
        assert_eq!(light.appearance, AppearanceMode::Light);
        assert_eq!(light.system_accent, expected_accent);
        assert_eq!(light.colors.primary, expected_accent.color(false));
        assert_eq!(
            light.colors.accent, light.colors.primary,
            "accent is the host accent, not orange"
        );
        assert!(light.flat_surfaces, "macOS controls are flat");
        assert_eq!(light.borders.radius, 6);

        let dark = Theme::macos_dark();
        assert_eq!(dark.name, "macos-dark");
        assert_eq!(dark.appearance, AppearanceMode::Dark);
        assert_eq!(dark.system_accent, expected_accent);
        assert_eq!(dark.colors.primary, expected_accent.color(true));
        assert_eq!(dark.colors.accent, dark.colors.primary);
        assert!(dark.flat_surfaces);

        // The presets must not be the library default: registering one is a host decision, and
        // a default palette silently swapped for a macOS one would move every SVG snapshot.
        assert_ne!(
            Theme::default().colors.primary,
            light.colors.primary,
            "the default light theme must stay neutral"
        );
        assert_ne!(
            Theme::dark().colors.primary,
            dark.colors.primary,
            "the built-in dark preset must stay as it was"
        );
    }

    /// The presets select the desktop metrics table rather than inheriting the shipped default.
    ///
    /// The field is what a control consults for its size, so a preset that claimed macOS colours
    /// while leaving the profile at `Material` would draw macOS colours in Material-sized controls.
    #[test]
    fn the_macos_presets_declare_the_macos_metrics_profile() {
        use crate::widget::metrics::ControlMetricsProfile;
        assert_eq!(Theme::macos_light().metrics_profile, ControlMetricsProfile::Macos);
        assert_eq!(Theme::macos_dark().metrics_profile, ControlMetricsProfile::Macos);
    }

    /// A preset only names the system UI family when that face was actually registered.
    ///
    /// # The two halves
    ///
    /// On a host where the face is available, every text token must name the registered family,
    /// and the monospace token must not (a proportional face breaks column alignment). On a host
    /// where it is not — a non-macOS build, a build without the runtime face registry, or a system
    /// image without the font file — the tokens must keep the compiled family rather than claim a
    /// face that would render as a fallback.
    #[test]
    fn the_macos_preset_only_names_the_system_ui_face_when_it_was_registered() {
        let registered = register_macos_system_ui_face();
        let light = Theme::macos_light();
        match registered {
            Some(family) => {
                assert_eq!(light.fonts.regular.family(), family);
                assert_eq!(light.fonts.body.family(), family);
                assert_eq!(light.fonts.title.family(), family);
                assert_eq!(
                    light.fonts.monospace.family(),
                    "Courier New",
                    "the monospace role keeps a fixed-advance face"
                );
            }
            None => {
                assert_eq!(
                    light.fonts.regular.family(),
                    "Arial",
                    "without a registered face the preset keeps the compiled family"
                );
            }
        }
    }

    /// The macOS presets state the native motion tempo, and the neutral presets keep theirs.
    ///
    /// The two values are deliberately different: 120/220/300 is the desktop rhythm, while
    /// 100/200/300 is the preset every existing control was tuned against. Changing the
    /// default triple would move every snapshot that contains an in-flight animation.
    #[test]
    fn the_macos_presets_state_the_native_motion_tokens() {
        for theme in [Theme::macos_light(), Theme::macos_dark()] {
            assert_eq!(theme.motion.fast, 120, "{} fast", theme.name);
            assert_eq!(theme.motion.normal, 220, "{} normal", theme.name);
            assert_eq!(theme.motion.slow, 300, "{} slow", theme.name);
            assert_eq!(
                theme.motion.curve,
                crate::style::MotionCurve::Easing,
                "the macOS presets keep the easing curve"
            );
        }
        assert_eq!(Theme::default().motion.fast, 100, "the existing default is unchanged");
        assert_eq!(Theme::dark().motion.normal, 200, "and so is the built-in dark preset");
    }
}

#[cfg(all(test, feature = "desktop", widgets_unstripped))]
mod theme_crossfade_tests {
    use super::*;
    use crate::style::environment::{
        install_environment, uninstall_environment, EnvironmentProvider,
    };
    use crate::style::MotionPreference;
    use alloc::boxed::Box;

    /// A manager with the light default and a registered dark theme to switch to.
    fn manager_with_dark() -> ThemeManager {
        let mut manager = ThemeManager::new();
        manager.register_theme(Theme::dark());
        manager
    }

    /// A faded switch shows the source palette first, an in-between palette halfway, and the
    /// target palette when it ends.
    ///
    /// The middle assertion is the one that matters: a transition that snapped from the source to
    /// the target would pass a start/end test, so the test has to observe a frame whose colour is
    /// neither endpoint. `EaseOut` moves more than half the distance by the halfway point, which is
    /// why the tick sizes below are halves of the duration rather than a fixed frame length.
    #[test]
    fn a_faded_switch_passes_through_an_intermediate_colour() {
        let _guard = crate::theme::theme_test_guard();
        let mut manager = manager_with_dark();
        let from = manager.effective_colors();
        let to = Theme::dark().colors;

        assert!(manager.set_theme_faded("dark", THEME_CROSSFADE_DEFAULT_MS));
        assert_eq!(
            manager.effective_colors().background,
            from.background,
            "the frame the switch was requested on still shows the source palette"
        );

        let half = THEME_CROSSFADE_DEFAULT_MS / 2;
        assert!(manager.tick_theme_crossfade(half), "halfway through, another frame is owed");
        let middle = manager.effective_colors();
        assert_ne!(middle.background, from.background);
        assert_ne!(middle.background, to.background);
        assert_ne!(middle.foreground, from.foreground);
        assert_ne!(middle.foreground, to.foreground);

        assert!(
            !manager.tick_theme_crossfade(THEME_CROSSFADE_DEFAULT_MS - half),
            "the transition reports that it is finished"
        );
        assert_eq!(manager.effective_colors().background, to.background);
        assert_eq!(manager.effective_colors().foreground, to.foreground);
    }

    /// Reduced motion collapses the faded switch to the instant path.
    ///
    /// The end state must be **reached** on the first frame, not skipped: the theme is already the
    /// target, and no frame is owed afterwards.
    #[test]
    fn reduced_motion_makes_a_faded_switch_instant() {
        struct Reduced;
        impl EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        let _guard = crate::theme::theme_test_guard();
        let mut manager = manager_with_dark();
        let previous = install_environment(Box::new(Reduced));

        assert!(manager.set_theme_faded_default("dark"));
        assert_eq!(
            manager.effective_colors().background,
            Theme::dark().colors.background,
            "the target palette is in force immediately"
        );
        assert!(!manager.tick_theme_crossfade(16), "no transition is running");

        drop(previous);
        uninstall_environment();
    }

    /// The ordinary switch keeps its instant, single-frame behaviour.
    ///
    /// This is the compatibility half of the fade: every existing caller uses `set_theme`, and the
    /// new state machine must not change what that call does.
    #[test]
    fn the_plain_switch_is_still_instant() {
        let _guard = crate::theme::theme_test_guard();
        let mut manager = manager_with_dark();
        assert!(manager.set_theme("dark"));
        assert_eq!(manager.effective_colors().background, Theme::dark().colors.background);
        assert!(!manager.tick_theme_crossfade(16), "`set_theme` started no transition");
    }

    /// A tick whose blend does not move keeps the resolved-style cache valid (G-402).
    ///
    /// # How the test proves the cache survived
    ///
    /// [`resolved_theme_style_shared`] hands back the cache entry itself, so a hit and the
    /// read before it are one `Rc` allocation. The faded switch and a moving tick must
    /// re-resolve into distinct allocations; a zero-length step cannot move the blend, so
    /// the read after it must still be the *same* allocation — proof the revision was not
    /// bumped and the entry the cache held stayed exact.
    #[test]
    fn a_tick_that_does_not_move_the_blend_keeps_the_style_cache_valid() {
        let _guard = crate::theme::theme_test_guard();
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::dark());
        }

        let steady = resolved_theme_style_shared("button").expect("theme resolves");
        {
            let mut manager = global_theme_manager();
            assert!(manager.set_theme_faded("dark", THEME_CROSSFADE_DEFAULT_MS));
        }
        let warm = resolved_theme_style_shared("button").expect("theme resolves");
        assert!(
            !Rc::ptr_eq(&steady, &warm),
            "the faded switch itself must invalidate the resolved-style cache"
        );

        let half = THEME_CROSSFADE_DEFAULT_MS / 2;
        {
            let mut manager = global_theme_manager();
            assert!(manager.tick_theme_crossfade(half), "halfway through, another frame is owed");
        }
        let moved = resolved_theme_style_shared("button").expect("theme resolves");
        assert!(
            !Rc::ptr_eq(&warm, &moved),
            "a tick that moved the blend must invalidate the resolved-style cache"
        );

        {
            let mut manager = global_theme_manager();
            assert!(manager.tick_theme_crossfade(0), "a zero-length step does not finish");
        }
        let still = resolved_theme_style_shared("button").expect("theme resolves");
        assert!(
            Rc::ptr_eq(&moved, &still),
            "a tick whose blend did not move must leave the resolved-style cache valid"
        );
    }

    /// A theme file written before the new fields existed still loads.
    ///
    /// The file is produced by serialising a current theme and deleting the two keys, which is
    /// exactly the shape an older build wrote. Both fields must take their documented defaults
    /// rather than failing to deserialize.
    #[test]
    fn an_older_theme_file_loads_with_the_new_fields_at_their_defaults() {
        use crate::widget::metrics::ControlMetricsProfile;

        let mut value = serde_json::to_value(Theme::default()).expect("a theme serialises");
        let object = value.as_object_mut().expect("a theme is a JSON object");
        assert!(
            object.remove("system_accent").is_some(),
            "the fixture must exercise the old shape"
        );
        assert!(
            object.remove("metrics_profile").is_some(),
            "the fixture must exercise the old shape"
        );

        let legacy: Theme = serde_json::from_value(value).expect("an older file must still load");
        assert_eq!(legacy.system_accent, SystemAccent::Blue);
        assert_eq!(legacy.metrics_profile, ControlMetricsProfile::Material);
    }

    /// A freshly built theme states both new fields explicitly.
    #[test]
    fn a_new_theme_defaults_to_the_blue_accent_and_the_material_profile() {
        use crate::widget::metrics::ControlMetricsProfile;

        let theme = Theme::default();
        assert_eq!(theme.system_accent, SystemAccent::Blue);
        assert_eq!(theme.metrics_profile, ControlMetricsProfile::Material);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Probe: what colour does the window's fill resolve to under each preset?
    #[test]
    fn probe_window_background_colours() {
        for theme in [Theme::default(), Theme::dark(), Theme::macos_light(), Theme::macos_dark()] {
            let name = theme.name.clone();
            let mut manager = ThemeManager::new();
            manager.register_theme(theme);
            manager.set_theme(&name);
            let style = manager.resolve_style("window");
            eprintln!(
                "[PROBE] {name}: window bg={:?} theme bg={:?} surface={:?} primary={:?} error={:?}",
                style.background_color,
                manager.effective_colors().background,
                manager.effective_colors().surface_container,
                manager.effective_colors().primary,
                manager.effective_colors().error
            );
        }
    }

    /// A theme mutation must arm the next frame's full repaint.
    ///
    /// The theme revision alone only makes the *next* paint resolve the new colours; it
    /// does not ask for that paint. Without the arming below, an idle event-driven host
    /// kept the frame it already drew, so a light/dark switch appeared to do nothing until
    /// unrelated input arrived.
    #[test]
    fn a_theme_mutation_arms_a_full_repaint() {
        let before = crate::widget::runtime::global_repaint_epoch_for_test();
        note_theme_mutation();
        assert_eq!(
            crate::widget::runtime::global_repaint_epoch_for_test(),
            before + 1,
            "a theme change must arm exactly one full repaint"
        );
    }

    /// The macOS preset maps every text role onto the one registered system UI slot.
    ///
    /// # Why this test does not register anything
    ///
    /// The registry is process-global and the test runner is parallel, so a test that registers a
    /// family changes what every concurrent test resolves a character to (a real host family named
    /// `Arial` shadows the compiled Latin face for the rest of the process). The registration
    /// honesty — "report success only for a face that is in the registry" — is asserted by
    /// [`the_macos_preset_only_names_the_system_ui_face_when_it_was_registered`]. What this test
    /// pins is the *mapping*: one family moves all eight text roles, preserves each role's weight
    /// (which is what makes the variable face serve bold from the same slot), and leaves the
    /// monospace role alone.
    #[test]
    fn the_system_ui_family_maps_every_text_role_and_preserves_weights() {
        let mut fonts = Theme::default().fonts;
        let monospace = fonts.monospace.family().to_string();
        let families = [
            fonts.regular.family().to_string(),
            fonts.bold.family().to_string(),
            fonts.italic.family().to_string(),
            fonts.caption.family().to_string(),
            fonts.body.family().to_string(),
            fonts.title.family().to_string(),
            fonts.headline.family().to_string(),
            fonts.display.family().to_string(),
        ];
        let weights = [
            fonts.regular.weight(),
            fonts.bold.weight(),
            fonts.italic.weight(),
            fonts.caption.weight(),
            fonts.body.weight(),
            fonts.title.weight(),
            fonts.headline.weight(),
            fonts.display.weight(),
        ];

        apply_system_ui_family(&mut fonts, "System UI Fixture");

        for family in [
            &fonts.regular,
            &fonts.bold,
            &fonts.italic,
            &fonts.caption,
            &fonts.body,
            &fonts.title,
            &fonts.headline,
            &fonts.display,
        ] {
            assert_eq!(family.family(), "System UI Fixture", "every text role moves");
        }
        assert_eq!(
            fonts.monospace.family(),
            monospace,
            "the monospace role keeps a fixed-advance face"
        );

        let moved_weights = [
            fonts.regular.weight(),
            fonts.bold.weight(),
            fonts.italic.weight(),
            fonts.caption.weight(),
            fonts.body.weight(),
            fonts.title.weight(),
            fonts.headline.weight(),
            fonts.display.weight(),
        ];
        assert_eq!(moved_weights, weights, "a family change must not change a role's weight");
        assert_eq!(
            fonts.bold.weight(),
            crate::core::Font::BOLD_WEIGHT,
            "the bold role asks for 700, which the variable face supplies from its wght axis"
        );

        // The compiled families are what the roles held before, so the fallback is observable.
        assert!(families.iter().all(|family| family != "System UI Fixture"));
    }
}

#[cfg(test)]
mod material_tests {
    use super::*;
    use crate::style::environment::{
        install_environment, uninstall_environment, DefaultEnvironment, EnvironmentProvider,
    };
    use alloc::boxed::Box;

    struct ReducedTransparency;
    impl EnvironmentProvider for ReducedTransparency {
        fn reduce_transparency(&self) -> bool {
            true
        }
    }

    /// A material asks for a blur radius and a palette-derived tint.
    #[test]
    fn a_material_requests_a_blur_and_a_tint() {
        let previous = install_environment(Box::new(DefaultEnvironment));
        let manager = ThemeManager::new();

        let (radius, tint) =
            manager.material_layer(Material::Sidebar).expect("a sidebar asks for a layer");
        assert_eq!(radius, Material::Sidebar.backdrop_blur_radius().unwrap());
        let (colour, weight) = tint.expect("the sidebar is tinted");
        assert_eq!(colour, manager.effective_colors().surface_container);
        assert!(weight > 0.0 && weight <= 1.0, "the tint weight is a fraction: {weight}");
        assert!(manager.material_layer(Material::None).is_none(), "None is the identity");

        drop(previous);
        uninstall_environment();
    }

    /// Reduced transparency removes the layer and gives the caller an opaque palette colour.
    #[test]
    fn reduced_transparency_degrades_a_material_to_an_opaque_surface() {
        let previous = install_environment(Box::new(ReducedTransparency));
        let manager = ThemeManager::new();

        assert!(manager.reduce_transparency());
        assert!(
            manager.material_is_opaque(Material::WindowChrome),
            "a host that reduced transparency gets an opaque surface"
        );
        assert!(
            manager.material_layer(Material::WindowChrome).is_none(),
            "a host that reduced transparency must not get a backdrop layer"
        );
        let surface = manager.material_surface(Material::Sidebar);
        assert_eq!(surface, manager.effective_colors().surface_container);
        assert_eq!(surface.a, 255, "the degraded surface is opaque");

        drop(previous);
        uninstall_environment();
    }

    /// A theme file written before `Motion::curve` existed still loads, and the missing field
    /// means `Easing` rather than a spring.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_legacy_motion_block_defaults_to_the_easing_curve() {
        let motion: Motion =
            serde_json::from_str(r#"{"fast":1,"normal":2,"slow":3,"easing":"EaseOut"}"#)
                .expect("the legacy block must load");
        assert_eq!(motion.curve, crate::style::MotionCurve::Easing);
        assert_eq!(Motion::default().curve, crate::style::MotionCurve::Easing);
    }
}

#[cfg(test)]
mod system_accent_refresh_tests {
    use super::*;
    use crate::style::environment::{
        install_environment, uninstall_environment, DefaultEnvironment, EnvironmentProvider,
    };
    use alloc::boxed::Box;

    /// A provider whose only fact is the accent.
    struct Accent(Option<Color>);

    impl EnvironmentProvider for Accent {
        fn system_accent(&self) -> Option<Color> {
            self.0
        }
    }

    /// A changed host accent re-derives the active preset once, and a second refresh is a no-op.
    #[test]
    fn a_changed_system_accent_rederives_the_active_palette_once() {
        // Build the preset while the host reports no accent, so it records `Blue` — the value
        // `Theme::macos_light` uses when the device cannot be asked.
        let neutral = install_environment(Box::new(DefaultEnvironment));
        let theme = Theme::macos_light();
        assert_eq!(theme.system_accent, SystemAccent::Blue);
        assert_eq!(theme.colors.primary, SystemAccent::Blue.color(false));

        // The user then picks Purple. `install_environment` refreshes the snapshot, so the
        // preset's recorded accent and the host's fact now disagree.
        let previous =
            install_environment(Box::new(Accent(Some(SystemAccent::Purple.color(false)))));

        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("macos-light"));
        assert_eq!(manager.effective_colors().primary, SystemAccent::Blue.color(false));

        assert!(
            manager.refresh_system_accent(),
            "a changed accent must re-derive the palette of a theme derived from it"
        );
        assert_eq!(
            manager.current_theme().expect("the active theme is registered").system_accent,
            SystemAccent::Purple,
            "the theme records the accent it was re-derived from"
        );
        assert_eq!(manager.effective_colors().primary, SystemAccent::Purple.color(false));
        assert_eq!(manager.effective_colors().accent, SystemAccent::Purple.color(false));

        let settled = manager.effective_colors();
        assert!(
            !manager.refresh_system_accent(),
            "a refresh with no accent change must not re-derive again"
        );
        assert_eq!(manager.effective_colors().primary, settled.primary);
        assert_eq!(manager.effective_colors().accent, settled.accent);

        drop(previous);
        drop(neutral);
        uninstall_environment();
    }

    /// A palette that states its own action colours is not overruled by the host accent.
    #[test]
    fn an_accent_refresh_leaves_a_neutral_palette_alone() {
        let neutral = install_environment(Box::new(DefaultEnvironment));
        let previous =
            install_environment(Box::new(Accent(Some(SystemAccent::Purple.color(false)))));

        let mut manager = ThemeManager::new();
        let before = manager.effective_colors();
        assert!(
            !manager.refresh_system_accent(),
            "the neutral preset did not derive its action colours from the host accent"
        );
        assert_eq!(manager.effective_colors().primary, before.primary);
        assert_eq!(manager.effective_colors().accent, before.accent);

        drop(previous);
        drop(neutral);
        uninstall_environment();
    }
}

#[cfg(test)]
mod resolution_cache_tests {
    use super::*;
    use crate::theme::{PaddingToken, ThemeStyleToken};

    /// A theme whose override table holds exactly the given `<key>: border` entries.
    fn theme_naming_borders(name: &str, entries: &[(&str, Color)]) -> Theme {
        let mut styles = crate::compat::BTreeMap::new();
        for (key, border) in entries {
            styles.insert(
                (*key).to_string(),
                ThemeStyleToken { border: Some(*border), ..ThemeStyleToken::default() },
            );
        }
        Theme { name: name.to_string(), overrides: ThemeOverrides { styles }, ..Theme::default() }
    }

    /// A theme switch must invalidate the semantic-border cache on both answers it can hold
    /// (G-168).
    ///
    /// # How the test proves a hit happened
    ///
    /// The cache is poisoned through its own storage without touching the revision. If the second
    /// read re-resolved, it would return the theme's border; it returns the poisoned value, so the
    /// read came from the cache. The switch then has to reach through it, or the poisoned value
    /// (and the cached `None`) would outlive the theme they were resolved under.
    #[test]
    fn a_theme_switch_invalidates_the_semantic_border_cache() {
        let _guard = theme_test_guard();
        RESOLVED_SEMANTIC_BORDER_CACHE.with(|cache| cache.borrow_mut().clear());

        let first = Color::rgb(11, 22, 33);
        let second = Color::rgb(44, 55, 66);
        {
            let mut manager = global_theme_manager();
            manager.register_theme(theme_naming_borders("border-a", &[("probe:error", first)]));
            manager.register_theme(theme_naming_borders(
                "border-b",
                &[("probe:error", second), ("probe:warning", second)],
            ));
            assert!(manager.set_theme("border-a"));
        }

        // Warm both a `Some` and a `None` answer.
        assert_eq!(resolved_semantic_border("probe", SemanticState::Error), Some(first));
        assert_eq!(resolved_semantic_border("probe", SemanticState::Warning), None);

        let poisoned = Color::rgb(200, 201, 202);
        RESOLVED_SEMANTIC_BORDER_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let entry = cache
                .get_mut("probe")
                .and_then(|meanings| meanings.get_mut(&SemanticState::Error))
                .expect("the first read must have cached the error border");
            entry.value = Some(poisoned);
        });
        assert_eq!(
            resolved_semantic_border("probe", SemanticState::Error),
            Some(poisoned),
            "a same-revision read must be answered from the cache, not re-resolved"
        );

        {
            let mut manager = global_theme_manager();
            assert!(manager.set_theme("border-b"));
        }
        assert_eq!(
            resolved_semantic_border("probe", SemanticState::Error),
            Some(second),
            "a theme switch must invalidate the poisoned entry"
        );
        assert_eq!(
            resolved_semantic_border("probe", SemanticState::Warning),
            Some(second),
            "a cached `None` must not outlive the theme it was resolved under"
        );
    }

    /// A hit refreshes the recency stamp, so the policy is the least-recently-used one the
    /// comments claim (G-169).
    #[test]
    fn a_cache_hit_refreshes_the_recency_stamp() {
        let _guard = theme_test_guard();
        let revision = theme_revision();
        // The lowest possible stamp: any hit must move it forward, whatever the thread's tick
        // counter happens to be.
        let stale = 0;
        RESOLVED_STYLE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.clear();
            cache.insert(
                "probe-kind".to_string(),
                CacheEntry { revision, last_used: stale, value: Rc::new(WidgetStyle::default()) },
            );
        });

        let _ = resolved_theme_style_shared("probe-kind");

        RESOLVED_STYLE_CACHE.with(|cache| {
            let cache = cache.borrow();
            let entry = cache.get("probe-kind").expect("the entry is still cached");
            assert_eq!(entry.revision, revision, "a hit keeps the entry it resolved");
            assert!(
                entry.last_used > stale,
                "a hit must become the most recently used entry, not keep its old stamp"
            );
        });
    }

    /// Past the bound, the **coldest** name is evicted — not whatever `keys().next()` happened to
    /// yield (G-169).
    #[test]
    fn the_style_cache_evicts_the_least_recently_used_name() {
        let _guard = theme_test_guard();
        let revision = theme_revision();
        RESOLVED_STYLE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.clear();
            for index in 0..RESOLVED_STYLE_CACHE_LIMIT {
                cache.insert(
                    format!("probe-{index}"),
                    CacheEntry {
                        revision,
                        // `probe-0` is the coldest entry and `probe-255` the hottest; a
                        // `HashMap`'s iteration order is unrelated to these stamps.
                        last_used: index as u64,
                        value: Rc::new(WidgetStyle::default()),
                    },
                );
            }
        });

        let _ = resolved_theme_style_shared("brand-new-kind");

        RESOLVED_STYLE_CACHE.with(|cache| {
            let cache = cache.borrow();
            assert!(!cache.contains_key("probe-0"), "the coldest entry is evicted");
            assert!(cache.contains_key("probe-1"), "one eviction, not a table clear");
            assert!(cache.contains_key("brand-new-kind"), "the new resolution stays");
            assert_eq!(cache.len(), RESOLVED_STYLE_CACHE_LIMIT, "the bound still holds");
        });
    }

    /// The state cache evicts a single `(kind, state)`, so a hot state does not lose its cold
    /// sibling (G-169).
    #[test]
    fn the_state_cache_evicts_one_state_not_the_whole_kind() {
        let _guard = theme_test_guard();
        let revision = theme_revision();
        RESOLVED_STATE_STYLE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.clear();
            let mut hot_kind = HashMap::new();
            hot_kind.insert(
                Some(WidgetState::Hover),
                CacheEntry { revision, last_used: 1_000, value: Rc::new(WidgetStyle::default()) },
            );
            hot_kind.insert(
                Some(WidgetState::Pressed),
                CacheEntry { revision, last_used: 1, value: Rc::new(WidgetStyle::default()) },
            );
            cache.insert("probe-kind".to_string(), hot_kind);

            let mut tick = 2u64;
            while cache.values().map(|states| states.len()).sum::<usize>()
                < RESOLVED_STATE_STYLE_CACHE_LIMIT
            {
                cache.insert(
                    format!("filler-{tick}"),
                    HashMap::from([(
                        Some(WidgetState::Normal),
                        CacheEntry {
                            revision,
                            last_used: tick,
                            value: Rc::new(WidgetStyle::default()),
                        },
                    )]),
                );
                tick += 1;
            }
        });

        let _ = resolved_theme_style_for_state_shared("brand-new-kind", WidgetState::Normal);

        RESOLVED_STATE_STYLE_CACHE.with(|cache| {
            let cache = cache.borrow();
            let kind = cache.get("probe-kind").expect("only one state of the kind was cold");
            assert!(
                !kind.contains_key(&Some(WidgetState::Pressed)),
                "the coldest single entry is evicted"
            );
            assert!(
                kind.contains_key(&Some(WidgetState::Hover)),
                "the hot sibling state must survive; evicting the whole kind is the G-169 defect"
            );
            assert!(cache.contains_key("brand-new-kind"));
        });
    }

    /// A theme whose override table holds exactly the given `<class>: background` entries,
    /// with a theme-level surface-container fill that differs per theme so even a class-free
    /// resolution moves on a switch (the Surface role's fill is `surface_container`).
    fn theme_naming_backgrounds(name: &str, surface: Color, entries: &[(&str, Color)]) -> Theme {
        let mut styles = crate::compat::BTreeMap::new();
        for (key, fill) in entries {
            styles.insert(
                (*key).to_string(),
                ThemeStyleToken { background: Some(*fill), ..ThemeStyleToken::default() },
            );
        }
        Theme {
            name: name.to_string(),
            colors: Colors { surface_container: surface, ..Theme::default().colors },
            overrides: ThemeOverrides { styles },
            ..Theme::default()
        }
    }

    /// A `(kind, class)` pair is answered from the cache on a repeat ask, and a theme switch
    /// invalidates both answers the pair can hold (G-401).
    ///
    /// # How the test proves a hit happened
    ///
    /// The cache is poisoned through its own storage without touching the revision, the same
    /// way `a_theme_switch_invalidates_the_semantic_border_cache` does it. If a repeat ask
    /// re-resolved, it would return the theme's style; it returns the poisoned marker, so the
    /// ask came from the cache. The switch then has to reach through it, or the poisoned pair
    /// would outlive the theme it was resolved under.
    #[test]
    fn a_kind_class_pair_is_answered_from_the_cache_and_invalidated_by_a_switch() {
        let _guard = theme_test_guard();
        RESOLVED_FOR_STYLE_CACHE.with(|cache| cache.borrow_mut().clear());

        let first = Color::rgb(11, 22, 33);
        let second = Color::rgb(44, 55, 66);
        {
            let mut manager = global_theme_manager();
            manager.register_theme(theme_naming_backgrounds(
                "pair-a",
                Color::rgb(1, 1, 1),
                &[("primary", first)],
            ));
            manager.register_theme(theme_naming_backgrounds(
                "pair-b",
                Color::rgb(2, 2, 2),
                &[("primary", second)],
            ));
            assert!(manager.set_theme("pair-a"));
        }

        // Warm both a class-bearing node and a class-free one.
        let classed = resolved_theme_style_for_shared("probe", Some("primary"))
            .expect("an active theme resolves a class pair");
        assert_eq!(classed.background_color, Some(first));
        let classless = resolved_theme_style_for_shared("probe", None)
            .expect("an active theme resolves the class-free pair");

        // Poison both entries; a same-revision ask must answer from the cache.
        let poisoned = Rc::new(WidgetStyle { opacity: Some(0.25), ..WidgetStyle::default() });
        RESOLVED_FOR_STYLE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let row = cache.get_mut("probe").expect("the warming asks must have cached the kind");
            for slot in [Some("primary"), None] {
                let entry = row.entry_mut(slot).expect("each warmed pair must hold an entry");
                entry.value = Rc::clone(&poisoned);
            }
        });
        for pair in [Some("primary"), None] {
            let again = resolved_theme_style_for_shared("probe", pair).expect("the cache answers");
            assert!(
                Rc::ptr_eq(&again, &poisoned),
                "a same-revision ask must be answered from the cache, not re-resolved"
            );
        }

        {
            let mut manager = global_theme_manager();
            assert!(manager.set_theme("pair-b"));
        }
        let switched = resolved_theme_style_for_shared("probe", Some("primary"))
            .expect("the theme resolves after the switch");
        assert_eq!(
            switched.background_color,
            Some(second),
            "a theme switch must invalidate the poisoned entry"
        );
        let switched_classless = resolved_theme_style_for_shared("probe", None)
            .expect("the theme resolves after the switch");
        assert_ne!(
            *switched_classless, *classless,
            "a cached class-free style must not outlive the theme it was resolved under"
        );
    }

    /// Past the bound, the **coldest** `(kind, class)` entry is evicted — alone, so a hot
    /// entry keeps its cold sibling (G-169).
    #[test]
    fn the_for_style_cache_evicts_one_pair_not_the_whole_kind() {
        let _guard = theme_test_guard();
        let revision = theme_revision();
        RESOLVED_FOR_STYLE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.clear();
            let mut hot_kind = ResolvedForKindEntry::default();
            hot_kind.no_class = Some(CacheEntry {
                revision,
                last_used: 1_000,
                value: Rc::new(WidgetStyle::default()),
            });
            hot_kind.by_class.insert(
                "primary".to_string(),
                CacheEntry { revision, last_used: 1, value: Rc::new(WidgetStyle::default()) },
            );
            cache.insert("probe-kind".to_string(), hot_kind);

            let mut tick = 2u64;
            while resolved_for_cache_len(&cache) < RESOLVED_FOR_STYLE_CACHE_LIMIT {
                cache.insert(
                    format!("filler-{tick}"),
                    ResolvedForKindEntry {
                        no_class: Some(CacheEntry {
                            revision,
                            last_used: tick,
                            value: Rc::new(WidgetStyle::default()),
                        }),
                        by_class: HashMap::new(),
                    },
                );
                tick += 1;
            }
        });

        let _ = resolved_theme_style_for_shared("brand-new-kind", None);

        RESOLVED_FOR_STYLE_CACHE.with(|cache| {
            let cache = cache.borrow();
            let row = cache.get("probe-kind").expect("only one pair of the kind was cold");
            assert!(!row.by_class.contains_key("primary"), "the coldest entry is evicted");
            assert!(
                row.no_class.is_some(),
                "the hot sibling entry must survive; evicting the whole kind is the G-169 defect"
            );
            assert!(cache.contains_key("brand-new-kind"));
            assert_eq!(
                resolved_for_cache_len(&cache),
                RESOLVED_FOR_STYLE_CACHE_LIMIT,
                "the bound still holds"
            );
        });
    }

    /// THM-01: a token's padding cascade reaches the resolved style, one role at a time.
    #[test]
    fn a_padding_token_resolves_into_the_resolved_style() {
        let mut manager = ThemeManager::new();
        let mut styles = crate::compat::BTreeMap::new();
        styles.insert(
            "probe".to_string(),
            ThemeStyleToken {
                padding: Some(PaddingToken {
                    all: Some(3),
                    horizontal: Some(12),
                    ..PaddingToken::default()
                }),
                ..ThemeStyleToken::default()
            },
        );
        manager.register_theme(Theme {
            name: "padding-probe".to_string(),
            overrides: ThemeOverrides { styles },
            ..Theme::default()
        });
        assert!(manager.set_theme("padding-probe"));

        let style = manager.resolve_style("probe");
        assert_eq!(style.padding.top, 3, "the general layer wins where the axis is silent");
        assert_eq!(style.padding.bottom, 3);
        assert_eq!(style.padding.left, 12, "the axis layer overrides the general one");
        assert_eq!(style.padding.right, 12);
    }

    /// THM-02: the icon slots are readable from the theme and through the manager, and the
    /// defaults are the literals the shipped controls already used.
    #[test]
    fn icon_size_tokens_are_readable() {
        let defaults = IconTokens::default();
        assert_eq!((defaults.small, defaults.medium, defaults.large), (16, 18, 32));
        assert_eq!(Theme::default().icon_size(IconSlot::Small), 16);
        assert_eq!(Theme::default().icon_size(IconSlot::Medium), 18);
        assert_eq!(Theme::default().icon_size(IconSlot::Large), 32);

        let mut manager = ThemeManager::new();
        manager.register_theme(Theme {
            name: "icon-probe".to_string(),
            icons: IconTokens { small: 20, medium: 24, large: 40 },
            ..Theme::default()
        });
        assert!(manager.set_theme("icon-probe"));
        assert_eq!(manager.icon_size(IconSlot::Small), Some(20));
        assert_eq!(manager.icon_size(IconSlot::Medium), Some(24));
        assert_eq!(manager.icon_size(IconSlot::Large), Some(40));
    }
}
