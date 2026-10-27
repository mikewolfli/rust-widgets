// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TagGroup` — the companion container for a row of `Tag` controls.
//!
//! A `Tag` is deliberately one label: it does not own the row it sits in. A host that shows
//! "the filters currently applied" therefore had to lay the tags out itself and forward every
//! close and selection by hand. This container owns that composition and reuses the tag's own
//! painting and event paths — it stores real `Tag` children rather than copying their drawing
//! code, so a tag and a tag in a group cannot render differently.
//!
//! # Where this lives
//!
//! The companion inventory (`src/widget/companions.rs`) is owned by another layer of this
//! change set, so the type is defined here and re-exported from there once the module line is
//! added. Until then it is reachable as
//! `crate::widget::display_widgets::tag_group::TagGroup`.

use crate::compat::{Arc, RefCell, String, Vec};
use crate::core::{Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string_list, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::display_widgets::tag::Tag;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How a group lets its tags be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagSelectionMode {
    /// Tags are labels only: a click is reported but no tag stays selected.
    #[default]
    None,
    /// At most one tag is selected at a time.
    Single,
    /// Every tag toggles independently.
    Multiple,
}

impl TagSelectionMode {
    /// Every mode, in publication order.
    pub const ALL: [Self; 3] = [Self::None, Self::Single, Self::Multiple];

    /// The token this mode is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.token() == token)
    }
}

/// A wrapping row of [`Tag`] children with aggregated close and selection signals.
pub struct TagGroup {
    base: BaseWidget,
    tags: Vec<Tag>,
    closable: bool,
    selection_mode: TagSelectionMode,
    wrap: bool,
    spacing: u32,
    /// The selected indices, recomputed from the child tags after any interaction.
    selected: Vec<usize>,
    /// Emitted with the removed label when any child tag is closed.
    pub tag_closed: Signal1<String>,
    /// Emitted whenever the selected set changes; read [`Self::selected_indices`].
    pub selection_changed: GenericSignal,
    /// The placed child rectangles and the revision set that produced them.
    ///
    /// `draw` and the event path both lay the children out, and the layout measured every
    /// label every frame. The cache is invalidated by the group's box, the wrap/spacing
    /// settings, and each child's own measurement revision (which covers text, face,
    /// closable and icon changes).
    layout_cache: RefCell<Option<LayoutCache>>,
    /// How many times the layout has actually been rebuilt. Test-only evidence that a still
    /// group reuses its cached layout.
    #[cfg(test)]
    layout_epoch: core::cell::Cell<u64>,
}

/// The last layout and the inputs that produced it.
struct LayoutCache {
    rect: Rect,
    wrap: bool,
    spacing: u32,
    revisions: Vec<u64>,
}

impl TagGroup {
    /// Creates an empty wrapping group.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chip, geometry, "TagGroup"),
            tags: Vec::new(),
            closable: false,
            selection_mode: TagSelectionMode::None,
            wrap: true,
            spacing: dimensions::TOOLBAR_SPACING,
            selected: Vec::new(),
            tag_closed: Signal1::new(),
            selection_changed: GenericSignal::new(),
            layout_cache: RefCell::new(None),
            #[cfg(test)]
            layout_epoch: Cell::new(0),
        }
    }

    /// The labels, in group order.
    pub fn tags(&self) -> Vec<String> {
        self.tags.iter().map(|tag| tag.text().to_string()).collect()
    }

    /// Returns the label at `index`, if it exists.
    pub fn tag(&self, index: usize) -> Option<&str> {
        self.tags.get(index).map(Tag::text)
    }

    /// The number of tags in the group.
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// Whether the group holds no tags.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// The selected indices, in ascending order.
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected
    }

    /// Whether the close glyph is enabled on every tag.
    pub fn closable(&self) -> bool {
        self.closable
    }

    /// Enables or disables the close glyph on every tag, including future ones.
    pub fn set_closable(&mut self, closable: bool) {
        if self.closable == closable {
            return;
        }
        self.closable = closable;
        for tag in &mut self.tags {
            tag.set_closable(closable);
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The group's selection model.
    pub fn selection_mode(&self) -> TagSelectionMode {
        self.selection_mode
    }

    /// Sets the group's selection model and normalises the current selection to it.
    pub fn set_selection_mode(&mut self, mode: TagSelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        match mode {
            TagSelectionMode::None => {
                for tag in &mut self.tags {
                    tag.set_selected(false);
                }
            }
            TagSelectionMode::Single => {
                let keep = self.selected.first().copied();
                for (index, tag) in self.tags.iter_mut().enumerate() {
                    tag.set_selected(Some(index) == keep);
                }
            }
            TagSelectionMode::Multiple => {}
        }
        self.refresh_selection();
        self.base.request_redraw();
    }

    /// Whether tags wrap onto a new row when the group's width is exhausted.
    pub fn wrap(&self) -> bool {
        self.wrap
    }

    /// Enables or disables wrapping.
    pub fn set_wrap(&mut self, wrap: bool) {
        if self.wrap == wrap {
            return;
        }
        self.wrap = wrap;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The gap between adjacent tags, in logical pixels.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Sets the gap between adjacent tags.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.spacing == spacing {
            return;
        }
        self.spacing = spacing;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends a tag and returns its index.
    pub fn add_tag(&mut self, text: impl Into<String>) -> usize {
        let text = text.into();
        let mut tag = Tag::new(&text, Rect::new(0, 0, 1, 1));
        tag.set_closable(self.closable);
        // Aggregate the child's own close signal rather than re-deriving the close hit test:
        // the tag owns the gesture, so the group cannot disagree about what was closed.
        let group_closed = self.tag_closed.clone();
        tag.closed.connect(move |value: Arc<String>| {
            group_closed.emit((*value).clone());
        });
        self.tags.push(tag);
        let index = self.tags.len() - 1;
        self.layout_tags();
        self.base.request_layout();
        self.base.request_redraw();
        index
    }

    /// Replaces the whole list of labels.
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags.clear();
        self.selected.clear();
        for text in tags {
            let _ = self.add_tag(text);
        }
        self.refresh_selection();
        self.base.request_redraw();
    }

    /// Removes the tag at `index`, returning whether it existed.
    pub fn remove_tag(&mut self, index: usize) -> bool {
        if index >= self.tags.len() {
            return false;
        }
        self.tags.remove(index);
        self.layout_tags();
        self.refresh_selection();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Lays the child tags out inside the group's rectangle.
    ///
    /// Each tag's own `size_hint` is the cell, so a group never invents a size the tag would not
    /// declare. Wrapping is row-major and the last row's height is not reserved.
    fn layout_tags(&mut self) {
        // Ask each child for its cell first: `Tag::size_hint` revalidates its own cache and
        // bumps its revision if a measurement input changed, so this read is what makes a
        // face or text change visible to the group's own cache.
        let revisions: Vec<u64> = self
            .tags
            .iter()
            .map(|tag| {
                let _ = tag.size_hint();
                tag.size_hint_revision()
            })
            .collect();
        let rect = self.geometry();
        let fresh = self.layout_cache.borrow().as_ref().is_some_and(|cache| {
            cache.rect == rect
                && cache.wrap == self.wrap
                && cache.spacing == self.spacing
                && cache.revisions == revisions
        });
        if fresh {
            return;
        }
        let spacing = self.spacing as i32;
        let mut x = rect.x;
        let mut y = rect.y;
        let mut row_height = 0u32;
        for tag in &mut self.tags {
            let hint = tag.size_hint();
            let width = hint.width.max(1);
            let height = hint.height.max(1);
            if self.wrap && x > rect.x && x + width as i32 > rect.x + rect.width as i32 {
                x = rect.x;
                y += row_height as i32 + spacing;
                row_height = 0;
            }
            tag.set_geometry(Rect::new(x, y, width, height));
            x += width as i32 + spacing;
            row_height = row_height.max(height);
        }
        *self.layout_cache.borrow_mut() =
            Some(LayoutCache { rect, wrap: self.wrap, spacing: self.spacing, revisions });
        #[cfg(test)]
        self.layout_epoch.set(self.layout_epoch.get().wrapping_add(1));
    }

    /// Reads the children's selection and emits once when it changed.
    fn refresh_selection(&mut self) {
        let mut selected = Vec::new();
        for (index, tag) in self.tags.iter().enumerate() {
            if tag.is_selected() {
                selected.push(index);
            }
        }
        if selected != self.selected {
            self.selected = selected;
            self.selection_changed.emit();
            self.base.request_redraw();
        }
    }
}

impl Widget for TagGroup {
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
        // A single row: the group cannot know the width a wrapping host will give it, so the
        // hint describes the natural extent of its children.
        let spacing = self.spacing;
        let mut width = 0u32;
        let mut height = 0u32;
        for tag in &self.tags {
            let hint = tag.size_hint();
            width = width.saturating_add(hint.width).saturating_add(spacing);
            height = height.max(hint.height);
        }
        if !self.tags.is_empty() {
            width = width.saturating_sub(spacing);
        }
        Size::new(width.max(80), height.max(1))
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "tag_closed" => Some(EventSignalRef::mapped("tag_closed", &self.tag_closed, |v| {
                CapabilityValue::String(v.clone())
            })),
            "selection_changed" => {
                Some(EventSignalRef::unit("selection_changed", &self.selection_changed))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for TagGroup {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tags" => Ok(CapabilityValue::Tuple(
                self.tags().into_iter().map(CapabilityValue::String).collect(),
            )),
            "closable" => Ok(CapabilityValue::Bool(self.closable())),
            "selection_mode" => {
                Ok(CapabilityValue::String(self.selection_mode().token().to_string()))
            }
            "wrap" => Ok(CapabilityValue::Bool(self.wrap())),
            "spacing" => Ok(CapabilityValue::UInt(u64::from(self.spacing()))),
            "selected_indices" => Ok(CapabilityValue::Tuple(
                self.selected_indices()
                    .iter()
                    .map(|index| CapabilityValue::UInt(*index as u64))
                    .collect(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "tags" => {
                self.set_tags(expect_string_list(value)?);
                Ok(())
            }
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            "selection_mode" => {
                let token = crate::widget::capability::coercion::expect_string(value)?;
                let mode = TagSelectionMode::from_token(&token)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_selection_mode(mode);
                Ok(())
            }
            "wrap" => {
                self.set_wrap(expect_bool(value)?);
                Ok(())
            }
            "spacing" => {
                self.set_spacing(expect_u32(value)?);
                Ok(())
            }
            // Derived from the children.
            "selected_indices" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tags",
            "closable",
            "selection_mode",
            "wrap",
            "spacing",
            "selected_indices",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for TagGroup {
    fn draw(&mut self, context: &mut RenderContext) {
        // Keep the children placed even when the host moved or resized the group between events.
        self.layout_tags();
        for tag in &mut self.tags {
            tag.draw(context);
        }
    }
}

impl EventHandler for TagGroup {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        self.layout_tags();
        let hit = if let Event::MousePress { pos, .. } = event {
            self.tags.iter().position(|tag| tag.geometry().contains_point(*pos))
        } else {
            None
        };
        // The child tags own the gestures: forwarding the event is what reuses their close-glyph
        // hit test, their hover state and their selection toggle.
        for tag in &mut self.tags {
            tag.handle_event(event);
        }
        if let Event::MousePress { .. } = event {
            match self.selection_mode {
                TagSelectionMode::None => {
                    for tag in &mut self.tags {
                        tag.set_selected(false);
                    }
                }
                TagSelectionMode::Single => {
                    // Keep exactly the tag the pointer hit, so the model is radio-like rather
                    // than the tag's own toggle.
                    for (index, tag) in self.tags.iter_mut().enumerate() {
                        tag.set_selected(Some(index) == hit);
                    }
                }
                TagSelectionMode::Multiple => {}
            }
        }
        self.refresh_selection();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn a_group_adds_removes_and_reports_its_tags() {
        let mut group = TagGroup::new(Rect::new(0, 0, 300, 40));
        assert_eq!(group.add_tag("alpha"), 0);
        assert_eq!(group.add_tag("beta"), 1);
        assert_eq!(group.len(), 2);
        assert_eq!(group.tag(1), Some("beta"));
        assert!(group.remove_tag(0));
        assert_eq!(group.tags(), vec!["beta".to_string()]);
        assert!(!group.remove_tag(9));
    }

    #[test]
    fn tags_are_laid_out_in_a_row_and_wrap() {
        let mut group = TagGroup::new(Rect::new(0, 0, 60, 80));
        group.set_wrap(false);
        group.add_tag("aaaaaaaa");
        group.add_tag("bbbbbbbb");
        group.set_wrap(true);
        group.layout_tags();
        let first = group.tags[0].geometry();
        let second = group.tags[1].geometry();
        assert_eq!(first.y, 0);
        assert!(second.y > first.y, "the second tag wraps to a new row");

        group.set_wrap(false);
        group.layout_tags();
        assert_eq!(group.tags[1].geometry().y, group.tags[0].geometry().y);
    }

    /// Single selection is radio-like; multiple toggles independently; none keeps nothing.
    #[test]
    fn the_selection_mode_shapes_the_group_selection() {
        let mut group = TagGroup::new(Rect::new(0, 0, 400, 40));
        group.add_tag("a");
        group.add_tag("b");
        group.set_selection_mode(TagSelectionMode::Multiple);

        let a = group.tags[0].geometry();
        let b = group.tags[1].geometry();
        group.handle_event(&Event::MousePress {
            pos: Point::new(a.x + 2, a.y + 2),
            button: 1,
            modifiers: 0,
        });
        group.handle_event(&Event::MousePress {
            pos: Point::new(b.x + 2, b.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(group.selected_indices(), &[0, 1]);

        group.set_selection_mode(TagSelectionMode::Single);
        assert_eq!(group.selected_indices(), &[0], "single keeps the leading selection");

        group.set_selection_mode(TagSelectionMode::None);
        assert!(group.selected_indices().is_empty());
    }

    /// A child's close signal is aggregated by the group.
    #[test]
    fn closing_a_tag_is_reported_by_the_group() {
        let mut group = TagGroup::new(Rect::new(0, 0, 300, 40));
        group.add_tag("draft");
        group.set_closable(true);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        group.tag_closed.connect(move |value: Arc<String>| {
            sink.lock().unwrap().push((*value).clone());
        });

        // The tag's own close gesture emits through its own signal; the group aggregated that
        // signal when the child was added.
        group.tags[0].closed.emit("draft".to_string());
        assert_eq!(*seen.lock().unwrap(), vec!["draft".to_string()]);
    }

    /// A still group reuses its layout; each input change rebuilds it exactly once.
    #[test]
    fn the_layout_is_cached_until_an_input_changes() {
        let mut group = TagGroup::new(Rect::new(0, 0, 300, 40));
        group.add_tag("alpha");
        group.add_tag("beta");
        group.layout_tags();
        let first = group.layout_epoch.get();
        let placed = group.tags[1].geometry();

        group.layout_tags();
        assert_eq!(group.layout_epoch.get(), first, "a still group does not re-measure");
        assert_eq!(group.tags[1].geometry(), placed);

        group.set_spacing(4);
        group.layout_tags();
        assert_eq!(group.layout_epoch.get(), first + 1, "a spacing change rebuilds once");

        group.tags[0].set_text("a much longer label".to_string());
        group.layout_tags();
        assert_eq!(group.layout_epoch.get(), first + 2, "a child measurement change rebuilds");
    }

    #[test]
    fn the_property_route_round_trips_the_group() {
        let mut group = TagGroup::new(Rect::new(0, 0, 300, 40));
        group
            .set(
                "tags",
                CapabilityValue::Tuple(vec![
                    CapabilityValue::String("one".into()),
                    CapabilityValue::String("two".into()),
                ]),
            )
            .expect("writable");
        assert_eq!(group.len(), 2);
        group.set("closable", CapabilityValue::Bool(true)).expect("writable");
        group.set("selection_mode", CapabilityValue::String("single".into())).expect("writable");
        group.set("wrap", CapabilityValue::Bool(false)).expect("writable");
        group.set("spacing", CapabilityValue::UInt(8)).expect("writable");
        assert!(group.closable());
        assert_eq!(group.selection_mode(), TagSelectionMode::Single);
        assert!(!group.wrap());
        assert_eq!(group.spacing(), 8);
        assert_eq!(
            group.get("tags").unwrap(),
            CapabilityValue::Tuple(vec![
                CapabilityValue::String("one".into()),
                CapabilityValue::String("two".into())
            ])
        );
        assert_eq!(
            group.set("selected_indices", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }
}
