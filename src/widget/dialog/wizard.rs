// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WizardDialog widget — a step-by-step wizard control with back/next/finish navigation.
//!
//! Displays a step indicator at the top (numbered circles), a content area with the
//! current step title, and navigation buttons (Back, Next/Finish, Cancel) at the bottom.
//! Emits `finished`, `cancelled`, and `step_changed` signals.
//!
//! # This is the control Flutter calls `Stepper` (BLUE22 · F-8)
//!
//! Flutter's `Stepper` is a multi-step flow; this crate's
//! [`Stepper`](crate::widget::container_widgets::Stepper) is a **numeric** spinner. Both names
//! exist here and mean different things, so the mapping is stated in both files rather than left
//! to a build error: a caller looking for the wizard reaches this module, and a caller looking for
//! the numeric spinner reaches `container_widgets/stepper.rs`. No control is missing.

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

    /// Height of the step-indicator band, in logical pixels.
    const STEP_INDICATOR_HEIGHT: u32 = 50;
    /// Height of a navigation button, in logical pixels.
    const NAV_BUTTON_HEIGHT: u32 = 36;
    /// Gap below the navigation separator before the button row.
    const NAV_ROW_GAP: i32 = 6;
    /// Outer margin at each end of the navigation row.
    const NAV_MARGIN: i32 = 8;
    /// Gap between two adjacent navigation buttons.
    const NAV_GAP: i32 = 4;
    /// Widest a navigation button may become, so three of them stay distinguishable.
    const NAV_MAX_BUTTON_WIDTH: u32 = 80;

    /// The three navigation buttons, in visual order: `[Cancel, Back, Next]`.
    ///
    /// # Why this is one function rather than two
    ///
    /// The row is both painted and hit-tested, and those two callers used to compute it
    /// independently: `draw` derived a 72 px column from the space actually available and
    /// placed the trio at `8 / 80 / 160`, while `handle_event` kept the older fixed 80 px
    /// column at `8 / 60 / 152`. Two of the three buttons therefore did not respond where
    /// they appeared — `Back` was painted 20 px right of its own hit box, `Next` 8 px right
    /// of its — and `Cancel` was painted 72 px wide but clickable 80 px.
    ///
    /// Deriving both from this function is the fix: a button cannot drift from its own
    /// click target if there is only one rectangle.
    ///
    /// # Placement
    ///
    /// `Cancel` is pinned to the left margin, `Next` to the right, and `Back` is centred
    /// between them. The width is taken from the space that is left over rather than fixed,
    /// because preserving 80 px while forcing the origins apart pushed the last button past
    /// the right edge (the row reached `x = 248` in a 240 px control).
    fn nav_button_rects(&self, rect: Rect) -> [Rect; 3] {
        let content_height =
            rect.height.saturating_sub(Self::STEP_INDICATOR_HEIGHT + Self::NAV_BUTTON_HEIGHT + 12);
        let nav_sep_y = rect.y + Self::STEP_INDICATOR_HEIGHT as i32 + content_height as i32;
        let btn_y = nav_sep_y + Self::NAV_ROW_GAP;

        let span = rect.width as i32;
        let btn_w = ((span - Self::NAV_MARGIN * 2 - Self::NAV_GAP * 2) / 3)
            .clamp(1, Self::NAV_MAX_BUTTON_WIDTH as i32) as u32;
        let cancel_x = rect.x + Self::NAV_MARGIN;
        let next_x = (rect.x + span - btn_w as i32 - Self::NAV_MARGIN).max(cancel_x + btn_w as i32);
        let back_x = ((cancel_x + next_x) / 2 - btn_w as i32 / 2).max(cancel_x + btn_w as i32);
        [
            Rect::new(cancel_x, btn_y, btn_w, Self::NAV_BUTTON_HEIGHT),
            Rect::new(back_x, btn_y, btn_w, Self::NAV_BUTTON_HEIGHT),
            Rect::new(next_x, btn_y, btn_w, Self::NAV_BUTTON_HEIGHT),
        ]
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
        let theme = crate::style::resolved_theme_style("wizard_dialog");
        // `wizard_dialog` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A frame painted in that colour would be byte-identical to the window
        // behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, the same distinction `Colors::input_background` draws for a
        // field.
        let window_fill = {
            let manager = crate::style::theme_manager();
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
        // The filled navigation button is an **affordance**, so it reads the theme's action
        // colour. The previous version took it from `background_color`, which is the wrong
        // token twice over: `wizard_dialog` classifies as `WidgetRole::Surface`, so that
        // token resolves to the *dialog's own fill*, and the three buttons then differed only
        // by the 0.12 blend the plain pads use. In dark the primary button came out grey
        // (139,139,139) instead of an accent, and on the last step a *different* token
        // (`success`) took over — so `Next` and `Finish` were two unrelated palettes rather
        // than one affordance in two states.
        //
        // `primary` is read from the theme directly, the same way `fab` reads it: an action
        // colour is what the token is for, and no state override exists for it.
        let primary = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .or_else(|| style.background_color.filter(|resolved| *resolved != window_fill))
            .unwrap_or_else(|| surface.blend(&ink, 0.5));
        let primary_ink = primary.contrast_color();
        // "Completed" is a *state*, so its green comes from the theme's semantic token
        // instead of the literal it used to carry. `semantic_color` takes and releases the
        // lock and returns an owned colour, so no guard outlives the call.
        let complete = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .unwrap_or(Color::rgb(52, 199, 89));

        // Background
        context.fill_rect(rect, surface);

        // The wizard's chrome needs 98 px before any content exists: the 50 px step
        // indicator, the 48 px navigation band and a separator. A shorter control used to
        // get a content band that `saturating_sub` had reduced to zero, and the navigation
        // row at `content_y + 0 + 6` then drew *on top of* the indicator — three labelled
        // buttons 78 px into a 120 px box, with their 36 px height reaching past y = 114.
        // The two bands are stacked from the top with the navigation band's height reserved,
        // so the content band is what shrinks; and when the frame itself is shorter than the
        // chrome, only the indicator is drawn, because a truncated step indicator still
        // reads as a wizard while an overlapping button row does not.
        let step_indicator_height = Self::STEP_INDICATOR_HEIGHT;
        let nav_button_height = Self::NAV_BUTTON_HEIGHT;
        let nav_area_height = nav_button_height + 12; // 12 padding
        let chrome_height = step_indicator_height + nav_area_height;
        let has_nav = rect.height >= chrome_height;
        let content_y = rect.y + step_indicator_height as i32;
        let content_height = if has_nav {
            rect.height - chrome_height
        } else {
            step_indicator_height.min(rect.height)
        };

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

                // Draw circle. The step indicator is the wizard's primary chrome, so it is
                // skipped — not clipped — when the frame is too short to hold it: a circle
                // that is still painted while the boxes below it are omitted reads as a
                // rendering fault rather than as a small dialog.
                if rect.height < step_indicator_height {
                    break;
                }
                context.fill_circle_aa(Point::new(cx, cy), circle_radius, circle_color);

                // Step number inside circle. Centred on the circle rather than offset by a
                // fixed `-4 / -6` written for a one-digit label: the offset put a two-digit
                // step to the right of centre, and the label was never fitted at all.
                let label = format!("{}", i + 1);
                let label_font = Font::bold("Arial", 11.0);
                let label_metrics = context.measure_text(&label, &label_font);
                let label_box = Rect::new(
                    cx - circle_radius as i32,
                    cy - circle_radius as i32,
                    circle_radius * 2,
                    circle_radius * 2,
                );
                context.draw_text_fitted(
                    Rect::new(
                        label_box.x,
                        (label_box.y + (label_box.height as i32 - label_metrics.height as i32) / 2)
                            .max(label_box.y),
                        label_box.width,
                        label_metrics.height.max(1),
                    ),
                    &label,
                    &label_font,
                    text_color,
                    HorizontalAlignment::Center,
                );

                // Step title below circle. Bounded by the step's own cell, so a long title
                // truncates inside its column instead of over the neighbouring step; the
                // former byte-slice truncation also panicked on a multi-byte character
                // boundary, which a fitted draw cannot.
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
                let title_metrics = context.measure_text("M", &title_font);
                let title_y = cy + circle_radius as i32 + 2;
                context.draw_text_fitted(
                    Rect::new(
                        cx - (spacing as i32 / 2),
                        title_y,
                        spacing,
                        title_metrics.height.max(1),
                    ),
                    &self.steps[i].title,
                    &title_font,
                    title_color,
                    HorizontalAlignment::Center,
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
            // No steps — draw placeholder. Confined to the band above the separator, which
            // is also where the navigation row begins when the frame is short.
            let empty_font = Font::new("Arial", 14.0, false, false);
            let empty_metrics = context.measure_text("No steps configured", &empty_font);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 10,
                    (rect.y + (step_indicator_height as i32 - empty_metrics.height as i32) / 2)
                        .max(rect.y),
                    rect.width.saturating_sub(20),
                    empty_metrics.height.max(1),
                ),
                "No steps configured",
                &empty_font,
                ink.blend(&surface, 0.45),
                HorizontalAlignment::Left,
            );
        }

        // ── Step indicator separator line ───────────────────────────────
        let sep_y = rect.y + step_indicator_height as i32;
        if sep_y <= rect.y + rect.height as i32 {
            context.draw_line(
                Point::new(rect.x, sep_y),
                Point::new(rect.x + rect.width as i32, sep_y),
                border,
            );
        }

        // ── Content Area ────────────────────────────────────────────────
        // The well is the region between the indicator and the navigation band. When the
        // frame is too short to hold both it is painted behind the buttons rather than
        // claiming a band of its own, so the two never overlap.
        let content_rect = Rect::new(rect.x, content_y, rect.width, content_height);
        context.fill_rect(content_rect, content_fill);
        if !self.steps.is_empty() {
            // Step title in content area
            let title_font = Font::bold("Arial", 16.0);
            let title_metrics =
                context.measure_text(&self.steps[self.current_step].title, &title_font);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 12,
                    content_rect.y + 8,
                    rect.width.saturating_sub(24),
                    title_metrics.height.max(1),
                ),
                &self.steps[self.current_step].title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );

            // Optional label
            if self.steps[self.current_step].optional {
                let opt_font = Font::new("Arial", 11.0, false, true);
                let opt_metrics = context.measure_text("(Optional step)", &opt_font);
                context.draw_text_fitted(
                    Rect::new(
                        rect.x + 12,
                        content_rect.y + 30,
                        rect.width.saturating_sub(24),
                        opt_metrics.height.max(1),
                    ),
                    "(Optional step)",
                    &opt_font,
                    ink.blend(&surface, 0.45),
                    HorizontalAlignment::Left,
                );
            }

            // Current step info. Anchored to the well's own bottom edge, so it follows the
            // band rather than a fixed offset from the frame that the button row shares.
            let info_font = Font::new("Arial", 11.0, false, false);
            let info_text = format!("Step {} of {}", self.current_step + 1, self.steps.len());
            let info_metrics = context.measure_text(&info_text, &info_font);
            let info_y = (content_rect.y + content_rect.height as i32 - info_metrics.height as i32)
                .max(content_rect.y);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 12,
                    info_y,
                    rect.width.saturating_sub(24),
                    info_metrics.height.max(1),
                ),
                &info_text,
                &info_font,
                ink.blend(&surface, 0.45),
                HorizontalAlignment::Left,
            );
        } else {
            // Empty content area. Drawn inside the well's two rows so a 120 px wizard no
            // longer stacks this above the first navigation button.
            let empty_font = Font::new("Arial", 14.0, false, false);
            let empty_metrics = context.measure_text("Add steps to begin", &empty_font);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 12,
                    (content_rect.y + 8).min(content_rect.y + content_rect.height as i32),
                    rect.width.saturating_sub(24),
                    empty_metrics.height.max(1),
                ),
                "Add steps to begin",
                &empty_font,
                ink.blend(&surface, 0.45),
                HorizontalAlignment::Left,
            );
        }

        // ── Navigation buttons separator ────────────────────────────────
        // Nothing below this line is drawn when the frame is too short to hold the band,
        // because the row would otherwise hang over the content well it is meant to follow.
        if !has_nav {
            return;
        }
        let nav_sep_y = content_y + content_height as i32;
        context.draw_line(
            Point::new(rect.x, nav_sep_y),
            Point::new(rect.x + rect.width as i32, nav_sep_y),
            border,
        );

        // ── Navigation Buttons ──────────────────────────────────────────
        // The three buttons come from **one** layout function, which the event handler also
        // calls. They used to be computed twice — here from a 72 px column and in
        // `handle_event` from a fixed 80 px one placed 20 px and 8 px elsewhere. The painted
        // `Back` therefore sat 20 px right of where it could be clicked, and the painted
        // `Next` 8 px right of its own hit box: two of the three buttons did not respond
        // where they appeared. Order alone was never the problem; **two layouts for one row**
        // was, so there is now one.
        let nav = self.nav_button_rects(rect);
        let (cancel_btn, back_btn, next_btn) = (nav[0], nav[1], nav[2]);
        // A label is one line box tall and the renderer's origin is its **top** edge, so
        // centring means offsetting by half the *difference*. Passing the button rect
        // unchanged put every label on the button's top edge with 24 px of empty pad below
        // it, which is what made the row look wrong even where the colours were right.
        let nav_font = Font::new("Arial", 12.0, false, false);
        let nav_text_height = context.measure_text("M", &nav_font).height.max(1);
        let label_of = |button: Rect| {
            Rect::new(
                button.x,
                button.y + (button.height as i32 - nav_text_height as i32) / 2,
                button.width,
                nav_text_height,
            )
        };

        // Cancel button (left side)
        context.fill_rounded_rect(cancel_btn, 4, button_fill);
        context.draw_rounded_rect_stroke(cancel_btn, 4, border, 1);
        context.draw_text_fitted(
            label_of(cancel_btn),
            "Cancel",
            &nav_font,
            button_ink,
            HorizontalAlignment::Center,
        );

        // Back button
        //
        // # Why the disabled state is now derived from *one* colour
        //
        // A disabled button was painted by dimming the fill and the label independently:
        // the fill was `button_fill.blend(ink, 0.5)` and the label `ink.blend(surface, 0.45)`.
        // Neither is wrong on its own, but together they landed on the same grey — fill
        // `139,139,139`, label `137,137,137`, a difference of two levels per channel — so the
        // `Back` button rendered as an **unlabelled grey block**. That is not a subtle contrast
        // problem; a button with no visible caption reads as an empty control, which is how it
        // was reported.
        //
        // Two independent dimming formulas for one button cannot be kept in agreement by
        // inspection, so the disabled button now takes the *same* pair of functions the
        // enabled one does: the fill is derived from the button's own base colour, and the
        // label is that fill's own `contrast_color()`. Contrast is therefore correct by
        // construction in every appearance, not by two constants happening to round apart.
        let back_enabled = !self.is_first() && !self.steps.is_empty();
        let back_color = if back_enabled { button_fill } else { button_fill.blend(&surface, 0.55) };
        let back_text_color = if back_enabled {
            button_ink
        } else {
            back_color.contrast_color().blend(&surface, 0.35)
        };
        context.fill_rounded_rect(back_btn, 4, back_color);
        context.draw_rounded_rect_stroke(back_btn, 4, border, 1);
        context.draw_text_fitted(
            label_of(back_btn),
            "Back",
            &nav_font,
            back_text_color,
            HorizontalAlignment::Center,
        );

        // Next/Finish button. It is one affordance in two states, so both states read the
        // same action token; only the label changes. Deriving the last step's fill from
        // `success` made `Next` and `Finish` look like two different controls.
        //
        // The disabled label is the same `contrast_color()` path as the enabled one, dimmed a
        // step toward the surface. The previous `ink.blend(surface, 0.45)` was a *fixed* grey
        // that ignored what it was being painted on, so on the accent fill it was whatever
        // that formula produced rather than something legible.
        let is_last_step = self.is_last();
        let btn_text = if is_last_step { "Finish" } else { "Next" };
        let btn_color = if is_enabled { primary } else { primary.blend(&surface, 0.55) };
        let btn_text_color = if is_enabled {
            primary.contrast_color()
        } else {
            btn_color.contrast_color().blend(&surface, 0.35)
        };
        context.fill_rounded_rect(next_btn, 4, btn_color);
        context.draw_text_fitted(
            label_of(next_btn),
            btn_text,
            &Font::bold("Arial", 12.0),
            btn_text_color,
            HorizontalAlignment::Center,
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
                let nav = self.nav_button_rects(rect);
                if nav[0].contains_point(*pos) {
                    self.do_cancel();
                    return;
                }
                if nav[1].contains_point(*pos) && !self.is_first() && !self.steps.is_empty() {
                    self.previous();
                    return;
                }
                if nav[2].contains_point(*pos) && !self.steps.is_empty() {
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
