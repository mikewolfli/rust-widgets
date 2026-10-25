// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WizardDialog widget — a step-by-step wizard control with back/next/finish navigation.
//!
//! Displays a step indicator at the top (numbered circles), a content area with the
//! current step title, and navigation buttons (Back, Next/Finish, Cancel) at the bottom.
//! Emits `finished`, `canceled`, and `step_changed` signals.
//!
//! # This is the control a toolkit calls `Stepper` (BLUE22 · F-8)
//!
//! A toolkit's `Stepper` is a multi-step flow; this crate's
//! [`Stepper`](crate::widget::container_widgets::Stepper) is a **numeric** spinner. Both names
//! exist here and mean different things, so the mapping is stated in both files rather than left
//! to a build error: a caller looking for the wizard reaches this module, and a caller looking for
//! the numeric spinner reaches `container_widgets/stepper.rs`. No control is missing.

use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// An option that hides or adds one of the wizard's navigation affordances.
///
/// The three names are the visible parts of a wizard's option set: a help action, and the
/// ability to remove the two trailing navigation actions. Everything here changes what the
/// navigation row paints and hit-tests, so an option is never merely recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WizardOption {
    /// Adds a Help action that emits [`WizardDialog::help_requested`].
    HelpButton,
    /// Removes the Cancel action.
    NoCancelButton,
    /// Removes the Back action.
    NoBackButton,
}

impl WizardOption {
    /// The spelling used by [`WizardDialog::set_option`] and its query counterpart.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HelpButton => "help_button",
            Self::NoCancelButton => "no_cancel_button",
            Self::NoBackButton => "no_back_button",
        }
    }
}

/// A named field the wizard tracks for its pages.
///
/// The wizard does not read or write the widget behind a field: a field is a **name to
/// handle** mapping, exactly as a wizard's field registry is. The host resolves `widget`
/// through the object table and reads or writes `property` (empty means the widget's
/// primary value). That keeps page state on the page while letting Finish read a named
/// answer without searching the tree.
pub struct WizardField {
    /// The field's name, unique within one wizard.
    pub name: String,
    /// The widget that holds the value.
    pub widget: ObjectId,
    /// The property to read or write; empty means the widget's primary value.
    pub property: String,
}

/// One action in the navigation row.
///
/// Public so [`WizardDialog::set_button_text`] / [`WizardDialog::button_text`] can name the
/// action whose label a caller wants to replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    /// The Cancel action.
    Cancel,
    /// The Back action.
    Back,
    /// Next, or Finish when the current step is the last.
    Next,
    /// The Help action.
    Help,
    /// The caller-added action at this index.
    Custom(usize),
}

/// How the wizard presents its steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardStyle {
    /// The original top strip of numbered circles.
    #[default]
    TopStrip,
    /// A clickable list of step titles down the left side.
    Sidebar,
}

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
    /// The page widget registered for each step, parallel to `steps`.
    pages: Vec<Option<ObjectId>>,
    /// Named fields the host can resolve after the flow finishes.
    fields: Vec<WizardField>,
    /// Free-form buttons the caller adds; each emits `custom_button_clicked` with its index.
    custom_buttons: Vec<String>,
    /// The enabled [`WizardOption`]s.
    options: Vec<WizardOption>,
    current_step: usize,
    /// Title shown in the wizard's own chrome.
    ///
    /// Kept on the control rather than in a host-side map: the title is part of
    /// what the dialog *is*, and a host that held it separately could not paint it.
    title: String,
    /// Secondary line drawn under the step title. Empty by default.
    subtitle: String,
    /// Optional tint for the step-indicator band.
    banner_color: Option<Color>,
    /// Optional tint for the content well.
    watermark_color: Option<Color>,
    /// An optional picture painted in the content well behind the step heading.
    watermark_pixmap: Option<crate::image::Image>,
    /// How the steps are presented.
    style: WizardStyle,
    /// Per-action label overrides; an absent action keeps its localised default.
    button_texts: Vec<(NavAction, String)>,
    /// Emitted when the user activates a step in the sidebar.
    pub sidebar_page_activated: Signal1<usize>,
    /// Emitted when the user clicks Finish on the last step.
    pub finished: GenericSignal,
    /// Emitted when the user clicks Cancel (or presses Escape).
    pub canceled: GenericSignal,
    /// Emitted when the current step index changes.
    pub step_changed: Signal1<usize>,
    /// Emitted when a Help action is activated.
    pub help_requested: GenericSignal,
    /// Emitted with the index of an activated custom button.
    pub custom_button_clicked: Signal1<usize>,
    /// Which navigation action a press began on, or `None` when no nav press is in flight.
    ///
    /// A press only arms the button; the action commits on the matching release. The
    /// combined press/release arm this replaced ran `previous`/`next`/`finish` twice for
    /// one click, once per event.
    pressed_button: Option<NavAction>,
    /// The navigation action the keyboard is on, for the focus ring and Enter.
    focused_button: Option<NavAction>,
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
            pages: Vec::new(),
            fields: Vec::new(),
            custom_buttons: Vec::new(),
            options: Vec::new(),
            current_step: 0,
            title,
            subtitle: String::new(),
            banner_color: None,
            watermark_color: None,
            watermark_pixmap: None,
            style: WizardStyle::TopStrip,
            button_texts: Vec::new(),
            sidebar_page_activated: Signal1::new(),
            finished: GenericSignal::new(),
            canceled: GenericSignal::new(),
            step_changed: Signal1::new(),
            help_requested: GenericSignal::new(),
            custom_button_clicked: Signal1::new(),
            pressed_button: None,
            focused_button: None,
        }
    }

    /// Returns the wizard's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Compatibility spelling of [`Self::canceled`] (W-19): the same emitted signal.
    pub fn cancelled(&self) -> &GenericSignal {
        &self.canceled
    }

    /// Sets the wizard's title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// Adds a step to the wizard.
    pub fn add_step(&mut self, title: impl Into<String>, optional: bool) {
        self.steps.push(WizardStep::new(title, optional));
        self.pages.push(None);
        self.base.request_redraw();
    }

    /// Adds a step and returns its index, which is the handle the page and field APIs use.
    ///
    /// This is [`add_step`](Self::add_step) with the index made explicit, so a caller can
    /// attach a page with [`set_page`](Self::set_page) without first counting the steps.
    pub fn add_page(&mut self, title: impl Into<String>, optional: bool) -> usize {
        let index = self.steps.len();
        self.add_step(title, optional);
        index
    }

    /// Removes the step at `index`, along with its page binding.
    ///
    /// Returns the removed step, or `None` when `index` is out of range. When the removed
    /// step was the last one and the wizard was on it, the current step moves back so the
    /// control is never on a step that does not exist.
    pub fn remove_page(&mut self, index: usize) -> Option<WizardStep> {
        if index >= self.steps.len() {
            return None;
        }
        let step = self.steps.remove(index);
        let page = if index < self.pages.len() { self.pages.remove(index) } else { None };
        if let Some(id) = page {
            self.base.remove_child_linked(id);
            crate::set_widget_visible(id, false);
        }
        if self.current_step >= self.steps.len() {
            self.current_step = self.steps.len().saturating_sub(1);
            self.step_changed.emit(self.current_step);
        }
        self.sync_page_visibility();
        self.base.request_redraw();
        Some(step)
    }

    /// Returns the page widget registered for `index`, if any.
    pub fn page(&self, index: usize) -> Option<ObjectId> {
        self.pages.get(index).copied().flatten()
    }

    /// Binds `page` to the step at `index`; `None` clears the binding.
    ///
    /// The binding is maintained in both directions, including on replacement: the old
    /// page is unlinked rather than left in the child list, and only the current step's
    /// page is left visible. An out-of-range index is ignored rather than resizing the
    /// registry behind the caller's back.
    pub fn set_page(&mut self, index: usize, page: Option<ObjectId>) {
        if index >= self.steps.len() {
            return;
        }
        while self.pages.len() < self.steps.len() {
            self.pages.push(None);
        }
        let old = self.pages[index];
        if old == page {
            return;
        }
        if let Some(id) = old {
            self.base.remove_child_linked(id);
            crate::set_widget_visible(id, false);
        }
        self.pages[index] = page;
        if let Some(id) = page {
            self.base.add_child_linked(id);
        }
        self.sync_page_visibility();
        self.base.request_redraw();
    }

    /// Registers a named field against `widget`, replacing any field with the same name.
    ///
    /// An empty name is rejected (returns `false`), because a field without a name cannot
    /// be looked up. `property` names what the host should read; an empty string means the
    /// widget's primary value.
    pub fn register_field(
        &mut self,
        name: impl Into<String>,
        widget: ObjectId,
        property: impl Into<String>,
    ) -> bool {
        let name = name.into();
        if name.is_empty() {
            return false;
        }
        let field = WizardField { name: name.clone(), widget, property: property.into() };
        if let Some(existing) = self.fields.iter_mut().find(|candidate| candidate.name == name) {
            *existing = field;
        } else {
            self.fields.push(field);
        }
        true
    }

    /// Returns the field registered under `name`.
    pub fn field(&self, name: &str) -> Option<&WizardField> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Returns every registered field, in registration order.
    pub fn fields(&self) -> &[WizardField] {
        &self.fields
    }

    /// Removes the field registered under `name`, returning whether it existed.
    pub fn remove_field(&mut self, name: &str) -> bool {
        let before = self.fields.len();
        self.fields.retain(|field| field.name != name);
        self.fields.len() != before
    }

    /// Returns the subtitle painted under the step title. Empty by default.
    pub fn subtitle(&self) -> &str {
        &self.subtitle
    }

    /// Sets the subtitle painted under the step title and requests a redraw.
    pub fn set_subtitle(&mut self, subtitle: impl Into<String>) {
        let subtitle = subtitle.into();
        if self.subtitle == subtitle {
            return;
        }
        self.subtitle = subtitle;
        self.base.request_redraw();
    }

    /// Returns the optional tint behind the step indicator.
    pub fn banner_color(&self) -> Option<Color> {
        self.banner_color
    }

    /// Tints the step-indicator band; `None` restores the untinted band.
    pub fn set_banner_color(&mut self, color: Option<Color>) {
        if self.banner_color == color {
            return;
        }
        self.banner_color = color;
        self.base.request_redraw();
    }

    /// Returns the optional tint of the content well.
    pub fn watermark_color(&self) -> Option<Color> {
        self.watermark_color
    }

    /// Tints the content well; `None` restores the untinted well.
    pub fn set_watermark_color(&mut self, color: Option<Color>) {
        if self.watermark_color == color {
            return;
        }
        self.watermark_color = color;
        self.base.request_redraw();
    }

    /// The picture painted in the content well, if one was installed.
    pub fn watermark_pixmap(&self) -> Option<&crate::image::Image> {
        self.watermark_pixmap.as_ref()
    }

    /// Installs the content-well watermark picture and repaints.
    ///
    /// The picture is fitted inside the content well preserving its aspect ratio, so a caller
    /// does not have to know the wizard's current size. It is painted behind the heading, which
    /// keeps the step title readable over it.
    pub fn set_watermark_pixmap(&mut self, pixmap: crate::image::Image) {
        if self
            .watermark_pixmap
            .as_ref()
            .is_some_and(|current| current.rgba8_data() == pixmap.rgba8_data())
        {
            return;
        }
        self.watermark_pixmap = Some(pixmap);
        self.base.request_redraw();
    }

    /// Removes the watermark picture; `None` restores the plain content well.
    pub fn clear_watermark_pixmap(&mut self) {
        if self.watermark_pixmap.is_none() {
            return;
        }
        self.watermark_pixmap = None;
        self.base.request_redraw();
    }

    /// How the steps are presented.
    pub fn wizard_style(&self) -> WizardStyle {
        self.style
    }

    /// Selects the top-strip or sidebar presentation and repaints when it changes.
    pub fn set_style(&mut self, style: WizardStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// Whether the clickable sidebar of step titles is shown.
    pub fn is_sidebar_visible(&self) -> bool {
        self.style == WizardStyle::Sidebar
    }

    /// Shows or hides the sidebar.
    ///
    /// This is the boolean spelling of [`WizardDialog::set_style`]: one field decides the
    /// presentation, so the two setters can never disagree about which one is in effect.
    pub fn set_sidebar_visible(&mut self, visible: bool) {
        self.set_style(if visible { WizardStyle::Sidebar } else { WizardStyle::TopStrip });
    }

    /// Replaces the label a navigation action paints.
    ///
    /// An empty string removes the override and restores the localised default. Custom actions
    /// can be renamed the same way, by their index.
    pub fn set_button_text(&mut self, action: NavAction, text: impl Into<String>) {
        let text = text.into();
        if text.is_empty() {
            self.button_texts.retain(|(candidate, _)| *candidate != action);
        } else if let Some((_, current)) =
            self.button_texts.iter_mut().find(|(candidate, _)| *candidate == action)
        {
            if *current == text {
                return;
            }
            *current = text;
        } else {
            self.button_texts.push((action, text));
        }
        self.base.request_redraw();
    }

    /// The label `action` paints: the caller's override or the localised default.
    pub fn button_text(&self, action: NavAction) -> String {
        self.label_of_action(action)
    }

    /// Adds a custom action to the navigation row and returns its index.
    ///
    /// Custom actions sit to the left of the built-in ones. Activating one emits
    /// [`custom_button_clicked`](Self::custom_button_clicked) with its index; the wizard
    /// itself takes no other action, because only the caller knows what the button means.
    pub fn add_custom_button(&mut self, text: impl Into<String>) -> usize {
        let index = self.custom_buttons.len();
        self.custom_buttons.push(text.into());
        self.base.request_redraw();
        index
    }

    /// Returns the custom action labels, in index order.
    pub fn custom_buttons(&self) -> &[String] {
        &self.custom_buttons
    }

    /// Enables or disables a [`WizardOption`].
    ///
    /// The change reaches both the painted row and its hit targets, because both are
    /// derived from [`nav_slots`](Self::nav_slots).
    pub fn set_option(&mut self, option: WizardOption, enabled: bool) {
        let present = self.options.contains(&option);
        if present == enabled {
            return;
        }
        if enabled {
            self.options.push(option);
        } else {
            self.options.retain(|candidate| *candidate != option);
        }
        // A hidden action cannot hold the keyboard focus.
        if !enabled && self.focused_button == Some(Self::action_of_option(option)) {
            self.focused_button = None;
        }
        self.base.request_redraw();
    }

    /// Returns whether `option` is enabled.
    pub fn has_option(&self, option: WizardOption) -> bool {
        self.options.contains(&option)
    }

    /// The action an option controls, for focus bookkeeping.
    fn action_of_option(option: WizardOption) -> NavAction {
        match option {
            WizardOption::HelpButton => NavAction::Help,
            WizardOption::NoCancelButton => NavAction::Cancel,
            WizardOption::NoBackButton => NavAction::Back,
        }
    }

    /// Shows the current step's page and hides every other registered page.
    fn sync_page_visibility(&mut self) {
        let visible = self.base.is_visible();
        for (index, page) in self.pages.iter().enumerate() {
            if let Some(id) = page {
                crate::set_widget_visible(*id, visible && index == self.current_step);
            }
        }
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
            self.sync_page_visibility();
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
            self.sync_page_visibility();
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
            self.sync_page_visibility();
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
        self.sync_page_visibility();
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

    /// Called when the user cancels: the Cancel action, Escape, or a host-driven command.
    fn do_cancel(&mut self) {
        self.canceled.emit();
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

    /// Widest the sidebar may become, in logical pixels.
    const SIDEBAR_MAX_WIDTH: u32 = 140;
    /// Height of one sidebar step row.
    const SIDEBAR_ROW_HEIGHT: i32 = 34;

    /// The sidebar's width: zero when the top strip is in effect.
    fn sidebar_width(&self, rect: Rect) -> u32 {
        if self.style == WizardStyle::Sidebar && !self.steps.is_empty() {
            Self::SIDEBAR_MAX_WIDTH.min(rect.width / 3)
        } else {
            0
        }
    }

    /// The sidebar rectangle, when the sidebar presentation is in effect.
    fn sidebar_rect(&self, rect: Rect) -> Option<Rect> {
        let width = self.sidebar_width(rect);
        (width > 0).then(|| Rect::new(rect.x, rect.y, width, rect.height))
    }

    /// The band the navigation row may occupy: the frame less the sidebar.
    fn nav_area(&self, rect: Rect) -> Rect {
        let width = self.sidebar_width(rect);
        Rect::new(rect.x + width as i32, rect.y, rect.width.saturating_sub(width), rect.height)
    }

    /// The step whose sidebar row contains `pos`.
    fn sidebar_row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.sidebar_rect(self.geometry())?;
        if !rect.contains_point(pos) {
            return None;
        }
        let top = rect.y + 8;
        if pos.y < top {
            return None;
        }
        let index = ((pos.y - top) / Self::SIDEBAR_ROW_HEIGHT) as usize;
        let row_y = top + index as i32 * Self::SIDEBAR_ROW_HEIGHT;
        (index < self.steps.len()
            && row_y + Self::SIDEBAR_ROW_HEIGHT <= rect.y + rect.height as i32)
            .then_some(index)
    }

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

    /// Returns the navigation action at `pos`, if any.
    ///
    /// The same rectangles are painted and hit-tested, so the press, the release and the
    /// picture all agree about which button is under the pointer.
    fn nav_button_at(&self, pos: Point) -> Option<NavAction> {
        let rect = self.nav_area(self.geometry());
        self.nav_slots(rect)
            .into_iter()
            .find(|(_, rect)| rect.contains_point(pos))
            .map(|(action, _)| action)
    }

    /// The actions actually in the navigation row, with their rectangles.
    ///
    /// # Why the default case delegates to `nav_button_rects`
    ///
    /// A wizard with no options and no custom actions has exactly the three-button row the
    /// control has always painted, and its geometry is the audited layout: `Cancel` at the
    /// left margin, `Next` at the right, `Back` centred between them. Reproducing that from
    /// a generic n-column formula would move the buttons. The generic formula is therefore
    /// used only when the row actually differs — an option removed, Help added, or a custom
    /// action installed. Both the paint and the hit test read this one function.
    fn nav_slots(&self, rect: Rect) -> Vec<(NavAction, Rect)> {
        let plain = self.custom_buttons.is_empty()
            && !self.has_option(WizardOption::HelpButton)
            && !self.has_option(WizardOption::NoCancelButton)
            && !self.has_option(WizardOption::NoBackButton);
        if plain {
            let legacy = self.nav_button_rects(rect);
            return vec![
                (NavAction::Cancel, legacy[0]),
                (NavAction::Back, legacy[1]),
                (NavAction::Next, legacy[2]),
            ];
        }

        let mut actions: Vec<NavAction> =
            (0..self.custom_buttons.len()).map(NavAction::Custom).collect();
        if self.has_option(WizardOption::HelpButton) {
            actions.push(NavAction::Help);
        }
        if !self.has_option(WizardOption::NoCancelButton) {
            actions.push(NavAction::Cancel);
        }
        if !self.has_option(WizardOption::NoBackButton) {
            actions.push(NavAction::Back);
        }
        actions.push(NavAction::Next);

        // The row's own vertical placement is the audited one: the same baseline the
        // three-button layout uses, so adding an action changes the columns, not the band.
        let legacy = self.nav_button_rects(rect);
        let btn_y = legacy[0].y;
        let count = actions.len() as i32;
        let available =
            (rect.width as i32 - Self::NAV_MARGIN * 2 - Self::NAV_GAP * (count - 1)).max(count);
        let btn_w = (available / count).clamp(1, Self::NAV_MAX_BUTTON_WIDTH as i32);
        actions
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                let x = rect.x + Self::NAV_MARGIN + index as i32 * (btn_w + Self::NAV_GAP);
                (action, Rect::new(x, btn_y, btn_w as u32, Self::NAV_BUTTON_HEIGHT))
            })
            .collect()
    }

    /// Performs a navigation action.
    ///
    /// Both the mouse release and the keyboard's Enter go through here, so a click and a
    /// key press cannot diverge in what they do.
    fn activate(&mut self, action: NavAction) {
        match action {
            NavAction::Cancel => self.do_cancel(),
            NavAction::Back => {
                if !self.is_first() && !self.steps.is_empty() {
                    self.previous();
                }
            }
            NavAction::Next => {
                if !self.steps.is_empty() {
                    if self.is_last() {
                        self.do_finish();
                    } else {
                        self.next();
                    }
                }
            }
            NavAction::Help => self.help_requested.emit(),
            NavAction::Custom(index) => self.custom_button_clicked.emit(index),
        }
    }

    /// The label a navigation action paints.
    ///
    /// A caller override wins; otherwise the label goes through the same
    /// [`localized_label`](super::localized_label) resolution every other dialog uses, so the
    /// wizard is no longer the one dialog whose buttons ignore the catalogue.
    fn label_of_action(&self, action: NavAction) -> String {
        if let Some((_, text)) =
            self.button_texts.iter().find(|(candidate, _)| *candidate == action)
        {
            return text.clone();
        }
        match action {
            NavAction::Cancel => super::localized_label("common.button.cancel", "Cancel"),
            NavAction::Back => super::localized_label("dialog.wizard.back", "Back"),
            NavAction::Next if self.is_last() => {
                super::localized_label("dialog.wizard.finish", "Finish")
            }
            NavAction::Next => super::localized_label("dialog.wizard.next", "Next"),
            NavAction::Help => super::localized_label("dialog.wizard.help", "Help"),
            NavAction::Custom(index) => self.custom_buttons.get(index).cloned().unwrap_or_default(),
        }
    }

    /// Moves the keyboard focus across the actions that are actually present.
    fn move_focus(&mut self, forward: bool) {
        let rect = self.nav_area(self.geometry());
        let slots = self.nav_slots(rect);
        if slots.is_empty() {
            return;
        }
        let actions: Vec<NavAction> = slots.into_iter().map(|(action, _)| action).collect();
        let position = self
            .focused_button
            .and_then(|focused| actions.iter().position(|action| *action == focused));
        let next = match position {
            Some(index) if forward => (index + 1) % actions.len(),
            Some(index) => (index + actions.len() - 1) % actions.len(),
            None if forward => 0,
            None => actions.len() - 1,
        };
        self.focused_button = Some(actions[next]);
        self.base.request_redraw();
    }
}

impl Widget for WizardDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Shows the wizard and its current page.
    fn show(&mut self) {
        self.base.show();
        self.sync_page_visibility();
    }

    /// Hides the wizard and the pages it owns.
    fn hide(&mut self) {
        self.base.hide();
        self.sync_page_visibility();
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
            "finished" => Some(EventSignalRef::unit("finished", &self.finished)),
            "canceled" => Some(EventSignalRef::unit("canceled", &self.canceled)),
            "step_changed" => {
                Some(EventSignalRef::mapped("step_changed", &self.step_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "help_requested" => Some(EventSignalRef::unit("help_requested", &self.help_requested)),
            "custom_button_clicked" => Some(EventSignalRef::mapped(
                "custom_button_clicked",
                &self.custom_button_clicked,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "sidebar_page_activated" => Some(EventSignalRef::mapped(
                "sidebar_page_activated",
                &self.sidebar_page_activated,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            _ => None,
        }
    }
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
            "subtitle" => Ok(CapabilityValue::String(self.subtitle().to_string())),
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
            "subtitle" => {
                self.set_subtitle(expect_string(value)?);
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
            "subtitle",
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
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("wizard_dialog");
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
        // frame in either appearance. A watermark is a caller-chosen tint for the well, so
        // it replaces (rather than blends with) the derived step when one is set; the
        // default is the derived colour, which is why the resting frame is unchanged.
        let content_fill = self.watermark_color.unwrap_or_else(|| surface.blend(&ink, 0.04));
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
        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // The wizard's chrome needs 98 px before any content exists: the 50 px step
        // indicator, the 48 px navigation band and a separator. A shorter control used to
        // get a content band that `saturating_sub` had reduced to zero, and the navigation
        // row at `content_y + 0 + 6` then drew *on top of* the indicator — three labelled
        // buttons 78 px into a 120 px box, with their 36 px height reaching past y = 114.
        // The two bands are stacked from the top with the navigation band's height reserved,
        // so the content band is what shrinks; and when the frame itself is shorter than the
        // chrome, only the indicator is drawn, because a truncated step indicator still
        // reads as a wizard while an overlapping button row does not.
        // The sidebar presentation replaces the numbered strip with a clickable list down the
        // left side. Selecting it zeroes the strip's height so the content well starts at the
        // frame's top edge on that side; the top-strip presentation keeps the audited layout.
        let sidebar = self.sidebar_rect(rect);
        let sidebar_width = sidebar.map_or(0, |band| band.width);
        let step_indicator_height = if sidebar.is_some() { 0 } else { Self::STEP_INDICATOR_HEIGHT };
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

        // A banner tint sits behind the step indicator. It is opt-in, so an untinted wizard
        // keeps the frame it has always painted. With the sidebar in effect there is no strip
        // to tint, and the content well's watermark colour is the band that owns that job.
        if let Some(banner) = self.banner_color {
            if sidebar.is_none() {
                context.fill_rect(
                    Rect::new(rect.x, rect.y, rect.width, step_indicator_height.min(rect.height)),
                    banner,
                );
            }
        }

        // ── Step Indicator ──────────────────────────────────────────────
        if sidebar.is_none() && !self.steps.is_empty() {
            let total_steps = self.steps.len();
            let circle_radius = 12u32;
            let spacing = (rect.width / total_steps as u32).min(120u32);
            let total_width = total_steps as u32 * spacing;
            let start_x = rect.x + (rect.width.saturating_sub(total_width) / 2) as i32;

            // One face per role for the whole strip: `role_font` re-derives the font (cloning
            // its family string) and this loop used to rebuild the number face every step and
            // the title face once or twice per step.
            let number_font = role_font(style, 11.0).with_bold(true);
            let title_font_plain = role_font(style, 9.0);
            let title_font_active = role_font(style, 9.0).with_bold(true);
            // One reusable buffer for the step numbers, cleared per step.
            let mut number_scratch = String::new();

            for i in 0..total_steps {
                let cx = start_x + (i as u32 * spacing + spacing / 2) as i32;
                let cy = rect.y + step_indicator_height as i32 / 2;

                // Determine circle state
                let is_active = i == self.current_step;
                let is_completed = self.steps[i].completed;

                let (circle_color, text_color) = if !is_enabled {
                    // The label is the ink receding from the surface it sits on; the circle is a
                    // glyph, so both are derived from the surface rather than blended toward it.
                    (ink.blend(&surface, 0.25), self.base.disabled_ink_on(ink, surface))
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
                let label = {
                    number_scratch.clear();
                    use core::fmt::Write as _;
                    let _ = write!(number_scratch, "{}", i + 1);
                    number_scratch.as_str()
                };
                let label_metrics = context.measure_text(label, &number_font);
                let label_box = Rect::new(
                    cx - circle_radius as i32,
                    cy - circle_radius as i32,
                    circle_radius * 2,
                    circle_radius * 2,
                );
                context.draw_text_fitted_cow(
                    Rect::new(
                        label_box.x,
                        (label_box.y + (label_box.height as i32 - label_metrics.height as i32) / 2)
                            .max(label_box.y),
                        label_box.width,
                        label_metrics.height.max(1),
                    ),
                    label,
                    &number_font,
                    text_color,
                    HorizontalAlignment::Center,
                );

                // Step title below circle. Bounded by the step's own cell, so a long title
                // truncates inside its column instead of over the neighbouring step; the
                // former byte-slice truncation also panicked on a multi-byte character
                // boundary, which a fitted draw cannot.
                let title_font: &crate::core::Font =
                    if is_active { &title_font_active } else { &title_font_plain };
                let title_color = if !is_enabled {
                    self.base.disabled_ink_on(ink, surface)
                } else if is_active {
                    primary
                } else {
                    ink
                };
                let title_metrics = context.measure_text("M", title_font);
                let title_y = cy + circle_radius as i32 + 2;
                context.draw_text_fitted_cow(
                    Rect::new(
                        cx - (spacing as i32 / 2),
                        title_y,
                        spacing,
                        title_metrics.height.max(1),
                    ),
                    &self.steps[i].title,
                    title_font,
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
        } else if sidebar.is_none() {
            // No steps — draw placeholder. Confined to the band above the separator, which
            // is also where the navigation row begins when the frame is short.
            let empty_font = role_font(style, 14.0);
            let empty_metrics = context.measure_text("No steps configured", &empty_font);
            context.draw_text_fitted_cow(
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
        if sidebar.is_none() && sep_y <= rect.y + rect.height as i32 {
            context.draw_line(
                Point::new(rect.x, sep_y),
                Point::new(rect.x + rect.width as i32, sep_y),
                border,
            );
        }

        // ── Sidebar ─────────────────────────────────────────────────────
        if let Some(band) = sidebar {
            context.fill_rect(band, surface.blend(&ink, 0.04));
            context.draw_line(
                Point::new(band.x + band.width as i32, band.y),
                Point::new(band.x + band.width as i32, band.y + band.height as i32),
                border,
            );
            context.push_clip(band.x, band.y, band.width, band.height);
            let row_font = role_font(style, 11.0);
            let active_font = role_font(style, 11.0).with_bold(true);
            let top = band.y + 8;
            for (index, step) in self.steps.iter().enumerate() {
                let row_y = top + index as i32 * Self::SIDEBAR_ROW_HEIGHT;
                if row_y + Self::SIDEBAR_ROW_HEIGHT > band.y + band.height as i32 {
                    break;
                }
                let row = Rect::new(band.x, row_y, band.width, Self::SIDEBAR_ROW_HEIGHT as u32);
                let is_active = index == self.current_step;
                if is_active {
                    context.fill_rect(row, primary.with_alpha_f32(0.16));
                }
                let band_inset =
                    Rect::new(row.x + 10, row.y, row.width.saturating_sub(20), row.height);
                let font = if is_active { &active_font } else { &row_font };
                let color = if !is_enabled {
                    self.base.disabled_ink_on(ink, surface)
                } else if is_active {
                    primary
                } else {
                    ink
                };
                context.draw_text_fitted_cow(
                    band_inset,
                    &step.title,
                    font,
                    color,
                    HorizontalAlignment::Left,
                );
            }
            context.pop_clip();
        }

        // ── Content Area ────────────────────────────────────────────────
        // The well is the region between the indicator and the navigation band. When the
        // frame is too short to hold both it is painted behind the buttons rather than
        // claiming a band of its own, so the two never overlap.
        let content_rect = Rect::new(
            rect.x + sidebar_width as i32,
            content_y,
            rect.width.saturating_sub(sidebar_width),
            content_height,
        );
        context.fill_rect(content_rect, content_fill);
        // A caller-supplied watermark picture is fitted inside the well, preserving its
        // aspect ratio, and painted before the heading so the text stays readable over it.
        if let Some(pixmap) = self.watermark_pixmap.as_ref() {
            let source_width = pixmap.width().max(1);
            let source_height = pixmap.height().max(1);
            let max_width = content_rect.width.max(1);
            let max_height = content_rect.height.max(1);
            let scale = (max_width as f32 / source_width as f32)
                .min(max_height as f32 / source_height as f32);
            let width = ((source_width as f32 * scale) as u32).clamp(1, max_width);
            let height = ((source_height as f32 * scale) as u32).clamp(1, max_height);
            if let Some(pixels) = super::message_box::pixmap_pixels(pixmap, width, height) {
                let x = content_rect.x + (content_rect.width.saturating_sub(width) / 2) as i32;
                let y = content_rect.y + (content_rect.height.saturating_sub(height) / 2) as i32;
                context.draw_image(x, y, width, height, &pixels);
            }
        }
        if !self.steps.is_empty() {
            // The well can be shorter than the text the current step asks for: a 120 px
            // wizard reserves 50 px for the indicator and ~40 px for the navigation row, and a
            // 16 pt title plus the "Step n of m" line does not fit in what is left. The title
            // was drawn at a fixed `+ 8` and unbounded, so it crossed the separator that marks
            // the well's bottom edge — the heading appeared to be underlined by the rule. Each
            // label is now bounded by the well and omitted when the well has no room for it.
            let well_top = content_rect.y;
            let well_bottom = content_rect.y + content_rect.height as i32;
            let info_font = role_font(style, 11.0);
            let info_text = format!("Step {} of {}", self.current_step + 1, self.steps.len());
            let info_metrics = context.measure_text(&info_text, &info_font);
            let info_h = info_metrics.height.max(1) as i32;
            // Step title in content area
            let title_font = role_font(style, 16.0).with_bold(true);
            let title_metrics =
                context.measure_text(&self.steps[self.current_step].title, &title_font);
            let title_natural = title_metrics.height.max(1) as i32;
            // The subtitle is a second, smaller line under the title. It reserves its row
            // only when it has text, so a wizard without one lays out exactly as before.
            let subtitle_font = role_font(style, 12.0);
            let subtitle_metrics = context.measure_text(&self.subtitle, &subtitle_font);
            let subtitle_h = if self.subtitle.is_empty() {
                0
            } else {
                subtitle_metrics.height.max(1) as i32 + 2
            };
            // The step title is the content a reader needs; "Step n of m" restates the numbered
            // indicator above. So the info line is what yields when the well cannot hold both,
            // rather than the title being squeezed to nothing.
            let both_fit = well_top + 6 + title_natural + subtitle_h + info_h + 4 <= well_bottom;

            let title_top = (well_top + 6).min(well_bottom);
            let title_room = if both_fit {
                (well_bottom - title_top - info_h - 4).max(0)
            } else {
                (well_bottom - title_top).max(0)
            } as u32;
            let title_h = (title_natural as u32).min(title_room);
            if title_h > 2 {
                context.draw_text_fitted_cow(
                    Rect::new(rect.x + 12, title_top, rect.width.saturating_sub(24), title_h),
                    &self.steps[self.current_step].title,
                    &title_font,
                    ink,
                    HorizontalAlignment::Left,
                );

                // Rows below the title share one cursor, so a subtitle and the optional
                // marker cannot be painted on top of each other.
                let mut next_row = title_top + title_h as i32 + 2;
                if subtitle_h > 0 {
                    let subtitle_h_px = subtitle_h - 2;
                    if next_row + subtitle_h_px
                        <= well_bottom - (if both_fit { info_h + 4 } else { 0 })
                    {
                        context.draw_text_fitted_cow(
                            Rect::new(
                                rect.x + 12,
                                next_row,
                                rect.width.saturating_sub(24),
                                subtitle_h_px as u32,
                            ),
                            &self.subtitle,
                            &subtitle_font,
                            ink.blend(&surface, 0.3),
                            HorizontalAlignment::Left,
                        );
                        next_row += subtitle_h;
                    }
                }

                // Optional label, on the row below the title (and subtitle) and only when
                // that row is inside the well.
                if self.steps[self.current_step].optional {
                    let opt_font = role_font(style, 11.0).with_italic(true);
                    let opt_metrics = context.measure_text("(Optional step)", &opt_font);
                    let opt_top = next_row;
                    let opt_h = opt_metrics.height.max(1) as i32;
                    if opt_top + opt_h <= well_bottom - (if both_fit { info_h + 4 } else { 0 }) {
                        context.draw_text_fitted_cow(
                            Rect::new(
                                rect.x + 12,
                                opt_top,
                                rect.width.saturating_sub(24),
                                opt_h as u32,
                            ),
                            "(Optional step)",
                            &opt_font,
                            ink.blend(&surface, 0.45),
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }

            // Current step info. Anchored to the well's own bottom edge, so it follows the
            // band rather than a fixed offset from the frame that the button row shares. It is
            // omitted when the well is too short to hold it without crossing the title.
            let info_y = (well_bottom - info_h).max(well_top);
            if both_fit && info_y >= title_top + title_h as i32 + 2 {
                context.draw_text_fitted_cow(
                    Rect::new(rect.x + 12, info_y, rect.width.saturating_sub(24), info_h as u32),
                    &info_text,
                    &info_font,
                    ink.blend(&surface, 0.45),
                    HorizontalAlignment::Left,
                );
            }
        } else {
            // Empty content area. Drawn inside the well's two rows so a 120 px wizard no
            // longer stacks this above the first navigation button.
            let empty_font = role_font(style, 14.0);
            let empty_metrics = context.measure_text("Add steps to begin", &empty_font);
            context.draw_text_fitted_cow(
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
        let nav_band = self.nav_area(rect);
        context.draw_line(
            Point::new(nav_band.x, nav_sep_y),
            Point::new(nav_band.x + nav_band.width as i32, nav_sep_y),
            border,
        );

        // ── Navigation Buttons ──────────────────────────────────────────
        // The row comes from **one** layout function, which the event handler also calls.
        // They used to be computed twice — here from a 72 px column and in `handle_event`
        // from a fixed 80 px one placed 20 px and 8 px elsewhere. The painted `Back`
        // therefore sat 20 px right of where it could be clicked, and the painted `Next`
        // 8 px right of its own hit box: two of the three buttons did not respond where
        // they appeared. Order alone was never the problem; **two layouts for one row**
        // was, so there is now one.
        //
        // The loop carries the options and custom actions too: a hidden action is absent
        // from `nav_slots`, so it is neither painted nor hit-tested, and the keyboard
        // focus ring is drawn by the same iteration.
        let nav_font = role_font(style, 12.0);
        let nav_text_height = context.measure_text("M", &nav_font).height.max(1);
        let label_of = |button: Rect| {
            Rect::new(
                button.x,
                button.y + (button.height as i32 - nav_text_height as i32) / 2,
                button.width,
                nav_text_height,
            )
        };
        // A disabled `Back` takes the *same* pair of functions the enabled one does: the
        // fill is derived from the button's own base colour, and the label is that fill's
        // own `contrast_color()`. Two independent dimming formulas for one button cannot be
        // kept in agreement by inspection — the old pair landed two levels apart per
        // channel, so the button rendered as an unlabelled grey block.
        let back_enabled = !self.is_first() && !self.steps.is_empty();
        let back_color = if back_enabled { button_fill } else { button_fill.blend(&surface, 0.55) };
        let back_text_color = if back_enabled {
            button_ink
        } else {
            back_color.contrast_color().blend(&surface, 0.35)
        };
        // Next/Finish is one affordance in two states, so both states read the same action
        // token; only the label changes.
        let btn_color = self.base.disabled_ink_on(primary, surface);
        let btn_text_color = if is_enabled {
            primary.contrast_color()
        } else {
            self.base.disabled_ink_on(btn_color.contrast_color(), btn_color)
        };
        let next_font = role_font(style, 12.0).with_bold(true);

        let slots = self.nav_slots(self.nav_area(rect));
        for (action, button) in slots {
            match action {
                NavAction::Next => {
                    context.fill_rounded_rect(button, 4, btn_color);
                    context.draw_text_fitted_cow(
                        label_of(button),
                        &self.label_of_action(action),
                        &next_font,
                        btn_text_color,
                        HorizontalAlignment::Center,
                    );
                }
                NavAction::Back => {
                    context.fill_rounded_rect(button, 4, back_color);
                    context.draw_rounded_rect_stroke(button, 4, border, 1);
                    context.draw_text_fitted_cow(
                        label_of(button),
                        &self.label_of_action(action),
                        &nav_font,
                        back_text_color,
                        HorizontalAlignment::Center,
                    );
                }
                NavAction::Cancel | NavAction::Help | NavAction::Custom(_) => {
                    context.fill_rounded_rect(button, 4, button_fill);
                    context.draw_rounded_rect_stroke(button, 4, border, 1);
                    context.draw_text_fitted_cow(
                        label_of(button),
                        &self.label_of_action(action),
                        &nav_font,
                        button_ink,
                        HorizontalAlignment::Center,
                    );
                }
            }
            if self.focused_button == Some(action) {
                context.draw_rounded_rect_stroke(button, 4, primary, 2);
            }
        }
    }
}

impl EventHandler for WizardDialog {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // A press arms the button under the pointer and nothing else. The action is
            // committed by the release, so one click reports one step change.
            Event::MousePress { pos, .. } => {
                // A sidebar row is a navigation target of its own: activating it jumps to that
                // step and reports the jump distinctly from `step_changed`.
                if let Some(index) = self.sidebar_row_at(*pos) {
                    self.go_to_step(index);
                    self.sidebar_page_activated.emit(index);
                    return;
                }
                let target = self.nav_button_at(*pos);
                if self.pressed_button != target {
                    self.pressed_button = target;
                    self.base.request_redraw();
                }
                // Clicking a button also moves the keyboard focus there, so the ring follows
                // the user's own last interaction rather than staying where Tab left it.
                if target.is_some() && self.focused_button != target {
                    self.focused_button = target;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos, .. } => {
                let Some(action) = self.pressed_button.take() else {
                    return;
                };
                self.base.request_redraw();
                // Releasing over a different button (or off the row) cancels the click.
                if self.nav_button_at(*pos) != Some(action) {
                    return;
                }
                self.activate(action);
            }
            // Keyboard navigation. Tab/Shift+Tab and the horizontal arrows move the focus
            // across the actions that are actually present; Enter/Space activate it, and
            // Backspace is the keyboard's Back. Escape keeps its existing cancel path.
            Event::KeyPress { key, modifiers } => {
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if *key == key_codes::ESCAPE {
                    self.do_cancel();
                } else if *key == key_codes::TAB {
                    let forward = !mods.contains(crate::shortcut::Modifiers::SHIFT);
                    self.move_focus(forward);
                } else if *key == key_codes::RIGHT {
                    self.move_focus(true);
                } else if *key == key_codes::LEFT {
                    self.move_focus(false);
                } else if *key == key_codes::ENTER || *key == key_codes::SPACE {
                    let action = self.focused_button.unwrap_or(NavAction::Next);
                    self.activate(action);
                } else if *key == key_codes::BACKSPACE {
                    if !self.is_first() && !self.steps.is_empty() {
                        self.previous();
                    }
                } else {
                    self.base.handle_event(event);
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        wiz.finished.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // Click Finish button
        let rect = wiz.geometry();
        let btn_x = rect.x + rect.width as i32 - 80 - 8;
        let btn_y = rect.y + rect.height as i32 - 36 - 6 - 6;
        let pos = Point::new(btn_x + 20, btn_y + 10);
        wiz.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press alone must not finish");
        wiz.handle_event(&Event::MouseRelease { pos, button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "one click finishes exactly once");
    }

    #[test]
    fn wizard_dialog_cancelled_signal() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        wiz.canceled.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // Click Cancel button
        let rect = wiz.geometry();
        let btn_x = rect.x + 8;
        let btn_y = rect.y + rect.height as i32 - 36 - 6 - 6;
        let pos = Point::new(btn_x + 20, btn_y + 10);
        wiz.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press alone must not cancel");
        wiz.handle_event(&Event::MouseRelease { pos, button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "one click cancels exactly once");
    }

    #[test]
    fn wizard_dialog_press_only_does_not_advance() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);

        let changes = Arc::new(AtomicUsize::new(0));
        let c = changes.clone();
        wiz.step_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        let next_rect = wiz.nav_button_rects(wiz.geometry())[2];
        let pos = Point::new(next_rect.x + 10, next_rect.y + 10);
        wiz.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(wiz.current_step(), 0, "a press must not advance the step");
        assert_eq!(changes.load(Ordering::SeqCst), 0, "a press must not emit step_changed");

        wiz.handle_event(&Event::MouseRelease { pos, button: 1 });
        assert_eq!(wiz.current_step(), 1, "the release commits the advance");
        assert_eq!(changes.load(Ordering::SeqCst), 1, "one click emits step_changed once");
    }

    #[test]
    fn wizard_dialog_escape_cancels() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        wiz.canceled.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        wiz.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "Escape must cancel the wizard");
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

    /// A step can carry a page widget, and the registry keeps the child link in step with it.
    #[test]
    fn a_step_carries_a_page_widget() {
        use crate::object::Object;
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        let first = wiz.add_page("One", false);
        let second = wiz.add_page("Two", false);
        assert_eq!((first, second), (0, 1));
        assert_eq!(wiz.page(0), None);

        let page = Object::new("PageOne").id();
        wiz.set_page(0, Some(page));
        assert_eq!(wiz.page(0), Some(page));
        assert!(wiz.children().contains(&page), "a bound page is linked as a child");

        // Replacing the page unlinks the old one rather than leaving it attached.
        let replacement = Object::new("PageOneB").id();
        wiz.set_page(0, Some(replacement));
        assert_eq!(wiz.page(0), Some(replacement));
        assert!(!wiz.children().contains(&page));
        assert!(wiz.children().contains(&replacement));

        // Removing a step removes its page binding too.
        let removed = wiz.remove_page(0).expect("the step exists");
        assert_eq!(removed.title, "One");
        assert_eq!(wiz.page(0), None, "the second step shifted into index 0 with no page");
        assert!(!wiz.children().contains(&replacement));
    }

    /// Named fields are a registry the host can resolve after the flow finishes.
    #[test]
    fn fields_are_registered_by_name() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        assert!(wiz.register_field("name", 11, "text"));
        assert!(wiz.register_field("port", 12, "value"));
        assert!(!wiz.register_field("", 13, "text"), "an unnamed field cannot be looked up");

        assert_eq!(wiz.fields().len(), 2);
        let field = wiz.field("name").expect("registered");
        assert_eq!(field.widget, 11);
        assert_eq!(field.property, "text");

        // Registering the same name replaces rather than duplicating.
        assert!(wiz.register_field("name", 21, "label_text"));
        assert_eq!(wiz.fields().len(), 2);
        assert_eq!(wiz.field("name").expect("replaced").widget, 21);

        assert!(wiz.remove_field("port"));
        assert!(!wiz.remove_field("port"));
        assert!(wiz.field("port").is_none());
    }

    /// Options change the row that is painted and hit-tested, not just a flag.
    #[test]
    fn options_change_the_navigation_row() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Only", false);
        assert_eq!(wiz.nav_slots(wiz.geometry()).len(), 3);

        wiz.set_option(WizardOption::NoCancelButton, true);
        assert!(wiz.has_option(WizardOption::NoCancelButton));
        assert_eq!(wiz.nav_slots(wiz.geometry()).len(), 2);

        wiz.set_option(WizardOption::HelpButton, true);
        let slots = wiz.nav_slots(wiz.geometry());
        assert_eq!(slots.len(), 3, "Help replaces the removed Cancel in the row's count");
        assert!(slots.iter().any(|(action, _)| *action == NavAction::Help));

        let help_rect = slots
            .iter()
            .find(|(action, _)| *action == NavAction::Help)
            .map(|(_, rect)| *rect)
            .expect("a Help slot");
        let fired = Arc::new(AtomicUsize::new(0));
        wiz.help_requested.connect({
            let sink = Arc::clone(&fired);
            move || {
                sink.fetch_add(1, Ordering::SeqCst);
            }
        });
        let pos = Point::new(help_rect.x + 4, help_rect.y + 4);
        wiz.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        wiz.handle_event(&Event::MouseRelease { pos, button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "Help must emit help_requested");
    }

    /// Custom actions emit their index; the wizard takes no other action for them.
    #[test]
    fn custom_buttons_emit_their_index() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("One", false);
        let index = wiz.add_custom_button("Export");
        assert_eq!(index, 0);
        assert_eq!(wiz.custom_buttons().len(), 1);
        assert_eq!(wiz.custom_buttons()[0], "Export");

        let custom_rect = wiz
            .nav_slots(wiz.geometry())
            .into_iter()
            .find(|(action, _)| *action == NavAction::Custom(0))
            .map(|(_, rect)| rect)
            .expect("a custom slot");

        let clicked = Arc::new(AtomicUsize::new(0));
        wiz.custom_button_clicked.connect({
            let sink = Arc::clone(&clicked);
            move |value: Arc<usize>| {
                sink.store(*value, Ordering::SeqCst);
            }
        });
        let pos = Point::new(custom_rect.x + 4, custom_rect.y + 4);
        wiz.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        wiz.handle_event(&Event::MouseRelease { pos, button: 1 });
        assert_eq!(clicked.load(Ordering::SeqCst), 0, "the clicked custom index is reported");
        assert_eq!(wiz.step_count(), 1, "a custom action does not navigate");
    }

    /// Tab/arrows move the focus, Enter activates it, and Backspace is the keyboard's Back.
    #[test]
    fn keyboard_navigation_moves_focus_and_activates() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("One", false);
        wiz.add_step("Two", false);
        let changes = Arc::new(AtomicUsize::new(0));
        wiz.step_changed.connect({
            let sink = Arc::clone(&changes);
            move |_| {
                sink.fetch_add(1, Ordering::SeqCst);
            }
        });

        // No focus yet: Enter is the default action, i.e. Next.
        wiz.handle_event(&Event::key_press(key_codes::ENTER, 0));
        assert_eq!(wiz.current_step(), 1, "Enter with no focus runs the default action");
        assert_eq!(changes.load(Ordering::SeqCst), 1);

        // Backspace is Back, without needing focus.
        wiz.handle_event(&Event::key_press(key_codes::BACKSPACE, 0));
        assert_eq!(wiz.current_step(), 0, "Backspace goes back");

        // Tab focuses the first action; two more forwards land on Back.
        wiz.handle_event(&Event::key_press(key_codes::TAB, 0));
        assert_eq!(wiz.focused_button, Some(NavAction::Cancel));
        wiz.handle_event(&Event::key_press(key_codes::TAB, 0));
        wiz.handle_event(&Event::key_press(key_codes::TAB, 0));
        assert_eq!(wiz.focused_button, Some(NavAction::Next));

        // Shift+Tab walks the other way.
        let shift = u32::from(crate::shortcut::Modifiers::SHIFT.bits());
        wiz.handle_event(&Event::key_press(key_codes::TAB, shift));
        assert_eq!(wiz.focused_button, Some(NavAction::Back));

        // Arrow keys move focus too; Enter then activates the focused action.
        wiz.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(wiz.focused_button, Some(NavAction::Next));
        wiz.handle_event(&Event::key_press(key_codes::ENTER, 0));
        assert_eq!(wiz.current_step(), 1, "Enter activates the focused Next");
        assert!(wiz.focused_button.is_some(), "the focus ring survives the activation");
    }

    /// The subtitle is painted under the step title.
    #[test]
    fn the_subtitle_reaches_the_pixels() {
        let mut plain = WizardDialog::new(Rect::new(0, 0, 400, 300));
        plain.add_step("Configure", false);
        let plain_svg = crate::widget::svg::render_to_svg(&mut plain);

        plain.set_subtitle("Choose how the wizard behaves");
        assert_eq!(plain.subtitle(), "Choose how the wizard behaves");
        let decorated = crate::widget::svg::render_to_svg(&mut plain);
        assert_ne!(plain_svg, decorated, "the subtitle must reach the pixels");

        plain.set_subtitle("");
        assert_eq!(plain_svg, crate::widget::svg::render_to_svg(&mut plain));
    }

    #[test]
    fn button_text_overrides_win_and_empty_restores_the_localised_default() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("A", false);
        wiz.add_step("B", false);
        assert_eq!(wiz.button_text(NavAction::Next), "Next");
        let default_svg = crate::widget::svg::render_to_svg(&mut wiz);
        wiz.set_button_text(NavAction::Next, "Continue");
        assert_eq!(wiz.button_text(NavAction::Next), "Continue");
        assert_ne!(
            default_svg,
            crate::widget::svg::render_to_svg(&mut wiz),
            "the override must reach the pixels"
        );
        wiz.set_button_text(NavAction::Next, "");
        assert_eq!(wiz.button_text(NavAction::Next), "Next");
    }

    #[test]
    fn the_sidebar_lists_steps_and_reports_page_activation() {
        use std::sync::Mutex;

        let mut wiz = WizardDialog::new(Rect::new(0, 0, 480, 320));
        wiz.add_step("One", false);
        wiz.add_step("Two", false);
        assert_eq!(wiz.wizard_style(), WizardStyle::TopStrip);
        let strip = crate::widget::svg::render_to_svg(&mut wiz);
        wiz.set_sidebar_visible(true);
        assert!(wiz.is_sidebar_visible());
        assert_eq!(wiz.wizard_style(), WizardStyle::Sidebar);

        let band = wiz.sidebar_rect(wiz.geometry()).expect("a sidebar has a rectangle");
        let sidebar = crate::widget::svg::render_to_svg(&mut wiz);
        assert_ne!(strip, sidebar, "the sidebar presentation reaches the pixels");

        let activated = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&activated);
        wiz.sidebar_page_activated.connect(move |index| {
            sink.lock().expect("lock").push(*index);
        });
        // The second row starts one row below the first.
        let row_y = band.y + 8 + WizardDialog::SIDEBAR_ROW_HEIGHT + 2;
        wiz.handle_event(&Event::mouse_press(band.x + 4, row_y, 1));
        assert_eq!(wiz.current_step(), 1);
        assert_eq!(*activated.lock().expect("lock"), vec![1]);
    }

    #[test]
    fn a_watermark_pixmap_reaches_the_content_well() {
        let mut wiz = WizardDialog::new(Rect::new(0, 0, 400, 300));
        wiz.add_step("Configure", false);
        let plain = crate::widget::svg::render_to_svg(&mut wiz);
        let image = crate::image::Image::try_from_rgba(
            vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255],
            2,
            2,
        )
        .expect("a 2x2 RGBA image is valid");
        wiz.set_watermark_pixmap(image);
        assert!(wiz.watermark_pixmap().is_some());
        let marked = crate::widget::svg::render_to_svg(&mut wiz);
        assert_ne!(plain, marked, "the watermark must reach the pixels");
        wiz.clear_watermark_pixmap();
        assert_eq!(plain, crate::widget::svg::render_to_svg(&mut wiz));
    }
}
