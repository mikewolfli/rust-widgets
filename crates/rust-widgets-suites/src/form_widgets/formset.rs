// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `FormSet` — an ordered set of forms with one active page and aggregate validity.
//!
//! # What a form set is for
//!
//! A long form is usually split into pages: contact details, shipping, payment. `FormSet` owns the
//! order, the page titles, the completed flags and the aggregate validity; each page's fields and
//! rules stay in its own [`Form`](crate::form_widgets::Form). The set highlights the current page,
//! shows only that page's form, and offers previous/next/submit navigation.
//!
//! # Layout
//!
//! `header_rect()` is a band split into one equal cell per page (`header_item_rect(index)`), drawn
//! either as tabs or as a numbered step strip. `content_rect()` is the remaining area (below the
//! header and above the navigation band), and the current page's form is written into it. Every
//! rectangle is public, so a host can align a companion with the strip or the footer.
//!
//! # Aggregate validity
//!
//! A page's validity is its form's `valid` property; a page with no form never blocks the set. The
//! set never re-implements a form's rules — it asks the form, which is the only owner of them. The
//! `validate_all` / `reset_all` commands drive each form through *its* command channel, so the two
//! controls cannot disagree about what "validate" means.
//!
//! # Events and wiring
//!
//! Every published name resolves to a real signal in [`Widget::event_signal_dyn`]:
//! `current_changed`, `page_added`, `page_removed`, `completed_changed`, `validity_changed`,
//! `dirty_changed`, `submitted`, `submit_failed` and `reset`.

use rust_widgets::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{GenericSignal, Signal1};
use rust_widgets::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

use super::model::FormPage;

/// How the page header is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormSetHeaderStyle {
    /// Equal-width tabs; the current one is filled.
    Tabs,
    /// Numbered circles joined by a rule; completed pages are ticked.
    Steps,
}

impl FormSetHeaderStyle {
    /// The published token naming this style.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Tabs => "tabs",
            Self::Steps => "steps",
        }
    }

    /// Parses a published token, or `None` for an unknown one.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "tabs" => Some(Self::Tabs),
            "steps" => Some(Self::Steps),
            _ => None,
        }
    }
}

const STEP_RADIUS: u32 = 9;
const BUTTON_MIN_W: u32 = 72;
const BUTTON_H: u32 = 28;

/// A set of forms with one active page.
pub struct FormSet {
    base: BaseWidget,
    pages: Vec<FormPage>,
    current: usize,
    show_header: bool,
    header_height: u32,
    header_style: FormSetHeaderStyle,
    show_navigation: bool,
    navigation_height: u32,
    content_padding: u32,
    submit_label: String,
    previous_label: String,
    next_label: String,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    last_valid: bool,
    last_dirty: bool,
    /// Emitted with the new zero-based current index.
    pub current_changed: Signal1<usize>,
    /// Emitted with the key of an added page.
    pub page_added: Signal1<String>,
    /// Emitted with the key of a removed page.
    pub page_removed: Signal1<String>,
    /// Emitted with `(key, completed)` when a page's completed flag changes.
    pub completed_changed: Signal1<(String, bool)>,
    /// Emitted with the aggregate validity when it changes.
    pub validity_changed: Signal1<bool>,
    /// Emitted with the aggregate dirty flag when it changes.
    pub dirty_changed: Signal1<bool>,
    /// Emitted after a successful submit.
    pub submitted: GenericSignal,
    /// Emitted with the number of pages still invalid when a submit was refused.
    pub submit_failed: Signal1<usize>,
    /// Emitted after `reset_all` reset every page's form.
    pub reset_requested: GenericSignal,
}

impl FormSet {
    /// Creates an empty form set.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FormSet, geometry, "FormSet"),
            pages: Vec::new(),
            current: 0,
            show_header: true,
            header_height: 32,
            header_style: FormSetHeaderStyle::Tabs,
            show_navigation: true,
            navigation_height: 36,
            content_padding: 12,
            submit_label: "Submit".to_string(),
            previous_label: "Back".to_string(),
            next_label: "Next".to_string(),
            registry: None,
            last_valid: true,
            last_dirty: false,
            current_changed: Signal1::new(),
            page_added: Signal1::new(),
            page_removed: Signal1::new(),
            completed_changed: Signal1::new(),
            validity_changed: Signal1::new(),
            dirty_changed: Signal1::new(),
            submitted: GenericSignal::new(),
            submit_failed: Signal1::new(),
            reset_requested: GenericSignal::new(),
        }
    }

    // ── Configuration ────────────────────────────────────────────────────────

    /// Whether the page header is drawn.
    pub fn show_header(&self) -> bool {
        self.show_header
    }

    /// Shows or hides the page header.
    pub fn set_show_header(&mut self, show: bool) {
        self.show_header = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The header band's height.
    pub fn header_height(&self) -> u32 {
        self.header_height
    }

    /// Sets the header band's height (at least 20).
    pub fn set_header_height(&mut self, height: u32) {
        self.header_height = height.max(20);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The header rendering style.
    pub fn header_style(&self) -> FormSetHeaderStyle {
        self.header_style
    }

    /// Chooses the header rendering style.
    pub fn set_header_style(&mut self, style: FormSetHeaderStyle) {
        self.header_style = style;
        self.base.request_redraw();
    }

    /// Whether the previous/next/submit band is drawn and clickable.
    pub fn show_navigation(&self) -> bool {
        self.show_navigation
    }

    /// Shows or hides the navigation band.
    pub fn set_show_navigation(&mut self, show: bool) {
        self.show_navigation = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The navigation band's height.
    pub fn navigation_height(&self) -> u32 {
        self.navigation_height
    }

    /// Sets the navigation band's height (at least `BUTTON_H + 8`).
    pub fn set_navigation_height(&mut self, height: u32) {
        self.navigation_height = height.max(BUTTON_H + 8);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The inset between the control's edge and its content.
    pub fn content_padding(&self) -> u32 {
        self.content_padding
    }

    /// Sets the content inset.
    pub fn set_content_padding(&mut self, padding: u32) {
        self.content_padding = padding;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The submit button's caption.
    pub fn submit_label(&self) -> &str {
        &self.submit_label
    }

    /// Sets the submit caption.
    pub fn set_submit_label(&mut self, label: impl Into<String>) {
        self.submit_label = label.into();
        self.base.request_redraw();
    }

    /// The previous button's caption.
    pub fn previous_label(&self) -> &str {
        &self.previous_label
    }

    /// Sets the previous caption.
    pub fn set_previous_label(&mut self, label: impl Into<String>) {
        self.previous_label = label.into();
        self.base.request_redraw();
    }

    /// The next button's caption.
    pub fn next_label(&self) -> &str {
        &self.next_label
    }

    /// Sets the next caption.
    pub fn set_next_label(&mut self, label: impl Into<String>) {
        self.next_label = label.into();
        self.base.request_redraw();
    }

    // ── Page model ───────────────────────────────────────────────────────────

    /// The pages in order.
    pub fn pages(&self) -> &[FormPage] {
        &self.pages
    }

    /// The page with `key`.
    pub fn page(&self, key: &str) -> Option<&FormPage> {
        self.pages.iter().find(|page| page.key == key)
    }

    /// The page with `key`, mutably.
    ///
    /// A direct edit through this reference does not know whether it changed the layout; call
    /// [`Self::refresh`] afterwards if it did.
    pub fn page_mut(&mut self, key: &str) -> Option<&mut FormPage> {
        self.pages.iter_mut().find(|page| page.key == key)
    }

    /// How many pages the set holds.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Every page key, in order.
    pub fn page_keys(&self) -> Vec<String> {
        self.pages.iter().map(|page| page.key.clone()).collect()
    }

    /// Adds a page, linking its form into this set.
    ///
    /// Returns `false` for a duplicate key. The first page added becomes current and is shown;
    /// every other form starts hidden.
    pub fn add_page(&mut self, page: FormPage) -> bool {
        if self.page(&page.key).is_some() {
            return false;
        }
        if let Some(form) = page.form {
            self.base.add_child_linked(form);
        }
        let key = page.key.clone();
        self.pages.push(page);
        let was_empty = self.pages.len() == 1;
        if was_empty {
            self.current = 0;
        }
        self.apply_page_visibility();
        self.base.request_layout();
        self.base.request_redraw();
        self.page_added.emit(key);
        if was_empty {
            // The set went from "no current page" to the first one, which is a real change a
            // host may be waiting for (to move focus or to reveal the page).
            self.current_changed.emit(0);
        }
        self.publish_validity(self.is_valid());
        self.publish_dirty();
        true
    }

    /// Adds a page bound to `form` in one call.
    pub fn add_page_form(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
        form: ObjectId,
    ) -> bool {
        self.add_page(FormPage::new(key, title).with_form(form))
    }

    /// Removes the page with `key`, returning it.
    ///
    /// The form itself is not destroyed or hidden by the removal — the set never owned it — but the
    /// page that referenced it is gone, so the next layout drops the link.
    pub fn remove_page(&mut self, key: &str) -> Option<FormPage> {
        let index = self.pages.iter().position(|page| page.key == key)?;
        let before = self.current;
        let page = self.pages.remove(index);
        if let Some(form) = page.form {
            self.base.remove_child_linked(form);
        }
        if index < self.current {
            self.current -= 1;
        }
        if self.current >= self.pages.len() {
            self.current = self.pages.len().saturating_sub(1);
        }
        self.apply_page_visibility();
        self.base.request_layout();
        self.base.request_redraw();
        self.page_removed.emit(key.to_string());
        if self.current != before {
            self.current_changed.emit(self.current);
        }
        self.publish_validity(self.is_valid());
        self.publish_dirty();
        Some(page)
    }

    /// Removes every page.
    pub fn clear_pages(&mut self) {
        let before = self.current;
        while let Some(page) = self.pages.pop() {
            if let Some(form) = page.form {
                self.base.remove_child_linked(form);
            }
            self.page_removed.emit(page.key);
        }
        self.current = 0;
        self.base.request_layout();
        self.base.request_redraw();
        if before != 0 {
            self.current_changed.emit(0);
        }
        self.publish_validity(true);
        self.publish_dirty();
    }

    /// The zero-based index of the active page.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Activates the page at `index`.
    ///
    /// Returns `false` when the index is out of range or the page is disabled; an out-of-range
    /// request is refused rather than clamped, because silently activating a different page than
    /// the caller named is the kind of surprise this project's rules forbid.
    pub fn set_current_index(&mut self, index: usize) -> bool {
        let Some(page) = self.pages.get(index) else {
            return false;
        };
        if !page.enabled {
            return false;
        }
        if index == self.current {
            return true;
        }
        self.current = index;
        self.apply_page_visibility();
        self.base.request_redraw();
        self.current_changed.emit(index);
        true
    }

    /// The active page, if any.
    pub fn current_page(&self) -> Option<&FormPage> {
        self.pages.get(self.current)
    }

    /// Activates the page whose key is `key`.
    ///
    /// Returns `false` for an unknown key or a disabled page: the index arithmetic stays in
    /// [`Self::set_current_index`], so the two entry points share one refusal rule.
    pub fn goto_page(&mut self, key: &str) -> bool {
        let Some(index) = self.pages.iter().position(|page| page.key == key) else {
            return false;
        };
        self.set_current_index(index)
    }

    /// Moves the page `key` to `index`, keeping the active page active.
    ///
    /// Returns `false` for an unknown key or an out-of-range index: a move silently clamped to
    /// the end would put the page somewhere the caller did not name. The active page is tracked
    /// by identity, so the move does not change which form is visible.
    pub fn move_page(&mut self, key: &str, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        let Some(from) = self.pages.iter().position(|page| page.key == key) else {
            return false;
        };
        if from == index {
            return true;
        }
        let active = self.current_key().map(str::to_string);
        let page = self.pages.remove(from);
        self.pages.insert(index, page);
        if let Some(key) = active {
            if let Some(position) = self.pages.iter().position(|page| page.key == key) {
                self.current = position;
            }
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The active page's key, if any.
    pub fn current_key(&self) -> Option<&str> {
        self.current_page().map(FormPage::key)
    }

    /// The active page's form id, if any.
    pub fn current_form_id(&self) -> Option<ObjectId> {
        self.current_page().and_then(FormPage::form_id)
    }

    /// Moves to the next enabled page, wrapping at the end.
    pub fn next_page(&mut self) -> bool {
        self.advance(1)
    }

    /// Moves to the previous enabled page, wrapping at the start.
    pub fn previous_page(&mut self) -> bool {
        self.advance(-1)
    }

    fn advance(&mut self, delta: i32) -> bool {
        if self.pages.is_empty() {
            return false;
        }
        let count = self.pages.len() as i32;
        let mut index = self.current as i32;
        for _ in 0..count {
            index = (index + delta).rem_euclid(count);
            if self.pages[index as usize].enabled {
                return self.set_current_index(index as usize);
            }
        }
        false
    }

    /// Sets one page's completed flag, emitting `completed_changed` on a real edge.
    pub fn set_page_completed(&mut self, key: &str, completed: bool) -> bool {
        let Some(page) = self.page_mut(key) else {
            return false;
        };
        if page.completed == completed {
            return true;
        }
        page.completed = completed;
        self.base.request_redraw();
        self.completed_changed.emit((key.to_string(), completed));
        true
    }

    /// Whether one page is marked complete.
    pub fn is_page_completed(&self, key: &str) -> bool {
        self.page(key).is_some_and(FormPage::is_completed)
    }

    /// Marks the active page complete.
    pub fn mark_current_completed(&mut self) {
        if let Some(key) = self.current_key().map(str::to_string) {
            let _ = self.set_page_completed(&key, true);
        }
    }

    /// How many pages are marked complete.
    pub fn completed_count(&self) -> usize {
        self.pages.iter().filter(|page| page.is_completed()).count()
    }

    /// Requests a layout pass and a repaint.
    pub fn refresh(&self) {
        self.base.request_layout();
        self.base.request_redraw();
    }

    // ── Validity and actions ─────────────────────────────────────────────────

    /// Whether every page reports valid.
    ///
    /// A page with no form has nothing to validate and never blocks the set. A bound control that
    /// does not publish `valid` is treated the same way: it has no validity to report, so the set
    /// does not invent one.
    pub fn is_valid(&self) -> bool {
        self.pages.iter().all(|page| match page.form {
            Some(form) => read_form_flag(form, "valid").unwrap_or(true),
            None => true,
        })
    }

    /// How many bound forms currently report at least one error.
    pub fn error_count(&self) -> usize {
        self.pages
            .iter()
            .filter_map(|page| page.form)
            .filter(|form| !read_form_flag(*form, "valid").unwrap_or(true))
            .count()
    }

    /// Whether any page's form reports uncommitted changes.
    ///
    /// Aggregated from each form's own `dirty` property the same way [`Self::is_valid`]
    /// aggregates `valid`; a page with no form has nothing to modify and never makes the set
    /// dirty.
    pub fn dirty(&self) -> bool {
        self.pages.iter().any(|page| match page.form {
            Some(form) => read_form_flag(form, "dirty").unwrap_or(false),
            None => false,
        })
    }

    /// Runs every bound form's own `validate` command and publishes the aggregate validity.
    pub fn validate_all(&mut self) -> bool {
        for form in self.pages.iter().filter_map(FormPage::form_id) {
            run_form_command(form, "validate");
        }
        let valid = self.is_valid();
        self.base.request_redraw();
        self.publish_validity(valid);
        valid
    }

    /// Submits: validates every page, then emits `submitted` or `submit_failed`.
    pub fn submit(&mut self) -> bool {
        if self.validate_all() {
            self.submitted.emit();
            true
        } else {
            self.submit_failed.emit(self.error_count());
            false
        }
    }

    /// Runs every bound form's own `reset` command and emits `reset`.
    pub fn reset_all(&mut self) {
        for form in self.pages.iter().filter_map(FormPage::form_id) {
            run_form_command(form, "reset");
        }
        self.base.request_redraw();
        self.reset_requested.emit();
        self.publish_validity(self.is_valid());
        self.publish_dirty();
    }

    // ── Layout ───────────────────────────────────────────────────────────────

    /// The page-header band, or an empty rectangle when the header is hidden.
    pub fn header_rect(&self) -> Rect {
        let geometry = self.geometry();
        let padding = self.content_padding as i32;
        let width = geometry.width.saturating_sub(self.content_padding.saturating_mul(2));
        if !self.show_header {
            return Rect::new(geometry.x + padding, geometry.y + padding, width, 0);
        }
        Rect::new(geometry.x + padding, geometry.y + padding, width, self.header_height)
    }

    /// One page's cell in the header band.
    pub fn header_item_rect(&self, index: usize) -> Option<Rect> {
        let count = self.pages.len();
        if index >= count || !self.show_header {
            return None;
        }
        let header = self.header_rect();
        let count = count as u64;
        let index = index as u64;
        let x0 = header.x + ((header.width as u64 * index) / count) as i32;
        let x1 = header.x + ((header.width as u64 * (index + 1)) / count) as i32;
        Some(Rect::new(x0, header.y, (x1 - x0).max(0) as u32, header.height))
    }

    /// The area the active page's form is written into.
    pub fn content_rect(&self) -> Rect {
        let geometry = self.geometry();
        let padding = self.content_padding as i32;
        let header = self.header_rect();
        let top = if self.show_header {
            header.y + self.header_height as i32 + self.content_padding as i32
        } else {
            geometry.y + padding
        };
        let bottom = geometry.bottom()
            - padding
            - if self.show_navigation { self.navigation_height as i32 } else { 0 };
        let width = geometry.width.saturating_sub(self.content_padding.saturating_mul(2));
        Rect::new(geometry.x + padding, top, width, (bottom - top).max(0) as u32)
    }

    /// The previous button's box.
    pub fn previous_rect(&self) -> Rect {
        self.navigation_button(0)
    }

    /// The next button's box.
    pub fn next_rect(&self) -> Rect {
        self.navigation_button(1)
    }

    /// The submit button's box.
    pub fn submit_rect(&self) -> Rect {
        self.navigation_button(2)
    }

    fn navigation_button(&self, slot: usize) -> Rect {
        let geometry = self.geometry();
        let padding = self.content_padding as i32;
        let y = geometry.bottom() - padding - BUTTON_H as i32;
        // Slot 0 (`previous`) is the leftmost of the three, slot 2 (`submit`) the rightmost.
        let from_right = 2 - slot.min(2);
        let x = geometry.right() - padding - ((from_right as i32) + 1) * (BUTTON_MIN_W as i32 + 8);
        Rect::new(x, y, BUTTON_MIN_W, BUTTON_H)
    }

    /// Writes the active page's form into the content area and shows only that form.
    fn apply_page_visibility(&self) {
        let content = self.content_rect();
        for (index, page) in self.pages.iter().enumerate() {
            let Some(form) = page.form else {
                continue;
            };
            let active = index == self.current;
            rust_widgets::set_widget_visible(form, active);
            // A disabled set disables the forms it hosts, the same propagation `visible` gets
            // (FM-06). The router can dispatch to a child form directly, so the container's own
            // flag alone would leave it interactive.
            rust_widgets::set_widget_enabled(form, self.base.is_enabled());
            if active {
                rust_widgets::set_widget_geometry(
                    form,
                    content.x,
                    content.y,
                    content.width,
                    content.height,
                );
            }
        }
    }

    /// Binds the shared forwarding table used to deliver events to the active form.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }

    /// The bound forwarding table, if any.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }

    fn publish_validity(&mut self, valid: bool) {
        if self.last_valid != valid {
            self.last_valid = valid;
            self.validity_changed.emit(valid);
        }
    }

    fn publish_dirty(&mut self) {
        let dirty = self.dirty();
        if self.last_dirty != dirty {
            self.last_dirty = dirty;
            self.dirty_changed.emit(dirty);
        }
    }

    fn draw_button(&self, context: &mut RenderContext, rect: Rect, label: &str, ink: Color) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // Hover and press are tracked by `BaseWidget`; a disabled set paints the row faded.
        let enabled = self.base.is_enabled();
        let fill_alpha = if !enabled {
            8
        } else if self.base.is_pressed() {
            30
        } else if self.base.is_hovered() {
            24
        } else {
            18
        };
        let stroke_alpha = if enabled { 90 } else { 40 };
        let ink = if enabled { ink } else { ink.with_alpha(120) };
        context.fill_rounded_rect(rect, 4, ink.with_alpha(fill_alpha));
        context.draw_rounded_rect_stroke(rect, 4, ink.with_alpha(stroke_alpha), 1);
        let font = role_font(self.base.style(), 12.0);
        context.draw_text_fitted_cow(rect, label, &font, ink, HorizontalAlignment::Center);
    }
}

/// Reads a boolean property from a mounted control.
fn read_form_flag(form: ObjectId, name: &str) -> Option<bool> {
    match rust_widgets::read_widget_property_by_id(form, name) {
        Ok(CapabilityValue::Bool(value)) => Some(value),
        _ => None,
    }
}

/// Runs one of a control's published commands, if it has one.
fn run_form_command(form: ObjectId, command: &str) {
    let _ = rust_widgets::widget::runtime::with_widget_mut(form, |widget| {
        if let Some(properties) = widget.properties_dyn_mut() {
            let _ = properties.command(command);
        }
    });
}

impl Widget for FormSet {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    /// The name a screen reader announces: the active page's title.
    ///
    /// A page title is what a sighted user reads as the name of the step being edited, so it is
    /// the honest answer; the tooltip fallback covers a set with no pages.
    fn accessible_name(&self) -> String {
        match self.current_page() {
            Some(page) => page.title().to_string(),
            None => self.base.tooltip().to_string(),
        }
    }

    /// A set with no pages has nothing to walk or submit, so it stays out of the tab order —
    /// the same conditional the splitter uses.
    fn is_focusable(&self) -> bool {
        !self.pages.is_empty()
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.apply_page_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    fn size_hint(&self) -> Size {
        let width = self.geometry().width.max(360);
        let height = self.content_padding.saturating_mul(2)
            + if self.show_header { self.header_height + self.content_padding } else { 0 }
            + 160
            + if self.show_navigation { self.navigation_height } else { 0 };
        Size::new(width, height)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves every published event name to the signal that emits it.
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "page_added" => Some(EventSignalRef::mapped("page_added", &self.page_added, |value| {
                CapabilityValue::String(value.clone())
            })),
            "page_removed" => {
                Some(EventSignalRef::mapped("page_removed", &self.page_removed, |value| {
                    CapabilityValue::String(value.clone())
                }))
            }
            "completed_changed" => Some(EventSignalRef::mapped(
                "completed_changed",
                &self.completed_changed,
                |(key, completed)| {
                    CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
                        CapabilityValue::String(key.clone()),
                        CapabilityValue::Bool(*completed),
                    ]))
                },
            )),
            "validity_changed" => {
                Some(EventSignalRef::mapped("validity_changed", &self.validity_changed, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            "dirty_changed" => {
                Some(EventSignalRef::mapped("dirty_changed", &self.dirty_changed, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            "submitted" => Some(EventSignalRef::unit("submitted", &self.submitted)),
            "submit_failed" => {
                Some(EventSignalRef::mapped("submit_failed", &self.submit_failed, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "reset" => Some(EventSignalRef::unit("reset", &self.reset_requested)),
            _ => None,
        }
    }
}

impl WidgetProperties for FormSet {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_index" => Ok(CapabilityValue::UInt(self.current as u64)),
            "page_count" => Ok(CapabilityValue::UInt(self.page_count() as u64)),
            "valid" => Ok(CapabilityValue::Bool(self.is_valid())),
            "dirty" => Ok(CapabilityValue::Bool(self.dirty())),
            "completed_count" => Ok(CapabilityValue::UInt(self.completed_count() as u64)),
            "show_header" => Ok(CapabilityValue::Bool(self.show_header)),
            "header_height" => Ok(CapabilityValue::UInt(self.header_height as u64)),
            "header_style" => Ok(CapabilityValue::String(self.header_style.token().to_string())),
            "show_navigation" => Ok(CapabilityValue::Bool(self.show_navigation)),
            "navigation_height" => Ok(CapabilityValue::UInt(self.navigation_height as u64)),
            "content_padding" => Ok(CapabilityValue::UInt(self.content_padding as u64)),
            "submit_label" => Ok(CapabilityValue::String(self.submit_label.clone())),
            "previous_label" => Ok(CapabilityValue::String(self.previous_label.clone())),
            "next_label" => Ok(CapabilityValue::String(self.next_label.clone())),
            "current_key" => Ok(match self.current_key() {
                Some(key) => CapabilityValue::String(key.to_string()),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                let index = usize::try_from(expect_u32(value)?)
                    .map_err(|_| CapabilityAccessError::OutOfRange)?;
                if !self.set_current_index(index) {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                Ok(())
            }
            "show_header" => {
                self.set_show_header(expect_bool(value)?);
                Ok(())
            }
            "header_height" => {
                self.set_header_height(expect_u32(value)?);
                Ok(())
            }
            "header_style" => {
                let token = expect_string(value)?;
                let style = FormSetHeaderStyle::from_token(&token)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_header_style(style);
                Ok(())
            }
            "show_navigation" => {
                self.set_show_navigation(expect_bool(value)?);
                Ok(())
            }
            "navigation_height" => {
                self.set_navigation_height(expect_u32(value)?);
                Ok(())
            }
            "content_padding" => {
                self.set_content_padding(expect_u32(value)?);
                Ok(())
            }
            "submit_label" => {
                self.set_submit_label(expect_string(value)?);
                Ok(())
            }
            "previous_label" => {
                self.set_previous_label(expect_string(value)?);
                Ok(())
            }
            "next_label" => {
                self.set_next_label(expect_string(value)?);
                Ok(())
            }
            // A disabled set disables the forms it hosts, the same propagation `visible` gets
            // (FM-06).
            "enabled" => {
                let enabled = expect_bool(value)?;
                self.base.set_enabled(enabled);
                for form in self.pages.iter().filter_map(FormPage::form_id) {
                    rust_widgets::set_widget_enabled(form, enabled);
                }
                Ok(())
            }
            // Derived from the page list and the forms' own state.
            "page_count" | "valid" | "completed_count" | "current_key" | "dirty" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "current_index",
            "page_count",
            "valid",
            "dirty",
            "completed_count",
            "show_header",
            "header_height",
            "header_style",
            "show_navigation",
            "navigation_height",
            "content_padding",
            "submit_label",
            "previous_label",
            "next_label",
            "current_key",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "next" => {
                let _ = self.next_page();
                Ok(())
            }
            "previous" => {
                let _ = self.previous_page();
                Ok(())
            }
            "submit" => {
                let _ = self.submit();
                Ok(())
            }
            "reset_all" => {
                self.reset_all();
                Ok(())
            }
            "validate_all" => {
                let _ = self.validate_all();
                Ok(())
            }
            "mark_completed" => {
                self.mark_current_completed();
                Ok(())
            }
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for FormSet {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(40, 40, 40, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(255, 255, 255, 255));
        let border = style.border_color.unwrap_or(ink.with_alpha(60));
        let radius = style.border_radius.unwrap_or(0);
        context.fill_rounded_rect(rect, radius, surface);
        context.draw_rounded_rect_stroke(rect, radius, border, 1);

        let font = role_font(style, 12.0);
        let error_ink =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Error)
                .unwrap_or_else(|| Color::rgba(190, 40, 40, 255));
        if self.show_header && !self.pages.is_empty() {
            for index in 0..self.pages.len() {
                let Some(cell) = self.header_item_rect(index) else {
                    continue;
                };
                let page = &self.pages[index];
                let active = index == self.current;
                // A page whose form reports a refusal is marked in the header: the tab title
                // alone cannot tell a user which step is blocking the submit.
                let invalid =
                    page.form.is_some_and(|form| read_form_flag(form, "valid") == Some(false));
                match self.header_style {
                    FormSetHeaderStyle::Tabs => {
                        if active {
                            context.fill_rounded_rect(cell, 4, ink.with_alpha(20));
                        }
                        context.draw_text_fitted_cow(
                            cell,
                            page.title(),
                            &font,
                            if page.enabled { ink } else { ink.with_alpha(90) },
                            HorizontalAlignment::Center,
                        );
                        if invalid {
                            context.fill_circle(
                                Point::new(cell.right() - 8, cell.y + cell.height as i32 / 2),
                                3,
                                error_ink,
                            );
                        }
                    }
                    FormSetHeaderStyle::Steps => {
                        let center_y = cell.y + cell.height as i32 / 2;
                        let center_x = cell.x + STEP_RADIUS as i32 + 4;
                        let number = if page.is_completed() {
                            "✓".to_string()
                        } else {
                            (index + 1).to_string()
                        };
                        let circle_fill = if invalid {
                            error_ink.with_alpha(60)
                        } else if active || page.is_completed() {
                            ink.with_alpha(40)
                        } else {
                            ink.with_alpha(12)
                        };
                        context.fill_rounded_rect(
                            Rect::new(
                                center_x - STEP_RADIUS as i32,
                                center_y - STEP_RADIUS as i32,
                                STEP_RADIUS * 2,
                                STEP_RADIUS * 2,
                            ),
                            STEP_RADIUS,
                            circle_fill,
                        );
                        context.draw_text_fitted_cow(
                            Rect::new(
                                center_x - STEP_RADIUS as i32,
                                center_y - STEP_RADIUS as i32,
                                STEP_RADIUS * 2,
                                STEP_RADIUS * 2,
                            ),
                            &number,
                            &font,
                            if invalid { error_ink } else { ink },
                            HorizontalAlignment::Center,
                        );
                        let title = Rect::new(
                            center_x + STEP_RADIUS as i32 + 6,
                            cell.y,
                            cell.width.saturating_sub(STEP_RADIUS * 2 + 10),
                            cell.height,
                        );
                        context.draw_text_fitted_cow(
                            title,
                            page.title(),
                            &font,
                            if page.enabled { ink } else { ink.with_alpha(90) },
                            HorizontalAlignment::Left,
                        );
                        if index + 1 < self.pages.len() {
                            let y = center_y;
                            context.draw_line(
                                Point::new(center_x + STEP_RADIUS as i32 + 2, y),
                                Point::new(cell.right(), y),
                                ink.with_alpha(30),
                            );
                        }
                    }
                }
            }
        }

        if self.show_navigation {
            self.draw_button(context, self.previous_rect(), &self.previous_label, ink);
            self.draw_button(context, self.next_rect(), &self.next_label, ink);
            self.draw_button(context, self.submit_rect(), &self.submit_label, ink);
        }

        // The active page's form is a linked child: the tree painter draws it. Re-writing its box
        // here keeps a cached layout frame in step with the current page.
        self.apply_page_visibility();
    }
}

impl EventHandler for FormSet {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.show_header {
                if let Some(index) = (0..self.pages.len()).find(|index| {
                    self.header_item_rect(*index).is_some_and(|rect| rect.contains_point(*pos))
                }) {
                    let _ = self.set_current_index(index);
                    return;
                }
            }
            if self.show_navigation {
                if self.submit_rect().contains_point(*pos) {
                    let _ = self.submit();
                    return;
                }
                if self.next_rect().contains_point(*pos) {
                    let _ = self.next_page();
                    return;
                }
                if self.previous_rect().contains_point(*pos) {
                    let _ = self.previous_page();
                    return;
                }
            }
        }
        match event {
            Event::KeyPress { key, .. } if *key == rust_widgets::event::key_codes::LEFT => {
                let _ = self.previous_page();
                return;
            }
            Event::KeyPress { key, .. } if *key == rust_widgets::event::key_codes::RIGHT => {
                let _ = self.next_page();
                return;
            }
            Event::KeyPress { key, .. } if *key == rust_widgets::event::key_codes::ENTER => {
                let _ = self.submit();
                return;
            }
            _ => {}
        }
        let Some(registry) = self.registry.as_ref() else {
            return;
        };
        if let Some(form) = self.current_form_id() {
            let _ = registry.borrow_mut().forward_event(form, event);
        }
    }
}
