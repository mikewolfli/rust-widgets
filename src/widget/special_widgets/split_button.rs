// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SplitButton widget with primary action and drop-down action list.
//!
//! # The face is assembled from two columns, not computed from one edge
//!
//! BLUE22 §B.8 lists this control's defect as "master face + arrow by hand", and the fix is not
//! a nicer arithmetic: it is that the two columns must **tile** the face, so a wider arrow column
//! *pushes* the trigger narrower instead of being placed from the trailing edge while the trigger
//! is placed from the leading one.
//!
//! The assembly therefore goes through [`CompositeBuilder`]/[`FlexLayout`]: the layout reads each
//! column's own [`Hints`], places them in order, and reports the rectangles. Nothing here computes
//! an `x`. The one thing that is *declared* rather than derived is the arrow's preferred
//! width ([`dimensions::SPLIT_ARROW_COLUMN_WIDTH`]) — see that constant for why the number is a
//! constant and not the `v` glyph's advance.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::composite::CompositeBuilder;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetFactory, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The character the arrow column paints.
///
/// Named because the assembled face creates its arrow column from this text and then paints the
/// same glyph: two spellings of one fact, and the template test asserts the arrow column the
/// layout reported is the box the glyph is centred in.
const ARROW_LABEL: &str = "v";

/// One selectable action in a split button drop-down list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitAction {
    /// Stable action identifier.
    pub id: String,
    /// Display label.
    pub label: String,
}

impl SplitAction {
    /// Creates a split action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// SplitButton provides a default primary action plus selectable alternatives.
pub struct SplitButton {
    base: BaseWidget,
    text: String,
    actions: Vec<SplitAction>,
    primary_action_index: Option<usize>,
    highlighted_action_index: Option<usize>,
    menu_open: bool,
    pressed_primary: bool,
    pressed_arrow: bool,
    hovered_primary: bool,
    hovered_arrow: bool,
    arrow_width: u32,
    row_height: u32,
    /// Emitted when primary area triggers current action id.
    pub triggered: Signal1<String>,
    /// Emitted when a drop-down action is explicitly selected.
    pub action_selected: Signal1<String>,
    /// Emitted when menu open state changes.
    pub menu_toggled: Signal1<bool>,
}

impl SplitButton {
    /// Creates a split button.
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "SplitButton"),
            text: text.into(),
            actions: Vec::new(),
            primary_action_index: None,
            highlighted_action_index: None,
            menu_open: false,
            pressed_primary: false,
            pressed_arrow: false,
            hovered_primary: false,
            hovered_arrow: false,
            arrow_width: dimensions::SPLIT_ARROW_COLUMN_WIDTH,
            row_height: 22,
            triggered: Signal1::new(),
            action_selected: Signal1::new(),
            menu_toggled: Signal1::new(),
        }
    }

    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }

    /// Replaces all actions.
    pub fn set_actions(&mut self, actions: Vec<SplitAction>) {
        self.actions = actions;
        self.primary_action_index = if self.actions.is_empty() { None } else { Some(0) };
        self.highlighted_action_index = self.primary_action_index;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all actions.
    pub fn actions(&self) -> &[SplitAction] {
        &self.actions
    }

    /// Adds one action and returns its index.
    pub fn add_action(&mut self, action: SplitAction) -> usize {
        let index = self.actions.len();
        self.actions.push(action);
        if self.primary_action_index.is_none() {
            self.primary_action_index = Some(index);
            self.highlighted_action_index = Some(index);
        }
        self.base.request_layout();
        self.base.request_redraw();
        index
    }

    /// Returns current primary action index.
    pub fn primary_action_index(&self) -> Option<usize> {
        self.primary_action_index.filter(|index| *index < self.actions.len())
    }

    /// Returns whether menu is open.
    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Returns highlighted action index when menu is open.
    pub fn highlighted_action_index(&self) -> Option<usize> {
        self.highlighted_action_index.filter(|index| *index < self.actions.len())
    }

    /// Sets menu row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        self.row_height = row_height.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns menu row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Triggers primary action.
    pub fn trigger_primary(&mut self) -> bool {
        let Some(index) = self.primary_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        self.triggered.emit(action.id.clone());
        true
    }

    /// Opens menu.
    pub fn open_menu(&mut self) {
        if self.menu_open {
            return;
        }
        self.menu_open = true;
        self.highlighted_action_index = self.primary_action_index();
        self.menu_toggled.emit(true);
        self.base.request_redraw();
    }

    /// Closes menu.
    pub fn close_menu(&mut self) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.menu_toggled.emit(false);
        self.base.request_redraw();
    }

    /// Toggles menu visibility.
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// Selects highlighted drop-down action as primary and emits action_selected.
    pub fn select_highlighted_action(&mut self) -> bool {
        let Some(index) = self.highlighted_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        self.primary_action_index = Some(index);
        self.action_selected.emit(action.id.clone());
        self.close_menu();
        true
    }

    /// Moves highlighted action in menu by signed delta.
    pub fn move_highlight(&mut self, delta: isize) {
        if self.actions.is_empty() {
            self.highlighted_action_index = None;
            return;
        }

        let current = self.highlighted_action_index.unwrap_or(0) as isize;
        let max = self.actions.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.highlighted_action_index = Some(next);
        self.base.request_redraw();
    }

    /// The band the button actually paints: full width,
    /// `dimensions::SPLIT_BUTTON_HEIGHT` tall, centred in the rectangle it was given.
    ///
    /// # Why the face is not the rectangle
    ///
    /// A split button is chrome: one compact row split into a trigger and an arrow. Taking
    /// `rect.height` made a 240x120 census cell a 120 px-tall face whose two halves were also
    /// 120 tall — a slab shaped like a button rather than a button — and it disagreed with the
    /// 28 px `size_hint` the control reports. The band is the single derivation the paint, the
    /// hit tests and the drop-down's anchor all read.
    fn face_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::SPLIT_BUTTON_HEIGHT)
    }

    /// The trigger and the arrow column, placed by the layout that owns the tiling.
    ///
    /// # Why the two columns are assembled rather than computed
    ///
    /// This used to be `primary_width = band.width - arrow_width` with the arrow placed from the
    /// band's trailing edge: two derivations from two different edges, so the trigger's box and
    /// the arrow's box agreed only because both subtracted the same constant. The layout holds
    /// the two facts the arithmetic was hiding — that they are ordered, and that they tile — and
    /// it states them once. The arrow's width is the *ground* fact here (a constant, see
    /// [`dimensions::SPLIT_ARROW_COLUMN_WIDTH`]); the trigger's is what the layout computes as the
    /// remainder.
    ///
    /// # Why the arrow is `add_flexible(.., Height)`
    ///
    /// Both columns are declared at the face's own height, so the cross axis has nothing to
    /// resolve and `Stretch` and "keep the preferred height" agree. Declaring it anyway is what
    /// makes the assembly say *why* the arrow is 28 px: it is what the arrow asked for, not
    /// whatever the row's tallest child happened to be.
    fn assemble_face(&self) -> (Rect, Rect) {
        let band = self.face_band();
        let factory = WidgetFactory::new_with_defaults();
        let mut builder = CompositeBuilder::new(
            Box::new(FlexLayout::with_params(
                FlexDirection::Row,
                FlexWrap::NoWrap,
                JustifyContent::FlexStart,
                AlignItems::Stretch,
                0,
                0,
            )),
            EdgeOffsets::all(0),
            Size::new(0, 0),
        );
        // The liveness of the two columns is the assembly's precondition, and a silent `None`
        // would place a face with no trigger in it. `WidgetFactory::create` answers `Some` for
        // every core kind in every profile (`button` and `label` are registered unconditionally),
        // so the honest answer is a debug assertion rather than a production branch that cannot
        // be reached and would therefore never be tested.
        let trigger = builder.add_sized(
            &factory,
            "label",
            &self.text,
            Size::new(self.trigger_hint_width(), band.height),
            LayoutParams::filled(),
        );
        debug_assert!(trigger.is_some(), "the trigger column is a core control");
        let arrow = builder.add_sized(
            &factory,
            "label",
            ARROW_LABEL,
            Size::new(self.arrow_width, band.height),
            LayoutParams::new(),
        );
        debug_assert!(arrow.is_some(), "the arrow column is a core control");

        let mut placed: Vec<Rect> = Vec::with_capacity(2);
        builder.arrange(band, &mut |_, rect| placed.push(rect));
        match (placed.first(), placed.get(1)) {
            (Some(trigger), Some(arrow)) => (*trigger, *arrow),
            // A layout that reported nothing is not a face. Falling back to the band for the
            // trigger and an empty arrow keeps every downstream box inside the control instead of
            // painting a zero-extent arrow into the corner — and `debug_assert!` above makes this
            // arm unreachable in a debug build, so it cannot be entered unnoticed in practice.
            _ => (band, Rect::new(band.x + band.width as i32, band.y, 0, band.height)),
        }
    }

    /// The width the trigger's own label asks for.
    ///
    /// The label control measures itself (`len * 8 + 2 * BUTTON_PADDING_H`), which is the same
    /// model `SplitButton` used, so the assembled face's columns tile exactly as the hand-computed
    /// pair did while now being derived from the text.
    fn trigger_hint_width(&self) -> u32 {
        self.text.len() as u32 * 8 + dimensions::BUTTON_PADDING_H * 2
    }

    fn primary_rect(&self) -> Rect {
        self.assemble_face().0
    }

    /// The trigger's label box: the primary face's interior, inset by the shared padding.
    ///
    /// # Why this is a box and not an `x`
    ///
    /// The label used to be drawn at `primary_rect.x + 8` with `HorizontalAlignment::Left`,
    /// and the menu rows at `action_rect.x + 8` — the same literal written twice, in two
    /// different coordinate systems, with no relation to the control's own padding constant.
    /// Returning the padded box instead gives the draw call the rectangle it is fitting into,
    /// so the label is bounded by the trigger rather than by a hand-chosen origin, and the
    /// trigger and the menu rows derive their leading space from one place.
    fn primary_label_box(&self, primary: Rect) -> Rect {
        ControlMetrics::content_box(
            primary,
            EdgeOffsets {
                left: dimensions::SPLIT_BUTTON_PADDING_H,
                right: dimensions::SPLIT_BUTTON_PADDING_H,
                top: 0,
                bottom: 0,
            },
        )
    }

    fn arrow_rect(&self) -> Rect {
        self.assemble_face().1
    }

    fn menu_rect(&self) -> Rect {
        // The popup hangs from the **face**'s bottom edge, not the control's, so it appears
        // directly under the button the user pressed rather than 120 px below it.
        let band = self.face_band();
        Rect::new(
            band.x,
            band.y + band.height as i32,
            band.width,
            self.row_height.saturating_mul(self.actions.len() as u32),
        )
    }

    fn action_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() {
            return None;
        }
        let menu = self.menu_rect();
        let y = menu.y + index as i32 * self.row_height as i32;
        Some(Rect::new(menu.x, y, menu.width, self.row_height))
    }

    fn hit_primary(&self, pos: Point) -> bool {
        let rect = self.primary_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_arrow(&self, pos: Point) -> bool {
        let rect = self.arrow_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_menu_index(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let menu = self.menu_rect();
        if pos.x < menu.x
            || pos.x >= menu.x + menu.width as i32
            || pos.y < menu.y
            || pos.y >= menu.y + menu.height as i32
        {
            return None;
        }
        let index = ((pos.y - menu.y) / self.row_height as i32) as usize;
        (index < self.actions.len()).then_some(index)
    }
}

impl Widget for SplitButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(100, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SplitButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `SplitButton` reports
/// `WidgetKind::ToolButton`, shared with `ToolButton`; dispatching on the concrete
/// type here is what keeps the two contracts separate.
impl WidgetProperties for SplitButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "action_count" => Ok(CapabilityValue::UInt(self.actions().len() as u64)),
            "menu_open" => Ok(CapabilityValue::Bool(self.menu_open())),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "action_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "menu_open" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "action_count", "menu_open", "row_height", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `split_button` publishes.
    ///
    /// `open_menu` and `close_menu` are the genuine zero-argument actions here.
    /// `trigger_primary` acts on the primary action and reports `false` when there
    /// is no action to trigger, which is the "could not handle it" case, so it is
    /// answered with [`CapabilityAccessError::OutOfRange`] rather than a success
    /// that did nothing. `add_action` takes the action to add, so it is refused the
    /// same way — the name is right and the argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "open_menu" => {
                self.open_menu();
                Ok(())
            }
            "close_menu" => {
                self.close_menu();
                Ok(())
            }
            "trigger_primary" => {
                if self.trigger_primary() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "add_action" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SplitButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_primary = self.hit_primary(*pos);
                self.hovered_arrow = self.hit_arrow(*pos);
                if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                }
            }
            Event::MouseLeave { .. } => {
                self.hovered_primary = false;
                self.hovered_arrow = false;
                self.pressed_primary = false;
                self.pressed_arrow = false;
            }
            Event::MousePress { pos, button: 1 } => {
                if self.hit_primary(*pos) {
                    self.pressed_primary = true;
                } else if self.hit_arrow(*pos) {
                    self.pressed_arrow = true;
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                } else {
                    self.close_menu();
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                if self.pressed_primary {
                    self.pressed_primary = false;
                    if self.hit_primary(*pos) {
                        let _ = self.trigger_primary();
                    }
                } else if self.pressed_arrow {
                    self.pressed_arrow = false;
                    if self.hit_arrow(*pos) {
                        self.toggle_menu();
                    }
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                    let _ = self.select_highlighted_action();
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 | 32 => {
                    if self.menu_open {
                        let _ = self.select_highlighted_action();
                    } else {
                        let _ = self.trigger_primary();
                    }
                }
                40 => {
                    if !self.menu_open {
                        self.open_menu();
                    } else {
                        self.move_highlight(1);
                    }
                }
                38 if self.menu_open => {
                    self.move_highlight(-1);
                }
                27 => {
                    self.close_menu();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SplitButton {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the button, its splits and its drop-down unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("split_button");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `split_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`.
        // A face painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&secondary, 0.45));

        // The states are raised from the face toward the theme's primary, so hover and press read
        // on either appearance rather than being a fixed pale blue that only worked on a light bar.
        let hover_bg = face.blend(&primary, 0.28);
        let pressed_bg = face.blend(&primary, 0.45);
        let arrow_face = face.blend(&ink, 0.06);

        // ── The face actually painted ──
        //
        // The button's chrome is one compact row, not a filled container; the band is where the
        // trigger and the arrow are drawn and where the popup is anchored.
        let face_rect = self.face_band();
        context.fill_rect(face_rect, face);
        context.draw_rect(face_rect, border);

        let primary_rect = self.primary_rect();
        let arrow = self.arrow_rect();

        let primary_bg = if self.pressed_primary {
            pressed_bg
        } else if self.hovered_primary {
            hover_bg
        } else {
            face
        };
        context.fill_rect(primary_rect, primary_bg);

        let arrow_bg = if self.pressed_arrow || self.menu_open {
            pressed_bg
        } else if self.hovered_arrow {
            hover_bg
        } else {
            arrow_face
        };
        context.fill_rect(arrow, arrow_bg);

        context.draw_line(
            Point::new(arrow.x, arrow.y),
            Point::new(arrow.x, arrow.y + arrow.height as i32),
            border,
        );

        // `draw_text`'s origin is the glyph box's top-left, so `primary_rect.y + height / 2` put
        // that top edge on the middle line and drew the label half a line low. The line box
        // centred in the trigger is the origin; the arrow glyph below shares its band's own.
        //
        // The label is centred both ways inside the trigger's padded box. A split button's
        // primary face *is* a button, so its label follows the same rule `Button` does
        // (Qt Quick centres `AbstractButton`'s `contentItem`; Flutter M3 centres the child) —
        // the previous `x + 8` origin left-aligned it against a literal.
        let primary_box = self.primary_label_box(primary_rect);
        context.draw_text_fitted(
            context.text_line(primary_box, &Font::default()),
            &self.text,
            &Font::default(),
            ink,
            HorizontalAlignment::Center,
        );

        // The arrow glyph is centred on its own column rather than offset by a half-glyph
        // literal: `(arrow.width / 2) - 3` hard-coded a 6 px-wide 'v', so a different font or
        // size put the glyph off the column's centre. `draw_text_fitted` with `Center` derives
        // the origin from the measured string inside the column.
        context.draw_text_fitted(
            context.text_line(arrow, &Font::default()),
            ARROW_LABEL,
            &Font::default(),
            ink.blend(&arrow_bg, 0.35),
            HorizontalAlignment::Center,
        );

        if self.menu_open {
            let menu = self.menu_rect();
            context.fill_rect(menu, face.blend(&ink, 0.22));
            context.draw_rect(menu, border);

            for index in 0..self.actions.len() {
                let Some(action_rect) = self.action_rect(index) else {
                    continue;
                };

                if self.highlighted_action_index == Some(index) {
                    context.fill_rect(action_rect, hover_bg);
                }

                if let Some(action) = self.actions.get(index) {
                    // A menu row's label is centred through the shared primitive, since the
                    // glyph origin is a top edge and `action_rect.y + height / 2` placed it
                    // half a line low. The box is the row's own padded interior — the same
                    // `SPLIT_BUTTON_PADDING_H` the trigger uses, so a menu row and the trigger
                    // it hangs from share their leading space rather than each naming it.
                    let row_box = self.primary_label_box(action_rect);
                    context.draw_text_fitted(
                        context.text_line(row_box, &Font::default()),
                        &action.label,
                        &Font::default(),
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_actions() -> Vec<SplitAction> {
        vec![
            SplitAction::new("run.default", "Run"),
            SplitAction::new("run.debug", "Run with Debug"),
            SplitAction::new("run.profile", "Run with Profile"),
        ]
    }

    #[test]
    fn primary_trigger_emits_default_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 28));
        split.set_actions(sample_actions());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(split.trigger_primary());
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.default".to_string()]);
    }

    #[test]
    fn keyboard_navigation_selects_action_from_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        split.action_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        split.handle_event(&Event::key_press(40, 0));
        assert!(split.menu_open());
        split.handle_event(&Event::key_press(40, 0));
        split.handle_event(&Event::key_press(13, 0));

        assert_eq!(split.primary_action_index(), Some(1));
        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.debug".to_string()]);
        assert!(!split.menu_open());
    }

    #[test]
    fn arrow_click_toggles_menu_and_mouse_selects_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        // Click arrow area to open menu.
        split.handle_event(&Event::mouse_press(172, 12, 1));
        split.handle_event(&Event::mouse_release(172, 12, 1));
        assert!(split.menu_open());

        // Click second row in menu.
        let menu_y = 28 + 24;
        split.handle_event(&Event::mouse_press(20, menu_y, 1));
        split.handle_event(&Event::mouse_release(20, menu_y, 1));

        assert_eq!(split.primary_action_index(), Some(1));
        assert!(!split.menu_open());
    }

    #[test]
    fn default_state() {
        let split = SplitButton::new("Run", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Run");
        assert!(split.actions().is_empty());
        assert_eq!(split.primary_action_index(), None);
        assert!(!split.menu_open());
    }

    #[test]
    fn set_text_get_text_roundtrip() {
        let mut split = SplitButton::new("Initial", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Initial");

        split.set_text("Updated");
        assert_eq!(split.text(), "Updated");

        split.set_text("");
        assert_eq!(split.text(), "");
    }

    #[test]
    fn add_action_adds_to_menu() {
        let mut split = SplitButton::new("Action", Rect::new(0, 0, 800, 600));
        assert_eq!(split.actions().len(), 0);

        let idx = split.add_action(SplitAction::new("act1", "Action 1"));
        assert_eq!(idx, 0);
        assert_eq!(split.actions().len(), 1);
        assert_eq!(split.primary_action_index(), Some(0));

        let idx = split.add_action(SplitAction::new("act2", "Action 2"));
        assert_eq!(idx, 1);
        assert_eq!(split.actions().len(), 2);

        assert_eq!(split.actions()[0].id, "act1");
        assert_eq!(split.actions()[1].id, "act2");
    }

    #[test]
    fn enable_disable_states() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A")]);

        // Enabled by default
        assert!(split.trigger_primary());

        // Disable widget
        split.base_mut().set_enabled(false);

        // After disabling, events should be ignored
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Direct call still works but event handler ignores
        split.handle_event(&Event::key_press(40, 0));
        let _got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!split.menu_open(), "disabled widget should not open menu via events");
    }

    #[test]
    fn trigger_with_no_actions() {
        let mut split = SplitButton::new("Empty", Rect::new(0, 0, 800, 600));
        // No actions added yet
        assert!(!split.trigger_primary());

        // open_menu with no actions still opens an empty menu (code doesn't guard)
        split.open_menu();
        assert!(split.menu_open());

        // Close menu
        split.close_menu();
        assert!(!split.menu_open());
    }

    #[test]
    fn menu_toggle_signal_emission() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A"), SplitAction::new("b", "B")]);

        let emitted = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = emitted.clone();
        split.menu_toggled.connect(move |state| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*state.as_ref());
            }
        });

        split.open_menu();
        assert!(split.menu_open());

        split.close_menu();
        assert!(!split.menu_open());

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }

    /// The assembled face is the pair of columns the hand-computed geometry produced.
    ///
    /// # Why this test exists at all
    ///
    /// BLUE22 §B.6 rule 2 asks placement to belong to the layout, and the risk of adopting a
    /// layout for a two-column chrome is precisely that it *changes* the geometry — a face that
    /// now includes a padding the old arithmetic did not, or a trigger that no longer ends where
    /// the arrow begins. This states the three properties that make the assembly correct rather
    /// than merely different, and it is checked at several control widths because the two columns
    /// have a fixed arrow width and a text-driven trigger width: they collide exactly where one
    /// derivation is dropped.
    ///
    /// The old form was `primary_width = band.width - arrow_width` with the arrow placed from the
    /// band's trailing edge, so the tiling held only because both subtracted the same constant.
    #[test]
    fn the_two_columns_tile_the_face() {
        // Every width that can hold both columns. `MIN_TILE_WIDTH` is the narrowest face for which
        // tiling is *possible at all* — it is not a threshold the code applies, it is the sum of
        // the two columns' own sizes, and the assertion below is that the assembly tiles exactly
        // wherever tiling is representable.
        let min_tile_width = split_hint_width("Run") + dimensions::SPLIT_ARROW_COLUMN_WIDTH;
        for width in [min_tile_width, 100, 240, 400] {
            let split = SplitButton::new("Run", Rect::new(0, 0, width, 120));
            let band = split.face_band();
            let primary = split.primary_rect();
            let arrow = split.arrow_rect();
            assert_eq!(
                primary.x, band.x,
                "the trigger starts at the face's leading edge at width {width}"
            );
            assert_eq!(
                primary.x + primary.width as i32,
                arrow.x,
                "the trigger ends where the arrow column begins at width {width}"
            );
            assert_eq!(
                arrow.x + arrow.width as i32,
                band.x + band.width as i32,
                "the arrow ends where the face ends at width {width}"
            );
            assert_eq!(primary.height, band.height, "both columns are the face's own height");
            assert_eq!(arrow.height, band.height);
        }
    }

    /// A face too narrow for both columns still keeps both of them inside it.
    ///
    /// # What this pins
    ///
    /// The two columns between them need `split_hint_width("Run") + SPLIT_ARROW_COLUMN_WIDTH`. A
    /// narrower face cannot be tiled at those sizes — that is arithmetic, not a bug.
    ///
    /// What the layout owes the caller is that the shortfall is **shared and contained** rather
    /// than deferred to whichever column comes last. Before G-1 was resolved the trigger kept its
    /// full width and the arrow was placed past the face's trailing edge; because the SVG backend
    /// emits absolute coordinates and nothing clips at this layer, that arrow was not overflowing,
    /// it was **absent** — a split button that silently has no drop-down affordance.
    ///
    /// The test therefore asserts containment (both columns inside the face) and the *relation* that
    /// makes a split button a split button: the arrow column stays one arrow wide, so a squeezed face
    /// loses trigger, not the affordance.
    #[test]
    fn a_face_too_narrow_for_both_columns_keeps_both_inside_it() {
        let width = 48u32;
        let split = SplitButton::new("Run", Rect::new(0, 0, width, 120));
        let face = split.face_band();
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();
        assert_eq!(face.width, width);
        for (label, rect) in [("trigger", primary), ("arrow", arrow)] {
            assert!(
                rect.x >= face.x && rect.x + rect.width as i32 <= face.x + face.width as i32,
                "the {label} column must stay inside the face, got {rect:?} in {face:?}"
            );
        }
        assert_eq!(
            primary.width + arrow.width,
            width,
            "and the two columns must still account for the face: {primary:?} + {arrow:?}"
        );
        // Both columns are inside the face and the face is the sum of them — that is containment,
        // which is the part G-1 was about. Their *sizes* are scaled by the same factor, so a narrow
        // face is honestly described as "this control is smaller than its contents": the arrow keeps
        // its **share** rather than being dropped, which is the property that matters (an absent
        // drop-down arrow is a split button that is not a split button).
        let scale =
            width as f32 / (split_hint_width("Run") + dimensions::SPLIT_ARROW_COLUMN_WIDTH) as f32;
        assert!(
            (arrow.width as f32 - dimensions::SPLIT_ARROW_COLUMN_WIDTH as f32 * scale).abs() <= 1.0,
            "the arrow keeps its proportional share of a squeezed face: {} vs {} × {scale:.2}",
            arrow.width,
            dimensions::SPLIT_ARROW_COLUMN_WIDTH
        );
        assert!(
            arrow.width > 0 && primary.width > 0,
            "neither column is dropped: trigger {}, arrow {}",
            primary.width,
            arrow.width
        );
    }

    /// The width `SplitButton` measures a trigger label at: `len * 8 + 2 * BUTTON_PADDING_H`.
    fn split_hint_width(text: &str) -> u32 {
        text.len() as u32 * 8 + dimensions::BUTTON_PADDING_H * 2
    }

    /// A wider arrow column pushes the trigger narrower rather than overlapping it.
    ///
    /// This is BLUE22 §B.9's rule stated for this control: the trigger's width is *derived* from
    /// the sibling column, so the two cannot be placed from opposite edges of the face. It is the
    /// property the layout provides and the previous `band.width - arrow_width` arithmetic only
    /// happened to satisfy.
    #[test]
    fn a_wider_arrow_column_pushes_the_trigger() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        let before = split.primary_rect().width;
        split.arrow_width = dimensions::SPLIT_ARROW_COLUMN_WIDTH + 10;
        let after = split.primary_rect().width;
        assert_eq!(
            before - after,
            10,
            "the trigger yields exactly the room the arrow took ({before} -> {after})"
        );
        assert_eq!(
            after + split.arrow_rect().width,
            split.face_band().width,
            "the two columns still tile the face after the arrow grew"
        );
    }

    /// The arrow column is the box the glyph is painted into.
    ///
    /// The assembled face creates the arrow column from [`ARROW_LABEL`] and then paints that same
    /// glyph, so this is the "one fact, one derivation" check between the two: a column the layout
    /// reported somewhere other than where the glyph is centred would mean the assembly and the
    /// paint were reading different boxes.
    #[test]
    fn the_arrow_glyph_is_centred_in_the_column_the_layout_reported() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        let svg = crate::widget::svg::render_to_svg(&mut split);
        let boxes = text_run_boxes(&svg);
        assert_eq!(boxes.len(), 2, "the face paints a trigger run and an arrow run: {boxes:?}");
        let arrow = split.arrow_rect();
        let (left, top, right, bottom) = boxes[1];
        let glyph_cx = (left + right) as f32 / 2.0;
        let column_cx = arrow.x as f32 + arrow.width as f32 / 2.0;
        assert!(
            (glyph_cx - column_cx).abs() <= 1.5,
            "the arrow must be centred in the column the layout reported: glyph {glyph_cx}, \
             column {column_cx}"
        );
        assert!(
            top >= arrow.y && bottom <= arrow.y + arrow.height as i32,
            "the arrow must stay inside its own column: ink {top}..{bottom}, column {arrow:?}"
        );
    }

    /// The button's face is one compact row, and the popup hangs from it.
    ///
    /// The defect this pins: the face and its two halves were sized from `rect`, so a 240x120
    /// census cell drew a 120 px-tall face, and the drop-down was anchored to the control's
    /// bottom edge — 120 px below the button the user pressed.
    #[test]
    fn the_face_keeps_its_own_height_and_anchors_the_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        split.set_actions(sample_actions());

        let band = split.face_band();
        assert_eq!(
            band.height,
            crate::widget::metrics::dimensions::SPLIT_BUTTON_HEIGHT,
            "the face is a compact row, not the whole rectangle"
        );
        // The trigger and the arrow divide the face rather than the control.
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();
        assert_eq!(primary.height, band.height);
        assert_eq!(arrow.height, band.height);
        assert_eq!(primary.width + arrow.width, band.width);
        // The popup begins at the face's bottom edge.
        let menu = split.menu_rect();
        assert_eq!(menu.y, band.y + band.height as i32);
    }

    /// One ink box per text `<path>` in document order, as `(left, top, right, bottom)`.
    ///
    /// # Why the ink and not the string
    ///
    /// Text leaves the SVG backend as the `font8x8` rectangles the rasteriser fills — one
    /// axis-aligned subpath per set bitmap bit — so neither the label nor the arrow is in the
    /// document in any form, and a test has to locate a run by *where* it is. That is the
    /// stronger check: the old form matched `>Sample</text>` and read the element's `x`, so a
    /// glyph placed off its column's centre with a correct attribute would have passed it.
    ///
    /// One element is one `draw_text`, so this is one box per run. Subpaths are not
    /// deduplicated: a glyph box wider than the 8 bitmap columns maps two columns to one pixel
    /// and emits the same rectangle twice, exactly as the rasteriser fills it twice.
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        let mut boxes = Vec::new();
        for line in svg.lines() {
            let Some(path_at) = line.find("<path ") else { continue };
            let Some(d_at) = line[path_at..].find("d=\"") else { continue };
            let start = path_at + d_at + 3;
            let Some(end) = line[start..].find('"') else { continue };
            let mut bounds: Option<(i32, i32, i32, i32)> = None;
            for subpath in line[start..start + end].split('M').skip(1) {
                let numbers: Vec<i32> = subpath
                    .split(|c: char| !c.is_ascii_digit() && c != '-')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse().ok())
                    .collect();
                if numbers.len() < 4 {
                    continue;
                }
                let (x, y, w, h) = (numbers[0], numbers[1], numbers[2], numbers[3]);
                let bit = (x, y, x + w, y + h);
                bounds = Some(match bounds {
                    None => bit,
                    Some((l, t, r, b)) => (l.min(bit.0), t.min(bit.1), r.max(bit.2), b.max(bit.3)),
                });
            }
            if let Some(union) = bounds {
                boxes.push(union);
            }
        }
        boxes
    }

    /// The trigger's label is centred in the trigger, and the arrow in its own column.
    ///
    /// Both used to be placed by hand: the label at `primary_rect.x + 8` (left-aligned against
    /// a literal, so a 240 px control drew a 50 px word flush to the left of a 218 px face) and
    /// the arrow at `arrow.x + (arrow.width / 2) - 3` (which hard-coded a 6 px-wide 'v', so any
    /// other glyph or font put the arrow off the column's centre).
    ///
    /// The check is on the emitted **ink**, not on a `<text x>` attribute. The two runs are told
    /// apart by the band their ink's centre falls in — the trigger's box or the trailing arrow's
    /// column — rather than by the string they spell, which is no longer in the document at all.
    /// A run's centre is compared to the box centre it must be centred on, within the fitter's
    /// own margin: a `font8x8` bitmap maps 8 columns onto `round(0.6 * size)` pixels, so the lit
    /// columns rarely start on the box's first pixel. Asserting the margin keeps this a statement
    /// about the layout rather than about the glyph table.
    #[test]
    fn the_label_and_the_arrow_are_each_centred_in_their_own_box() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut split = SplitButton::new("Sample", rect);
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();

        let svg = crate::widget::svg::render_to_svg(&mut split);
        let runs = text_run_boxes(&svg);
        // The label's ink is centred in the trigger, which is the same point as the centre of the
        // trigger's *padded* content box: the padding is symmetric.
        let label = runs
            .iter()
            .find(|(l, _, r, _)| {
                let centre = (l + r) / 2;
                centre >= primary.x && centre < primary.x + primary.width as i32
            })
            .copied()
            .unwrap_or_else(|| panic!("the label painted no ink in the trigger: {svg}"));
        let arrow_run = runs
            .iter()
            .find(|(l, _, r, _)| {
                let centre = (l + r) / 2;
                centre >= arrow.x && centre < arrow.x + arrow.width as i32
            })
            .copied()
            .unwrap_or_else(|| panic!("the arrow painted no ink in its column: {svg}"));

        let margin = crate::render::TEXT_FIT_MARGIN as i32;
        let label_centre = (label.0 + label.2) / 2;
        let primary_centre = primary.x + primary.width as i32 / 2;
        assert!(
            (label_centre - primary_centre).abs() <= margin,
            "the label's ink centre {label_centre} must be the trigger's centre {primary_centre}"
        );
        // The old literal origin was the trigger's left padding: a centred label cannot start
        // there, because the padding alone is narrower than half the label's shortfall.
        assert!(
            label.0 > primary.x + dimensions::SPLIT_BUTTON_PADDING_H as i32,
            "the label is centred, not left-aligned at the padding: {label:?}"
        );
        assert!(label.2 > label.0, "the label laid down ink: {label:?}");
        // The arrow's glyph is centred on its column, which is the fact the literal
        // `(arrow.width / 2) - 3` got wrong for any glyph but a 6 px-wide 'v'.
        let arrow_centre = (arrow_run.0 + arrow_run.2) / 2;
        let column_centre = arrow.x + arrow.width as i32 / 2;
        assert!(
            (arrow_centre - column_centre).abs() <= margin,
            "the arrow's ink centre {arrow_centre} must be the column's centre {column_centre}"
        );
        // And the two runs occupy their own halves of the face rather than colliding in it.
        assert!(
            label.2 <= arrow_run.0,
            "the label {label:?} must not reach into the arrow column {arrow_run:?}"
        );
        // Both runs are painted inside the face band, so neither can drift off the control.
        let band = split.face_band();
        for (name, run) in [("label", label), ("arrow", arrow_run)] {
            assert!(
                run.1 >= band.y && run.3 <= band.y + band.height as i32,
                "the {name} ink {run:?} must stay inside the face {band:?}"
            );
        }
    }

    /// The trigger's label box and a menu row's label box share one padding derivation.
    #[test]
    fn the_trigger_and_the_menu_rows_share_their_leading_space() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        split.set_actions(sample_actions());
        let primary = split.primary_rect();
        let trigger_box = split.primary_label_box(primary);
        let row = split.action_rect(0).expect("the fixture has an action");
        let row_box = split.primary_label_box(row);
        assert_eq!(
            trigger_box.x - primary.x,
            row_box.x - row.x,
            "the trigger and its menu rows must inset their labels by the same amount"
        );
        assert_eq!(
            trigger_box.x - primary.x,
            dimensions::SPLIT_BUTTON_PADDING_H as i32,
            "and that amount is the named constant, not a literal"
        );
    }
}
