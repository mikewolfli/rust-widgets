// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Snackbar widget.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Horizontal padding of a snackbar's message from the bar's leading edge: 10.
///
/// One value for the message inset and the action button's trailing margin, so the two ends of
/// the bar are inset by the same amount rather than by `10` on one side and `14` on the other.
const SNACKBAR_PADDING_H: i32 = 10;

/// A snackbar action button's width: 76.
const SNACKBAR_ACTION_WIDTH: u32 = 76;

/// A snackbar action button's height: 20.
const SNACKBAR_ACTION_HEIGHT: u32 = 20;

/// The thickness of a snackbar's progress rule: 3.
const SNACKBAR_PROGRESS_HEIGHT: u32 = 3;

/// The default cap on messages waiting behind the visible one.
const SNACKBAR_MAX_QUEUED: usize = 4;

/// Where the bar sits within the control's rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnackbarAnchor {
    /// At the top of the control.
    Top,
    /// Vertically centred, the historical default.
    #[default]
    Center,
    /// At the bottom of the control.
    Bottom,
}

impl SnackbarAnchor {
    /// The property spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            SnackbarAnchor::Top => "top",
            SnackbarAnchor::Center => "center",
            SnackbarAnchor::Bottom => "bottom",
        }
    }

    /// Parses a property spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "top" => SnackbarAnchor::Top,
            "center" => SnackbarAnchor::Center,
            "bottom" => SnackbarAnchor::Bottom,
            _ => return None,
        })
    }
}

/// Snackbar with optional action and progress display.
pub struct Snackbar {
    base: BaseWidget,
    message: String,
    action_label: Option<String>,
    /// The stable identity reported when the action is triggered.
    action_id: Option<String>,
    visible: bool,
    /// The bar's enter/exit reveal, so it rises from the bottom edge rather than blinking in.
    ///
    /// A snackbar is the one surface in a window that arrives on its own initiative, so it is the
    /// control where an entrance matters most: appearing instantly reads as a layout glitch, while
    /// rising reads as "something has just been said to you". The flag above stays the authority
    /// (it is what `is_visible` and `dismiss` answer with); this says how far the transition got.
    reveal: PopupReveal,
    progress: Option<f32>,
    /// How long the bar stays up after `show`, in milliseconds. Zero means it stays until
    /// dismissed.
    duration_ms: u32,
    /// Milliseconds elapsed since the bar was shown.
    elapsed_ms: u32,
    /// Whether progress is drawn as an indeterminate sweep rather than a fraction.
    indeterminate: bool,
    /// The indeterminate sweep's phase, in `0.0..=1.0`.
    indeterminate_phase: f32,
    /// Whether additional `show` calls queue behind the visible message.
    queueing: bool,
    /// The maximum number of messages held in the queue.
    max_queued: usize,
    /// Messages waiting behind the visible one: message, action label and action id.
    queue: Vec<(String, Option<String>, Option<String>)>,
    /// Where the bar sits in the control's rectangle.
    anchor: SnackbarAnchor,
    /// Emitted when action is triggered.
    pub action_triggered: Signal1<String>,
    /// Emitted when snackbar is dismissed.
    pub dismissed: Signal1<()>,
}

impl Snackbar {
    /// Creates hidden snackbar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "Snackbar"),
            message: String::new(),
            action_label: None,
            action_id: None,
            visible: false,
            reveal: PopupReveal::new(false),
            progress: None,
            duration_ms: 0,
            elapsed_ms: 0,
            indeterminate: false,
            indeterminate_phase: 0.0,
            queueing: false,
            max_queued: SNACKBAR_MAX_QUEUED,
            queue: Vec::new(),
            anchor: SnackbarAnchor::default(),
            action_triggered: Signal1::new(),
            dismissed: Signal1::new(),
        }
    }

    /// Returns visibility.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns current message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Shows snackbar without action.
    pub fn show(&mut self, message: impl Into<String>) {
        self.enqueue_or_show(message.into(), None, None);
    }

    /// Shows snackbar with action button.
    ///
    /// The action's id is its label, which keeps the historical single-string spelling and
    /// makes a plain label a valid identity. Use [`Self::show_with_action_id`] when two actions
    /// can share a label.
    pub fn show_with_action(
        &mut self,
        message: impl Into<String>,
        action_label: impl Into<String>,
    ) {
        let label = action_label.into();
        self.enqueue_or_show(message.into(), Some(label.clone()), Some(label));
    }

    /// Shows snackbar with an action whose stable id differs from its label.
    pub fn show_with_action_id(
        &mut self,
        message: impl Into<String>,
        action_id: impl Into<String>,
        action_label: impl Into<String>,
    ) {
        self.enqueue_or_show(message.into(), Some(action_label.into()), Some(action_id.into()));
    }

    fn enqueue_or_show(
        &mut self,
        message: String,
        action_label: Option<String>,
        action_id: Option<String>,
    ) {
        if self.visible && self.queueing {
            if self.max_queued == 0 {
                // No room to wait: the message is dropped, and the drop is reported rather
                // than hidden, so a host can count what it lost.
                self.dismissed.emit(());
                return;
            }
            while self.queue.len() >= self.max_queued {
                self.queue.remove(0);
                self.dismissed.emit(());
            }
            self.queue.push((message, action_label, action_id));
            return;
        }
        self.message = message;
        self.action_label = action_label;
        self.action_id = action_id;
        self.visible = true;
        self.elapsed_ms = 0;
        self.reveal.set_open(true);
        self.base.request_redraw();
    }

    /// The maximum number of queued messages.
    pub fn max_queued(&self) -> usize {
        self.max_queued
    }

    /// Caps the queue; when full, the oldest waiting message is dropped and `dismissed` is
    /// emitted for it.
    pub fn set_max_queued(&mut self, max_queued: usize) {
        let max_queued = max_queued.min(64);
        if self.max_queued == max_queued {
            return;
        }
        self.max_queued = max_queued;
        while self.queue.len() > self.max_queued {
            self.queue.remove(0);
            self.dismissed.emit(());
        }
        self.base.request_redraw();
    }

    /// Where the bar sits in the control's rectangle.
    pub fn anchor(&self) -> SnackbarAnchor {
        self.anchor
    }

    /// Sets where the bar sits in the control's rectangle.
    pub fn set_anchor(&mut self, anchor: SnackbarAnchor) {
        if self.anchor == anchor {
            return;
        }
        self.anchor = anchor;
        self.base.request_redraw();
    }

    /// Sets how long the bar stays visible after it is shown.
    ///
    /// Zero (the default) keeps the previous behaviour: the bar waits for `dismiss` or an
    /// action. A non-zero duration makes it retire itself, which is what a status message
    /// wants — it is information, not a prompt, and should not require a click to clear.
    pub fn set_duration(&mut self, duration_ms: u32) {
        if self.duration_ms == duration_ms {
            return;
        }
        self.duration_ms = duration_ms;
        self.elapsed_ms = 0;
        self.base.request_redraw();
    }

    /// Returns the auto-dismiss duration in milliseconds, or zero when it never retires.
    pub fn duration(&self) -> u32 {
        self.duration_ms
    }

    /// Enables or disables queueing of messages shown while another is visible.
    pub fn set_queueing(&mut self, queueing: bool) {
        if self.queueing == queueing {
            return;
        }
        self.queueing = queueing;
        if !queueing {
            self.queue.clear();
        }
    }

    /// Returns whether messages queue behind the visible one.
    pub fn is_queueing(&self) -> bool {
        self.queueing
    }

    /// Returns the number of messages waiting.
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    /// Draws the progress rule as an indeterminate sweep.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        if self.indeterminate == indeterminate {
            return;
        }
        self.indeterminate = indeterminate;
        self.base.request_redraw();
    }

    /// Returns whether progress is indeterminate.
    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Dismisses snackbar.
    pub fn dismiss(&mut self) {
        if self.visible {
            self.visible = false;
            // Aimed at closed before the signal, so a listener that immediately pushes a new
            // message re-opens from wherever the exit had got to rather than from the bottom.
            self.reveal.set_open(false);
            self.dismissed.emit(());
            if let Some((message, action, action_id)) = self.queue.first().cloned() {
                self.queue.remove(0);
                self.message = message;
                self.action_label = action;
                self.action_id = action_id;
                self.visible = true;
                self.elapsed_ms = 0;
                self.reveal.set_open(true);
            }
            self.base.request_redraw();
        }
    }

    /// Sets optional progress indicator.
    pub fn set_progress(&mut self, progress: Option<f32>) {
        let progress = progress.map(|value| value.clamp(0.0, 1.0));
        if self.progress == progress {
            return;
        }
        self.progress = progress;
        self.base.request_redraw();
    }

    /// Returns action label.
    pub fn action_label(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    fn trigger_action(&mut self) -> bool {
        if !self.visible {
            return false;
        }
        let Some(id) = self.action_id.clone() else {
            return false;
        };
        self.action_triggered.emit(id);
        true
    }

    /// The bar's rectangle, placed by the anchor inside the control.
    fn bar_rect(&self) -> Rect {
        let rect = self.geometry();
        match self.anchor {
            SnackbarAnchor::Center => {
                ControlMetrics::full_width_band(rect, dimensions::SNACKBAR_HEIGHT)
            }
            SnackbarAnchor::Bottom => {
                ControlMetrics::bottom_band(rect, dimensions::SNACKBAR_HEIGHT)
            }
            SnackbarAnchor::Top => ControlMetrics::top_band(rect, dimensions::SNACKBAR_HEIGHT),
        }
    }

    fn action_rect(&self) -> Option<Rect> {
        self.action_label.as_ref()?;
        // Derived from the same bar the message is painted in, so the button sits on the right
        // end of the snackbar rather than at a fixed offset from the control's bottom edge —
        // which placed it 34 px below the bar in a 120 px cell.
        let bar = self.bar_rect();
        let height = SNACKBAR_ACTION_HEIGHT.min(bar.height);
        let width = SNACKBAR_ACTION_WIDTH.min(bar.width);
        Some(Rect::new(
            bar.x + bar.width as i32 - width as i32 - SNACKBAR_PADDING_H,
            bar.y + (bar.height.saturating_sub(height) / 2) as i32,
            width,
            height,
        ))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }
}

impl Widget for Snackbar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 48)
    }

    /// Advances the bar's enter/exit reveal; `true` while it still owes frames.
    ///
    /// Forwarded to the trait so the animation bus drives it: `tick_animations` sweeps every
    /// mounted control, so a bar that kept its tick inherent is one the host cannot advance.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut moving = self.reveal.tick(delta_ms);
        if self.visible {
            if self.duration_ms > 0 {
                self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
                if self.elapsed_ms >= self.duration_ms {
                    self.dismiss();
                    return true;
                }
            }
            if self.indeterminate && self.progress.is_some() {
                self.indeterminate_phase =
                    (self.indeterminate_phase + delta_ms as f32 / 1200.0) % 1.0;
                self.base.request_redraw();
                moving = true;
            }
        }
        moving
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
            || (self.visible && self.indeterminate && self.progress.is_some())
    }

    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "action_triggered" => {
                Some(EventSignalRef::mapped("action_triggered", &self.action_triggered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "dismissed" => Some(EventSignalRef::mapped("dismissed", &self.dismissed, |_| {
                CapabilityValue::Null
            })),
            _ => None,
        }
    }
}

/// `Snackbar`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_base.in.rs` dispatch. `visible` is deliberately *not* listed
/// here: the snackbar's own visibility flag hides it through `dismiss`, and the
/// shared four already publish a `visible` property, so a second name for the
/// same concept would be ambiguous. `action_label` is owned by `show_with_action`
/// and has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for Snackbar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.message().to_string())),
            "action_label" => match self.action_label() {
                Some(label) => Ok(CapabilityValue::String(label.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "queued_count" => Ok(CapabilityValue::UInt(self.queued_count() as u64)),
            "max_queued" => Ok(CapabilityValue::UInt(self.max_queued() as u64)),
            "anchor" => Ok(CapabilityValue::String(self.anchor().as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                // `show` clears the action label, so setting a plain message
                // matches the legacy "message" write, which showed without action.
                self.show(expect_string(value)?);
                Ok(())
            }
            "action_label" => Err(CapabilityAccessError::ReadOnlyProperty),
            "queued_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "max_queued" => match value {
                CapabilityValue::UInt(max) => {
                    self.set_max_queued(usize::try_from(max).unwrap_or(usize::MAX));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "anchor" => {
                let anchor = expect_string(value)?;
                let anchor =
                    SnackbarAnchor::from_name(&anchor).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_anchor(anchor);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "message",
            "action_label",
            "queued_count",
            "max_queued",
            "anchor",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `snackbar` publishes.
    ///
    /// `dismiss` maps onto the widget's real `dismiss`; it needs no payload.
    /// `show` and `show_with_action` both require the message (and the action
    /// label, and the slot behind it) the caller wants shown, so a bare
    /// invocation is reported as needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "dismiss" => {
                self.dismiss();
                Ok(())
            }
            "show" | "show_with_action" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Snackbar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(rect) = self.action_rect() {
                    if Self::point_in_rect(*pos, rect) {
                        let _ = self.trigger_action();
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 => {
                    let _ = self.trigger_action();
                }
                27 => self.dismiss(),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Snackbar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("snackbar");
        // `Snackbar` is built on `WidgetKind::StatusBar` and classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The panel's own fill is therefore a step toward
        // the foreground, so it reads as a surface of its own rather than as bare
        // window.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(248, 250, 253));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let background = resolved.blend(&text_color, 0.08);
        // The floating bar is the control's own emphasis surface: a first step away
        // from the panel, so it stays distinct in either appearance.
        //
        // # Why the theme's `inverse_surface` / `on_inverse_surface` pair is read here
        //
        // A snackbar deliberately **inverts** — it is the one surface in a window that is meant to
        // read as "above" the rest by being the opposite of it — and that inversion used to be two
        // inline blends of the panel's own colours (`background.blend(&text_color, 0.85)` for the
        // bar and the reverse for its text). The pair is what names that intent, and it is not
        // equivalent to the blend: a theme that wants its snackbar to *not* invert (a high-contrast
        // or print theme) can now say so, where the blend could only ever produce the inverted
        // pair. The blends survive as the fallback for a theme that predates the roles.
        let (inverse_surface, on_inverse_surface) = crate::style::theme_manager()
            .current_theme()
            .map(|active| (active.colors.inverse_surface, active.colors.on_inverse_surface))
            .unwrap_or_else(|| {
                let surface = background.blend(&text_color, 0.85);
                let on_surface = surface.blend(&background, 0.92);
                (surface, on_surface)
            });
        let bar_background = inverse_surface;
        let bar_border = bar_background.blend(&text_color, 0.18);
        // Text on the bar is the role's own paired ink, which is the reading the literal pair
        // encoded (dark bar, near-white text) without hardcoding either end.
        let bar_text = on_inverse_surface;
        // The action is a brand action, so it reads the theme's primary token rather
        // than a literal blue.
        let action_background = crate::style::resolved_theme_style_shared("button")
            .and_then(|button| button.background_color)
            .unwrap_or_else(|| background.blend(&text_color, 0.55));
        let action_border = action_background.blend(&text_color, 0.18);
        let action_text = action_background.blend(&background, 0.92);
        // Progress is a completion *state*, so it reads the theme's success token.
        let track = bar_background.blend(&text_color, 0.12);
        let progress_fill = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .map(|token| token.blend(&bar_background, 0.2))
            .unwrap_or_else(|| bar_background.blend(&background, 0.5));

        // ── The bar actually painted ──
        //
        // `rect` is the area the control was *given*; a snackbar is one floating bar of
        // [`dimensions::SNACKBAR_HEIGHT`], centred in that area. Filling the whole rectangle
        // made a 240x120 census cell a full-bleed panel with a 26 px bar pinned near its
        // bottom — a surface shaped like a snackbar rather than a snackbar — and the panel's
        // own fill is not part of this control's chrome at all: a snackbar sits *over* the
        // page, it does not paint the page. The band is the single derivation the message, the
        // action button and the progress rule are all placed from.
        let bar = self.bar_rect();
        // # Why the bar is painted while it is *either* shown or still leaving
        //
        // `visible` flips on the frame `dismiss` is called, but a `bool` cannot describe a bar
        // halfway out. The bar is therefore drawn whenever the reveal has anything left to show,
        // which is the shown state **and** the frames of an exit; testing the flag alone would cut
        // the exit off at its first frame — the hard cut this control was fixing.
        if !self.visible && self.reveal.is_closed() {
            return;
        }
        // A snackbar rises from the bottom edge of its own band, so the reveal is anchored at the
        // bottom: the bar's trailing edge stays put and its leading edge comes down to meet it.
        let revealed = self.reveal.revealed(
            bar,
            RevealDirection::Up,
            PopupReveal::travel_cap_for(crate::core::Size::new(bar.width, bar.height)),
        );
        let clipped = !self.reveal.is_closed();
        if clipped {
            context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
        }
        context.fill_rect(bar, bar_background);
        context.draw_rect(bar, bar_border);

        // Both labels are centred on their own band. The origins were fixed offsets
        // (`+ 16` and `+ 13`) written for one font size: a 14 px line in a 26 px bar spans
        // 16..30, four pixels past the bar's bottom edge, and the action label likewise.
        // The message and action labels take the **effective font** — the resolved theme/caller font
        // — so the theme body font and the user's text scale reach them (D09-STYLE-01).
        let message_font = effective_font(style);
        let message_h = context.measure_text("M", message_font).height;
        // The message owns the band up to the action button: it used to be drawn at its full
        // measured width, so a longer message ran under the action label and past the bar.
        let action_rect = self.action_rect();
        if !self.message.is_empty() {
            let message_x = bar.x + SNACKBAR_PADDING_H;
            let message_right = action_rect
                .map(|action| action.x - SNACKBAR_PADDING_H)
                .unwrap_or(bar.x + bar.width as i32 - SNACKBAR_PADDING_H);
            let message_width = (message_right - message_x).max(0) as u32;
            // Compensate the fitted draw's own inset, so the message starts at the padding and may
            // use the whole band instead of losing six pixels to the fit margin.
            let fit_inset = crate::render::TEXT_FIT_MARGIN as i32;
            context.draw_text_fitted_cow(
                Rect::new(
                    message_x - fit_inset,
                    bar.y + (bar.height as i32 - message_h as i32) / 2,
                    message_width + 2 * fit_inset as u32,
                    message_h.max(1),
                ),
                &self.message,
                message_font,
                bar_text,
                HorizontalAlignment::Left,
            );
        }

        if let Some(action_rect) = action_rect {
            context.fill_rect(action_rect, action_background);
            context.draw_rect(action_rect, action_border);
            if let Some(label) = &self.action_label {
                if !label.is_empty() {
                    context.draw_text_fitted_cow(
                        Rect::new(
                            action_rect.x + 4,
                            action_rect.y + (action_rect.height as i32 - message_h as i32) / 2,
                            action_rect.width.saturating_sub(8),
                            message_h,
                        ),
                        label,
                        message_font,
                        action_text,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }

        if let Some(progress) = self.progress {
            // The progress rule sits on the bar's own bottom edge, not on a literal 3 px above
            // a bar whose position moved with the canvas.
            let progress_bar = Rect::new(
                bar.x,
                bar.y + bar.height as i32 - SNACKBAR_PROGRESS_HEIGHT as i32,
                bar.width,
                SNACKBAR_PROGRESS_HEIGHT,
            );
            context.fill_rect(progress_bar, track);
            if self.indeterminate {
                // A moving window roughly a third of the track wide, wrapping at the end. The
                // phase is advanced by `tick`, so the indicator moves without any value to set.
                let window = (progress_bar.width as f32 * 0.35).max(8.0);
                let travel = progress_bar.width as f32 - window;
                let x = progress_bar.x as f32 + travel.max(0.0) * self.indeterminate_phase;
                context.fill_rect(
                    Rect::new(
                        x.round() as i32,
                        progress_bar.y,
                        window.round().max(1.0) as u32,
                        progress_bar.height,
                    ),
                    progress_fill,
                );
            } else {
                let fill = (progress_bar.width as f32 * progress).round() as u32;
                if fill > 0 {
                    context.fill_rect(
                        Rect::new(progress_bar.x, progress_bar.y, fill, progress_bar.height),
                        progress_fill,
                    );
                }
            }
        }
        // Popped on the same path it was pushed, so the reveal cannot leak a clip onto whatever
        // the host draws after this bar.
        if clipped {
            context.pop_clip();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// The queue is bounded; overflow drops the oldest waiting message and says so.
    #[test]
    fn the_queue_drops_the_oldest_message_when_full() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.set_queueing(true);
        bar.set_max_queued(2);
        let dismissed = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&dismissed);
        bar.dismissed.connect(move |_| *sink.lock().unwrap() += 1);

        bar.show("first");
        bar.show("second");
        bar.show("third");
        bar.show("fourth");
        assert_eq!(bar.queued_count(), 2, "the queue stays at its cap");
        assert_eq!(*dismissed.lock().unwrap(), 1, "each dropped message is reported");

        // The two most recent waiters survive, in order.
        bar.dismiss();
        assert_eq!(bar.message(), "third");
        bar.dismiss();
        assert_eq!(bar.message(), "fourth");
    }

    /// The action payload is the stable id, which may differ from the visible label.
    #[test]
    fn the_action_payload_is_the_id_not_the_label() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action_id("Sync failed", "retry_sync", "Retry");
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&seen);
        bar.action_triggered.connect(move |id| sink.lock().unwrap().push(id.to_string()));
        let rect = bar.action_rect().expect("an action button");
        bar.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 1));
        assert_eq!(*seen.lock().unwrap(), vec!["retry_sync".to_string()]);
    }

    /// The anchor actually moves the painted band.
    #[test]
    fn the_anchor_moves_the_bar() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 320, 120));
        bar.show("Moved");
        let centered = bar.bar_rect();
        bar.set_anchor(SnackbarAnchor::Top);
        assert_eq!(bar.bar_rect().y, 0);
        bar.set_anchor(SnackbarAnchor::Bottom);
        assert_eq!(bar.bar_rect().y + bar.bar_rect().height as i32, 120);
        assert_ne!(bar.bar_rect(), centered);
    }

    #[test]
    fn show_and_dismiss_change_visibility() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        assert!(!bar.is_visible());

        bar.show("Saved successfully");
        assert!(bar.is_visible());
        assert_eq!(bar.message(), "Saved successfully");

        bar.dismiss();
        assert!(!bar.is_visible());
    }

    #[test]
    fn enter_key_triggers_action_signal() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action("Retry deployment", "Retry");

        let actions = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = actions.clone();
        bar.action_triggered.connect(move |label| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(label.as_ref().clone());
            }
        });

        bar.handle_event(&Event::key_press(13, 0));

        let got = actions.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["Retry".to_string()]);
    }

    #[test]
    fn esc_key_dismisses() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action("Sync pending", "Open");
        assert!(bar.is_visible());

        bar.handle_event(&Event::key_press(27, 0));
        assert!(!bar.is_visible());
    }

    /// The bar is one floating row whatever height the control was given.
    ///
    /// The defect this pins: the control painted a full-canvas panel and then a 26 px bar
    /// pinned 34 px above its bottom, so a 240x120 census cell was a 120 px surface with the
    /// snackbar somewhere near its lower edge. A snackbar is its bar; it does not paint the
    /// page behind it.
    #[test]
    fn the_bar_keeps_its_own_height_in_any_rectangle() {
        use crate::widget::metrics::{dimensions, ControlMetrics};
        for height in [48u32, 60, 120, 300] {
            let mut bar = Snackbar::new(Rect::new(0, 0, 320, height));
            bar.show("Saved successfully");
            let svg = crate::widget::svg::render_to_svg(&mut bar);
            // The bar is the filled rectangle the control paints. It is at most
            // `SNACKBAR_HEIGHT` tall, centred in the control, so no emitted fill rectangle may
            // span the control's own full height (which is what a panel fill would do).
            let expected = ControlMetrics::full_width_band(
                Rect::new(0, 0, 320, height),
                dimensions::SNACKBAR_HEIGHT,
            );
            let fill = format!(
                "x=\"0\" y=\"{}\" width=\"320\" height=\"{}\"",
                expected.y, expected.height
            );
            assert!(
                svg.contains(&fill),
                "at control height {height} the bar must be {expected:?}, in:\n{svg}"
            );
        }
    }

    /// The action button sits on the bar, not below it.
    #[test]
    fn the_action_button_sits_on_the_bar() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 320, 120));
        bar.show_with_action("Retry deployment", "Retry");
        let action = bar.action_rect().expect("an action button");
        let bar_band = crate::widget::metrics::ControlMetrics::full_width_band(
            Rect::new(0, 0, 320, 120),
            crate::widget::metrics::dimensions::SNACKBAR_HEIGHT,
        );
        assert!(action.y >= bar_band.y, "the button starts on the bar");
        assert!(
            action.y + action.height as i32 <= bar_band.y + bar_band.height as i32,
            "the button ends on the bar"
        );
    }
}
