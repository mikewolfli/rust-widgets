//! Box layout manager — arranges items in a single row or column.
use crate::core::{ObjectId, Rect};
use super::{Layout, LayoutConstraints, Orientation, SizePolicy};
struct BoxLayoutItem {
    widget_id: Option<ObjectId>,
    stretch: u32,
    constraints: LayoutConstraints,
    policy: SizePolicy,
}
/// Linear layout that arranges items in one direction.
pub struct BoxLayout {
    orientation: Orientation,
    spacing: u32,
    margin: u32,
    items: Vec<BoxLayoutItem>,
}
impl BoxLayout {
    /// Create a box layout with orientation, spacing and margin.
    pub fn new(orientation: Orientation, spacing: u32, margin: u32) -> Self {
        Self { orientation, spacing, margin, items: Vec::new() }
    }
    /// Returns layout orientation.
    pub fn orientation(&self) -> Orientation { self.orientation }
    /// Returns inter-item spacing.
    pub fn spacing(&self) -> u32 { self.spacing }
    /// Updates inter-item spacing.
    pub fn set_spacing(&mut self, spacing: u32) { self.spacing = spacing; }
    /// Returns outer margin.
    pub fn margin(&self) -> u32 { self.margin }
    /// Updates outer margin.
    pub fn set_margin(&mut self, margin: u32) { self.margin = margin; }
    /// Returns number of managed items (widgets + spacers).
    pub fn item_count(&self) -> usize { self.items.len() }
    /// Adds an empty spacer item with the provided stretch factor.
    pub fn add_spacer(&mut self, stretch: u32) {
        self.items.push(BoxLayoutItem {
            widget_id: None,
            stretch: stretch.max(1),
            constraints: LayoutConstraints::new(0, None),
            policy: SizePolicy::Expanding,
        });
    }
    /// Sets size constraints for an existing widget item.
    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) {
        if let Some(item) = self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            item.constraints = constraints;
        }
    }
    /// Sets size policy for an existing widget item.
    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) {
        if let Some(item) = self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            item.policy = policy;
        }
    }
    fn allocate_major_lengths(&self, primary: u32) -> Vec<u32> {
        if self.items.is_empty() { return Vec::new(); }
        let total_stretch: u32 = self.items.iter().map(|item| item.stretch).sum::<u32>().max(1);
        let mut assigned = Vec::with_capacity(self.items.len());
        for item in &self.items {
            let mut major = if item.policy == SizePolicy::Fixed {
                item.constraints.max.unwrap_or(item.constraints.min)
            } else {
                primary.saturating_mul(item.stretch) / total_stretch
            };
            major = major.max(item.constraints.min);
            if let Some(max) = item.constraints.max {
                major = major.min(max.max(item.constraints.min));
            }
            assigned.push(major);
        }
        let mut total_assigned: u32 = assigned.iter().sum();
        while total_assigned < primary {
            let mut grew = false;
            for (index, item) in self.items.iter().enumerate() {
                if total_assigned >= primary { break; }
                let max_allowed = item.constraints.max.unwrap_or(u32::MAX).max(item.constraints.min);
                if assigned[index] < max_allowed {
                    assigned[index] = assigned[index].saturating_add(1);
                    total_assigned = total_assigned.saturating_add(1);
                    grew = true;
                }
            }
            if !grew { break; }
        }
        while total_assigned > primary {
            let mut shrank = false;
            for (index, _item) in self.items.iter().enumerate().rev() {
                if total_assigned <= primary { break; }
                let min_allowed = self.items[index].constraints.min;
                if assigned[index] > min_allowed {
                    assigned[index] = assigned[index].saturating_sub(1);
                    total_assigned = total_assigned.saturating_sub(1);
                    shrank = true;
                }
            }
            if !shrank { break; }
        }
        assigned
    }
}
impl Layout for BoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.items.push(BoxLayoutItem {
            widget_id: Some(widget_id),
            stretch: stretch.max(1),
            constraints: LayoutConstraints::new(0, None),
            policy: SizePolicy::Expanding,
        });
    }
    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|item| item.widget_id != Some(widget_id));
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.items.is_empty() { return; }
        let gaps = (self.items.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width,
            Orientation::Vertical => rect.height,
        }.saturating_sub(self.margin * 2).saturating_sub(gaps * self.spacing);
        let majors = self.allocate_major_lengths(primary);
        let mut cursor_x = rect.x + self.margin as i32;
        let mut cursor_y = rect.y + self.margin as i32;
        for (index, item) in self.items.iter().enumerate() {
            let major = majors.get(index).copied().unwrap_or(0);
            let child_rect = match self.orientation {
                Orientation::Horizontal => Rect::new(cursor_x, cursor_y, major, rect.height.saturating_sub(self.margin * 2)),
                Orientation::Vertical => Rect::new(cursor_x, cursor_y, rect.width.saturating_sub(self.margin * 2), major),
            };
            if let Some(widget_id) = item.widget_id { widgets(widget_id, child_rect); }
            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + self.spacing) as i32,
                Orientation::Vertical => cursor_y += (major + self.spacing) as i32,
            }
        }
    }
}
/// Horizontal box layout with explicit naming parity.
pub struct HBoxLayout { inner: BoxLayout }
impl HBoxLayout {
    pub fn new(spacing: u32, margin: u32) -> Self { Self { inner: BoxLayout::new(Orientation::Horizontal, spacing, margin) } }
    pub fn add_spacer(&mut self, stretch: u32) { self.inner.add_spacer(stretch); }
    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) { self.inner.set_constraints(widget_id, constraints); }
    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) { self.inner.set_size_policy(widget_id, policy); }
    pub fn spacing(&self) -> u32 { self.inner.spacing() }
    pub fn set_spacing(&mut self, spacing: u32) { self.inner.set_spacing(spacing); }
    pub fn margin(&self) -> u32 { self.inner.margin() }
    pub fn set_margin(&mut self, margin: u32) { self.inner.set_margin(margin); }
    pub fn item_count(&self) -> usize { self.inner.item_count() }
}
impl Layout for HBoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) { self.inner.add_widget(widget_id, stretch); }
    fn remove_widget(&mut self, widget_id: ObjectId) { self.inner.remove_widget(widget_id); }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) { self.inner.update(rect, widgets); }
}
/// Vertical box layout with explicit naming parity.
pub struct VBoxLayout { inner: BoxLayout }
impl VBoxLayout {
    pub fn new(spacing: u32, margin: u32) -> Self { Self { inner: BoxLayout::new(Orientation::Vertical, spacing, margin) } }
    pub fn add_spacer(&mut self, stretch: u32) { self.inner.add_spacer(stretch); }
    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) { self.inner.set_constraints(widget_id, constraints); }
    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) { self.inner.set_size_policy(widget_id, policy); }
    pub fn spacing(&self) -> u32 { self.inner.spacing() }
    pub fn set_spacing(&mut self, spacing: u32) { self.inner.set_spacing(spacing); }
    pub fn margin(&self) -> u32 { self.inner.margin() }
    pub fn set_margin(&mut self, margin: u32) { self.inner.set_margin(margin); }
    pub fn item_count(&self) -> usize { self.inner.item_count() }
}
impl Layout for VBoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) { self.inner.add_widget(widget_id, stretch); }
    fn remove_widget(&mut self, widget_id: ObjectId) { self.inner.remove_widget(widget_id); }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) { self.inner.update(rect, widgets); }
}
