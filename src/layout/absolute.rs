use crate::core::{Rect, Size};
use crate::widget::Widget;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsolutePosition {
    pub x: i32,
    pub y: i32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub anchor: Anchor,
}
impl AbsolutePosition {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            width: None,
            height: None,
            anchor: Anchor::TopLeft,
        }
    }
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
    pub fn with_anchor(mut self, anchor: Anchor, offset_x: i32, offset_y: i32) -> Self {
        self.anchor = anchor;
        self.x = offset_x;
        self.y = offset_y;
        self
    }
    pub fn with_anchor_only(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }
    pub fn to_rect(&self, _parent_size: Size, child_size: Size) -> Rect {
        let width = self.width.unwrap_or(child_size.width);
        let height = self.height.unwrap_or(child_size.height);
        let (x, y) = match self.anchor {
            Anchor::TopLeft => (self.x, self.y),
            Anchor::TopCenter => {
                let x = self.x - (width as i32) / 2;
                (x, self.y)
            }
            Anchor::TopRight => {
                let x = self.x - width as i32;
                (x, self.y)
            }
            Anchor::CenterLeft => {
                let y = self.y - (height as i32) / 2;
                (self.x, y)
            }
            Anchor::Center => {
                let x = self.x - (width as i32) / 2;
                let y = self.y - (height as i32) / 2;
                (x, y)
            }
            Anchor::CenterRight => {
                let x = self.x - width as i32;
                let y = self.y - (height as i32) / 2;
                (x, y)
            }
            Anchor::BottomLeft => {
                let y = self.y - height as i32;
                (self.x, y)
            }
            Anchor::BottomCenter => {
                let x = self.x - (width as i32) / 2;
                let y = self.y - height as i32;
                (x, y)
            }
            Anchor::BottomRight => {
                let x = self.x - width as i32;
                let y = self.y - height as i32;
                (x, y)
            }
        };
        Rect::new(x, y, width, height)
    }
}
impl Default for AbsolutePosition {
    fn default() -> Self {
        Self::new(0, 0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}
/// Alias for Anchor to match test expectations
pub use Anchor as AbsoluteAnchor;
impl Default for Anchor {
    fn default() -> Self {
        Self::TopLeft
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Constraint {
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub aspect_ratio: Option<f32>,
}
impl Constraint {
    pub fn new() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            aspect_ratio: None,
        }
    }
    pub fn with_min_width(mut self, width: u32) -> Self {
        self.min_width = Some(width);
        self
    }
    pub fn with_max_width(mut self, width: u32) -> Self {
        self.max_width = Some(width);
        self
    }
    pub fn with_min_height(mut self, height: u32) -> Self {
        self.min_height = Some(height);
        self
    }
    pub fn with_max_height(mut self, height: u32) -> Self {
        self.max_height = Some(height);
        self
    }
    pub fn with_aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }
    pub fn apply(&self, size: Size) -> Size {
        let mut width = size.width;
        let mut height = size.height;
        if let Some(min) = self.min_width {
            width = width.max(min);
        }
        if let Some(max) = self.max_width {
            width = width.min(max);
        }
        if let Some(min) = self.min_height {
            height = height.max(min);
        }
        if let Some(max) = self.max_height {
            height = height.min(max);
        }
        if let Some(ratio) = self.aspect_ratio {
            let current_ratio = width as f32 / height as f32;
            if current_ratio > ratio {
                width = (height as f32 * ratio) as u32;
            } else {
                height = (width as f32 / ratio) as u32;
            }
        }
        Size::new(width, height)
    }
}
impl Default for Constraint {
    fn default() -> Self {
        Self::new()
    }
}
pub struct AbsoluteLayout {
    children: Vec<(Box<dyn Widget>, AbsolutePosition, Option<Constraint>)>,
}
impl AbsoluteLayout {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
    pub fn add_child(&mut self, child: Box<dyn Widget>, position: AbsolutePosition) {
        self.children.push((child, position, None));
    }
    pub fn add_child_with_constraint(
        &mut self,
        child: Box<dyn Widget>,
        position: AbsolutePosition,
        constraint: Constraint,
    ) {
        self.children.push((child, position, Some(constraint)));
    }
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            Some(self.children.remove(index).0)
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
    pub fn layout(&self, parent_rect: Rect) -> Vec<Rect> {
        let parent_size = parent_rect.size();
        let mut positions = Vec::new();
        for (child, position, constraint) in &self.children {
            let child_size = child.size_hint();
            let constrained_size = if let Some(constraint) = constraint {
                constraint.apply(child_size)
            } else {
                child_size
            };
            let rect = position.to_rect(parent_size, constrained_size);
            positions.push(rect);
        }
        positions
    }
    /// Calculate positions for given sizes (for testing)
    pub fn calculate_positions(
        &self,
        container: &Rect,
        positions: &[AbsolutePosition],
        sizes: &[Size],
    ) -> Vec<Rect> {
        let mut rects = Vec::new();
        let parent_size = container.size();
        for (position, size) in positions.iter().zip(sizes.iter()) {
            let rect = position.to_rect(parent_size, *size);
            rects.push(rect);
        }
        rects
    }
    pub fn set_position(&mut self, index: usize, position: AbsolutePosition) -> bool {
        if let Some((_, pos, _)) = self.children.get_mut(index) {
            *pos = position;
            true
        } else {
            false
        }
    }
    pub fn set_constraint(&mut self, index: usize, constraint: Constraint) -> bool {
        if let Some((_, _, cons)) = self.children.get_mut(index) {
            *cons = Some(constraint);
            true
        } else {
            false
        }
    }
    pub fn get_position(&self, index: usize) -> Option<&AbsolutePosition> {
        self.children.get(index).map(|(_, pos, _)| pos)
    }
    pub fn get_constraint(&self, index: usize) -> Option<&Constraint> {
        self.children
            .get(index)
            .and_then(|(_, _, cons)| cons.as_ref())
    }
}
impl Default for AbsoluteLayout {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_absolute_position() {
        let pos = AbsolutePosition::new(100, 100)
            .with_size(50, 50)
            .with_anchor_only(Anchor::Center);
        let rect = pos.to_rect(Size::new(200, 200), Size::new(50, 50));
        assert_eq!(rect.x, 75);
        assert_eq!(rect.y, 75);
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 50);
    }
    #[test]
    fn test_constraint() {
        let constraint = Constraint::new()
            .with_min_width(50)
            .with_max_width(100)
            .with_min_height(50)
            .with_max_height(100);
        let size = constraint.apply(Size::new(200, 200));
        assert_eq!(size.width, 100);
        assert_eq!(size.height, 100);
        let size = constraint.apply(Size::new(30, 30));
        assert_eq!(size.width, 50);
        assert_eq!(size.height, 50);
    }
    #[test]
    fn test_aspect_ratio() {
        let constraint = Constraint::new().with_aspect_ratio(2.0);
        let size = constraint.apply(Size::new(200, 100));
        assert_eq!(size.width, 200);
        assert_eq!(size.height, 100);
        let size = constraint.apply(Size::new(200, 200));
        assert_eq!(size.width, 200);
        assert_eq!(size.height, 100);
    }
}
