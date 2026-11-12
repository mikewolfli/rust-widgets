use super::*;

/// Stroke width of the emphasis ring on a **default** button, in pixels.
///
/// Wider than the ordinary border so the emphasis reads as an emphasis rather than as a second
/// hairline next to the first, and drawn on the same inset as the focus ring so the two cannot
/// overlap. Named rather than inlined because the draw and its test both read it.
pub(crate) const DEFAULT_EMPHASIS_WIDTH: u32 = 2;

/// Button interaction state.
///
/// Derived from the enabled and pressed flags, never stored. Pressed and
/// disabled are exclusive with disabled taking precedence, and hover is not
/// represented at all — see [`Button::is_hovered`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Enabled and not pressed.
    Normal,
    /// Enabled and currently held down.
    Pressed,
    /// Not enabled; reported even if the pressed flag was set.
    Disabled,
}

/// Where the icon sits relative to the button's label.
///
/// The default is [`Leading`](Self::Leading), which is the historical picture: the icon's box
/// comes first and the label follows it. The other three slots let a caller express a toolbar
/// button (icon above the label), a trailing action (label first, icon last) or an icon below a
/// caption without composing two separate controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconPosition {
    /// The icon is the leading slot and the label follows it — the default.
    #[default]
    Leading,
    /// The label comes first and the icon follows it.
    Trailing,
    /// The icon is above the label.
    Top,
    /// The icon is below the label.
    Bottom,
}

impl IconPosition {
    /// Returns the token this position is published under.
    ///
    /// The property layer reads and writes exactly these spellings, so the token set is the
    /// enum's own vocabulary rather than a second table that could drift (WC6-05).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leading => "leading",
            Self::Trailing => "trailing",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    /// Parses an [`IconPosition`] from its published token, or `None` when the token names no
    /// position.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "leading" => Some(Self::Leading),
            "trailing" => Some(Self::Trailing),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// The inputs [`Button::implicit_size`] derives its answer from.
///
/// # Why the key is the inputs, not a bare dirty flag
///
/// A dirty flag has to be cleared by every path that can change one of these facts. Two of them
/// are not setters at all — the resolved font comes from the active theme and the text scale, and
/// the profile comes from the theme — so a flag would be wrong the first time a theme changed and
/// nobody remembered to clear it. Comparing the inputs cannot go stale: a different font, icon or
/// profile simply misses the cache and the answer is measured again.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ButtonHintKey {
    /// The caption's revision; see [`Button::text_revision`].
    pub(crate) text_revision: u32,
    /// The font the label is measured and painted with.
    pub(crate) font: Font,
    /// Whether any icon source is present, which is what the hint reserves room for.
    pub(crate) icon: bool,
    /// The icon slot's size, which the hint adds as advance and height.
    pub(crate) icon_size: u32,
    /// Where the icon sits relative to the label, which decides whether its size is added as
    /// a horizontal advance or as a stacked row.
    pub(crate) icon_position: IconPosition,
    /// The metrics profile whose button floor and padding decide the answer.
    pub(crate) profile: ControlMetricsProfile,
    /// The declared shortcut, whose formatted text the draw-time hint caches (WGT-F7).
    ///
    /// Part of the same key so a shortcut change invalidates both derived values at once; the
    /// intrinsic-size answer does not depend on it, so a shortcut change merely re-measures.
    pub(crate) shortcut: Option<crate::shortcut::Shortcut>,
}
/// Button widget for clickable actions.
///
/// # Animated interaction
///
/// The button interpolates its fill between the theme's resting and interactive colours instead of
/// switching instantly. `tick` advances the interpolation by a millisecond delta and reports whether
/// another frame is still needed — the contract this library uses for a self-animating control (see
/// `FloatingLabel`'s `tick`, where the shape was first written): a host that receives `false` stops
/// scheduling frames, so a settled button costs nothing per frame.
///
/// The duration is not a constant in this file. It comes from the active theme's `Motion::normal`,
/// so a theme can state its own tempo and a test can shorten it to reach the end state
/// deterministically.
pub struct Button {
    pub(crate) base: BaseWidget,
    pub(crate) text: String,
    #[cfg(feature = "image")]
    pub(crate) icon: Option<Image>,
    pub(crate) icon_source: Option<IconSource>,
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub(crate) shared_icon_pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    /// The icon's own colour channel, independent of the label ink.
    ///
    /// `None` keeps the pre-existing behaviour: a vector icon is painted in the label's colour and
    /// an image is drawn with its own colours. Shared with `ToolButton`
    /// ([`crate::widget::IconTint`]) so "tint an icon" has one definition and one cache strategy.
    pub(crate) icon_tint: IconTint,
    pub(crate) default_button: bool,
    /// The icon slot's size in logical pixels.
    ///
    /// [`dimensions::BUTTON_ICON_SIZE`] is the default, but the reference toolkit treats the icon
    /// size as a property of the button rather than of the widget class, so a control with a
    /// larger icon can say so.
    pub(crate) icon_size: u32,
    /// Where the icon sits relative to the label.
    ///
    /// Defaults to [`IconPosition::Leading`], so a button that never asks for a position keeps
    /// the historical leading slot.
    pub(crate) icon_position: IconPosition,
    /// The keyboard chord that activates this button, if one was declared.
    pub(crate) shortcut: Option<crate::shortcut::Shortcut>,
    /// Whether gaining focus makes this button the default button.
    pub(crate) auto_default: bool,
    /// Whether this button's default emphasis is exclusive with its siblings.
    pub(crate) auto_exclusive: bool,
    /// Whether the button paints without a face or frame.
    pub(crate) flat: bool,
    /// Whether the caller explicitly chose a corner radius.
    ///
    /// The draw path used to compare `style.border_radius` against the Material baseline and
    /// treat equality as "the theme wrote it". That made the baseline value unchoosable: a
    /// caller asking for Material's 4 px on a macOS profile got the profile's 6 px instead.
    /// This flag records authorship instead of guessing it from the value.
    pub(crate) border_radius_explicit: bool,
    /// How a label wider than the button is shortened.
    pub(crate) elide_mode: TextElideMode,
    /// The auto-repeat timing while this button is held.
    ///
    /// Off by default, so a button that never opts in still activates exactly once per press.
    /// Advanced from [`Button::tick`]; see [`AutoRepeat`] for the contract.
    pub(crate) auto_repeat: AutoRepeat,
    /// A monotonic counter bumped whenever the caption changes.
    ///
    /// [`Button::implicit_size`] caches its answer against this instead of against the string: a
    /// same-value `set_text` leaves the cache intact, and a real change invalidates it without the
    /// comparison being repeated on every draw.
    pub(crate) text_revision: u32,
    /// The cached intrinsic size, with the inputs it was derived from.
    ///
    /// `RefCell` rather than `Cell` because the key holds a [`Font`], which is not `Copy`. The
    /// borrow is confined to one lookup and one store, and nothing called from
    /// [`Button::implicit_size`] can reach this control, so a nested borrow cannot occur.
    pub(crate) hint_cache: RefCell<Option<(ButtonHintKey, Size)>>,
    /// The cached draw-time shortcut hint: its key, the formatted text and the font it was
    /// formatted for (WGT-F7).
    ///
    /// `format_shortcut` builds a `String` and `with_size` a `Font`; both were rebuilt on every
    /// frame a button with a shortcut was painted. The key is the one [`Button::implicit_size`]
    /// uses, so a text/font/profile change invalidates both caches together.
    pub(crate) shortcut_hint_cache: RefCell<Option<(ButtonHintKey, String, Font)>>,
    /// The attached drop-down menu's rows, when the `full_widgets` surface is built.
    ///
    /// A push button that owns a menu is the reference toolkit's `setMenu`: the menu is part of
    /// the button's behaviour, not a separate control the caller has to wire up.
    #[cfg(full_widgets)]
    pub(crate) menu_items: Vec<MenuItem>,
    /// Whether the attached menu is currently showing.
    #[cfg(full_widgets)]
    pub(crate) menu_open: bool,
    /// Where an open menu is anchored, when it was opened as a context menu.
    ///
    /// `None` means "below the button", which is where a click-opened menu belongs. A right-click
    /// opens the menu *at the pointer*, so that anchor is stored instead.
    #[cfg(full_widgets)]
    pub(crate) menu_anchor: Option<Point>,
    /// The open menu's active row, for keyboard activation.
    #[cfg(full_widgets)]
    pub(crate) active_menu_index: Option<usize>,
    /// How long a press must be held before the attached menu opens.
    ///
    /// The delayed-popup path `ToggleButton` already carried: holding a menu button opens
    /// its menu after this delay, while a quick click keeps the historical release
    /// behaviour. The timer only runs for a button that actually has a menu.
    #[cfg(full_widgets)]
    pub(crate) popup_delay_ms: u32,
    /// Elapsed milliseconds of the current held press, when a delayed popup is timing.
    #[cfg(full_widgets)]
    pub(crate) press_elapsed: Option<u32>,
    /// Whether a press on this button can start a cross-control drag.
    pub(crate) drag_enabled: bool,
    /// Whether this button accepts a dropped payload.
    pub(crate) accepts_drops: bool,
    /// Whether a drag payload is currently hovering this button.
    pub(crate) drop_hovered: bool,
    /// Emitted when a row of the attached menu is activated.
    #[cfg(full_widgets)]
    pub item_triggered: crate::signal::Signal1<u64>,
    /// The last value `pressed_signal`/`released_signal` were emitted for.
    ///
    /// # Why a second flag exists at all
    ///
    /// The *paint* flag is [`BaseWidget::is_pressed`], which is the crate's single source
    /// for "this control looks held". The two signals, though, are edges: they fire once
    /// per transition, not once per frame. A control that used the base flag as its own
    /// edge latch would find it already set — the base records it as the pointer goes
    /// down, before the control's arm runs — and would therefore stay silent.
    ///
    /// So this latch answers a *different* question from the base flag: "have I already
    /// announced the current value?" It is signal bookkeeping, not appearance, and the two
    /// cannot drift because this one is only ever compared to the base flag, never
    /// painted.
    pub(crate) signaled_pressed: bool,
    /// The interaction transition: `0.0` at rest, `1.0` fully at the hovered/pressed fill.
    ///
    /// The value actually painted, interpolated by [`Button::tick`]. Kept as a fraction
    /// rather than a colour so the target can change mid-flight — a press during a hover
    /// animation retargets the same transition instead of restarting it, which is what
    /// makes a quick press-and-release read as one movement rather than two fades.
    ///
    /// [`crate::style::PropertyDriver`] owns the interpolation, the theme-derived duration
    /// **and the target**. The target used to be a second field here
    /// (`interaction_target: f32`), which meant two controls carried a cache of the same
    /// fact that had to stay in step with `widget_state()` on its own; the driver holds it
    /// now, so there is no cache left to go stale (BLUE24 §2.3).
    pub(crate) interaction_progress: crate::style::PropertyDriver,
    /// Emitted on the rising edge of the pressed flag (button down).
    ///
    /// Suppressed entirely while the button is disabled, so a disabled button
    /// emits no press or release. A repeated press without an intervening
    /// release emits nothing.
    pub pressed_signal: GenericSignal,
    /// Emitted when a context menu is requested on this button, with the pointer position.
    ///
    /// A button has no rows to select, so the payload is all a host needs to place its menu. It is
    /// emitted only when the button has no attached menu of its own: a menu button opens that menu
    /// instead (the historical behaviour). This is the button half of feedback C-8 — the editor and
    /// tab-page surfaces are buttons, and neither could offer a right-click menu before.
    pub context_menu_requested: crate::signal::Signal1<Point>,
    /// Emitted on the falling edge of the pressed flag (button up).
    ///
    /// Like [`Button::pressed_signal`], suppressed while disabled — so disabling
    /// a button that is currently held down leaves the press unreleased rather
    /// than emitting a spurious release.
    pub released_signal: GenericSignal,
    /// Emitted with the recomputed [`ButtonState`] whenever the pressed or
    /// enabled flag changes. Not emitted when only the hover state changes.
    pub state_changed: Signal1<ButtonState>,
    /// Emitted when a press is abandoned rather than completed.
    ///
    /// # Why this is not folded into `released`
    ///
    /// `released` answers "the pointer came up", which is true for a drag that started
    /// on the button and ended off it. `canceled` answers "this gesture will not
    /// activate", which is the fact a caller routing a destructive action needs. A
    /// button that reported both as `released` would leave the caller unable to tell a
    /// completed activation from an abandoned one — the distinction the shared table draws
    /// by reporting a cancel rather than an activation.
    pub canceled: GenericSignal,
    /// The registry command this button presents, when it was resolved from `ActionManager`.
    ///
    /// The id is what a host routes a `clicked` to; the live cell beside it is the state the
    /// command's own signals update, so a second host of the same command stays in step.
    #[cfg(full_widgets)]
    pub(crate) action_id: Option<String>,
    /// Live state cell subscribed to the linked command, when one is bound.
    #[cfg(full_widgets)]
    pub(crate) live_action: Option<Arc<Mutex<crate::action::ActionLiveState>>>,
}
