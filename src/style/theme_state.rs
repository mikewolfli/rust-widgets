// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

/// The state layers a control should **overlay** on top of its resolved fill.
///
/// # Why this is separate from [`WidgetState`]
///
/// [`WidgetState`] answers "which colour set do I use" — a single value, usable as a theme
/// lookup key, and something a theme can define a transition for. It cannot express
/// "focused **and** hovered at once" without the theme carrying a rule for every combination
/// (the Cartesian product that makes state-set transitions explode).
///
/// This struct answers the second, independent question: "what else do I lay on top?". Its
/// members may all be true together, and they are **draw** instructions rather than lookup
/// keys, so they are not transitioned — a hover blend and a focus ring are two separate
/// channels that never fight for the same pixel. That is the split every toolkit with both
/// a fill state and a focus ring arrives at: a fill answers to one state at a time, the ring
/// is drawn over whatever fill won.
///
/// A control that reports `Hover` from [`crate::widget::Widget::widget_state`] should also
/// set [`StateOverlay::hovered`], so a draw path that wants both the theme's hover colour
/// **and** a focus ring does not have to re-derive one of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateOverlay {
    /// The pointer is over the control: blend the fill a step toward the content colour.
    pub hovered: bool,
    /// An active press: blend the fill a firmer step toward the content colour.
    pub pressed: bool,
    /// Keyboard focus that warrants a ring. Painted as a ring, **not** as a fill.
    pub focused: bool,
    /// A persistent on/selected fact the control wants reflected as well.
    pub checked: bool,
}

/// One of the independent layers [`StateOverlay`] can carry.
///
/// A named set rather than four `bool` arguments, so a caller asking "which layer wins?"
/// cannot transpose two flags, and so [`StateOverlay::layer_weight`] has one spelling per
/// layer to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayLayer {
    /// The pointer is over the control.
    Hovered,
    /// The control is being held down.
    Pressed,
    /// Keyboard focus worth a ring.
    Focused,
    /// A latched on/selected fact.
    Checked,
}

impl OverlayLayer {
    /// Every layer, in the order the vocabulary is documented.
    pub const ALL: &'static [OverlayLayer] =
        &[Self::Hovered, Self::Pressed, Self::Focused, Self::Checked];

    /// The layer's precedence, higher meaning later on the pixel.
    pub const fn weight(self) -> u8 {
        match self {
            Self::Hovered => 1,
            Self::Pressed => 2,
            // A latch persists after the gesture that made it, so it outranks both gestures.
            Self::Checked => 3,
            // The focus ring is the accessibility channel: it is drawn over every fill.
            Self::Focused => 4,
        }
    }
}

impl StateOverlay {
    /// The relative precedence of one overlay layer while several hold at once.
    ///
    /// # What the number is (and is not)
    ///
    /// This is an **ordering**, not an opacity: the fill blend factors live in
    /// [`Self::fill_blend`], and they answer a different question (how far to move a colour).
    /// A draw path that has to decide which of several simultaneous facts gets the last word —
    /// a focus ring over a checked fill, a press over a hover — reads this instead of inventing
    /// an order at the call site. `0` means the layer is not set at all.
    ///
    /// The order is: a press outranks a hover (it already implies one), a latched check outranks
    /// both (it persists after the gesture), and the focus ring has the last word because it is
    /// the accessibility channel and must stay visible over every fill.
    pub const fn layer_weight(&self, layer: OverlayLayer) -> u8 {
        let set = match layer {
            OverlayLayer::Hovered => self.hovered,
            OverlayLayer::Pressed => self.pressed,
            OverlayLayer::Focused => self.focused,
            OverlayLayer::Checked => self.checked,
        };
        if set {
            layer.weight()
        } else {
            0
        }
    }

    /// The layer that wins when several hold at once, or `None` for an empty overlay.
    ///
    /// Ties are impossible because every layer has a distinct weight, so the answer does not
    /// depend on iteration order.
    pub fn dominant_layer(&self) -> Option<OverlayLayer> {
        OverlayLayer::ALL
            .iter()
            .copied()
            .max_by_key(|layer| self.layer_weight(*layer))
            .filter(|layer| self.layer_weight(*layer) > 0)
    }

    /// The overlay a control's base state alone implies.
    ///
    /// Built from the four facts [`crate::widget::BaseWidget`] maintains, so a control that
    /// overrides nothing still gets the hover/press/focus overlay for free. `checked` is
    /// not derivable from those facts — a control that latches a value adds it itself.
    pub fn from_base(hovered: bool, pressed: bool, focused: bool) -> Self {
        Self { hovered, pressed, focused, checked: false }
    }

    /// Whether any layer is set, i.e. whether a draw path has anything to add.
    pub fn is_empty(&self) -> bool {
        !(self.hovered || self.pressed || self.focused || self.checked)
    }

    /// The fill blend factor this overlay asks for, in `0.0..=1.0`.
    ///
    /// A press is a firmer step than a hover, and the two are mutually exclusive here
    /// because a press already implies the pointer is down on the control. The values match
    /// the ones the preset theme's `"<kind>:hover"` / `"<kind>:pressed"` overrides derive,
    /// so an overlaid control and a theme-driven one move by the same amount.
    pub fn fill_blend(&self) -> f32 {
        if self.pressed {
            0.12
        } else if self.hovered {
            0.08
        } else {
            0.0
        }
    }
}

/// The state of **meaning** a widget carries, independent of how it is being interacted with.
///
/// # Why this is a third channel and not two more [`WidgetState`] variants
///
/// [`WidgetState`] answers "which colour set do I use" and is a single value (see its own docs
/// for why a set would make state transitions explode). `Error` / `Warning` / `Success` are of a
/// different kind: they are **not another interaction**, they are extra information about the
/// interaction that is already happening. A refusal that is hovered is *both* hovering and
/// refused, and it must look like both.
///
/// Squeezing them onto the same single-valued chain is what made a validation message disappear
/// while the pointer was over the field that failed — which is precisely when the user is looking
/// for it. That is the shape [`StateOverlay`] was introduced to fix for the *fill*; this type is
/// the same split for the **border**: the fill answers to the interaction, the border to the
/// meaning.
///
/// # How it is consumed
///
/// A control reports this from [`crate::widget::Widget::semantic_state`]. A draw path takes the
/// fill from [`crate::widget::Widget::widget_state`] and the border colour from this when it is
/// not [`SemanticState::None`], so `"line_edit:error"` and `"line_edit:hover"` can both apply to
/// one control in one frame instead of overwriting each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum SemanticState {
    /// No extra meaning: the interaction state alone describes the control.
    #[default]
    None,
    /// A validation refusal. The border takes [`crate::theme::Colors::error`], not the fill.
    Error,
    /// A non-fatal warning worth surfacing without claiming failure.
    Warning,
    /// Confirmation of a completed action.
    Success,
}

impl SemanticState {
    /// The `"<kind>:<state>"` suffix this semantic state is addressed by, or `None` for
    /// [`SemanticState::None`].
    ///
    /// The spelling is **not** repeated here: each meaning maps to the [`WidgetState`] that
    /// carries it and takes that state's canonical suffix, so the three words have exactly one
    /// definition ([`WidgetState::state_suffix`]) that the theme's key builder and the CSS
    /// parser both read (G-405, principle #54).
    pub fn state_suffix(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Error => Some(WidgetState::Error.state_suffix()),
            Self::Warning => Some(WidgetState::Warning.state_suffix()),
            Self::Success => Some(WidgetState::Success.state_suffix()),
        }
    }
}

/// The interaction state a widget is painted in.
///
/// This is the single value a draw path resolves its fill from — [`crate::widget::Widget::widget_state`]
/// reports whichever state best describes the widget right now, and the caller decides which one
/// wins when several apply (a widget can be both focused and hovered, so the states are not
/// mutually exclusive in reality).
///
/// # The three semantic variants are carried by [`SemanticState`]
///
/// [`Self::Error`], [`Self::Warning`] and [`Self::Success`] are **frozen**: they remain in this
/// enum for the public shape and the theme keys that already name them, but no control returns
/// them from [`crate::widget::Widget::widget_state`] any more — that method's default and every
/// override report an *interaction*. A control that has a meaning to report says so through
/// [`crate::widget::Widget::semantic_state`], which is orthogonal to the interaction and so can
/// hold at the same time as `Hover`/`Pressed`/`Focused`. The two are resolved on separate
/// channels (fill from here, border from there), so there is no second source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum WidgetState {
    /// Resting state with no interaction. The default.
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
    ///
    /// Carried by [`SemanticState::Error`]; `widget_state()` never returns it. See this enum's
    /// docs for why the meaning channel is separate from the interaction channel.
    Error,
    /// The widget is displaying a non-fatal warning.
    ///
    /// Carried by [`SemanticState::Warning`]; `widget_state()` never returns it.
    Warning,
    /// The widget is confirming a completed action.
    ///
    /// Carried by [`SemanticState::Success`]; `widget_state()` never returns it.
    Success,
}

impl WidgetState {
    /// Every state, in the order the vocabulary is documented.
    ///
    /// This is the **single spelling source** for the widget-state vocabulary (G-405). The
    /// theme's `"<kind>:<state>"` key builder, the CSS pseudo-class parser and
    /// [`SemanticState::state_suffix`] all resolve through [`Self::state_suffix`] /
    /// [`Self::from_state_suffix`] rather than each carrying a `match` of their own, so adding a
    /// state cannot leave one of the four call sites behind.
    pub const ALL: &'static [WidgetState] = &[
        Self::Normal,
        Self::Hover,
        Self::Pressed,
        Self::Focused,
        Self::Disabled,
        Self::Checked,
        Self::Selected,
        Self::Active,
        Self::Inactive,
        Self::Error,
        Self::Warning,
        Self::Success,
    ];

    /// The canonical `"<kind>:<state>"` suffix, for example `"hover"`.
    ///
    /// Every state has exactly one suffix and every suffix names exactly one state; the round
    /// trip is pinned by `every_state_has_one_round_tripping_suffix`.
    pub const fn state_suffix(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Hover => "hover",
            Self::Pressed => "pressed",
            Self::Focused => "focused",
            Self::Disabled => "disabled",
            Self::Checked => "checked",
            Self::Selected => "selected",
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Success => "success",
        }
    }

    /// Resolves a state suffix to the state it names.
    ///
    /// Canonical suffixes are tried first, so a state can never be shadowed by an alias of
    /// another state. The aliases after that are spellings stylesheet authors already use in
    /// this crate and that mean the same state; they are input spellings only, and
    /// [`Self::state_suffix`] never produces them.
    ///
    /// # Why `:active` is not an alias for `Pressed`
    ///
    /// It used to be, which made [`Self::Active`] unspellable from CSS even though the theme's
    /// `"<kind>:active"` key names it. With one table driving both directions the canonical
    /// spelling has to win, so a press is spelled `:pressed` and `:active` now selects the
    /// active/foreground state — the same state the theme key means (G-405).
    pub fn from_state_suffix(name: &str) -> Option<Self> {
        if let Some(state) = Self::ALL.iter().copied().find(|state| state.state_suffix() == name) {
            return Some(state);
        }
        match name {
            "focus" => Some(Self::Focused),
            "invalid" => Some(Self::Error),
            "valid" => Some(Self::Success),
            _ => None,
        }
    }
}
/// How a theme-mode consumer chooses between a light and a dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Always use the light theme. The default.
    #[default]
    Light,
    /// Always use the dark theme.
    Dark,
    /// Follow the installed device preference (see
    /// [`crate::style::environment::environment`]); an uninstalled or `Auto` device
    /// preference resolves to light.
    Auto,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    /// The default meaning is the no-meaning variant, so a control that never overrides
    /// `semantic_state` carries nothing and draws as it did before the channel existed.
    #[test]
    fn the_default_semantic_state_is_no_meaning() {
        assert_eq!(SemanticState::default(), SemanticState::None);
    }

    /// Each meaning resolves to the same `"<kind>:<state>"` vocabulary the interaction states use,
    /// so a theme author writes one form of override; and `None` names no key, so a control with no
    /// meaning cannot be handed an override it did not ask for.
    #[test]
    fn each_meaning_addresses_a_theme_key() {
        assert_eq!(SemanticState::None.state_suffix(), None);
        assert_eq!(SemanticState::Error.state_suffix(), Some("error"));
        assert_eq!(SemanticState::Warning.state_suffix(), Some("warning"));
        assert_eq!(SemanticState::Success.state_suffix(), Some("success"));
    }

    /// G-405: one table drives both directions. Every state appears exactly once, its
    /// canonical suffix round-trips, and the semantic meanings read the same table rather than
    /// repeating their own words.
    #[test]
    fn every_state_has_one_round_tripping_suffix() {
        let mut seen: Vec<&str> = Vec::new();
        for state in WidgetState::ALL {
            let suffix = state.state_suffix();
            assert!(!seen.contains(&suffix), "`{suffix}` is claimed by more than one state");
            seen.push(suffix);
            assert_eq!(
                WidgetState::from_state_suffix(suffix),
                Some(*state),
                "`{suffix}` must resolve back to the state that spells it"
            );
        }
        assert_eq!(seen.len(), 12, "the documented vocabulary has twelve states");
        assert_eq!(SemanticState::Error.state_suffix(), Some(WidgetState::Error.state_suffix()));
    }

    /// The documented aliases resolve, and an unknown spelling is refused rather than guessed
    /// (the former maps every unknown pseudo-class onto a resting control).
    #[test]
    fn aliases_resolve_and_unknown_names_are_refused() {
        assert_eq!(WidgetState::from_state_suffix("focus"), Some(WidgetState::Focused));
        assert_eq!(WidgetState::from_state_suffix("invalid"), Some(WidgetState::Error));
        assert_eq!(WidgetState::from_state_suffix("valid"), Some(WidgetState::Success));
        assert_eq!(WidgetState::from_state_suffix("erorr"), None);
        // A press is `pressed`; `active` names the canonical Active state, not a press.
        assert_eq!(WidgetState::from_state_suffix("pressed"), Some(WidgetState::Pressed));
        assert_eq!(WidgetState::from_state_suffix("active"), Some(WidgetState::Active));
    }

    /// The semantic variants stay in the public shape (BLUE24 §3.4's ruling: keep and freeze),
    /// so a theme file that names `"<kind>:error"` keeps loading.
    #[test]
    fn the_semantic_widget_states_are_still_present() {
        // If any of these were removed, this would not compile -- which is the point.
        let _ = [WidgetState::Error, WidgetState::Warning, WidgetState::Success];
    }

    /// The overlay carries states that hold at once, which is the whole reason it exists
    /// alongside the single-valued [`WidgetState`].
    #[test]
    fn overlay_carries_simultaneous_states() {
        let overlay = StateOverlay::from_base(true, false, true);
        assert!(overlay.hovered && overlay.focused, "focused and hovered at once");
        assert!(!overlay.is_empty());
        assert_eq!(overlay.fill_blend(), 0.08, "a hover is the lighter step");
    }

    /// A press outranks a hover for the fill, and an idle control asks for no blend.
    #[test]
    fn overlay_fill_blend_follows_press_over_hover() {
        let pressed = StateOverlay { pressed: true, hovered: true, ..StateOverlay::default() };
        assert_eq!(pressed.fill_blend(), 0.12, "a press is the firmer step");
        assert_eq!(StateOverlay::default().fill_blend(), 0.0);
        assert!(StateOverlay::default().is_empty());
    }

    /// STY-006: every combination of overlay facts has a defined precedence, including the two
    /// layers that used to have no weight at all.
    #[test]
    fn every_overlay_layer_has_a_distinct_weight_and_a_combination_order() {
        let mut weights: Vec<u8> = Vec::new();
        for layer in OverlayLayer::ALL {
            let weight = layer.weight();
            assert!(!weights.contains(&weight), "{layer:?} must not share a weight");
            weights.push(weight);
        }
        assert_eq!(OverlayLayer::ALL.len(), 4, "hovered, pressed, focused and checked");

        let all = StateOverlay { hovered: true, pressed: true, focused: true, checked: true };
        assert_eq!(all.layer_weight(OverlayLayer::Checked), 3);
        assert_eq!(all.layer_weight(OverlayLayer::Focused), 4);
        assert_eq!(all.dominant_layer(), Some(OverlayLayer::Focused));

        // The absent layer weighs nothing, so a combination is never handed a phantom layer.
        let resting = StateOverlay::default();
        for layer in OverlayLayer::ALL {
            assert_eq!(resting.layer_weight(*layer), 0);
        }
        assert_eq!(resting.dominant_layer(), None);

        let checked_only = StateOverlay { checked: true, ..StateOverlay::default() };
        assert_eq!(checked_only.dominant_layer(), Some(OverlayLayer::Checked));
    }
}
