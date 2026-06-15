use super::Layout;
use crate::core::{ObjectId, Rect, Size};
use crate::widget::Widget;
use core::fmt;

/// Internal child entry that holds a widget ID and optionally the widget object.
/// Ensures `add_widget` (ID only) and `add_child` (full widget) use the same list.
struct FlowChild {
    widget_id: ObjectId,
    widget: Option<Box<dyn Widget>>,
    /// Default size used when widget is absent (added via `add_widget`).
    default_size: Size,
}

impl FlowChild {
    fn size_hint(&self) -> Size {
        self.widget.as_ref().map(|w| w.size_hint()).unwrap_or(self.default_size)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowDirection {
    #[default]
    Horizontal,
    Vertical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowAlignment {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}
#[derive(Debug, Clone, Copy)]
pub struct FlowLayoutConfig {
    pub direction: FlowDirection,
    pub alignment: FlowAlignment,
    pub spacing: i32,
    pub padding: i32,
    pub wrap: bool,
}
impl Default for FlowLayoutConfig {
    fn default() -> Self {
        Self {
            direction: FlowDirection::Horizontal,
            alignment: FlowAlignment::Start,
            spacing: 8,
            padding: 8,
            wrap: false,
        }
    }
}
pub struct FlowLayout {
    config: FlowLayoutConfig,
    children: Vec<FlowChild>,
}
impl fmt::Debug for FlowLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowLayout")
            .field("config", &self.config)
            .field("children", &format_args!("{} children", self.children.len()))
            .finish()
    }
}
impl FlowLayout {
    pub fn new() -> Self {
        Self { config: FlowLayoutConfig::default(), children: Vec::new() }
    }
    pub fn with_config(config: FlowLayoutConfig) -> Self {
        Self { config, children: Vec::new() }
    }
    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        let widget_id = child.id();
        let default_size = child.size_hint();
        self.children.push(FlowChild { widget_id, widget: Some(child), default_size });
    }
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            self.children.remove(index).widget
        } else {
            None
        }
    }
    /// Override the default size hint for a child added via `add_widget` (no widget ref).
    pub fn set_child_size(&mut self, widget_id: ObjectId, size: Size) {
        if let Some(child) = self.children.iter_mut().find(|c| c.widget_id == widget_id) {
            child.default_size = size;
        }
    }

    pub fn clear_children(&mut self) {
        self.children.clear();
    }
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
    pub fn layout(&self, available_rect: Rect) -> Vec<Rect> {
        let content_rect = Rect::new(
            available_rect.x + self.config.padding,
            available_rect.y + self.config.padding,
            available_rect.width.saturating_sub(2 * self.config.padding as u32),
            available_rect.height.saturating_sub(2 * self.config.padding as u32),
        );
        match self.config.direction {
            FlowDirection::Horizontal => self.layout_horizontal(&content_rect),
            FlowDirection::Vertical => self.layout_vertical(&content_rect),
        }
    }
    fn layout_horizontal(&self, content_rect: &Rect) -> Vec<Rect> {
        let mut positions = Vec::new();
        let mut current_x = content_rect.x;
        let mut current_y = content_rect.y;
        let mut row_height = 0i32;
        for child in &self.children {
            let size = child.size_hint();
            let child_width = size.width as i32;
            let child_height = size.height as i32;
            if self.config.wrap
                && current_x + child_width > content_rect.x + content_rect.width as i32
            {
                current_x = content_rect.x;
                current_y += row_height + self.config.spacing;
                row_height = 0;
            }
            if current_y + child_height > content_rect.y + content_rect.height as i32 {
                continue;
            }
            positions.push(Rect::new(
                current_x,
                current_y,
                child_width as u32,
                child_height as u32,
            ));
            current_x += child_width + self.config.spacing;
            row_height = row_height.max(child_height);
        }
        self.apply_alignment(&mut positions, content_rect);
        positions
    }
    fn layout_vertical(&self, content_rect: &Rect) -> Vec<Rect> {
        let mut positions = Vec::new();
        let mut current_x = content_rect.x;
        let mut current_y = content_rect.y;
        let mut column_width = 0i32;
        for child in &self.children {
            let size = child.size_hint();
            let child_width = size.width as i32;
            let child_height = size.height as i32;
            if self.config.wrap
                && current_y + child_height > content_rect.y + content_rect.height as i32
            {
                current_y = content_rect.y;
                current_x += column_width + self.config.spacing;
                column_width = 0;
            }
            if current_x + child_width > content_rect.x + content_rect.width as i32 {
                continue;
            }
            positions.push(Rect::new(
                current_x,
                current_y,
                child_width as u32,
                child_height as u32,
            ));
            current_y += child_height + self.config.spacing;
            column_width = column_width.max(child_width);
        }
        self.apply_alignment(&mut positions, content_rect);
        positions
    }
    fn apply_alignment(&self, positions: &mut [Rect], content_rect: &Rect) {
        match self.config.alignment {
            FlowAlignment::Start => {}
            FlowAlignment::Center => {
                let total_width: i32 = positions.iter().map(|r| r.width as i32).sum::<i32>()
                    + (positions.len().saturating_sub(1) as i32) * self.config.spacing;
                let total_height: i32 = positions.iter().map(|r| r.height as i32).sum::<i32>()
                    + (positions.len().saturating_sub(1) as i32) * self.config.spacing;
                let offset_x = (content_rect.width as i32 - total_width) / 2;
                let offset_y = (content_rect.height as i32 - total_height) / 2;
                for pos in positions.iter_mut() {
                    pos.x += offset_x;
                    pos.y += offset_y;
                }
            }
            FlowAlignment::End => {
                let total_width: i32 = positions.iter().map(|r| r.width as i32).sum::<i32>()
                    + (positions.len().saturating_sub(1) as i32) * self.config.spacing;
                let total_height: i32 = positions.iter().map(|r| r.height as i32).sum::<i32>()
                    + (positions.len().saturating_sub(1) as i32) * self.config.spacing;
                let offset_x = content_rect.width as i32 - total_width;
                let offset_y = content_rect.height as i32 - total_height;
                for pos in positions.iter_mut() {
                    pos.x += offset_x;
                    pos.y += offset_y;
                }
            }
            FlowAlignment::SpaceBetween => {
                if positions.len() > 1 {
                    let total_size: i32 = match self.config.direction {
                        FlowDirection::Horizontal => positions.iter().map(|r| r.width as i32).sum(),
                        FlowDirection::Vertical => positions.iter().map(|r| r.height as i32).sum(),
                    };
                    let available = match self.config.direction {
                        FlowDirection::Horizontal => content_rect.width as i32,
                        FlowDirection::Vertical => content_rect.height as i32,
                    };
                    let spacing = if positions.len() > 1 {
                        (available - total_size) / (positions.len() as i32 - 1)
                    } else {
                        0
                    };
                    let mut current = match self.config.direction {
                        FlowDirection::Horizontal => content_rect.x,
                        FlowDirection::Vertical => content_rect.y,
                    };
                    for pos in positions.iter_mut() {
                        match self.config.direction {
                            FlowDirection::Horizontal => {
                                pos.x = current;
                                current += pos.width as i32 + spacing;
                            }
                            FlowDirection::Vertical => {
                                pos.y = current;
                                current += pos.height as i32 + spacing;
                            }
                        }
                    }
                }
            }
            FlowAlignment::SpaceAround => {
                let total_size: i32 = match self.config.direction {
                    FlowDirection::Horizontal => positions.iter().map(|r| r.width as i32).sum(),
                    FlowDirection::Vertical => positions.iter().map(|r| r.height as i32).sum(),
                };
                let available = match self.config.direction {
                    FlowDirection::Horizontal => content_rect.width as i32,
                    FlowDirection::Vertical => content_rect.height as i32,
                };
                let spacing = (available - total_size) / (positions.len() as i32 + 1);
                let mut current = match self.config.direction {
                    FlowDirection::Horizontal => content_rect.x + spacing,
                    FlowDirection::Vertical => content_rect.y + spacing,
                };
                for pos in positions.iter_mut() {
                    match self.config.direction {
                        FlowDirection::Horizontal => {
                            pos.x = current;
                            current += pos.width as i32 + spacing;
                        }
                        FlowDirection::Vertical => {
                            pos.y = current;
                            current += pos.height as i32 + spacing;
                        }
                    }
                }
            }
        }
    }
    pub fn preferred_size(&self) -> Size {
        let mut width = 0u32;
        let mut height = 0u32;
        match self.config.direction {
            FlowDirection::Horizontal => {
                for child in &self.children {
                    let size = child.size_hint();
                    width += size.width + self.config.spacing as u32;
                    height = height.max(size.height);
                }
                width = width.saturating_sub(self.config.spacing as u32);
            }
            FlowDirection::Vertical => {
                for child in &self.children {
                    let size = child.size_hint();
                    height += size.height + self.config.spacing as u32;
                    width = width.max(size.width);
                }
                height = height.saturating_sub(self.config.spacing as u32);
            }
        }
        width += 2 * self.config.padding as u32;
        height += 2 * self.config.padding as u32;
        Size::new(width, height)
    }
}
impl Default for FlowLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout for FlowLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        // Push to the unified children list so layout and update stay in sync.
        if !self.children.iter().any(|c| c.widget_id == widget_id) {
            // Use a reasonable default size (100x100) so layout doesn't collapse
            // children added without a widget reference. The caller can later
            // set the actual size via `set_child_size` or through the Widget trait.
            self.children.push(FlowChild {
                widget_id,
                widget: None,
                default_size: Size::new(100, 100),
            });
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|c| c.widget_id != widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let positions = self.layout(rect);
        for (i, child_rect) in positions.iter().enumerate() {
            if let Some(child) = self.children.get(i) {
                widgets(child.widget_id, *child_rect);
            }
        }
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.iter().map(|c| c.widget_id).collect()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.children.iter().any(|c| c.widget_id == id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal test widget that returns a fixed size hint.
    struct TestWidget {
        id: ObjectId,
        size: Size,
    }
    impl TestWidget {
        fn new(id: ObjectId, width: u32, height: u32) -> Self {
            Self { id, size: Size::new(width, height) }
        }
    }
    impl crate::event::EventHandler for TestWidget {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Widget for TestWidget {
        fn id(&self) -> ObjectId {
            self.id
        }
        fn size_hint(&self) -> Size {
            self.size
        }
    }

    // --- Empty layout tests ---

    #[test]
    fn test_empty_layout_returns_empty() {
        let layout = FlowLayout::new();
        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }

    #[test]
    fn test_empty_layout_center_alignment() {
        let mut layout = FlowLayout::new();
        layout.config.alignment = FlowAlignment::Center;
        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }

    // --- Child positioning tests via add_child (with full widget) ---

    #[test]
    fn test_horizontal_positions_children_in_a_row() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 10;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 60, 30)));

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 2);
        // First child at (0, 0), second immediately after with 10px spacing
        assert_eq!(positions[0], Rect::new(0, 0, 40, 20));
        assert_eq!(positions[1], Rect::new(50, 0, 60, 30));
    }

    #[test]
    fn test_vertical_positions_children_in_a_column() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Vertical;
        layout.config.spacing = 5;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 30, 50)));

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], Rect::new(0, 0, 40, 20));
        assert_eq!(positions[1], Rect::new(0, 25, 30, 50));
    }

    #[test]
    fn test_horizontal_wrap_to_next_row() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.config.wrap = true;

        // Three 60px children in a 125px-wide container: row1 gets two, row2 gets one.
        layout.add_child(Box::new(TestWidget::new(1, 60, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 60, 20)));
        layout.add_child(Box::new(TestWidget::new(3, 60, 20)));

        let positions = layout.layout(Rect::new(0, 0, 125, 100));
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], Rect::new(0, 0, 60, 20)); // row 1, child 1
        assert_eq!(positions[1], Rect::new(60, 0, 60, 20)); // row 1, child 2
        assert_eq!(positions[2], Rect::new(0, 20, 60, 20)); // row 2, child 3
    }

    #[test]
    fn test_vertical_wrap_to_next_column() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Vertical;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.config.wrap = true;

        // Three 50px children in a 105px-tall container: col1 gets two, col2 gets one.
        layout.add_child(Box::new(TestWidget::new(1, 30, 50)));
        layout.add_child(Box::new(TestWidget::new(2, 30, 50)));
        layout.add_child(Box::new(TestWidget::new(3, 30, 50)));

        let positions = layout.layout(Rect::new(0, 0, 200, 105));
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], Rect::new(0, 0, 30, 50)); // col 1, child 1
        assert_eq!(positions[1], Rect::new(0, 50, 30, 50)); // col 1, child 2
        assert_eq!(positions[2], Rect::new(30, 0, 30, 50)); // col 2, child 3
    }

    #[test]
    fn test_layout_honors_padding() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 10;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));

        let positions = layout.layout(Rect::new(0, 0, 100, 50));
        assert_eq!(positions.len(), 1);
        // Content starts at (10, 10) due to padding
        assert_eq!(positions[0], Rect::new(10, 10, 40, 20));
    }

    #[test]
    fn test_horizontal_clips_child_taller_than_container() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 0;

        // A child taller than the available content height is clipped (break).
        layout.add_child(Box::new(TestWidget::new(1, 50, 30)));

        let positions = layout.layout(Rect::new(0, 0, 200, 20));
        assert!(positions.is_empty());
    }

    #[test]
    fn test_horizontal_clips_after_first_child_when_second_exceeds_height() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 0;

        // Two children: first fits height (20), second exceeds it (30).
        layout.add_child(Box::new(TestWidget::new(1, 50, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 50, 30)));

        let positions = layout.layout(Rect::new(0, 0, 200, 25));
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], Rect::new(0, 0, 50, 20));
    }

    #[test]
    fn test_add_widget_sets_default_size() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.padding = 0;
        layout.config.spacing = 0;

        layout.add_widget(42, 0); // no widget ref → uses default_size 100x100

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn test_set_child_size_overrides_default() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.padding = 0;
        layout.config.spacing = 0;

        layout.add_widget(42, 0);
        layout.set_child_size(42, Size::new(50, 30));

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], Rect::new(0, 0, 50, 30));
    }

    #[test]
    fn test_preferred_size_with_children() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 10;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 60, 30)));

        // width = 40 + 10 + 60 = 110, height = max(20,30) = 30
        assert_eq!(layout.preferred_size(), Size::new(110, 30));
    }
}
