//! SplitButton widget with primary action and drop-down action list.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// One selectable action in a split button drop-down list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitAction {
    /// Stable action identifier.
    pub id: String,
    /// Display label.
    pub label: String,
}

impl SplitAction {
    /// Creates a split action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// SplitButton provides a default primary action plus selectable alternatives.
pub struct SplitButton {
    base: BaseWidget,
    text: String,
    actions: Vec<SplitAction>,
    primary_action_index: Option<usize>,
    highlighted_action_index: Option<usize>,
    menu_open: bool,
    pressed_primary: bool,
    pressed_arrow: bool,
    hovered_primary: bool,
    hovered_arrow: bool,
    arrow_width: u32,
    row_height: u32,
    /// Emitted when primary area triggers current action id.
    pub triggered: Signal1<String>,
    /// Emitted when a drop-down action is explicitly selected.
    pub action_selected: Signal1<String>,
    /// Emitted when menu open state changes.
    pub menu_toggled: Signal1<bool>,
}

impl SplitButton {
    /// Creates a split button.
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "SplitButton"),
            text: text.into(),
            actions: Vec::new(),
            primary_action_index: None,
            highlighted_action_index: None,
            menu_open: false,
            pressed_primary: false,
            pressed_arrow: false,
            hovered_primary: false,
            hovered_arrow: false,
            arrow_width: 22,
            row_height: 22,
            triggered: Signal1::new(),
            action_selected: Signal1::new(),
            menu_toggled: Signal1::new(),
        }
    }

    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }

    /// Replaces all actions.
    pub fn set_actions(&mut self, actions: Vec<SplitAction>) {
        self.actions = actions;
        self.primary_action_index = if self.actions.is_empty() { None } else { Some(0) };
        self.highlighted_action_index = self.primary_action_index;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all actions.
    pub fn actions(&self) -> &[SplitAction] {
        &self.actions
    }

    /// Adds one action and returns its index.
    pub fn add_action(&mut self, action: SplitAction) -> usize {
        let index = self.actions.len();
        self.actions.push(action);
        if self.primary_action_index.is_none() {
            self.primary_action_index = Some(index);
            self.highlighted_action_index = Some(index);
        }
        self.base.request_layout();
        self.base.request_redraw();
        index
    }

    /// Returns current primary action index.
    pub fn primary_action_index(&self) -> Option<usize> {
        self.primary_action_index.filter(|index| *index < self.actions.len())
    }

    /// Returns whether menu is open.
    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Returns highlighted action index when menu is open.
    pub fn highlighted_action_index(&self) -> Option<usize> {
        self.highlighted_action_index.filter(|index| *index < self.actions.len())
    }

    /// Sets menu row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        self.row_height = row_height.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns menu row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Triggers primary action.
    pub fn trigger_primary(&mut self) -> bool {
        let Some(index) = self.primary_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        self.triggered.emit(action.id.clone());
        true
    }

    /// Opens menu.
    pub fn open_menu(&mut self) {
        if self.menu_open {
            return;
        }
        self.menu_open = true;
        self.highlighted_action_index = self.primary_action_index();
        self.menu_toggled.emit(true);
        self.base.request_redraw();
    }

    /// Closes menu.
    pub fn close_menu(&mut self) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.menu_toggled.emit(false);
        self.base.request_redraw();
    }

    /// Toggles menu visibility.
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// Selects highlighted drop-down action as primary and emits action_selected.
    pub fn select_highlighted_action(&mut self) -> bool {
        let Some(index) = self.highlighted_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        self.primary_action_index = Some(index);
        self.action_selected.emit(action.id.clone());
        self.close_menu();
        true
    }

    /// Moves highlighted action in menu by signed delta.
    pub fn move_highlight(&mut self, delta: isize) {
        if self.actions.is_empty() {
            self.highlighted_action_index = None;
            return;
        }

        let current = self.highlighted_action_index.unwrap_or(0) as isize;
        let max = self.actions.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.highlighted_action_index = Some(next);
        self.base.request_redraw();
    }

    fn primary_rect(&self) -> Rect {
        let rect = self.geometry();
        let primary_width = rect.width.saturating_sub(self.arrow_width);
        Rect::new(rect.x, rect.y, primary_width, rect.height)
    }

    fn arrow_rect(&self) -> Rect {
        let rect = self.geometry();
        let arrow_x = rect.x + rect.width as i32 - self.arrow_width as i32;
        Rect::new(arrow_x, rect.y, self.arrow_width, rect.height)
    }

    fn menu_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x,
            rect.y + rect.height as i32,
            rect.width,
            self.row_height.saturating_mul(self.actions.len() as u32),
        )
    }

    fn action_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() {
            return None;
        }
        let menu = self.menu_rect();
        let y = menu.y + index as i32 * self.row_height as i32;
        Some(Rect::new(menu.x, y, menu.width, self.row_height))
    }

    fn hit_primary(&self, pos: Point) -> bool {
        let rect = self.primary_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_arrow(&self, pos: Point) -> bool {
        let rect = self.arrow_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_menu_index(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let menu = self.menu_rect();
        if pos.x < menu.x
            || pos.x >= menu.x + menu.width as i32
            || pos.y < menu.y
            || pos.y >= menu.y + menu.height as i32
        {
            return None;
        }
        let index = ((pos.y - menu.y) / self.row_height as i32) as usize;
        (index < self.actions.len()).then_some(index)
    }
}

impl Widget for SplitButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for SplitButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_primary = self.hit_primary(*pos);
                self.hovered_arrow = self.hit_arrow(*pos);
                if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                }
            }
            Event::MouseLeave { .. } => {
                self.hovered_primary = false;
                self.hovered_arrow = false;
                self.pressed_primary = false;
                self.pressed_arrow = false;
            }
            Event::MousePress { pos, button: 1 } => {
                if self.hit_primary(*pos) {
                    self.pressed_primary = true;
                } else if self.hit_arrow(*pos) {
                    self.pressed_arrow = true;
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                } else {
                    self.close_menu();
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                if self.pressed_primary {
                    self.pressed_primary = false;
                    if self.hit_primary(*pos) {
                        let _ = self.trigger_primary();
                    }
                } else if self.pressed_arrow {
                    self.pressed_arrow = false;
                    if self.hit_arrow(*pos) {
                        self.toggle_menu();
                    }
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                    let _ = self.select_highlighted_action();
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 | 32 => {
                    if self.menu_open {
                        let _ = self.select_highlighted_action();
                    } else {
                        let _ = self.trigger_primary();
                    }
                }
                40 => {
                    if !self.menu_open {
                        self.open_menu();
                    } else {
                        self.move_highlight(1);
                    }
                }
                38 if self.menu_open => {
                    self.move_highlight(-1);
                }
                27 => {
                    self.close_menu();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SplitButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(244, 246, 250));
        context.draw_rect(rect, Color::from_rgb(188, 194, 206));

        let primary = self.primary_rect();
        let arrow = self.arrow_rect();

        let primary_bg = if self.pressed_primary {
            Color::from_rgb(192, 218, 247)
        } else if self.hovered_primary {
            Color::from_rgb(216, 232, 250)
        } else {
            Color::from_rgb(244, 246, 250)
        };
        context.fill_rect(primary, primary_bg);

        let arrow_bg = if self.pressed_arrow || self.menu_open {
            Color::from_rgb(192, 218, 247)
        } else if self.hovered_arrow {
            Color::from_rgb(216, 232, 250)
        } else {
            Color::from_rgb(236, 239, 245)
        };
        context.fill_rect(arrow, arrow_bg);

        context.draw_line(
            Point::new(arrow.x, arrow.y),
            Point::new(arrow.x, arrow.y + arrow.height as i32),
            Color::from_rgb(188, 194, 206),
        );

        context.draw_text(
            Point::new(primary.x + 8, primary.y + primary.height as i32 / 2),
            &self.text,
            &Font::default(),
            Color::from_rgb(34, 45, 64),
        );

        context.draw_text(
            Point::new(arrow.x + (arrow.width as i32 / 2) - 3, arrow.y + arrow.height as i32 / 2),
            "v",
            &Font::default(),
            Color::from_rgb(64, 74, 88),
        );

        if self.menu_open {
            let menu = self.menu_rect();
            context.fill_rect(menu, Color::from_rgb(255, 255, 255));
            context.draw_rect(menu, Color::from_rgb(188, 194, 206));

            for index in 0..self.actions.len() {
                let Some(action_rect) = self.action_rect(index) else {
                    continue;
                };

                if self.highlighted_action_index == Some(index) {
                    context.fill_rect(action_rect, Color::from_rgb(218, 232, 250));
                }

                if let Some(action) = self.actions.get(index) {
                    context.draw_text(
                        Point::new(
                            action_rect.x + 8,
                            action_rect.y + action_rect.height as i32 / 2,
                        ),
                        &action.label,
                        &Font::default(),
                        Color::from_rgb(34, 45, 64),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_actions() -> Vec<SplitAction> {
        vec![
            SplitAction::new("run.default", "Run"),
            SplitAction::new("run.debug", "Run with Debug"),
            SplitAction::new("run.profile", "Run with Profile"),
        ]
    }

    #[test]
    fn primary_trigger_emits_default_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 28));
        split.set_actions(sample_actions());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(split.trigger_primary());
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.default".to_string()]);
    }

    #[test]
    fn keyboard_navigation_selects_action_from_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        split.action_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        split.handle_event(&Event::key_press(40, 0));
        assert!(split.menu_open());
        split.handle_event(&Event::key_press(40, 0));
        split.handle_event(&Event::key_press(13, 0));

        assert_eq!(split.primary_action_index(), Some(1));
        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.debug".to_string()]);
        assert!(!split.menu_open());
    }

    #[test]
    fn arrow_click_toggles_menu_and_mouse_selects_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        // Click arrow area to open menu.
        split.handle_event(&Event::mouse_press(172, 12, 1));
        split.handle_event(&Event::mouse_release(172, 12, 1));
        assert!(split.menu_open());

        // Click second row in menu.
        let menu_y = 28 + 24;
        split.handle_event(&Event::mouse_press(20, menu_y, 1));
        split.handle_event(&Event::mouse_release(20, menu_y, 1));

        assert_eq!(split.primary_action_index(), Some(1));
        assert!(!split.menu_open());
    }

    #[test]
    fn default_state() {
        let split = SplitButton::new("Run", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Run");
        assert!(split.actions().is_empty());
        assert_eq!(split.primary_action_index(), None);
        assert!(!split.menu_open());
    }

    #[test]
    fn set_text_get_text_roundtrip() {
        let mut split = SplitButton::new("Initial", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Initial");

        split.set_text("Updated");
        assert_eq!(split.text(), "Updated");

        split.set_text("");
        assert_eq!(split.text(), "");
    }

    #[test]
    fn add_action_adds_to_menu() {
        let mut split = SplitButton::new("Action", Rect::new(0, 0, 800, 600));
        assert_eq!(split.actions().len(), 0);

        let idx = split.add_action(SplitAction::new("act1", "Action 1"));
        assert_eq!(idx, 0);
        assert_eq!(split.actions().len(), 1);
        assert_eq!(split.primary_action_index(), Some(0));

        let idx = split.add_action(SplitAction::new("act2", "Action 2"));
        assert_eq!(idx, 1);
        assert_eq!(split.actions().len(), 2);

        assert_eq!(split.actions()[0].id, "act1");
        assert_eq!(split.actions()[1].id, "act2");
    }

    #[test]
    fn enable_disable_states() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A")]);

        // Enabled by default
        assert!(split.trigger_primary());

        // Disable widget
        split.base_mut().set_enabled(false);

        // After disabling, events should be ignored
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Direct call still works but event handler ignores
        split.handle_event(&Event::key_press(40, 0));
        let _got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!split.menu_open(), "disabled widget should not open menu via events");
    }

    #[test]
    fn trigger_with_no_actions() {
        let mut split = SplitButton::new("Empty", Rect::new(0, 0, 800, 600));
        // No actions added yet
        assert!(!split.trigger_primary());

        // open_menu with no actions still opens an empty menu (code doesn't guard)
        split.open_menu();
        assert!(split.menu_open());

        // Close menu
        split.close_menu();
        assert!(!split.menu_open());
    }

    #[test]
    fn menu_toggle_signal_emission() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A"), SplitAction::new("b", "B")]);

        let emitted = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = emitted.clone();
        split.menu_toggled.connect(move |state| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*state.as_ref());
            }
        });

        split.open_menu();
        assert!(split.menu_open());

        split.close_menu();
        assert!(!split.menu_open());

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }
}
