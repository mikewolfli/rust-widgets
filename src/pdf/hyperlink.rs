use crate::core::Rect;
use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq)]
pub enum LinkAction {
    GoToPage { page: u32, x: f32, y: f32 },
    GoToNamedDestination(String),
    Uri(String),
    LaunchFile(String),
    JavaScript(String),
    NamedAction(NamedAction),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedAction {
    NextPage,
    PrevPage,
    FirstPage,
    LastPage,
    Print,
    SaveAs,
}
#[derive(Debug, Clone)]
pub struct Hyperlink {
    pub id: String,
    pub page: u32,
    pub rect: Rect,
    pub action: LinkAction,
    pub border: LinkBorder,
    pub highlight_mode: HighlightMode,
    pub uri: Option<String>,
    pub tooltip: String,
}
impl Hyperlink {
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
    pub fn with_uri(mut self, uri: String) -> Self {
        self.uri = Some(uri.clone());
        self.action = LinkAction::Uri(uri);
        self
    }
    pub fn with_page_target(mut self, page: u32, x: f32, y: f32) -> Self {
        self.action = LinkAction::GoToPage { page, x, y };
        self
    }
    pub fn with_tooltip(mut self, tooltip: String) -> Self {
        self.tooltip = tooltip;
        self
    }
    pub fn with_border(mut self, border: LinkBorder) -> Self {
        self.border = border;
        self
    }
    pub fn with_highlight_mode(mut self, mode: HighlightMode) -> Self {
        self.highlight_mode = mode;
        self
    }
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        self.rect.contains_point(crate::core::Point::new(x, y))
    }
}
#[derive(Debug, Clone)]
pub struct LinkBorder {
    pub horizontal_corner_radius: f32,
    pub vertical_corner_radius: f32,
    pub border_width: f32,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightMode {
    None,
    Invert,
    Outline,
    Push,
}
impl Default for HighlightMode {
    fn default() -> Self {
        Self::Invert
    }
}
#[derive(Debug, Clone)]
pub struct NamedDestination {
    pub name: String,
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}
impl NamedDestination {
    pub fn new(name: String, page: u32, x: f32, y: f32) -> Self {
        Self {
            name,
            page,
            x,
            y,
            zoom: 0.0,
        }
    }
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
}
pub struct HyperlinkManager {
    links: HashMap<String, Hyperlink>,
    page_links: HashMap<u32, Vec<String>>,
    named_destinations: HashMap<String, NamedDestination>,
}
impl HyperlinkManager {
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            page_links: HashMap::new(),
            named_destinations: HashMap::new(),
        }
    }
    pub fn add_link(&mut self, link: Hyperlink) {
        let id = link.id.clone();
        let page = link.page;
        self.links.insert(id.clone(), link);
        self.page_links.entry(page).or_default().push(id);
    }
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
    pub fn get_link(&self, id: &str) -> Option<&Hyperlink> {
        self.links.get(id)
    }
    pub fn get_link_at_point(&self, page: u32, x: i32, y: i32) -> Option<&Hyperlink> {
        self.page_links.get(&page).and_then(|ids| {
            ids.iter()
                .filter_map(|id| self.links.get(id))
                .find(|link| link.contains_point(x, y))
        })
    }
    pub fn get_page_links(&self, page: u32) -> Vec<&Hyperlink> {
        self.page_links
            .get(&page)
            .map(|ids| ids.iter().filter_map(|id| self.links.get(id)).collect())
            .unwrap_or_default()
    }
    pub fn add_named_destination(&mut self, destination: NamedDestination) {
        self.named_destinations
            .insert(destination.name.clone(), destination);
    }
    pub fn get_named_destination(&self, name: &str) -> Option<&NamedDestination> {
        self.named_destinations.get(name)
    }
    pub fn remove_named_destination(&mut self, name: &str) -> Option<NamedDestination> {
        self.named_destinations.remove(name)
    }
    pub fn clear(&mut self) {
        self.links.clear();
        self.page_links.clear();
        self.named_destinations.clear();
    }
    pub fn link_count(&self) -> usize {
        self.links.len()
    }
    pub fn destination_count(&self) -> usize {
        self.named_destinations.len()
    }
}
impl Default for HyperlinkManager {
    fn default() -> Self {
        Self::new()
    }
}
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
            LinkAction::GoToPage {
                page: 2,
                x: 0.0,
                y: 0.0,
            },
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
