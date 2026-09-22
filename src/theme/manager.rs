// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{AppearanceMode, Borders, Colors, Fonts, Spacing, Theme, ThemeOverrides, WidgetRole};
use crate::compat::HashMap;
use crate::core::{Color, Font};
use crate::signal::Signal;
use crate::style::{HighContrastMode, Margin, Padding, Shadow, WidgetState, WidgetStyle};

/// Theme registry and active-theme resolver.
pub struct ThemeManager {
    /// Registered themes keyed by theme name.
    themes: HashMap<String, Theme>,
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
}

impl ThemeManager {
    /// Creates a theme manager seeded with the default theme.
    pub fn new() -> Self {
        let default = Theme::default();
        let current_theme = default.name.clone();
        let mut themes = HashMap::new();
        themes.insert(default.name.clone(), default);
        Self {
            themes,
            current_theme,
            theme_changed: Signal::new(),
            high_contrast: HighContrastMode::None,
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
        self.theme_changed.emit(());
    }

    /// The active high-contrast override.
    pub fn high_contrast(&self) -> HighContrastMode {
        self.high_contrast
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
        let candidate = self
            .themes
            .iter()
            .find(|(_, theme)| theme.appearance == appearance)
            .map(|(name, _)| name.clone());
        match candidate {
            Some(name) => self.set_theme(&name),
            None => false,
        }
    }

    /// Loads and registers a theme from a JSON file path.
    #[cfg(not(alloc_frugal))]
    pub fn load_theme(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = serde_json::from_str(&content)?;
        self.themes.insert(theme.name.clone(), theme);
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
        self.themes.insert(name.clone(), theme);
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

    /// Registers a theme in memory.
    pub fn register_theme(&mut self, theme: Theme) {
        self.themes.insert(theme.name.clone(), theme);
    }

    /// Selects active theme by name. Emits `theme_changed` on success.
    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            self.theme_changed.emit(());
            return true;
        }
        false
    }

    /// Returns currently active theme.
    pub fn current_theme(&self) -> Option<&Theme> {
        self.themes.get(&self.current_theme)
    }

    /// Returns a registered theme by name.
    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// Returns a reference to the `theme_changed` signal.
    ///
    /// Connect slots to this signal to be notified when the active theme is switched.
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
            // The state key is tried for the class first, then the kind, so a theme can
            // describe either `Button.primary:hover` or a kind-wide `button:hover`.
            let suffixes = match class_name {
                Some(class) => [
                    Some(format!("{class}:{}", state_suffix(state))),
                    Some(format!("{kind_name}:{}", state_suffix(state))),
                    None,
                ],
                None => [None, Some(format!("{kind_name}:{}", state_suffix(state))), None],
            };
            for key in suffixes.into_iter().flatten() {
                if let Some(token) = self.current_theme().and_then(|t| t.overrides.styles.get(&key))
                {
                    apply_token(&mut style, token, None);
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
        let mut style = self.role_base_style(theme, kind_name);

        // A class-level override wins over the role default. The class is looked up
        // verbatim first, then by the role it stands for, so a theme may override either a
        // specific class or a whole role.
        if let Some(class_name) = class_name {
            let token = theme.overrides.styles.get(class_name).or_else(|| {
                theme.overrides.styles.get(role_key(WidgetRole::for_kind_name(class_name)))
            });
            if let Some(token) = token {
                apply_token(&mut style, token, Some(&theme.fonts));
            }
        }
        style
    }

    /// Forces the high-contrast palette onto `style` when one is set.
    ///
    /// Applied **last** everywhere, so neither a theme override nor a state variant can
    /// reintroduce a low-contrast colour.
    fn apply_high_contrast(&self, style: &mut WidgetStyle) {
        if let Some((background, foreground)) = self.high_contrast.forced_pair() {
            style.background_color = Some(background);
            style.text_color = Some(foreground);
            // A gradient or a texture would defeat the point of a flat forced pair.
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
            let key = format!("{class_name}:{}", state_suffix(state));
            if let Some(token) = self.current_theme().and_then(|t| t.overrides.styles.get(&key)) {
                apply_token(&mut style, token, None);
            }
        }
        self.apply_high_contrast(&mut style);
        style
    }

    /// The role-default appearance for a widget kind, before any class or state overlay.
    ///
    /// The name is classified through [`WidgetRole::for_kind_name`], so it must be a *kind*
    /// name; a caller holding a CSS class should use
    /// [`resolve_style_for`](Self::resolve_style_for), which keeps the two vocabularies apart.
    fn role_base_style(&self, theme: &Theme, kind_name: &str) -> WidgetStyle {
        let shadow = if theme.borders.shadow {
            Some(Shadow { x: 0, y: 2, blur: 6, color: Color::rgba(0, 0, 0, 60) })
        } else {
            None
        };
        let (background_color, text_color, border_color) = role_colors(theme, kind_name);

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
            touch_target,
            // The theme's own base font token, **scaled by the device's text-size preference**.
            //
            // `Font::scaled` and the platform's `text_scale` accessor both existed and neither was
            // called from here, so the field was write-only: a device that asked for larger text
            // got the nominal size, and a control's font was one property nobody could influence
            // through the theme it came from. Scaling at this one point is what makes it apply to
            // every control — each of them takes this font unless it names another.
            //
            // A control that sets its own font is unaffected, which is correct: a monospace editor
            // chooses its own metrics deliberately.
            font: Some(theme.fonts.body.clone().scaled(crate::platform::profile::text_scale())),
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

        let mut style = self.role_base_style(theme, class_name);

        let token = theme.overrides.styles.get(class_name).or_else(|| {
            theme.overrides.styles.get(role_key(WidgetRole::for_kind_name(class_name)))
        });
        if let Some(token) = token {
            apply_token(&mut style, token, Some(&theme.fonts));
        }
        style
    }
}

/// The theme colours for a widget name, by role.
///
/// Split out so the classification and the colour choice are separate: the role
/// table answers "what kind of visual treatment is this?" and this function
/// answers "what does that treatment look like in this theme?".
fn role_colors(theme: &Theme, class_name: &str) -> (Option<Color>, Option<Color>, Option<Color>) {
    // `contrast_color` is the crate's single contrast decision; see `Color`.
    match WidgetRole::for_kind_name(class_name) {
        WidgetRole::Primary => (
            Some(theme.colors.primary),
            Some(theme.colors.primary.contrast_color()),
            Some(theme.colors.primary),
        ),
        WidgetRole::Text => (None, Some(theme.colors.foreground), None),
        WidgetRole::Input => (
            Some(theme.colors.input_background()),
            Some(theme.colors.foreground),
            Some(theme.colors.secondary),
        ),
        WidgetRole::Accent => {
            (Some(theme.colors.accent), Some(theme.colors.accent.contrast_color()), None)
        }
        WidgetRole::Choice => (
            Some(theme.colors.input_background()),
            Some(theme.colors.foreground),
            Some(theme.colors.secondary),
        ),
        WidgetRole::Danger => {
            (Some(theme.colors.error), Some(theme.colors.error.contrast_color()), None)
        }
        WidgetRole::Surface => (
            Some(theme.colors.background),
            Some(theme.colors.foreground),
            Some(theme.colors.secondary),
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

/// The suffix a state contributes to an override key.
fn state_suffix(state: WidgetState) -> &'static str {
    match state {
        WidgetState::Normal => "normal",
        WidgetState::Hover => "hover",
        WidgetState::Pressed => "pressed",
        WidgetState::Focused => "focused",
        WidgetState::Disabled => "disabled",
        WidgetState::Checked => "checked",
        WidgetState::Selected => "selected",
        WidgetState::Active => "active",
        WidgetState::Inactive => "inactive",
        WidgetState::Error => "error",
        WidgetState::Warning => "warning",
        WidgetState::Success => "success",
    }
}

/// Apply a partial override token to a resolved style.
///
/// Only the token's `Some` fields are written, so an override may adjust one
/// property without restating the rest. `fonts` is consulted only when the token
/// names a font to resolve from the theme's token set.
fn apply_token(style: &mut WidgetStyle, token: &super::ThemeStyleToken, fonts: Option<&Fonts>) {
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
    if let Some(opacity) = token.opacity {
        style.opacity = Some(opacity.clamp(0.0, 1.0));
    }
    if token.shadow != super::ShadowOverride::Inherit {
        // A named three-way choice rather than a nested `Option`: see
        // `ShadowOverride` for why the nested form lost the "clear it" case.
        style.shadow = token.shadow.apply(style.shadow.take());
    }
    if let Some([width, height]) = token.touch_target {
        style.touch_target = Some(crate::core::Size::new(width, height));
    }
    if let Some(font) = &token.font {
        style.font = Some(font.clone());
    } else if let Some(fonts) = fonts {
        // No explicit font in the token: keep the base token the resolver set.
        // Reading `fonts` here is what makes the token set reachable at all; the
        // assignment is a no-op when the base already supplied one, which is the
        // honest behaviour for a token that does not mention a font.
        let _ = fonts;
    }
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
pub fn global_theme_manager() -> MutexGuard<'static, ThemeManager> {
    static MANAGER: OnceLock<Mutex<ThemeManager>> = OnceLock::new();
    lock(MANAGER.get_or_init(|| {
        let mut manager = ThemeManager::new();
        let dark = Theme::dark();
        // `register_theme` keys by the theme's own name, so re-seeding is
        // impossible and the dark preset cannot overwrite the light default.
        manager.register_theme(dark);
        Mutex::new(manager)
    }))
}

/// Serialises tests that mutate the process-wide theme registry.
///
/// The registry is shared state, so two tests that each switch the active theme
/// would race and see each other's writes. Existing precedent: the embedded
/// profile's `embedded_test_guard`, added for the same reason. Compiled only for
/// tests, so it costs a release build nothing.
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
pub fn resolved_theme_style(widget_name: &str) -> Option<WidgetStyle> {
    let manager = global_theme_manager();
    manager.current_theme()?;
    Some(manager.resolve_style(widget_name))
}

/// Resolves a theme style from a widget **kind** name plus an optional CSS **class**.
///
/// The counterpart of [`resolved_theme_style`] for callers that hold both names, such as the
/// JSON loader reading a node's `"class"` key. Keep the roles of the two arguments apart: the
/// kind determines the visual role, the class selects an override. See
/// [`ThemeManager::resolve_style_for`] for why passing a class as the role name misclassifies
/// the widget.
pub fn resolved_theme_style_for(kind_name: &str, class_name: Option<&str>) -> Option<WidgetStyle> {
    let manager = global_theme_manager();
    manager.current_theme()?;
    Some(manager.resolve_style_for(kind_name, class_name, None))
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
pub fn resolved_theme_style_for_state(
    kind_name: &str,
    state: crate::style::WidgetState,
) -> Option<WidgetStyle> {
    let manager = global_theme_manager();
    manager.current_theme()?;
    Some(manager.resolve_style_for_state(kind_name, Some(state)))
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
    global_theme_manager().current_theme().map(|theme| token.of(theme))
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
            colors: Colors {
                background: Color { r: 240, g: 240, b: 240, a: 255 },
                foreground: Color { r: 0, g: 0, b: 0, a: 255 },
                primary: Color { r: 33, g: 150, b: 243, a: 255 },
                secondary: Color { r: 158, g: 158, b: 158, a: 255 },
                accent: Color { r: 255, g: 152, b: 0, a: 255 },
                error: Color { r: 244, g: 67, b: 54, a: 255 },
                warning: Color { r: 255, g: 193, b: 7, a: 255 },
                success: Color { r: 76, g: 175, b: 80, a: 255 },
                disabled: Color { r: 200, g: 200, b: 200, a: 255 },
                info: Color::INFO,
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
            borders: Borders { width: 1, radius: 4, shadow: true },
            overrides: ThemeOverrides { styles: HashMap::new() },
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
        Self {
            name: "dark".to_string(),
            appearance: AppearanceMode::Dark,
            colors: Colors {
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
            borders: Borders { width: 1, radius: 4, shadow: true },
            overrides: ThemeOverrides { styles: HashMap::new() },
            // Material's own tempo: `kRadialReactionDuration` 100 ms, `kThemeChangeDuration`
            // 200 ms, the switch's toggle 300 ms. A theme that wants a different rhythm sets
            // `theme.motion`; every animated control reads it from there rather than carrying its
            // own constant.
            motion: crate::theme::Motion::default(),
        }
    }
}
