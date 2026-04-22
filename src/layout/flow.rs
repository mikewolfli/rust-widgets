use crate::core::{Rect, Size};
use crate::widget::Widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    Horizontal,
    Vertical,
}

impl Default for FlowDirection {
    fn default() -> Self {
        Self::Horizontal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowAlignment {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

impl Default for FlowAlignment {
    fn default() -> Self {
        Self::Start
    }
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
    children: Vec<Box<dyn Widget>>,
}

impl FlowLayout {
    pub fn new() -> Self {
        Self {
            config: FlowLayoutConfig::default(),
            children: Vec::new(),
        }
    }

    pub fn with_config(config: FlowLayoutConfig) -> Self {
        Self {
            config,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            Some(self.children.remove(index))
        } else {
            None
        }
    }

    pub fn clear_children(&mut self) {
        self.children.clear();
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn layout(&mut self, available_rect: Rect) -> Vec<Rect> {
        let content_rect = Rect::new(
            available_rect.x + self.config.padding,
            available_rect.y + self.config.padding,
            available_rect
                .width
                .saturating_sub(2 * self.config.padding as u32),
            available_rect
                .height
                .saturating_sub(2 * self.config.padding as u32),
        );

        match self.config.direction {
            FlowDirection::Horizontal => self.layout_horizontal(&content_rect),
            FlowDirection::Vertical => self.layout_vertical(&content_rect),
        }
    }

    fn layout_horizontal(&mut self, content_rect: &Rect) -> Vec<Rect> {
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
                break;
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

    fn layout_vertical(&mut self, content_rect: &Rect) -> Vec<Rect> {
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
                break;
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
                    let spacing = (available - total_size) / (positions.len() as i32 - 1);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_layout_horizontal() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 10;

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }

    #[test]
    fn test_flow_layout_vertical() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Vertical;
        layout.config.spacing = 10;

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }

    #[test]
    fn test_flow_layout_alignment() {
        let mut layout = FlowLayout::new();
        layout.config.alignment = FlowAlignment::Center;

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }
}
