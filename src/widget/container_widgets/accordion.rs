// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Accordion — a vertical stack of collapsible sections.
//!
//! # Why an accordion and not a `CollapsiblePane`
//!
//! A collapsible pane owns **one** section: it is either open or closed. An accordion owns a
//! *set* of them and therefore carries the policy the single pane cannot express — whether
//! opening one closes the others (`single_expand`), which section is open, and how many are.
//! Building an accordion out of N panes would leave that policy to the host, where it would
//! be re-derived (and get out of step) at every call site.
//!
//! # Interaction
//!
//! A press on a header toggles its section and emits `section_toggled` with the section id and
//! the new open state. In single-expand mode, opening a section closes the one that was open;
//! closing the open section is allowed and leaves all sections closed.
//!
//! # Content
//!
//! A section's body is either a line of text or a real control. [`Accordion::set_section_widget`]
//! mounts a control in the body band: it is linked as this control's child in both directions and
//! given the band rectangle on every arrangement, so the tree painter draws it. A closed section's
//! control is hidden rather than merely clipped — the hit test walks geometry and visibility, not
//! the ancestor clip, so a clipped-but-visible control would still receive clicks.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Height of one section header, in logical pixels.
const HEADER_HEIGHT: u32 = 30;
/// Height of an open section's body, in logical pixels.
const BODY_HEIGHT: u32 = 48;
/// The horizontal padding inside a header.
const HEADER_PADDING: u32 = 10;
/// The side of the chevron's own box.
const CHEVRON_SIDE: u32 = 8;
/// The side of a header icon tile, and the gap between it and the title.
const HEADER_ICON_SIDE: u32 = 16;
/// The gap between a header icon and the title that follows it.
const HEADER_ICON_GAP: u32 = 4;

/// One section of an [`Accordion`].
#[derive(Debug, Clone, PartialEq)]
pub struct AccordionSection {
    id: String,
    title: String,
    body: String,
    expanded: bool,
    /// An icon drawn in the header ahead of the title, if one was assigned.
    icon: Option<IconSource>,
    /// A real control mounted in the section's body band, if any.
    ///
    /// `None` keeps the text body. When set, the control is a child of the accordion and is given
    /// the body band on every arrangement; the body text is then not drawn, because the control is
    /// what fills the band.
    body_widget: Option<ObjectId>,
}

impl AccordionSection {
    /// The section's stable id, which is what the toggle event carries.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The title drawn in the header.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The text drawn in the body while the section is open.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The icon drawn in the header, if one was assigned.
    pub fn icon(&self) -> Option<IconSource> {
        self.icon
    }

    /// The control mounted in the section body, if any.
    pub fn body_widget(&self) -> Option<ObjectId> {
        self.body_widget
    }

    /// Whether the section is open.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
}

/// Accordion — a vertical stack of collapsible sections.
pub struct Accordion {
    base: BaseWidget,
    sections: Vec<AccordionSection>,
    single_expand: bool,
    /// Per-section body height; a section with no entry uses [`BODY_HEIGHT`].
    body_heights: HashMap<String, u32>,
    /// Per-section open fraction while an animation is in flight.
    open_progress: HashMap<String, f32>,
    /// Whether toggles animate; off by default so the historical pixels are unchanged.
    animated: bool,
    /// Header index carrying the keyboard focus.
    focused_header: Option<usize>,
    /// Emitted when a section opens or closes, with `(section_id, new_expanded_state)`.
    ///
    /// The state is carried rather than left for the subscriber to query: a subscriber that only
    /// receives the id has to call back into the control to learn *what* happened, and the answer
    /// it reads may already be the next toggle's.
    pub section_toggled: Signal1<(String, bool)>,
    /// Cached row stack (C-P12).
    ///
    /// The stack is a closed function of the section list, each section's open fraction, the
    /// body heights and the control's geometry, so recomputing it for `draw`, the press test
    /// and `sync_body_widgets` was three allocations of the same answer per frame. The key is a
    /// revision counter bumped by exactly those mutators.
    row_cache: RowCache,
    /// Bumped whenever anything the row stack depends on changes.
    row_revision: Cell<u64>,
}

/// The cached row stack and the revision it was built under.
type RowCache = RefCell<Option<(u64, Rc<Vec<(usize, Rect, bool)>>)>>;

impl Accordion {
    /// Creates an accordion with no sections, in single-expand mode.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Accordion, geometry, "Accordion"),
            sections: Vec::new(),
            single_expand: true,
            body_heights: HashMap::new(),
            open_progress: HashMap::new(),
            animated: false,
            focused_header: None,
            section_toggled: Signal1::new(),
            row_cache: RefCell::new(None),
            row_revision: Cell::new(0),
        }
    }

    /// The sections, in display order.
    pub fn sections(&self) -> &[AccordionSection] {
        &self.sections
    }

    /// Appends a closed section. A duplicate id is refused, because the id is the identity the
    /// toggle event and the accessors address.
    pub fn add_section(&mut self, id: &str, title: &str, body: &str) -> bool {
        if self.sections.iter().any(|section| section.id == id) {
            return false;
        }
        self.sections.push(AccordionSection {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            expanded: false,
            icon: None,
            body_widget: None,
        });
        self.sync_body_widgets();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Mounts `widget` (or clears it, with `None`) as the body control of section `id`.
    ///
    /// Returns `false` for an id no section owns. A replacement unlinks and hides the previous
    /// control — a detached control keeps its own visibility flag, so leaving it shown would let it
    /// paint over the body that replaced it.
    pub fn set_section_widget(&mut self, id: &str, widget: Option<ObjectId>) -> bool {
        let Some(section) = self.sections.iter_mut().find(|section| section.id == id) else {
            return false;
        };
        let previous = section.body_widget;
        if previous == widget {
            return true;
        }
        if let Some(previous) = previous {
            self.base.remove_child_linked(previous);
            crate::set_widget_visible(previous, false);
        }
        if let Some(section) = self.sections.iter_mut().find(|section| section.id == id) {
            section.body_widget = widget;
        }
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            crate::set_widget_visible(widget_id, true);
        }
        self.sync_body_widgets();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The control mounted as the body of section `id`, if any.
    pub fn section_widget(&self, id: &str) -> Option<ObjectId> {
        self.sections
            .iter()
            .find(|section| section.id == id)
            .and_then(|section| section.body_widget)
    }

    /// Replaces a section's title. Returns whether a section with that id exists.
    ///
    /// The section stack is laid out from the header height and the titles are measured by the
    /// painter, so a change repaints and asks for a layout rather than only storing the string.
    pub fn set_section_title(&mut self, id: &str, title: &str) -> bool {
        let Some(section) = self.sections.iter_mut().find(|section| section.id == id) else {
            return false;
        };
        if section.title == title {
            return true;
        }
        section.title = title.to_string();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Replaces a section's body text. Returns whether a section with that id exists.
    ///
    /// A section with a mounted body control keeps the text (it is the fallback a caller reads
    /// back) but does not draw it: the control fills the band.
    pub fn set_section_body(&mut self, id: &str, body: &str) -> bool {
        let Some(section) = self.sections.iter_mut().find(|section| section.id == id) else {
            return false;
        };
        if section.body == body {
            return true;
        }
        section.body = body.to_string();
        self.base.request_redraw();
        true
    }

    /// Replaces a section's header icon (`None` clears it). Returns whether the id exists.
    pub fn set_section_icon(&mut self, id: &str, icon: Option<IconSource>) -> bool {
        let Some(section) = self.sections.iter_mut().find(|section| section.id == id) else {
            return false;
        };
        if section.icon == icon {
            return true;
        }
        section.icon = icon;
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Moves the section at `from` to `to`, shifting the sections in between.
    ///
    /// Returns `false` when either index is out of range or the move is a no-op. The keyboard
    /// focus follows the section it was on, and the body controls are re-placed, because the stack
    /// order is what their bands are derived from.
    pub fn move_section(&mut self, from: usize, to: usize) -> bool {
        if from >= self.sections.len() || to >= self.sections.len() || from == to {
            return false;
        }
        let section = self.sections.remove(from);
        self.sections.insert(to, section);
        self.focused_header = self.focused_header.map(|focused| {
            if focused == from {
                to
            } else if from < focused && focused <= to {
                focused - 1
            } else if to <= focused && focused < from {
                focused + 1
            } else {
                focused
            }
        });
        self.sync_body_widgets();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Removes the section owning `id`. Returns whether a section was removed.
    pub fn remove_section(&mut self, id: &str) -> bool {
        let before = self.sections.len();
        let removed_widget = self
            .sections
            .iter()
            .find(|section| section.id == id)
            .and_then(|section| section.body_widget);
        self.sections.retain(|section| section.id != id);
        if let Some(widget_id) = removed_widget {
            self.base.remove_child_linked(widget_id);
            crate::set_widget_visible(widget_id, false);
        }
        self.body_heights.remove(id);
        self.open_progress.remove(id);
        let removed = self.sections.len() != before;
        if removed {
            self.focused_header = self.focused_header.filter(|index| *index < self.sections.len());
            self.sync_body_widgets();
            self.base.request_layout();
            self.base.request_redraw();
        }
        removed
    }

    /// Whether at most one section may be open at a time.
    pub fn single_expand(&self) -> bool {
        self.single_expand
    }

    /// Sets whether at most one section may be open at a time.
    ///
    /// Switching to single-expand with several sections open keeps the **first** open one and
    /// closes the rest, so the policy is true after the write rather than only for the next
    /// toggle.
    pub fn set_single_expand(&mut self, single_expand: bool) {
        if self.single_expand == single_expand {
            return;
        }
        self.single_expand = single_expand;
        if single_expand {
            // The first open section in display order stays open; every later one closes, so the
            // policy is true after the write rather than only for the next toggle.
            let mut keep = true;
            for section in &mut self.sections {
                if section.expanded {
                    section.expanded = keep;
                    keep = false;
                }
            }
        }
        // Collapsing sections closes their bands, so the controls mounted in them have to be hidden
        // with the bands they no longer have.
        self.sync_body_widgets();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the section owning `id` is open. `None` for an id no section owns.
    pub fn is_expanded(&self, id: &str) -> Option<bool> {
        self.sections.iter().find(|section| section.id == id).map(|section| section.expanded)
    }

    /// The ids of the open sections, in display order.
    pub fn expanded_ids(&self) -> Vec<&str> {
        self.sections.iter().filter(|s| s.expanded).map(|s| s.id.as_str()).collect()
    }

    /// Opens or closes a section. Returns whether the state changed.
    ///
    /// The toggle event is emitted only when the state actually changed, so a host that mirrors
    /// this control cannot be told twice about one press.
    pub fn set_expanded(&mut self, id: &str, expanded: bool) -> bool {
        if self.is_expanded(id) != Some(!expanded) {
            // Either the id is unknown, or it is already in the requested state.
            return false;
        }
        if expanded && self.single_expand {
            for section in &mut self.sections {
                if section.id != id {
                    section.expanded = false;
                }
            }
        }
        if let Some(section) = self.sections.iter_mut().find(|section| section.id == id) {
            section.expanded = expanded;
        }
        if self.animated {
            self.open_progress.entry(id.to_string()).or_insert(if expanded { 0.0 } else { 1.0 });
        } else {
            self.open_progress.remove(id);
        }
        self.base.request_layout();
        self.base.request_redraw();
        self.sync_body_widgets();
        self.section_toggled.emit((id.to_string(), expanded));
        true
    }

    /// Flips the state of the section owning `id`.
    pub fn toggle(&mut self, id: &str) -> bool {
        match self.is_expanded(id) {
            Some(expanded) => self.set_expanded(id, !expanded),
            None => false,
        }
    }

    /// The rectangle each header occupies, in display order, with the body boxes interleaved.
    ///
    /// Returned as one shared list so the press test, the painter and the body placement agree
    /// on where a header is — and so they share one derivation rather than each rebuilding it
    /// (C-P12). The value is cached under [`Self::row_revision`]; every mutator that can move a
    /// row calls [`Self::invalidate_rows`].
    fn row_rects(&self) -> Rc<Vec<(usize, Rect, bool)>> {
        let revision = self.row_revision.get();
        if let Some((cached_revision, rows)) = self.row_cache.borrow().as_ref() {
            if *cached_revision == revision {
                return Rc::clone(rows);
            }
        }
        let rows = Rc::new(self.derive_row_rects());
        *self.row_cache.borrow_mut() = Some((revision, Rc::clone(&rows)));
        rows
    }

    /// Marks the cached row stack stale.
    fn invalidate_rows(&self) {
        self.row_revision.set(self.row_revision.get().wrapping_add(1));
    }

    /// Recomputes the row stack from the current state.
    fn derive_row_rects(&self) -> Vec<(usize, Rect, bool)> {
        let mut rows = Vec::with_capacity(self.sections.len() * 2);
        let mut y = self.geometry().y;
        let width = self.geometry().width;
        for (index, section) in self.sections.iter().enumerate() {
            rows.push((index, Rect::new(self.geometry().x, y, width, HEADER_HEIGHT), true));
            y = y.saturating_add(HEADER_HEIGHT as i32);
            let progress = self.open_progress(section.id());
            if progress > 0.0 {
                let body_height = self.body_height(section.id());
                let visible_height = (body_height as f32 * progress).round() as u32;
                if visible_height > 0 {
                    rows.push((
                        index,
                        Rect::new(self.geometry().x, y, width, visible_height),
                        false,
                    ));
                    y = y.saturating_add(visible_height as i32);
                }
            }
        }
        rows
    }

    /// The draw height of the section body before animation.
    pub fn body_height(&self, id: &str) -> u32 {
        self.body_heights.get(id).copied().unwrap_or(BODY_HEIGHT)
    }

    /// Sets a section's open body height.
    ///
    /// Returns `false` when `id` names no section. The value is the fully-open height; an
    /// animation in flight scales it by the open fraction.
    pub fn set_body_height(&mut self, id: &str, height: u32) -> bool {
        if !self.sections.iter().any(|section| section.id == id) {
            return false;
        }
        if self.body_height(id) == height {
            return true;
        }
        self.body_heights.insert(id.to_string(), height);
        self.sync_body_widgets();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// How far the section `id` is open: `0.0` closed, `1.0` fully open.
    ///
    /// Without animation enabled this is the logical state. With animation enabled it is
    /// the in-flight fraction, which is what [`Widget::tick`] advances.
    pub fn open_progress(&self, id: &str) -> f32 {
        let expanded = self.is_expanded(id).unwrap_or(false);
        if !self.animated {
            return if expanded { 1.0 } else { 0.0 };
        }
        self.open_progress.get(id).copied().unwrap_or(if expanded { 1.0 } else { 0.0 })
    }

    /// Enables or disables open/close animation. Off by default.
    ///
    /// Turning it off clears any in-flight open fraction. `open_progress` is the map
    /// `Self::row_rects` reads for the *drawn* band, so leaving an entry behind kept a section
    /// frozen at a half-open height that nothing would ever finish: `tick` stops early when
    /// `animated` is false, so the stale entry was unreachable and unrepairable.
    pub fn set_animated(&mut self, animated: bool) {
        let changed = self.animated != animated;
        self.animated = animated;
        if !animated {
            self.open_progress.clear();
        }
        if changed {
            self.sync_body_widgets();
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Whether toggles animate.
    pub fn animated(&self) -> bool {
        self.animated
    }

    /// The header index carrying keyboard focus, if any.
    pub fn focused_header(&self) -> Option<usize> {
        self.focused_header
    }

    /// Sets the header index carrying keyboard focus, clamped to the section list.
    pub fn set_focused_header(&mut self, index: Option<usize>) {
        let next = index.filter(|index| *index < self.sections.len());
        if self.focused_header == next {
            return;
        }
        self.focused_header = next;
        self.base.request_redraw();
    }

    /// Moves focus by `delta`, wrapping at both ends.
    pub fn move_focus(&mut self, delta: isize) -> bool {
        if self.sections.is_empty() {
            return false;
        }
        let count = self.sections.len() as isize;
        let current = self.focused_header.map(|index| index as isize).unwrap_or(0);
        let next = (current + delta).rem_euclid(count) as usize;
        self.set_focused_header(Some(next));
        true
    }

    /// Toggles the focused section. Returns whether a section was focused and toggled.
    pub fn toggle_focused(&mut self) -> bool {
        let Some(index) = self.focused_header else {
            return false;
        };
        let Some(section) = self.sections.get(index) else {
            return false;
        };
        let id = section.id.clone();
        self.toggle(&id)
    }

    /// Applies the current row stack to every mounted body control.
    ///
    /// # Why a body control is hidden rather than clipped
    ///
    /// Clipping is a painting rule. The hit test walks geometry and visibility, not the ancestor
    /// clip, so a closed section whose control was merely clipped would still receive clicks
    /// through the header above it. Hiding is the one state both paths read — the same rule
    /// `CollapsiblePane` and `TabWidget` apply to their own contents.
    ///
    /// # Why it is not called from `draw`
    ///
    /// The rows change only when a section, a body height, the geometry or an animation tick
    /// changes, and every one of those paths calls this. Repeating the platform writes on every
    /// frame would be work proportional to the section count per repaint for no new information.
    fn sync_body_widgets(&self) {
        // The row stack is about to be consumed for placement; make sure it reflects the state
        // that caused this call (C-P12). Matches on the revision are otherwise reused, so a
        // frame that only repaints (`draw`) pays for no recomputation at all.
        self.invalidate_rows();
        let mut bands: Vec<(usize, Rect)> = Vec::new();
        for (index, row, is_header) in self.row_rects().iter().copied() {
            if !is_header {
                bands.push((index, row));
            }
        }
        for (index, section) in self.sections.iter().enumerate() {
            let Some(widget_id) = section.body_widget else {
                continue;
            };
            match bands.iter().find(|(row_index, _)| *row_index == index) {
                Some((_, rect)) => {
                    crate::set_widget_visible(widget_id, true);
                    crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
                }
                None => crate::set_widget_visible(widget_id, false),
            }
        }
    }
}

impl Widget for Accordion {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Moves the stack and carries every mounted body control with it.
    ///
    /// The body bands are derived from the control's own rectangle, so a layout that moves the
    /// accordion has to move the controls inside its sections.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.sync_body_widgets();
    }

    /// The stacked height of every section, headers and open bodies together.
    fn size_hint(&self) -> Size {
        let mut height: u32 = 0;
        for section in &self.sections {
            height = height.saturating_add(HEADER_HEIGHT);
            if self.open_progress(section.id()) > 0.0 {
                height = height.saturating_add(self.body_height(section.id()));
            }
        }
        Size::new(240, height.max(HEADER_HEIGHT))
    }

    /// Advances any open/close animation; `true` while a section is still moving.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animated || self.open_progress.is_empty() {
            return false;
        }
        // The tempo is a theme token, not a literal: a reduced-motion preference shortens
        // every motion token, so the same section opens faster without this control knowing
        // about the preference. `180` is the no-theme fallback the preset produces.
        let duration_ms = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.motion.normal)
            .unwrap_or(180)
            .max(1) as f32;
        let step = delta_ms as f32 / duration_ms;
        let mut moving = false;
        // C-P12: the old code cloned every section id into a `targets` Vec and then ran
        // `retain` with a linear `find` over it, i.e. O(n²) work each frame. The target is
        // already a field of the section, so the update and the "still moving" decision are
        // one pass, and an entry that reached its target is collected for removal in that
        // same pass.
        let mut settled: Vec<String> = Vec::new();
        for section in &self.sections {
            let target = if section.expanded { 1.0 } else { 0.0 };
            let Some(progress) = self.open_progress.get_mut(&section.id) else {
                continue;
            };
            if (*progress - target).abs() <= f32::EPSILON {
                settled.push(section.id.clone());
                continue;
            }
            if *progress < target {
                *progress = (*progress + step).min(target);
            } else {
                *progress = (*progress - step).max(target);
            }
            if (*progress - target).abs() > f32::EPSILON {
                moving = true;
            } else {
                settled.push(section.id.clone());
            }
        }
        for id in settled {
            self.open_progress.remove(&id);
        }
        if !moving {
            self.open_progress.clear();
        }
        self.sync_body_widgets();
        self.base.request_layout();
        self.base.request_redraw();
        moving
    }

    /// Whether any section is mid-animation.
    fn is_animating(&self) -> bool {
        self.animated && !self.open_progress.is_empty()
    }

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "section_toggled" => {
                Some(EventSignalRef::mapped("section_toggled", &self.section_toggled, |value| {
                    // The payload is `(section_id, expanded)`. Carrying it as a tuple keeps both
                    // parts readable with their own kinds instead of flattening the pair into a
                    // `Debug` spelling a subscriber would have to parse.
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(value.0.clone()),
                        CapabilityValue::Bool(value.1),
                    ]))
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Accordion {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "section_count" => Ok(CapabilityValue::UInt(self.sections.len() as u64)),
            "single_expand" => Ok(CapabilityValue::Bool(self.single_expand)),
            "expanded_count" => Ok(CapabilityValue::UInt(
                self.sections.iter().filter(|s| s.expanded).count() as u64,
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Both counts are derived from the section list.
            "section_count" | "expanded_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "single_expand" => {
                self.set_single_expand(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["section_count", "single_expand", "expanded_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Accordion {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let themed = crate::style::resolved_theme_style_shared("accordion");
        let (window_background, window_foreground, accent) = {
            match crate::style::palette_shared() {
                Some(colors) => (colors.background, colors.foreground, colors.primary),
                None => (Color::WHITE, Color::BLACK, Color::PRIMARY),
            }
        };
        let surface = style
            .background_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.background_color))
            .unwrap_or(window_background);
        let ink = style
            .text_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
            .unwrap_or(window_foreground);
        let accent = themed.as_ref().and_then(|resolved| resolved.border_color).unwrap_or(accent);
        let font = style.font.clone().unwrap_or_else(|| Font::simple("Arial", 12.0));

        // The sections run as tall as their content makes them; the clip bounds them to the
        // control, so a stack longer than the control cannot paint on whatever is below it.
        ctx.push_clip(rect.x, rect.y, rect.width, rect.height);
        let header_fill = ink.blend(&surface, 0.92);
        let body_fill = ink.blend(&surface, 0.97);
        let divider = ink.blend(&surface, 0.8);

        for (index, row, is_header) in self.row_rects().iter().copied() {
            if row.y >= rect.y.saturating_add(rect.height as i32) {
                continue;
            }
            let section = &self.sections[index];
            if is_header {
                ctx.fill_rect(row, header_fill);
                if self.focused_header == Some(index) {
                    ctx.draw_rect_stroke(row, accent, 1);
                }
                ctx.draw_line(
                    crate::core::Point::new(row.x, row.y.saturating_add(row.height as i32 - 1)),
                    crate::core::Point::new(
                        row.x.saturating_add(row.width as i32 - 1),
                        row.y.saturating_add(row.height as i32 - 1),
                    ),
                    divider,
                );
                // The chevron points right when the section is closed and down when it is open —
                // the two states are told apart by shape, not only by the body appearing.
                let chevron_x = row
                    .x
                    .saturating_add(row.width as i32)
                    .saturating_sub(HEADER_PADDING as i32)
                    .saturating_sub(CHEVRON_SIDE as i32);
                let chevron_y = row.y.saturating_add(((row.height - CHEVRON_SIDE) / 2) as i32);
                let half = CHEVRON_SIDE / 2;
                let (from, first, second) = if section.expanded {
                    (
                        crate::core::Point::new(chevron_x, chevron_y.saturating_add(half as i32)),
                        crate::core::Point::new(
                            chevron_x.saturating_add(half as i32),
                            chevron_y.saturating_add(CHEVRON_SIDE as i32),
                        ),
                        crate::core::Point::new(
                            chevron_x.saturating_add(CHEVRON_SIDE as i32),
                            chevron_y.saturating_add(half as i32),
                        ),
                    )
                } else {
                    (
                        crate::core::Point::new(chevron_x.saturating_add(half as i32), chevron_y),
                        crate::core::Point::new(
                            chevron_x.saturating_add(CHEVRON_SIDE as i32),
                            chevron_y.saturating_add(half as i32),
                        ),
                        crate::core::Point::new(
                            chevron_x.saturating_add(half as i32),
                            chevron_y.saturating_add(CHEVRON_SIDE as i32),
                        ),
                    )
                };
                ctx.draw_line_stroke(from, first, accent, 1);
                ctx.draw_line_stroke(first, second, accent, 1);

                let title_band = Rect::new(
                    row.x.saturating_add(HEADER_PADDING as i32),
                    row.y,
                    row.width.saturating_sub(
                        HEADER_PADDING.saturating_mul(2).saturating_add(CHEVRON_SIDE),
                    ),
                    row.height,
                );
                // A section icon reserves a leading tile and the title starts after it, so the
                // caption and the glyph cannot overlap. The room charged is the same constant the
                // band is narrowed by, so what is painted and what is reserved agree.
                let title_band = match section.icon {
                    None => title_band,
                    Some(source) => {
                        let side = HEADER_ICON_SIDE.min(title_band.height.max(1));
                        let icon_rect = Rect::new(
                            title_band.x,
                            title_band.y + (title_band.height.saturating_sub(side) / 2) as i32,
                            side,
                            side,
                        );
                        if let Err(error) =
                            crate::widget::paint_icon(ctx, source.into(), icon_rect, ink)
                        {
                            log::warn!("accordion section icon could not be painted: {error:?}");
                        }
                        let shift = (side + HEADER_ICON_GAP) as i32;
                        Rect::new(
                            title_band.x.saturating_add(shift),
                            title_band.y,
                            title_band.width.saturating_sub(shift as u32),
                            title_band.height,
                        )
                    }
                };
                // An open section's title is emphasised; a closed one is ordinary ink.
                let title_ink = if section.expanded { accent } else { ink };
                ctx.draw_text_line_cow(
                    title_band,
                    section.title(),
                    &font,
                    title_ink,
                    HorizontalAlignment::Left,
                );
            } else {
                ctx.fill_rect(row, body_fill);
                // A section with a mounted control paints only the band: the control is what fills
                // it, and a caption drawn underneath would be covered by the child anyway.
                if section.body_widget.is_some() {
                    continue;
                }
                let body_band = Rect::new(
                    row.x.saturating_add(HEADER_PADDING as i32),
                    row.y,
                    row.width.saturating_sub(HEADER_PADDING.saturating_mul(2)),
                    row.height,
                );
                ctx.draw_text_line_cow(
                    body_band,
                    section.body(),
                    &font,
                    ink.with_alpha_f32(0.8),
                    HorizontalAlignment::Left,
                );
            }
        }
        ctx.pop_clip();

        if self.sections.is_empty() {
            // An empty accordion still shows where it is; an outline is chrome, not content.
            ctx.draw_rect(rect, ink.blend(&surface, 0.8));
        }
    }
}

impl EventHandler for Accordion {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            // A disabled control is inert: it consumes no input and emits nothing the user
            // cannot reach (`check_enabled_is_honoured.sh`).
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            let hit = self
                .row_rects()
                .iter()
                .find(|(_, row, is_header)| *is_header && row.contains(*pos))
                .map(|(index, _, _)| self.sections[*index].id.clone());
            if let Some(id) = hit {
                if let Some(index) = self.sections.iter().position(|section| section.id == id) {
                    self.focused_header = Some(index);
                }
                self.toggle(&id);
            }
        }
        if let Event::KeyPress { key, .. } = event {
            if self.base.focus_reason().is_none() {
                return;
            }
            match *key {
                crate::event::key_codes::UP => {
                    let _ = self.move_focus(-1);
                }
                crate::event::key_codes::DOWN => {
                    let _ = self.move_focus(1);
                }
                crate::event::key_codes::SPACE | crate::event::key_codes::ENTER => {
                    let _ = self.toggle_focused();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    fn accordion() -> Accordion {
        let mut accordion = Accordion::new(Rect::new(0, 0, 240, 200));
        accordion.add_section("a", "Alpha", "first body");
        accordion.add_section("b", "Beta", "second body");
        accordion
    }

    #[test]
    fn sections_start_closed() {
        let accordion = accordion();
        assert_eq!(accordion.sections().len(), 2);
        assert_eq!(accordion.expanded_ids(), Vec::<&str>::new());
        assert_eq!(accordion.kind(), WidgetKind::Accordion);
    }

    #[test]
    fn a_duplicate_id_is_refused() {
        let mut accordion = accordion();
        assert!(!accordion.add_section("a", "Again", ""));
        assert_eq!(accordion.sections().len(), 2);
    }

    #[test]
    fn single_expand_keeps_one_section_open() {
        let mut accordion = accordion();
        assert!(accordion.set_expanded("a", true));
        assert!(accordion.set_expanded("b", true));
        assert_eq!(accordion.expanded_ids(), vec!["b"]);
    }

    #[test]
    fn multi_expand_keeps_both_open() {
        let mut accordion = accordion();
        accordion.set_single_expand(false);
        accordion.set_expanded("a", true);
        accordion.set_expanded("b", true);
        assert_eq!(accordion.expanded_ids(), vec!["a", "b"]);
    }

    #[test]
    fn switching_to_single_expand_keeps_the_first_open_section() {
        let mut accordion = accordion();
        accordion.set_single_expand(false);
        accordion.set_expanded("a", true);
        accordion.set_expanded("b", true);
        accordion.set_single_expand(true);
        assert_eq!(accordion.expanded_ids(), vec!["a"]);
    }

    #[test]
    fn a_toggle_is_emitted_only_when_the_state_changes() {
        let mut accordion = accordion();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        accordion.section_toggled.connect_ref({
            let seen = std::sync::Arc::clone(&seen);
            // The payload carries the section id *and* the new state, so a subscriber does not have
            // to call back into the control to learn what the toggle did.
            move |toggle: &(String, bool)| seen.lock().expect("lock").push(toggle.clone())
        });
        accordion.set_expanded("a", true);
        assert!(!accordion.set_expanded("a", true), "a no-op must report no change");
        accordion.set_expanded("a", false);
        assert_eq!(
            seen.lock().expect("lock").as_slice(),
            [("a".to_string(), true), ("a".to_string(), false)]
        );
    }

    #[test]
    fn pressing_a_header_toggles_its_section() {
        let mut accordion = accordion();
        // The first header occupies y 0..30 at full width.
        accordion.handle_event(&Event::MousePress {
            pos: Point::new(20, 10),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(accordion.expanded_ids(), vec!["a"]);
        // Pressing the body must not toggle: it is not a header.
        accordion.handle_event(&Event::MousePress {
            pos: Point::new(20, HEADER_HEIGHT as i32 + 10),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(accordion.expanded_ids(), vec!["a"]);
        // The second header moved down by the first section's opened body.
        accordion.handle_event(&Event::MousePress {
            pos: Point::new(20, (HEADER_HEIGHT + BODY_HEIGHT) as i32 + 10),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(accordion.expanded_ids(), vec!["b"]);
    }

    #[test]
    fn the_counts_are_derived_and_read_only() {
        let mut accordion = accordion();
        accordion.set_expanded("a", true);
        assert_eq!(accordion.get("section_count"), Ok(CapabilityValue::UInt(2)));
        assert_eq!(accordion.get("expanded_count"), Ok(CapabilityValue::UInt(1)));
        assert_eq!(
            accordion.set("section_count", CapabilityValue::UInt(7)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn an_accordion_paints_every_title() {
        let mut accordion = accordion();
        accordion.set_expanded("a", true);
        let svg = crate::widget::svg::render_to_svg(&mut accordion);
        // Two titles and one open body.
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 3, "{svg}");
    }

    #[test]
    fn removing_a_section_shrinks_the_stack() {
        let mut accordion = accordion();
        accordion.set_expanded("a", true);
        let tall = accordion.size_hint().height;
        assert!(accordion.remove_section("a"));
        assert!(accordion.size_hint().height < tall);
        assert!(!accordion.remove_section("a"));
    }

    /// A body height is per section, an animation advances over frames, and keyboard focus
    /// can move and toggle a section.
    #[test]
    fn body_height_animation_and_keyboard_are_wired() {
        let mut accordion = accordion();
        assert!(accordion.set_body_height("a", 80));
        assert!(!accordion.set_body_height("missing", 80));
        accordion.set_expanded("a", true);
        assert_eq!(accordion.size_hint().height, 30 + 80 + 30);

        accordion.set_animated(true);
        assert_eq!(accordion.open_progress("b"), 0.0);
        accordion.set_expanded("b", true);
        assert_eq!(accordion.open_progress("b"), 0.0);
        assert!(accordion.tick(90));
        assert!(accordion.is_animating());
        assert!(accordion.open_progress("b") > 0.0);
        accordion.tick(1000);
        assert_eq!(accordion.open_progress("b"), 1.0);
        assert!(!accordion.is_animating());

        accordion.set_focused_header(Some(1));
        assert!(accordion.move_focus(1), "focus wraps to the first header");
        assert_eq!(accordion.focused_header(), Some(0));
        assert!(accordion.toggle_focused(), "the focused section toggles");
        assert_eq!(accordion.is_expanded("a"), Some(true));
    }
}
