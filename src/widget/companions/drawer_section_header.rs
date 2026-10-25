// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DrawerSectionHeader` — the small heading row that groups a drawer's items (COMP-A04).
//!
//! `NavigationDrawer` gained section rows in N-06. This companion is the same row as a
//! standalone control, so an adaptive scaffold's drawer slot, a bottom sheet or a host's
//! own list can paint the identical heading without depending on the drawer.

use crate::compat::String;
use crate::core::{HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_draw_bridge, impl_widget_property_hooks, property_names_of};

/// Height of a heading row, in logical pixels.
const SECTION_HEIGHT: u32 = 28;

/// A non-selectable section heading for a list or a drawer.
pub struct DrawerSectionHeader {
    base: BaseWidget,
    label: String,
    /// The painted spelling, precomputed so a repaint does not allocate for `uppercase`.
    displayed: String,
    inset: u32,
    uppercase: bool,
}

impl DrawerSectionHeader {
    /// Creates a heading with an empty label.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "DrawerSectionHeader"),
            label: String::new(),
            displayed: String::new(),
            inset: 16,
            uppercase: false,
        }
    }

    /// Returns the label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets the label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.label == label {
            return;
        }
        self.label = label;
        self.refresh_displayed();
        self.base.request_redraw();
    }

    /// Returns the leading inset.
    pub fn inset(&self) -> u32 {
        self.inset
    }

    /// Sets the leading inset, in logical pixels.
    pub fn set_inset(&mut self, inset: u32) {
        if self.inset == inset {
            return;
        }
        self.inset = inset;
        self.base.request_redraw();
    }

    /// Whether the label is painted in upper case.
    pub fn uppercase(&self) -> bool {
        self.uppercase
    }

    /// Sets whether the label is painted in upper case.
    pub fn set_uppercase(&mut self, uppercase: bool) {
        if self.uppercase == uppercase {
            return;
        }
        self.uppercase = uppercase;
        self.refresh_displayed();
        self.base.request_redraw();
    }

    /// Rebuilds the painted spelling once, at the setter.
    fn refresh_displayed(&mut self) {
        self.displayed.clear();
        if self.uppercase {
            self.displayed.push_str(&self.label.to_uppercase());
        } else {
            self.displayed.push_str(&self.label);
        }
    }
}

impl EventHandler for DrawerSectionHeader {
    fn handle_event(&mut self, event: &Event) {
        // A heading is display-only: it owns no gesture, but it still reports hover/focus to the
        // base so a host's generic state channel sees the same facts as any other control.
        self.base.handle_event(event);
    }
}

impl Draw for DrawerSectionHeader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let surface = style.background_color.unwrap_or(crate::core::Color::WHITE);
        let ink = style.text_color.unwrap_or(crate::core::Color::BLACK);
        // The drawer's own pair: a dimmed label ink and the hairline divider tone, so a
        // section heading reads as part of the same list.
        let divider = ink.with_alpha_f32(0.1);
        let color = ink.blend(&surface, 0.45);
        let font = role_font(style, 12.0);
        let line = context.text_line(rect, &font);
        let x = rect.x + self.inset as i32;
        let width = (rect.x + rect.width as i32 - x).max(0) as u32;
        if width > 0 {
            context.draw_text_fitted_cow(
                Rect::new(x, line.y, width, line.height.max(1)),
                &self.displayed,
                &font,
                color,
                HorizontalAlignment::Left,
            );
        }
        // A hairline under the heading ties the group together without a second colour.
        if width > 0 && rect.height > 0 {
            context.fill_rect(Rect::new(x, rect.y + rect.height as i32 - 1, width, 1), divider);
        }
    }
}

impl Widget for DrawerSectionHeader {
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
        Size::new(160, SECTION_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for DrawerSectionHeader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "label" => Ok(CapabilityValue::String(self.label.clone())),
            "inset" => Ok(CapabilityValue::UInt(self.inset as u64)),
            "uppercase" => Ok(CapabilityValue::Bool(self.uppercase)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "label" => {
                let CapabilityValue::String(label) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_label(label);
                Ok(())
            }
            "inset" => {
                let CapabilityValue::UInt(inset) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_inset(inset.min(u64::from(u32::MAX)) as u32);
                Ok(())
            }
            "uppercase" => {
                let CapabilityValue::Bool(uppercase) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_uppercase(uppercase);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["label", "inset", "uppercase", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_can_be_uppercased_without_reallocating_per_frame() {
        let mut header = DrawerSectionHeader::new(Rect::new(0, 0, 160, 28));
        header.set_label("Library");
        assert_eq!(header.label(), "Library");
        header.set_uppercase(true);
        assert!(header.uppercase());
        assert_eq!(header.displayed, "LIBRARY");
        header.set_uppercase(false);
        assert_eq!(header.displayed, "Library");
    }

    #[test]
    fn the_inset_is_a_bounded_property() {
        let mut header = DrawerSectionHeader::new(Rect::new(0, 0, 160, 28));
        assert_eq!(header.inset(), 16);
        header.set_inset(24);
        assert_eq!(header.inset(), 24);
    }
}
