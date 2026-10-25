// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Popup window widget.
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// Corner radius of the floating chrome, in logical pixels.
const FLOATING_CORNER_RADIUS: u32 = 8;
/// The blur radius of the floating chrome's shadow, in logical pixels.
const FLOATING_SHADOW_BLUR: u32 = 12;
/// The floating chrome's shadow offset below the popup, in logical pixels.
const FLOATING_SHADOW_OFFSET_Y: i32 = 4;
/// Side length of the bottom-right resize grip, in logical pixels.
const RESIZE_GRIP_SIZE: u32 = 14;
/// The smallest size a resize gesture may produce, in logical pixels.
const MIN_RESIZE: u32 = 32;

/// Popup window widget.
pub struct PopupWindow {
    base: BaseWidget,
    content_widget: Option<ObjectId>,
    /// Title shown in the popup's own chrome.
    ///
    /// Kept on the control rather than in a host-side map: the title is part of
    /// what the popup *is*, and a host that held it separately could not paint it.
    title: String,
    /// Emitted when the popup is opened.
    pub opened: GenericSignal,
    /// Emitted when the popup is closed.
    pub closed: GenericSignal,
    /// Whether this popup is modal: a modal popup is not dismissed by an outside click.
    modal: bool,
    /// The window this popup is transient for, if any.
    ///
    /// The link is declarative: the popup does not query the owner, but a host can read it
    /// to close the popup with its parent or to raise it above that window. `None` means the
    /// popup stands alone.
    transient_parent: Option<ObjectId>,
    /// Whether the popup paints floating chrome (rounded corners and a blurred shadow).
    ///
    /// Opt-in, so the default frame remains the square-bordered rectangle it has always
    /// been.
    floating_chrome: bool,
    /// Pointer position where a title-bar drag began, in window coordinates.
    drag_origin: Option<Point>,
    /// The popup's geometry at the moment that drag began.
    drag_geometry: Option<Rect>,
    /// Whether the bottom-right grip resizes the popup.
    ///
    /// Off by default, so a popup that never asks for it keeps the fixed-size behaviour it
    /// has always had.
    resizable: bool,
    /// Pointer position where a grip resize began, in window coordinates.
    resize_origin: Option<Point>,
    /// The popup's geometry at the moment that resize began.
    resize_geometry: Option<Rect>,
    /// Emitted with the new size whenever a resize gesture changes it.
    pub resized: Signal1<Size>,
}
impl PopupWindow {
    /// Creates a popup window with geometry and no title.
    pub fn new(geometry: Rect) -> Self {
        Self::with_title(String::new(), geometry)
    }

    /// Creates a popup window with a title and geometry.
    pub fn with_title(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow"),
            content_widget: None,
            title,
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
            modal: false,
            transient_parent: None,
            floating_chrome: false,
            drag_origin: None,
            drag_geometry: None,
            resizable: false,
            resize_origin: None,
            resize_geometry: None,
            resized: Signal1::new(),
        }
    }

    /// Returns whether the popup is modal.
    ///
    /// A modal popup stays up when the pointer clicks outside it; a non-modal one is
    /// dismissed by that click, which is the convention for a transient popup.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Sets whether the popup is modal and requests a redraw.
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        self.base.request_redraw();
    }

    /// Returns the window this popup is transient for, if any.
    pub fn transient_parent(&self) -> Option<ObjectId> {
        self.transient_parent
    }

    /// Sets (or clears) the window this popup is transient for.
    pub fn set_transient_parent(&mut self, parent: Option<ObjectId>) {
        if self.transient_parent == parent {
            return;
        }
        self.transient_parent = parent;
        self.base.request_redraw();
    }

    /// Returns whether the popup paints floating chrome.
    pub fn is_floating_chrome(&self) -> bool {
        self.floating_chrome
    }

    /// Enables or disables rounded corners plus a blurred shadow.
    ///
    /// Off by default: the square border is the frame this control has always painted, and
    /// the floating form is a visual choice a caller makes.
    pub fn set_floating_chrome(&mut self, floating: bool) {
        if self.floating_chrome == floating {
            return;
        }
        self.floating_chrome = floating;
        self.base.request_redraw();
    }

    /// Returns whether the pointer is over the popup.
    ///
    /// Hover is tracked from `MouseEnter`/`MouseLeave` and changes the painted surface by
    /// one step, so the flag has a visible consequence rather than being bookkeeping only.
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
    }

    /// Sets the pointer-hover flag; the setter exists for a host that synthesises hover.
    ///
    /// Named for the pointer rather than `set_hovered` so it does not shadow
    /// [`Widget::set_hovered`]: that trait entry re-resolves the widget's theme state,
    /// which would replace a caller's own style on the first hover, while this control's
    /// hover cue is derived in `draw` and needs only the flag and a repaint.
    pub fn set_pointer_hovered(&mut self, hovered: bool) {
        if self.base.is_hovered() == hovered {
            return;
        }
        self.base.set_hovered(hovered);
        self.base.request_redraw();
    }

    /// Returns whether the title-bar drag is in flight.
    pub fn is_dragging(&self) -> bool {
        self.drag_origin.is_some()
    }

    /// Whether the popup can be resized by its grip. Defaults to `false`.
    pub fn is_resizable(&self) -> bool {
        self.resizable
    }

    /// Enables or disables the resize grip.
    ///
    /// Disabling it also ends an in-flight resize, so a gesture cannot continue after the
    /// permission was withdrawn.
    pub fn set_resizable(&mut self, resizable: bool) {
        if self.resizable == resizable {
            return;
        }
        self.resizable = resizable;
        if !resizable {
            self.resize_origin = None;
            self.resize_geometry = None;
        }
        self.base.request_redraw();
    }

    /// The bottom-right resize grip's box, or `None` when the popup is not resizable.
    ///
    /// The grip is a real hit target, not just ink: a press inside it starts a resize instead
    /// of reaching the content underneath.
    pub fn resize_grip_rect(&self) -> Option<Rect> {
        if !self.resizable {
            return None;
        }
        let rect = self.base.geometry();
        let side = RESIZE_GRIP_SIZE.min(rect.width).min(rect.height);
        if side == 0 {
            return None;
        }
        Some(Rect::new(
            rect.x + rect.width as i32 - side as i32,
            rect.y + rect.height as i32 - side as i32,
            side,
            side,
        ))
    }

    /// Whether a grip resize is in flight.
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

    /// Ends a title-bar drag without moving the popup further.
    pub fn end_drag(&mut self) {
        if self.drag_origin.take().is_some() {
            self.drag_geometry = None;
            self.base.request_redraw();
        }
    }

    /// Advances an in-flight title-bar drag to `pos` (window coordinates).
    ///
    /// The geometry is recomputed from the drag's origin rather than accumulated, so a
    /// pointer that moves back to where it started restores the original position exactly.
    pub fn drag_to(&mut self, pos: Point) {
        let (Some(origin), Some(start)) = (self.drag_origin, self.drag_geometry) else {
            return;
        };
        let next = Rect::new(
            start.x + (pos.x - origin.x),
            start.y + (pos.y - origin.y),
            start.width,
            start.height,
        );
        if self.base.geometry() == next {
            return;
        }
        self.base.set_geometry(next);
        self.base.request_redraw();
    }

    /// Advances an in-flight grip resize to `pos` (window coordinates).
    ///
    /// The size is recomputed from the gesture's origin rather than accumulated, so moving
    /// the pointer back restores the original size exactly. The top-left corner is fixed and
    /// both dimensions are clamped to `MIN_RESIZE`.
    pub fn resize_to(&mut self, pos: Point) {
        let (Some(origin), Some(start)) = (self.resize_origin, self.resize_geometry) else {
            return;
        };
        let width = (start.width as i32 + (pos.x - origin.x)).max(MIN_RESIZE as i32) as u32;
        let height = (start.height as i32 + (pos.y - origin.y)).max(MIN_RESIZE as i32) as u32;
        self.apply_size(start, width, height);
    }

    /// Sets the popup's size and emits `resized` when it changes.
    ///
    /// This is the programmatic counterpart of the grip gesture: a host that recomputes a
    /// popup's size from its content does not have to fake a pointer drag. Both dimensions are
    /// clamped to `MIN_RESIZE`.
    pub fn set_size(&mut self, size: Size) {
        let rect = self.base.geometry();
        self.apply_size(rect, size.width.max(MIN_RESIZE), size.height.max(MIN_RESIZE));
    }

    /// Writes a new size anchored at `anchor`'s top-left corner and reports a real change.
    fn apply_size(&mut self, anchor: Rect, width: u32, height: u32) {
        let next = Rect::new(anchor.x, anchor.y, width, height);
        if self.base.geometry() == next {
            return;
        }
        self.base.set_geometry(next);
        self.resized.emit(Size::new(width, height));
        self.base.request_redraw();
    }

    /// Whether `pos` is inside the popup's title bar.
    fn is_in_title_bar(&self, pos: Point) -> bool {
        let rect = self.base.geometry();
        if self.title.is_empty() || !rect.contains_point(pos) {
            return false;
        }
        let bar_height = TITLE_BAR_HEIGHT.min(rect.height) as i32;
        pos.y < rect.y + bar_height
    }

    /// Returns the popup's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the popup's title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// Returns the content widget ID, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget for this popup.
    ///
    /// Both directions of the link are maintained, including on **replacement**: the previous
    /// content is unlinked rather than left in the child list. `add_child` does not reject
    /// duplicates and nothing else removes the old id, so setting a second content widget used to
    /// leave the first attached forever — a walk would still reach it. See BLUE22 §B.6 rule 5.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if let Some(old) = self.content_widget {
            if Some(old) != widget {
                self.base.remove_child_linked(old);
                // See `Frame::set_widget`: a replaced child must stop claiming to be visible.
                crate::set_widget_visible(old, false);
            }
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child_linked(id);
            crate::set_widget_visible(id, true);
        }
        self.base.request_redraw();
    }

    /// Shows the popup and emits `opened`.
    pub fn open(&mut self) {
        self.show();
        self.opened.emit();
    }

    /// Hides the popup and emits `closed`.
    pub fn close(&mut self) {
        self.hide();
        // Base lifecycle signal (handle-layer `on_close` / JSON `closed`) alongside the widget's
        // own `closed` for its direct listeners.
        self.base.closed.emit();
        self.closed.emit();
    }
}
impl Widget for PopupWindow {
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
        crate::core::Size::new(300, 200)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `PopupWindow` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

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
            "opened" => Some(EventSignalRef::unit("opened", &self.opened)),
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            "resized" => Some(EventSignalRef::mapped("resized", &self.resized, |size| {
                CapabilityValue::Tuple(vec![
                    CapabilityValue::UInt(size.width as u64),
                    CapabilityValue::UInt(size.height as u64),
                ])
            })),
            _ => None,
        }
    }
}

/// `PopupWindow`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. `has_content` is read-only: a popup's
/// content is supplied by its owner, not through the property layer.
impl WidgetProperties for PopupWindow {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "has_content" => Ok(CapabilityValue::Bool(self.content_widget().is_some())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            // `0` means "no transient parent", so the relationship is clearable over the
            // property route as well as through the setter.
            "transient_parent" => Ok(CapabilityValue::UInt(self.transient_parent().unwrap_or(0))),
            "floating_chrome" => Ok(CapabilityValue::Bool(self.is_floating_chrome())),
            "resizable" => Ok(CapabilityValue::Bool(self.is_resizable())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            // A popup's content is supplied by its owner via `set_content_widget`,
            // not through the property layer: the property reports presence only.
            "has_content" => Err(CapabilityAccessError::ReadOnlyProperty),
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            "transient_parent" => {
                let id = expect_u32(value)?;
                self.set_transient_parent(if id == 0 { None } else { Some(u64::from(id)) });
                Ok(())
            }
            "floating_chrome" => {
                self.set_floating_chrome(expect_bool(value)?);
                Ok(())
            }
            "resizable" => {
                self.set_resizable(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "has_content",
            "modal",
            "transient_parent",
            "floating_chrome",
            "resizable",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `popup_window` publishes.
    ///
    /// `set_content_widget` takes an `ObjectId` payload, so it is answered through
    /// the caller's own handle rather than the scalar property route; reporting
    /// `OutOfRange` for a payload-less call is the established convention.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_content_widget" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl Draw for PopupWindow {
    /// Paints the popup's own chrome, then its content child.
    ///
    /// # Why the title is painted here
    ///
    /// The struct documents the title as "part of what the popup *is*" and keeps it
    /// on the control "rather than in a host-side map: ... a host that held it
    /// separately could not paint it". That reasoning only holds if this function
    /// actually paints it, and it did not — the title was stored, published as a
    /// read/write property, and never rendered. An empty title now means "no title
    /// bar", so a popup created through [`PopupWindow::new`] keeps the titleless
    /// chrome it had before.
    ///
    /// # Child clipping
    ///
    /// A content child is drawn by the host's tree walk, not from here, so this only
    /// needs to leave the content area free of chrome. The title bar is taken out of
    /// the top of the content rect so a child laid out at the popup's origin does not
    /// sit under the title text.
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so
        // a light/dark switch left the popup and its title bar unchanged — the rendering
        // census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // One theme-manager read supplies the window fill and the fallback ink/border; the old
        // path resolved a widget style and then took the manager lock again (G-P13).
        let (window_fill, theme_ink, theme_border) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    Some(active.colors.foreground),
                    Some(active.colors.outline),
                ),
                None => (Color::WHITE, None, None),
            }
        };
        let ink = style.text_color.or(theme_ink).unwrap_or(Color::rgb(40, 40, 40));
        let surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        // Hover is a state the control tracks, so it has to change something. One step toward
        // the ink is enough to read as "the pointer is over this window" without altering the
        // resting frame, which is what keeps the default snapshot valid.
        let surface = if self.base.is_hovered() { surface.blend(&ink, 0.03) } else { surface };
        let border = style
            .border_color
            .or(theme_border)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.35));
        // The title bar is a distinct band on the popup, derived from it so the two stay one
        // visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.06);

        // Background and border. The fill is opaque on purpose — a popup that wants
        // translucency has to say so through its own background property, and the
        // earlier "semi-transparent effect" comment described something this code
        // never did.
        //
        // Two chrome forms: the historical square panel, and the floating form a detachable
        // window uses (rounded corners over a blurred shadow). The floating form is opt-in so
        // the default frame is byte-identical.
        if self.floating_chrome {
            context.draw_box_shadow(
                rect,
                Color::rgba(0, 0, 0, 48),
                0,
                FLOATING_SHADOW_OFFSET_Y,
                FLOATING_SHADOW_BLUR,
                0,
            );
            context.fill_rounded_rect(rect, FLOATING_CORNER_RADIUS, surface);
            context.draw_rounded_rect_stroke(rect, FLOATING_CORNER_RADIUS, border, 1);
        } else {
            context.face_with_gradient(
                rect,
                surface,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }

        // The resize grip is painted even for a titleless popup, which is why it sits before
        // the title-bar early return. Three short diagonals are the conventional "drag this
        // corner" mark and need no font glyph.
        if let Some(grip) = self.resize_grip_rect() {
            let ink = border.with_alpha(170);
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
                    ink,
                );
            }
        }

        if self.title.is_empty() {
            return;
        }
        // Title bar height is clamped to the popup so a short popup shows the title
        // rather than painting outside itself.
        let bar_height = TITLE_BAR_HEIGHT.min(rect.height);
        // On the rounded form the band is inset by the corner radius, which is exactly the
        // width of the card's flat top edge; the square form keeps the full width.
        let bar_inset = if self.floating_chrome { FLOATING_CORNER_RADIUS } else { 0 };
        context.fill_rect(
            Rect::new(
                rect.x + bar_inset as i32,
                rect.y,
                rect.width.saturating_sub(bar_inset * 2),
                bar_height,
            ),
            title_bar,
        );
        context.draw_line(
            Point::new(rect.x + bar_inset as i32, rect.y + bar_height as i32),
            Point::new(rect.x + rect.width as i32 - bar_inset as i32, rect.y + bar_height as i32),
            border,
        );
        // The title is centred inside the bar. The origin is the glyph's **top** edge, so the
        // offset is half the difference between the bar and the line box; using the bar's own
        // midline put the glyph's top *at* the centre, leaving a 14 px title spanning 12..26 in
        // a 24 px bar — two pixels over the separator below it.
        // The **effective font** — the resolved theme/caller font — so the popup title honours the
        // theme body font and the user's text scale (D09-STYLE-01).
        let title_font = effective_font(style);
        let title_h = context.measure_text("M", title_font).height;
        // The title-bar close button occupies the strip's trailing edge, so the title elides
        // before it rather than running underneath (DLG-18).
        let close_button = self.close_button_rect();
        let title_width = match close_button {
            Some(button) => button.x.saturating_sub(rect.x + 8).max(0) as u32,
            None => rect.width.saturating_sub(16),
        };
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + 8,
                rect.y + (bar_height as i32 - title_h as i32) / 2,
                title_width,
                title_h,
            ),
            &self.title,
            title_font,
            ink,
            HorizontalAlignment::Left,
        );
        if let Some(button) = close_button {
            crate::widget::draw_icon_centered(context, button, button.height, ink, IconName::Close);
        }
    }
}

/// Height of the popup's title bar, in pixels.
///
/// Also the amount the content area is inset by when a title is present, so the two
/// cannot drift apart.
pub const TITLE_BAR_HEIGHT: u32 = 24;

impl PopupWindow {
    /// The title-bar close button, or `None` for a titleless popup.
    ///
    /// The paint and the press path both read this one derivation, so the mark and its target
    /// cannot drift (DLG-18).
    fn close_button_rect(&self) -> Option<Rect> {
        if self.title.is_empty() {
            return None;
        }
        let rect = self.base.geometry();
        let bar_height = TITLE_BAR_HEIGHT.min(rect.height);
        if bar_height == 0 || rect.width < 32 {
            return None;
        }
        let side = 14.min(bar_height.saturating_sub(4)).max(1);
        let inset = if self.floating_chrome { FLOATING_CORNER_RADIUS } else { 0 };
        Some(Rect::new(
            rect.x + rect.width as i32 - inset as i32 - side as i32 - 6,
            rect.y + ((bar_height - side) / 2) as i32,
            side,
            side,
        ))
    }

    /// The rectangle available to the content child.
    ///
    /// Insets the top by the title bar **only when a title is set**, so a titleless
    /// popup gives its child the full rect.
    pub fn content_rect(&self) -> Rect {
        let rect = self.base.geometry();
        if self.title.is_empty() {
            return rect;
        }
        let inset = TITLE_BAR_HEIGHT.min(rect.height);
        Rect::new(rect.x, rect.y + inset as i32, rect.width, rect.height - inset)
    }
}
impl EventHandler for PopupWindow {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseEnter { .. } => {
                self.set_pointer_hovered(true);
            }
            Event::MouseLeave { .. } => {
                self.end_drag();
                self.end_resize();
                self.set_pointer_hovered(false);
            }
            _ => {}
        }
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                // Click-outside dismissal: a non-modal popup is a transient surface, so a
                // press anywhere else closes it. A modal one is kept up by definition.
                if !self.modal
                    && self.base.is_visible()
                    && !self.base.geometry().contains_point(*pos)
                {
                    self.close();
                    return;
                }
                // The grip wins over the title bar and over the content: it is the only
                // affordance that says "this corner resizes me".
                if self.resize_grip_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.resize_origin = Some(*pos);
                    self.resize_geometry = Some(self.base.geometry());
                    return;
                }
                // The title bar's close button is a press target before the drag bar, so a
                // click on the mark closes the popup instead of starting a move (DLG-18).
                if self.close_button_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.close();
                    return;
                }
                // A press on the title bar starts moving the window; a press in the body is
                // the content's business.
                if self.is_in_title_bar(*pos) {
                    self.drag_origin = Some(*pos);
                    self.drag_geometry = Some(self.base.geometry());
                }
            }
            Event::MouseMove { pos } => {
                if self.is_resizing() {
                    self.resize_to(*pos);
                } else {
                    self.drag_to(*pos);
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(false);
                self.end_drag();
                self.end_resize();
            }
            // Escape is the keyboard's close affordance. A popup is a floating surface with
            // no other keyboard exit, so the key goes through the same `close` path as the
            // host's own close request (which emits both lifecycle signals).
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE && self.base.is_visible() => {
                self.close();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::object::Object;
    use crate::widget::capability::types::CapabilityValue;
    use crate::widget::capability::WidgetProperties;
    use std::sync::{Arc, Mutex};

    #[test]
    fn popup_open_close_emits_lifecycle_signals() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        let opened = Arc::new(Mutex::new(0usize));
        let closed = Arc::new(Mutex::new(0usize));

        let opened_sink = opened.clone();
        popup.opened.connect(move || {
            if let Ok(mut count) = opened_sink.lock() {
                *count += 1;
            }
        });

        let closed_sink = closed.clone();
        popup.closed.connect(move || {
            if let Ok(mut count) = closed_sink.lock() {
                *count += 1;
            }
        });

        popup.open();
        popup.close();

        assert!(!popup.is_visible());
        assert_eq!(*opened.lock().expect("opened lock poisoned"), 1);
        assert_eq!(*closed.lock().expect("closed lock poisoned"), 1);
    }

    #[test]
    fn popup_replaces_content_widget_child_binding() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        let old_id = Object::new("OldContent").id();
        let new_id = Object::new("NewContent").id();

        popup.set_content_widget(Some(old_id));
        assert_eq!(popup.content_widget(), Some(old_id));
        assert_eq!(popup.children(), &[old_id]);

        popup.set_content_widget(Some(new_id));
        assert_eq!(popup.content_widget(), Some(new_id));
        assert_eq!(popup.children(), &[new_id]);
    }

    /// The title must reach the pixels, not just the property table.
    ///
    /// `PopupWindow` stores a title, publishes it as a read/write property, and
    /// documents it as something the control owns *because* a host could not paint it.
    /// None of that was true while `draw` ignored the field. This asserts the rendered
    /// frame differs once a title is set, which is the only evidence that survives a
    /// future refactor of the chrome.
    #[test]
    fn popup_with_a_title_paints_chrome_the_titleless_one_does_not() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut titleless = PopupWindow::new(Rect::new(0, 0, 160, 100));
        let mut titled = PopupWindow::with_title("Details".to_string(), Rect::new(0, 0, 160, 100));

        let plain = crate::widget::svg::render_to_svg(&mut titleless);
        let decorated = crate::widget::svg::render_to_svg(&mut titled);

        assert_ne!(plain, decorated, "a titled popup must paint more than a titleless one");
        // The title is `font8x8` glyph geometry rather than a `<text>` element, so the string is
        // not in the document and the check is on **where the ink landed**: the titled popup
        // gains a run on its title bar, and the titleless one paints no text at all.
        let (left, top, right, bottom) = crate::widget::svg::text_ink_box(&decorated)
            .unwrap_or_else(|| panic!("a titled popup must paint its title: {decorated}"));
        assert!(right > left, "the title laid down ink: {left}..{right}");
        assert!(
            (0..TITLE_BAR_HEIGHT as i32).contains(&top) && bottom <= TITLE_BAR_HEIGHT as i32,
            "the title must sit on the title bar, got {top}..{bottom}"
        );
        assert!(
            crate::widget::svg::text_ink_box(&plain).is_none(),
            "a titleless popup paints no title ink: {plain}"
        );
    }

    /// A titleless popup keeps the whole rect for its content.
    #[test]
    fn popup_content_rect_insets_only_when_a_title_is_present() {
        let titleless = PopupWindow::new(Rect::new(0, 0, 160, 100));
        assert_eq!(titleless.content_rect(), Rect::new(0, 0, 160, 100));

        let titled = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 160, 100));
        assert_eq!(
            titled.content_rect(),
            Rect::new(0, TITLE_BAR_HEIGHT as i32, 160, 100 - TITLE_BAR_HEIGHT)
        );
    }

    /// A popup shorter than its own title bar must still render inside itself.
    #[test]
    fn popup_title_bar_clamps_to_a_short_popup() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut tiny = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 60, 8));
        // The assertion is that drawing does not panic and does not escape the rect;
        // the clamp is what makes the second half true.
        let _ = crate::widget::svg::render_to_svg(&mut tiny);
        assert_eq!(tiny.content_rect().height, 0);
    }

    /// Escape closes the popup through the same path as a host close request.
    #[test]
    fn popup_escape_closes_once() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        popup.open();
        let closed = Arc::new(Mutex::new(0usize));
        popup.closed.connect({
            let sink = Arc::clone(&closed);
            move || {
                if let Ok(mut count) = sink.lock() {
                    *count += 1;
                }
            }
        });

        popup.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert!(!popup.is_visible());
        assert_eq!(*closed.lock().expect("closed lock"), 1);

        popup.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert_eq!(*closed.lock().expect("closed lock"), 1, "a hidden popup stays quiet");
    }

    /// An outside click dismisses a transient popup but not a modal one.
    #[test]
    fn popup_outside_click_respects_modality() {
        let outside = Event::MousePress { pos: Point::new(400, 400), button: 1, modifiers: 0 };

        let mut transient = PopupWindow::new(Rect::new(0, 0, 120, 80));
        transient.open();
        transient.handle_event(&outside);
        assert!(!transient.is_visible(), "a non-modal popup is dismissed by an outside click");

        let mut modal = PopupWindow::new(Rect::new(0, 0, 120, 80));
        modal.set_modal(true);
        modal.open();
        modal.handle_event(&outside);
        assert!(modal.is_visible(), "a modal popup is kept up by an outside click");
        assert!(modal.is_modal());

        // The transient-parent link is declarative state a host can read.
        modal.set_transient_parent(Some(7));
        assert_eq!(modal.transient_parent(), Some(7));
        assert_eq!(
            modal.get("transient_parent"),
            Ok(CapabilityValue::UInt(7)),
            "the property route reports the owner id"
        );
        modal.set_transient_parent(None);
        assert_eq!(modal.get("transient_parent"), Ok(CapabilityValue::UInt(0)));
    }

    /// The title bar drags the popup; the body does not.
    #[test]
    fn popup_title_bar_drags_the_window() {
        let mut popup = PopupWindow::with_title("Drag me".to_string(), Rect::new(0, 0, 160, 100));

        // A press below the title bar is the content's business.
        popup.handle_event(&Event::MousePress {
            pos: Point::new(10, TITLE_BAR_HEIGHT as i32 + 10),
            button: 1,
            modifiers: 0,
        });
        assert!(!popup.is_dragging(), "only the title bar starts a drag");

        popup.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(popup.is_dragging());
        popup.handle_event(&Event::MouseMove { pos: Point::new(30, 25) });
        assert_eq!(popup.geometry(), Rect::new(20, 15, 160, 100));
        // Moving back restores the original position exactly, because the delta is taken
        // from the press origin rather than accumulated.
        popup.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        assert_eq!(popup.geometry(), Rect::new(0, 0, 160, 100));

        popup.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!popup.is_dragging());
    }

    /// The floating chrome is opt-in: rounded corners and a real blurred shadow.
    #[test]
    fn popup_floating_chrome_is_opt_in() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut square = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 160, 100));
        let square_svg = crate::widget::svg::render_to_svg(&mut square);
        assert!(!square_svg.contains("shadow_blur_"), "the default frame casts no shadow");
        assert!(!square_svg.contains("rx=\""), "the default frame is square");

        let mut floating = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 160, 100));
        floating.set_floating_chrome(true);
        let floating_svg = crate::widget::svg::render_to_svg(&mut floating);
        assert!(floating.is_floating_chrome());
        assert!(
            floating_svg.contains("shadow_blur_"),
            "the floating frame must cast a blurred shadow: {floating_svg}"
        );
        assert!(floating_svg.contains("rx=\""), "the floating frame is rounded");
        assert_ne!(square_svg, floating_svg);
    }

    /// Hover is tracked and has a visible consequence.
    #[test]
    fn popup_hover_changes_the_painted_frame() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut popup = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 160, 100));
        let resting = crate::widget::svg::render_to_svg(&mut popup);
        assert!(!popup.is_hovered());

        popup.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(popup.is_hovered());
        let hovered = crate::widget::svg::render_to_svg(&mut popup);
        assert_ne!(resting, hovered, "hover must be visible, not just a flag");

        popup.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert!(!popup.is_hovered());
        assert_eq!(resting, crate::widget::svg::render_to_svg(&mut popup));
    }

    /// The resize grip is opt-in, changes the size, and reports the new size.
    #[test]
    fn popup_resize_grip_changes_the_size_and_emits_resized() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 160, 100));
        assert!(!popup.is_resizable());
        assert!(popup.resize_grip_rect().is_none(), "a fixed popup advertises no grip");

        let sizes = Arc::new(Mutex::new(Vec::new()));
        popup.resized.connect({
            let sink = Arc::clone(&sizes);
            move |size| sink.lock().expect("sizes").push(*size)
        });
        popup.set_resizable(true);
        let grip = popup.resize_grip_rect().expect("a resizable popup has a grip");
        assert_eq!(grip.x + grip.width as i32, 160);
        assert_eq!(grip.y + grip.height as i32, 100);

        popup.handle_event(&Event::MousePress {
            pos: Point::new(grip.x + 2, grip.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert!(popup.is_resizing());
        popup.handle_event(&Event::MouseMove { pos: Point::new(grip.x + 32, grip.y + 22) });
        assert_eq!(popup.geometry(), Rect::new(0, 0, 190, 120));
        assert_eq!(&*sizes.lock().expect("sizes"), &[Size::new(190, 120)]);

        popup.handle_event(&Event::MouseRelease {
            pos: Point::new(grip.x + 32, grip.y + 22),
            button: 1,
        });
        assert!(!popup.is_resizing());

        // A shrink is clamped to the minimum, not to zero or a negative size.
        popup.set_resizable(false);
        popup.set_resizable(true);
        let grip = popup.resize_grip_rect().expect("grip");
        popup.handle_event(&Event::MousePress {
            pos: Point::new(grip.x + 2, grip.y + 2),
            button: 1,
            modifiers: 0,
        });
        popup.handle_event(&Event::MouseMove { pos: Point::new(-400, -400) });
        assert_eq!(popup.geometry().width, MIN_RESIZE);
        assert_eq!(popup.geometry().height, MIN_RESIZE);

        // The programmatic size setter is the same change, reported the same way.
        popup.set_size(Size::new(200, 150));
        assert_eq!(popup.geometry(), Rect::new(0, 0, 200, 150));
        assert_eq!(sizes.lock().expect("sizes").last(), Some(&Size::new(200, 150)));
    }
}
