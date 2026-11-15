// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `StepStrip` — a numbered step indicator a multi-step page can own (W7-06).
//!
//! `WizardDialog` paints a step indicator as part of its own chrome, where it is coupled to
//! the banner tint and the sidebar variant. A page that is not a wizard — an onboarding flow,
//! a checkout, a setup assistant — has no wizard to borrow the strip from, and drawing one
//! by hand re-derives the circle states, the numbering and the clamping each time. This
//! companion is that strip as a standalone control.
//!
//! It is deliberately **zero-dependency**: the control exposes `step_activated` and leaves
//! the decision of what "go to that step" means to the host, the same split
//! `LayerPanel`/`PlaylistPanel` use. A wizard that wants to drive it binds its own
//! `go_to_step` on that signal.

use crate::compat::String;
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, estimate_line_height, role_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_draw_bridge, impl_widget_property_hooks, property_names_of};

/// The radius of a step's numbered circle, in logical pixels.
const NODE_RADIUS: u32 = 12;
/// Vertical pitch of a sidebar step row, in logical pixels.
const SIDEBAR_ROW_HEIGHT: u32 = 36;

/// One step's title and its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepEntry {
    /// The caption painted beside (or under) the number.
    pub title: String,
    /// Whether the step may be skipped. Painted as a dimmer title.
    pub optional: bool,
    /// Whether the step has been completed.
    pub completed: bool,
}

impl StepEntry {
    /// Creates a pending, required step.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), optional: false, completed: false }
    }

    /// Marks the step completed.
    pub fn completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    /// Marks the step optional.
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }
}

/// One row of the `steps` records property: a step's title and its two flags.
///
/// The list is flat and positional, the same shape [`crate::widget::LayerPanel`] uses for its
/// layers. A step's identity is its title, but two steps may legitimately share a caption, so
/// the schema names `title` as the nominal identity with uniqueness and non-emptiness
/// explicitly **not** claimed (the `PlaylistPanel` precedent).
pub const STEP_ENTRY_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "title",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "optional",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "completed",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
        ],
        id_field: "title",
        ids_unique: false,
        ids_required: false,
    };

/// How the strip lays its steps out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StepStripStyle {
    /// Circles in a row with the titles underneath; the strip a dialog header uses.
    #[default]
    TopStrip,
    /// A vertical list with the titles to the right; the strip a side rail uses.
    Sidebar,
}

impl StepStripStyle {
    /// The published token for this style.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopStrip => "top_strip",
            Self::Sidebar => "sidebar",
        }
    }

    /// Parses a published token, or `None` for an unknown one.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "top_strip" => Some(Self::TopStrip),
            "sidebar" => Some(Self::Sidebar),
            _ => None,
        }
    }
}

/// A numbered step indicator for a multi-step page.
pub struct StepStrip {
    base: BaseWidget,
    steps: Vec<StepEntry>,
    /// The active step index. Always below `steps.len()` after a clamp.
    current: usize,
    style: StepStripStyle,
    /// Whether a press on a step activates it.
    clickable: bool,
    /// An explicit band fill; `None` follows the theme surface.
    band_color: Option<Color>,
    /// The leading inset, in logical pixels.
    inset: u32,
    /// Emitted with the index of a step the user activated.
    pub step_activated: Signal1<usize>,
}

impl StepStrip {
    /// Creates an empty strip with no steps.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Stepper, geometry, "StepStrip"),
            steps: Vec::new(),
            current: 0,
            style: StepStripStyle::TopStrip,
            clickable: false,
            band_color: None,
            inset: 12,
            step_activated: Signal1::new(),
        }
    }

    /// The steps, in order.
    pub fn steps(&self) -> &[StepEntry] {
        &self.steps
    }

    /// Replaces the steps, clamping the active index to the new set.
    ///
    /// The clamp rule is `WizardDialog`'s: an empty strip reports `0`, and a non-empty one
    /// keeps the last index rather than pointing past the end.
    pub fn set_steps(&mut self, steps: Vec<StepEntry>) {
        self.steps = steps;
        self.current = self.clamp_index(self.current);
        self.base.request_redraw();
    }

    /// Number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// The active step index.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Sets the active step, clamped to the current set.
    pub fn set_current(&mut self, current: usize) {
        let current = self.clamp_index(current);
        if self.current == current {
            return;
        }
        self.current = current;
        self.base.request_redraw();
    }

    /// Sets one step's `completed`/`optional` flags.
    ///
    /// Returns `false` for an index outside the strip, so a caller can tell a no-op from a
    /// write that landed.
    pub fn set_step_state(&mut self, index: usize, completed: bool, optional: bool) -> bool {
        let Some(step) = self.steps.get_mut(index) else {
            return false;
        };
        if step.completed == completed && step.optional == optional {
            return true;
        }
        step.completed = completed;
        step.optional = optional;
        self.base.request_redraw();
        true
    }

    /// How the strip is laid out.
    pub fn style(&self) -> StepStripStyle {
        self.style
    }

    /// Sets the layout style.
    pub fn set_style(&mut self, style: StepStripStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// Whether a press on a step activates it.
    pub fn clickable(&self) -> bool {
        self.clickable
    }

    /// Sets whether a press on a step activates it.
    pub fn set_clickable(&mut self, clickable: bool) {
        if self.clickable == clickable {
            return;
        }
        self.clickable = clickable;
        self.base.request_redraw();
    }

    /// The explicit band fill, or `None` while it follows the theme.
    pub fn band_color(&self) -> Option<Color> {
        self.band_color
    }

    /// Sets the band fill; `None` restores the theme surface.
    pub fn set_band_color(&mut self, color: Option<Color>) {
        if self.band_color == color {
            return;
        }
        self.band_color = color;
        self.base.request_redraw();
    }

    /// The leading inset, in logical pixels.
    pub fn inset(&self) -> u32 {
        self.inset
    }

    /// Sets the leading inset.
    pub fn set_inset(&mut self, inset: u32) {
        if self.inset == inset {
            return;
        }
        self.inset = inset;
        self.base.request_redraw();
    }

    /// The step whose circle contains `point`, if any.
    ///
    /// The hit test is the **drawn** circle (radius plus a small pointer margin), so the
    /// clickable area and the painted marker are the same object.
    pub fn step_at(&self, point: Point) -> Option<usize> {
        let rect = self.geometry();
        for index in 0..self.steps.len() {
            let center = self.node_center(rect, index, self.steps.len());
            let reach = NODE_RADIUS as i32 + 4;
            if (point.x - center.x).abs() <= reach && (point.y - center.y).abs() <= reach {
                return Some(index);
            }
        }
        None
    }

    fn clamp_index(&self, index: usize) -> usize {
        if self.steps.is_empty() {
            0
        } else {
            index.min(self.steps.len() - 1)
        }
    }

    /// The centre of one step's circle, derived from the same geometry the draw uses.
    fn node_center(&self, rect: Rect, index: usize, count: usize) -> Point {
        match self.style {
            StepStripStyle::TopStrip => {
                let count = count.max(1) as u32;
                let cell = rect.width.saturating_sub(self.inset.saturating_mul(2)) / count;
                Point::new(
                    rect.x + self.inset as i32 + (cell * index as u32 + cell / 2) as i32,
                    rect.y + NODE_RADIUS as i32 + 4,
                )
            }
            StepStripStyle::Sidebar => Point::new(
                rect.x + self.inset as i32 + NODE_RADIUS as i32,
                rect.y
                    + self.inset as i32
                    + (SIDEBAR_ROW_HEIGHT * index as u32) as i32
                    + SIDEBAR_ROW_HEIGHT as i32 / 2,
            ),
        }
    }

    fn activate(&mut self, index: usize) {
        if !self.clickable {
            return;
        }
        self.set_current(index);
        self.step_activated.emit(self.current);
    }
}

impl EventHandler for StepStrip {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let Some(index) = self.step_at(*pos) {
                    self.activate(index);
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                if let Some(index) = self.step_at(*pos) {
                    self.activate(index);
                }
            }
            _ => {}
        }
    }
}

impl Draw for StepStrip {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let widget_style = self.base.style();
        let band = self.band_color.unwrap_or_else(|| {
            widget_style
                .background_color
                .or_else(|| crate::style::palette_shared().map(|colors| colors.background))
                .unwrap_or(Color::WHITE)
        });
        let ink = widget_style
            .text_color
            .or_else(|| crate::style::palette_shared().map(|colors| colors.foreground))
            .unwrap_or(Color::BLACK);
        let accent =
            crate::style::palette_shared().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let completed_ink = accent.contrast_color();
        let surface = widget_style.background_color.unwrap_or(band);
        let pending = surface.blend(&ink, 0.15);
        let connector = ink.with_alpha_f32(0.25);
        let number_font = role_font(widget_style, 11.0).with_bold(true);
        let title_font = role_font(widget_style, 9.0);

        context.fill_rect(rect, band);
        if self.steps.is_empty() {
            self.base.paint_disabled_veil(context, rect);
            return;
        }

        let count = self.steps.len();
        // Connectors first, so the circles sit on top of the line ends.
        for index in 0..count.saturating_sub(1) {
            let from = self.node_center(rect, index, count);
            let to = self.node_center(rect, index + 1, count);
            context.draw_line_stroke(from, to, connector, 2);
        }

        for index in 0..count {
            let step = &self.steps[index];
            let center = self.node_center(rect, index, count);
            let active = index == self.current;
            let (fill, number_ink) = if !self.base.is_enabled() {
                (pending, ink.blend(&band, 0.35))
            } else if step.completed {
                (accent, completed_ink)
            } else if active {
                (accent, completed_ink)
            } else {
                (pending, ink)
            };
            context.fill_circle_aa(center, NODE_RADIUS, fill);
            if !step.completed && !active {
                context.draw_circle_stroke(center, NODE_RADIUS, ink.with_alpha_f32(0.4), 1);
            }

            let number = crate::compat::format!("{}", index + 1);
            let metrics = context.measure_text(&number, &number_font);
            context.draw_text_fitted_ellipsis(
                Rect::new(
                    center.x - NODE_RADIUS as i32,
                    center.y - metrics.height as i32 / 2,
                    NODE_RADIUS * 2,
                    metrics.height.max(1),
                ),
                &number,
                &number_font,
                number_ink,
                HorizontalAlignment::Center,
            );

            let title_ink = if !self.base.is_enabled() {
                ink.blend(&band, 0.35)
            } else if active || step.completed {
                ink
            } else if step.optional {
                ink.blend(&band, 0.45)
            } else {
                ink.blend(&band, 0.3)
            };
            // The title box is the step's own cell, so a long caption truncates inside its
            // column instead of painting over its neighbour.
            let title_box = match self.style {
                StepStripStyle::TopStrip => {
                    let count = count.max(1) as u32;
                    let cell = rect.width.saturating_sub(self.inset.saturating_mul(2)) / count;
                    Rect::new(
                        center.x - cell as i32 / 2,
                        center.y + NODE_RADIUS as i32 + 4,
                        cell,
                        rect.height.saturating_sub(NODE_RADIUS * 2 + 8),
                    )
                }
                StepStripStyle::Sidebar => Rect::new(
                    center.x + NODE_RADIUS as i32 + 8,
                    center.y - SIDEBAR_ROW_HEIGHT as i32 / 2,
                    rect.width.saturating_sub(self.inset + NODE_RADIUS * 2 + 16),
                    SIDEBAR_ROW_HEIGHT,
                ),
            };
            let line = context.text_line(title_box, &title_font);
            context.draw_text_fitted_ellipsis(
                Rect::new(title_box.x, line.y, title_box.width, line.height.max(1)),
                &step.title,
                &title_font,
                title_ink,
                HorizontalAlignment::Left,
            );
        }

        self.base.paint_disabled_veil(context, rect);
    }
}

impl Widget for StepStrip {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    /// The strip's natural size, derived from the shared metric system (`W7-06`).
    ///
    /// The horizontal style is one circle row plus the title row under it; the sidebar style is
    /// three rows of `SIDEBAR_ROW_HEIGHT`. Both go through `ControlMetrics::implicit_size` so the
    /// padding/floor arithmetic is the same one every other control uses, and the title row is
    /// measured with `estimate_line_height` rather than assumed.
    fn size_hint(&self) -> Size {
        let line_height = estimate_line_height(effective_font(self.base.style()), 1.0);
        let circle = NODE_RADIUS.saturating_mul(2);
        match self.style {
            StepStripStyle::TopStrip => ControlMetrics::implicit_size(
                // Three circles is the width a step strip is worth before a layout should
                // stretch it; the titles live in the same cells, so they do not widen it.
                Size::new(
                    circle.saturating_mul(3),
                    circle.saturating_add(4).saturating_add(line_height),
                ),
                EdgeOffsets::symmetric(6, self.inset),
                Size::new(circle.saturating_mul(2), line_height.saturating_mul(2)),
            ),
            StepStripStyle::Sidebar => ControlMetrics::implicit_size(
                Size::new(
                    circle.saturating_add(8).saturating_add(circle.saturating_mul(4)),
                    SIDEBAR_ROW_HEIGHT.saturating_mul(3),
                ),
                EdgeOffsets::symmetric(0, self.inset),
                Size::new(circle.saturating_mul(2), SIDEBAR_ROW_HEIGHT),
            ),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Publishes [`Self::step_activated`] as this control's one capability event.
    ///
    /// The payload is the step's index, the same number `set_current` takes, so a host can
    /// forward it to its own `go_to_step` without translating anything.
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "step_activated" => {
                Some(EventSignalRef::mapped("step_activated", &self.step_activated, |index| {
                    CapabilityValue::UInt(*index as u64)
                }))
            }
            _ => None,
        }
    }
}

/// One row of [`STEP_ENTRY_SCHEMA`].
fn step_entry_record(step: &StepEntry) -> crate::widget::capability::RecordValue {
    crate::widget::capability::RecordValue::new()
        .with("title", CapabilityValue::String(step.title.clone()))
        .with("optional", CapabilityValue::Bool(step.optional))
        .with("completed", CapabilityValue::Bool(step.completed))
}

/// Rebuilds one step row from a row of [`STEP_ENTRY_SCHEMA`].
fn step_entry_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<StepEntry, CapabilityAccessError> {
    let title = match record.get("title") {
        Some(CapabilityValue::String(title)) => title.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    // An omitted flag is the same default `StepEntry::new` gives a row.
    let flag = |name: &str| match record.get(name) {
        None | Some(CapabilityValue::Null) => Ok(false),
        Some(CapabilityValue::Bool(value)) => Ok(*value),
        Some(_) => Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(StepEntry { title, optional: flag("optional")?, completed: flag("completed")? })
}

impl WidgetProperties for StepStrip {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "steps" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &STEP_ENTRY_SCHEMA,
                    self.steps.iter().map(step_entry_record).collect(),
                )))
            }
            "current" => Ok(CapabilityValue::UInt(self.current as u64)),
            "step_count" => Ok(CapabilityValue::UInt(self.steps.len() as u64)),
            "style" => Ok(CapabilityValue::String(self.style.as_str().to_string())),
            "clickable" => Ok(CapabilityValue::Bool(self.clickable)),
            "band_color" => Ok(match self.band_color {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            "inset" => Ok(CapabilityValue::UInt(self.inset as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "steps" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&STEP_ENTRY_SCHEMA).map_err(|violation| {
                    log::warn!("step_strip.steps refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("steps"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let steps = list
                    .items()
                    .iter()
                    .map(step_entry_from_record)
                    .collect::<Result<Vec<_>, CapabilityAccessError>>()?;
                self.set_steps(steps);
                Ok(())
            }
            "current" => {
                let CapabilityValue::UInt(current) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_current(current as usize);
                Ok(())
            }
            "style" => {
                let CapabilityValue::String(token) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let style =
                    StepStripStyle::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_style(style);
                Ok(())
            }
            "clickable" => {
                let CapabilityValue::Bool(clickable) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_clickable(clickable);
                Ok(())
            }
            "band_color" => {
                let color = match value {
                    CapabilityValue::Color(color) => Some(color),
                    CapabilityValue::Null => None,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_band_color(color);
                Ok(())
            }
            "inset" => {
                let CapabilityValue::UInt(inset) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_inset(inset.min(u64::from(u32::MAX)) as u32);
                Ok(())
            }
            "step_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "steps",
            "current",
            "step_count",
            "style",
            "clickable",
            "band_color",
            "inset",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn strip() -> StepStrip {
        let mut strip = StepStrip::new(Rect::new(0, 0, 240, 56));
        strip.set_steps(Vec::from([
            StepEntry::new("Account").completed(true),
            StepEntry::new("Details"),
            StepEntry::new("Review").optional(true),
        ]));
        strip
    }

    #[test]
    fn the_active_index_is_clamped_to_the_step_set() {
        let mut strip = StepStrip::new(Rect::new(0, 0, 240, 56));
        strip.set_steps(Vec::from([StepEntry::new("One"), StepEntry::new("Two")]));
        strip.set_current(9);
        assert_eq!(strip.current(), 1);
        strip.set_steps(Vec::new());
        assert_eq!(strip.current(), 0);
    }

    #[test]
    fn a_click_activates_the_pressed_step_only_when_clickable() {
        let mut strip = strip();
        let center = strip.node_center(strip.geometry(), 1, strip.step_count());
        let seen = crate::compat::Arc::new(core::sync::atomic::AtomicUsize::new(0));
        strip.step_activated.connect({
            let seen = crate::compat::Arc::clone(&seen);
            move |index| {
                seen.store(*index, core::sync::atomic::Ordering::SeqCst);
            }
        });
        strip.handle_event(&Event::MousePress { pos: center, button: 1, modifiers: 0 });
        assert_eq!(strip.current(), 0, "a non-clickable strip does not navigate");

        strip.set_clickable(true);
        strip.handle_event(&Event::MousePress { pos: center, button: 1, modifiers: 0 });
        assert_eq!(strip.current(), 1);
        assert_eq!(seen.load(core::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn step_state_is_written_by_index() {
        let mut strip = strip();
        assert!(strip.set_step_state(1, true, true));
        assert!(strip.steps()[1].completed);
        assert!(strip.steps()[1].optional);
        assert!(!strip.set_step_state(9, true, true), "an out-of-range index is refused");
    }

    #[test]
    fn both_styles_paint_without_panicking() {
        for style in [StepStripStyle::TopStrip, StepStripStyle::Sidebar] {
            let mut strip = strip();
            strip.set_style(style);
            let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
            backend.begin_frame(Color::WHITE);
            let mut context = RenderContext::new(&mut backend);
            strip.draw(&mut context);
            backend.end_frame();
            assert!(!backend.frame_rgba().is_empty());
        }
    }

    /// The `steps` records property round-trips through `set`, and it is the same list the paint
    /// reads: a designer that writes it gets the clamped active index, not a second model.
    #[test]
    fn the_steps_property_round_trips_through_the_capability_layer() {
        let mut strip = StepStrip::new(Rect::new(0, 0, 240, 56));
        let rows = crate::widget::capability::RecordList::from_items(
            &STEP_ENTRY_SCHEMA,
            Vec::from([
                step_entry_record(&StepEntry::new("Account").completed(true)),
                step_entry_record(&StepEntry::new("Review").optional(true)),
            ]),
        );
        strip.set("steps", CapabilityValue::Records(rows)).expect("the list validates");
        assert_eq!(strip.step_count(), 2);
        assert_eq!(strip.steps()[0].title, "Account");
        assert!(strip.steps()[0].completed && !strip.steps()[0].optional);
        assert!(strip.steps()[1].optional && !strip.steps()[1].completed);

        let read = strip.get("steps").expect("readable");
        let CapabilityValue::Records(read) = read else {
            panic!("steps reads back as records");
        };
        assert_eq!(read.items().len(), 2);
        assert_eq!(strip.get("step_count"), Ok(CapabilityValue::UInt(2)));
        assert_eq!(
            strip.set("step_count", CapabilityValue::UInt(5)),
            Err(CapabilityAccessError::ReadOnlyProperty),
            "the count is derived from the list, never written directly"
        );
    }

    /// `band_color` is declared writable, so writing `Null` must clear it rather than fail.
    #[test]
    fn the_band_colour_property_clears_with_null() {
        let mut strip = strip();
        let tint = Color::rgba(20, 40, 60, 255);
        strip.set("band_color", CapabilityValue::Color(tint)).expect("writable");
        assert_eq!(strip.get("band_color"), Ok(CapabilityValue::Color(tint)));
        strip.set("band_color", CapabilityValue::Null).expect("null clears");
        assert_eq!(strip.get("band_color"), Ok(CapabilityValue::Null));
    }
}
