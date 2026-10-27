// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dialog widget — a secondary, usually-modal window that hosts its own children.
//!
//! This is the generic dialog control: a titled frame that carries a single
//! content widget, exposes modal intent (enforced through [`crate::widget::runtime`]'s
//! modal stack), and announces accept/reject through dedicated signals. It is the
//! standard desktop dialog — distinct from `MessageBox` (a fixed message + buttons)
//! and `PopupWindow` (a chrome-only popup), which `WidgetKind::Dialog` was previously
//! aliased to.

use crate::compat::RefCell;
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::composite::ButtonRole;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The height of the dialog's title bar, in pixels.
///
/// Also the amount the content area is inset by when a title is present, so the two
/// cannot drift apart. It is [`dimensions::DIALOG_TITLE_BAR_HEIGHT`] rather than a local
/// `24`: every dialog in this module draws the same strip, and a second value in the same
/// module is how the eight of them acquired four different heights (rule #101).
const DIALOG_TITLE_BAR_HEIGHT: u32 = dimensions::DIALOG_TITLE_BAR_HEIGHT;

/// How small a revealing dialog is at the instant it appears: 0.92 of its settled size.
///
/// The value lives in [`super::REVEAL_MIN_SCALE`] so `Dialog` and `MessageBox` settle identically —
/// they are the same class of object and a caller must not be able to tell which reveal it is
/// watching. Named locally only because the draw and its test both read it.
const DIALOG_REVEAL_MIN_SCALE: f32 = super::REVEAL_MIN_SCALE;

/// The side of the square close button drawn on a closable dialog's title bar.
const DIALOG_CLOSE_BUTTON: u32 = 16;

/// The side of the square resize grip painted in a resizable dialog's bottom-right corner.
const DIALOG_RESIZE_GRIP: u32 = 14;

/// The smallest size a grip resize may produce, in logical pixels.
const DIALOG_RESIZE_MIN: u32 = 64;

/// A button a [`Dialog`] can paint in its title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarButton {
    /// The close mark; activating it rejects the dialog.
    Close,
    /// The context-help mark; activating it emits `help_requested`.
    Help,
}

/// A caller-defined title-bar action: an icon and the id a handler receives (DLG-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleBarAction {
    /// The mark painted in the title bar.
    pub icon: IconName,
    /// The identifier emitted through `title_bar_action` when the mark is pressed.
    pub id: String,
}

impl TitleBarAction {
    /// Creates an action from its icon and id.
    pub fn new(icon: IconName, id: impl Into<String>) -> Self {
        Self { icon, id: id.into() }
    }
}

/// The drag in progress on the dialog's title bar.
#[derive(Debug, Clone, Copy)]
struct TitleDrag {
    /// The pointer position at the press, in the same space events carry.
    origin: Point,
    /// The dialog's rectangle at the press, so a move is an offset from a fixed anchor rather
    /// than an accumulation that drifts.
    origin_rect: Rect,
}

/// A generic dial‑log window: a titled frame that owns one content widget.
///
/// The dialog is the secondary window a caller fills with its own form. Modality is a
/// stored intent that [`crate::widget::runtime::enter_modal`] / [`crate::widget::runtime::exit_modal`]
/// make real — `open` and `close` drive both the visible state and the modal stack.
pub struct Dialog {
    base: BaseWidget,
    /// Title shown in the dialog's own chrome.
    title: String,
    /// The single content widget the dialog hosts, if any.
    content_widget: Option<ObjectId>,
    /// Whether the dialog blocks interaction with its owner while open.
    modal: bool,
    /// The result code the dialog finished with, if it has finished.
    ///
    /// `None` while the dialog is still open or was never accepted/rejected. This is the value
    /// [`Dialog::done`] writes and the one [`Dialog::exec`] reports.
    result: Option<i32>,
    /// Whether the last finish was an acceptance.
    accepted_state: bool,
    /// Whether the last finish was a rejection.
    rejected_state: bool,
    /// The title-bar buttons this dialog paints, in set order.
    ///
    /// Empty by default so a dialog that never asked for the affordance keeps the exact chrome
    /// it always had; [`Dialog::set_closable`] is the one-button spelling of the same set.
    title_bar_buttons: Vec<TitleBarButton>,
    /// The title-bar drag in progress, if any.
    drag: Option<TitleDrag>,
    /// How far the dialog has revealed, `0.0` hidden and `1.0` fully shown.
    ///
    /// # Why this is separate from the widget's own visible flag
    ///
    /// `show`/`hide` carry the *logical* state and the modal stack acts on it; they are answered the
    /// instant they change. This is what the draw path measures the scrim's strength and the frame's
    /// own placement with — so a dialog grows into place rather than appearing at its final size on
    /// one frame and vanishing on another. Same split, same reason, as `Switch`'s `checked`/`travel`;
    /// it starts at `0.0` because a freshly built dialog is hidden.
    reveal: PropertyDriver,
    /// Emitted by [`Dialog::accept`], when the dialog is accepted.
    pub accepted: GenericSignal,
    /// Emitted by [`Dialog::reject`], when the dialog is rejected.
    pub rejected: GenericSignal,
    /// Emitted by [`Dialog::open`].
    pub opened: GenericSignal,
    /// Emitted by [`Dialog::close`].
    pub closed: GenericSignal,
    /// Emitted with the result code when [`Dialog::done`] finishes the dialog.
    pub finished: Signal1<i32>,
    /// Emitted when the title bar's Help button is activated.
    pub help_requested: GenericSignal,
    /// Emitted with a [`TitleBarAction`]'s id when that mark is activated (DLG-02).
    pub title_bar_action: Signal1<String>,
    /// The caller-defined title-bar actions. Empty keeps the [`TitleBarButton`] set (DLG-02).
    title_bar_actions: Vec<TitleBarAction>,
    /// The action row: each button's label and role (DLG-01).
    buttons: Vec<(String, ButtonRole)>,
    /// The index of the button Enter activates (DLG-01).
    default_button: Option<usize>,
    /// The index of the button Escape activates (DLG-004); `None` keeps the reject fallback.
    cancel_button: Option<usize>,
    /// The action row's painted rectangles, cached for the press path (DLG-01).
    button_rects: RefCell<Vec<Rect>>,
    /// Whether the dialog paints a bottom-right resize grip and honours the gesture.
    ///
    /// Off by default: a dialog has no resize path to reuse unless the caller asks for one, so
    /// enabling this is the only write that changes the painted frame (DLG-003).
    resizable: bool,
    /// The pointer position at the start of a grip resize, with the geometry it started from.
    ///
    /// Kept as an anchor rather than accumulated, so moving the pointer back restores the
    /// original size exactly — the same rule the title-bar move drag uses.
    resize_origin: Option<Point>,
    resize_geometry: Option<Rect>,
    /// The content frame written on the last paint, so a still frame skips the write (G-P12).
    placed_content: Option<(Rect, ObjectId)>,
}

impl Dialog {
    /// Creates a titleless, non-modal dialog.
    pub fn new(geometry: Rect) -> Self {
        Self::with_title(String::new(), geometry)
    }

    /// Creates a dialog with a title.
    pub fn with_title(title: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dialog, geometry, "Dialog"),
            title: title.into(),
            content_widget: None,
            modal: true,
            result: None,
            accepted_state: false,
            rejected_state: false,
            title_bar_buttons: Vec::new(),
            drag: None,
            // At rest at the hidden end: a freshly built dialog is not shown, so it must not animate
            // itself away on its first frame.
            reveal: PropertyDriver::at(0.0, MotionSlot::Normal),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
            finished: Signal1::new(),
            help_requested: GenericSignal::new(),
            title_bar_action: Signal1::new(),
            title_bar_actions: Vec::new(),
            buttons: Vec::new(),
            default_button: None,
            cancel_button: None,
            button_rects: RefCell::new(Vec::new()),
            resizable: false,
            resize_origin: None,
            resize_geometry: None,
            placed_content: None,
        }
    }

    /// Returns the dialog's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the dialog's title and requests a redraw.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// Returns the content widget id, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget this dialog hosts.
    ///
    /// Replaces any previous content child and records the link in **both** directions, so the tree
    /// walk (and the modal subtree test) sees it. That pairing is BLUE22 §B.6 rule 5, and it is now
    /// expressed through the shared [`BaseWidget::add_child_linked`]/`remove_child_linked` pair
    /// rather than three inline writes: the inline form was correct here but was one of eleven
    /// hand-written copies of the same rule, of which eight were incomplete.
    ///
    /// A replacement also **unlinks the previous content**. `add_child` does not reject duplicates
    /// and nothing else removed the old id, so setting a second content widget left the first in
    /// the child list forever — a stale id a tree walk would still visit, and one that also kept
    /// claiming this dialog as its parent.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if self.content_widget == widget {
            return;
        }
        if let Some(old) = self.content_widget {
            if Some(old) != widget {
                self.base.remove_child_linked(old);
                // See `Frame::set_widget`: a replaced content child must not stay visible.
                crate::set_widget_visible(old, false);
            }
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child_linked(id);
            crate::set_widget_visible(id, true);
        }
        // The new child has never been placed: a control is constructed at a caller-chosen
        // rectangle (usually `0x0`) and the dialog is the only thing that knows the box it
        // should occupy. Without this write the content is a mounted child with a rectangle
        // of its own choosing, which is not the dialog's content box.
        self.place_content_in(self.frame_rect());
        self.base.request_redraw();
    }

    /// Returns whether the dialog is modal. Defaults to `true`.
    ///
    /// Records the intent; enforcement is the modal stack in [`crate::widget::runtime`]
    /// (`enter_modal` / `exit_modal`), which [`Dialog::open`] drives.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Sets the modality intent.
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        // `draw` paints the page-dimming layer from this, so a write that did not repaint left the
        // modal/non-modal distinction invisible until something else forced a frame.
        self.base.request_redraw();
    }

    /// Shows the dialog, emits `opened`, and — when modal — enters the modal stack.
    pub fn open(&mut self) {
        // DLG-002: a second open starts a second round. The outcome belongs to the round that
        // produced it, so leaving it in place let a subscriber read the previous answer before
        // this round had one — the dialog answering a question nobody had been asked yet.
        self.result = None;
        self.accepted_state = false;
        self.rejected_state = false;
        self.show();
        // Aim the reveal; the frames that follow grow and settle the frame. A dialog that appears at
        // full size on the frame it is shown and vanishes on the frame it is hidden is the one
        // gesture a dialog is *for* being skipped, and `show`/`hide` only carry the logical state.
        self.reveal.set_target(1.0);
        if self.modal {
            // DLG-001: the scrim stack is an open order, not a paint order, so a dialog opened
            // second owns the page's single dimming plane from this instant.
            super::push_modal_layer(self.id());
            let _ = crate::widget::runtime::enter_modal(self.id());
        }
        self.opened.emit();
    }

    /// Hides the dialog, pops it from the modal stack (if it was there), and emits
    /// `closed`.
    pub fn close(&mut self) {
        super::pop_modal_layer(self.id());
        let _ = crate::widget::runtime::exit_modal(self.id());
        self.hide();
        self.reveal.set_target(0.0);
        self.drag = None;
        self.resize_origin = None;
        self.resize_geometry = None;
        // The base lifecycle signal is what a handle-layer `on_close` / the JSON `closed` route
        // connects to; the widget's own signal is kept for its own listeners. Emitting both means
        // a handler fires from either path that closed the dialog.
        self.base.closed.emit();
        self.closed.emit();
    }

    /// How far the dialog has revealed, `0.0` fully hidden and `1.0` fully shown.
    ///
    /// The *drawn* fraction: the scrim's strength and the frame's own offset are functions of it, so
    /// a test can assert the dialog appeared rather than teleported by sampling it per frame.
    pub fn reveal_progress(&self) -> f32 {
        self.reveal.value()
    }

    /// Advances the reveal by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    /// Whether the dialog is between two reveal states -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.reveal.is_moving()
    }

    /// Accepts the dialog: records the accepted outcome, emits `accepted`, then closes it.
    ///
    /// The result code is `1`, the conventional accept value. Use [`Dialog::done`] when the
    /// caller needs a specific code.
    pub fn accept(&mut self) {
        self.done(1);
    }

    /// Rejects the dialog: records the rejected outcome, emits `rejected`, then closes it.
    ///
    /// The result code is `0`, the conventional reject value.
    pub fn reject(&mut self) {
        self.done(0);
    }

    /// Finishes the dialog with `result`.
    ///
    /// A non-zero code emits `accepted`; zero emits `rejected`. In both cases the outcome is
    /// recorded on [`Dialog::result`], the public `accepted`/`rejected` state properties, the
    /// `finished` signal, and then the dialog closes.
    pub fn done(&mut self, result: i32) {
        self.result = Some(result);
        self.accepted_state = result != 0;
        self.rejected_state = result == 0;
        if result != 0 {
            self.accepted.emit();
        } else {
            self.rejected.emit();
        }
        self.finished.emit(result);
        self.close();
    }

    /// The result code the dialog finished with, or `None` while it is still open.
    pub fn result(&self) -> Option<i32> {
        self.result
    }

    /// Whether the dialog finished by accepting.
    pub fn was_accepted(&self) -> bool {
        self.accepted_state
    }

    /// Whether the dialog finished by rejecting.
    pub fn was_rejected(&self) -> bool {
        self.rejected_state
    }

    /// Opens the dialog and reports the result it currently holds.
    ///
    /// # Why this does not block
    ///
    /// A widget has no event loop of its own: the host owns the loop, so a blocking `exec` would
    /// either re-enter it or deadlock. This entry point therefore performs the open half and
    /// answers [`Dialog::result`] — `None` until the host delivers the accept/reject that finishes
    /// the dialog. A host that needs a synchronous answer can connect `finished` and run its own
    /// loop until it fires.
    pub fn exec(&mut self) -> Option<i32> {
        self.open();
        self.result
    }

    /// Whether the title bar paints a close button.
    pub fn is_closable(&self) -> bool {
        self.title_bar_buttons.contains(&TitleBarButton::Close)
    }

    /// The title-bar buttons currently painted, in set order.
    pub fn title_bar_buttons(&self) -> &[TitleBarButton] {
        &self.title_bar_buttons
    }

    /// Replaces the title-bar button set.
    ///
    /// The set is de-duplicated while order is preserved, so a caller passing `[Help, Close]`
    /// gets Help left of Close rather than two close marks. This is the one write path for the
    /// chrome: [`Dialog::set_closable`] is a convenience for adding or removing `Close`, and it
    /// reads this same vector, so the two cannot disagree.
    pub fn set_title_bar_buttons(&mut self, buttons: &[TitleBarButton]) {
        let mut next: Vec<TitleBarButton> = Vec::with_capacity(buttons.len());
        for button in buttons {
            if !next.contains(button) {
                next.push(*button);
            }
        }
        if self.title_bar_buttons == next {
            return;
        }
        self.title_bar_buttons = next;
        self.base.request_redraw();
    }

    /// Appends an action button and returns its index (DLG-01).
    ///
    /// The role states what pressing it means: a [`ButtonRole::Destructive`] button rejects
    /// the dialog, every other role accepts it. A [`ButtonRole::Primary`] button also becomes
    /// the default (DLG-004), so the role's documented meaning and Enter agree; otherwise the
    /// first button is the default, which is the conventional safe answer.
    pub fn add_button(&mut self, text: impl Into<String>, role: ButtonRole) -> usize {
        self.buttons.push((text.into(), role));
        let index = self.buttons.len() - 1;
        match role {
            ButtonRole::Primary => self.default_button = Some(index),
            _ if self.default_button.is_none() => self.default_button = Some(index),
            _ => {}
        }
        self.base.request_redraw();
        index
    }

    /// Marks `index` as the button Enter activates (DLG-01).
    pub fn set_default_button(&mut self, index: usize) {
        if index >= self.buttons.len() {
            return;
        }
        self.default_button = Some(index);
        self.base.request_redraw();
    }

    /// The index Enter activates, or `None` when the row is empty (DLG-004).
    ///
    /// The explicit default marker wins; without one, the button whose role is
    /// [`ButtonRole::Primary`] is the caller's stated default action, and the first button is
    /// the last resort. That ordering is what makes the role a *marker* rather than a colour.
    pub fn enter_target(&self) -> Option<usize> {
        if let Some(index) = self.default_button.filter(|index| *index < self.buttons.len()) {
            return Some(index);
        }
        self.buttons
            .iter()
            .position(|(_, role)| *role == ButtonRole::Primary)
            .or(if self.buttons.is_empty() { None } else { Some(0) })
    }

    /// Marks `index` as the button Escape activates; `None` restores the reject fallback
    /// (DLG-004).
    ///
    /// An out-of-range index is ignored rather than recorded, so a stale marker cannot make
    /// Escape reach a button the row no longer contains.
    pub fn set_cancel_button(&mut self, index: Option<usize>) {
        let index = index.filter(|index| *index < self.buttons.len());
        if self.cancel_button == index {
            return;
        }
        self.cancel_button = index;
        self.base.request_redraw();
    }

    /// The index Escape activates, if the caller marked one (DLG-004).
    pub fn cancel_button(&self) -> Option<usize> {
        self.cancel_button
    }

    /// The index Escape activates, or `None` when Escape should reject outright (DLG-004).
    ///
    /// The explicit cancel marker wins. Without one, the **last** button is the fallback — the
    /// row's rightmost action is the one a keyboard dismissal conventionally belongs to — but
    /// only when it is a [`ButtonRole::Destructive`] action, because activating a plain button
    /// would *accept* the dialog and that is not what Escape means.
    fn escape_target(&self) -> Option<usize> {
        if let Some(index) = self.cancel_button.filter(|index| *index < self.buttons.len()) {
            return Some(index);
        }
        let last = self.buttons.len().checked_sub(1)?;
        self.buttons.get(last).filter(|(_, role)| *role == ButtonRole::Destructive).map(|_| last)
    }

    /// The action row's labels, in order (DLG-01).
    pub fn button_texts(&self) -> Vec<&str> {
        self.buttons.iter().map(|(text, _)| text.as_str()).collect()
    }

    /// The role of the button at `index` (DLG-01).
    pub fn button_role(&self, index: usize) -> Option<ButtonRole> {
        self.buttons.get(index).map(|(_, role)| *role)
    }

    /// Presses the button at `index`: a destructive action rejects, anything else accepts.
    pub fn activate_button(&mut self, index: usize) -> bool {
        let Some(role) = self.buttons.get(index).map(|(_, role)| *role) else {
            return false;
        };
        if role == ButtonRole::Destructive {
            self.reject();
        } else {
            self.accept();
        }
        true
    }

    /// Replaces the caller-defined title-bar actions (DLG-02).
    pub fn set_title_bar_actions(&mut self, actions: impl IntoIterator<Item = TitleBarAction>) {
        let next: Vec<TitleBarAction> = actions.into_iter().collect();
        if self.title_bar_actions == next {
            return;
        }
        self.title_bar_actions = next;
        self.base.request_redraw();
    }

    /// The caller-defined title-bar actions, in paint order.
    pub fn title_bar_actions(&self) -> &[TitleBarAction] {
        &self.title_bar_actions
    }

    /// The title-bar action rectangles, in the same right-to-left order the enum buttons use.
    fn custom_title_bar_rects(&self) -> Vec<(TitleBarAction, Rect)> {
        if self.title.is_empty() || self.title_bar_actions.is_empty() {
            return Vec::new();
        }
        let frame = self.frame_rect();
        let bar = ControlMetrics::top_band(frame, DIALOG_TITLE_BAR_HEIGHT);
        if bar.height == 0 || bar.width < DIALOG_CLOSE_BUTTON {
            return Vec::new();
        }
        let side = DIALOG_CLOSE_BUTTON.min(bar.height);
        let mut right = bar.x + bar.width as i32 - side as i32 - 4;
        let mut rects = Vec::with_capacity(self.title_bar_actions.len());
        for action in self.title_bar_actions.iter().rev() {
            rects.push((
                action.clone(),
                Rect::new(right, bar.y + ((bar.height - side) / 2) as i32, side, side),
            ));
            right -= side as i32 + 4;
        }
        rects
    }

    /// Shows or hides the title-bar close button, leaving any Help button in place.
    ///
    /// Off by default, so the default chrome is unchanged. When on, a press on the button
    /// rejects the dialog.
    pub fn set_closable(&mut self, closable: bool) {
        if self.is_closable() == closable {
            return;
        }
        let mut buttons: Vec<TitleBarButton> = self
            .title_bar_buttons
            .iter()
            .copied()
            .filter(|button| *button != TitleBarButton::Close)
            .collect();
        if closable {
            buttons.push(TitleBarButton::Close);
        }
        self.set_title_bar_buttons(&buttons);
    }

    /// Whether the dialog can be resized by its bottom-right grip. Off by default (DLG-003).
    pub fn is_resizable(&self) -> bool {
        self.resizable
    }

    /// Enables or disables the resize grip.
    ///
    /// Disabling also ends an in-flight resize, so a gesture cannot continue after the
    /// permission was withdrawn. Enabling changes the frame to fill the rectangle the dialog
    /// was given: a resizable window owns its own extent, while a fixed dialog is centred at
    /// its intrinsic size in that rectangle. Off is the historical behaviour, to the pixel.
    pub fn set_resizable(&mut self, resizable: bool) {
        if self.resizable == resizable {
            return;
        }
        self.resizable = resizable;
        if !resizable {
            self.resize_origin = None;
            self.resize_geometry = None;
        }
        self.place_content_in(self.frame_rect());
        self.base.request_redraw();
    }

    /// The bottom-right resize grip's box, or `None` when the dialog is not resizable.
    ///
    /// The grip is a real hit target, not just ink: a press inside it resizes the dialog
    /// instead of moving it or reaching the content.
    pub fn resize_grip_rect(&self) -> Option<Rect> {
        if !self.resizable {
            return None;
        }
        let frame = self.frame_rect();
        let side = DIALOG_RESIZE_GRIP.min(frame.width).min(frame.height);
        if side == 0 {
            return None;
        }
        Some(Rect::new(
            frame.x + frame.width as i32 - side as i32,
            frame.y + frame.height as i32 - side as i32,
            side,
            side,
        ))
    }

    /// Whether a grip resize is in flight (DLG-003).
    pub fn is_resizing(&self) -> bool {
        self.resize_origin.is_some()
    }

    /// Ends an in-flight resize without changing the size further.
    pub fn end_resize(&mut self) {
        if self.resize_origin.take().is_some() {
            self.resize_geometry = None;
            self.base.request_redraw();
        }
    }

    /// Advances an in-flight grip resize to `pos`.
    ///
    /// The size is recomputed from the gesture's origin rather than accumulated, so moving the
    /// pointer back restores the original size exactly. The top-left corner is fixed and both
    /// dimensions are clamped to [`DIALOG_RESIZE_MIN`].
    pub fn resize_to(&mut self, pos: Point) {
        let (Some(origin), Some(start)) = (self.resize_origin, self.resize_geometry) else {
            return;
        };
        let width = (start.width as i32 + (pos.x - origin.x)).max(DIALOG_RESIZE_MIN as i32) as u32;
        let height =
            (start.height as i32 + (pos.y - origin.y)).max(DIALOG_RESIZE_MIN as i32) as u32;
        self.apply_size(start, width, height);
    }

    /// Writes a new size anchored at `anchor`'s top-left corner.
    fn apply_size(&mut self, anchor: Rect, width: u32, height: u32) {
        let next = Rect::new(anchor.x, anchor.y, width, height);
        if self.base.geometry() == next {
            return;
        }
        // `set_geometry` also re-places the content child, so the two cannot disagree about
        // the box a resize hands over.
        self.set_geometry(next);
        self.base.request_redraw();
    }

    /// The title-bar buttons and their rectangles, in paint order (right to left).
    ///
    /// Drawing and hit-testing both read this, so a mark and its target cannot drift.
    fn title_bar_button_rects(&self) -> Vec<(TitleBarButton, Rect)> {
        if self.title.is_empty() {
            return Vec::new();
        }
        let frame = self.frame_rect();
        let bar = ControlMetrics::top_band(frame, DIALOG_TITLE_BAR_HEIGHT);
        if bar.height == 0 || bar.width < DIALOG_CLOSE_BUTTON {
            return Vec::new();
        }
        let side = DIALOG_CLOSE_BUTTON.min(bar.height);
        let mut right = bar.x + bar.width as i32 - side as i32 - 4;
        let mut rects = Vec::with_capacity(self.title_bar_buttons.len());
        // Reverse order so the first button in the set sits leftmost: `[Close, Help]` paints
        // Help at the right edge and Close to its left, matching the order the caller wrote.
        for button in self.title_bar_buttons.iter().rev() {
            rects.push((
                *button,
                Rect::new(right, bar.y + ((bar.height - side) / 2) as i32, side, side),
            ));
            right -= side as i32 + 4;
        }
        rects
    }

    /// The title-bar band that starts a move drag.
    fn drag_bar(&self) -> Option<Rect> {
        if self.title.is_empty() {
            return None;
        }
        let bar = ControlMetrics::top_band(self.frame_rect(), DIALOG_TITLE_BAR_HEIGHT);
        (bar.width > 0 && bar.height > 0).then_some(bar)
    }

    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered** — a census cell, a layout slot, a
    /// parent's whole client area. Painting it verbatim is what turned the 240x120 census
    /// cell into a 240x120 frame: a rectangle shaped like a dialog rather than a dialog.
    /// [`ControlMetrics::painted_box`] caps each axis at [`dimensions::DIALOG_MIN_WIDTH`] /
    /// [`dimensions::DIALOG_MIN_HEIGHT`] and centres what is left, and it clamps the result
    /// *up* to one pixel so a dialog squeezed to nothing is still visible. Every element
    /// below — the frame, the title bar and the title's line box — is derived from this one
    /// box, so the chrome and its label cannot be placed from different rectangles.
    fn frame_rect(&self) -> Rect {
        if self.resizable {
            // A resizable dialog owns its extent: the offered rectangle is its size, not a
            // maximum it is centred in. This is what gives the grip something to change and
            // is why the behaviour is opt-in — a fixed dialog keeps the exact frame it had.
            let rect = self.base.geometry();
            return Rect::new(rect.x, rect.y, rect.width.max(1), rect.height.max(1));
        }
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }

    /// The rectangle available to the content child.
    ///
    /// Insets the top by the title bar **only when a title is set**, so a titleless
    /// dialog gives its child the full rect.
    pub fn content_rect(&self) -> Rect {
        self.content_rect_of(self.frame_rect())
    }

    /// The content rectangle for a frame the dialog paints at `frame`.
    ///
    /// `frame` is the **painted** frame, which during the reveal is smaller than the resting
    /// one. Deriving the content box from the same rectangle the chrome is drawn from keeps the
    /// child inside the frame on every frame of the animation instead of snapping into place
    /// when the animation settles.
    fn content_rect_of(&self, frame: Rect) -> Rect {
        let top = if self.title.is_empty() {
            frame
        } else {
            let inset = DIALOG_TITLE_BAR_HEIGHT.min(frame.height);
            Rect::new(frame.x, frame.y + inset as i32, frame.width, frame.height - inset)
        };
        if self.buttons.is_empty() {
            return top;
        }
        // The action row owns the frame's bottom band, so the content stops above it.
        let bottom = dimensions::DIALOG_BUTTON_HEIGHT.min(top.height);
        Rect::new(top.x, top.y, top.width, top.height.saturating_sub(bottom))
    }

    /// Places the content child inside the frame the dialog paints at `frame`.
    ///
    /// # Why placement is a geometry write and not a layout
    ///
    /// A dialog's content model is **one child in one rectangle**, not a list of tracks. Giving
    /// it a `Layout` would let a caller describe a second, competing placement for the same
    /// child, which is the second-layout-path defect rule #101 names. The child keeps whatever
    /// layout it uses for *its* children; the dialog owns only the box it hands over.
    ///
    /// The write goes through the public geometry entry point rather than through a registry of
    /// draw closures: the content is an ordinary child in the widget tree, so the tree painter
    /// draws it after the dialog's chrome and the hit test reaches it without forwarding. An
    /// unmounted id is reported by that entry point as nothing to do, which is the honest answer
    /// for a document whose content was declared but never created.
    fn place_content_in(&self, frame: Rect) {
        let Some(content) = self.content_widget else {
            return;
        };
        let rect = self.content_rect_of(frame);
        crate::set_widget_geometry(content, rect.x, rect.y, rect.width, rect.height);
    }
}

impl Widget for Dialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(
            crate::widget::metrics::dimensions::DIALOG_WIDGET_DEFAULT_WIDTH,
            dimensions::DIALOG_MIN_HEIGHT,
        )
    }

    /// Moves the dialog and re-places its content child.
    ///
    /// A layout positions the dialog through the same geometry entry point, so this is where a
    /// window or panel resize reaches the content. The child is placed from the dialog's own
    /// painted frame, so the two cannot disagree about where the content begins.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_content_in(self.frame_rect());
    }

    // The reveal is the control's own animation; the trait spelling is what the frame bus reaches
    // through `&mut dyn Widget`, which is the only way the dialog actually appears.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Dialog::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        Dialog::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            "opened" => Some(EventSignalRef::unit("opened", &self.opened)),
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            "finished" => Some(EventSignalRef::mapped("finished", &self.finished, |v| {
                CapabilityValue::Int(i64::from(*v))
            })),
            "help_requested" => Some(EventSignalRef::unit("help_requested", &self.help_requested)),
            "title_bar_action" => {
                Some(EventSignalRef::mapped("title_bar_action", &self.title_bar_action, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `Dialog`'s property contract.
impl WidgetProperties for Dialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            "has_content" => Ok(CapabilityValue::Bool(self.content_widget().is_some())),
            "result" => Ok(match self.result {
                Some(code) => CapabilityValue::Int(i64::from(code)),
                None => CapabilityValue::Null,
            }),
            "accepted" => Ok(CapabilityValue::Bool(self.was_accepted())),
            "rejected" => Ok(CapabilityValue::Bool(self.was_rejected())),
            "closable" => Ok(CapabilityValue::Bool(self.is_closable())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "modal" => match value {
                CapabilityValue::Bool(v) => {
                    self.set_modal(v);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "has_content" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The outcome is written by finishing the dialog (`done`), not by assignment: a
            // writable `result` would record an answer nobody gave.
            "result" | "accepted" | "rejected" => Err(CapabilityAccessError::ReadOnlyProperty),
            "closable" => match value {
                CapabilityValue::Bool(v) => {
                    self.set_closable(v);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "modal",
            "has_content",
            "result",
            "accepted",
            "rejected",
            "closable",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "accept" => {
                self.accept();
                Ok(())
            }
            "reject" => {
                self.reject();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Dialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if !self.is_visible() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                for (action, rect) in self.custom_title_bar_rects() {
                    if rect.contains_point(*pos) {
                        self.title_bar_action.emit(action.id);
                        return;
                    }
                }
                for (kind, rect) in self.title_bar_button_rects() {
                    if rect.contains_point(*pos) {
                        match kind {
                            TitleBarButton::Close => self.reject(),
                            TitleBarButton::Help => self.help_requested.emit(),
                        }
                        return;
                    }
                }
                // The grip wins over the title bar and over the content: it is the one
                // affordance that says "this corner resizes me" (DLG-003).
                if self.resize_grip_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.resize_origin = Some(*pos);
                    self.resize_geometry = Some(self.frame_rect());
                    self.base.set_mouse_pressed(true);
                    return;
                }
                // The action row is painted at the frame's bottom band; a press there is a
                // button press, not a window move (DLG-01).
                let pressed_button = {
                    let rects = self.button_rects.borrow();
                    rects.iter().position(|rect| rect.contains_point(*pos))
                };
                if let Some(index) = pressed_button {
                    self.activate_button(index);
                    return;
                }
                if self.drag_bar().is_some_and(|bar| bar.contains_point(*pos)) {
                    // The *painted* frame moves with the pointer, not the offered rectangle: the
                    // frame is what the user grabbed.
                    self.drag = Some(TitleDrag { origin: *pos, origin_rect: self.frame_rect() });
                }
                self.base.set_mouse_pressed(true);
            }
            Event::MouseMove { pos } => {
                if self.is_resizing() {
                    self.resize_to(*pos);
                } else if let Some(drag) = self.drag {
                    let dx = pos.x - drag.origin.x;
                    let dy = pos.y - drag.origin.y;
                    if dx != 0 || dy != 0 {
                        let moved = Rect::new(
                            drag.origin_rect.x + dx,
                            drag.origin_rect.y + dy,
                            drag.origin_rect.width,
                            drag.origin_rect.height,
                        );
                        self.set_geometry(moved);
                    }
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.drag = None;
                self.end_resize();
                self.base.set_mouse_pressed(false);
            }
            // A release outside the dialog is never delivered, so a resize is ended when the
            // pointer leaves the control rather than left half-finished.
            Event::MouseLeave { .. } if self.is_resizing() => self.end_resize(),
            Event::KeyPress { key, .. } if *key == crate::event::key_codes::ESCAPE => {
                // DLG-004: a marked cancel action is what Escape means; without one the
                // dialog rejects, which is the behaviour every caller already had.
                match self.escape_target() {
                    Some(index) => {
                        self.activate_button(index);
                    }
                    None => self.reject(),
                }
            }
            Event::KeyPress { key, .. }
                if *key == crate::event::key_codes::ENTER
                    || *key == crate::event::key_codes::LINE_FEED =>
            {
                // DLG-004: the default marker, then the Primary role, then the first button.
                if let Some(index) = self.enter_target() {
                    self.activate_button(index);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for Dialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // A fully hidden dialog paints nothing at all: "reveal is zero" means the dialog is not on
        // screen, not that it is a very small dialog. Read once and used for everything below — the
        // scrim, the frame's scale — so no two of them can disagree about how far along it is.
        let reveal = self.reveal.value();
        if reveal <= 0.0 {
            return;
        }

        // ── The modal scrim ──
        //
        // A **modal** dialog dims what is behind it: that dimming is the entire visual difference
        // between "a dialog is up and the page is inert" and "a panel happened to be drawn on the
        // page". It was absent here while `bottom_sheet`/`modal_bottom_sheet` both had it, so the
        // one control whose whole purpose is modality was the one that did not show it. The
        // backdrop fades in with the reveal, so it is never fully dimmed while the frame is still
        // growing; one value drives both, so they cannot get out of step.
        if self.modal {
            super::draw_modal_scrim_scaled(
                context,
                self.id(),
                self.geometry(),
                self.base.parent(),
                reveal,
            );
        }

        // The min-scale floor below applies to a reveal that is *in progress*, so the first visible
        // frame already shows the dialog's shape rather than a speck.

        // The **frame**, not the control's rectangle: see `frame_rect`. A dialog offered a
        // 240x120 census cell paints its own centred box, and every band below — the title
        // strip, its separator and the title's line box — is taken from this one rect.
        let resting = self.frame_rect();
        if resting.width == 0 || resting.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen, because the frame, the title bar
        // and the title text were all hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("dialog");
        // The dialog is a `Surface`-role control, and `Surface` resolves to
        // `theme.colors.background` — the colour the window behind it is already filled
        // with. Painting the frame in it would make the dialog indistinguishable from the
        // window, so the surface is tinted one step toward the foreground: the same
        // distinction `Colors::input_background` makes for a field, applied to a panel.
        // The window fill is read as its own lock acquisition and copied out as a value, so
        // the guard is dropped before anything else touches the theme — the global
        // manager's mutex is not re-entrant.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let themed_surface = theme.as_ref().and_then(|t| t.background_color);
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::rgb(40, 40, 40));
        // The filter is on the **resolved** value, not only on the theme's: the active
        // theme is applied to every control before it is drawn, so `style.background_color`
        // already holds `Surface`'s window-fill colour and letting it through unfiltered
        // is exactly the invisible-panel defect this guards against.
        let surface = match style.background_color.or(themed_surface) {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border =
            crate::widget::draw::border_ink(style.border_color, themed_border, surface, ink);
        // The title bar is a distinct band on the frame, derived from it so the two stay
        // one visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.08);

        // The frame **scales up** from its own centre as the reveal progresses, so the appear reads
        // as a growth rather than a fade-in of a static box. `MIN_SCALE` is the floor for a reveal
        // that is in progress: at reveal 1.0 the scale is exactly 1.0, so a settled dialog is
        // pixel-identical to the pre-animation one. This is the whole reason the resting snapshot
        // is unchanged by this feature.
        let scale = DIALOG_REVEAL_MIN_SCALE + (1.0 - DIALOG_REVEAL_MIN_SCALE) * reveal;
        let rect = super::scale_about_centre(resting, scale);
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // The content follows the frame as it grows, so a half-revealed dialog holds its content
        // inside the chrome rather than showing a full-size child around a small frame. The
        // tree painter draws the content after this function returns, so the placement made
        // here is the rectangle that very frame is drawn into. A still frame writes the same
        // rectangle it wrote last frame, so the write is skipped (G-P12).
        if let Some(content) = self.content_widget {
            let changed = self.placed_content != Some((rect, content));
            if changed {
                self.place_content_in(rect);
                self.placed_content = Some((rect, content));
            }
        }

        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.face_with_gradient_corners(
                rect,
                surface,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().resolved_corner_radii().to_array(),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }

        // ── The action row ──
        //
        // The buttons live in the frame's bottom band, painted before the title-bar early
        // return so a titleless dialog keeps them. The row's own geometry is the shared
        // `action_row_geometry` derivation, so a Dialog button is the same object as one in
        // every other dialog in this module (DLG-01).
        if !self.buttons.is_empty() {
            let labels: Vec<String> = self.buttons.iter().map(|(label, _)| label.clone()).collect();
            let action_font = effective_font(style);
            let band = super::message_box::inset_action_band(
                ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
                rect.y,
            );
            let row =
                super::message_box::action_row_geometry(context, action_font, &labels, band, true);
            for (index, button) in row.buttons.iter().enumerate() {
                let is_default = self.default_button == Some(index);
                let fill = if is_default { ink } else { surface.blend(&ink, 0.12) };
                let text_ink = if is_default { surface } else { ink };
                context.fill_rect(*button, fill);
                context.draw_rect(*button, border);
                context.draw_text_line_cow(
                    *button,
                    &labels[index],
                    action_font,
                    text_ink,
                    HorizontalAlignment::Center,
                );
                if is_default {
                    context.draw_rounded_rect_stroke(*button, 2, border, 2);
                }
            }
            *self.button_rects.borrow_mut() = row.buttons;
        } else {
            self.button_rects.borrow_mut().clear();
        }

        // The resize grip is painted before the title-bar early return so a titleless dialog
        // keeps it. Three short diagonals are the conventional "drag this corner" mark and
        // need no font glyph (DLG-003).
        if let Some(grip) = self.resize_grip_rect() {
            let grip_ink = border.with_alpha(170);
            for step in 1..=3 {
                let offset = step * 4;
                context.draw_line(
                    Point::new(
                        grip.x + grip.width as i32 - offset,
                        grip.y + grip.height as i32 - 2,
                    ),
                    Point::new(
                        grip.x + grip.width as i32 - 2,
                        grip.y + grip.height as i32 - offset,
                    ),
                    grip_ink,
                );
            }
        }

        if self.title.is_empty() {
            return;
        }
        // The title strip is a **band** of the frame, not a sub-rectangle reconstructed
        // from the frame's top edge: `top_band` keeps the strip's thickness and clamps it
        // to the frame, so it cannot start above the frame or grow past it.
        let bar = ControlMetrics::top_band(rect, DIALOG_TITLE_BAR_HEIGHT);
        if bar.height == 0 {
            return;
        }
        context.fill_rect(bar, title_bar);
        context.draw_line(
            Point::new(bar.x, bar.y + bar.height as i32),
            Point::new(bar.x + bar.width as i32, bar.y + bar.height as i32),
            border,
        );
        // The title is centred **on the bar's own band** through the shared primitive: the
        // origin is the glyph's top edge, so `text_line` is what places it on the strip's
        // middle line. The old `rect.y + (bar - title_h) / 2` measured against a height the
        // strip did not have, and a bare literal `y` would reintroduce exactly that.
        // The **effective font** — the resolved theme/caller font — so the title honours the theme
        // body font and the user's text scale (D09-STYLE-01).
        let title_font = effective_font(style);
        let band = ControlMetrics::band_inset(bar, 0);
        let title_line = context.text_line(band, title_font);
        // Title-bar buttons occupy the right end of the strip, so a long title elides before
        // them instead of running underneath.
        let reserved = if self.title_bar_actions.is_empty() {
            self.title_bar_buttons.len() as u32 * (DIALOG_CLOSE_BUTTON + 4)
        } else {
            self.title_bar_actions.len() as u32 * (DIALOG_CLOSE_BUTTON + 4)
        };
        let title_width = rect.width.saturating_sub(16).saturating_sub(reserved);
        context.draw_text_fitted_cow(
            Rect::new(rect.x + 8, title_line.y, title_width, title_line.height.max(1)),
            &self.title,
            title_font,
            ink,
            HorizontalAlignment::Left,
        );
        for (action, button) in self.custom_title_bar_rects() {
            crate::widget::draw_icon_centered(context, button, button.height, ink, action.icon);
        }
        for (kind, button) in self.title_bar_button_rects() {
            crate::widget::draw_icon_centered(
                context,
                button,
                button.height,
                ink,
                match kind {
                    TitleBarButton::Close => crate::widget::IconName::Close,
                    TitleBarButton::Help => crate::widget::IconName::Help,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn dialog() -> Dialog {
        Dialog::with_title("Settings", Rect::new(0, 0, 400, 300))
    }

    #[test]
    fn dialog_creation_defaults() {
        let d = Dialog::new(Rect::new(0, 0, 400, 300));
        assert_eq!(d.kind(), WidgetKind::Dialog);
        assert!(d.title().is_empty());
        assert!(d.is_modal());
        assert!(d.content_widget().is_none());
    }

    #[test]
    fn dialog_title_and_modal_roundtrip() {
        let mut d = dialog();
        assert_eq!(d.title(), "Settings");
        d.set_title("Preferences");
        assert_eq!(d.title(), "Preferences");
        d.set_modal(false);
        assert!(!d.is_modal());
    }

    #[test]
    fn dialog_accept_and_reject_emit_their_signals() {
        let mut d = dialog();
        let accepted = Arc::new(AtomicUsize::new(0));
        let rejected = Arc::new(AtomicUsize::new(0));
        let a = accepted.clone();
        let r = rejected.clone();
        d.accepted.connect(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });
        d.rejected.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });

        d.accept();
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        assert_eq!(rejected.load(Ordering::SeqCst), 0);

        d.reject();
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dialog_open_close_emit_lifecycle_signals() {
        let mut d = dialog();
        let opened = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let o = opened.clone();
        let c = closed.clone();
        d.opened.connect(move || {
            o.fetch_add(1, Ordering::SeqCst);
        });
        d.closed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        d.open();
        assert!(d.is_visible());
        assert_eq!(opened.load(Ordering::SeqCst), 1);

        d.close();
        assert!(!d.is_visible());
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn done_records_the_result_and_finishes_the_dialog() {
        let mut d = dialog();
        d.open();
        let finished = Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));
        let sink = finished.clone();
        d.finished.connect(move |code: std::sync::Arc<i32>| {
            sink.lock().expect("lock").push(*code);
        });
        assert_eq!(d.result(), None);
        assert!(!d.was_accepted() && !d.was_rejected());

        d.done(42);
        assert_eq!(d.result(), Some(42));
        assert!(d.was_accepted());
        assert!(!d.was_rejected());
        assert!(!d.is_visible(), "finishing closes the dialog");
        assert_eq!(*finished.lock().expect("lock"), vec![42]);

        d.open();
        d.done(0);
        assert_eq!(d.result(), Some(0));
        assert!(!d.was_accepted());
        assert!(d.was_rejected());
        assert_eq!(*finished.lock().expect("lock"), vec![42, 0]);
    }

    #[test]
    fn reopening_a_dialog_clears_the_previous_result() {
        let mut d = dialog();
        d.open();
        d.done(42);
        assert_eq!(d.result(), Some(42));
        assert!(d.was_accepted());

        // DLG-002: a second round starts with no answer, so a subscriber cannot read the
        // previous round's outcome before this one has one.
        d.open();
        assert_eq!(d.result(), None);
        assert!(!d.was_accepted());
        assert!(!d.was_rejected());
        d.close();
    }

    #[test]
    fn enter_prefers_the_primary_role_and_escape_the_cancel_marker() {
        let mut d = dialog();
        d.add_button("Back", ButtonRole::Normal);
        d.add_button("Apply", ButtonRole::Primary);
        d.add_button("Cancel", ButtonRole::Destructive);
        d.set_cancel_button(Some(2));
        d.open();

        // Enter follows the Primary marker rather than the first button.
        assert_eq!(d.enter_target(), Some(1));
        d.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));
        assert_eq!(d.result(), Some(1), "Enter must activate the Primary button");

        d.open();
        // Escape follows the cancel marker, which is the destructive action here.
        assert_eq!(d.escape_target(), Some(2));
        d.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(d.result(), Some(0), "Escape must activate the marked cancel button");
    }

    #[test]
    fn escape_falls_back_to_reject_and_enter_to_the_first_button() {
        let mut d = dialog();
        d.add_button("Only", ButtonRole::Normal);
        d.open();
        assert_eq!(d.cancel_button(), None);
        // The last button is not a cancellation marker, so Escape keeps its reject meaning
        // rather than accepting the dialog through a plain action.
        assert_eq!(d.escape_target(), None);
        d.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(d.result(), Some(0));

        let mut empty = dialog();
        empty.open();
        assert_eq!(empty.enter_target(), None, "an empty row has nothing to activate");
        empty.close();
    }

    #[test]
    fn a_resizable_dialog_paints_and_drags_a_size_grip() {
        let mut d = Dialog::with_title("Resize", Rect::new(0, 0, 400, 320));
        d.set_resizable(true);
        let grip = d.resize_grip_rect().expect("a resizable dialog paints a grip");
        assert_eq!(grip.x + grip.width as i32, 400);
        assert_eq!(grip.y + grip.height as i32, 320);

        d.handle_event(&Event::MousePress {
            pos: Point::new(grip.x + 1, grip.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert!(d.is_resizing(), "a press on the grip starts a resize, not a move");
        d.handle_event(&Event::MouseMove { pos: Point::new(grip.x + 41, grip.y + 21) });
        assert_eq!(d.geometry(), Rect::new(0, 0, 440, 340));
        d.handle_event(&Event::MouseRelease { pos: Point::new(440, 340), button: 1 });
        assert!(!d.is_resizing());

        // A shrink clamps to the minimum rather than to zero.
        let grip = d.resize_grip_rect().expect("still resizable");
        d.handle_event(&Event::MousePress {
            pos: Point::new(grip.x + 1, grip.y + 1),
            button: 1,
            modifiers: 0,
        });
        d.handle_event(&Event::MouseMove { pos: Point::new(-1000, -1000) });
        assert_eq!(d.geometry().width, DIALOG_RESIZE_MIN);
        assert_eq!(d.geometry().height, DIALOG_RESIZE_MIN);

        d.set_resizable(false);
        assert!(d.resize_grip_rect().is_none(), "a fixed dialog advertises no grip");
    }

    #[test]
    fn escape_rejects_an_open_dialog() {
        let mut d = dialog();
        d.open();
        let rejected = Arc::new(AtomicUsize::new(0));
        let r = rejected.clone();
        d.rejected.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });
        d.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
        assert_eq!(d.result(), Some(0));
        assert!(!d.is_visible());
    }

    #[test]
    fn dragging_the_title_bar_moves_the_dialog() {
        let mut d = Dialog::with_title("Move", Rect::new(10, 20, 200, 160));
        let before = d.geometry();
        let bar = d.drag_bar().expect("a titled dialog has a drag bar");
        let grab = Point::new(bar.x + 4, bar.y + 4);
        d.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });
        d.handle_event(&Event::MouseMove { pos: Point::new(grab.x + 15, grab.y + 25) });
        assert_eq!(
            d.geometry(),
            Rect::new(before.x + 15, before.y + 25, before.width, before.height)
        );
        d.handle_event(&Event::MouseRelease {
            pos: Point::new(grab.x + 15, grab.y + 25),
            button: 1,
        });
        // A move after the release no longer drags.
        let settled = d.geometry();
        d.handle_event(&Event::MouseMove { pos: Point::new(grab.x + 90, grab.y + 90) });
        assert_eq!(d.geometry(), settled);
    }

    #[test]
    fn a_closable_dialog_owns_a_close_hit_area() {
        let mut d = Dialog::with_title("Close", Rect::new(0, 0, 300, 200));
        assert!(!d.is_closable(), "the button is opt-in, so the default chrome is unchanged");
        d.set_closable(true);
        let close = d
            .title_bar_button_rects()
            .into_iter()
            .find(|(button, _)| *button == TitleBarButton::Close)
            .map(|(_, rect)| rect)
            .expect("a closable titled dialog paints a close button");
        let rejected = Arc::new(AtomicUsize::new(0));
        let r = rejected.clone();
        d.rejected.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });
        d.handle_event(&Event::MousePress {
            pos: Point::new(close.x + 1, close.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
        assert_eq!(d.result(), Some(0));
    }

    #[test]
    fn dialog_content_rect_insets_only_when_titled() {
        let titleless = Dialog::new(Rect::new(0, 0, 160, 100));
        assert_eq!(titleless.content_rect(), Rect::new(0, 0, 160, 100));

        let titled = Dialog::with_title("T", Rect::new(0, 0, 160, 100));
        assert_eq!(
            titled.content_rect(),
            Rect::new(0, DIALOG_TITLE_BAR_HEIGHT as i32, 160, 100 - DIALOG_TITLE_BAR_HEIGHT)
        );
    }

    #[test]
    fn dialog_with_a_title_paints_chrome() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        // Both are opened and settled first: a dialog that has not been shown has not begun to
        // reveal, and paints nothing at all -- so an unopened pair would compare two empty
        // documents and the title difference would be invisible for the wrong reason.
        let mut titleless = Dialog::new(Rect::new(0, 0, 160, 100));
        let mut titled = Dialog::with_title("Details".to_string(), Rect::new(0, 0, 160, 100));
        for dialog in [&mut titleless, &mut titled] {
            dialog.open();
            while dialog.tick(1000) {}
        }
        let plain = crate::widget::svg::render_to_svg(&mut titleless);
        let decorated = crate::widget::svg::render_to_svg(&mut titled);
        assert_ne!(plain, decorated);
        // The title is `font8x8` glyph geometry rather than a `<text>` element, so the string is
        // not in the document and the check is on **where the ink landed**: a titled dialog gains
        // a run on the title strip, and a titleless one paints no text at all.
        let (left, top, right, bottom) = crate::widget::svg::text_ink_box(&decorated)
            .unwrap_or_else(|| panic!("a titled dialog must paint its title: {decorated}"));
        assert!(right > left, "the title laid down ink: {left}..{right}");
        assert!(
            (0..DIALOG_TITLE_BAR_HEIGHT as i32).contains(&top)
                && bottom <= DIALOG_TITLE_BAR_HEIGHT as i32,
            "the title must sit on the title strip, got {top}..{bottom}"
        );
        assert!(
            crate::widget::svg::text_ink_box(&plain).is_none(),
            "a titleless dialog paints no title ink: {plain}"
        );
    }

    /// A modal dialog dims the page behind it, and a non-modal one does not.
    ///
    /// # The defect this pins
    ///
    /// The dialog is the one control whose whole purpose is modality, and it was the only one of
    /// the modal family with **no scrim**: `bottom_sheet` and `modal_bottom_sheet` both dim the
    /// page, so a dialog and a sheet looked like different degrees of the same thing when they
    /// should have looked like the same thing. The assertion is the composited page: the scrim laid
    /// over the window fill must be **darker** than the window fill itself, which is what "the page
    /// is inert" looks like and what a scrim blended toward the ink would get backwards.
    #[test]
    fn a_modal_dialog_dims_the_page_behind_it() {
        use crate::style::LayerColor;

        let _theme_guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [crate::theme::AppearanceMode::Light, crate::theme::AppearanceMode::Dark]
        {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let page = crate::style::theme_manager()
                .current_theme()
                .expect("a preset is active")
                .colors
                .background;
            let scrim = crate::style::layer_color(LayerColor::Scrim)
                .expect("the preset defines a scrim role");
            let composited = page.blend(&scrim, scrim.a as f32 / 255.0);
            assert!(
                composited.luminance() < page.luminance(),
                "the {appearance:?} scrim must dim the page: {page:?} -> {composited:?}"
            );
        }

        // And it is the *modal* intent that paints it, not merely being a dialog: a non-modal one
        // leaves the page it sits beside untouched.
        //
        // Both are *opened* first, because a dialog that has not been shown has not begun to reveal
        // and paints nothing -- so comparing two hidden dialogs would compare two empty documents and
        // the modality difference would be invisible for the wrong reason.
        let mut modal = Dialog::with_title("Hi", Rect::new(0, 0, 200, 120));
        modal.set_modal(true);
        modal.open();
        while modal.tick(1000) {}
        let with_scrim = crate::widget::svg::render_to_svg(&mut modal);
        let mut modeless = Dialog::with_title("Hi", Rect::new(0, 0, 200, 120));
        modeless.set_modal(false);
        modeless.open();
        while modeless.tick(1000) {}
        let without_scrim = crate::widget::svg::render_to_svg(&mut modeless);
        assert_ne!(
            with_scrim, without_scrim,
            "modality must be visible, or the dialog lies about whether the page is inert"
        );
    }

    /// Opening a dialog reveals it rather than placing it there at full size.
    ///
    /// # The defect this pins
    ///
    /// `open`/`close` carried only the logical state, and the draw read it directly, so the frame
    /// appeared complete on the frame it was shown and vanished on the frame it was hidden. The
    /// assertions are in two halves, because a progress nothing reads is not a reveal: the model must
    /// take an interior value, and the *painted* frame must be smaller at that moment.
    #[test]
    fn opening_a_dialog_reveals_it() {
        use crate::widget::svg::render_to_svg;

        let mut dialog = Dialog::with_title("Hi", Rect::new(0, 0, 200, 120));
        assert_eq!(dialog.reveal_progress(), 0.0, "a fresh dialog is hidden");
        assert!(!dialog.is_animating(), "and owes no frames");

        dialog.open();
        assert!(dialog.is_visible(), "the logical state answers at once");
        assert!(dialog.is_animating(), "while the drawn frame owes frames");

        // The painted frame's width, read from the document: the frame is the widest rounded
        // rectangle the dialog paints.
        fn frame_width(svg: &str) -> u32 {
            svg.split("<rect ")
                .filter(|chunk| chunk.contains("rx=\""))
                .filter_map(|chunk| {
                    let w = chunk.split("width=\"").nth(1)?;
                    w.split('"').next()?.parse::<u32>().ok()
                })
                .max()
                .unwrap_or(0)
        }

        assert!(dialog.tick(60), "still growing after one step");
        let mid = dialog.reveal_progress();
        assert!(
            mid > 0.0 && mid < 1.0,
            "the dialog must pass through an interior reveal (got {mid})"
        );
        let mid_width = frame_width(&render_to_svg(&mut dialog));

        while dialog.tick(60) {}
        assert_eq!(dialog.reveal_progress(), 1.0, "and settle fully shown");
        let settled_width = frame_width(&render_to_svg(&mut dialog));

        assert!(
            mid_width > 0 && mid_width < settled_width,
            "a revealing frame must be smaller than a settled one: mid={mid_width} settled={settled_width}"
        );
    }

    // ── D09-STYLE-01: the dialog title consumes the resolved theme/caller font ──

    /// A caller-authored `style.font` must change the title the dialog paints.
    ///
    /// # The defect this pins
    ///
    /// The title bar built a `Font::default()` by hand, so the theme body font and the user's text
    /// scale never reached a dialog title. It now reads `metrics::effective_font`.
    #[test]
    fn a_custom_font_changes_the_dialog_title_ink() {
        let render = |font: Option<crate::core::Font>| {
            let mut d = Dialog::with_title("Settings", Rect::new(0, 0, 400, 300));
            if let Some(font) = font {
                d.set_style(crate::style::WidgetStyle::default().with_font(font));
            }
            d.open();
            while d.tick(1000) {}
            let svg = crate::widget::svg::render_to_svg(&mut d);
            crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
        };

        let base_ink = render(None);
        let big_ink = render(Some(crate::core::Font::simple("Test", 30.0)));
        assert!(base_ink > 0, "the default title must paint ink for this test to mean anything");
        assert!(
            big_ink > base_ink,
            "a larger font must widen the painted title ink: {big_ink} vs {base_ink}"
        );
    }

    // ── the dialog places the content child it hosts ──

    /// A dialog hands its content child the box the child is supposed to fill.
    ///
    /// # The defect this pins
    ///
    /// The content id was linked in both directions, so a tree walk found it and hit-tested
    /// it — but nothing ever wrote its geometry. The child therefore kept whatever rectangle it
    /// was constructed with (normally `0x0`), which meant a dialog with a form in it painted
    /// **chrome over nothing**: the model said `has_content = true`, the tree contained the
    /// child, and the pixels contained no content. Both ends are mounted here because the
    /// placement is a geometry write addressed to the runtime registry; an unregistered id
    /// would make the test pass without proving the write lands.
    #[test]
    fn the_dialog_places_its_content_inside_its_frame() {
        use crate::widget::runtime;

        let dialog_id = runtime::register(Box::new(dialog())).expect("mount dialog");
        let content_id = runtime::register(Box::new(crate::widget::Label::new(
            "body".to_string(),
            Rect::new(0, 0, 10, 10),
        )))
        .expect("mount content");

        runtime::with_widget_mut(dialog_id, |widget| {
            crate::widget::capability::coercion::widget_as_mut::<Dialog>(widget)
                .expect("the mounted widget is a dialog")
                .set_content_widget(Some(content_id));
        })
        .expect("the dialog is mounted");

        // `painted_box` centres the intrinsic 280x240 frame in the 400x300 offered, and the
        // content box is that frame below its 28px title strip.
        assert!(runtime::set_geometry(dialog_id, Rect::new(20, 30, 400, 300)));
        assert_eq!(
            runtime::geometry_of(content_id),
            Some(Rect::new(80, 88, 280, 212)),
            "the dialog's own frame decides where its content begins"
        );

        runtime::unregister(content_id);
        runtime::unregister(dialog_id);
    }

    /// The content follows the frame while the dialog reveals, instead of appearing at its
    /// settled size around a smaller frame.
    #[test]
    fn a_revealing_dialog_keeps_its_content_inside_the_growing_frame() {
        use crate::widget::runtime;

        let content_id = runtime::register(Box::new(crate::widget::Label::new(
            "body".to_string(),
            Rect::new(0, 0, 10, 10),
        )))
        .expect("mount content");

        // The dialog itself is owned here rather than mounted: placement addresses the child,
        // and `render_to_svg` calls `Draw` directly, which is the path a caller-held control
        // uses.
        let mut widget = Dialog::with_title("Settings", Rect::new(0, 0, 400, 300));
        widget.set_content_widget(Some(content_id));
        widget.open();
        assert!(widget.tick(30), "still growing after one step");
        let _ = crate::widget::svg::render_to_svg(&mut widget);
        let mid = runtime::geometry_of(content_id).expect("the content is mounted");

        while widget.tick(1000) {}
        let _ = crate::widget::svg::render_to_svg(&mut widget);
        let settled = runtime::geometry_of(content_id).expect("the content is mounted");

        // The resting box is 280x212 at (60, 58); a mid-reveal frame is scaled by less than
        // one about its centre, so the content box is strictly smaller and still inside it.
        assert_eq!(settled, Rect::new(60, 58, 280, 212));
        assert!(
            mid.width < settled.width && mid.height < settled.height,
            "a revealing dialog must place its content in the smaller frame: {mid:?} vs {settled:?}"
        );
        assert!(
            mid.x >= settled.x
                && mid.y >= settled.y
                && mid.x + mid.width as i32 <= settled.x + settled.width as i32
                && mid.y + mid.height as i32 <= settled.y + settled.height as i32,
            "the content must stay inside the frame: {mid:?} vs {settled:?}"
        );

        runtime::unregister(content_id);
    }

    #[test]
    fn title_bar_buttons_are_configurable_and_help_is_reported() {
        let mut d = dialog();
        assert!(d.title_bar_buttons().is_empty(), "the default chrome stays unchanged");
        d.set_title_bar_buttons(&[TitleBarButton::Close, TitleBarButton::Help]);
        assert_eq!(d.title_bar_buttons(), [TitleBarButton::Close, TitleBarButton::Help]);
        assert!(d.is_closable());
        // Duplicates collapse instead of painting two identical marks.
        d.set_title_bar_buttons(&[TitleBarButton::Help, TitleBarButton::Help]);
        assert_eq!(d.title_bar_buttons(), [TitleBarButton::Help]);
        assert!(!d.is_closable());

        d.set_title_bar_buttons(&[TitleBarButton::Close, TitleBarButton::Help]);
        d.show();
        let help = d
            .title_bar_button_rects()
            .into_iter()
            .find(|(button, _)| *button == TitleBarButton::Help)
            .map(|(_, rect)| rect)
            .expect("the help mark has a rectangle");
        let asked = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&asked);
        d.help_requested.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        d.handle_event(&Event::mouse_press(help.x + 2, help.y + 2, 1));
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        assert!(d.is_visible(), "help must not close the dialog");
    }
}
