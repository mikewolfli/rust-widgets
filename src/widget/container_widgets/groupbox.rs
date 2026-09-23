// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Group box widget.
//!
//! # The rule this control embodies
//!
//! BLUE22 §B.8 cites `GroupBox.qml:20`: `topPadding = padding + label_height + spacing`, i.e.
//! the content area begins below the title **plus the gap that separates the two**. This
//! control derived the same quantity as a bare literal `14` in `content_rect` while the title
//! band was separately anchored to the frame's top edge, so "where the title is" and "where the
//! content starts" were two numbers that happened to describe the same layout. Deriving the
//! content's top edge from the title's own box and the named gap is what makes the two
//! move together — a larger font or a taller title band pushes the content down instead of
//! letting it overlap.

use crate::compat::{Rc, RefCell, String, ToString};
use crate::core::{Alignment, Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    alignment_to_str, expect_alignment, expect_bool, expect_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Padding between the title's own box and the band that erases the border behind it.
///
/// Named rather than written as `10`/`20` in two places: the band's left edge and its
/// width were derived from the same number with different signs, which is how a band
/// twice as wide as intended was easy to leave behind.
const TITLE_PADDING: i32 = 10;

/// Height of the title's band, in pixels. The band is what hides the border behind the
/// label; `2` is the frame's own drawn border thickness, so the two cannot mismatch.
const BORDER_WIDTH: u32 = 2;

/// The vertical gap between the title row and the content below it: 6.
///
/// This is the `spacing` of `GroupBox.qml:20`'s `topPadding = padding + label_height +
/// spacing` — the distance between two adjacent *elements* of this control, as distinct from
/// the frame's edge-to-content padding. It used to be folded into a single `top: 14` literal,
/// which is why the title and the content could not be moved independently and why nothing
/// recorded which part of the 14 was air and which was the label.
const TITLE_CONTENT_SPACING: u32 = dimensions::BUTTON_ICON_SPACING;

/// The frame's own edge-to-content padding: 4.
///
/// The `padding` of the same QML formula, kept separate from [`TITLE_CONTENT_SPACING`] for the
/// reason rule 4 gives: padding is edge-to-content, spacing is element-to-element. The old
/// `content_rect` used one `inset = 4` for both the horizontal and the vertical edges and let
/// the title band stand in for the top one, so there was no place for a second value to live.
const FRAME_PADDING: u32 = 4;

/// Group box widget.
pub struct GroupBox {
    base: BaseWidget,
    title: String,
    alignment: Alignment,
    checkable: bool,
    checked: bool,
    /// Emitted when the group box is toggled, with the new checked state.
    /// `checked` starts as `true` even when `checkable` is false; the signal is
    /// only emitted for a real state change.
    pub toggled: Signal1<bool>,
    /// Cached title width computed in draw() via RenderContext::measure_text().
    cached_title_width: Option<u32>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
impl GroupBox {
    /// Creates a group box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GroupBox, geometry, "GroupBox"),
            title: String::new(),
            alignment: Alignment::Left,
            checkable: false,
            checked: true,
            toggled: Signal1::new(),
            cached_title_width: None,
            registry: None,
        }
    }
    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Returns whether group box is checkable.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    /// Sets checkable state.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }
    /// Returns whether group box is checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Sets checked state.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.toggled.emit(checked);
    }
    /// Toggles checked state.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }

    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// The leading inset of the title's own box from the frame's edge: 10.
    ///
    /// Half of the frame's own padding plus the label's air, and deliberately *not*
    /// [`FRAME_PADDING`]: the title is not content, it is chrome that interrupts the border, and
    /// the classic group-box look has it start further in than the content below it.
    const TITLE_INSET: i32 = TITLE_PADDING;

    /// Returns title rectangle.
    /// The rectangle the title occupies, **inside** the frame.
    ///
    /// # Why the title sits on the border rather than above it
    ///
    /// The title was previously centred on the top edge (`y - text_height / 2`),
    /// which put three quarters of every glyph above the widget. The software
    /// rasteriser paints a glyph *downward* from its origin, so the pixels landed on
    /// rows `-8..+8`: over half of the title — `Sample` reads as `Sampl` — fell outside
    /// the widget's own rectangle. The software backend draws into a rectangular buffer,
    /// so those rows were silently clipped and the frame *looked* right; the SVG backend
    /// emits absolute coordinates with no canvas bound, so the same defect showed up as
    /// text above the picture. Neither reading was the truth, and the two disagreed.
    ///
    /// The title is therefore banded **inside** the frame, its centre line on the top
    /// border. That keeps the classic group-box look (the title interrupts the border)
    /// while making every painted pixel fall within the widget, which is the property
    /// both backends then agree on.
    ///
    /// # Why the horizontal inset is a named constant
    ///
    /// The three alignment arms each spelled the inset as a literal `10` — three copies of one
    /// fact, any of which could be edited alone. `Alignment::Top`/`Bottom` are not horizontal
    /// positions at all, so they take the leading inset rather than pretending to place the
    /// title; the previous `=> rect.x + 10` arm is now the documented leading case.
    fn title_rect(&self) -> Rect {
        let rect = self.geometry();
        let text_width = self.cached_title_width.unwrap_or_else(|| {
            // Fallback approximate measurement if draw() hasn't run yet.
            self.title.len() as u32 * 8
        });
        let text_height = self.title_band_height();
        // The title's leading inset is the frame's own — plus the checkbox's column when the
        // box is checkable, because the indicator is drawn *before* the title. Without that
        // term an 18 px indicator on a 10 px inset started at x = -14 and was partly clipped by
        // the frame it belongs to: the box's own width was derived from a sibling, but its
        // **space was not reserved**, which is the same §B.9 failure in the other direction.
        // Qt's `GroupBox.qml` states the relation the same way — the title's padding includes
        // the indicator's width when there is one.
        let leading = Self::TITLE_INSET + self.indicator_reserve() as i32;
        let x = match self.alignment {
            Alignment::Left | Alignment::Top | Alignment::Bottom => rect.x + leading,
            Alignment::Center => rect.x + ((rect.width.saturating_sub(text_width)) / 2) as i32,
            Alignment::Right => rect.x + rect.width as i32 - text_width as i32 - leading,
        };
        // Clamped so a tall glyph cannot start above the frame: the origin is the
        // glyph's top, so `rect.y` is the highest row any title pixel can occupy. The band's
        // width is clamped to the frame for the same reason in the trailing direction — a
        // title wider than the group would paint over its neighbour.
        let width = text_width.min(rect.width.saturating_sub(leading.max(0) as u32));
        let x = x.min(rect.x + rect.width.saturating_sub(width) as i32);
        // The *reserved* inset is what the clamp must respect, so a narrow frame cannot push the
        // title back under the indicator it just made room for.
        let x = x.max(rect.x + leading.min(rect.width as i32));
        Rect::new(x, (rect.y + text_height as i32 / 2).max(rect.y), width, text_height)
    }

    /// The width the title must leave for the indicator, if the box is checkable: 0 otherwise.
    ///
    /// One derivation read by both the reserve above and the indicator's own placement below,
    /// so the space reserved and the space used cannot disagree — which is what made the
    /// indicator hang outside the frame while its width was computed correctly.
    fn indicator_reserve(&self) -> u32 {
        if !self.checkable {
            return 0;
        }
        dimensions::CHECKBOX_BOX + dimensions::INDICATOR_TEXT_SPACING
    }

    /// The height of one row of the control's own chrome: the title band.
    ///
    /// One line of the default font, which is what the title is drawn in and what the content
    /// below it must yield to. Deriving it here (rather than using a `16` literal) is what lets
    /// a larger font push the content down instead of overlapping the title.
    fn title_band_height(&self) -> u32 {
        Font::default().size().max(1.0) as u32
    }

    /// The top edge the frame's content begins at, below the title band and its gap.
    ///
    /// This is BLUE22 §B.8's `topPadding = padding + label_height + spacing`
    /// (`GroupBox.qml:20`). The three terms are separately named because they are three
    /// different facts: the frame's edge-to-content padding, the label's own height, and the
    /// element-to-element gap. Summing them is what makes the content start exactly below the
    /// title, so the pair cannot drift into an overlap.
    fn content_top(&self) -> i32 {
        let rect = self.geometry();
        let offset = FRAME_PADDING + self.title_band_height() + TITLE_CONTENT_SPACING;
        rect.y.saturating_add(offset as i32)
    }

    /// Returns checkbox rectangle if checkable.
    fn checkbox_rect(&self) -> Option<Rect> {
        if !self.checkable {
            return None;
        }
        let title_rect = self.title_rect();
        // The indicator's own size and its gap to the label come from the shared table, so a
        // group box's checkbox matches the one a `CheckBox` draws in the same form rather than
        // being a second, slightly different box.
        let checkbox_size = dimensions::CHECKBOX_BOX.min(title_rect.height) as i32;
        // Placed from the title's *leading edge* — the inset the title reserved for it — so the
        // indicator and the title tile the leading row instead of the indicator hanging off the
        // frame. The reserve and the placement read one number (`indicator_reserve`).
        let frame = self.geometry();
        let x = title_rect.x - dimensions::INDICATOR_TEXT_SPACING as i32 - checkbox_size;
        Some(Rect::new(
            x.max(frame.x),
            title_rect.y + (title_rect.height as i32 - checkbox_size) / 2,
            checkbox_size.max(0) as u32,
            checkbox_size.max(0) as u32,
        ))
    }

    /// The area the group's children are clipped to and forwarded events from.
    ///
    /// Bounded by the title band above, the frame's own padding on the three other edges, and
    /// the same padding below — so it is the frame's content box minus the title, and it can
    /// never describe a negative extent when the control is squeezed.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let top = self.content_top();
        Rect::new(
            rect.x + FRAME_PADDING as i32,
            top,
            rect.width.saturating_sub(FRAME_PADDING * 2),
            (rect.y + rect.height as i32)
                .saturating_sub(top)
                .saturating_sub(FRAME_PADDING as i32)
                .max(0) as u32,
        )
    }
}
// Implement Widget trait
impl Widget for GroupBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // A group box is a panel: it has no content of its own, so its size wish is the chrome
        // floor plus room for one title, and nothing about it is content-driven. The floor is
        // expressed through the same two constants the frame's own geometry is derived from,
        // and the height through `content_top`, so the reported size cannot describe a frame
        // narrower than its title band or shorter than its own padding.
        let padding = crate::style::EdgeOffsets {
            top: FRAME_PADDING + self.title_band_height() + TITLE_CONTENT_SPACING,
            right: FRAME_PADDING,
            bottom: FRAME_PADDING,
            left: FRAME_PADDING,
        };
        let title = self.cached_title_width.unwrap_or_else(|| self.title.len() as u32 * 8);
        let floor = Size::new(
            title + padding.horizontal_total() + (Self::TITLE_INSET as u32) * 2,
            self.content_top() as u32 + padding.bottom,
        );
        ControlMetrics::implicit_size(Size::new(0, 0), padding, floor)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `GroupBox`'s property contract.
///
/// `Panel` is a type alias for this type (`pub type Panel = GroupBox`), so this
/// single impl serves both `WidgetKind::GroupBox` and every `WidgetKind::Panel`
/// value that is actually a group box. The alias is deliberately not given an
/// impl of its own — it would be a second, duplicate impl of the same type.
impl WidgetProperties for GroupBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "alignment" => {
                Ok(CapabilityValue::String(alignment_to_str(self.alignment()).to_string()))
            }
            "checkable" => Ok(CapabilityValue::Bool(self.is_checkable())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_alignment(value)?);
                Ok(())
            }
            "checkable" => {
                self.set_checkable(expect_bool(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "alignment", "checkable", "checked", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `group_box` publishes.
    ///
    /// `toggle` flips the group's checkable latch — the one zero-argument action in
    /// the list. `set_title`, `set_checkable` and `set_checked` assign state, so they
    /// are refused as [`CapabilityAccessError::OutOfRange`] (use the property route)
    /// rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_title" | "set_checkable" | "set_checked" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for GroupBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Handle checkbox toggle
        if self.checkable {
            if let Event::MousePress { pos, button } = event {
                if *button == 1 {
                    if let Some(checkbox_rect) = self.checkbox_rect() {
                        if checkbox_rect.contains(*pos) {
                            self.toggle();
                        }
                    }
                }
            }
        }
        let content = self.content_rect();
        if let Some(ref reg) = self.registry {
            let target = match event {
                Event::MousePress { pos, .. }
                | Event::MouseRelease { pos, .. }
                | Event::MouseMove { pos } => {
                    content.contains(*pos).then(|| self.base.children.first().copied()).flatten()
                }
                _ => self.base.children.first().copied(),
            };
            if let Some(child_id) = target {
                let _ = reg.borrow_mut().forward_event(child_id, event);
            }
        }
    }
}
impl Draw for GroupBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Cache actual title width from render context.
        if !self.title.is_empty() {
            let metrics = context.measure_text(&self.title, &Font::default());
            self.cached_title_width = Some(metrics.width);
        }
        // Draw base widget
        let rect = self.geometry();
        let content = self.content_rect();
        let title_rect = self.title_rect();
        let style = self.style();
        // Draw border
        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(200, 200, 200)));
        // Draw title background to hide the border behind the title. The band is widened
        // by `TITLE_PADDING` on both sides, so it is clipped to the frame: a band that ran
        // past `rect.right()` would paint over the sibling to the right of this group. Its
        // height is the title band's own, so the erasure covers exactly the row the label
        // occupies rather than a fixed `BORDER_WIDTH` that a taller font would step past.
        let title_bg_left = (title_rect.x - TITLE_PADDING).max(rect.x);
        let title_bg_right = (title_rect.x + title_rect.width as i32 + TITLE_PADDING)
            .min(rect.x + rect.width as i32);
        let title_bg_width = (title_bg_right - title_bg_left).max(0) as u32;
        if title_bg_width > 0 {
            context.fill_rect(
                Rect::new(
                    title_bg_left,
                    rect.y,
                    title_bg_width,
                    BORDER_WIDTH.max(title_rect.height),
                ),
                style.background_color.unwrap_or(Color::rgb(255, 255, 255)),
            );
        }
        // Draw checkbox if checkable
        if self.checkable {
            if let Some(checkbox_rect) = self.checkbox_rect() {
                // The box takes the frame's own border colour rather than the near-invisible
                // mid-grey literal it carried (`Color::rgb(100, 100, 100)`). And the tick is
                // painted in the **contrast colour of that box's fill**, not pure black: a
                // pure-black stroke on a dark appearance is the least readable mark in the
                // control, and it is the very part the user toggles. Same rule as
                // `mdiarea.rs`, which draws its active title in `primary.contrast_color()`.
                let box_color = style.border_color.unwrap_or(Color::rgb(100, 100, 100));
                context.draw_rect(checkbox_rect, box_color);
                // Draw checkmark if checked
                if self.checked {
                    // The box above is an outline, so the fill the tick sits on is the
                    // control's own background — the colour the contrast decision must be
                    // made against.
                    let tick_color = style
                        .background_color
                        .unwrap_or_else(|| box_color.contrast_color())
                        .contrast_color();
                    context.draw_line(
                        Point::from_f32(
                            checkbox_rect.x as f32 + 2.0,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 * 0.5,
                        ),
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 * 0.5,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 - 2.0,
                        ),
                        tick_color,
                    );
                    context.draw_line(
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 * 0.5,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 - 2.0,
                        ),
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 - 2.0,
                            checkbox_rect.y as f32 + 2.0,
                        ),
                        tick_color,
                    );
                }
            }
        }
        // Draw title text. The separator is drawn first and the label on top of it, so
        // the label's own glyph pixels are what the eye reads at the join.
        if !self.title.is_empty() {
            let text_color = if self.base.is_enabled() {
                style.text_color.unwrap_or(Color::rgb(0, 0, 0))
            } else {
                Color::rgb(150, 150, 150)
            };
            context.draw_text(
                Point::from_f32(title_rect.x as f32, title_rect.y as f32),
                &self.title,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
        }
        if let Some(ref reg) = self.registry {
            context.push_clip(content.x, content.y, content.width, content.height);
            for child_id in &self.base.children {
                reg.borrow_mut().draw_widget(*child_id, context);
            }
            context.pop_clip();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ObjectId, Rect};

    #[test]
    fn groupbox_creation_defaults() {
        let gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        assert_eq!(gb.geometry(), Rect::new(0, 0, 200, 100));
        assert!(gb.title().is_empty());
        assert!(gb.is_checked());
        assert!(!gb.is_checkable());
    }

    #[test]
    fn groupbox_title_and_toggle() {
        let mut gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        gb.set_title("Options".to_string());
        assert_eq!(gb.title(), "Options");
        gb.set_checkable(true);
        assert!(gb.is_checkable());
        gb.set_checked(false);
        assert!(!gb.is_checked());
        gb.toggle();
        assert!(gb.is_checked());
    }

    // ── Panel / child management tests ─────────────────────────────────
    #[test]
    fn test_panel_add_remove_children() {
        let mut gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        let child1: ObjectId = 100;
        let child2: ObjectId = 200;
        let child3: ObjectId = 300;

        assert!(gb.children().is_empty());

        gb.add_child(child1);
        assert_eq!(gb.children().len(), 1);
        assert_eq!(gb.children()[0], child1);

        gb.add_child(child2);
        assert_eq!(gb.children().len(), 2);

        gb.add_child(child3);
        assert_eq!(gb.children().len(), 3);

        // Remove middle child
        gb.remove_child(child2);
        assert_eq!(gb.children().len(), 2);
        assert_eq!(gb.children()[0], child1);
        assert_eq!(gb.children()[1], child3);

        // Remove remaining children
        gb.remove_child(child1);
        assert_eq!(gb.children().len(), 1);

        gb.remove_child(child3);
        assert!(gb.children().is_empty());
    }

    /// A checkable group's indicator is inside the frame, and the title yields to it.
    ///
    /// The indicator's *width* was already derived (18 from the shared table), but its **space
    /// was not reserved**: the title still started at the 10 px inset, so an 18 px box began at
    /// x = -14 and was partly clipped by the frame it belongs to. Qt's `GroupBox.qml` states the
    /// relation — the title's padding includes the indicator's width when there is one.
    #[test]
    fn a_checkable_groups_indicator_is_inside_the_frame() {
        let frame = Rect::new(0, 0, 200, 100);
        let mut gb = GroupBox::new(frame);
        gb.set_title("Options".to_string());

        // Uncheckable: nothing is reserved and there is no indicator.
        assert!(gb.checkbox_rect().is_none());
        let plain = gb.title_rect();

        gb.set_checkable(true);
        let indicator = gb.checkbox_rect().expect("a checkable group has an indicator");
        let reserved = gb.title_rect();

        assert!(
            indicator.x >= frame.x,
            "the indicator must not start left of the frame: {indicator:?}"
        );
        assert!(
            indicator.x + indicator.width as i32 <= frame.x + frame.width as i32,
            "and it must not run past the frame: {indicator:?}"
        );
        // The indicator and the title tile the leading row: the indicator ends one spacing
        // before the title begins, and the title moved right by exactly the reserve.
        assert_eq!(
            reserved.x,
            indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32,
            "the title begins one gap after the indicator ends"
        );
        assert_eq!(
            reserved.x - plain.x,
            (dimensions::CHECKBOX_BOX + dimensions::INDICATOR_TEXT_SPACING) as i32,
            "the title reserved exactly the indicator column"
        );
    }

    /// The space the title reserves is the space the indicator uses, at any frame width.
    #[test]
    fn the_indicator_reserve_and_placement_are_one_derivation() {
        for width in [80u32, 200, 400] {
            let frame = Rect::new(0, 0, width, 100);
            let mut gb = GroupBox::new(frame);
            gb.set_title("T".to_string());
            gb.set_checkable(true);
            let indicator = gb.checkbox_rect().expect("checkable");
            let title = gb.title_rect();
            assert!(
                indicator.x >= frame.x,
                "width {width}: the indicator left the frame at {}",
                indicator.x
            );
            assert_eq!(
                title.x,
                indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32,
                "width {width}: the reserved column and the used column must agree"
            );
        }
    }

    #[test]
    fn test_panel_empty_has_no_children() {
        let gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        assert!(gb.children().is_empty());
        assert_eq!(gb.children().len(), 0);
    }
}
