// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::Rect;
use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq)]
/// Destination a link annotation points at when it is activated.
pub enum LinkAction {
    /// Jump within the document to `page` (zero-based page index) at the
    /// coordinates `x`, `y` in PDF user-space units, measured from the page's
    /// lower-left corner.
    GoToPage {
        /// Zero-based index of the target page.
        page: u32,
        /// Horizontal offset on the target page, in PDF user-space units.
        x: f32,
        /// Vertical offset on the target page, in PDF user-space units.
        y: f32,
    },
    /// Jump to the destination registered under this name in the document's
    /// name dictionary. Resolution happens at activation time, so an unknown
    /// name simply fails to navigate.
    GoToNamedDestination(String),
    /// Open an external URI.
    Uri(String),
    /// Launch a file on the local machine, identified by its path.
    LaunchFile(String),
    /// Run a JavaScript snippet, given as source text.
    JavaScript(String),
    /// Perform one of the standard viewer actions.
    NamedAction(NamedAction),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Standard, viewer-defined navigation commands a link can trigger.
pub enum NamedAction {
    /// Advance to the next page.
    NextPage,
    /// Go back to the previous page.
    PrevPage,
    /// Jump to the document's first page.
    FirstPage,
    /// Jump to the document's last page.
    LastPage,
    /// Open the viewer's print dialog.
    Print,
    /// Open a save-as dialog for the document.
    SaveAs,
}
#[derive(Debug, Clone)]
/// A single link annotation: the page region that responds to clicks plus the
/// behaviour and appearance used when it is activated.
pub struct Hyperlink {
    /// Identifier unique within a [`HyperlinkManager`]; the manager stores links
    /// by this key, so a later link with the same id replaces the earlier one.
    pub id: String,
    /// Zero-based index of the page the link annotation is placed on.
    pub page: u32,
    /// Clickable region on that page, in document coordinates.
    pub rect: Rect,
    /// What activation does.
    pub action: LinkAction,
    /// Border drawn around the annotation region; all defaults are zero-width,
    /// so no border is drawn unless one is set with `with_border`.
    pub border: LinkBorder,
    /// Visual feedback shown while the pointer is over or pressing the link.
    pub highlight_mode: HighlightMode,
    /// Cached copy of the target URI for quick inspection. `None` unless set via
    /// `with_uri`, which also rewrites `action` to [`LinkAction::Uri`].
    pub uri: Option<String>,
    /// Text shown in a hover tooltip; empty when unset.
    pub tooltip: String,
}
impl Hyperlink {
    /// Creates a link with default appearance: no border, [`HighlightMode::Invert`],
    /// no tooltip, and `uri` left as `None` even when `action` is [`LinkAction::Uri`].
    pub fn new(id: String, page: u32, rect: Rect, action: LinkAction) -> Self {
        Self {
            id,
            page,
            rect,
            action,
            border: LinkBorder::default(),
            highlight_mode: HighlightMode::Invert,
            uri: None,
            tooltip: String::new(),
        }
    }
    /// Builder that sets both `uri` and [`LinkAction::Uri`] to the same value,
    /// overwriting any action previously assigned.
    pub fn with_uri(mut self, uri: String) -> Self {
        self.uri = Some(uri.clone());
        self.action = LinkAction::Uri(uri);
        self
    }
    /// Builder that replaces the action with a jump to `page` at `x`, `y`
    /// (PDF user-space units). Does not move the annotation's own `page`.
    pub fn with_page_target(mut self, page: u32, x: f32, y: f32) -> Self {
        self.action = LinkAction::GoToPage { page, x, y };
        self
    }
    /// Builder that sets the hover tooltip text.
    pub fn with_tooltip(mut self, tooltip: String) -> Self {
        self.tooltip = tooltip;
        self
    }
    /// Builder that sets the annotation border appearance.
    pub fn with_border(mut self, border: LinkBorder) -> Self {
        self.border = border;
        self
    }
    /// Builder that sets the visual highlight behaviour.
    pub fn with_highlight_mode(mut self, mode: HighlightMode) -> Self {
        self.highlight_mode = mode;
        self
    }
    /// Returns whether the integer point `(x, y)` falls inside the annotation
    /// rectangle. The values are widened to `f32` before the test, so
    /// sub-pixel precision in `rect` is honoured.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        self.rect.contains_point(crate::core::Point::from_f32(x as f32, y as f32))
    }
}
#[derive(Debug, Clone)]
/// Border drawn around a link annotation's clickable region.
pub struct LinkBorder {
    /// Corner radius along the horizontal axis, in PDF user-space units.
    pub horizontal_corner_radius: f32,
    /// Corner radius along the vertical axis, in PDF user-space units.
    pub vertical_corner_radius: f32,
    /// Stroke width in PDF user-space units; zero draws no line.
    pub border_width: f32,
    /// Dash pattern as alternating on/off run lengths in user-space units, or
    /// `None` for a solid stroke. An empty vec is treated as no dashes.
    pub dash_pattern: Option<Vec<f32>>,
}
impl Default for LinkBorder {
    fn default() -> Self {
        Self {
            horizontal_corner_radius: 0.0,
            vertical_corner_radius: 0.0,
            border_width: 0.0,
            dash_pattern: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Visual feedback applied to a link while the pointer is over it or pressing it.
pub enum HighlightMode {
    /// Leave the page unchanged.
    None,
    /// Invert the pixels inside the link rectangle (the PDF default).
    #[default]
    Invert,
    /// Draw an outline around the link rectangle without changing its pixels.
    Outline,
    /// Emphasise the link as if it were depressed.
    Push,
}
#[derive(Debug, Clone)]
/// A named view destination that link actions can refer to by string name.
pub struct NamedDestination {
    /// Name used to look the destination up; unique within a [`HyperlinkManager`],
    /// since a later destination with the same name replaces the earlier one.
    pub name: String,
    /// Zero-based index of the target page.
    pub page: u32,
    /// Horizontal offset on the target page, in PDF user-space units.
    pub x: f32,
    /// Vertical offset on the target page, in PDF user-space units.
    pub y: f32,
    /// Zoom factor to apply on navigation. `0.0` is the sentinel for "keep the
    /// current zoom", which is why `new` starts there.
    pub zoom: f32,
}
impl NamedDestination {
    /// Creates a destination that keeps the current zoom (`zoom` is 0.0).
    pub fn new(name: String, page: u32, x: f32, y: f32) -> Self {
        Self { name, page, x, y, zoom: 0.0 }
    }
    /// Builder that sets the zoom factor applied on navigation.
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
}
/// Stores link annotations and named destinations for a document.
///
/// Links are keyed by [`Hyperlink::id`], so ids must be unique: re-adding an
/// existing id overwrites the stored link while leaving the old id in the
/// per-page index, meaning both entries resolve to the newer link. Per-page
/// index vectors preserve insertion order, which is the order `get_link_at_point`
/// searches in, so the first added overlapping link wins.
pub struct HyperlinkManager {
    links: HashMap<String, Hyperlink>,
    page_links: HashMap<u32, Vec<String>>,
    named_destinations: HashMap<String, NamedDestination>,
}
impl HyperlinkManager {
    /// Creates an empty manager with no links or named destinations.
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            page_links: HashMap::new(),
            named_destinations: HashMap::new(),
        }
    }
    /// Adds `link`, indexing it under its own `page` for hit testing.
    pub fn add_link(&mut self, link: Hyperlink) {
        let id = link.id.clone();
        let page = link.page;
        self.links.insert(id.clone(), link);
        self.page_links.entry(page).or_default().push(id);
    }
    /// Removes the link with `id` and drops it from its page index, returning it
    /// if it was present and `None` otherwise.
    pub fn remove_link(&mut self, id: &str) -> Option<Hyperlink> {
        if let Some(link) = self.links.remove(id) {
            if let Some(page_links) = self.page_links.get_mut(&link.page) {
                page_links.retain(|l| l != id);
            }
            Some(link)
        } else {
            None
        }
    }
    /// Returns the link stored under `id`, or `None` if no such link exists.
    pub fn get_link(&self, id: &str) -> Option<&Hyperlink> {
        self.links.get(id)
    }
    /// Returns the first link on `page` whose rectangle contains the integer
    /// point `(x, y)`, in the order the links were added; `None` when the page
    /// has no links or the point misses all of them.
    pub fn get_link_at_point(&self, page: u32, x: i32, y: i32) -> Option<&Hyperlink> {
        self.page_links.get(&page).and_then(|ids| {
            ids.iter().filter_map(|id| self.links.get(id)).find(|link| link.contains_point(x, y))
        })
    }
    /// Returns every link on `page` in insertion order, or an empty vector when
    /// the page has none.
    pub fn get_page_links(&self, page: u32) -> Vec<&Hyperlink> {
        self.page_links
            .get(&page)
            .map(|ids| ids.iter().filter_map(|id| self.links.get(id)).collect())
            .unwrap_or_default()
    }
    /// Registers `destination`, replacing any destination already stored under
    /// the same name.
    pub fn add_named_destination(&mut self, destination: NamedDestination) {
        self.named_destinations.insert(destination.name.clone(), destination);
    }
    /// Returns the destination registered under `name`, or `None` if there is none.
    pub fn get_named_destination(&self, name: &str) -> Option<&NamedDestination> {
        self.named_destinations.get(name)
    }
    /// Removes and returns the destination registered under `name`, or `None` if
    /// there is none.
    pub fn remove_named_destination(&mut self, name: &str) -> Option<NamedDestination> {
        self.named_destinations.remove(name)
    }
    /// Drops every link, page index and named destination.
    pub fn clear(&mut self) {
        self.links.clear();
        self.page_links.clear();
        self.named_destinations.clear();
    }
    /// Returns how many distinct link ids are stored.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }
    /// Returns how many named destinations are stored.
    pub fn destination_count(&self) -> usize {
        self.named_destinations.len()
    }
}
crate::impl_default_via_new!(HyperlinkManager);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hyperlink_creation() {
        let link = Hyperlink::new(
            "link-1".to_string(),
            1,
            Rect::new(100, 100, 200, 50),
            LinkAction::Uri("https://example.com".to_string()),
        )
        .with_tooltip("Click to visit".to_string());
        assert_eq!(link.id, "link-1");
        assert!(matches!(link.action, LinkAction::Uri(_)));
        assert_eq!(link.tooltip, "Click to visit");
    }
    #[test]
    fn test_hyperlink_manager() {
        let mut manager = HyperlinkManager::new();
        let link = Hyperlink::new(
            "link-1".to_string(),
            1,
            Rect::new(100, 100, 200, 50),
            LinkAction::GoToPage { page: 2, x: 0.0, y: 0.0 },
        );
        manager.add_link(link);
        assert_eq!(manager.link_count(), 1);
        let found = manager.get_link_at_point(1, 150, 125);
        assert!(found.is_some());
        let not_found = manager.get_link_at_point(1, 50, 50);
        assert!(not_found.is_none());
    }
    #[test]
    fn test_named_destination() {
        let mut manager = HyperlinkManager::new();
        let dest = NamedDestination::new("intro".to_string(), 1, 0.0, 0.0).with_zoom(1.0);
        manager.add_named_destination(dest);
        let found = manager.get_named_destination("intro");
        assert!(found.is_some());
        assert_eq!(found.unwrap().page, 1);
    }
}
