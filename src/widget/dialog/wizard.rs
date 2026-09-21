// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WizardDialog widget — a step-by-step wizard control with back/next/finish navigation.
//!
//! Displays a step indicator at the top (numbered circles), a content area with the
//! current step title, and navigation buttons (Back, Next/Finish, Cancel) at the bottom.
//! Emits `finished`, `cancelled`, and `step_changed` signals.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single step in the wizard.
#[derive(Debug, Clone)]
pub struct WizardStep {
    /// Display title shown in the step indicator and content area.
    pub title: String,
    /// Whether this step has been completed.
    pub completed: bool,
    /// Whether this step can be skipped.
    pub optional: bool,
}

impl WizardStep {
    /// Creates a new wizard step.
    pub fn new(title: impl Into<String>, optional: bool) -> Self {
        Self { title: title.into(), completed: false, optional }
    }
}

/// WizardDialog widget — a multi-step wizard with navigation controls.
pub struct WizardDialog {
    base: BaseWidget,
    steps: Vec<WizardStep>,
    current_step: usize,
    /// Title shown in the wizard's own chrome.
    ///
    /// Kept on the control rather than in a host-side map: the title is part of
    /// what the dialog *is*, and a host that held it separately could not paint it.
    title: String,
    /// Emitted when the user clicks Finish on the last step.
    pub finished: GenericSignal,
    /// Emitted when the user clicks Cancel.
    pub cancelled: GenericSignal,
    /// Emitted when the current step index changes.
    pub step_changed: Signal1<usize>,
}

impl WizardDialog {
    /// Creates a new WizardDialog with geometry and no title.
    pub fn new(geometry: Rect) -> Self {
        Self::with_title(String::new(), geometry)
    }

    /// Creates a new WizardDialog with a title and geometry.
    pub fn with_title(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WizardDialog, geometry, "WizardDialog"),
            steps: Vec::new(),
            current_step: 0,
            title,
            finished: GenericSignal::new(),
            cancelled: GenericSignal::new(),
            step_changed: Signal1::new(),
        }
    }

    /// Returns the wizard's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the wizard's title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }

    /// Adds a step to the wizard.
    pub fn add_step(&mut self, title: impl Into<String>, optional: bool) {
        self.steps.push(WizardStep::new(title, optional));
        self.base.request_redraw();
    }

    /// Advances to the next step if not already on the last step.
    /// Returns `true` if the step actually changed.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        if self.current_step < self.steps.len().saturating_sub(1) {
            // Mark current step as completed
            if let Some(step) = self.steps.get_mut(self.current_step) {
                step.completed = true;
            }
            self.current_step += 1;
            self.step_changed.emit(self.current_step);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Goes back to the previous step if not already on the first step.
    /// Returns `true` if the step actually changed.
    pub fn previous(&mut self) -> bool {
        if self.current_step > 0 {
            self.current_step -= 1;
            self.step_changed.emit(self.current_step);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Jumps to a specific step index. Clamps to valid range.
    /// Returns `true` if the step actually changed.
    pub fn go_to_step(&mut self, index: usize) -> bool {
        let clamped = index.min(self.steps.len().saturating_sub(1));
        if self.current_step != clamped {
            self.current_step = clamped;
            self.step_changed.emit(self.current_step);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Returns the current step index.
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// Returns `true` if the current step is the first step.
    pub fn is_first(&self) -> bool {
        self.current_step == 0
    }

    /// Returns `true` if the current step is the last step.
    pub fn is_last(&self) -> bool {
        self.steps.is_empty() || self.current_step >= self.steps.len().saturating_sub(1)
    }

    /// Returns the total number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns a reference to the steps.
    pub fn steps(&self) -> &[WizardStep] {
        &self.steps
    }

    /// Returns a mutable reference to the steps.
    pub fn steps_mut(&mut self) -> &mut Vec<WizardStep> {
        &mut self.steps
    }

    /// Returns the title of the current step, or an empty string if there are no steps.
    pub fn current_step_title(&self) -> &str {
        self.steps.get(self.current_step).map(|s| s.title.as_str()).unwrap_or("")
    }

    /// Resets the wizard to the first step and marks all steps as incomplete.
    pub fn reset(&mut self) {
        self.current_step = 0;
        for step in &mut self.steps {
            step.completed = false;
        }
        self.step_changed.emit(0);
        self.base.request_redraw();
    }

    /// Called when the Finish button is clicked.
    fn do_finish(&mut self) {
        // Mark all steps as completed
        for step in &mut self.steps {
            step.completed = true;
        }
        self.finished.emit();
        self.base.request_redraw();
    }

    /// Called when the Cancel button is clicked.
    fn do_cancel(&mut self) {
        self.cancelled.emit();
    }
}

impl Widget for WizardDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(500, 400)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `WizardDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `WizardDialog`'s property contract.
///
/// The wizard exposes its title and its step bookkeeping. Step *content* is
/// managed through `add_step` rather than the property layer, because a step is a
/// structured object and flattening it into a scalar would lose information.
impl WidgetProperties for WizardDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "step_count" => Ok(CapabilityValue::UInt(self.step_count() as u64)),
            "current_step" => Ok(CapabilityValue::UInt(self.current_step() as u64)),
            // `can_go_back` / `can_go_forward` are the schema's names for the same
            // facts `is_first` / `is_last` express; publishing the negations keeps
            // the schema's wording while reading the real state.
            "can_go_back" => Ok(CapabilityValue::Bool(!self.is_first())),
            "can_go_forward" => Ok(CapabilityValue::Bool(!self.is_last())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            // Moving between steps is navigation, not an assignment: it is driven
            // by `next` / `previous`, which emit `step_changed`. The `can_go_*`
            // pair is derived from the current position.
            "step_count" | "current_step" | "can_go_back" | "can_go_forward" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "step_count",
            "current_step",
            "can_go_back",
            "can_go_forward",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `wizard_dialog` publishes.
    ///
    /// `next` / `back` are payload-free navigation and map onto the widget's real
    /// `next` / `previous`. `set_current_step` needs an index and is answered
    /// through `go_to_step`, and the wizard has no terminal "finish" action (its
    /// last step is simply reached), so both report `OutOfRange` for a
    /// payload-less call rather than pretending to complete something.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "next" => {
                if self.next() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "back" => {
                if self.previous() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "set_current_step" | "finish" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for WizardDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so
        // a light/dark switch left the frame, the step circles, the buttons and every text
        // run unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("wizard_dialog");
        // `wizard_dialog` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A frame painted in that colour would be byte-identical to the window
        // behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, the same distinction `Colors::input_background` draws for a
        // field.
        let window_fill = {
            let manager = crate::theme::global_theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.45));
        // The content well and the secondary buttons are one visible step apart from the
        // frame in either appearance.
        let content_fill = surface.blend(&ink, 0.04);
        let button_fill = surface.blend(&ink, 0.12);
        let button_ink = button_fill.contrast_color();
        // The accent drives the wizard's progress affordances; the button fill is the
        // fallback when the theme declines to resolve one.
        let primary = theme.as_ref().and_then(|t| t.background_color).unwrap_or(button_fill);
        let primary_ink = primary.contrast_color();
        // "Completed" is a *state*, so its green comes from the theme's semantic token
        // instead of the literal it used to carry. `semantic_color` takes and releases the
        // lock and returns an owned colour, so no guard outlives the call.
        let complete = crate::theme::semantic_color(crate::theme::SemanticColor::Success)
            .unwrap_or(Color::rgb(52, 199, 89));

        // Background
        context.fill_rect(rect, surface);

        let step_indicator_height = 50u32;
        let nav_button_height = 36u32;
        let nav_area_height = nav_button_height + 12; // 12 padding
        let content_y = rect.y + step_indicator_height as i32;
        let content_height = rect.height.saturating_sub(step_indicator_height + nav_area_height);

        // ── Step Indicator ──────────────────────────────────────────────
        if !self.steps.is_empty() {
            let total_steps = self.steps.len();
            let circle_radius = 12u32;
            let spacing = (rect.width / total_steps as u32).min(120u32);
            let total_width = total_steps as u32 * spacing;
            let start_x = rect.x + (rect.width.saturating_sub(total_width) / 2) as i32;

            for i in 0..total_steps {
                let cx = start_x + (i as u32 * spacing + spacing / 2) as i32;
                let cy = rect.y + step_indicator_height as i32 / 2;

                // Determine circle state
                let is_active = i == self.current_step;
                let is_completed = self.steps[i].completed;

                let (circle_color, text_color) = if !is_enabled {
                    (ink.blend(&surface, 0.25), ink.blend(&surface, 0.35))
                } else if is_active {
                    (primary, primary_ink)
                } else if is_completed {
                    (complete, complete.contrast_color())
                } else {
                    (surface.blend(&ink, 0.15), ink)
                };

                // Draw circle
                context.fill_circle_aa(Point::new(cx, cy), circle_radius, circle_color);

                // Step number inside circle
                let label = format!("{}", i + 1);
                let label_font = Font::bold("Arial", 11.0);
                let label_x = cx - 4;
                let label_y = cy - 6;
                context.draw_text(
                    Point::new(label_x, label_y),
                    &label,
                    &label_font,
                    text_color,
                    HorizontalAlignment::Left,
                );

                // Step title below circle
                let title_font = if is_active {
                    Font::bold("Arial", 9.0)
                } else {
                    Font::new("Arial", 9.0, false, false)
                };
                let title_color = if !is_enabled {
                    ink.blend(&surface, 0.45)
                } else if is_active {
                    primary
                } else {
                    ink
                };
                let title_x = cx - (spacing as i32 / 2) + 2;
                let title_y = cy + circle_radius as i32 + 2;
                // Truncate title text to fit
                let max_title_len = (spacing / 8).max(4) as usize;
                let display_title = if self.steps[i].title.len() > max_title_len {
                    format!("{}..", &self.steps[i].title[..max_title_len.saturating_sub(2)])
                } else {
                    self.steps[i].title.clone()
                };
                context.draw_text(
                    Point::new(title_x, title_y),
                    &display_title,
                    &title_font,
                    title_color,
                    HorizontalAlignment::Left,
                );

                // Connect steps with lines
                if i < total_steps - 1 {
                    let next_cx = start_x + ((i + 1) as u32 * spacing + spacing / 2) as i32;
                    let line_color =
                        if self.steps[i].completed { complete } else { surface.blend(&ink, 0.15) };
                    context.draw_line(
                        Point::new(cx + circle_radius as i32 + 2, cy),
                        Point::new(next_cx - circle_radius as i32 - 2, cy),
                        line_color,
                    );
                }
            }
        } else {
            // No steps — draw placeholder
            let empty_font = Font::new("Arial", 14.0, false, false);
            context.draw_text(
                Point::new(rect.x + 10, rect.y + step_indicator_height as i32 / 2 - 6),
                "No steps configured",
                &empty_font,
                ink.blend(&surface, 0.45),
                HorizontalAlignment::Left,
            );
        }

        // ── Step indicator separator line ───────────────────────────────
        let sep_y = rect.y + step_indicator_height as i32;
        context.draw_line(
            Point::new(rect.x, sep_y),
            Point::new(rect.x + rect.width as i32, sep_y),
            border,
        );

        // ── Content Area ────────────────────────────────────────────────
        if !self.steps.is_empty() {
            // Content background
            let content_rect = Rect::new(rect.x, content_y, rect.width, content_height);
            context.fill_rect(content_rect, content_fill);

            // Step title in content area
            let title_font = Font::bold("Arial", 16.0);
            context.draw_text(
                Point::new(rect.x + 12, content_y + 8),
                &self.steps[self.current_step].title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );

            // Optional label
            if self.steps[self.current_step].optional {
                let opt_font = Font::new("Arial", 11.0, false, true);
                context.draw_text(
                    Point::new(rect.x + 12, content_y + 30),
                    "(Optional step)",
                    &opt_font,
                    ink.blend(&surface, 0.45),
                    HorizontalAlignment::Left,
                );
            }

            // Current step info
            let info_font = Font::new("Arial", 11.0, false, false);
            let info_text = format!("Step {} of {}", self.current_step + 1, self.steps.len());
            context.draw_text(
                Point::new(rect.x + 12, content_y + content_height as i32 - 16),
                &info_text,
                &info_font,
                ink.blend(&surface, 0.45),
                HorizontalAlignment::Left,
            );
        } else {
            // Empty content area
            let content_rect = Rect::new(rect.x, content_y, rect.width, content_height);
            context.fill_rect(content_rect, content_fill);
            let empty_font = Font::new("Arial", 14.0, false, false);
            context.draw_text(
                Point::new(rect.x + 12, content_y + 8),
                "Add steps to begin",
                &empty_font,
                ink.blend(&surface, 0.45),
                HorizontalAlignment::Left,
            );
        }

        // ── Navigation buttons separator ────────────────────────────────
        let nav_sep_y = content_y + content_height as i32;
        context.draw_line(
            Point::new(rect.x, nav_sep_y),
            Point::new(rect.x + rect.width as i32, nav_sep_y),
            border,
        );

        // ── Navigation Buttons ──────────────────────────────────────────
        let nav_y = nav_sep_y + 6;
        let btn_w = 80u32;
        let btn_h = nav_button_height;
        let btn_y = nav_y;

        // Cancel button (left side)
        let cancel_btn = Rect::new(rect.x + 8, btn_y, btn_w, btn_h);
        context.fill_rounded_rect(cancel_btn, 4, button_fill);
        context.draw_rounded_rect_stroke(cancel_btn, 4, border, 1);
        context.draw_text(
            Point::new(cancel_btn.x + 14, cancel_btn.y + 10),
            "Cancel",
            &Font::new("Arial", 12.0, false, false),
            button_ink,
            HorizontalAlignment::Left,
        );

        // Back button
        let back_enabled = !self.is_first() && !self.steps.is_empty();
        let back_btn =
            Rect::new(rect.x + rect.width as i32 - 2 * btn_w as i32 - 20, btn_y, btn_w, btn_h);
        // A disabled control dims toward its own ink rather than to a fixed light grey, which
        // is what previously made the disabled state ignore the appearance entirely.
        let back_color = if !back_enabled { button_fill.blend(&ink, 0.5) } else { button_fill };
        context.fill_rounded_rect(back_btn, 4, back_color);
        context.draw_rounded_rect_stroke(back_btn, 4, border, 1);
        context.draw_text(
            Point::new(back_btn.x + 18, back_btn.y + 10),
            "Back",
            &Font::new("Arial", 12.0, false, false),
            if !back_enabled { ink.blend(&surface, 0.45) } else { button_ink },
            HorizontalAlignment::Left,
        );

        // Next/Finish button
        let next_btn =
            Rect::new(rect.x + rect.width as i32 - btn_w as i32 - 8, btn_y, btn_w, btn_h);
        let is_last_step = self.is_last();
        let btn_text = if is_last_step { "Finish" } else { "Next" };
        let btn_color = if !is_enabled {
            primary.blend(&ink, 0.5)
        } else if is_last_step {
            complete
        } else {
            primary
        };
        context.fill_rounded_rect(next_btn, 4, btn_color);
        let btn_text_color =
            if !is_enabled { ink.blend(&surface, 0.45) } else { btn_color.contrast_color() };
        let text_x = if is_last_step { next_btn.x + 14 } else { next_btn.x + 20 };
        context.draw_text(
            Point::new(text_x, next_btn.y + 10),
            btn_text,
            &Font::bold("Arial", 12.0),
            btn_text_color,
            HorizontalAlignment::Left,
        );
    }
}

impl EventHandler for WizardDialog {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: _ } | Event::MouseRelease { pos, button: _ } => {
                let rect = self.geometry();
                let step_indicator_height = 50u32;
                let nav_button_height = 36u32;
                let nav_area_height = nav_button_height + 12;
                let content_height =
                    rect.height.saturating_sub(step_indicator_height + nav_area_height);
                let content_y = rect.y + step_indicator_height as i32;
                let nav_sep_y = content_y + content_height as i32;
                let nav_y = nav_sep_y + 6;
                let btn_w = 80u32;
                let btn_h = nav_button_height;
                let btn_y = nav_y;

                // Cancel button
                let cancel_btn = Rect::new(rect.x + 8, btn_y, btn_w, btn_h);
                if cancel_btn.contains_point(*pos) {
                    self.do_cancel();
                    return;
                }

                // Back button
                let back_btn = Rect::new(
                    rect.x + rect.width as i32 - 2 * btn_w as i32 - 20,
                    btn_y,
                    btn_w,
                    btn_h,
                );
                if back_btn.contains_point(*pos) && !self.is_first() && !self.steps.is_empty() {
                    self.previous();
                    return;
                }

                // Next/Finish button
                let next_btn =
                    Rect::new(rect.x + rect.width as i32 - btn_w as i32 - 8, btn_y, btn_w, btn_h);
                if next_btn.contains_point(*pos) && !self.steps.is_empty() {
                    if self.is_last() {
                        self.do_finish();
                    } else {
                        self.next();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn wizard_dialog_new_is_empty() {
        let wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        assert_eq!(wiz.step_count(), 0);
        assert_eq!(wiz.current_step(), 0);
        assert!(wiz.is_first());
        assert!(wiz.is_last());
        assert_eq!(wiz.kind(), WidgetKind::WizardDialog);
    }

    #[test]
    fn wizard_dialog_add_step() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Step 1", false);
        wiz.add_step("Step 2", true);
        wiz.add_step("Step 3", false);
        assert_eq!(wiz.step_count(), 3);
        assert_eq!(wiz.current_step(), 0);
        assert!(wiz.is_first());
        assert!(!wiz.is_last());
    }

    #[test]
    fn wizard_dialog_next_advances_step() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);
        wiz.add_step("C", false);

        assert!(wiz.next());
        assert_eq!(wiz.current_step(), 1);
        assert!(!wiz.is_first());
        assert!(!wiz.is_last());

        assert!(wiz.next());
        assert_eq!(wiz.current_step(), 2);
        assert!(wiz.is_last());
    }

    #[test]
    fn wizard_dialog_next_does_not_exceed_last() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Only", false);
        assert!(wiz.is_last());
        assert!(!wiz.next()); // Cannot advance past last
        assert_eq!(wiz.current_step(), 0);
    }

    #[test]
    fn wizard_dialog_previous_goes_back() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);
        wiz.next();
        assert_eq!(wiz.current_step(), 1);

        assert!(wiz.previous());
        assert_eq!(wiz.current_step(), 0);
        assert!(wiz.is_first());
    }

    #[test]
    fn wizard_dialog_previous_does_not_go_below_first() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        assert!(!wiz.previous());
        assert_eq!(wiz.current_step(), 0);
    }

    #[test]
    fn wizard_dialog_go_to_step() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);
        wiz.add_step("C", false);

        assert!(wiz.go_to_step(2));
        assert_eq!(wiz.current_step(), 2);

        // Same step — no change
        assert!(!wiz.go_to_step(2));

        // Go back to step 0 first, then out of range clamps to max
        wiz.go_to_step(0);
        assert!(wiz.go_to_step(99));
        assert_eq!(wiz.current_step(), 2);
    }

    #[test]
    fn wizard_dialog_reset() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);

        wiz.next();
        assert_eq!(wiz.current_step(), 1);
        assert!(wiz.steps()[0].completed);

        wiz.reset();
        assert_eq!(wiz.current_step(), 0);
        assert!(!wiz.steps()[0].completed);
    }

    #[test]
    fn wizard_dialog_current_step_title() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Introduction", false);
        assert_eq!(wiz.current_step_title(), "Introduction");

        wiz.add_step("Configuration", false);
        wiz.next();
        assert_eq!(wiz.current_step_title(), "Configuration");
    }

    #[test]
    fn wizard_dialog_finished_signal() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        wiz.finished.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Click Finish button
        let rect = wiz.geometry();
        let btn_x = rect.x + rect.width as i32 - 80 - 8;
        let btn_y = rect.y + rect.height as i32 - 36 - 6 - 6;
        wiz.handle_event(&Event::MousePress { pos: Point::new(btn_x + 20, btn_y + 10), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn wizard_dialog_cancelled_signal() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        wiz.cancelled.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Click Cancel button
        let rect = wiz.geometry();
        let btn_x = rect.x + 8;
        let btn_y = rect.y + rect.height as i32 - 36 - 6 - 6;
        wiz.handle_event(&Event::MousePress { pos: Point::new(btn_x + 20, btn_y + 10), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn wizard_dialog_step_changed_signal() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);

        let last_step = Arc::new(AtomicUsize::new(0));
        let ls = last_step.clone();
        wiz.step_changed.connect(move |val: Arc<usize>| {
            ls.store(*val, Ordering::SeqCst);
        });

        wiz.next();
        assert_eq!(last_step.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn wizard_dialog_next_marks_completed() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);

        wiz.next();
        assert!(wiz.steps()[0].completed);
        assert!(!wiz.steps()[1].completed);
    }

    #[test]
    fn wizard_dialog_optional_step() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Required", false);
        wiz.add_step("Optional", true);
        wiz.add_step("Required", false);

        assert!(!wiz.steps()[0].optional);
        assert!(wiz.steps()[1].optional);
    }

    #[test]
    fn wizard_dialog_svg_output() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Welcome", false);
        wiz.add_step("Config", false);
        wiz.add_step("Finish", false);
        let svg = crate::widget::svg::render_to_svg(&mut wiz);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn wizard_dialog_empty_svg() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        let svg = crate::widget::svg::render_to_svg(&mut wiz);
        assert!(svg.starts_with("<svg"));
    }
}
