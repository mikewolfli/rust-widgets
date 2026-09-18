// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{Box, HashMap, MiniToString, String, Vec};
use crate::core::Color;
use alloc::rc::Rc;
use core::cell::RefCell;

/// Callback type for theme mode change notifications.
pub type ModeChangedCallback = Rc<RefCell<Vec<Box<dyn FnMut(ThemeMode)>>>>;

/// The interaction state a widget is painted in.
///
/// States are used as lookup keys into a [`StatefulTheme`], so a widget reports
/// whichever single state best describes it right now; the values are not
/// mutually exclusive in reality (a widget can be both focused and hovered) and
/// the caller decides which one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum WidgetState {
    /// Resting state with no interaction. The default, and the fallback state of
    /// a [`StatefulTheme`] that does not define this key itself.
    #[default]
    Normal,
    /// Pointer is hovering over the widget.
    Hover,
    /// Widget is being held down by a pointer or key.
    Pressed,
    /// Widget holds keyboard focus.
    Focused,
    /// Widget is non-interactive; hover/press feedback is usually suppressed.
    Disabled,
    /// A toggle-like widget is in its checked/on state.
    Checked,
    /// The widget is one of the currently selected items in a collection.
    Selected,
    /// The widget or window is the active/foreground one.
    Active,
    /// The widget or window is present but not active (e.g. a background window).
    Inactive,
    /// The widget is displaying a validation error.
    Error,
    /// The widget is displaying a non-fatal warning.
    Warning,
    /// The widget is confirming a completed action.
    Success,
}
/// The complete visual description of one widget in one [`WidgetState`].
///
/// Every field is a plain value; a `StateTheme` never refers to a parent theme,
/// so overlaying one over another must be done by the caller.
#[derive(Debug, Clone)]
pub struct StateTheme {
    /// Fill colour painted behind the widget.
    pub background_color: Color,
    /// Fill colour for content drawn inside the widget, used where the widget
    /// does not have a more specific colour (e.g. tracks and wells).
    pub foreground_color: Color,
    /// Outline colour; only meaningful when [`Self::border_width`] is non-zero.
    pub border_color: Color,
    /// Outline thickness in pixels. Zero disables the border entirely.
    pub border_width: u32,
    /// Colour for text rendered by the widget. Distinct from
    /// [`Self::foreground_color`] so text can contrast with filled shapes.
    pub text_color: Color,
    /// Drop-shadow colour, or `None` for no shadow. Even when `Some`, the shadow
    /// is invisible if [`Self::shadow_blur`] and the offset are both zero.
    pub shadow_color: Option<Color>,
    /// Shadow displacement in pixels as `(dx, dy)`, positive `y` meaning
    /// downwards. May be negative; the pair is not otherwise validated.
    pub shadow_offset: (i32, i32),
    /// Shadow softness (blur radius) in pixels; `0` gives a hard-edged shadow.
    pub shadow_blur: u32,
    /// Overall widget alpha in the range `0.0` (fully transparent) to `1.0`
    /// (opaque). Clamped by [`StateTheme::with_opacity`], but a value assigned
    /// directly to the field is not re-clamped at paint time.
    pub opacity: f32,
    /// Arbitrary key/value extras for renderers or application code that need
    /// styling data the typed fields do not cover (e.g. `"corner-radius"`).
    /// Keys are free-form: the theme engine attaches no meaning to them, and a
    /// duplicate key overwrites the previous value.
    pub custom_properties: HashMap<String, String>,
}
impl StateTheme {
    /// Creates a theme with the three required colours and nothing else: no
    /// border, no shadow, fully opaque and no custom properties.
    pub fn new(background: Color, foreground: Color, text: Color) -> Self {
        Self {
            background_color: background,
            foreground_color: foreground,
            border_color: Color::TRANSPARENT,
            border_width: 0,
            text_color: text,
            shadow_color: None,
            shadow_offset: (0, 0),
            shadow_blur: 0,
            opacity: 1.0,
            custom_properties: HashMap::new(),
        }
    }
    /// Sets the border colour and thickness in pixels; a `width` of `0` leaves the
    /// border invisible. Builder-style, returns `self`.
    pub fn with_border(mut self, color: Color, width: u32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }
    /// Enables a drop shadow with the given colour, pixel `offset` as `(dx, dy)`
    /// and blur radius in pixels. Builder-style, returns `self`.
    pub fn with_shadow(mut self, color: Color, offset: (i32, i32), blur: u32) -> Self {
        self.shadow_color = Some(color);
        self.shadow_offset = offset;
        self.shadow_blur = blur;
        self
    }
    /// Sets overall opacity, clamping `opacity` into `0.0..=1.0` so an
    /// out-of-range value cannot be stored. Builder-style, returns `self`.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
    /// Stores an arbitrary extra property by `key`, replacing any previous value
    /// for that key. Both strings are copied. Builder-style, returns `self`.
    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.custom_properties.insert(key.to_string(), value.to_string());
        self
    }
}
impl Default for StateTheme {
    /// A plain opaque theme: white background, black foreground and text, no
    /// border or shadow. This is the theme returned for any [`WidgetState`] a
    /// [`StatefulTheme`] has not explicitly defined.
    fn default() -> Self {
        Self::new(Color::WHITE, Color::BLACK, Color::BLACK)
    }
}
/// A named theme holding one [`StateTheme`] per [`WidgetState`].
///
/// States are opt-in: any state not registered via [`Self::add_state`] resolves to
/// the single shared fallback theme, so a partially populated theme never panics
/// or returns nothing.
#[derive(Debug, Clone)]
pub struct StatefulTheme {
    name: String,
    states: HashMap<WidgetState, StateTheme>,
    default_state: StateTheme,
    /// Transition durations in milliseconds between state pairs (from, to).
    /// Stored here for future animation pipeline integration.
    transitions: HashMap<(WidgetState, WidgetState), u32>,
}
impl StatefulTheme {
    /// Creates an empty theme with the given `name` (used only for identification
    /// and lookup by the caller) and no registered states.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            states: HashMap::new(),
            default_state: StateTheme::default(),
            transitions: HashMap::new(),
        }
    }
    /// Registers the theme used when a widget is in `state`, replacing any previous
    /// theme for that state.
    pub fn add_state(&mut self, state: WidgetState, theme: StateTheme) {
        self.states.insert(state, theme);
    }
    /// Looks up the theme for `state`.
    ///
    /// Never fails: a state that was never registered yields a reference to the
    /// fallback theme set by [`Self::set_default_state`].
    pub fn get_state(&self, state: &WidgetState) -> &StateTheme {
        self.states.get(state).unwrap_or(&self.default_state)
    }
    /// Replaces the theme returned for every unregistered state.
    ///
    /// Does not affect states that have been explicitly registered.
    pub fn set_default_state(&mut self, theme: StateTheme) {
        self.default_state = theme;
    }
    /// Records a state-to-state transition duration in milliseconds.
    ///
    /// The pair is directed, so `(A, B)` and `(B, A)` are independent entries.
    /// Stored only; nothing in this module drives animations from it.
    pub fn set_transition(&mut self, from: WidgetState, to: WidgetState, duration_ms: u32) {
        self.transitions.insert((from, to), duration_ms);
    }
    /// Returns the transition duration in milliseconds for the directed pair
    /// `from` -> `to`, or `None` if none was recorded. Symmetric transitions are
    /// not implied.
    pub fn get_transition(&self, from: &WidgetState, to: &WidgetState) -> Option<u32> {
        self.transitions.get(&(*from, *to)).copied()
    }
    /// The theme's identifying name, as passed to [`Self::new`].
    pub fn name(&self) -> &str {
        &self.name
    }
}
/// How a [`ThemeStateManager`] chooses between its light and dark themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Always use the light theme. The default.
    #[default]
    Light,
    /// Always use the dark theme.
    Dark,
    /// Choose per [`ThemeStateManager::set_auto_switch`]; with no window
    /// configured, falls back to the light theme.
    Auto,
}
/// Pairs a light and a dark [`StatefulTheme`] and tracks which is active.
///
/// Owns both themes for the lifetime of the manager. Not reference-counted or
/// synchronised, so it is confined to one thread; the mode-changed callbacks are
/// `Rc<RefCell<..>>`-based and are invoked re-entrantly from [`Self::set_mode`].
pub struct ThemeStateManager {
    light_theme: StatefulTheme,
    dark_theme: StatefulTheme,
    current_mode: ThemeMode,
    auto_switch_threshold: Option<(u8, u8)>,
    /// Callbacks invoked when the theme mode changes.
    on_mode_changed: ModeChangedCallback,
}
impl ThemeStateManager {
    /// Creates a manager holding the two themes, starting in [`ThemeMode::Light`]
    /// with automatic switching disabled and no callbacks registered.
    pub fn new(light: StatefulTheme, dark: StatefulTheme) -> Self {
        Self {
            light_theme: light,
            dark_theme: dark,
            current_mode: ThemeMode::Light,
            auto_switch_threshold: None,
            on_mode_changed: Rc::new(RefCell::new(Vec::new())),
        }
    }
    /// Sets the active mode and notifies callbacks if it actually changed.
    ///
    /// Setting the mode it already has is a no-op as far as callbacks are
    /// concerned. Callbacks are invoked synchronously, with the callback list
    /// mutably borrowed, so a callback that re-enters `set_mode` will panic on the
    /// `RefCell` borrow.
    pub fn set_mode(&mut self, mode: ThemeMode) {
        let old_mode = self.current_mode;
        self.current_mode = mode;
        if old_mode != mode {
            let mut guard = self.on_mode_changed.borrow_mut();
            for callback in guard.iter_mut() {
                callback(mode);
            }
        }
    }
    /// The mode last set by [`Self::set_mode`], [`Self::toggle_mode`] or the
    /// initial [`ThemeMode::Light`].
    ///
    /// This is the *requested* mode; it is not rewritten when [`ThemeMode::Auto`]
    /// resolves to dark, so it can disagree with the theme [`Self::current_theme`]
    /// actually returns.
    pub fn current_mode(&self) -> ThemeMode {
        self.current_mode
    }
    /// Borrows whichever of the two themes is currently active.
    ///
    /// [`ThemeMode::Light`] and [`ThemeMode::Dark`] return their theme directly;
    /// [`ThemeMode::Auto`] consults the time window from [`Self::set_auto_switch`].
    pub fn current_theme(&self) -> &StatefulTheme {
        match self.current_mode {
            ThemeMode::Light => &self.light_theme,
            ThemeMode::Dark => &self.dark_theme,
            ThemeMode::Auto => {
                if self.should_use_dark() {
                    &self.dark_theme
                } else {
                    &self.light_theme
                }
            }
        }
    }
    /// Flips between light and dark, going through [`Self::set_mode`] so callbacks
    /// fire.
    ///
    /// Toggling from [`ThemeMode::Auto`] selects light rather than dark, which is
    /// not a strict inversion of the auto-resolved theme.
    pub fn toggle_mode(&mut self) {
        let new_mode = match self.current_mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Auto => ThemeMode::Light,
        };
        self.set_mode(new_mode);
    }
    /// Enables [`ThemeMode::Auto`] resolution using an hour window.
    ///
    /// `hour_start` and `hour_end` are UTC hours in `0..24`, and dark mode is
    /// chosen when the current hour is in `[hour_start, hour_end)`. The window
    /// does not wrap past midnight, so `(22, 6)` never selects dark; use
    /// `(0, 6)` plus `(22, 24)` semantics in application code if needed. Calling
    /// this does not by itself switch the mode to [`ThemeMode::Auto`].
    pub fn set_auto_switch(&mut self, hour_start: u8, hour_end: u8) {
        self.auto_switch_threshold = Some((hour_start, hour_end));
    }
    /// Resolves whether the automatic window currently calls for the dark theme.
    ///
    /// Compares against the system clock read as UTC whole hours from the Unix
    /// epoch, which is approximate for local-time expectations. Returns `false`
    /// when no window is configured or the clock is unavailable (an error is
    /// treated as the epoch).
    ///
    /// # Under `mini`
    ///
    /// The wall clock is not readable: `SystemTime` is `std`-only and `mini` has no
    /// `compat` clock that reports a calendar time (`compat::Instant` is monotonic
    /// and has no epoch). The `mini` arm therefore reports `false` — the same value
    /// the documented "clock is unavailable" case already produces, so a `mini`
    /// build keeps the light theme rather than guessing an hour.
    #[cfg(not(alloc_frugal))]
    fn should_use_dark(&self) -> bool {
        if let Some((start, end)) = self.auto_switch_threshold {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // UTC hour (approximate - good enough for dark mode toggle)
            let hour = ((now / 3600) % 24) as u8;
            hour >= start && hour < end
        } else {
            false
        }
    }
    /// Resolves whether the automatic window currently calls for the dark theme.
    ///
    /// See the `not(alloc_frugal)` definition above: no wall clock is available
    /// under `mini`, so this is always `false`.
    #[cfg(alloc_frugal)]
    fn should_use_dark(&self) -> bool {
        false
    }
    /// Looks up the [`StateTheme`] for `state` in the theme that is currently
    /// active, falling back to that theme's default state as
    /// [`StatefulTheme::get_state`] does.
    pub fn get_state_theme(&self, state: &WidgetState) -> &StateTheme {
        self.current_theme().get_state(state)
    }
    /// Registers a callback that is invoked when the theme mode changes.
    ///
    /// The callback receives the new `ThemeMode` value.
    /// Multiple callbacks can be registered; they will all be invoked
    /// in registration order when the mode changes.
    pub fn on_mode_changed<F>(&self, callback: F)
    where
        F: FnMut(ThemeMode) + 'static,
    {
        let mut guard = self.on_mode_changed.borrow_mut();
        guard.push(Box::new(callback));
    }
}
impl Default for ThemeStateManager {
    fn default() -> Self {
        let light = StatefulTheme::new("light");
        let dark = StatefulTheme::new("dark");
        Self::new(light, dark)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_state_theme() {
        let theme = StateTheme::new(Color::WHITE, Color::BLACK, Color::BLACK)
            .with_border(Color::GRAY, 1)
            .with_opacity(0.8);
        assert_eq!(theme.background_color, Color::WHITE);
        assert_eq!(theme.border_width, 1);
        assert_eq!(theme.opacity, 0.8);
    }
    #[test]
    fn test_stateful_theme() {
        let mut theme = StatefulTheme::new("test");
        let normal = StateTheme::new(Color::WHITE, Color::BLACK, Color::BLACK);
        let hover = StateTheme::new(Color::LIGHT_GRAY, Color::BLACK, Color::BLACK);
        theme.add_state(WidgetState::Normal, normal);
        theme.add_state(WidgetState::Hover, hover);
        assert!(theme.get_state(&WidgetState::Normal).background_color == Color::WHITE);
        assert!(theme.get_state(&WidgetState::Hover).background_color == Color::LIGHT_GRAY);
    }
    #[test]
    fn test_theme_manager() {
        let light = StatefulTheme::new("light");
        let dark = StatefulTheme::new("dark");
        let mut manager = ThemeStateManager::new(light, dark);
        assert_eq!(manager.current_mode(), ThemeMode::Light);
        manager.set_mode(ThemeMode::Dark);
        assert_eq!(manager.current_mode(), ThemeMode::Dark);
        manager.toggle_mode();
        assert_eq!(manager.current_mode(), ThemeMode::Light);
    }
    #[test]
    fn test_mode_changed_callback() {
        let light = StatefulTheme::new("light");
        let dark = StatefulTheme::new("dark");
        let mut manager = ThemeStateManager::new(light, dark);
        let fired = crate::compat::Rc::new(crate::compat::RefCell::new(false));
        let fired_clone = fired.clone();
        manager.on_mode_changed(move |_mode| {
            *fired_clone.borrow_mut() = true;
        });
        manager.set_mode(ThemeMode::Dark);
        let is_fired = *fired.borrow();
        assert!(is_fired, "callback should have been invoked on mode change");
    }
}
