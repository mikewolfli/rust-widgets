// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ModalHost` — the stack that decides which modal is on top and how a dismissal is routed.
//!
//! # What this is
//!
//! Every modal control in this crate knows how to draw its own scrim and how to reject
//! itself, but none of them knows which of several nested modals is the *current* one: a
//! confirmation opened above a file dialog and a file dialog opened above a settings panel
//! are three independent widgets, so "Escape closes the top one" had no owner. This is that
//! owner (COMP-G02).
//!
//! The host does **not** perform the close: [`dismiss_requested`](ModalHost::dismiss_requested)
//! states the request, and the host application decides whether the top modal may close. That
//! split is deliberate — a widget that closed another widget would bypass the latter's own
//! guards (an unsaved-changes question, for example).

use crate::compat::Vec;
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The modal stack: which modals are open, and which one is on top.
pub struct ModalHost {
    base: BaseWidget,
    /// The open modals, bottom first.
    stack: Vec<ObjectId>,
    /// Whether the host paints the page-dimming layer.
    scrim_enabled: bool,
    /// The scrim's tint; `None` reads the theme's `LayerColor::Scrim` token.
    scrim_color: Option<Color>,
    /// Emitted with the new depth whenever the stack changes.
    pub modal_changed: Signal1<usize>,
    /// Emitted when Escape or a press outside the top modal asks the host to dismiss it.
    pub dismiss_requested: GenericSignal,
}

impl ModalHost {
    /// Creates an empty host with the scrim enabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "ModalHost"),
            stack: Vec::new(),
            scrim_enabled: true,
            scrim_color: None,
            modal_changed: Signal1::new(),
            dismiss_requested: GenericSignal::new(),
        }
    }

    /// Pushes `id` as the new top modal.
    ///
    /// Pushing an id that is already open moves it to the top rather than duplicating it: one
    /// modal is one entry, and a second push describes a re-raise.
    pub fn push(&mut self, id: ObjectId) {
        if let Some(position) = self.stack.iter().position(|existing| *existing == id) {
            if position + 1 == self.stack.len() {
                return;
            }
            self.stack.remove(position);
        }
        self.stack.push(id);
        self.modal_changed.emit(self.stack.len());
        self.base.request_redraw();
    }

    /// Pops the top modal, returning it.
    pub fn pop(&mut self) -> Option<ObjectId> {
        let popped = self.stack.pop()?;
        self.modal_changed.emit(self.stack.len());
        self.base.request_redraw();
        Some(popped)
    }

    /// Removes `id` wherever it sits, returning whether it was open.
    pub fn remove(&mut self, id: ObjectId) -> bool {
        let Some(position) = self.stack.iter().position(|existing| *existing == id) else {
            return false;
        };
        self.stack.remove(position);
        self.modal_changed.emit(self.stack.len());
        self.base.request_redraw();
        true
    }

    /// Whether `id` is the top modal.
    pub fn is_modal_top(&self, id: ObjectId) -> bool {
        self.stack.last().copied() == Some(id)
    }

    /// The top modal, if any.
    pub fn top_modal(&self) -> Option<ObjectId> {
        self.stack.last().copied()
    }

    /// The stack, bottom first.
    pub fn stack(&self) -> &[ObjectId] {
        &self.stack
    }

    /// The number of open modals.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Whether the page-dimming layer is painted.
    pub fn scrim_enabled(&self) -> bool {
        self.scrim_enabled
    }

    /// Enables or disables the scrim and repaints.
    pub fn set_scrim_enabled(&mut self, enabled: bool) {
        if self.scrim_enabled == enabled {
            return;
        }
        self.scrim_enabled = enabled;
        self.base.request_redraw();
    }

    /// The scrim tint, or `None` when the theme token is used.
    pub fn scrim_color(&self) -> Option<Color> {
        self.scrim_color
    }

    /// Sets the scrim tint and repaints.
    pub fn set_scrim_color(&mut self, color: Option<Color>) {
        if self.scrim_color == color {
            return;
        }
        self.scrim_color = color;
        self.base.request_redraw();
    }
}

impl Widget for ModalHost {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(320, 240)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "modal_changed" => {
                Some(EventSignalRef::mapped("modal_changed", &self.modal_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "dismiss_requested" => {
                Some(EventSignalRef::unit("dismiss_requested", &self.dismiss_requested))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ModalHost {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "modal_depth" => Ok(CapabilityValue::UInt(self.stack.len() as u64)),
            "top_modal" => Ok(match self.top_modal() {
                Some(id) => CapabilityValue::UInt(id),
                None => CapabilityValue::Null,
            }),
            "scrim_enabled" => Ok(CapabilityValue::Bool(self.scrim_enabled)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scrim_enabled" => {
                self.set_scrim_enabled(expect_bool(value)?);
                Ok(())
            }
            "modal_depth" | "top_modal" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["modal_depth", "top_modal", "scrim_enabled", BASE_PROPERTY_NAMES]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "pop" => {
                self.pop();
                Ok(())
            }
            "push" | "remove" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ModalHost {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // Escape asks the host to dismiss the top modal. The host does not close it
            // itself: the modal owns the guards that decide whether closing is allowed.
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE && !self.stack.is_empty() => {
                self.dismiss_requested.emit();
            }
            Event::MousePress { pos, button: 1, .. }
                if !self.stack.is_empty() && !self.modal_area_contains(*pos) =>
            {
                self.dismiss_requested.emit();
            }
            _ => {}
        }
    }
}

impl ModalHost {
    /// Whether `pos` lies inside this host's own area.
    ///
    /// A press outside the host — or outside the window the modal is attached to — is the
    /// scrim-click gesture that a modal host forwards as a dismissal request.
    fn modal_area_contains(&self, pos: Point) -> bool {
        self.base.geometry().contains_point(pos)
    }
}

impl Draw for ModalHost {
    fn draw(&mut self, context: &mut RenderContext) {
        if self.stack.is_empty() || !self.scrim_enabled {
            return;
        }
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let color = self.scrim_color.unwrap_or_else(|| {
            crate::style::layer_color(crate::style::LayerColor::Scrim)
                .unwrap_or(Color::rgba(0, 0, 0, 82))
        });
        context.fill_rect(rect, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_push_raises_one_entry_instead_of_duplicating_it() {
        let mut host = ModalHost::new(Rect::new(0, 0, 400, 300));
        host.push(7);
        host.push(9);
        host.push(7);
        assert_eq!(host.stack(), &[9, 7]);
        assert!(host.is_modal_top(7));
        assert_eq!(host.top_modal(), Some(7));
        assert_eq!(host.pop(), Some(7));
        assert_eq!(host.depth(), 1);
        assert_eq!(host.pop(), Some(9));
        assert_eq!(host.pop(), None);
    }
}
