// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
#![cfg(full_widgets)]

//! Regression: every control with a keyboard arm must be reachable in the tab order.
//!
//! `Widget::is_focusable` defaults to `WidgetKind::wants_keyboard_focus()`. A control whose kind
//! is not on that list and that does not override `is_focusable` can never receive focus, so its
//! `Event::KeyPress` arm is dead code. These ten were exactly that (U3-01); `DrumColumn` had
//! already been fixed and is included so the set stays pinned.

use rust_widgets::core::Rect;
use rust_widgets::widget::container_widgets::accordion::Accordion;
use rust_widgets::widget::container_widgets::carousel::Carousel;
use rust_widgets::widget::container_widgets::tabwidget::TabWidget;
use rust_widgets::widget::container_widgets::toolbox::ToolBox;
use rust_widgets::widget::display_widgets::roller::Roller;
use rust_widgets::widget::misc_widgets::drum_column::DrumColumn;
use rust_widgets::widget::misc_widgets::mobile_date_picker::MobileDatePicker;
use rust_widgets::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar;
use rust_widgets::widget::nav_widgets::navigation_drawer::NavigationDrawer;
use rust_widgets::widget::nav_widgets::pagination::Pagination;
use rust_widgets::widget::Widget;

fn geometry() -> Rect {
    Rect::new(0, 0, 320, 240)
}

#[test]
fn controls_with_key_arms_are_keyboard_focusable() {
    assert!(Roller::new(vec!["a".to_string(), "b".to_string()], geometry()).is_focusable());
    assert!(Pagination::new(geometry()).is_focusable());
    assert!(Carousel::new(geometry()).is_focusable());
    assert!(DrumColumn::new(geometry()).is_focusable());
    assert!(TabWidget::new(geometry()).is_focusable());
    assert!(ToolBox::new(geometry()).is_focusable());
    assert!(MobileDatePicker::new(geometry()).is_focusable());
    assert!(NavigationDrawer::new(geometry()).is_focusable());
    assert!(BottomNavigationBar::new(geometry()).is_focusable());
    assert!(Accordion::new(geometry()).is_focusable());
}
