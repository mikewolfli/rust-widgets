// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Border layout manager — fixed edge tracks around an elastic center.
//!
//! # Why this exists
//!
//! A workbench-shaped window (menu/tool bar on top, status bar at the bottom, sidebars,
//! and an editor in the middle) is a two-dimensional arrangement with **fixed pixel
//! tracks** on the edges and one elastic center. [`BoxLayout`](super::BoxLayout) is
//! single-axis, and [`GridLayout`](super::GridLayout) distributes columns by stretch
//! factor rather than by a fixed pixel extent, so neither can express "48px toolbar,
//! rest is the editor" without the caller computing the tracks itself.
//!
//! # Semantics
//!
//! * Each [region](BorderRegion) holds an ordered list of tracks.
//! * `North`/`South` tracks have a fixed **height**; `West`/`East` tracks have a fixed
//!   **width**; `Center` tracks share whatever remains by stretch factor.
//! * `spacing` separates tracks **within** one region. Regions themselves touch; use
//!   `margin` (or a nested layout) for padding between a band and the center.
//! * A track never escapes the parent: when the edge extents cannot all be paid, later
//!   tracks receive the remaining pixels (possibly zero) instead of being placed outside.
//! * [`LayoutContext`] scales extents, spacing and margin by the larger of the layout
//!   scale and the text scale, the same rule the box layout follows.
//!
//! # Touch targets
//!
//! Edge extents are explicit caller intent, so they are **not** grown to the device's
//! minimum touch size: a 32px toolbar that silently became 44px would no longer be the
//! layout the caller asked for. Controls that need a larger target belong in the center
//! region, or inside a nested layout that owns that policy.

use super::{Layout, LayoutContext};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};

/// A named area of a [`BorderLayout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorderRegion {
    /// Top edge; tracks have a fixed height, stacked downward.
    North,
    /// Bottom edge; tracks have a fixed height, stacked downward.
    South,
    /// Left edge; tracks have a fixed width, stacked rightward.
    West,
    /// Right edge; tracks have a fixed width, stacked rightward.
    East,
    /// Remaining area; tracks share it horizontally by stretch factor.
    Center,
}

impl BorderRegion {
    /// The stable token this region is spelled with in a declarative document.
    pub const fn token(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
            Self::Center => "center",
        }
    }

    /// The region a declarative `region` token names, if any.
    ///
    /// Accepts the capitalised spellings as well, because a document written by hand
    /// commonly uses them and they name the same area rather than a different one.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "north" | "North" => Some(Self::North),
            "south" | "South" => Some(Self::South),
            "west" | "West" => Some(Self::West),
            "east" | "East" => Some(Self::East),
            "center" | "Center" | "centre" | "Centre" => Some(Self::Center),
            _ => None,
        }
    }
}

/// One track in a region.
#[derive(Debug, Clone, Copy)]
struct BorderItem {
    widget_id: ObjectId,
    /// Fixed major-axis extent (height for `North`/`South`, width for `West`/`East`).
    /// Ignored by `Center`, whose extent is whatever remains.
    extent: u32,
    /// Relative share of the center area. Ignored by the fixed edges.
    stretch: u32,
}

/// The allocation of one region's tracks.
struct TrackAllocation {
    lengths: Vec<u32>,
    /// The spacing actually used, which is reduced when the region cannot pay it.
    spacing: u32,
    /// Sum of the lengths plus the spacing between them. Never exceeds the available
    /// extent the allocation was asked for.
    total: u32,
}

/// Fixed-edge plus elastic-center layout manager.
#[derive(Debug, Default)]
pub struct BorderLayout {
    spacing: u32,
    margin: u32,
    /// Four-way margins. `None` means the uniform [`Self::margin`] is used on every side.
    margins: Option<crate::style::EdgeOffsets>,
    north: Vec<BorderItem>,
    south: Vec<BorderItem>,
    west: Vec<BorderItem>,
    east: Vec<BorderItem>,
    center: Vec<BorderItem>,
}

impl BorderLayout {
    /// Creates an empty border layout.
    ///
    /// `spacing` separates tracks inside one region; `margin` insets the whole layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self { spacing, margin, ..Self::default() }
    }

    /// Returns the inter-track spacing.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Updates the inter-track spacing.
    pub fn set_spacing(&mut self, spacing: u32) {
        self.spacing = spacing;
    }

    /// Returns the outer margin.
    pub fn margin(&self) -> u32 {
        self.margin
    }

    /// Updates the outer margin.
    pub fn set_margin(&mut self, margin: u32) {
        self.margin = margin;
    }

    /// Sets four-way margins, overriding the uniform [`Self::margin`].
    pub fn set_margins(&mut self, margins: crate::style::EdgeOffsets) {
        self.margins = Some(margins);
    }

    /// The four-way margins, or `None` when the uniform [`Self::margin`] applies.
    pub fn margins(&self) -> Option<crate::style::EdgeOffsets> {
        self.margins
    }

    /// Left/top/right/bottom insets, scaled by `scale`.
    fn insets(&self, scale: f32) -> (u32, u32, u32, u32) {
        match self.margins {
            Some(margins) => (
                (margins.left as f32 * scale).round() as u32,
                (margins.top as f32 * scale).round() as u32,
                (margins.right as f32 * scale).round() as u32,
                (margins.bottom as f32 * scale).round() as u32,
            ),
            None => {
                let uniform = (self.margin as f32 * scale).round() as u32;
                (uniform, uniform, uniform, uniform)
            }
        }
    }

    /// Adds a widget to `region` with a fixed `extent` and a stretch factor of 1.
    ///
    /// A widget already placed anywhere in this layout is moved rather than duplicated,
    /// which keeps `child_ids` (and therefore the layout's idea of its children) exact.
    pub fn add(&mut self, region: BorderRegion, widget_id: ObjectId, extent: u32) -> &mut Self {
        self.add_with_stretch(region, widget_id, extent, 1)
    }

    /// Adds a widget to `region` with an explicit stretch factor.
    ///
    /// The stretch factor only matters in [`BorderRegion::Center`]; fixed edges ignore it.
    pub fn add_with_stretch(
        &mut self,
        region: BorderRegion,
        widget_id: ObjectId,
        extent: u32,
        stretch: u32,
    ) -> &mut Self {
        self.take(widget_id);
        self.region_mut(region).push(BorderItem { widget_id, extent, stretch: stretch.max(1) });
        self
    }

    /// Moves an existing child to `region`, preserving its extent.
    ///
    /// Returns `false` when `widget_id` is not a child of this layout. The declarative
    /// loader uses this after it has registered a child through the generic
    /// [`Layout::add_widget`], which has no region parameter.
    pub fn set_region(&mut self, widget_id: ObjectId, region: BorderRegion, extent: u32) -> bool {
        let Some(existing) = self.take(widget_id) else {
            return false;
        };
        self.region_mut(region).push(BorderItem { widget_id, extent, stretch: existing.stretch });
        true
    }

    /// Updates the fixed extent of a child.
    ///
    /// Returns `false` when the child is not in this layout.
    pub fn set_extent(&mut self, widget_id: ObjectId, extent: u32) -> bool {
        for item in self
            .north
            .iter_mut()
            .chain(self.south.iter_mut())
            .chain(self.west.iter_mut())
            .chain(self.east.iter_mut())
            .chain(self.center.iter_mut())
        {
            if item.widget_id == widget_id {
                item.extent = extent;
                return true;
            }
        }
        false
    }

    /// Returns the fixed extent recorded for a child.
    pub fn extent(&self, widget_id: ObjectId) -> Option<u32> {
        self.item(widget_id).map(|item| item.extent)
    }

    /// Returns the region a child is placed in.
    pub fn region_of(&self, widget_id: ObjectId) -> Option<BorderRegion> {
        [
            (BorderRegion::North, &self.north),
            (BorderRegion::South, &self.south),
            (BorderRegion::West, &self.west),
            (BorderRegion::East, &self.east),
            (BorderRegion::Center, &self.center),
        ]
        .into_iter()
        .find(|(_, items)| items.iter().any(|item| item.widget_id == widget_id))
        .map(|(region, _)| region)
    }

    /// Returns the number of tracks in a region.
    pub fn track_count(&self, region: BorderRegion) -> usize {
        self.region(region).len()
    }

    /// Returns every child in placement order.
    pub fn children(&self) -> Vec<ObjectId> {
        self.north
            .iter()
            .chain(self.south.iter())
            .chain(self.west.iter())
            .chain(self.east.iter())
            .chain(self.center.iter())
            .map(|item| item.widget_id)
            .collect()
    }

    fn region(&self, region: BorderRegion) -> &Vec<BorderItem> {
        match region {
            BorderRegion::North => &self.north,
            BorderRegion::South => &self.south,
            BorderRegion::West => &self.west,
            BorderRegion::East => &self.east,
            BorderRegion::Center => &self.center,
        }
    }

    fn region_mut(&mut self, region: BorderRegion) -> &mut Vec<BorderItem> {
        match region {
            BorderRegion::North => &mut self.north,
            BorderRegion::South => &mut self.south,
            BorderRegion::West => &mut self.west,
            BorderRegion::East => &mut self.east,
            BorderRegion::Center => &mut self.center,
        }
    }

    fn item(&self, widget_id: ObjectId) -> Option<&BorderItem> {
        self.north
            .iter()
            .chain(self.south.iter())
            .chain(self.west.iter())
            .chain(self.east.iter())
            .chain(self.center.iter())
            .find(|item| item.widget_id == widget_id)
    }

    fn take(&mut self, widget_id: ObjectId) -> Option<BorderItem> {
        for region in [
            BorderRegion::North,
            BorderRegion::South,
            BorderRegion::West,
            BorderRegion::East,
            BorderRegion::Center,
        ] {
            let items = self.region_mut(region);
            if let Some(index) = items.iter().position(|item| item.widget_id == widget_id) {
                return Some(items.remove(index));
            }
        }
        None
    }

    fn arrange(
        &self,
        rect: Rect,
        spacing: u32,
        insets: (u32, u32, u32, u32),
        scale: f32,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // Each inset is honoured only as far as the rect can pay for it, so a control
        // resized smaller than twice the margin still produces a non-negative area.
        let limit_x = rect.width / 2;
        let limit_y = rect.height / 2;
        let left = insets.0.min(limit_x);
        let top = insets.1.min(limit_y);
        let right = insets.2.min(limit_x);
        let bottom = insets.3.min(limit_y);
        let inner = Rect::new(
            rect.x + left as i32,
            rect.y + top as i32,
            rect.width.saturating_sub(left).saturating_sub(right),
            rect.height.saturating_sub(top).saturating_sub(bottom),
        );

        let north = allocate_tracks(&self.north, inner.height, spacing, scale);
        let south =
            allocate_tracks(&self.south, inner.height.saturating_sub(north.total), spacing, scale);
        let west = allocate_tracks(&self.west, inner.width, spacing, scale);
        let east =
            allocate_tracks(&self.east, inner.width.saturating_sub(west.total), spacing, scale);

        // Vertical range left for the west/east bands and the center.
        let middle_top = inner.y.saturating_add(north.total as i32);
        let middle_bottom = inner
            .y
            .saturating_add(inner.height as i32)
            .saturating_sub(south.total as i32)
            .max(middle_top);
        let middle_height = (middle_bottom - middle_top).max(0) as u32;

        place_vertical(&self.north, &north, inner.x, inner.y, inner.width, widgets);
        place_vertical(
            &self.south,
            &south,
            inner.x,
            inner.y + inner.height.saturating_sub(south.total) as i32,
            inner.width,
            widgets,
        );
        place_horizontal(&self.west, &west, inner.x, middle_top, middle_height, widgets);
        place_horizontal(
            &self.east,
            &east,
            inner.x + inner.width.saturating_sub(east.total) as i32,
            middle_top,
            middle_height,
            widgets,
        );

        let center_x = inner.x.saturating_add(west.total as i32);
        let center_right = inner
            .x
            .saturating_add(inner.width as i32)
            .saturating_sub(east.total as i32)
            .max(center_x);
        let center_width = (center_right - center_x).max(0) as u32;
        place_center(
            &self.center,
            Rect::new(center_x, middle_top, center_width, middle_height),
            spacing,
            widgets,
        );
    }
}

/// Allocates fixed lengths for one region, clamped to `available`.
///
/// The spacing is reduced first when the region cannot pay the configured gaps, and the
/// per-track extents are then paid in insertion order. A track that cannot be paid in
/// full receives what is left, which is how the invariant "no child outside the parent"
/// survives an over-subscribed edge.
fn allocate_tracks(
    items: &[BorderItem],
    available: u32,
    spacing: u32,
    scale: f32,
) -> TrackAllocation {
    if items.is_empty() {
        return TrackAllocation { lengths: Vec::new(), spacing: 0, total: 0 };
    }
    let gaps = items.len().saturating_sub(1) as u32;
    let spacing = spacing.min(available.checked_div(gaps).unwrap_or(0));
    let budget = available.saturating_sub(gaps * spacing);
    let mut remaining = budget;
    let mut lengths = Vec::with_capacity(items.len());
    for item in items {
        let extent = scale_extent(item.extent, scale).min(remaining);
        remaining -= extent;
        lengths.push(extent);
    }
    let total = lengths.iter().copied().sum::<u32>().saturating_add(gaps * spacing);
    TrackAllocation { lengths, spacing, total }
}

fn scale_extent(extent: u32, scale: f32) -> u32 {
    (extent as f32 * scale).round().max(0.0) as u32
}

fn place_vertical(
    items: &[BorderItem],
    allocation: &TrackAllocation,
    x: i32,
    y: i32,
    width: u32,
    widgets: &mut dyn FnMut(ObjectId, Rect),
) {
    let mut cursor = y;
    for (item, length) in items.iter().zip(allocation.lengths.iter().copied()) {
        widgets(item.widget_id, Rect::new(x, cursor, width, length));
        cursor = cursor.saturating_add(length as i32 + allocation.spacing as i32);
    }
}

fn place_horizontal(
    items: &[BorderItem],
    allocation: &TrackAllocation,
    x: i32,
    y: i32,
    height: u32,
    widgets: &mut dyn FnMut(ObjectId, Rect),
) {
    let mut cursor = x;
    for (item, length) in items.iter().zip(allocation.lengths.iter().copied()) {
        widgets(item.widget_id, Rect::new(cursor, y, length, height));
        cursor = cursor.saturating_add(length as i32 + allocation.spacing as i32);
    }
}

/// Places the center tracks side by side, splitting the remaining width by stretch.
fn place_center(
    items: &[BorderItem],
    rect: Rect,
    spacing: u32,
    widgets: &mut dyn FnMut(ObjectId, Rect),
) {
    if items.is_empty() || rect.width == 0 || rect.height == 0 {
        return;
    }
    let gaps = items.len().saturating_sub(1) as u32;
    let spacing = spacing.min(rect.width.checked_div(gaps).unwrap_or(0));
    let available = rect.width.saturating_sub(gaps * spacing);
    let total_stretch = items.iter().map(|item| u64::from(item.stretch)).sum::<u64>().max(1);
    let mut widths = Vec::with_capacity(items.len());
    let mut assigned = 0u32;
    for item in items {
        let width = (u64::from(available) * u64::from(item.stretch) / total_stretch) as u32;
        assigned = assigned.saturating_add(width);
        widths.push(width);
    }
    // The integer division leaves a remainder; give it to the first track rather than
    // leaving a strip of the parent unpainted.
    if let Some(first) = widths.first_mut() {
        *first = first.saturating_add(available.saturating_sub(assigned));
    }
    let mut cursor = rect.x;
    for (item, width) in items.iter().zip(widths.iter().copied()) {
        widgets(item.widget_id, Rect::new(cursor, rect.y, width, rect.height));
        cursor = cursor.saturating_add(width as i32 + spacing as i32);
    }
}

impl Layout for BorderLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// Adds a child to [`BorderRegion::Center`], where the stretch factor is meaningful.
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.add_with_stretch(BorderRegion::Center, widget_id, 0, stretch);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.take(widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.item(id).is_some()
    }

    fn clear(&mut self) {
        self.north.clear();
        self.south.clear();
        self.west.clear();
        self.east.clear();
        self.center.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        self.arrange(rect, self.spacing, self.insets(1.0), 1.0, widgets);
    }

    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        // Same rule as `BoxLayout`: a HiDPI screen needs more logical spacing, and a
        // device whose text is larger needs more room between tracks at the same DPI.
        let scale = context.layout_scale.max(context.font_scale);
        let spacing = (self.spacing as f32 * scale).round() as u32;
        let insets = self.insets(scale);
        self.arrange(rect, spacing, insets, scale, widgets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

    fn rects(layout: &BorderLayout, rect: Rect) -> HashMap<ObjectId, Rect> {
        let mut out = HashMap::new();
        layout.update(rect, &mut |id, geometry| {
            out.insert(id, geometry);
        });
        out
    }

    #[test]
    fn edge_tracks_are_fixed_and_the_center_takes_the_rest() {
        let mut layout = BorderLayout::new(0, 0);
        layout.add(BorderRegion::North, 1, 40);
        layout.add(BorderRegion::South, 2, 30);
        layout.add(BorderRegion::West, 3, 100);
        layout.add(BorderRegion::East, 4, 50);
        layout.add(BorderRegion::Center, 5, 0);

        let out = rects(&layout, Rect::new(0, 0, 400, 300));
        assert_eq!(out.get(&1), Some(&Rect::new(0, 0, 400, 40)), "north");
        assert_eq!(out.get(&2), Some(&Rect::new(0, 270, 400, 30)), "south");
        assert_eq!(out.get(&3), Some(&Rect::new(0, 40, 100, 230)), "west");
        assert_eq!(out.get(&4), Some(&Rect::new(350, 40, 50, 230)), "east");
        assert_eq!(out.get(&5), Some(&Rect::new(100, 40, 250, 230)), "center");
    }

    #[test]
    fn several_tracks_in_one_region_stack_with_spacing() {
        let mut layout = BorderLayout::new(5, 0);
        layout.add(BorderRegion::North, 1, 20);
        layout.add(BorderRegion::North, 2, 30);
        layout.add(BorderRegion::Center, 3, 0);

        let out = rects(&layout, Rect::new(0, 0, 100, 200));
        assert_eq!(out.get(&1), Some(&Rect::new(0, 0, 100, 20)));
        assert_eq!(out.get(&2), Some(&Rect::new(0, 25, 100, 30)));
        assert_eq!(out.get(&3), Some(&Rect::new(0, 55, 100, 145)));
        assert_eq!(layout.track_count(BorderRegion::North), 2);
    }

    #[test]
    fn an_over_subscribed_edge_stays_inside_the_parent() {
        let mut layout = BorderLayout::new(10, 4);
        layout.add(BorderRegion::North, 1, 90);
        layout.add(BorderRegion::North, 2, 90);
        layout.add(BorderRegion::South, 3, 90);
        layout.add(BorderRegion::West, 4, 90);
        layout.add(BorderRegion::East, 5, 90);
        layout.add(BorderRegion::Center, 6, 0);

        let parent = Rect::new(0, 0, 20, 16);
        let out = rects(&layout, parent);
        for (id, rect) in &out {
            assert!(
                rect.x >= parent.x
                    && rect.y >= parent.y
                    && rect.x + rect.width as i32 <= parent.x + parent.width as i32
                    && rect.y + rect.height as i32 <= parent.y + parent.height as i32,
                "track {id} escaped {parent:?}: {rect:?}"
            );
        }
    }

    #[test]
    fn context_scaling_grows_extents_spacing_and_margin() {
        let mut layout = BorderLayout::new(4, 2);
        layout.add(BorderRegion::North, 1, 20);
        layout.add(BorderRegion::Center, 2, 0);

        let context =
            LayoutContext { layout_scale: 2.0, font_scale: 2.0, ..LayoutContext::default() };
        let mut out = HashMap::new();
        layout.update_with_context(Rect::new(0, 0, 200, 100), &context, &mut |id, geometry| {
            out.insert(id, geometry);
        });
        // margin 2*2=4, north 20*2=40, spacing irrelevant for one track.
        assert_eq!(out.get(&1), Some(&Rect::new(4, 4, 192, 40)));
        assert_eq!(out.get(&2), Some(&Rect::new(4, 44, 192, 52)));
    }

    #[test]
    fn moving_a_widget_to_a_region_does_not_duplicate_it() {
        let mut layout = BorderLayout::new(0, 0);
        layout.add_widget(7, 1);
        assert_eq!(layout.region_of(7), Some(BorderRegion::Center));

        assert!(layout.set_region(7, BorderRegion::North, 36));
        assert_eq!(layout.region_of(7), Some(BorderRegion::North));
        assert_eq!(layout.track_count(BorderRegion::Center), 0);
        assert_eq!(layout.child_ids().as_slice(), &[7]);
    }

    #[test]
    fn center_tracks_share_the_remaining_width_by_stretch() {
        let mut layout = BorderLayout::new(0, 0);
        layout.add(BorderRegion::West, 1, 40);
        layout.add_with_stretch(BorderRegion::Center, 2, 0, 1);
        layout.add_with_stretch(BorderRegion::Center, 3, 0, 3);

        let out = rects(&layout, Rect::new(0, 0, 360, 100));
        assert_eq!(out.get(&2), Some(&Rect::new(40, 0, 80, 100)));
        assert_eq!(out.get(&3), Some(&Rect::new(120, 0, 240, 100)));
    }

    #[test]
    fn removing_a_widget_removes_it_from_every_region() {
        let mut layout = BorderLayout::new(0, 0);
        layout.add(BorderRegion::South, 9, 20);
        assert!(layout.has_child(9));
        layout.remove_widget(9);
        assert!(!layout.has_child(9));
        assert!(rects(&layout, Rect::new(0, 0, 10, 10)).is_empty());
    }
}
