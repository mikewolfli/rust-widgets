// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The layout-family wrapper controls: `Spacer`, `Center`, `Align`, `SizedBox`, `Padding`,
//! `Expanded`, `Flexible`, `IntrinsicWidth` and `AspectRatio`.
//!
//! # Why a wrapper is a control here, not only a layout manager
//!
//! `src/layout/` already answers "where does this child rectangle go", but it answers it for
//! a **host tree**: every rule needs an `ObjectId` per child and a parent that owns them. A
//! form assembled from JSON, a designer palette or a Rust demo often has no such tree — it
//! has one control per box. These wrappers carry the same intent in a control that owns its
//! content, so the two paths agree on the *rule* rather than on the mechanism:
//!
//! * `Center` and `AspectRatio` delegate to [`crate::layout::CenterLayout`] and
//!   [`crate::layout::AspectRatioLayout`], so their content rectangle is byte-identical to
//!   the layout manager's.
//! * `Expanded`, `Flexible` and `Spacer` are the three answers to "how should my parent
//!   distribute the leftover room", and they publish it through
//!   [`Widget::default_layout_params`] and [`Widget::hints`] — the channels the layout
//!   engines already read. They therefore work *inside* a real layout tree as well.
//! * `Align`, `SizedBox`, `Padding` and `IntrinsicWidth` state a single-child rule that had
//!   no layout manager: nine anchors, a fixed box, four independent insets, and "my width is
//!   my content's width".
//!
//! # The content model
//!
//! A wrapper positions **one** content box: a label and the size it asks for. The size is a
//! property in its own right (`content_width` / `content_height`) so a form authored in JSON
//! can describe its content without a second control, and so the wrapper can report a real
//! [`Widget::size_hint`] instead of guessing.
//!
//! # Painting
//!
//! Every wrapper paints the same three things, through [`paint_wrapper`]: its own box as a
//! hairline frame (so a wrapper whose content is smaller than itself is still visible), the
//! content area filled with a theme-derived tint, and the content label centred in it. The
//! tint is the theme's accent blended into the resolved surface, which is what makes the
//! wrapper follow an appearance switch without a hardcoded colour.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::layout::LayoutParams;
use crate::layout::{AspectRatioLayout, AxisHints, CenterLayout, Hints, Layout};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// ── Shared content model ────────────────────────────────────────────────────

/// The single content a wrapper positions: a label, the size it asks for, and optionally a real
/// control mounted in that box.
///
/// `size` is what the wrapper reports from [`Widget::size_hint`] and what `Center`'s
/// factors are applied to. It is deliberately the *content's* size, not the wrapper's: a
/// wrapper that claimed its own geometry as its hint would give a shrink-wrapping parent
/// nothing to shrink to.
///
/// `child` is the control that fills the content box, if one was mounted. A wrapper previously
/// carried only a caption: it painted a placeholder frame and could not take part in the widget
/// tree, so a fixed box, a padded box or a centred box could not hold a real control. The slot
/// is the id of a mounted child; [`Widget`]'s geometry entry places it in the content rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    label: String,
    size: Size,
    child: Option<ObjectId>,
}

impl Content {
    /// An empty content box with no size and no mounted control.
    pub fn new() -> Self {
        Self { label: String::new(), size: Size::new(0, 0), child: None }
    }

    /// The label drawn in the content area.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The size the content asks for.
    pub fn size(&self) -> Size {
        self.size
    }

    /// The control mounted in the content area, if any.
    pub fn child(&self) -> Option<ObjectId> {
        self.child
    }

    /// Replaces the label. Returns whether this changed anything.
    pub fn set_label(&mut self, label: String) -> bool {
        if self.label == label {
            return false;
        }
        self.label = label;
        true
    }

    /// Replaces the requested size. Returns whether this changed anything.
    pub fn set_size(&mut self, size: Size) -> bool {
        if self.size == size {
            return false;
        }
        self.size = size;
        true
    }

    /// Reads one of the three content properties, or `None` when `name` is not one of them.
    ///
    /// The `None` is what lets each control's `get` fall through to its own properties and
    /// then to [`base_property_get`] with a single `match` chain, instead of restating the
    /// three names in nine files.
    fn get(&self, name: &str) -> Option<CapabilityValue> {
        match name {
            "content" => Some(CapabilityValue::String(self.label.clone())),
            "content_width" => Some(CapabilityValue::UInt(u64::from(self.size.width))),
            "content_height" => Some(CapabilityValue::UInt(u64::from(self.size.height))),
            _ => None,
        }
    }

    /// Writes one of the three content properties. `None` when `name` is not one of them.
    fn set(
        &mut self,
        name: &str,
        value: CapabilityValue,
    ) -> Option<Result<(), CapabilityAccessError>> {
        let result = match name {
            "content" => expect_string(value).map(|label| {
                self.set_label(label);
            }),
            "content_width" => expect_u32(value).map(|width| {
                self.set_size(Size::new(width, self.size.height));
            }),
            "content_height" => expect_u32(value).map(|height| {
                self.set_size(Size::new(self.size.width, height));
            }),
            _ => return None,
        };
        Some(result)
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
}

// The three content property names, restated as literals in each control's
// `property_names_of!` list because that macro takes literals. Kept here so a reader can see
// the set in one place.

// ── Mounting a real control ─────────────────────────────────────────────────

/// Mounts `widget` in `slot`, replacing whatever the slot held.
///
/// The link is written in **both** directions through [`BaseWidget::add_child_linked`]: `add_child`
/// alone leaves the child believing it has no parent, so a tree walk from the root never reaches a
/// wrapper's content. A replacement unlinks and hides the previous control, because a detached
/// control keeps its own visibility flag and a host that renders by id would still paint it in the
/// box it no longer fills (the same rule `TabWidget::set_tab_widget` follows).
///
/// Returns whether the slot changed.
fn mount_content_child(
    base: &mut BaseWidget,
    slot: &mut Option<ObjectId>,
    widget: Option<ObjectId>,
) -> bool {
    if *slot == widget {
        return false;
    }
    if let Some(previous) = slot.take() {
        base.remove_child_linked(previous);
        crate::set_widget_visible(previous, false);
    }
    *slot = widget;
    if let Some(widget_id) = widget {
        base.add_child_linked(widget_id);
        crate::set_widget_visible(widget_id, true);
    }
    true
}

/// Writes `rect` into the control mounted in `slot`, if any.
///
/// This is the placement half of the contract, not a drawing path: the tree painter already draws
/// mounted children, and it draws each one at its own geometry. Writing through the public
/// geometry entry is what keeps that geometry equal to the box the wrapper's rule produced.
fn place_mounted_child(slot: Option<ObjectId>, rect: Rect) {
    if let Some(widget_id) = slot {
        crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
    }
}

/// Reads the `widget` property: an id, or `Null` for "nothing mounted".
///
/// Ids travel as `UInt` (the variant `mdi_area`'s `active_subwindow` reads back), so a document can
/// write one and a designer reads the same representation. The whole 64-bit id range is accepted
/// rather than narrowed to a `u32`, because an id is not a size.
fn expect_object_id(value: CapabilityValue) -> Result<Option<ObjectId>, CapabilityAccessError> {
    match value {
        CapabilityValue::Null => Ok(None),
        CapabilityValue::UInt(raw) => Ok(Some(ObjectId::from(raw))),
        CapabilityValue::Int(raw) if raw >= 0 => Ok(Some(ObjectId::from(raw as u64))),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// The `widget` property's readable form.
fn object_id_value(slot: Option<ObjectId>) -> CapabilityValue {
    match slot {
        Some(id) => CapabilityValue::UInt(id),
        None => CapabilityValue::Null,
    }
}

// ── Painting ────────────────────────────────────────────────────────────────

/// The colours one wrapper resolves from its style and the active theme.
///
/// Read once per draw and copied out, because the theme read holds the manager's lock: a
/// guard held across the drawing calls would deadlock the first time a primitive took it
/// again (the mutex is not re-entrant).
pub(crate) struct WrapperPalette {
    /// The wrapper's own surface — the colour the frame and the tint are derived against.
    pub(crate) surface: Color,
    /// The resolved ink, used for the frame.
    pub(crate) ink: Color,
    /// The accent the content area is tinted with.
    pub(crate) accent: Color,
    /// The label font.
    pub(crate) font: Font,
}

/// Resolves [`WrapperPalette`] for `base`, preferring an explicit style over the theme.
fn wrapper_palette(base: &BaseWidget, role: &str) -> WrapperPalette {
    let style = base.style();
    let themed = crate::style::resolved_theme_style_shared(role);
    let (theme_background, theme_foreground, theme_accent) = {
        let manager = crate::style::theme_manager();
        match manager.current_theme() {
            Some(theme) => (theme.colors.background, theme.colors.foreground, theme.colors.accent),
            None => (Color::WHITE, Color::BLACK, Color::PRIMARY),
        }
    };
    let surface = style
        .background_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.background_color))
        .unwrap_or(theme_background);
    let ink = style
        .text_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
        .unwrap_or(theme_foreground);
    let accent = themed.as_ref().and_then(|resolved| resolved.border_color).unwrap_or(theme_accent);
    let font = style
        .font
        .clone()
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.font.clone()))
        .unwrap_or_else(|| Font::simple("Arial", 12.0));
    WrapperPalette { surface, ink, accent, font }
}

/// Paints a wrapper: its own frame, the content area, and the content label.
///
/// `outer` is the wrapper's own rectangle and `content` the area its rule produced, which is
/// always inside `outer`. The clip is the wrapper's rectangle, so a rule that produces a
/// rectangle larger than the control cannot paint on a neighbour (nothing clips a widget at
/// this layer).
fn paint_wrapper(
    ctx: &mut RenderContext,
    base: &BaseWidget,
    role: &str,
    outer: Rect,
    content: Rect,
    content_box: &Content,
) {
    if outer.width == 0 || outer.height == 0 {
        return;
    }
    let palette = wrapper_palette(base, role);

    // The wrapper's own box: a hairline frame, so a wrapper whose content is smaller than
    // itself (Align, Padding, SizedBox, IntrinsicWidth) shows where the control ends. It is
    // painted as a stroke rather than a fill so it never hides what the host put behind it.
    ctx.draw_rect(outer, palette.ink.blend(&palette.surface, 0.7));

    if content.width == 0 || content.height == 0 {
        return;
    }

    // The content area is tinted with the accent, so the wrapper follows the appearance:
    // `blend` moves the tint towards the surface in light mode and away from it in dark mode
    // by construction, because both terms come from the theme.
    let tint = palette.accent.blend(&palette.surface, 0.82);
    let radius = (content.height.min(content.width) / 8).clamp(2, 6);
    ctx.fill_rounded_rect(content, radius, tint);
    ctx.draw_rounded_rect_stroke(content, radius, palette.accent.with_alpha_f32(0.55), 1);

    if content_box.label().is_empty() || content_box.child().is_some() {
        return;
    }
    // Bounded text: the label is fitted to the content area, so a long one is truncated with
    // an ellipsis instead of running past the control.
    let label_ink = tint.contrast_color();
    ctx.draw_text_line(
        content,
        content_box.label(),
        &palette.font,
        label_ink,
        HorizontalAlignment::Center,
    );
}

// ── Spacer ──────────────────────────────────────────────────────────────────

/// Spacer — an empty area that consumes free space in a linear layout.
///
/// # What it actually does
///
/// The control paints nothing in an application: its whole contract is the two layout
/// channels. [`Widget::hints`] reports `min = 0`, `pref = base_size`, `max = unbounded`, and
/// [`Widget::default_layout_params`] reports `LayoutParams::stretched(flex)`, which is what
/// tells a `BoxLayout` / `Flex` parent to hand it the leftover room in proportion to
/// `flex`. That is exactly what a spacer is to a box layout.
///
/// `paint_hint` defaults to `true` and draws a faint dashed frame: a form under construction
/// needs to see its spacers, and a spacer that was invisible in the designer was the one
/// control nobody could find. A shipping application sets it to `false`.
pub struct Spacer {
    base: BaseWidget,
    flex: u32,
    base_size: u32,
    paint_hint: bool,
    /// A control mounted in the spacer's own box, if any.
    ///
    /// A spacer is normally empty, but a layout that needs "this much room, and a status line in
    /// it" had no way to express the second half: the wrapper family positions one content box, and
    /// a spacer is the box with no rule of its own. The child fills the spacer's whole rectangle.
    content_child: Option<ObjectId>,
}

impl Spacer {
    /// Creates a spacer with `flex = 1`, `base_size = 0` and the design-time hint enabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Spacer, geometry, "Spacer"),
            flex: 1,
            base_size: 0,
            paint_hint: true,
            content_child: None,
        }
    }

    /// The share of the leftover room this spacer claims.
    pub fn flex(&self) -> u32 {
        self.flex
    }

    /// Sets the share of the leftover room. Zero means "do not grow", which is valid: the
    /// spacer then only enforces [`Self::base_size`].
    pub fn set_flex(&mut self, flex: u32) {
        if self.flex == flex {
            return;
        }
        self.flex = flex;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The size a parent uses before distributing the leftover room.
    pub fn base_size(&self) -> u32 {
        self.base_size
    }

    /// Sets the size a parent uses before distributing the leftover room.
    pub fn set_base_size(&mut self, size: u32) {
        if self.base_size == size {
            return;
        }
        self.base_size = size;
        self.base.request_layout();
    }

    /// Whether the design-time dashed frame is painted.
    pub fn paint_hint(&self) -> bool {
        self.paint_hint
    }

    /// Turns the design-time dashed frame on or off.
    pub fn set_paint_hint(&mut self, paint_hint: bool) {
        if self.paint_hint == paint_hint {
            return;
        }
        self.paint_hint = paint_hint;
        self.base.request_redraw();
    }

    /// The control mounted in the spacer's box, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content_child
    }

    /// Mounts `widget` (or clears it, with `None`) in the spacer's box.
    ///
    /// The child is linked in both directions and fills the spacer's own rectangle — a spacer has
    /// no content rule, so its box *is* the content box.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content_child, widget) {
            place_mounted_child(self.content_child, self.geometry());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }
}

impl Widget for Spacer {
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
        Size::new(self.base_size, self.base_size)
    }

    /// A spacer may shrink to nothing and grow without bound; only `base_size` is a wish.
    fn hints(&self) -> Hints {
        Hints {
            width: AxisHints::new(0, self.base_size, u32::MAX),
            height: AxisHints::new(0, self.base_size, u32::MAX),
        }
    }

    /// The one statement that makes a spacer a spacer: absorb the leftover room, weighted.
    fn default_layout_params(&self) -> LayoutParams {
        LayoutParams::stretched(self.flex)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Spacer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "flex" => Ok(CapabilityValue::UInt(u64::from(self.flex))),
            "base_size" => Ok(CapabilityValue::UInt(u64::from(self.base_size))),
            "paint_hint" => Ok(CapabilityValue::Bool(self.paint_hint)),
            "widget" => Ok(object_id_value(self.content_child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "flex" => {
                self.set_flex(expect_u32(value)?);
                Ok(())
            }
            "base_size" => {
                self.set_base_size(expect_u32(value)?);
                Ok(())
            }
            "paint_hint" => {
                self.set_paint_hint(expect_bool(value)?);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["flex", "base_size", "paint_hint", "widget", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Spacer {
    fn draw(&mut self, ctx: &mut RenderContext) {
        // The mounted control fills the spacer's box; the hint frame is drawn under it.
        place_mounted_child(self.content_child, self.geometry());
        if !self.paint_hint {
            // The whole contract in an application: occupy the room, paint nothing.
            return;
        }
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let palette = wrapper_palette(&self.base, "spacer");
        let color = palette.ink.blend(&palette.surface, 0.55);
        paint_dashed_frame(ctx, rect, color);
    }
}

/// Paints a one-pixel dashed frame inside `rect`.
///
/// The dashes are `DASH` long with a `GAP` between them, walking each edge independently so
/// the corners are drawn exactly once. An empty rectangle paints nothing — a zero-extent
/// element is a drawing command with no ink.
pub(crate) fn paint_dashed_frame(ctx: &mut RenderContext, rect: Rect, color: Color) {
    const DASH: u32 = 4;
    const GAP: u32 = 4;
    let step = DASH + GAP;
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let bottom = rect.y.saturating_add(rect.height as i32).saturating_sub(1);
    let mut x = 0;
    while x < rect.width {
        let dash = DASH.min(rect.width - x);
        ctx.fill_rect(Rect::new(rect.x.saturating_add(x as i32), rect.y, dash, 1), color);
        ctx.fill_rect(Rect::new(rect.x.saturating_add(x as i32), bottom, dash, 1), color);
        x = x.saturating_add(step);
    }
    let right = rect.x.saturating_add(rect.width as i32).saturating_sub(1);
    let mut y = 0;
    while y < rect.height {
        let dash = DASH.min(rect.height - y);
        ctx.fill_rect(Rect::new(rect.x, rect.y.saturating_add(y as i32), 1, dash), color);
        ctx.fill_rect(Rect::new(right, rect.y.saturating_add(y as i32), 1, dash), color);
        y = y.saturating_add(step);
    }
}

impl EventHandler for Spacer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── Center ──────────────────────────────────────────────────────────────────

/// Center — centres its content in a box that is a fraction of its own size.
///
/// The content rectangle comes from [`CenterLayout`], the same rule the layout manager
/// applies to a tree child, so a `Center` control and a centred child of a real layout
/// occupy identical pixels. `width_factor` / `height_factor` are that layout's
/// `0.0..=1.0` fractions; a non-finite factor falls back to `1.0` there, and the value read
/// back here is the one the control stored.
pub struct Center {
    base: BaseWidget,
    content: Content,
    width_factor: f32,
    height_factor: f32,
}

impl Center {
    /// Creates a centre with no content, sized 1:1 with its own box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Center, geometry, "Center"),
            content: Content::new(),
            width_factor: 1.0,
            height_factor: 1.0,
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced. `set_widget` requests a layout, because the content box is what
    /// the child now occupies and the wrapper's own hint has changed shape.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The fraction of the control's width the content area occupies.
    pub fn width_factor(&self) -> f32 {
        self.width_factor
    }

    /// Sets the fraction of the control's width the content area occupies.
    pub fn set_width_factor(&mut self, factor: f32) {
        if self.width_factor == factor {
            return;
        }
        self.width_factor = factor;
        self.base.request_redraw();
    }

    /// The fraction of the control's height the content area occupies.
    pub fn height_factor(&self) -> f32 {
        self.height_factor
    }

    /// Sets the fraction of the control's height the content area occupies.
    pub fn set_height_factor(&mut self, factor: f32) {
        if self.height_factor == factor {
            return;
        }
        self.height_factor = factor;
        self.base.request_redraw();
    }

    /// The rectangle the content occupies inside this control.
    pub fn content_rect(&self) -> Rect {
        let mut layout = CenterLayout::with_factors(self.width_factor, self.height_factor);
        let child = self.base.id();
        layout.add_widget(child, 0);
        let mut produced = None;
        layout.update(self.geometry(), &mut |id, rect| {
            if id == child {
                produced = Some(rect);
            }
        });
        produced.unwrap_or_else(|| self.geometry())
    }
}

impl Widget for Center {
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
        self.content.size()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Center {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "width_factor" => Ok(CapabilityValue::Float(f64::from(self.width_factor))),
            "height_factor" => Ok(CapabilityValue::Float(f64::from(self.height_factor))),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                // The content size is what `size_hint()` reports, so this is a layout change.
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "width_factor" => {
                self.set_width_factor(expect_f32(value)?);
                Ok(())
            }
            "height_factor" => {
                self.set_height_factor(expect_f32(value)?);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "width_factor",
            "height_factor",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Center {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "center", outer, content, &self.content);
    }
}

impl EventHandler for Center {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── Align ───────────────────────────────────────────────────────────────────

/// The nine anchors [`Align`] can place its content at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignPosition {
    /// Top-left corner.
    TopLeft,
    /// Top edge, horizontally centred.
    TopCentre,
    /// Top-right corner.
    TopRight,
    /// Left edge, vertically centred.
    CentreLeft,
    /// The exact centre.
    #[default]
    Centre,
    /// Right edge, vertically centred.
    CentreRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, horizontally centred.
    BottomCentre,
    /// Bottom-right corner.
    BottomRight,
}

impl AlignPosition {
    /// Every anchor, in reading order.
    pub const ALL: [Self; 9] = [
        Self::TopLeft,
        Self::TopCentre,
        Self::TopRight,
        Self::CentreLeft,
        Self::Centre,
        Self::CentreRight,
        Self::BottomLeft,
        Self::BottomCentre,
        Self::BottomRight,
    ];

    /// The token this anchor is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCentre => "top_centre",
            Self::TopRight => "top_right",
            Self::CentreLeft => "centre_left",
            Self::Centre => "centre",
            Self::CentreRight => "centre_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCentre => "bottom_centre",
            Self::BottomRight => "bottom_right",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|anchor| anchor.token() == token)
    }

    /// Whether the anchor is on the left, centre or right of the horizontal axis.
    fn horizontal(self) -> Anchor {
        match self {
            Self::TopLeft | Self::CentreLeft | Self::BottomLeft => Anchor::Start,
            Self::TopCentre | Self::Centre | Self::BottomCentre => Anchor::Middle,
            Self::TopRight | Self::CentreRight | Self::BottomRight => Anchor::End,
        }
    }

    /// Whether the anchor is on the top, middle or bottom of the vertical axis.
    fn vertical(self) -> Anchor {
        match self {
            Self::TopLeft | Self::TopCentre | Self::TopRight => Anchor::Start,
            Self::CentreLeft | Self::Centre | Self::CentreRight => Anchor::Middle,
            Self::BottomLeft | Self::BottomCentre | Self::BottomRight => Anchor::End,
        }
    }
}

/// One axis' half of an anchor: where the content sits on that axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    /// Against the start edge.
    Start,
    /// Centred.
    Middle,
    /// Against the end edge.
    End,
}

/// Where a box of `content` extent sits inside `available` extent for one anchor.
fn anchored_offset(available: u32, content: u32, anchor: Anchor) -> i32 {
    let free = available.saturating_sub(content);
    match anchor {
        Anchor::Start => 0,
        Anchor::Middle => (free / 2) as i32,
        Anchor::End => free as i32,
    }
}

/// Align — places its content at one of nine anchors.
///
/// # Why this is not a mode of `Center`
///
/// `Center` scales its content area by a factor and then centres it; that is the
/// `CenterLayout` contract. An aligned child keeps its own size and moves between nine
/// positions — the two rules have different inputs (a factor vs an anchor), so making one a
/// flag of the other would give every `Align` a meaningless `width_factor`.
pub struct Align {
    base: BaseWidget,
    content: Content,
    alignment: AlignPosition,
}

impl Align {
    /// Creates an align control with no content, anchored at the centre.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Align, geometry, "Align"),
            content: Content::new(),
            alignment: AlignPosition::default(),
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced. `set_widget` requests a layout, because the content box is what
    /// the child now occupies and the wrapper's own hint has changed shape.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The anchor the content is placed at.
    pub fn alignment(&self) -> AlignPosition {
        self.alignment
    }

    /// Sets the anchor the content is placed at.
    pub fn set_alignment(&mut self, alignment: AlignPosition) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// The rectangle the content occupies inside this control.
    ///
    /// The content keeps its own size, clamped to the control's box, and is offset by the
    /// anchor. A content larger than the control is pinned to the start of that axis rather
    /// than underflowing to a negative offset.
    pub fn content_rect(&self) -> Rect {
        let outer = self.geometry();
        let size = Size::new(
            self.content.size().width.min(outer.width),
            self.content.size().height.min(outer.height),
        );
        let dx = anchored_offset(outer.width, size.width, self.alignment.horizontal());
        let dy = anchored_offset(outer.height, size.height, self.alignment.vertical());
        Rect::new(outer.x.saturating_add(dx), outer.y.saturating_add(dy), size.width, size.height)
    }
}

impl Widget for Align {
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
        self.content.size()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Align {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "alignment" => Ok(CapabilityValue::String(self.alignment.token().to_string())),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                // The content size is what `size_hint()` reports, so this is a layout change.
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "alignment" => {
                let token = expect_string(value)?;
                let alignment =
                    AlignPosition::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_alignment(alignment);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "alignment",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Align {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "align", outer, content, &self.content);
    }
}

impl EventHandler for Align {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── SizedBox ────────────────────────────────────────────────────────────────

/// SizedBox — a box with a fixed width and height, whose content is centred inside it.
///
/// The published `box_width` / `box_height` are the box the *control* asks for, reported
/// through [`Widget::hints`] as a fixed size, which is what makes a `SizedBox` a hard
/// constraint in a layout that honours hints. They are independent of the control's own
/// geometry: the geometry is where the parent put it, the box is what it demanded.
pub struct SizedBox {
    base: BaseWidget,
    content: Content,
    fixed: Size,
}

impl SizedBox {
    /// Creates an empty box whose requested size is its own geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SizedBox, geometry, "SizedBox"),
            content: Content::new(),
            fixed: Size::new(geometry.width, geometry.height),
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced. `set_widget` requests a layout, because the content box is what
    /// the child now occupies and the wrapper's own hint has changed shape.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The fixed size this control asks its parent for.
    pub fn box_size(&self) -> Size {
        self.fixed
    }

    /// Sets the fixed size this control asks its parent for.
    pub fn set_box_size(&mut self, size: Size) {
        if self.fixed == size {
            return;
        }
        self.fixed = size;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle the content occupies: the box, centred in the control's own geometry.
    pub fn content_rect(&self) -> Rect {
        let outer = self.geometry();
        let width = self.fixed.width.min(outer.width);
        let height = self.fixed.height.min(outer.height);
        Rect::new(
            outer.x.saturating_add(((outer.width - width) / 2) as i32),
            outer.y.saturating_add(((outer.height - height) / 2) as i32),
            width,
            height,
        )
    }
}

impl Widget for SizedBox {
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
        self.fixed
    }

    /// A box is a hard constraint on both axes: the three hint values coincide.
    fn hints(&self) -> Hints {
        Hints::fixed(self.fixed.width, self.fixed.height)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for SizedBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "box_width" => Ok(CapabilityValue::UInt(u64::from(self.fixed.width))),
            "box_height" => Ok(CapabilityValue::UInt(u64::from(self.fixed.height))),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "box_width" => {
                let width = expect_u32(value)?;
                self.set_box_size(Size::new(width, self.fixed.height));
                Ok(())
            }
            "box_height" => {
                let height = expect_u32(value)?;
                self.set_box_size(Size::new(self.fixed.width, height));
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "box_width",
            "box_height",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for SizedBox {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "sized_box", outer, content, &self.content);
    }
}

impl EventHandler for SizedBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── Padding ─────────────────────────────────────────────────────────────────

/// Padding — insets its content by four independent edge paddings.
///
/// Unlike [`Widget::style`]'s `padding` (which a painter may apply to itself), these are the
/// control's *content* insets: they change the rectangle the content occupies, and they are
/// published so a form can set them per instance. Each edge saturates, so padding larger
/// than the control leaves a zero-extent content area rather than underflowing.
pub struct Padding {
    base: BaseWidget,
    content: Content,
    top: u32,
    right: u32,
    bottom: u32,
    left: u32,
}

impl Padding {
    /// Creates a padding wrapper with no content and zero insets.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Padding, geometry, "Padding"),
            content: Content::new(),
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced. `set_widget` requests a layout, because the content box is what
    /// the child now occupies and the wrapper's own hint has changed shape.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The four insets as `(top, right, bottom, left)`.
    pub fn insets(&self) -> (u32, u32, u32, u32) {
        (self.top, self.right, self.bottom, self.left)
    }

    /// Replaces all four insets at once.
    pub fn set_insets(&mut self, top: u32, right: u32, bottom: u32, left: u32) {
        if (self.top, self.right, self.bottom, self.left) == (top, right, bottom, left) {
            return;
        }
        self.top = top;
        self.right = right;
        self.bottom = bottom;
        self.left = left;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle the content occupies: the control's box inset by the four paddings.
    pub fn content_rect(&self) -> Rect {
        let outer = self.geometry();
        Rect::new(
            outer.x.saturating_add(self.left as i32),
            outer.y.saturating_add(self.top as i32),
            outer.width.saturating_sub(self.left.saturating_add(self.right)),
            outer.height.saturating_sub(self.top.saturating_add(self.bottom)),
        )
    }
}

impl Widget for Padding {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The content's own size plus the insets: what this control asks its parent for.
    fn size_hint(&self) -> Size {
        Size::new(
            self.content.size().width.saturating_add(self.left.saturating_add(self.right)),
            self.content.size().height.saturating_add(self.top.saturating_add(self.bottom)),
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Padding {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "padding_top" => Ok(CapabilityValue::UInt(u64::from(self.top))),
            "padding_right" => Ok(CapabilityValue::UInt(u64::from(self.right))),
            "padding_bottom" => Ok(CapabilityValue::UInt(u64::from(self.bottom))),
            "padding_left" => Ok(CapabilityValue::UInt(u64::from(self.left))),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                // `content_width`/`content_height` change what `size_hint()` answers, which is a
                // **layout** change: `request_redraw` only repaints, so the parent's row kept the
                // old allocation and the content was drawn into it. The public setter
                // (`set_content_size`) requests both, and the same rule is applied here.
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        let (top, right, bottom, left) = self.insets();
        match name {
            "padding_top" => {
                self.set_insets(expect_u32(value)?, right, bottom, left);
                Ok(())
            }
            "padding_right" => {
                self.set_insets(top, expect_u32(value)?, bottom, left);
                Ok(())
            }
            "padding_bottom" => {
                self.set_insets(top, right, expect_u32(value)?, left);
                Ok(())
            }
            "padding_left" => {
                self.set_insets(top, right, bottom, expect_u32(value)?);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "padding_top",
            "padding_right",
            "padding_bottom",
            "padding_left",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Padding {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        // The inset bands are this control's own chrome, so they get a hairline frame; the
        // content area is filled by the shared painter. `Padding` is the one wrapper whose
        // outer frame would otherwise be indistinguishable from a plain container.
        paint_wrapper(ctx, &self.base, "padding", outer, content, &self.content);
    }
}

impl EventHandler for Padding {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── Expanded ────────────────────────────────────────────────────────────────

/// Expanded — a child that takes the leftover room along its parent's main axis, weighted.
///
/// # How it reaches the parent
///
/// Not by painting anything special: [`Widget::default_layout_params`] answers
/// `LayoutParams::stretched(flex)`, and that is the record `BoxLayout` and `Flex` read to
/// decide which child absorbs the free space. A control that only *looked* like an expanded
/// box would be the "declaration without a reader" defect this crate has been bitten by, so
/// the rule is published on the channel the engines actually consume, and a test pins it.
pub struct Expanded {
    base: BaseWidget,
    content: Content,
    flex: u32,
}

impl Expanded {
    /// Creates an expanded child with `flex = 1` and no content.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Expanded, geometry, "Expanded"),
            content: Content::new(),
            flex: 1,
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The share of the leftover room this child claims.
    pub fn flex(&self) -> u32 {
        self.flex
    }

    /// Sets the share of the leftover room this child claims.
    pub fn set_flex(&mut self, flex: u32) {
        if self.flex == flex {
            return;
        }
        self.flex = flex;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle the content occupies: this control's whole box.
    pub fn content_rect(&self) -> Rect {
        self.geometry()
    }
}

impl Widget for Expanded {
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
        self.content.size()
    }

    fn hints(&self) -> Hints {
        Hints {
            width: AxisHints::new(0, self.content.size().width, u32::MAX),
            height: AxisHints::new(0, self.content.size().height, u32::MAX),
        }
    }

    fn default_layout_params(&self) -> LayoutParams {
        LayoutParams::stretched(self.flex)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Expanded {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "flex" => Ok(CapabilityValue::UInt(u64::from(self.flex))),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                // The content size is what `size_hint()`/`hints()` report, so this is a layout
                // change; `request_redraw` alone left the parent's row on the old allocation.
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "flex" => {
                self.set_flex(expect_u32(value)?);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "flex",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Expanded {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "expanded", outer, content, &self.content);
    }
}

impl EventHandler for Expanded {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── Flexible ────────────────────────────────────────────────────────────────

/// How a [`Flexible`] child reacts when it is given more room than it asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexFit {
    /// Take the preferred size and leave the surplus to the siblings.
    #[default]
    Loose,
    /// Grow into the surplus, up to the parent's share.
    Tight,
}

impl FlexFit {
    /// The token this fit is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Loose => "loose",
            Self::Tight => "tight",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "loose" => Some(Self::Loose),
            "tight" => Some(Self::Tight),
            _ => None,
        }
    }
}

/// Flexible — a child that may shrink to its floor, and grows only as far as its fit allows.
///
/// # The difference from `Expanded`
///
/// `Expanded` always fills its share (`tight`). `Flexible` defaults to `loose`: it claims a
/// share of the leftover room but may stop at its preferred size, which is what makes a
/// row of flexible labels stay label-sized while a flexible text field grows. Both publish
/// the same `LayoutParams` channel; only `fill` differs.
pub struct Flexible {
    base: BaseWidget,
    content: Content,
    flex: u32,
    fit: FlexFit,
}

impl Flexible {
    /// Creates a flexible child with `flex = 1` and a loose fit.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Flexible, geometry, "Flexible"),
            content: Content::new(),
            flex: 1,
            fit: FlexFit::default(),
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The share of the leftover room this child claims.
    pub fn flex(&self) -> u32 {
        self.flex
    }

    /// Sets the share of the leftover room this child claims.
    pub fn set_flex(&mut self, flex: u32) {
        if self.flex == flex {
            return;
        }
        self.flex = flex;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How the child reacts to surplus room.
    pub fn fit(&self) -> FlexFit {
        self.fit
    }

    /// Sets how the child reacts to surplus room.
    pub fn set_fit(&mut self, fit: FlexFit) {
        if self.fit == fit {
            return;
        }
        self.fit = fit;
        self.base.request_layout();
    }

    /// The rectangle the content occupies: this control's whole box.
    pub fn content_rect(&self) -> Rect {
        self.geometry()
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }
}

impl Widget for Flexible {
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
        self.content.size()
    }

    /// A loose fit has no ceiling; a tight fit may not exceed the size it asked for, so its
    /// hint is a fixed one — the layout then has nothing to grow.
    fn hints(&self) -> Hints {
        let size = self.content.size();
        match self.fit {
            FlexFit::Loose => Hints {
                width: AxisHints::new(0, size.width, u32::MAX),
                height: AxisHints::new(0, size.height, u32::MAX),
            },
            FlexFit::Tight => Hints::fixed(size.width, size.height),
        }
    }

    fn default_layout_params(&self) -> LayoutParams {
        let mut params = LayoutParams::stretched(self.flex);
        // `LayoutParams::stretched` sets `fill`; a loose fit keeps the weight but refuses to
        // absorb the surplus, which is precisely the difference from `Expanded`.
        params.fill = matches!(self.fit, FlexFit::Tight);
        params
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Flexible {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "flex" => Ok(CapabilityValue::UInt(u64::from(self.flex))),
            "fit" => Ok(CapabilityValue::String(self.fit.token().to_string())),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                // The content size is what `size_hint()`/`hints()` report, so this is a layout
                // change; `request_redraw` alone left the parent's row on the old allocation.
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "flex" => {
                self.set_flex(expect_u32(value)?);
                Ok(())
            }
            "fit" => {
                let token = expect_string(value)?;
                let fit = FlexFit::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_fit(fit);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "flex",
            "fit",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Flexible {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "flexible", outer, content, &self.content);
    }
}

impl EventHandler for Flexible {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── IntrinsicWidth ──────────────────────────────────────────────────────────

/// IntrinsicWidth — a box whose width is exactly its content's width.
///
/// The request is reported through [`Widget::hints`]: the width is *fixed* at
/// `content_width`, while the height is only a floor, because a row of intrinsic boxes
/// should stretch vertically with the row. The drawn box is exactly that width (clamped to
/// the control's own geometry), so what the control asks for and what it paints agree.
pub struct IntrinsicWidth {
    base: BaseWidget,
    content: Content,
}

impl IntrinsicWidth {
    /// Creates an intrinsic-width wrapper with no content.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::IntrinsicWidth, geometry, "IntrinsicWidth"),
            content: Content::new(),
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for. The width part is this control's width.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The rectangle the content occupies: the content's width, the control's height.
    pub fn content_rect(&self) -> Rect {
        let outer = self.geometry();
        Rect::new(outer.x, outer.y, self.content.size().width.min(outer.width), outer.height)
    }
}

impl Widget for IntrinsicWidth {
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
        self.content.size()
    }

    fn hints(&self) -> Hints {
        Hints {
            width: AxisHints::fixed(self.content.size().width),
            height: AxisHints::at_least(self.content.size().height),
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for IntrinsicWidth {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for IntrinsicWidth {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "intrinsic_width", outer, content, &self.content);
    }
}

impl EventHandler for IntrinsicWidth {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

// ── AspectRatio ─────────────────────────────────────────────────────────────

/// The ratio a freshly created [`AspectRatio`] starts with: 16:9, the shape a video, a
/// thumbnail or a preview pane is normally authored at.
pub const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;

/// AspectRatio — a box that derives one extent from the other through a fixed ratio.
///
/// The content rectangle comes from [`AspectRatioLayout`], the same rule a tree child of that
/// layout gets, so the control and the manager agree. `respect_parent` selects that layout's
/// two modes: bounded on both axes (fit inside) or derived from the parent's width alone.
pub struct AspectRatio {
    base: BaseWidget,
    content: Content,
    ratio: f32,
    respect_parent: bool,
}

impl AspectRatio {
    /// Creates an aspect-ratio box at 16:9, bounded by its parent.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AspectRatio, geometry, "AspectRatio"),
            content: Content::new(),
            ratio: DEFAULT_ASPECT_RATIO,
            respect_parent: true,
        }
    }

    /// The content this control positions.
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The control mounted as this wrapper's content, if any.
    pub fn widget(&self) -> Option<ObjectId> {
        self.content.child
    }

    /// Mounts `widget` (or clears it, with `None`) as this control's content.
    ///
    /// The control is linked in both directions and fills [`Self::content_rect`], which is the
    /// placement half of the contract: the tree painter draws mounted children, so no second
    /// drawing path is introduced.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if mount_content_child(&mut self.base, &mut self.content.child, widget) {
            place_mounted_child(self.content.child, self.content_rect());
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Replaces the content label.
    pub fn set_content_label(&mut self, label: String) {
        if self.content.set_label(label) {
            self.base.request_redraw();
        }
    }

    /// Replaces the size the content asks for.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content.set_size(size) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The width:height ratio.
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Sets the width:height ratio.
    ///
    /// A non-finite or non-positive ratio has no rectangle to describe, so it is refused
    /// rather than stored: [`AspectRatioLayout`] would panic on it, and a control must not be
    /// able to reach that panic from a property write.
    pub fn set_ratio(&mut self, ratio: f32) -> Result<(), CapabilityAccessError> {
        if !ratio.is_finite() || ratio <= 0.0 {
            return Err(CapabilityAccessError::OutOfRange);
        }
        if self.ratio == ratio {
            return Ok(());
        }
        self.ratio = ratio;
        self.base.request_layout();
        self.base.request_redraw();
        Ok(())
    }

    /// Whether the content is bounded on both axes by the control's box.
    pub fn respect_parent(&self) -> bool {
        self.respect_parent
    }

    /// Sets whether the content is bounded on both axes by the control's box.
    pub fn set_respect_parent(&mut self, respect_parent: bool) {
        if self.respect_parent == respect_parent {
            return;
        }
        self.respect_parent = respect_parent;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle the content occupies inside this control.
    pub fn content_rect(&self) -> Rect {
        let mut layout = AspectRatioLayout::new(self.ratio, self.respect_parent);
        let child = self.base.id();
        layout.add_widget(child, 0);
        let mut produced = None;
        layout.update(self.geometry(), &mut |id, rect| {
            if id == child {
                produced = Some(rect);
            }
        });
        produced.unwrap_or_else(|| self.geometry())
    }

    /// The size this control asks its parent for: the content's height at the configured
    /// ratio, or its width when no height was given.
    fn requested_size(&self) -> Size {
        let content = self.content.size();
        let height = if content.height > 0 {
            content.height
        } else {
            (content.width as f32 / self.ratio).round().max(0.0) as u32
        };
        let width = if content.width > 0 {
            content.width
        } else {
            (height as f32 * self.ratio).round().max(0.0) as u32
        };
        Size::new(width, height)
    }
}

impl Widget for AspectRatio {
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
        self.requested_size()
    }

    /// The child's hint is the rectangle this control derives from its own box, so the parent
    /// sees the *ratio-corrected* size rather than the raw content size.
    fn hints(&self) -> Hints {
        let rect = self.content_rect();
        Hints::fixed(rect.width, rect.height)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for AspectRatio {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        if let Some(value) = self.content.get(name) {
            return Ok(value);
        }
        match name {
            "ratio" => Ok(CapabilityValue::Float(f64::from(self.ratio))),
            "respect_parent" => Ok(CapabilityValue::Bool(self.respect_parent)),
            "widget" => Ok(object_id_value(self.content.child)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        if let Some(result) = self.content.set(name, value.clone()) {
            if result.is_ok() {
                // `requested_size()` derives the ratio-corrected size from the content, so this
                // is a layout change.
                self.base.request_layout();
                self.base.request_redraw();
            }
            return result;
        }
        match name {
            "ratio" => self.set_ratio(expect_f32(value)?),
            "respect_parent" => {
                self.set_respect_parent(expect_bool(value)?);
                Ok(())
            }
            "widget" => {
                self.set_widget(expect_object_id(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "content",
            "content_width",
            "content_height",
            "ratio",
            "respect_parent",
            "widget",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// Formats `ratio` as the smallest whole-number `w:h` pair whose value is within half a percent
/// of the real ratio, falling back to two decimals when no small pair matches.
///
/// The caption under a ratio box must read like the author wrote it — `16:9`, not `1.78` — so the
/// search walks small denominators and stops at the first honest pair. A value outside the domain
/// [`AspectRatio::set_ratio`] enforces (finite, positive) falls back to the decimal form rather
/// than lying with a pair it does not match.
fn ratio_label(ratio: f32) -> String {
    if ratio.is_finite() && ratio > 0.0 {
        for denominator in 1..=32u32 {
            let numerator = (ratio * denominator as f32).round();
            if numerator < 1.0 {
                continue;
            }
            let candidate = numerator / denominator as f32;
            if (candidate - ratio).abs() / ratio < 0.005 {
                return format!("{}:{}", numerator as u32, denominator);
            }
        }
    }
    format!("{ratio:.2}")
}

/// Paints the ratio geometry over the shared wrapper card: the two diagonals that announce "this
/// area holds a shape", corner brackets like a camera viewfinder, and the ratio caption in the
/// content's bottom-left corner.
///
/// Without these the control reads as a plain label — at the default 16:9 the content rectangle
/// fills nearly the whole box, so nothing told "ratio box" from "text label". The guides are
/// geometry, not decoration: the diagonals are the exact shape the content rectangle bounds, so a
/// wrong ratio is visible as a wrong shape.
fn paint_ratio_guides(
    ctx: &mut RenderContext,
    base: &BaseWidget,
    role: &str,
    content: Rect,
    ratio: f32,
) {
    if content.width == 0 || content.height == 0 {
        return;
    }
    let palette = wrapper_palette(base, role);
    // The guides are inset one pixel from the content's own edges: a two-pixel bracket stroke is
    // centred on its line, so a line on the edge itself would spill half a stroke outside the
    // control's box, which the rendering census rightly rejects.
    let content = Rect::new(
        content.x + 1,
        content.y + 1,
        content.width.saturating_sub(2),
        content.height.saturating_sub(2),
    );
    if content.width == 0 || content.height == 0 {
        return;
    }
    let right = content.x + content.width as i32;
    let bottom = content.y + content.height as i32;

    // The two diagonals: the shape the ratio describes, drawn corner to corner. Soft, so the
    // centred label stays readable underneath.
    let guide = palette.accent.with_alpha_f32(0.35);
    ctx.draw_line_aa(Point::new(content.x, content.y), Point::new(right, bottom), guide);
    ctx.draw_line_aa(Point::new(right, content.y), Point::new(content.x, bottom), guide);

    // Corner brackets, like a camera viewfinder: short L strokes in the full accent, so the
    // frame's corners read before the tint does. The inward arm is clamped to half the box, so a
    // squeezed box crosses itself at worst instead of escaping the control.
    let tick = (content.height.min(content.width) / 6).clamp(4, 12);
    let tick = tick.min(content.width / 2).min(content.height / 2).max(1) as i32;
    for (corner_x, corner_y, inward_x, inward_y) in [
        (content.x, content.y, 1, 1),
        (right, content.y, -1, 1),
        (content.x, bottom, 1, -1),
        (right, bottom, -1, -1),
    ] {
        ctx.draw_line_stroke_aa(
            Point::new(corner_x, corner_y),
            Point::new(corner_x + inward_x * tick, corner_y),
            palette.accent,
            2,
        );
        ctx.draw_line_stroke_aa(
            Point::new(corner_x, corner_y),
            Point::new(corner_x, corner_y + inward_y * tick),
            palette.accent,
            2,
        );
    }

    // The caption: the ratio itself, in the content's bottom-left corner, where the centred label
    // cannot collide with it. Secondary ink, so it supports the label rather than competing.
    let advance = palette.font.size().ceil().max(1.0) as u32;
    let line_height = advance + 2;
    let caption_rect = Rect::new(
        content.x + 4,
        content
            .y
            .saturating_add(content.height as i32)
            .saturating_sub(line_height as i32)
            .saturating_sub(4),
        content.width.saturating_sub(8),
        line_height,
    );
    ctx.draw_text_line(
        caption_rect,
        &ratio_label(ratio),
        &palette.font,
        palette.ink.with_alpha_f32(0.8),
        HorizontalAlignment::Left,
    );
}

impl Draw for AspectRatio {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let content = self.content_rect();
        place_mounted_child(self.content.child, content);
        paint_wrapper(ctx, &self.base, "aspect_ratio", outer, content, &self.content);
        // The guides announce the shape the box bounds; a mounted control covers them, so they are
        // only painted for the placeholder spellings.
        if self.content.child.is_none() {
            paint_ratio_guides(ctx, &self.base, "aspect_ratio", content, self.ratio);
        }
    }
}

impl EventHandler for AspectRatio {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// The shared painter a sibling module uses for its own item boxes.
///
/// Kept `pub(crate)` rather than copied: the distribution layouts in `linear_layouts` paint
/// the same tinted box and fitted label per item, and a second painter would be the "two
/// answers to one question" defect (rule #101).
pub(crate) use self::shared::paint_item_box;

mod shared {
    use super::*;

    /// Paints one item box: the tinted rectangle and its fitted label.
    pub(crate) fn paint_item_box(
        ctx: &mut RenderContext,
        rect: Rect,
        label: &str,
        palette: &WrapperPalette,
        radius: u32,
    ) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let tint = palette.accent.blend(&palette.surface, 0.82);
        ctx.fill_rounded_rect(rect, radius, tint);
        ctx.draw_rounded_rect_stroke(rect, radius, palette.accent.with_alpha_f32(0.55), 1);
        if label.is_empty() {
            return;
        }
        let ink = tint.contrast_color();
        ctx.draw_text_line(rect, label, &palette.font, ink, HorizontalAlignment::Center);
    }
}

/// The palette resolution, re-exported for the distribution layouts so they resolve the same
/// colours through the same precedence ladder.
pub(crate) use self::palette_bridge::resolve_palette;

mod palette_bridge {
    use super::*;

    /// Resolves a palette for `role`, preferring an explicit style over the theme.
    pub(crate) fn resolve_palette(base: &BaseWidget, role: &str) -> WrapperPalette {
        wrapper_palette(base, role)
    }
}

#[cfg(test)]
impl Center {
    /// A centre whose content was set through the public setters.
    fn with_content_for_test(geometry: Rect, size: Size) -> Self {
        let mut center = Self::new(geometry);
        center.set_content_size(size);
        center
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn every_wrapper_reports_a_real_size_hint() {
        let mut center = Center::new(Rect::new(0, 0, 200, 200));
        center.set_content_size(Size::new(80, 40));
        assert_eq!(center.size_hint(), Size::new(80, 40));

        let mut align = Align::new(Rect::new(0, 0, 200, 200));
        align.set_content_size(Size::new(30, 30));
        assert_eq!(align.size_hint(), Size::new(30, 30));

        let mut padding = Padding::new(Rect::new(0, 0, 200, 200));
        padding.set_content_size(Size::new(50, 20));
        padding.set_insets(4, 8, 4, 8);
        assert_eq!(
            padding.size_hint(),
            Size::new(66, 28),
            "a padding wrapper asks for its content plus its insets"
        );
    }

    #[test]
    fn spacer_publishes_the_channels_a_layout_reads() {
        let mut spacer = Spacer::new(Rect::new(0, 0, 10, 10));
        spacer.set_flex(3);
        spacer.set_base_size(12);
        assert!(spacer.default_layout_params().fill, "a spacer absorbs leftover room");
        assert_eq!(spacer.default_layout_params().stretch, 3);
        assert_eq!(spacer.hints().width.pref, 12);
        assert_eq!(spacer.hints().width.max, u32::MAX);
    }

    #[test]
    fn expanded_fills_while_flexible_may_stay_loose() {
        let mut expanded = Expanded::new(Rect::new(0, 0, 10, 10));
        expanded.set_flex(2);
        assert!(expanded.default_layout_params().fill);
        assert_eq!(expanded.default_layout_params().stretch, 2);

        let mut flexible = Flexible::new(Rect::new(0, 0, 10, 10));
        assert!(!flexible.default_layout_params().fill, "the default fit is loose");
        flexible.set_fit(FlexFit::Tight);
        assert!(flexible.default_layout_params().fill, "a tight fit fills its share");
    }

    #[test]
    fn center_rect_is_the_factor_box_centred() {
        let mut center =
            Center::with_content_for_test(Rect::new(100, 100, 200, 100), Size::new(40, 20));
        center.set_width_factor(0.5);
        center.set_height_factor(0.5);
        let rect = center.content_rect();
        assert_eq!(rect, Rect::new(150, 125, 100, 50));
    }

    #[test]
    fn align_reaches_all_nine_anchors() {
        let geometry = Rect::new(10, 20, 100, 60);
        let expected = [
            (AlignPosition::TopLeft, 10, 20),
            (AlignPosition::TopCentre, 45, 20),
            (AlignPosition::TopRight, 80, 20),
            (AlignPosition::CentreLeft, 10, 40),
            (AlignPosition::Centre, 45, 40),
            (AlignPosition::CentreRight, 80, 40),
            (AlignPosition::BottomLeft, 10, 60),
            (AlignPosition::BottomCentre, 45, 60),
            (AlignPosition::BottomRight, 80, 60),
        ];
        for (anchor, x, y) in expected {
            let mut align = Align::new(geometry);
            align.set_content_size(Size::new(30, 20));
            align.set_alignment(anchor);
            let rect = align.content_rect();
            assert_eq!((rect.x, rect.y), (x, y), "{anchor:?}");
            assert_eq!((rect.width, rect.height), (30, 20), "{anchor:?} keeps its own size");
        }
    }

    #[test]
    fn align_tokens_round_trip() {
        for anchor in AlignPosition::ALL {
            assert_eq!(AlignPosition::from_token(anchor.token()), Some(anchor));
        }
        assert_eq!(AlignPosition::from_token("middle"), None);
    }

    #[test]
    fn sized_box_pins_its_content_to_the_requested_box() {
        let mut sized = SizedBox::new(Rect::new(0, 0, 200, 200));
        sized.set_box_size(Size::new(100, 50));
        assert_eq!(sized.content_rect(), Rect::new(50, 75, 100, 50));
        assert_eq!(sized.hints().width, AxisHints::fixed(100), "a box is a hard constraint");
        assert_eq!(sized.hints().height, AxisHints::fixed(50));
    }

    #[test]
    fn padding_saturates_rather_than_underflowing() {
        let mut padding = Padding::new(Rect::new(0, 0, 40, 40));
        padding.set_insets(30, 30, 30, 30);
        let rect = padding.content_rect();
        assert_eq!(rect.width, 0);
        assert_eq!(rect.height, 0);
    }

    #[test]
    fn intrinsic_width_pins_the_width_and_keeps_the_height() {
        let mut intrinsic = IntrinsicWidth::new(Rect::new(0, 0, 200, 80));
        intrinsic.set_content_size(Size::new(60, 20));
        assert_eq!(intrinsic.content_rect(), Rect::new(0, 0, 60, 80));
        assert_eq!(intrinsic.hints().width, AxisHints::fixed(60));
        assert_eq!(intrinsic.hints().height.min, 20);
    }

    #[test]
    fn aspect_ratio_refuses_a_ratio_that_describes_no_rectangle() {
        let mut ratio = AspectRatio::new(Rect::new(0, 0, 320, 240));
        assert!(ratio.set_ratio(0.0).is_err());
        assert!(ratio.set_ratio(f32::NAN).is_err());
        assert!(ratio.set_ratio(f32::INFINITY).is_err());
        assert_eq!(ratio.ratio(), DEFAULT_ASPECT_RATIO);
    }

    #[test]
    fn aspect_ratio_derives_the_missing_extent() {
        let mut ratio = AspectRatio::new(Rect::new(0, 0, 320, 240));
        ratio.set_ratio(2.0).expect("2:1 is a valid ratio");
        ratio.set_content_size(Size::new(100, 0));
        assert_eq!(ratio.requested_size(), Size::new(100, 50));
        ratio.set_content_size(Size::new(0, 40));
        assert_eq!(ratio.requested_size(), Size::new(80, 40));
    }

    #[test]
    fn aspect_ratio_content_rect_fits_inside_the_control() {
        let ratio = AspectRatio::new(Rect::new(0, 0, 320, 240));
        let rect = ratio.content_rect();
        assert!(rect.width <= 320 && rect.height <= 240, "{rect:?} must fit inside");
        // 16:9 inside 320x240 is width-bound: 320 x 180.
        assert_eq!(rect, Rect::new(0, 30, 320, 180));
    }

    #[test]
    fn the_ratio_caption_reads_like_the_author_wrote_it() {
        assert_eq!(ratio_label(16.0 / 9.0), "16:9");
        assert_eq!(ratio_label(4.0 / 3.0), "4:3");
        assert_eq!(ratio_label(1.0), "1:1");
        assert_eq!(ratio_label(2.0), "2:1");
        assert_eq!(ratio_label(1.25), "5:4");
        assert_eq!(ratio_label(0.5), "1:2");
        // No small pair hugs the golden ratio until 13/8, which lands within half a percent.
        assert_eq!(ratio_label(1.618_034), "13:8");
        // Out-of-domain values cannot be a ratio; the decimal form at least does not lie.
        assert_eq!(ratio_label(f32::INFINITY), "inf");
        assert_eq!(ratio_label(0.0), "0.00");
        assert_eq!(ratio_label(-2.0), "-2.00");
    }

    #[test]
    fn an_aspect_ratio_paints_its_guides_and_caption() {
        let mut ratio = AspectRatio::new(Rect::new(0, 0, 240, 120));
        ratio.set_content_size(Size::new(80, 40));
        ratio.set_content_label("Sample".to_string());
        let svg = crate::widget::svg::render_to_svg(&mut ratio);
        // Two diagonals plus the four corner brackets' two strokes each.
        let lines = svg.lines().filter(|line| line.contains("<line")).count();
        assert_eq!(lines, 10, "{svg}");
        // The caption is text under the centred label: one ink box per text run, so the label
        // and the caption are the two runs, and the caption's is the lower one.
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(boxes.len(), 2, "{svg}");
        let lowest = boxes.iter().map(|b| b.1).max().expect("caption ink");
        let highest = boxes.iter().map(|b| b.1).min().expect("label ink");
        assert!(lowest > highest, "caption must sit below the label: {boxes:?}");
    }

    #[test]
    fn a_wrapper_property_route_agrees_with_its_own_accessors() {
        let mut align = Align::new(Rect::new(0, 0, 100, 100));
        align
            .set("alignment", CapabilityValue::String("bottom_right".to_string()))
            .expect("writable");
        assert_eq!(align.alignment(), AlignPosition::BottomRight);
        assert_eq!(
            align.get("alignment").expect("readable"),
            CapabilityValue::String("bottom_right".to_string())
        );
        // An unknown token is refused rather than silently kept.
        assert!(align.set("alignment", CapabilityValue::String("nowhere".to_string())).is_err());
    }

    #[test]
    fn a_wrapper_paints_inside_its_own_rect() {
        let rect = Rect::new(0, 0, 120, 80);

        fn assert_paints<W: Draw + Widget>(widget: &mut W, name: &str) {
            let svg = crate::widget::svg::render_to_svg(widget);
            assert!(svg.starts_with("<svg"), "{name} produced no document: {svg}");
            assert!(svg.contains("fill="), "{name} must paint something: {svg}");
        }

        assert_paints(&mut Spacer::new(rect), "spacer");

        let mut center = Center::new(rect);
        center.set_content_size(Size::new(60, 30));
        center.set_content_label("C".to_string());
        assert_paints(&mut center, "center");

        let mut align = Align::new(rect);
        align.set_content_size(Size::new(60, 30));
        align.set_content_label("A".to_string());
        assert_paints(&mut align, "align");

        let mut sized = SizedBox::new(rect);
        sized.set_box_size(Size::new(60, 40));
        sized.set_content_label("S".to_string());
        assert_paints(&mut sized, "sized_box");

        let mut padding = Padding::new(rect);
        padding.set_insets(6, 6, 6, 6);
        padding.set_content_label("P".to_string());
        assert_paints(&mut padding, "padding");

        let mut expanded = Expanded::new(rect);
        expanded.set_content_label("E".to_string());
        assert_paints(&mut expanded, "expanded");

        let mut flexible = Flexible::new(rect);
        flexible.set_content_label("F".to_string());
        assert_paints(&mut flexible, "flexible");

        let mut intrinsic = IntrinsicWidth::new(rect);
        intrinsic.set_content_size(Size::new(70, 20));
        intrinsic.set_content_label("I".to_string());
        assert_paints(&mut intrinsic, "intrinsic_width");

        assert_paints(&mut AspectRatio::new(rect), "aspect_ratio");
    }

    #[test]
    fn a_wrapper_event_delegation_does_not_panic() {
        let mut center = Center::new(Rect::new(0, 0, 100, 100));
        center.handle_event(&Event::MouseMove { pos: Point::new(5, 5) });
        center.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
        center.handle_event(&Event::MouseRelease { pos: Point::new(5, 5), button: 1 });
    }

    #[test]
    fn the_spacer_design_hint_can_be_turned_off() {
        let mut spacer = Spacer::new(Rect::new(0, 0, 20, 20));
        assert!(spacer.paint_hint());
        spacer.set("paint_hint", CapabilityValue::Bool(false)).expect("writable");
        let svg = crate::widget::svg::render_to_svg(&mut spacer);
        assert_eq!(
            svg.matches("<rect").count(),
            1,
            "a shipping spacer paints nothing but the document's own background: {svg}"
        );
    }

    /// A content-size property change requests **layout**, not only a repaint.
    ///
    /// `content_width`/`content_height` are what `size_hint()` and `hints()` report for these
    /// wrappers, so the parent's row has to be re-solved. `request_redraw` alone repainted the
    /// wrapper into the allocation it already had, and nothing else forced a relayout. `SizedBox`
    /// is absent on purpose: its hint is its own fixed box, not the content's size.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_content_size_property_change_requests_layout() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        macro_rules! assert_requests_layout {
            ($widget:expr, $label:literal) => {{
                // Bound to a local: expanding the expression twice would connect the signal on
                // one instance and change the property on another.
                let mut widget = $widget;
                let counter = Arc::new(AtomicUsize::new(0));
                // `ConnectionHandle` is a plain id (copying it does not disconnect), so the
                // binding exists to keep the handle nameable, not to scope the subscription.
                let _guard = widget.base.layout_requested.connect({
                    let counter = Arc::clone(&counter);
                    move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                });
                widget
                    .set("content_width", CapabilityValue::UInt(300))
                    .expect("content_width is a published property");
                assert!(
                    counter.load(Ordering::Relaxed) > 0,
                    concat!($label, " must request layout when its content size changes")
                );
            }};
        }

        assert_requests_layout!(Center::new(Rect::new(0, 0, 200, 200)), "Center");
        assert_requests_layout!(Align::new(Rect::new(0, 0, 200, 200)), "Align");
        assert_requests_layout!(Padding::new(Rect::new(0, 0, 200, 200)), "Padding");
        assert_requests_layout!(Expanded::new(Rect::new(0, 0, 200, 200)), "Expanded");
        assert_requests_layout!(Flexible::new(Rect::new(0, 0, 200, 200)), "Flexible");
        assert_requests_layout!(AspectRatio::new(Rect::new(0, 0, 200, 200)), "AspectRatio");
    }
}
